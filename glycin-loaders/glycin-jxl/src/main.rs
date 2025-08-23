#![allow(clippy::large_enum_variant)]

mod editing;

use std::io::{Cursor, Read, Write};
use std::mem::MaybeUninit;

use glycin_utils::*;
use gufo_common::cicp::{Cicp, ColorPrimaries, MatrixCoefficients, TransferCharacteristics};
use jpegxl_sys::color::color_encoding::{
    JxlColorEncoding, JxlColorSpace, JxlPrimaries, JxlTransferFunction, JxlWhitePoint,
};
use jpegxl_sys::common::types::{JxlBool, JxlBoxType};
use jpegxl_sys::decode::*;
use jpegxl_sys::metadata::codestream_header::*;
use zerocopy::IntoBytes;

use crate::editing::ImgEditor;

init_main_loader_editor!(ImgDecoder, ImgEditor);

#[derive(Default)]
pub struct ImgDecoder {
    data: Vec<u8>,
    icc_profile: Option<Vec<u8>>,
    cicp: Option<Cicp>,
}

impl LoaderImplementation for ImgDecoder {
    fn init(
        mut stream: UnixStream,
        _mime_type: String,
        _details: InitializationDetails,
    ) -> Result<(Self, ImageDetails), ProcessError> {
        let mut data = Vec::new();
        stream.read_to_end(&mut data).expected_error()?;
        let (info, icc_profile, exif, cicp) = basic_info(&data);

        let info = info.expected_error()?;

        let mut image_info = ImageDetails::new(info.xsize, info.ysize);
        image_info.info_format_name = Some(String::from("JPEG XL"));
        image_info.metadata_exif = exif
            .map(BinaryData::from_data)
            .transpose()
            .expected_error()?;
        image_info.transformation_ignore_exif = true;

        let loader_implementation = ImgDecoder {
            data,
            icc_profile,
            cicp,
        };

        Ok((loader_implementation, image_info))
    }

    fn frame(&mut self, _frame_request: FrameRequest) -> Result<Frame, ProcessError> {
        let runner = jpegxl_rs::parallel::resizable_runner::ResizableRunner::new(None).unwrap();
        let decoder = jpegxl_rs::decoder_builder()
            .parallel_runner(&runner)
            .build()
            .expected_error()?;

        let (metadata, pixels) = decoder.decode(&self.data).expected_error()?;

        let memory_format;
        let bytes;
        let bits;
        let f16_bytes;

        let alpha_channel = metadata.has_alpha_channel;
        let grayscale = metadata.num_color_channels == 1;

        match &pixels {
            jpegxl_rs::decode::Pixels::Float(data) => {
                bits = 32;
                bytes = data.as_bytes();

                memory_format = match (metadata.num_color_channels, metadata.has_alpha_channel) {
                    (3, false) => MemoryFormat::R32g32b32Float,
                    (3, true) => MemoryFormat::R32g32b32a32Float,
                    _ => unimplemented!(),
                };
            }
            jpegxl_rs::decode::Pixels::Float16(data) => {
                bits = 16;
                f16_bytes = data
                    .into_iter()
                    .map(|x| x.to_le_bytes())
                    .flatten()
                    .collect::<Vec<u8>>();
                bytes = f16_bytes.as_bytes();

                memory_format = match (metadata.num_color_channels, metadata.has_alpha_channel) {
                    (3, false) => MemoryFormat::R16g16b16Float,
                    (3, true) => MemoryFormat::R16g16b16a16Float,
                    _ => unimplemented!(),
                };
            }
            jpegxl_rs::decode::Pixels::Uint16(data) => {
                bits = 16;
                bytes = data.as_bytes();

                memory_format = match (metadata.num_color_channels, metadata.has_alpha_channel) {
                    (3, false) => MemoryFormat::R16g16b16,
                    (3, true) => MemoryFormat::R16g16b16a16,
                    (1, false) => MemoryFormat::G16,
                    (1, true) => MemoryFormat::G16a16,
                    _ => unimplemented!(),
                };
            }
            jpegxl_rs::decode::Pixels::Uint8(data) => {
                bits = 8;
                bytes = data.as_bytes();

                memory_format = match (metadata.num_color_channels, metadata.has_alpha_channel) {
                    (3, false) => MemoryFormat::R8g8b8,
                    (3, true) => MemoryFormat::R8g8b8a8,
                    (1, false) => MemoryFormat::G8,
                    (1, true) => MemoryFormat::G8a8,
                    _ => unimplemented!(),
                };
            }
        }

        let width = metadata.width;
        let height = metadata.height;

        let mut memory = SharedMemory::new(bytes.len() as u64).expected_error()?;

        Cursor::new(memory.as_mut())
            .write_all(&bytes)
            .internal_error()?;
        let texture = memory.into_binary_data();

        let mut frame = Frame::new(width, height, memory_format, texture).expected_error()?;

        frame.details.color_icc_profile = self
            .icc_profile
            .clone()
            .map(BinaryData::from_data)
            .transpose()
            .expected_error()?;

        frame.details.color_cicp = self.cicp.map(|x| x.to_bytes());

        if bits != 8 {
            frame.details.info_bit_depth = Some(bits);
        }

        if alpha_channel {
            frame.details.info_alpha_channel = Some(true);
        }

        if grayscale {
            frame.details.info_grayscale = Some(true);
        }

        Ok(frame)
    }
}

fn basic_info(
    data: &[u8],
) -> (
    Option<JxlBasicInfo>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Option<Cicp>,
) {
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
        let mut cicp = None;

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
                    let mut color_encoding = MaybeUninit::uninit();
                    let res = JxlDecoderGetColorAsEncodedProfile(
                        decoder,
                        JxlColorProfileTarget::Original,
                        color_encoding.as_mut_ptr(),
                    );

                    if res == JxlDecoderStatus::Success {
                        let color_encoding = color_encoding.assume_init();
                        cicp = color_encoding_to_cicp(color_encoding);
                    }

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

        JxlDecoderDestroy(decoder);

        (basic_info, icc_profile, exif, cicp)
    }
}

fn color_encoding_to_cicp(c: JxlColorEncoding) -> Option<Cicp> {
    if c.color_space != JxlColorSpace::Rgb {
        return None;
    }

    let color_primaries = if c.primaries == JxlPrimaries::P3 && c.white_point == JxlWhitePoint::Dci
    {
        ColorPrimaries::DciP3
    } else if c.primaries == JxlPrimaries::P3 && c.white_point == JxlWhitePoint::D65 {
        ColorPrimaries::DisplayP3
    } else if c.primaries == JxlPrimaries::Rec2100 && c.white_point == JxlWhitePoint::D65 {
        ColorPrimaries::Rec2020
    } else {
        return None;
    };

    let transfer_characteristics = match c.transfer_function {
        JxlTransferFunction::Linear => TransferCharacteristics::Linear,
        JxlTransferFunction::HLG => TransferCharacteristics::Hlg,
        JxlTransferFunction::PQ => TransferCharacteristics::Pq,
        JxlTransferFunction::SRGB => TransferCharacteristics::Gamma24,
        JxlTransferFunction::BT709 => TransferCharacteristics::Gamma22,
        JxlTransferFunction::DCI => TransferCharacteristics::Dci,
        _ => {
            return None;
        }
    };

    Some(Cicp {
        color_primaries,
        matrix_coefficients: MatrixCoefficients::Identity,
        transfer_characteristics,
        video_full_range_flag: gufo_common::cicp::VideoRangeFlag::Full,
    })
}
