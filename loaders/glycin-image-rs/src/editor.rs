mod jpeg;
mod png;

use std::io::Cursor;

use glycin_utils::*;
use image::{ExtendedColorType, ImageEncoder, ImageFormat};

pub enum ImgEditor {
    Png(png::EditorPng),
    Jpeg(jpeg::EditJpeg),
}

impl EditorImplementation for ImgEditor {
    fn edit(
        stream: UnixStream,
        mime_type: String,
        _details: InitializationDetails,
    ) -> Result<Self, ProcessError> {
        Ok(match mime_type.as_str() {
            "image/png" => Self::Png(png::load(stream)?),
            "image/jpeg" => Self::Jpeg(jpeg::load(stream)?),
            mime_type => return Err(ProcessError::UnsupportedImageFormat(mime_type.to_string())),
        })
    }

    fn apply_sparse(
        &self,
        operations: Operations,
    ) -> Result<SparseEditorOutput, glycin_utils::ProcessError> {
        match self {
            Self::Jpeg(jpeg) => Ok(jpeg::apply_sparse(jpeg, operations)?),
            _ => Ok(SparseEditorOutput::from(Self::apply_complete(
                self, operations,
            )?)),
        }
    }

    fn apply_complete(&self, operations: Operations) -> Result<CompleteEditorOutput, ProcessError> {
        match self {
            Self::Png(png) => png::apply(png, operations),
            Self::Jpeg(jpeg) => jpeg::apply_complete(jpeg, operations),
        }
    }

    fn create(
        mime_type: String,
        mut new_image: NewImage,
        encoding_options: EncodingOptions,
    ) -> Result<EncodedImage, ProcessError> {
        let frame = new_image.frames.remove(0);

        let image_format = image_format(&mime_type)?;

        let memory_format = (MemoryFormatSelection::G8
            | MemoryFormatSelection::G8a8
            | MemoryFormatSelection::R8g8b8
            | MemoryFormatSelection::R8g8b8a8
            | MemoryFormatSelection::G16
            | MemoryFormatSelection::G16a16
            | MemoryFormatSelection::R16g16b16
            | MemoryFormatSelection::R16g16b16a16)
            .best_format_for(frame.memory_format)
            .internal_error()?;

        let v = frame.texture.get_full().expected_error()?;
        let img_buf = ImgBuf::Vec(v);
        let (frame, img_buf) =
            glycin_utils::editing::change_memory_format(img_buf, frame, memory_format)
                .expected_error()?;

        let memory_format = image_memory_format(memory_format)?;

        let icc_profile = frame.details.color_icc_profile.as_ref().and_then(|x| {
            x.get_full()
                .inspect_err(|err| log::error!("Can't read the ICC profile {err}"))
                .ok()
        });

        let image_buf = match image_format {
            ImageFormat::Png => {
                let compression = if let Some(compression) = encoding_options.compression {
                    if compression < 30 {
                        image::codecs::png::CompressionType::Fast
                    } else if compression < 80 {
                        image::codecs::png::CompressionType::Default
                    } else {
                        image::codecs::png::CompressionType::Best
                    }
                } else {
                    image::codecs::png::CompressionType::Default
                };

                let mut out_buf = Vec::new();
                let mut encoder = image::codecs::png::PngEncoder::new_with_quality(
                    &mut out_buf,
                    compression,
                    image::codecs::png::FilterType::default(),
                );

                if let Some(icc_profile) = icc_profile {
                    let _ = encoder.set_icc_profile(icc_profile);
                }

                encoder
                    .write_image(&img_buf, frame.width, frame.height, memory_format)
                    .internal_error()?;

                let out_buf = png::add_metadata(out_buf, &new_image.image_info, &frame.details);

                out_buf
            }
            ImageFormat::Jpeg => {
                let mut out_buf = Vec::new();
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                    &mut out_buf,
                    encoding_options
                        .quality
                        .map(|x| u8::min(x, 100))
                        .unwrap_or(90),
                );

                if let Some(icc_profile) = icc_profile {
                    let _ = encoder.set_icc_profile(icc_profile);
                }

                encoder
                    .write_image(&img_buf, frame.width, frame.height, memory_format)
                    .internal_error()?;

                out_buf
            }
            _ => {
                let mut cur = Cursor::new(Vec::new());
                image::write_buffer_with_format(
                    &mut cur,
                    &img_buf,
                    frame.width,
                    frame.height,
                    memory_format,
                    image_format,
                )
                .expected_error()?;

                cur.into_inner()
            }
        };

        let data = BinaryData::from_data(image_buf).expected_error()?;
        Ok(EncodedImage::new(data))
    }
}

fn image_format(mime_type: &str) -> Result<ImageFormat, ProcessError> {
    Ok(match mime_type {
        "image/bmp" => ImageFormat::Bmp,
        "image/x-ff" => ImageFormat::Farbfeld,
        "image/gif" => ImageFormat::Gif,
        "image/vnd.microsoft.icon" => ImageFormat::Ico,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/x-exr" => ImageFormat::OpenExr,
        "image/png" => ImageFormat::Png,
        "image/qoi" => ImageFormat::Qoi,
        "image/x-tga" => ImageFormat::Tga,
        "image/tiff" => ImageFormat::Tiff,
        "image/webp" => ImageFormat::WebP,
        _ => return Err(ProcessError::UnsupportedImageFormat(mime_type.to_string())),
    })
}

fn image_memory_format(memory_format: MemoryFormat) -> Result<ExtendedColorType, ProcessError> {
    Ok(match memory_format {
        MemoryFormat::G8 => ExtendedColorType::L8,
        MemoryFormat::G8a8 => ExtendedColorType::La8,
        MemoryFormat::R8g8b8 => ExtendedColorType::Rgb8,
        MemoryFormat::R8g8b8a8 => ExtendedColorType::Rgba8,
        MemoryFormat::G16 => ExtendedColorType::L16,
        MemoryFormat::G16a16 => ExtendedColorType::La16,
        MemoryFormat::R16g16b16 => ExtendedColorType::Rgb16,
        MemoryFormat::R16g16b16a16 => ExtendedColorType::Rgba16,
        _ => unreachable!(),
    })
}
