use std::collections::BTreeMap;
use std::ops::Deref;
use std::os::fd::AsRawFd;
use std::sync::Arc;
use std::time::Duration;

use memmap::MmapMut;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zbus::zvariant::{self, DeserializeDict, Optional, SerializeDict, Type};

use crate::error::DimensionTooLargerError;
use crate::memory_format::{MemoryFormat, MemoryFormatInfo};
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

#[derive(DeserializeDict, SerializeDict, Type, Debug, Clone, Default)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct FrameRequest {
    /// Scale image to these dimensions
    pub scale: Option<(u32, u32)>,
    /// Instruction to only decode part of the image
    pub clip: Option<(u32, u32, u32, u32)>,
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
    pub color_iccp: Option<BinaryData>,
    /// Coding-independent code points (HDR information)
    pub color_cicp: Option<Vec<u8>>,
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

#[derive(zvariant::Type, Debug, Clone)]
#[zvariant(signature = "h")]
pub struct BinaryData {
    pub(crate) memfd: Arc<zvariant::OwnedFd>,
}

impl Serialize for BinaryData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.memfd.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BinaryData {
    fn deserialize<D>(deserializer: D) -> Result<BinaryData, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self {
            memfd: Arc::new(zvariant::OwnedFd::deserialize(deserializer)?),
        })
    }
}

impl AsRawFd for BinaryData {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.memfd.as_raw_fd()
    }
}

impl AsRawFd for &BinaryData {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.memfd.as_raw_fd()
    }
}

impl BinaryData {
    /// Get a copy of the binary data
    pub fn get_full(&self) -> std::io::Result<Vec<u8>> {
        Ok(self.get()?.to_vec())
    }

    /// Get a reference to the binary data
    pub fn get(&self) -> std::io::Result<BinaryDataRef> {
        Ok(BinaryDataRef {
            mmap: { unsafe { memmap::MmapOptions::new().map_copy_read_only(&self.memfd)? } },
        })
    }
}

#[derive(Debug)]
pub struct BinaryDataRef {
    mmap: memmap::Mmap,
}

impl Deref for BinaryDataRef {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.mmap.deref()
    }
}

impl AsRef<[u8]> for BinaryDataRef {
    fn as_ref(&self) -> &[u8] {
        self.mmap.deref()
    }
}
