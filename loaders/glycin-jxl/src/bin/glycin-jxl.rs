#![allow(clippy::large_enum_variant)]

use std::io::{Cursor, Read, Write};
use std::mem::MaybeUninit;
use std::sync::Mutex;

use glycin_utils::*;
use jpegxl_rs::image::ToDynamic;
use jpegxl_sys::codestream_header::*;
use jpegxl_sys::decode::*;
use jpegxl_sys::types::{JxlBool, JxlBoxType};

init_main_loader!(ImgDecoder::default());

type InitData = Option<(Vec<u8>, Option<Vec<u8>>)>;

#[derive(Default)]
pub struct ImgDecoder {
    pub decoder: Mutex<InitData>,
}

impl LoaderImplementation for ImgDecoder {
    fn init(
        &self,
        mut stream: UnixStream,
        _mime_type: String,
        _details: InitializationDetails,
    ) -> Result<ImageInfo, ProcessError> {
        let mut data = Vec::new();
        stream.read_to_end(&mut data).expected_error()?;
        let (info, iccp, exif) = basic_info(&data);

        let info = info.expected_error()?;

        let mut image_info = ImageInfo::new(info.xsize, info.ysize);
        image_info.details.format_name = Some(String::from("JPEG XL"));
        image_info.details.exif = exif
            .map(BinaryData::from_data)
            .transpose()
            .expected_error()?;
        image_info.details.transformations_applied = true;

        *self.decoder.lock().unwrap() = Some((data, iccp));

        Ok(image_info)
    }

    fn frame(&self, _frame_request: FrameRequest) -> Result<Frame, ProcessError> {
        let (data, iccp) = std::mem::take(&mut *self.decoder.lock().unwrap()).expected_error()?;

        let decoder = jpegxl_rs::decode::decoder_builder()
            .build()
            .expected_error()?;

        let image = decoder
            .decode_to_image(&data)
            .expected_error()?
            .expected_error()?;

        let memory_format = MemoryFormat::from(image.color());
        let (alpha_channel, grayscale, bits) =
            image_rs::channel_details(image.color().into()).internal_error()?;
        let width = image.width();
        let height = image.height();

        let bytes = image.into_bytes();
        let mut memory = SharedMemory::new(bytes.len() as u64).expected_error()?;

        Cursor::new(memory.as_mut())
            .write_all(&bytes)
            .internal_error()?;
        let texture = memory.into_binary_data();

        let mut frame = Frame::new(width, height, memory_format, texture).expected_error()?;

        frame.details.iccp = iccp
            .map(BinaryData::from_data)
            .transpose()
            .expected_error()?;

        if bits != 8 {
            frame.details.bit_depth = Some(bits);
        }

        if alpha_channel {
            frame.details.alpha_channel = Some(true);
        }

        if grayscale {
            frame.details.grayscale = Some(true);
        }

        Ok(frame)
    }
}

fn basic_info(data: &[u8]) -> (Option<JxlBasicInfo>, Option<Vec<u8>>, Option<Vec<u8>>) {
    unsafe {
        let decoder = JxlDecoderCreate(std::ptr::null());

        JxlDecoderSubscribeEvents(
            decoder,
            JxlDecoderStatus::BasicInfo as i32
                | JxlDecoderStatus::ColorEncoding as i32
                | JxlDecoderStatus::Box as i32,
        );
        JxlDecoderSetDecompressBoxes(decoder, JxlBool::True);
        JxlDecoderSetInput(decoder, data.as_ptr(), data.len());
        JxlDecoderCloseInput(decoder);

        let mut basic_info = None;
        let mut icc_profile = None;
        let mut exif = None;

        let mut exif_buf = Vec::new();
        let mut buf = Vec::new();

        loop {
            let status = JxlDecoderProcessInput(decoder);
            match status {
                JxlDecoderStatus::BasicInfo => {
                    let mut info = MaybeUninit::uninit();
                    if JxlDecoderGetBasicInfo(decoder, info.as_mut_ptr())
                        == JxlDecoderStatus::Success
                    {
                        basic_info = Some(info.assume_init());
                    }
                }
                JxlDecoderStatus::Box => {
                    let mut type_ = JxlBoxType([0; 4]);
                    JxlDecoderGetBoxType(decoder, &mut type_, JxlBool::True);

                    if &type_.0.map(|x| x as u8) == b"Exif" {
                        buf.resize(65536, 0);
                        JxlDecoderSetBoxBuffer(decoder, buf.as_mut_ptr(), buf.len());
                    }
                }
                JxlDecoderStatus::BoxNeedMoreOutput => {
                    let remaining = JxlDecoderReleaseBoxBuffer(decoder);
                    buf.truncate(buf.len() - remaining);
                    exif_buf.push(buf.clone());

                    JxlDecoderSetBoxBuffer(decoder, buf.as_mut_ptr(), buf.len());
                }
                JxlDecoderStatus::ColorEncoding => {
                    let mut size = 0;
                    let mut iccp = Vec::new();

                    if JxlDecoderGetICCProfileSize(decoder, JxlColorProfileTarget::Data, &mut size)
                        != JxlDecoderStatus::Success
                    {
                        break;
                    }

                    iccp.resize(size, 0);

                    if JxlDecoderGetColorAsICCProfile(
                        decoder,
                        JxlColorProfileTarget::Data,
                        iccp.as_mut_ptr(),
                        size,
                    ) == JxlDecoderStatus::Success
                    {
                        icc_profile = Some(iccp);
                    }
                }
                JxlDecoderStatus::Success => {
                    let remaining = JxlDecoderReleaseBoxBuffer(decoder);

                    if !buf.is_empty() {
                        exif_buf.push(buf.clone());
                    }

                    if remaining > 0 {
                        if let Some(last) = exif_buf.last_mut() {
                            last.resize(last.len() - remaining, 0);
                        }
                    }

                    let exif_data = exif_buf.concat();
                    if exif_data.len() > 4 {
                        let (_, data) = exif_data.split_at(4);
                        exif = Some(data.to_vec());
                    }

                    break;
                }
                status => {
                    eprintln!("Unexpected metadata status: {status:?}")
                }
            }
        }

        (basic_info, icc_profile, exif)
    }
}
