use std::cell::OnceCell;
use std::io::{Cursor, Read};
use std::sync::Mutex;

use glycin_utils::*;
use libheif_rs::{ColorProfile, ColorSpace, HeifContext, LibHeif, RgbChroma, StreamReader};

init_main!(ImgDecoder::default());

#[derive(Default)]
pub struct ImgDecoder {
    pub decoder: Mutex<Option<HeifContext<'static>>>,
    pub mime_type: OnceCell<String>,
}

impl LoaderImplementation for ImgDecoder {
    fn init(
        &self,
        mut stream: UnixStream,
        mime_type: String,
        _details: InitializationDetails,
    ) -> Result<ImageInfo, LoaderError> {
        let mut data = Vec::new();
        let total_size = stream.read_to_end(&mut data).internal_error()?;

        let stream_reader = StreamReader::new(Cursor::new(data), total_size.try_u64()?);
        let context = HeifContext::read_from_reader(Box::new(stream_reader)).loading_error()?;

        let handle = context.primary_image_handle().loading_error()?;

        let format_name = match mime_type.as_str() {
            "image/heif" => "HEIC",
            "image/avif" => "AVIF",
            _ => "HEIF (Unknown)",
        };

        let mut image_info = ImageInfo::new(handle.width(), handle.height());
        image_info.details.exif = exif(&handle).map(BinaryData::from);
        image_info.details.format_name = Some(format_name.to_string());

        // TODO: Later use libheif 1.16 to get info if there is a transformation
        image_info.details.transformations_applied = true;

        *self.decoder.lock().unwrap() = Some(context);
        let _ = self.mime_type.set(mime_type);
        Ok(image_info)
    }

    fn frame(&self, _frame_request: FrameRequest) -> Result<Frame, LoaderError> {
        let context = std::mem::take(&mut *self.decoder.lock().unwrap()).loading_error()?;
        decode(context, self.mime_type.get().unwrap())
    }
}

fn decode(context: HeifContext, mime_type: &str) -> Result<Frame, LoaderError> {
    let handle = context.primary_image_handle().loading_error()?;

    let rgb_chroma = if handle.luma_bits_per_pixel() > 8 {
        if handle.has_alpha_channel() {
            #[cfg(target_endian = "little")]
            {
                RgbChroma::HdrRgbaLe
            }
            #[cfg(target_endian = "big")]
            {
                RgbChroma::HdrRgbaBe
            }
        } else {
            #[cfg(target_endian = "little")]
            {
                RgbChroma::HdrRgbLe
            }
            #[cfg(target_endian = "big")]
            {
                RgbChroma::HdrRgbBe
            }
        }
    } else if handle.has_alpha_channel() {
        RgbChroma::Rgba
    } else {
        RgbChroma::Rgb
    };

    let libheif = LibHeif::new();
    let image_result = libheif.decode(&handle, ColorSpace::Rgb(rgb_chroma), None);

    let mut image = match image_result {
        Err(err) if matches!(err.sub_code, libheif_rs::HeifErrorSubCode::UnsupportedCodec) => {
            return Err(LoaderError::UnsupportedImageFormat(mime_type.to_string()));
        }
        image => image.loading_error()?,
    };

    let icc_profile = if let Some(profile) = handle.color_profile_raw() {
        if [
            libheif_rs::color_profile_types::R_ICC,
            libheif_rs::color_profile_types::PROF,
        ]
        .contains(&profile.profile_type())
        {
            Some(profile.data)
        } else {
            None
        }
    } else {
        None
    };

    let plane = image.planes_mut().interleaved.loading_error()?;

    let memory_format = match rgb_chroma {
        RgbChroma::HdrRgbBe | RgbChroma::HdrRgbaBe | RgbChroma::HdrRgbLe | RgbChroma::HdrRgbaLe => {
            if let Ok(transmuted) = safe_transmute::transmute_many_pedantic_mut::<u16>(plane.data) {
                // Scale HDR pixels to 16bit (they are usually 10bit or 12bit)
                for pixel in transmuted.iter_mut() {
                    *pixel <<= 16 - plane.bits_per_pixel;
                }
            } else {
                eprintln!("Could not transform HDR (16bit) data to u16");
            }

            if handle.has_alpha_channel() {
                if handle.is_premultiplied_alpha() {
                    MemoryFormat::R16g16b16a16Premultiplied
                } else {
                    MemoryFormat::R16g16b16a16
                }
            } else {
                MemoryFormat::R16g16b16
            }
        }
        RgbChroma::Rgb | RgbChroma::Rgba => {
            if handle.has_alpha_channel() {
                if handle.is_premultiplied_alpha() {
                    MemoryFormat::R8g8b8a8Premultiplied
                } else {
                    MemoryFormat::R8g8b8a8
                }
            } else {
                MemoryFormat::R8g8b8
            }
        }
        RgbChroma::C444 => unreachable!(),
    };

    let mut memory = SharedMemory::new(plane.stride.try_u64()? * u64::from(plane.height));
    Cursor::new(plane.data).read_exact(&mut memory).unwrap();
    let texture = memory.into_binary_data();

    let mut frame = Frame::new(plane.width, plane.height, memory_format, texture)?;
    frame.stride = plane.stride.try_u32()?;
    frame.details.iccp = icc_profile.map(BinaryData::from);
    if plane.bits_per_pixel > 8 {
        frame.details.bit_depth = Some(plane.bits_per_pixel);
    }
    frame.details.alpha_channel = Some(handle.has_alpha_channel());

    Ok(frame)
}

fn exif(handle: &libheif_rs::ImageHandle) -> Option<Vec<u8>> {
    let mut meta_ids = vec![0];
    handle.metadata_block_ids(&mut meta_ids, b"Exif");

    if let Some(meta_id) = meta_ids.first() {
        match handle.metadata(*meta_id) {
            Ok(mut exif_bytes) => {
                if let Some(skip) = exif_bytes
                    .get(0..4)
                    .map(|x| u32::from_be_bytes(x.try_into().unwrap()) as usize)
                {
                    if exif_bytes.len() > skip + 4 {
                        exif_bytes.drain(0..skip + 4);
                        return Some(exif_bytes);
                    } else {
                        eprintln!("EXIF data has far too few bytes");
                    }
                } else {
                    eprintln!("EXIF data has far too few bytes");
                }
            }
            Err(_) => return None,
        }
    }

    None
}
