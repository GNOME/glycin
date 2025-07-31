use glycin_common::ChannelType;
use glycin_utils::{
    BinaryData, EditorImplementation, GenericContexts, MemoryFormatInfo, MemoryFormatSelection,
};
use jpegxl_rs::encode::{EncoderFrame, Metadata};

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
        _mime_type: String,
        mut new_image: glycin_utils::NewImage,
        encoding_options: glycin_utils::EncodingOptions,
    ) -> Result<glycin_utils::EncodedImage, glycin_utils::ProcessError> {
        let frame = new_image.frames.remove(0);

        let mut encoder = jpegxl_rs::encoder_builder().build().internal_error()?;

        // You can change the settings after initialization
        if let Some(quality) = encoding_options.quality {
            encoder.quality = quality as f32 / 100. * 15.;
        }

        if let Some(exif) = new_image.image_info.metadata_exif {
            encoder
                .add_metadata(&Metadata::Exif(&exif.get().internal_error()?), true)
                .expected_error()?;
        }

        if let Some(xmp) = new_image.image_info.metadata_xmp {
            encoder
                .add_metadata(&Metadata::Xmp(&xmp.get().internal_error()?), true)
                .expected_error()?;
        }

        /*
        TODO:
        | MemoryFormatSelection::R16g16b16
        | MemoryFormatSelection::R16g16b16a16
        | MemoryFormatSelection::R32g32b32Float
        | MemoryFormatSelection::R32g32b32a32Float
         */
        let memory_format = (MemoryFormatSelection::R8g8b8 | MemoryFormatSelection::R8g8b8a8)
            .best_format_for(frame.memory_format)
            .internal_error()?;

        let v = frame.texture.get_full().expected_error()?;
        let img_buf = glycin_utils::ImgBuf::Vec(v);
        let (frame, img_buf) =
            glycin_utils::editing::change_memory_format(img_buf, frame, memory_format)
                .expected_error()?;

        let num_channels = memory_format.n_channels() as u32;

        let encoder_result = match memory_format.channel_type() {
            ChannelType::U8 => encoder.encode_frame::<u8, u8>(
                &EncoderFrame::new(&img_buf).num_channels(num_channels),
                frame.width,
                frame.height,
            ),
            _ => unreachable!(),
        }
        .expected_error()?;

        let data = BinaryData::from_data(encoder_result.data).expected_error()?;

        Ok(glycin_utils::EncodedImage::new(data))
    }
}
