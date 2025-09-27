use std::collections::BTreeMap;
use std::os::fd::AsRawFd;
use std::time::Duration;

use glycin_common::{BinaryData, MemoryFormat, MemoryFormatInfo};
use gufo_common::orientation::Orientation;
use memmap::MmapMut;
use serde::{Deserialize, Serialize};
use zbus::zvariant::as_value::{self, optional};
use zbus::zvariant::{self, DeserializeDict, Optional, SerializeDict, Type};

use crate::error::DimensionTooLargerError;
use crate::safe_math::{SafeConversion, SafeMath};
use crate::ImgBuf;

#[derive(Deserialize, Serialize, Type, Debug)]
pub struct InitRequest {
    /// Source from which the loader reads the image data
    pub fd: zvariant::OwnedFd,
    pub mime_type: String,
    pub details: InitializationDetails,
}

#[derive(DeserializeDict, SerializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct InitializationDetails {
    pub base_dir: Option<std::path::PathBuf>,
}

#[derive(Deserialize, Serialize, Type, Debug, Clone, Default)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct FrameRequest {
    /// Scale image to these dimensions
    #[serde(with = "optional", skip_serializing_if = "Option::is_none", default)]
    pub scale: Option<(u32, u32)>,
    /// Instruction to only decode part of the image
    #[serde(with = "optional", skip_serializing_if = "Option::is_none", default)]
    pub clip: Option<(u32, u32, u32, u32)>,
    /// Get first frame, if previously selected frame was the last one
    #[serde(with = "as_value", skip_serializing_if = "std::ops::Not::not", default)]
    pub loop_animation: bool,
}

/// Various image metadata
///
/// This is returned from the initial `InitRequest` call
#[derive(Deserialize, Serialize, Type, Debug, Clone)]
pub struct RemoteImage {
    pub frame_request: zvariant::OwnedObjectPath,
    pub details: ImageDetails,
}

impl RemoteImage {
    pub fn new(details: ImageDetails, frame_request: zvariant::OwnedObjectPath) -> Self {
        Self {
            frame_request,
            details,
        }
    }
}

#[derive(DeserializeDict, SerializeDict, Type, Debug, Clone, Default)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct ImageDetails {
    /// Early dimension information.
    ///
    /// This information is often correct. However, it should only be used for
    /// an early rendering estimates. For everything else, the specific frame
    /// information should be used.
    pub width: u32,
    pub height: u32,
    /// Image dimensions in inch
    pub dimensions_inch: Option<(f64, f64)>,
    pub info_format_name: Option<String>,
    /// Textual description of the image dimensions
    pub info_dimensions_text: Option<String>,
    pub metadata_exif: Option<BinaryData>,
    pub metadata_xmp: Option<BinaryData>,
    pub metadata_key_value: Option<BTreeMap<String, String>>,
    pub transformation_ignore_exif: bool,
    /// Explicit orientation. If `None` check Exif or XMP.
    pub transformation_orientation: Option<Orientation>,
}

impl ImageDetails {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            dimensions_inch: None,
            info_dimensions_text: None,
            info_format_name: None,
            metadata_exif: None,
            metadata_xmp: None,
            metadata_key_value: None,
            transformation_ignore_exif: false,
            transformation_orientation: None,
        }
    }
}

#[derive(Deserialize, Serialize, Type, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Line stride
    pub stride: u32,
    pub memory_format: MemoryFormat,
    pub texture: BinaryData,
    /// Duration to show frame for animations.
    ///
    /// If the value is not set, the image is not animated.
    pub delay: Optional<Duration>,
    pub details: FrameDetails,
}

impl Frame {
    pub fn n_bytes(&self) -> Result<usize, DimensionTooLargerError> {
        self.stride.try_usize()?.smul(self.height.try_usize()?)
    }
}

#[derive(DeserializeDict, SerializeDict, Type, Debug, Default, Clone)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
/// More information about a frame
pub struct FrameDetails {
    /// ICC color profile
    pub color_icc_profile: Option<BinaryData>,
    /// Coding-independent code points (HDR information)
    pub color_cicp: Option<[u8; 4]>,
    /// Bit depth per channel
    ///
    /// Only set if it can differ for the format
    pub info_bit_depth: Option<u8>,
    /// Image has alpha channel
    ///
    /// Only set if it can differ for the format
    pub info_alpha_channel: Option<bool>,
    /// Image uses grayscale mode
    ///
    /// Only set if it can differ for the format
    pub info_grayscale: Option<bool>,
    pub n_frame: Option<u64>,
}

impl Frame {
    pub fn new(
        width: u32,
        height: u32,
        memory_format: MemoryFormat,
        texture: BinaryData,
    ) -> Result<Self, DimensionTooLargerError> {
        let stride = memory_format
            .n_bytes()
            .u32()
            .checked_mul(width)
            .ok_or(DimensionTooLargerError)?;

        Ok(Self {
            width,
            height,
            stride,
            memory_format,
            texture,
            delay: None.into(),
            details: Default::default(),
        })
    }
}

impl Frame {
    pub fn as_img_buf(&self) -> std::io::Result<ImgBuf> {
        let raw_fd = self.texture.as_raw_fd();
        let original_mmap = unsafe { MmapMut::map_mut(raw_fd) }?;

        Ok(ImgBuf::MMap {
            mmap: original_mmap,
            raw_fd,
        })
    }
}

/// Editable image
#[derive(Deserialize, Serialize, Type, Debug, Clone)]
pub struct RemoteEditableImage {
    pub edit_request: zvariant::OwnedObjectPath,
}

impl RemoteEditableImage {
    pub fn new(frame_request: zvariant::OwnedObjectPath) -> Self {
        Self {
            edit_request: frame_request,
        }
    }
}

#[derive(DeserializeDict, SerializeDict, Type, Debug)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct NewImage {
    pub image_info: ImageDetails,
    pub frames: Vec<Frame>,
}

impl NewImage {
    pub fn new(image_info: ImageDetails, frames: Vec<Frame>) -> Self {
        Self { image_info, frames }
    }
}

#[derive(DeserializeDict, SerializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct EncodingOptions {
    pub quality: Option<u8>,
    pub compression: Option<u8>,
}

#[derive(DeserializeDict, SerializeDict, Type, Debug)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct EncodedImage {
    pub data: BinaryData,
}

impl EncodedImage {
    pub fn new(data: BinaryData) -> Self {
        Self { data }
    }
}
