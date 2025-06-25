mod jpeg;
mod png;

use std::io::Cursor;

use glycin_utils::*;
use image::{ExtendedColorType, ImageFormat};

#[derive(Default)]
pub struct ImgEditor {}

impl EditorImplementation for ImgEditor {
    fn apply_sparse(
        stream: glycin_utils::UnixStream,
        mime_type: String,
        details: glycin_utils::InitializationDetails,
        operations: glycin_utils::operations::Operations,
    ) -> Result<SparseEditorOutput, glycin_utils::ProcessError> {
        match mime_type.as_str() {
            "image/jpeg" => Ok(jpeg::apply_sparse(stream, operations)?),
            _ => Ok(SparseEditorOutput::from(Self::apply_complete(
                stream, mime_type, details, operations,
            )?)),
        }
    }

    fn apply_complete(
        stream: UnixStream,
        mime_type: String,
        _details: InitializationDetails,
        operations: operations::Operations,
    ) -> Result<CompleteEditorOutput, ProcessError> {
        match mime_type.as_str() {
            "image/png" => png::apply(stream, operations),
            "image/jpeg" => Ok(jpeg::apply_complete(stream, operations)?),
            mime_type => Err(ProcessError::UnsupportedImageFormat(mime_type.to_string())),
        }
    }

    fn create(mime_type: String, mut new_image: NewImage) -> Result<EncodedImage, ProcessError> {
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

        let mut cur = Cursor::new(Vec::new());

        let v = frame.texture.get_full().expected_error()?;
        let img_buf = ImgBuf::Vec(v);
        let (frame, img_buf) =
            glycin_utils::editing::change_memory_format(img_buf, frame, memory_format)
                .expected_error()?;

        image::write_buffer_with_format(
            &mut cur,
            &img_buf,
            frame.width,
            frame.height,
            image_memory_format(memory_format)?,
            image_format,
        )
        .expected_error()?;

        let data = BinaryData::from_data(cur.into_inner())?;
        Ok(EncodedImage::new(data))
    }
}

fn image_format(mime_type: &str) -> Result<ImageFormat, ProcessError> {
    Ok(match mime_type {
        "image/bmp" => ImageFormat::Bmp,
        "image/x-dds" => ImageFormat::Dds,
        "image/x-ff" => ImageFormat::Farbfeld,
        "image/gif" => ImageFormat::Gif,
        "image/vnd.microsoft.icon" => ImageFormat::Ico,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/x-exr" => ImageFormat::OpenExr,
        "image/png" => ImageFormat::Png,
        "image/x-portable-bitmap"
        | "image/x-portable-graymap"
        | "image/x-portable-pixmap"
        | "image/x-portable-anymap" => ImageFormat::Pnm,
        "image/x-qoi" | "image/qoi" => ImageFormat::Qoi,
        "image/x-targa" | "image/x-tga" => ImageFormat::Tga,
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
