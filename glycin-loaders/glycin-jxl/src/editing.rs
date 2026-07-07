use std::io::Read;

use glycin_common::ChannelType;
use glycin_utils::{
    ByteData, EditorImplementation, GenericContexts, MemoryFormatInfo, MemoryFormatSelection,
};
use jpegxl_rs::encode::{EncoderFrame, Metadata};

pub struct ImgEditor {
    mime_type: String,
}

impl EditorImplementation for ImgEditor {
    fn edit<S: Read>(
        _stream: S,
        mime_type: String,
        _details: glycin_utils::InitializationDetails,
    ) -> Result<Self, glycin_utils::ProcessError> {
        Err(glycin_utils::RemoteError::UnsupportedImageFormat(
            mime_type.clone(),
        ))
        .expected_error()
    }

    fn apply_complete<B: ByteData>(
        &self,
        _operations: glycin_utils::Operations,
    ) -> Result<glycin_utils::CompleteEditorOutput<B>, glycin_utils::ProcessError> {
        Err(glycin_utils::RemoteError::UnsupportedImageFormat(
            self.mime_type.clone(),
        ))
        .expected_error()
    }

    fn create<B: ByteData>(
        _mime_type: String,
        mut new_image: glycin_utils::NewImage<B>,
        encoding_options: glycin_utils::EncodingOptions,
    ) -> Result<glycin_utils::EncodedImage<B>, glycin_utils::ProcessError> {
        let frame = new_image.frames.remove(0);

        let mut encoder = jpegxl_rs::encoder_builder().build().internal_error()?;

        // You can change the settings after initialization
        if let Some(quality) = encoding_options.quality {
            encoder.quality = quality as f32 / 100. * 15.;
        }

        if let Some(exif) = new_image.image_info.metadata_exif {
            encoder
                .add_metadata(&Metadata::Exif(&exif), true)
                .expected_error()?;
        }

        if let Some(xmp) = new_image.image_info.metadata_xmp {
            encoder
                .add_metadata(&Metadata::Xmp(&xmp), true)
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

        let mut frame = frame.into_fungible();
        glycin_utils::editing::change_memory_format(&mut frame, memory_format).expected_error()?;

        let num_channels = memory_format.n_channels() as u32;

        let encoder_result = match memory_format.channel_type() {
            ChannelType::U8 => encoder.encode_frame::<u8, u8>(
                &EncoderFrame::new(&frame.texture).num_channels(num_channels),
                frame.width,
                frame.height,
            ),
            _ => unreachable!(),
        }
        .expected_error()?;

        let data = B::try_from_vec(encoder_result.data).expected_error()?;

        Ok(glycin_utils::EncodedImage::new(data))
    }
}
