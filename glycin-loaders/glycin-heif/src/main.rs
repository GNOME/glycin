mod editing;

use std::io::{Cursor, Read};

use glycin_utils::safe_math::*;
use glycin_utils::*;
use gufo_common::cicp::Cicp;
use libheif_rs::{
    ColorProfile, ColorProfileNCLX, ColorProfileRaw, ColorSpace, HeifContext, ImageHandle, LibHeif,
    RgbChroma, StreamReader,
};

use crate::editing::ImgEditor;

init_main_loader_editor!(ImgDecoder, ImgEditor);

pub struct ImgDecoder {
    pub decoder: Option<HeifContext<'static>>,
    pub mime_type: String,
}

unsafe impl Sync for ImgDecoder {}

fn rgb_chroma(handle: &ImageHandle) -> RgbChroma {
    if handle.luma_bits_per_pixel() > 8 {
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
    }
}

fn memory_format(handle: &ImageHandle, rgb_chroma: RgbChroma) -> MemoryFormat {
    match rgb_chroma {
        RgbChroma::HdrRgbBe | RgbChroma::HdrRgbaBe | RgbChroma::HdrRgbLe | RgbChroma::HdrRgbaLe => {
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
    }
}

fn is_rgb_chroma_hdr(rgb_chroma: RgbChroma) -> bool {
    matches!(
        rgb_chroma,
        RgbChroma::HdrRgbBe | RgbChroma::HdrRgbaBe | RgbChroma::HdrRgbLe | RgbChroma::HdrRgbaLe
    )
}

fn scale_image_to_16bit(image: &mut libheif_rs::Image) {
    let plane = image.planes_mut().interleaved.unwrap();
    if let Ok(transmuted) = safe_transmute::transmute_many_pedantic_mut::<u16>(plane.data) {
        for pixel in transmuted.iter_mut() {
            *pixel <<= 16 - plane.bits_per_pixel;
        }
    } else {
        eprintln!("Could not transform HDR (16bit) data to u16");
    }
}

impl LoaderImplementation for ImgDecoder {
    fn load<B: ByteData, S: Read>(
        mut stream: S,
        mime_type: String,
        _details: InitializationDetails,
    ) -> Result<(Self, ImageDetails<B>), ProcessError> {
        let mut data = Vec::new();
        let total_size = stream.read_to_end(&mut data).internal_error()?;

        let stream_reader = StreamReader::new(Cursor::new(data), total_size.try_u64()?);
        let context = HeifContext::read_from_reader(Box::new(stream_reader)).expected_error()?;

        let handle = context.primary_image_handle().expected_error()?;

        let format_name = match mime_type.as_str() {
            "image/heif" => "HEIC",
            "image/avif" => "AVIF",
            _ => "HEIF (Unknown)",
        };

        let mut image_info = ImageDetails::new(handle.width(), handle.height());
        image_info.metadata_exif = exif(&handle)
            .map(B::try_from_vec)
            .transpose()
            .expected_error()?;
        image_info.info_format_name = Some(format_name.to_string());

        // TODO: Later use libheif 1.16 to get info if there is a transformation
        image_info.transformation_ignore_exif = true;

        let decoder = ImgDecoder {
            decoder: Some(context),
            mime_type,
        };

        Ok((decoder, image_info))
    }

    fn specific_frame<B: ByteData>(
        &mut self,
        _frame_request: FrameRequest,
    ) -> Result<Frame<B>, ProcessError> {
        decode(self.decoder.take().unwrap(), &self.mime_type)
    }
}

fn decode<B: ByteData>(context: HeifContext, mime_type: &str) -> Result<Frame<B>, ProcessError> {
    let handle = context.primary_image_handle().expected_error()?;

    let rgb_chroma = rgb_chroma(&handle);

    let libheif = LibHeif::new();
    let image_result = libheif.decode(&handle, ColorSpace::Rgb(rgb_chroma), None);

    let mut image = match image_result {
        Err(err) if matches!(err.sub_code, libheif_rs::HeifErrorSubCode::UnsupportedCodec) => {
            return Err(ProcessError::UnsupportedImageFormat(mime_type.to_string()));
        }
        image => image.expected_error()?,
    };

    let icc_profile = get_icc_profile(image.color_profile_raw())
        .or_else(|| get_icc_profile(handle.color_profile_raw()));

    let cicp = if icc_profile.is_none() {
        get_cicp(image.color_profile_nclx()).or_else(|| get_cicp(handle.color_profile_nclx()))
    } else {
        None
    };

    let memory_format = memory_format(&handle, rgb_chroma);

    // Scale HDR pixels to 16bit (they are usually 10bit or 12bit)
    if is_rgb_chroma_hdr(rgb_chroma) {
        scale_image_to_16bit(&mut image);
    }

    let plane = image.planes().interleaved.expected_error()?;

    let texture = B::try_from_slice(plane.data).expected_error()?;

    let mut frame = Frame::new(plane.width, plane.height, memory_format, texture)?;
    frame.stride = plane.stride.try_u32()?;
    frame.details.color_icc_profile = icc_profile
        .map(B::try_from_vec)
        .transpose()
        .expected_error()?;
    frame.details.color_cicp = cicp.map(|x| x.to_bytes());
    if plane.bits_per_pixel > 8 {
        frame.details.info_bit_depth = Some(plane.bits_per_pixel);
    }
    frame.details.info_alpha_channel = Some(handle.has_alpha_channel());

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

fn get_cicp(profile: Option<ColorProfileNCLX>) -> Option<Cicp> {
    if let Some(nclx) = profile {
        if nclx.profile_type() == libheif_rs::color_profile_types::NCLX {
            Cicp::from_bytes(&[
                nclx.color_primaries() as u8,
                nclx.transfer_characteristics() as u8,
                // Force RGB until we support YCbCr
                0,
                nclx.full_range_flag(),
            ])
            .ok()
        } else {
            None
        }
    } else {
        None
    }
}

fn get_icc_profile(profile: Option<ColorProfileRaw>) -> Option<Vec<u8>> {
    if let Some(profile) = profile {
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
    }
}
