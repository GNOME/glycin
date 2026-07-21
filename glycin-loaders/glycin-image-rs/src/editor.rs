mod jpeg;
mod png;
mod tiff;

use std::io::{Cursor, Read};

use glycin_utils::*;
use image::{ExtendedColorType, ImageFormat};

pub enum ImgEditor {
    Png(png::EditorPng),
    Jpeg(jpeg::EditJpeg),
}

impl EditorImplementation for ImgEditor {
    fn edit<S: Read>(
        stream: S,
        mime_type: String,
        _details: InitializationDetails,
    ) -> Result<Self, ProcessError> {
        Ok(match mime_type.as_str() {
            "image/png" => Self::Png(png::load(stream)?),
            "image/jpeg" => Self::Jpeg(jpeg::load(stream)?),
            mime_type => return Err(ProcessError::UnsupportedImageFormat(mime_type.to_string())),
        })
    }

    fn apply_sparse<B: ByteData>(
        &self,
        operations: Operations,
    ) -> Result<SparseEditorOutput<B>, glycin_utils::ProcessError> {
        match self {
            Self::Jpeg(jpeg) => Ok(jpeg::apply_sparse(jpeg, operations)?),
            _ => Ok(SparseEditorOutput::from(Self::apply_complete(
                self, operations,
            )?)),
        }
    }

    fn apply_complete<B: ByteData>(
        &self,
        operations: Operations,
    ) -> Result<CompleteEditorOutput<B>, ProcessError> {
        match self {
            Self::Png(png) => png::apply(png, operations),
            Self::Jpeg(jpeg) => jpeg::apply_complete(jpeg, operations),
        }
    }

    fn create<B: ByteData>(
        mime_type: String,
        mut new_image: NewImage<B>,
        encoding_options: EncodingOptions,
    ) -> Result<EncodedImage<B>, ProcessError> {
        if new_image.frames.is_empty() {
            return Err(ProcessError::expected(&"No frames passed."));
        }
        let frame = new_image.frames.remove(0);

        let image_format = image_format(&mime_type)?;

        let frame = frame.into_fungible();

        let memory_format = image_memory_format(frame.memory_format)?;

        let icc_profile = frame.details.color_icc_profile.as_ref().map(|x| x.to_vec());

        let image_buf = match image_format {
            ImageFormat::Png => png::create(
                new_image,
                frame,
                encoding_options,
                memory_format,
                icc_profile,
            )?,
            ImageFormat::Jpeg => jpeg::create(frame, encoding_options, icc_profile)?,
            ImageFormat::Tiff => tiff::create(frame)?,
            _ => {
                let mut cur = Cursor::new(Vec::new());
                image::write_buffer_with_format(
                    &mut cur,
                    &frame.texture,
                    frame.width,
                    frame.height,
                    memory_format,
                    image_format,
                )
                .expected_error()?;

                cur.into_inner()
            }
        };

        let data = B::try_from_vec(image_buf).expected_error()?;
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
        MemoryFormat::R32g32b32Float => ExtendedColorType::Rgb32F,
        MemoryFormat::R32g32b32a32Float => ExtendedColorType::Rgba32F,
        _ => unreachable!(),
    })
}
