use glycin_common::shared_memory::SharedMemory;
use glycin_common::{BinaryData, ExtendedMemoryFormat, MemoryFormat, MemoryFormatInfo};

use super::Frame;
use crate::editing::EditingFrame;
use crate::{DimensionTooLargerError, FrameDetails, GenericContexts, ImageDetails, ProcessError};

#[derive(Default, Clone, Debug)]
pub struct Handler {
    pub format_name: Option<String>,
    pub default_bit_depth: Option<u8>,
    pub supports_two_alpha_modes: bool,
    pub supports_two_grayscale_modes: bool,
}

impl Handler {
    pub fn format_name(mut self, format_name: impl ToString) -> Self {
        self.format_name = Some(format_name.to_string());

        self
    }

    pub fn default_bit_depth(mut self, default_bit_depth: u8) -> Self {
        self.default_bit_depth = Some(default_bit_depth);

        self
    }

    pub fn supports_two_alpha_modes(mut self, supports_two_alpha_modes: bool) -> Self {
        self.supports_two_alpha_modes = supports_two_alpha_modes;

        self
    }

    pub fn supports_two_grayscale_modes(mut self, supports_two_grayscale_modes: bool) -> Self {
        self.supports_two_grayscale_modes = supports_two_grayscale_modes;

        self
    }

    pub fn info(&self, decoder: &mut impl image::ImageDecoder) -> ImageDetails {
        let (width, height) = decoder.dimensions();
        let mut info = ImageDetails::new(width, height);
        info.info_format_name.clone_from(&self.format_name);

        info
    }

    pub fn frame(&self, mut decoder: impl image::ImageDecoder) -> Result<Frame, ProcessError> {
        let simple_frame = self.editing_frame(&decoder)?;

        let width = simple_frame.width;
        let height = simple_frame.height;
        let color_type = decoder.color_type();
        let memory_format = memory_format_from_color_type(color_type);

        let details = self.frame_details(&mut decoder);

        let mut memory = SharedMemory::new(decoder.total_bytes()).expected_error()?;
        decoder.read_image(&mut memory).expected_error()?;
        let texture = memory.into_binary_data();

        let mut frame = Frame::new(width, height, memory_format, texture)?;
        frame.details = details.expected_error()?;

        Ok(frame)
    }

    pub fn editing_frame(
        &self,
        decoder: &impl image::ImageDecoder,
    ) -> Result<EditingFrame, ProcessError> {
        let color_type = decoder.color_type();
        let memory_format = ExtendedMemoryFormat::from(memory_format_from_color_type(color_type));
        let (width, height) = decoder.dimensions();
        let stride = memory_format
            .n_bytes()
            .u32()
            .checked_mul(width)
            .ok_or(DimensionTooLargerError)?;

        Ok(EditingFrame {
            width,
            height,
            stride,
            memory_format,
        })
    }

    pub fn frame_details(
        &self,
        decoder: &mut impl image::ImageDecoder,
    ) -> Result<FrameDetails, ProcessError> {
        let mut details = FrameDetails {
            color_icc_profile: decoder
                .icc_profile()
                .ok()
                .flatten()
                .map(BinaryData::from_data)
                .transpose()
                .expected_error()?,
            ..Default::default()
        };

        if let Some((alpha_channel, grayscale, bits)) =
            channel_details(decoder.original_color_type())
        {
            if self.default_bit_depth != Some(bits) {
                details.info_bit_depth = Some(bits);
            }
            if self.supports_two_alpha_modes {
                details.info_alpha_channel = Some(alpha_channel);
            }
            if self.supports_two_grayscale_modes {
                details.info_grayscale = Some(grayscale);
            }
        }

        Ok(details)
    }
}

/*
impl ImageInfo {
    pub fn from_decoder(
        decoder: &mut impl image::ImageDecoder,
        _format_name: impl ToString,
    ) -> Self {
        let (width, height) = decoder.dimensions();

        Self::new(width, height)
    }
}
     */

pub fn memory_format_to_color_type(memory_format: &MemoryFormat) -> Option<image::ColorType> {
    match memory_format {
        MemoryFormat::G8 => Some(image::ColorType::L8),
        MemoryFormat::G8a8 => Some(image::ColorType::La8),
        MemoryFormat::R8g8b8 => Some(image::ColorType::Rgb8),
        MemoryFormat::R8g8b8a8 => Some(image::ColorType::Rgba8),
        MemoryFormat::G16 => Some(image::ColorType::L16),
        MemoryFormat::G16a16 => Some(image::ColorType::La16),
        MemoryFormat::R16g16b16 => Some(image::ColorType::Rgb16),
        MemoryFormat::R16g16b16a16 => Some(image::ColorType::Rgba16),
        MemoryFormat::R32g32b32Float => Some(image::ColorType::Rgb32F),
        MemoryFormat::R32g32b32a32Float => Some(image::ColorType::Rgba32F),
        _ => None,
    }
}

pub fn extended_memory_format_to_color_type(
    extended_memory_format: &ExtendedMemoryFormat,
) -> Option<image::ColorType> {
    match extended_memory_format {
        ExtendedMemoryFormat::Basic(basic) => memory_format_to_color_type(basic),
        _ => None,
    }
}

pub fn memory_format_from_color_type(color_type: image::ColorType) -> MemoryFormat {
    match color_type {
        image::ColorType::L8 => MemoryFormat::G8,
        image::ColorType::La8 => MemoryFormat::G8a8,
        image::ColorType::Rgb8 => MemoryFormat::R8g8b8,
        image::ColorType::Rgba8 => MemoryFormat::R8g8b8a8,
        image::ColorType::L16 => MemoryFormat::G16,
        image::ColorType::La16 => MemoryFormat::G16a16,
        image::ColorType::Rgb16 => MemoryFormat::R16g16b16,
        image::ColorType::Rgba16 => MemoryFormat::R16g16b16a16,
        image::ColorType::Rgb32F => MemoryFormat::R32g32b32Float,
        image::ColorType::Rgba32F => MemoryFormat::R32g32b32a32Float,
        _ => unimplemented!(),
    }
}

pub fn channel_details(color_type: image::ExtendedColorType) -> Option<(bool, bool, u8)> {
    Some(match color_type {
        image::ExtendedColorType::A8 => (true, false, 8),
        image::ExtendedColorType::L1 => (false, true, 1),
        image::ExtendedColorType::La1 => (true, true, 1),
        image::ExtendedColorType::Rgb1 => (false, false, 1),
        image::ExtendedColorType::Rgba1 => (true, false, 1),
        image::ExtendedColorType::L2 => (false, true, 2),
        image::ExtendedColorType::La2 => (true, true, 2),
        image::ExtendedColorType::Rgb2 => (false, false, 2),
        image::ExtendedColorType::Rgba2 => (true, false, 2),
        image::ExtendedColorType::L4 => (false, true, 4),
        image::ExtendedColorType::La4 => (true, true, 4),
        image::ExtendedColorType::Rgb4 => (false, false, 4),
        image::ExtendedColorType::Rgba4 => (true, false, 4),
        image::ExtendedColorType::L8 => (false, true, 8),
        image::ExtendedColorType::La8 => (true, true, 8),
        image::ExtendedColorType::Rgb8 => (false, false, 8),
        image::ExtendedColorType::Rgba8 => (true, false, 8),
        image::ExtendedColorType::L16 => (false, true, 16),
        image::ExtendedColorType::La16 => (true, true, 16),
        image::ExtendedColorType::Rgb16 => (false, false, 16),
        image::ExtendedColorType::Rgba16 => (true, false, 16),
        image::ExtendedColorType::Bgr8 => (false, false, 8),
        image::ExtendedColorType::Bgra8 => (true, false, 8),
        image::ExtendedColorType::Rgb32F => (false, false, 32),
        image::ExtendedColorType::Rgba32F => (true, false, 32),
        image::ExtendedColorType::Unknown(bits) => (false, false, bits),
        _ => return None,
    })
}
