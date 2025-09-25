#![allow(clippy::large_enum_variant)]

mod editor;

use std::io::{Cursor, Read};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

use editor::ImgEditor;
use glycin_utils::image_rs::Handler;
use glycin_utils::safe_math::*;
use glycin_utils::{MemoryFormatInfo, *};
use gufo_common::cicp::Cicp;
use image::{codecs, AnimationDecoder, ImageDecoder, ImageResult, Limits};
use log::trace;

init_main_loader_editor!(ImgDecoder, ImgEditor);

type Reader = Cursor<Vec<u8>>;
type FrameReceiver = Receiver<Result<(Frame, bool), ProcessError>>;
type FrameSender = Sender<Result<(Frame, bool), ProcessError>>;

#[derive(Default)]
pub struct ImgDecoder {
    pub format: Mutex<Option<ImageRsFormat<Reader>>>,
    pub thread: Mutex<Option<(std::thread::JoinHandle<()>, FrameReceiver)>>,
    pub cicp: Mutex<Option<Cicp>>,
}

fn animated_worker(
    format: ImageRsFormat<Reader>,
    data: Reader,
    mime_type: String,
    send: FrameSender,
) {
    let mut format = Some(format);

    std::thread::park();

    let mut looped = false;

    // Replay animation from beginning
    loop {
        log::trace!("animated: Start loading loop for {mime_type}");

        if format.is_none() {
            format = ImageRsFormat::create(data.clone(), &mime_type).ok();
        }

        let mut decoder = format.as_mut().map(|x| &mut x.decoder);

        // Use transparent background instead of suggested background color
        if let Some(ImageRsDecoder::WebP(webp)) = &mut decoder {
            let _result = webp.set_background_color(image::Rgba::from([0, 0, 0, 0]));
        }

        let frame_details = match format.as_mut().unwrap().frame_details() {
            Ok(frame_details) => Some(frame_details),
            Err(err) => {
                send.send(Err(err)).unwrap();
                return;
            }
        };

        let mut frames = std::mem::take(&mut format)
            .unwrap()
            .decoder
            .into_frames()
            .unwrap();
        let mut first_frames = Vec::new();

        // Decode first two frames to check if actually an animation
        trace!("animated: Decoding first two frames");
        for _ in 0..2 {
            if let Some(frame) = frames.next() {
                first_frames.push(frame);
            }
        }

        let is_animated = match first_frames.len() {
            0 => {
                send.send(Err(ProcessError::expected(&"No frame found.")))
                    .unwrap();
                return;
            }
            1 => false,
            _ => true,
        };

        for frame in first_frames.into_iter().chain(frames).enumerate() {
            // Only use FrameDetails for still images because they might not make too much
            // sense otherwise
            let frame_details = (!is_animated).then(|| frame_details.clone()).flatten();

            let decoded_frame = animated_get_frame(frame, frame_details, is_animated);
            send.send(decoded_frame.map(|x| (x, looped))).unwrap();

            // If not really an animation no need to keep the thread around
            if !is_animated {
                log::debug!("animated: Image is actually not animated");
                return;
            }

            std::thread::park();
        }

        looped = true;
    }
}

pub fn animated_get_frame(
    (n_frame, frame): (usize, Result<image::Frame, image::ImageError>),
    frame_details: Option<FrameDetails>,
    is_animated: bool,
) -> Result<Frame, ProcessError> {
    log::trace!("animated: Treating decoded frame {n_frame}");
    let frame = frame.expected_error()?;

    let (delay_num, delay_den) = frame.delay().numer_denom_ms();

    let delay = if !is_animated {
        None
    } else if delay_num == 0 || delay_den == 0 {
        // Other decoders default to this value as well
        Some(std::time::Duration::from_millis(100))
    } else {
        let micros = f64::round(delay_num as f64 * 1000. / delay_den as f64) as u64;
        Some(std::time::Duration::from_micros(micros))
    };

    let buffer = frame.into_buffer();

    let memory_format = MemoryFormat::R8g8b8a8;
    let width = buffer.width();
    let height = buffer.height();

    let mut memory =
        SharedMemory::new(u64::from(width) * u64::from(height) * memory_format.n_bytes().u64())
            .expected_error()
            .unwrap();
    Cursor::new(buffer.into_raw())
        .read_exact(&mut memory)
        .unwrap();
    let texture = memory.into_binary_data();

    let mut out_frame = Frame::new(width, height, memory_format, texture).unwrap();
    out_frame.delay = delay.into();

    // Set frame info for still pictures
    if let Some(frame_details) = frame_details {
        out_frame.details = frame_details;
    };

    out_frame.details.n_frame = Some(n_frame.try_u64()?);

    Ok(out_frame)
}

impl LoaderImplementation for ImgDecoder {
    fn init(
        mut stream: UnixStream,
        mime_type: String,
        _details: InitializationDetails,
    ) -> Result<(Self, ImageDetails), ProcessError> {
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).internal_error()?;
        let data = Cursor::new(buf);

        let mut format = ImageRsFormat::create(data.clone(), &mime_type)?;
        if let Err(err) = format.set_no_limits() {
            eprint!("Failed to unset decoder limits: {err}");
        }
        let mut image_info = format.info();

        // TODO: Unnecessary clone of data
        let metadata = gufo::RawMetadata::for_guessed(data.into_inner());

        let data = match metadata {
            Ok((metadata, data)) => {
                image_info.metadata_exif = metadata
                    .exif
                    .first()
                    .map(BinaryData::from_data)
                    .transpose()
                    .expected_error()?;

                image_info.metadata_xmp = metadata
                    .xmp
                    .first()
                    .map(BinaryData::from_data)
                    .transpose()
                    .expected_error()?;

                image_info.metadata_key_value = Some(metadata.key_value);

                data
            }
            Err(err) => err.into_inner(),
        };

        let loader_impelementation = Self::default();

        // TODO: Unnecessary clone of data
        let gufo_image = gufo::Image::new(data);
        let data = Cursor::new(match gufo_image {
            Ok(gufo_image) => {
                *loader_impelementation.cicp.lock().unwrap() = gufo_image.cicp();
                gufo_image.into_inner()
            }
            Err(err) => err.into_inner(),
        });

        if format.decoder.is_animated() {
            let (send, recv) = channel();
            let thead = std::thread::spawn(move || animated_worker(format, data, mime_type, send));
            *loader_impelementation.thread.lock().unwrap() = Some((thead, recv));
        } else {
            *loader_impelementation.format.lock().unwrap() = Some(format);
        }

        Ok((loader_impelementation, image_info))
    }

    fn frame(&mut self, frame_request: FrameRequest) -> Result<Frame, ProcessError> {
        let mut frame = if let Some(decoder) = std::mem::take(&mut *self.format.lock().unwrap()) {
            decoder.frame().expected_error()?
        } else if let Some((ref thread, ref recv)) = *self.thread.lock().unwrap() {
            thread.thread().unpark();
            let (frame, looped) = recv.recv().internal_error()??;
            if !frame_request.loop_animation && matches!(frame.details.n_frame, Some(0)) && looped {
                return Err(ProcessError::NoMoreFrames);
            }
            frame
        } else {
            return Err(ProcessError::NoMoreFrames);
        };

        frame.details.color_cicp = self.cicp.lock().unwrap().map(|x| {
            [
                x.color_primaries.into(),
                x.transfer_characteristics.into(),
                x.matrix_coefficients.into(),
                x.video_full_range_flag.into(),
            ]
        });

        Ok(frame)
    }
}

pub enum ImageRsDecoder<T: std::io::BufRead + std::io::Seek> {
    Bmp(codecs::bmp::BmpDecoder<T>),
    Dds(codecs::dds::DdsDecoder<T>),
    Farbfeld(codecs::farbfeld::FarbfeldDecoder<T>),
    Gif(codecs::gif::GifDecoder<T>),
    Ico(codecs::ico::IcoDecoder<T>),
    Jpeg(codecs::jpeg::JpegDecoder<T>),
    OpenExr(codecs::openexr::OpenExrDecoder<T>),
    Png(codecs::png::PngDecoder<T>),
    Pnm(codecs::pnm::PnmDecoder<T>),
    Qoi(codecs::qoi::QoiDecoder<T>),
    Tga(codecs::tga::TgaDecoder<T>),
    Tiff(codecs::tiff::TiffDecoder<T>),
    WebP(codecs::webp::WebPDecoder<T>),
}

pub struct ImageRsFormat<T: std::io::BufRead + std::io::Seek> {
    decoder: ImageRsDecoder<T>,
    handler: Handler,
}

impl ImageRsFormat<Reader> {
    fn create(data: Reader, mime_type: &str) -> Result<Self, ProcessError> {
        Ok(match mime_type {
            "image/apng" => Self::new(ImageRsDecoder::Png(
                codecs::png::PngDecoder::new(data).expected_error()?,
            ))
            .format_name("Animated PNG")
            .supports_two_alpha_modes(true)
            .supports_two_grayscale_modes(true)
            .default_bit_depth(8),
            "image/bmp" => Self::new(ImageRsDecoder::Bmp(
                codecs::bmp::BmpDecoder::new(data).expected_error()?,
            ))
            .format_name("BMP")
            .default_bit_depth(8),
            "image/x-dds" => Self::new(ImageRsDecoder::Dds(
                codecs::dds::DdsDecoder::new(data).expected_error()?,
            ))
            .format_name("DDS")
            .supports_two_grayscale_modes(true),
            "image/x-ff" => Self::new(ImageRsDecoder::Farbfeld(
                codecs::farbfeld::FarbfeldDecoder::new(data).expected_error()?,
            ))
            .format_name("Farbfeld")
            .default_bit_depth(16),
            "image/gif" => Self::new(ImageRsDecoder::Gif(
                codecs::gif::GifDecoder::new(data).expected_error()?,
            ))
            .format_name("GIF")
            .default_bit_depth(8),
            "image/vnd.microsoft.icon" => Self::new(ImageRsDecoder::Ico(
                codecs::ico::IcoDecoder::new(data).expected_error()?,
            ))
            .format_name("ICO"),
            "image/jpeg" => Self::new(ImageRsDecoder::Jpeg(
                codecs::jpeg::JpegDecoder::new(data).expected_error()?,
            ))
            .format_name("JPEG")
            .default_bit_depth(8)
            .supports_two_grayscale_modes(true),
            "image/x-exr" => Self::new(ImageRsDecoder::OpenExr(
                codecs::openexr::OpenExrDecoder::new(data).expected_error()?,
            ))
            .format_name("OpenEXR")
            .default_bit_depth(32)
            .supports_two_grayscale_modes(true),
            "image/png" => Self::new(ImageRsDecoder::Png(
                codecs::png::PngDecoder::new(data).expected_error()?,
            ))
            .format_name("PNG")
            .supports_two_alpha_modes(true)
            .supports_two_grayscale_modes(true)
            .default_bit_depth(8),
            "image/x-portable-bitmap" => Self::new(ImageRsDecoder::Pnm(
                codecs::pnm::PnmDecoder::new(data).expected_error()?,
            ))
            .format_name("PBM")
            .default_bit_depth(1),
            "image/x-portable-graymap" => Self::new(ImageRsDecoder::Pnm(
                codecs::pnm::PnmDecoder::new(data).expected_error()?,
            ))
            .format_name("PGM"),
            "image/x-portable-pixmap" => Self::new(ImageRsDecoder::Pnm(
                codecs::pnm::PnmDecoder::new(data).expected_error()?,
            ))
            .format_name("PPM"),
            "image/x-portable-anymap" => Self::new(ImageRsDecoder::Pnm(
                codecs::pnm::PnmDecoder::new(data).expected_error()?,
            ))
            .format_name("PAM"),
            "image/x-qoi" | "image/qoi" => Self::new(ImageRsDecoder::Qoi(
                codecs::qoi::QoiDecoder::new(data).expected_error()?,
            ))
            .format_name("QOI")
            .default_bit_depth(8)
            .supports_two_alpha_modes(true),
            "image/x-targa" | "image/x-tga" => Self::new(ImageRsDecoder::Tga(
                codecs::tga::TgaDecoder::new(data).expected_error()?,
            ))
            .format_name("TGA")
            .supports_two_grayscale_modes(true),
            "image/tiff" => Self::new(ImageRsDecoder::Tiff(
                codecs::tiff::TiffDecoder::new(data).expected_error()?,
            ))
            .format_name("TIFF")
            .supports_two_alpha_modes(true)
            .supports_two_grayscale_modes(true),
            "image/webp" => Self::new(ImageRsDecoder::WebP(
                codecs::webp::WebPDecoder::new(data).expected_error()?,
            ))
            .format_name("WebP")
            .default_bit_depth(8)
            .supports_two_alpha_modes(true),
            mime_type => return Err(ProcessError::UnsupportedImageFormat(mime_type.to_string())),
        })
    }
}

impl<'a, T: std::io::BufRead + std::io::Seek + 'a> ImageRsFormat<T> {
    pub fn format_name(mut self, format_name: impl ToString) -> Self {
        self.handler = self.handler.format_name(format_name);
        self
    }

    pub fn supports_two_alpha_modes(mut self, supports_two_alpha_modes: bool) -> Self {
        self.handler = self
            .handler
            .supports_two_alpha_modes(supports_two_alpha_modes);
        self
    }

    pub fn supports_two_grayscale_modes(mut self, supports_two_grayscale_modes: bool) -> Self {
        self.handler = self
            .handler
            .supports_two_grayscale_modes(supports_two_grayscale_modes);
        self
    }

    pub fn default_bit_depth(mut self, default_bit_depth: u8) -> Self {
        self.handler = self.handler.default_bit_depth(default_bit_depth);
        self
    }

    fn new(decoder: ImageRsDecoder<T>) -> Self {
        Self {
            decoder,
            handler: Handler::default(),
        }
    }

    fn info(&mut self) -> ImageDetails {
        match self.decoder {
            ImageRsDecoder::Bmp(ref mut d) => self.handler.info(d),
            ImageRsDecoder::Dds(ref mut d) => self.handler.info(d),
            ImageRsDecoder::Farbfeld(ref mut d) => self.handler.info(d),
            ImageRsDecoder::Gif(ref mut d) => self.handler.info(d),
            ImageRsDecoder::Ico(ref mut d) => self.handler.info(d),
            ImageRsDecoder::Jpeg(ref mut d) => self.handler.info(d),
            ImageRsDecoder::OpenExr(ref mut d) => self.handler.info(d),
            ImageRsDecoder::Png(ref mut d) => self.handler.info(d),
            ImageRsDecoder::Pnm(ref mut d) => self.handler.info(d),
            ImageRsDecoder::Qoi(ref mut d) => self.handler.info(d),
            ImageRsDecoder::Tga(ref mut d) => self.handler.info(d),
            ImageRsDecoder::Tiff(ref mut d) => self.handler.info(d),
            ImageRsDecoder::WebP(ref mut d) => self.handler.info(d),
        }
    }

    fn frame(self) -> Result<Frame, ProcessError> {
        match self.decoder {
            ImageRsDecoder::Bmp(d) => self.handler.frame(d),
            ImageRsDecoder::Dds(d) => self.handler.frame(d),
            ImageRsDecoder::Farbfeld(d) => self.handler.frame(d),
            ImageRsDecoder::Gif(d) => self.handler.frame(d),
            ImageRsDecoder::Ico(d) => self.handler.frame(d),
            ImageRsDecoder::Jpeg(d) => self.handler.frame(d),
            ImageRsDecoder::OpenExr(d) => self.handler.frame(d),
            ImageRsDecoder::Png(d) => self.handler.frame(d),
            ImageRsDecoder::Pnm(d) => self.handler.frame(d),
            ImageRsDecoder::Qoi(d) => self.handler.frame(d),
            ImageRsDecoder::Tga(d) => self.handler.frame(d),
            ImageRsDecoder::Tiff(d) => self.handler.frame(d),
            ImageRsDecoder::WebP(d) => self.handler.frame(d),
        }
    }

    fn frame_details(&mut self) -> Result<FrameDetails, ProcessError> {
        match self.decoder {
            ImageRsDecoder::Bmp(ref mut d) => self.handler.frame_details(d),
            ImageRsDecoder::Dds(ref mut d) => self.handler.frame_details(d),
            ImageRsDecoder::Farbfeld(ref mut d) => self.handler.frame_details(d),
            ImageRsDecoder::Gif(ref mut d) => self.handler.frame_details(d),
            ImageRsDecoder::Ico(ref mut d) => self.handler.frame_details(d),
            ImageRsDecoder::Jpeg(ref mut d) => self.handler.frame_details(d),
            ImageRsDecoder::OpenExr(ref mut d) => self.handler.frame_details(d),
            ImageRsDecoder::Png(ref mut d) => self.handler.frame_details(d),
            ImageRsDecoder::Pnm(ref mut d) => self.handler.frame_details(d),
            ImageRsDecoder::Qoi(ref mut d) => self.handler.frame_details(d),
            ImageRsDecoder::Tga(ref mut d) => self.handler.frame_details(d),
            ImageRsDecoder::Tiff(ref mut d) => self.handler.frame_details(d),
            ImageRsDecoder::WebP(ref mut d) => self.handler.frame_details(d),
        }
    }

    fn set_no_limits(&mut self) -> ImageResult<()> {
        let limits = Limits::no_limits();

        match self.decoder {
            ImageRsDecoder::Bmp(ref mut d) => d.set_limits(limits),
            ImageRsDecoder::Dds(ref mut d) => d.set_limits(limits),
            ImageRsDecoder::Farbfeld(ref mut d) => d.set_limits(limits),
            ImageRsDecoder::Gif(ref mut d) => d.set_limits(limits),
            ImageRsDecoder::Ico(ref mut d) => d.set_limits(limits),
            ImageRsDecoder::Jpeg(ref mut d) => d.set_limits(limits),
            ImageRsDecoder::OpenExr(ref mut d) => d.set_limits(limits),
            ImageRsDecoder::Png(ref mut d) => d.set_limits(limits),
            ImageRsDecoder::Pnm(ref mut d) => d.set_limits(limits),
            ImageRsDecoder::Qoi(ref mut d) => d.set_limits(limits),
            ImageRsDecoder::Tga(ref mut d) => d.set_limits(limits),
            ImageRsDecoder::Tiff(ref mut d) => d.set_limits(limits),
            ImageRsDecoder::WebP(ref mut d) => d.set_limits(limits),
        }
    }
}

impl<'a, T: std::io::BufRead + std::io::Seek + 'a> ImageRsDecoder<T> {
    fn into_frames(self) -> Option<image::Frames<'a>> {
        match self {
            Self::Png(d) => Some(d.apng().unwrap().into_frames()),
            Self::Gif(d) => Some(d.into_frames()),
            Self::WebP(d) => Some(d.into_frames()),
            _ => None,
        }
    }

    fn is_animated(&self) -> bool {
        match self {
            Self::Gif(_) => true,
            Self::Png(d) => d.is_apng().unwrap(),
            Self::WebP(d) => d.has_animation(),
            _ => false,
        }
    }
}
