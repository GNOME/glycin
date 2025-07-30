// SPDX-Copyright: 2024 Hubert Figuière

use std::io::{Cursor, Read};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

use glycin_utils::safe_math::SafeConversion;
use glycin_utils::*;
use libopenraw::metadata::Value;
use libopenraw::Bitmap;

init_main_loader!(ImgDecoder);

pub struct ImgDecoder {
    thread: ImgDecoderDetails,
}

pub struct ImgDecoderDetails {
    frame_recv: Mutex<Receiver<Result<Frame, ProcessError>>>,
    instr_send: Sender<()>,
}

pub fn thread(
    mut stream: UnixStream,
    info_send: Sender<Result<ImageDetails, ProcessError>>,
    frame_send: Sender<Result<Frame, ProcessError>>,
    instr_recv: Receiver<()>,
) {
    let mut buf = vec![];
    stream.read_to_end(&mut buf).unwrap();
    let rawfile = libopenraw::rawfile_from_memory(buf, None).unwrap();
    let rawimage = rawfile.raw_data(false).unwrap();
    let w = rawimage.width();
    let h = rawimage.height();
    let xmp = rawfile
        .metadata_value(&"Exif.Image.ApplicationNotes".to_string())
        .and_then(|value| {
            if let Value::Bytes(xmp) = value {
                Some(xmp)
            } else {
                None
            }
        });

    let mut image_info = ImageDetails::new(w, h);

    image_info.info_format_name = Some(String::from("RAW"));
    image_info.metadata_xmp = xmp.and_then(|xmp| BinaryData::from_data(xmp).ok());
    image_info.transformation_ignore_exif = false;

    info_send.send(Ok(image_info)).unwrap();

    while instr_recv.recv().is_ok() {
        let frame = render(&rawimage);

        frame_send.send(frame).unwrap();
    }
}

pub fn render(rawdata: &libopenraw::RawImage) -> Result<Frame, ProcessError> {
    let rawimage = rawdata
        .rendered_image(&libopenraw::RenderingOptions::default())
        .expected_error()?;
    let width = rawimage.width();
    let height = rawimage.height();
    let mut memory = SharedMemory::new(rawimage.data_size() as u64).expected_error()?;

    let data8 = if let Some(data16) = rawimage.data16() {
        unsafe {
            std::slice::from_raw_parts(data16.as_ptr().cast::<u8>(), std::mem::size_of_val(data16))
        }
    } else {
        rawimage.data8().expected_error()?
    };
    Cursor::new(data8)
        .read_exact(&mut memory)
        .internal_error()?;
    let texture = memory.into_binary_data();

    Frame::new(
        width.try_u32()?,
        height.try_u32()?,
        MemoryFormat::R16g16b16,
        texture,
    )
    .internal_error()
}

impl LoaderImplementation for ImgDecoder {
    fn init(
        stream: UnixStream,
        _mime_type: String,
        _details: InitializationDetails,
    ) -> Result<(ImgDecoder, ImageDetails), ProcessError> {
        let (info_send, info_recv) = channel();
        let (frame_send, frame_recv) = channel();
        let (instr_send, instr_recv) = channel();

        std::thread::spawn(move || thread(stream, info_send, frame_send, instr_recv));
        let image_info = info_recv.recv().unwrap()?;

        let decoder = ImgDecoder {
            thread: ImgDecoderDetails {
                frame_recv: Mutex::new(frame_recv),
                instr_send,
            },
        };

        Ok((decoder, image_info))
    }

    fn frame(&mut self, _frame_request: FrameRequest) -> Result<Frame, ProcessError> {
        let frame_recv_lock = self.thread.frame_recv.lock().unwrap();

        self.thread.instr_send.send(()).internal_error()?;
        frame_recv_lock.recv().internal_error()?
    }
}
