use std::collections::BTreeMap;
use std::io::Read;
use std::time::Duration;

use glycin_common::{ColorProfilePreference, MemoryFormat, MemoryFormatInfo};
use gufo_common::orientation::Orientation;
#[cfg(feature = "external")]
use zbus::zvariant::as_value::{self, optional};
#[cfg(feature = "external")]
use zbus::zvariant::{self, Optional, Type};

use crate::error::DimensionTooLargerError;
use crate::safe_math::{SafeConversion, SafeMath};
use crate::{ByteData, FungibleMemory, Limits, MemoryAllocationError, ProcessError};

pub trait LoaderImplementation: Send + Sync + Sized + 'static {
    fn load<B: ByteData, R: Read + Send + 'static>(
        stream: R,
        mime_type: String,
        details: InitializationDetails,
    ) -> Result<(Self, ImageDetails<B>), ProcessError>;

    fn specific_frame<T: ByteData>(
        &mut self,
        frame_request: FrameRequest,
    ) -> Result<Frame<T>, ProcessError>;
}

#[cfg(feature = "external")]
#[derive(serde::Deserialize, serde::Serialize, Type, Debug)]
pub struct InitRequest {
    /// Source from which the loader reads the image data
    pub fd: zvariant::OwnedFd,
    pub mime_type: String,
    pub details: InitializationDetails,
}

#[derive(Debug, Default)]
#[cfg_attr(
    feature = "external",
    derive(serde::Deserialize, serde::Serialize, Type)
)]
#[cfg_attr(feature = "external", zvariant(signature = "a{sv}"))]
#[cfg_attr(feature = "external", serde(default))]
#[non_exhaustive]
pub struct InitializationDetails {
    #[cfg_attr(
        feature = "external",
        serde(with = "optional", skip_serializing_if = "Option::is_none")
    )]
    pub base_dir: Option<std::path::PathBuf>,
    #[cfg_attr(feature = "external", serde(with = "as_value"))]
    pub limits: Limits,
}

#[cfg(feature = "external")]
const fn true_const() -> bool {
    true
}

#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "external",
    derive(serde::Deserialize, serde::Serialize, Type)
)]
#[cfg_attr(feature = "external", zvariant(signature = "dict"))]
#[non_exhaustive]
pub struct FrameRequest {
    /// Scale image to these dimensions
    #[cfg_attr(
        feature = "external",
        serde(with = "optional", skip_serializing_if = "Option::is_none", default)
    )]
    pub scale: Option<(u32, u32)>,
    /// Instruction to only decode part of the image
    #[cfg_attr(
        feature = "external",
        serde(with = "optional", skip_serializing_if = "Option::is_none", default)
    )]
    pub clip: Option<(u32, u32, u32, u32)>,
    /// Get first frame, if previously selected frame was the last one
    #[cfg_attr(feature = "external", serde(with = "as_value", default = "true_const"))]
    pub loop_animation: bool,
}

impl Default for FrameRequest {
    fn default() -> Self {
        Self {
            scale: None,
            clip: None,
            loop_animation: true,
        }
    }
}

/// Various image metadata
///
/// This is returned from the initial `InitRequest` call
#[cfg(feature = "external")]
#[derive(Debug, Type, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "B: ByteData + serde::Serialize + zbus::zvariant::Type + 'static",
    deserialize = "B: ByteData + serde::de::DeserializeOwned + zbus::zvariant::Type + 'static"
))]
#[non_exhaustive]
pub struct RemoteImage<B: ByteData> {
    pub frame_request: zvariant::OwnedObjectPath,
    pub details: ImageDetails<B>,
}

#[cfg(feature = "external")]
impl<B: ByteData> RemoteImage<B> {
    pub fn new(details: ImageDetails<B>, frame_request: zvariant::OwnedObjectPath) -> Self {
        Self {
            frame_request,
            details,
        }
    }

    pub async fn initial_seal(&mut self) -> Result<(), MemoryAllocationError> {
        self.details.initial_seal().await
    }

    pub async fn final_seal(&mut self) -> Result<(), MemoryAllocationError> {
        self.details.final_seal().await
    }
}

#[derive(Debug)]
#[cfg_attr(
    feature = "external",
    derive(Type, serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "external", zvariant(signature = "dict"))]
#[cfg_attr(
    feature = "external",
    serde(bound(
        serialize = "B: ByteData + serde::Serialize + zbus::zvariant::Type + 'static",
        deserialize = "B: ByteData + serde::de::DeserializeOwned + zbus::zvariant::Type + 'static"
    ))
)]
#[non_exhaustive]
pub struct ImageDetails<B: ByteData> {
    /// Early dimension information.
    ///
    /// This information is often correct. However, it should only be used for
    /// an early rendering estimates. For everything else, the specific frame
    /// information should be used.
    #[cfg_attr(feature = "external", serde(with = "as_value"))]
    pub width: u32,
    #[cfg_attr(feature = "external", serde(with = "as_value"))]
    pub height: u32,
    /// Image dimensions in inch
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub dimensions_inch: Option<(f64, f64)>,
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub info_format_name: Option<String>,
    /// Textual description of the image dimensions
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub info_dimensions_text: Option<String>,
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub metadata_exif: Option<B>,
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub metadata_xmp: Option<B>,
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub metadata_key_value: Option<BTreeMap<String, String>>,
    #[cfg_attr(feature = "external", serde(with = "as_value"))]
    pub transformation_ignore_exif: bool,
    /// Explicit orientation. If `None` check Exif or XMP.
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub transformation_orientation: Option<Orientation>,
}

impl<B: ByteData> ImageDetails<B> {
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

    pub fn into_fungible(self) -> ImageDetails<FungibleMemory> {
        ImageDetails {
            width: self.width,
            height: self.height,
            dimensions_inch: self.dimensions_inch,
            info_format_name: self.info_format_name,
            info_dimensions_text: self.info_dimensions_text,
            metadata_exif: self.metadata_exif.map(B::into_fungible),
            metadata_xmp: self.metadata_xmp.map(B::into_fungible),
            metadata_key_value: self.metadata_key_value,
            transformation_ignore_exif: self.transformation_ignore_exif,
            transformation_orientation: self.transformation_orientation,
        }
    }

    pub fn into_other<O: ByteData>(self) -> Result<ImageDetails<O>, MemoryAllocationError> {
        Ok(ImageDetails {
            width: self.width,
            height: self.height,
            dimensions_inch: self.dimensions_inch,
            info_format_name: self.info_format_name,
            info_dimensions_text: self.info_dimensions_text,
            metadata_exif: self.metadata_exif.map(|x| x.into_other()).transpose()?,
            metadata_xmp: self.metadata_xmp.map(|x| x.into_other()).transpose()?,
            metadata_key_value: self.metadata_key_value,
            transformation_ignore_exif: self.transformation_ignore_exif,
            transformation_orientation: self.transformation_orientation,
        })
    }

    pub async fn initial_seal(&mut self) -> Result<(), MemoryAllocationError> {
        if let Some(metadata_exif) = &mut self.metadata_exif {
            metadata_exif.initial_seal().await?;
        }

        if let Some(metadata_xmp) = &mut self.metadata_xmp {
            metadata_xmp.initial_seal().await?;
        }

        Ok(())
    }

    pub async fn final_seal(&mut self) -> Result<(), MemoryAllocationError> {
        if let Some(metadata_exif) = &mut self.metadata_exif {
            metadata_exif.final_seal().await?;
        }

        if let Some(metadata_xmp) = &mut self.metadata_xmp {
            metadata_xmp.final_seal().await?;
        }

        Ok(())
    }
}

impl<B: ByteData> Default for FrameDetails<B> {
    fn default() -> Self {
        Self {
            color_icc_profile: None,
            color_cicp: None,
            color_profile_preference: None,
            info_bit_depth: None,
            info_alpha_channel: None,
            info_grayscale: None,
            n_frame: None,
        }
    }
}

#[cfg(not(feature = "external"))]
pub type Optional<T> = Option<T>;

#[derive(Debug)]
#[cfg_attr(feature = "external", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "external",
    serde(bound(
        serialize = "B: ByteData + serde::Serialize + zbus::zvariant::Type + 'static",
        deserialize = "B: ByteData + serde::de::DeserializeOwned + zbus::zvariant::Type + 'static"
    ))
)]
pub struct Frame<B: ByteData> {
    pub width: u32,
    pub height: u32,
    /// Line stride
    pub stride: u32,
    pub memory_format: MemoryFormat,
    pub texture: B,
    /// Duration to show frame for animations.
    ///
    /// If the value is not set, the image is not animated.
    pub delay: Optional<Duration>,
    pub details: FrameDetails<B>,
}

#[cfg(feature = "external")]
impl<B: ByteData + zvariant::Type> zvariant::Type for Frame<B> {
    const SIGNATURE: &'static zvariant::Signature = <(
        u32,
        u32,
        u32,
        MemoryFormat,
        B,
        Optional<Duration>,
        FrameDetails<B>,
    )>::SIGNATURE;
}

impl<B: ByteData> Frame<B> {
    pub fn new(
        width: u32,
        height: u32,
        memory_format: MemoryFormat,
        texture: B,
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

    pub fn n_bytes(&self) -> Result<usize, DimensionTooLargerError> {
        self.stride.try_usize()?.smul(self.height.try_usize()?)
    }

    pub fn into_fungible(self) -> Frame<FungibleMemory> {
        Frame {
            width: self.width,
            height: self.height,
            stride: self.stride,
            memory_format: self.memory_format,
            texture: self.texture.into_fungible(),
            delay: self.delay,
            details: self.details.into_fungible(),
        }
    }

    pub fn into_other<O: ByteData>(self) -> Result<Frame<O>, MemoryAllocationError> {
        Ok(Frame {
            width: self.width,
            height: self.height,
            stride: self.stride,
            memory_format: self.memory_format,
            texture: self.texture.into_other()?,
            delay: self.delay,
            details: self.details.into_other()?,
        })
    }

    pub fn desc(&self) -> String {
        format!(
            "{}x{} stride: {}, natural_stride: {}",
            self.width,
            self.height,
            self.stride,
            self.width * self.memory_format.n_bytes().u32()
        )
    }

    pub async fn initial_seal(&mut self) -> Result<(), MemoryAllocationError> {
        self.texture.initial_seal().await?;
        self.details.initial_seal().await
    }

    pub async fn final_seal(&mut self) -> Result<(), MemoryAllocationError> {
        self.texture.final_seal().await?;
        self.details.final_seal().await
    }
}

#[derive(Debug)]
#[cfg_attr(
    feature = "external",
    derive(Type, serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "external", zvariant(signature = "dict"))]
#[cfg_attr(
    feature = "external",
    serde(bound(
        serialize = "B: ByteData + serde::Serialize + zbus::zvariant::Type + 'static",
        deserialize = "B: ByteData + serde::de::DeserializeOwned + zbus::zvariant::Type + 'static"
    ))
)]
//#[serde(bound(deserialize = "B: ByteData"))]
#[non_exhaustive]
/// More information about a frame
pub struct FrameDetails<B: ByteData> {
    /// ICC color profile
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub color_icc_profile: Option<B>,
    /// Coding-independent code points (HDR information)
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub color_cicp: Option<[u8; 4]>,
    /// Which profile to use if ICC profile and CICP are defined
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub color_profile_preference: Option<ColorProfilePreference>,
    /// Bit depth per channel
    ///
    /// Only set if it can differ for the format
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub info_bit_depth: Option<u8>,
    /// Image has alpha channel
    ///
    /// Only set if it can differ for the format
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub info_alpha_channel: Option<bool>,
    /// Image uses grayscale mode
    ///
    /// Only set if it can differ for the format
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub info_grayscale: Option<bool>,
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub n_frame: Option<u64>,
}

impl<B: ByteData> FrameDetails<B> {
    pub fn into_fungible(self) -> FrameDetails<FungibleMemory> {
        FrameDetails {
            color_icc_profile: self.color_icc_profile.map(B::into_fungible),
            color_cicp: self.color_cicp,
            color_profile_preference: self.color_profile_preference,
            info_bit_depth: self.info_bit_depth,
            info_alpha_channel: self.info_alpha_channel,
            info_grayscale: self.info_grayscale,
            n_frame: self.n_frame,
        }
    }

    pub fn into_other<O: ByteData>(self) -> Result<FrameDetails<O>, MemoryAllocationError> {
        Ok(FrameDetails {
            color_icc_profile: self.color_icc_profile.map(B::into_other).transpose()?,
            color_cicp: self.color_cicp,
            color_profile_preference: self.color_profile_preference,
            info_bit_depth: self.info_bit_depth,
            info_alpha_channel: self.info_alpha_channel,
            info_grayscale: self.info_grayscale,
            n_frame: self.n_frame,
        })
    }

    pub async fn initial_seal(&mut self) -> Result<(), MemoryAllocationError> {
        if let Some(color_icc_profile) = &mut self.color_icc_profile {
            color_icc_profile.initial_seal().await?;
        }

        Ok(())
    }

    pub async fn final_seal(&mut self) -> Result<(), MemoryAllocationError> {
        if let Some(color_icc_profile) = &mut self.color_icc_profile {
            color_icc_profile.final_seal().await?;
        }

        Ok(())
    }
}
