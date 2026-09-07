use glycin_utils::*;

init_main_editor!(ImgEditor);

pub struct ImgEditor {
    pub mime_type: String,
}

impl EditorImplementation for ImgEditor {
    fn create<B: ByteData>(
        _mime_type: String,
        new_image: NewImage<B>,
        encoding_options: EncodingOptions,
    ) -> Result<EncodedImage<B>, ProcessError> {
        let frame = new_image.frames.first().expected_error()?;
        let layout = match frame.memory_format {
            MemoryFormat::R8g8b8 => webp::PixelLayout::Rgb,
            MemoryFormat::R8g8b8a8 => webp::PixelLayout::Rgba,
            _ => unreachable!(),
        };

        let encoder = webp::Encoder::new(&frame.texture, layout, frame.width, frame.height);
        let quality = encoding_options.quality.unwrap_or(85) as f32;
        let texture = encoder
            .encode_simple(false, quality)
            .map_err(|e| ProcessError::expected(&format!("Encoding Error {e:?}")))?;

        Ok(EncodedImage::new(
            B::try_from_slice(&texture).expected_error()?,
        ))
    }

    fn edit<S: std::io::Read + std::any::Any>(
        _stream: S,
        mime_type: String,
        _details: InitializationDetails,
    ) -> Result<Self, ProcessError> {
        Err(glycin_utils::RemoteError::UnsupportedImageFormat(
            mime_type.clone(),
        ))
        .expected_error()
    }

    fn apply_complete<B: ByteData>(
        &self,
        _operations: Operations,
    ) -> Result<CompleteEditorOutput<B>, ProcessError> {
        Err(glycin_utils::RemoteError::UnsupportedImageFormat(
            self.mime_type.clone(),
        ))
        .expected_error()
    }
}
