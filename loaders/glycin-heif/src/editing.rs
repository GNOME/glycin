use glycin_utils::{BinaryData, EditorImplementation, GenericContexts, MemoryFormatInfo};
use libheif_rs::{
    Channel, ColorProfileRaw, ColorSpace, CompressionFormat, EncoderQuality, HeifContext, Image,
    LibHeif, RgbChroma,
};

pub struct ImgEditor {
    mime_type: String,
}

impl EditorImplementation for ImgEditor {
    fn edit(
        _stream: std::os::unix::net::UnixStream,
        mime_type: String,
        _details: glycin_utils::InitializationDetails,
    ) -> Result<Self, glycin_utils::ProcessError> {
        Err(glycin_utils::RemoteError::UnsupportedImageFormat(
            mime_type.clone(),
        ))
        .expected_error()
    }

    fn apply_complete(
        &self,
        _operations: glycin_utils::Operations,
    ) -> Result<glycin_utils::CompleteEditorOutput, glycin_utils::ProcessError> {
        Err(glycin_utils::RemoteError::UnsupportedImageFormat(
            self.mime_type.clone(),
        ))
        .expected_error()
    }

    fn create(
        mime_type: String,
        mut new_image: glycin_utils::NewImage,
        encoding_options: glycin_utils::EncodingOptions,
    ) -> Result<glycin_utils::EncodedImage, glycin_utils::ProcessError> {
        let frame = new_image.frames.remove(0);

        let memory_format = (glycin_utils::MemoryFormatSelection::R8g8b8
            | glycin_utils::MemoryFormatSelection::R8g8b8a8)
            .best_format_for(frame.memory_format)
            .internal_error()?;

        let v = frame.texture.get_full().expected_error()?;
        let img_buf = glycin_utils::ImgBuf::Vec(v);
        let (frame, img_buf) =
            glycin_utils::editing::change_memory_format(img_buf, frame, memory_format)
                .expected_error()?;

        let width = frame.width;
        let height = frame.height;

        let heif_chroma = heif_chroma(frame.memory_format).internal_error()?;
        let mut image = Image::new(width, height, ColorSpace::Rgb(heif_chroma)).expected_error()?;

        image
            .create_plane(Channel::Interleaved, width, height, 8)
            .expected_error()?;

        if let Some(icc_profile) = &frame.details.color_icc_profile {
            image
                .set_color_profile_raw(&ColorProfileRaw::new(
                    four_cc::FourCC(*b"prof"),
                    icc_profile.get_full().internal_error()?,
                ))
                .expected_error()?;
        }

        let plane = image.planes_mut().interleaved.internal_error()?;
        let new_stride = width as usize * memory_format.n_bytes().usize();

        for y in 0..height as usize {
            for x in 0..new_stride {
                plane.data[plane.stride * y + x] = img_buf[y * new_stride + x];
            }
        }

        // Encode image and save it into file.
        let lib_heif = LibHeif::new();
        let mut context = HeifContext::new().expected_error()?;

        let format = match mime_type.as_str() {
            "image/heif" => CompressionFormat::Hevc,
            "image/avif" => CompressionFormat::Av1,
            _ => {
                return Err(glycin_utils::ProcessError::UnsupportedImageFormat(
                    mime_type,
                ));
            }
        };
        let mut encoder = lib_heif.encoder_for_format(format).expected_error()?;

        encoder
            .set_quality(EncoderQuality::Lossy(
                encoding_options.quality.unwrap_or(90),
            ))
            .expected_error()?;

        context
            .encode_image(&image, &mut encoder, None)
            .expected_error()?;

        let bytes = context.write_to_bytes().expected_error()?;
        let data = BinaryData::from_data(bytes).expected_error()?;

        Ok(glycin_utils::EncodedImage::new(data))
    }
}

fn heif_chroma(memory_format: glycin_utils::MemoryFormat) -> Option<RgbChroma> {
    Some(match memory_format {
        glycin_utils::MemoryFormat::R8g8b8 => RgbChroma::Rgb,
        glycin_utils::MemoryFormat::R8g8b8a8 => RgbChroma::Rgba,
        _ => return None,
    })
}
