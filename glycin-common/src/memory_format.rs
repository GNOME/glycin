use std::io::Read;

use serde::{Deserialize, Serialize};
use zerocopy::{FromBytes, IntoBytes};
use zvariant::Type;

pub trait MemoryFormatInfo: Sized {
    fn n_bytes(self) -> MemoryFormatBytes;
    fn n_channels(self) -> u8;
}

gufo_common::maybe_convertible_enum!(
    #[repr(i32)]
    #[derive(Deserialize, Serialize, Type, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(feature = "gobject", derive(glib::Enum))]
    #[cfg_attr(feature = "gobject", enum_type(name = "GlyMemoryFormat"))]
    #[zvariant(signature = "u")]
    /// Describes the formats the image data can have.
    ///
    /// Orderings like "RGB" are independent of the systems endianness. Single
    /// channels like "R16" ([`u16`]) follow the endianness of the system.
    pub enum MemoryFormat {
        B8g8r8a8Premultiplied = 0,
        A8r8g8b8Premultiplied = 1,
        R8g8b8a8Premultiplied = 2,
        B8g8r8a8 = 3,
        A8r8g8b8 = 4,
        R8g8b8a8 = 5,
        A8b8g8r8 = 6,
        R8g8b8 = 7,
        B8g8r8 = 8,
        R16g16b16 = 9,
        R16g16b16a16Premultiplied = 10,
        R16g16b16a16 = 11,
        R16g16b16Float = 12,
        R16g16b16a16Float = 13,
        R32g32b32Float = 14,
        R32g32b32a32FloatPremultiplied = 15,
        R32g32b32a32Float = 16,
        G8a8Premultiplied = 17,
        G8a8 = 18,
        G8 = 19,
        G16a16Premultiplied = 20,
        G16a16 = 21,
        G16 = 22,
        C8m8y8k8 = 30,
        C8m8y8k8a8 = 31,
    }
);

impl MemoryFormatInfo for MemoryFormat {
    fn n_bytes(self) -> MemoryFormatBytes {
        match self {
            MemoryFormat::B8g8r8a8Premultiplied => MemoryFormatBytes::B4,
            MemoryFormat::A8r8g8b8Premultiplied => MemoryFormatBytes::B4,
            MemoryFormat::R8g8b8a8Premultiplied => MemoryFormatBytes::B4,
            MemoryFormat::B8g8r8a8 => MemoryFormatBytes::B4,
            MemoryFormat::A8r8g8b8 => MemoryFormatBytes::B4,
            MemoryFormat::R8g8b8a8 => MemoryFormatBytes::B4,
            MemoryFormat::A8b8g8r8 => MemoryFormatBytes::B4,
            MemoryFormat::R8g8b8 => MemoryFormatBytes::B3,
            MemoryFormat::B8g8r8 => MemoryFormatBytes::B3,
            MemoryFormat::R16g16b16 => MemoryFormatBytes::B6,
            MemoryFormat::R16g16b16a16Premultiplied => MemoryFormatBytes::B8,
            MemoryFormat::R16g16b16a16 => MemoryFormatBytes::B8,
            MemoryFormat::R16g16b16Float => MemoryFormatBytes::B6,
            MemoryFormat::R16g16b16a16Float => MemoryFormatBytes::B8,
            MemoryFormat::R32g32b32Float => MemoryFormatBytes::B12,
            MemoryFormat::R32g32b32a32FloatPremultiplied => MemoryFormatBytes::B16,
            MemoryFormat::R32g32b32a32Float => MemoryFormatBytes::B16,
            MemoryFormat::G8a8Premultiplied => MemoryFormatBytes::B2,
            MemoryFormat::G8a8 => MemoryFormatBytes::B2,
            MemoryFormat::G8 => MemoryFormatBytes::B1,
            MemoryFormat::G16a16Premultiplied => MemoryFormatBytes::B4,
            MemoryFormat::G16a16 => MemoryFormatBytes::B4,
            MemoryFormat::G16 => MemoryFormatBytes::B2,
            MemoryFormat::C8m8y8k8 => MemoryFormatBytes::B4,
            MemoryFormat::C8m8y8k8a8 => MemoryFormatBytes::B5,
        }
    }

    fn n_channels(self) -> u8 {
        match self {
            MemoryFormat::C8m8y8k8a8 => 5,
            MemoryFormat::B8g8r8a8Premultiplied
            | MemoryFormat::A8r8g8b8Premultiplied
            | MemoryFormat::R8g8b8a8Premultiplied
            | MemoryFormat::B8g8r8a8
            | MemoryFormat::A8r8g8b8
            | MemoryFormat::R8g8b8a8
            | MemoryFormat::A8b8g8r8
            | MemoryFormat::R16g16b16a16Premultiplied
            | MemoryFormat::R16g16b16a16
            | MemoryFormat::R16g16b16a16Float
            | MemoryFormat::R32g32b32a32FloatPremultiplied
            | MemoryFormat::R32g32b32a32Float
            | MemoryFormat::C8m8y8k8 => 4,
            MemoryFormat::R8g8b8
            | MemoryFormat::B8g8r8
            | MemoryFormat::R16g16b16
            | MemoryFormat::R16g16b16Float
            | MemoryFormat::R32g32b32Float => 3,
            MemoryFormat::G8a8Premultiplied
            | MemoryFormat::G8a8
            | MemoryFormat::G16a16Premultiplied
            | MemoryFormat::G16a16 => 2,
            MemoryFormat::G8 | MemoryFormat::G16 => 1,
        }
    }
}

impl MemoryFormat {
    pub const ALL: &[Self] = &[
        Self::B8g8r8a8Premultiplied,
        Self::A8r8g8b8Premultiplied,
        Self::R8g8b8a8Premultiplied,
        Self::B8g8r8a8,
        Self::A8r8g8b8,
        Self::R8g8b8a8,
        Self::A8b8g8r8,
        Self::R8g8b8,
        Self::B8g8r8,
        Self::R16g16b16,
        Self::R16g16b16a16Premultiplied,
        Self::R16g16b16a16,
        Self::R16g16b16Float,
        Self::R16g16b16a16Float,
        Self::R32g32b32Float,
        Self::R32g32b32a32FloatPremultiplied,
        Self::R32g32b32a32Float,
        Self::G8a8Premultiplied,
        Self::G8a8,
        Self::G8,
        Self::G16a16Premultiplied,
        Self::G16a16,
        Self::G16,
        Self::C8m8y8k8,
        Self::C8m8y8k8a8,
    ];

    pub const fn channel_type(self) -> ChannelType {
        match self {
            MemoryFormat::B8g8r8a8Premultiplied
            | MemoryFormat::A8r8g8b8Premultiplied
            | MemoryFormat::R8g8b8a8Premultiplied
            | MemoryFormat::B8g8r8a8
            | MemoryFormat::A8r8g8b8
            | MemoryFormat::R8g8b8a8
            | MemoryFormat::A8b8g8r8
            | MemoryFormat::R8g8b8
            | MemoryFormat::B8g8r8
            | MemoryFormat::G8a8Premultiplied
            | MemoryFormat::G8a8
            | MemoryFormat::G8
            | MemoryFormat::C8m8y8k8
            | MemoryFormat::C8m8y8k8a8 => ChannelType::U8,

            MemoryFormat::R16g16b16
            | MemoryFormat::R16g16b16a16Premultiplied
            | MemoryFormat::R16g16b16a16
            | MemoryFormat::G16a16Premultiplied
            | MemoryFormat::G16a16
            | MemoryFormat::G16 => ChannelType::U16,

            MemoryFormat::R16g16b16Float | MemoryFormat::R16g16b16a16Float => ChannelType::F16,

            MemoryFormat::R32g32b32Float
            | MemoryFormat::R32g32b32a32FloatPremultiplied
            | MemoryFormat::R32g32b32a32Float => ChannelType::F32,
        }
    }

    pub const fn has_alpha(self) -> bool {
        match self {
            MemoryFormat::B8g8r8a8Premultiplied
            | MemoryFormat::A8r8g8b8Premultiplied
            | MemoryFormat::R8g8b8a8Premultiplied
            | MemoryFormat::B8g8r8a8
            | MemoryFormat::A8r8g8b8
            | MemoryFormat::R8g8b8a8
            | MemoryFormat::A8b8g8r8
            | MemoryFormat::R16g16b16a16Premultiplied
            | MemoryFormat::R32g32b32a32FloatPremultiplied
            | MemoryFormat::R32g32b32a32Float
            | MemoryFormat::G8a8Premultiplied
            | MemoryFormat::G8a8
            | MemoryFormat::R16g16b16a16
            | MemoryFormat::R16g16b16a16Float
            | MemoryFormat::G16a16Premultiplied
            | MemoryFormat::G16a16
            | MemoryFormat::C8m8y8k8a8 => true,

            MemoryFormat::R8g8b8
            | MemoryFormat::B8g8r8
            | MemoryFormat::R16g16b16
            | MemoryFormat::R16g16b16Float
            | MemoryFormat::R32g32b32Float
            | MemoryFormat::G8
            | MemoryFormat::G16
            | MemoryFormat::C8m8y8k8 => false,
        }
    }

    pub const fn is_premultiplied(self) -> bool {
        match self {
            MemoryFormat::B8g8r8a8Premultiplied
            | MemoryFormat::A8r8g8b8Premultiplied
            | MemoryFormat::R8g8b8a8Premultiplied
            | MemoryFormat::R16g16b16a16Premultiplied
            | MemoryFormat::R32g32b32a32FloatPremultiplied
            | MemoryFormat::G8a8Premultiplied
            | MemoryFormat::G16a16Premultiplied => true,

            MemoryFormat::B8g8r8a8
            | MemoryFormat::A8r8g8b8
            | MemoryFormat::R8g8b8a8
            | MemoryFormat::A8b8g8r8
            | MemoryFormat::R8g8b8
            | MemoryFormat::B8g8r8
            | MemoryFormat::R16g16b16
            | MemoryFormat::R16g16b16a16
            | MemoryFormat::R16g16b16Float
            | MemoryFormat::R16g16b16a16Float
            | MemoryFormat::R32g32b32Float
            | MemoryFormat::R32g32b32a32Float
            | MemoryFormat::G8a8
            | MemoryFormat::G8
            | MemoryFormat::G16a16
            | MemoryFormat::G16
            | MemoryFormat::C8m8y8k8
            | MemoryFormat::C8m8y8k8a8 => false,
        }
    }

    /// Defines from which channels to get the RGBA values
    ///
    /// The return value is in the order `[R, G, B, A]`.
    pub const fn swizzle(self) -> NormalizeSwizzle {
        match self {
            MemoryFormat::B8g8r8a8Premultiplied | MemoryFormat::B8g8r8a8 => {
                NormalizeSwizzle::Rgba([
                    SwizzleChannel::_2,
                    SwizzleChannel::_1,
                    SwizzleChannel::_0,
                    SwizzleChannel::_3,
                ])
            }

            MemoryFormat::A8r8g8b8Premultiplied | MemoryFormat::A8r8g8b8 => {
                NormalizeSwizzle::Rgba([
                    SwizzleChannel::_1,
                    SwizzleChannel::_2,
                    SwizzleChannel::_3,
                    SwizzleChannel::_0,
                ])
            }

            MemoryFormat::R8g8b8a8Premultiplied
            | MemoryFormat::R8g8b8a8
            | MemoryFormat::R16g16b16a16Premultiplied
            | MemoryFormat::R16g16b16a16
            | MemoryFormat::R16g16b16a16Float
            | MemoryFormat::R32g32b32a32FloatPremultiplied
            | MemoryFormat::R32g32b32a32Float => NormalizeSwizzle::Rgba([
                SwizzleChannel::_0,
                SwizzleChannel::_1,
                SwizzleChannel::_2,
                SwizzleChannel::_3,
            ]),

            MemoryFormat::A8b8g8r8 => NormalizeSwizzle::Rgba([
                SwizzleChannel::_1,
                SwizzleChannel::_2,
                SwizzleChannel::_3,
                SwizzleChannel::_0,
            ]),

            MemoryFormat::R8g8b8
            | MemoryFormat::R16g16b16
            | MemoryFormat::R16g16b16Float
            | MemoryFormat::R32g32b32Float => NormalizeSwizzle::Rgba([
                SwizzleChannel::_0,
                SwizzleChannel::_1,
                SwizzleChannel::_2,
                SwizzleChannel::ConstMax,
            ]),

            MemoryFormat::B8g8r8 => NormalizeSwizzle::Rgba([
                SwizzleChannel::_2,
                SwizzleChannel::_1,
                SwizzleChannel::_0,
                SwizzleChannel::ConstMax,
            ]),

            MemoryFormat::G8a8Premultiplied
            | MemoryFormat::G8a8
            | MemoryFormat::G16a16Premultiplied
            | MemoryFormat::G16a16 => {
                NormalizeSwizzle::Ga([SwizzleChannel::_0, SwizzleChannel::_1])
            }

            MemoryFormat::G8 | MemoryFormat::G16 => {
                NormalizeSwizzle::Ga([SwizzleChannel::_0, SwizzleChannel::ConstMax])
            }

            MemoryFormat::C8m8y8k8 => NormalizeSwizzle::Cmyka([
                SwizzleChannel::_0,
                SwizzleChannel::_1,
                SwizzleChannel::_2,
                SwizzleChannel::_3,
                SwizzleChannel::ConstMax,
            ]),

            MemoryFormat::C8m8y8k8a8 => NormalizeSwizzle::Cmyka([
                SwizzleChannel::_0,
                SwizzleChannel::_1,
                SwizzleChannel::_2,
                SwizzleChannel::_3,
                SwizzleChannel::_4,
            ]),
        }
    }

    pub const fn target_definition(self) -> TargetSwizzle<'static> {
        match self {
            MemoryFormat::B8g8r8a8Premultiplied | MemoryFormat::B8g8r8a8 => {
                TargetSwizzle::Rgba(&[TargetRgba::B, TargetRgba::G, TargetRgba::R, TargetRgba::A])
            }
            MemoryFormat::A8r8g8b8Premultiplied | MemoryFormat::A8r8g8b8 => {
                TargetSwizzle::Rgba(&[TargetRgba::A, TargetRgba::R, TargetRgba::G, TargetRgba::B])
            }
            MemoryFormat::R8g8b8a8Premultiplied
            | MemoryFormat::R8g8b8a8
            | MemoryFormat::R16g16b16a16Premultiplied
            | MemoryFormat::R16g16b16a16
            | MemoryFormat::R16g16b16a16Float
            | MemoryFormat::R32g32b32a32FloatPremultiplied
            | MemoryFormat::R32g32b32a32Float => {
                TargetSwizzle::Rgba(&[TargetRgba::R, TargetRgba::G, TargetRgba::B, TargetRgba::A])
            }
            MemoryFormat::A8b8g8r8 => {
                TargetSwizzle::Rgba(&[TargetRgba::A, TargetRgba::B, TargetRgba::G, TargetRgba::R])
            }
            MemoryFormat::R8g8b8
            | MemoryFormat::R16g16b16
            | MemoryFormat::R16g16b16Float
            | MemoryFormat::R32g32b32Float => {
                TargetSwizzle::Rgba(&[TargetRgba::R, TargetRgba::G, TargetRgba::B])
            }
            MemoryFormat::B8g8r8 => {
                TargetSwizzle::Rgba(&[TargetRgba::B, TargetRgba::G, TargetRgba::R])
            }
            MemoryFormat::G8a8Premultiplied
            | MemoryFormat::G8a8
            | MemoryFormat::G16a16Premultiplied
            | MemoryFormat::G16a16 => TargetSwizzle::Ga(&[TargetGa::G, TargetGa::A]),
            MemoryFormat::G8 | MemoryFormat::G16 => TargetSwizzle::Ga(&[TargetGa::G]),
            MemoryFormat::C8m8y8k8 => TargetSwizzle::Cmyk(&[
                TargetCmyka::C,
                TargetCmyka::M,
                TargetCmyka::Y,
                TargetCmyka::K,
            ]),
            MemoryFormat::C8m8y8k8a8 => TargetSwizzle::Cmyk(&[
                TargetCmyka::C,
                TargetCmyka::M,
                TargetCmyka::Y,
                TargetCmyka::K,
                TargetCmyka::A,
            ]),
        }
    }

    #[inline]
    pub fn transform(src_format: Self, src: &[u8], target_format: Self, target: &mut [u8]) {
        let channels_f32 = Self::to_pixel(src_format, src);
        Self::from_pixel(channels_f32, target_format, target);
    }

    #[inline]
    pub fn to_pixel(src_format: Self, mut src: &[u8]) -> NoramlizedPixel {
        match src_format.channel_type() {
            ChannelType::U8 => {
                Self::to_f32_internal::<u8>(FromBytes::ref_from_bytes(src).unwrap(), src_format)
            }
            ChannelType::U16 => {
                Self::to_f32_internal::<u16>(FromBytes::ref_from_bytes(src).unwrap(), src_format)
            }
            ChannelType::F16 => {
                let bytes = &mut [0; 2];
                let mut f16_data = Vec::new();
                while let Ok(()) = src.read_exact(bytes) {
                    f16_data.push(half::f16::from_ne_bytes(*bytes));
                }
                Self::to_f32_internal::<half::f16>(&f16_data, src_format)
            }
            ChannelType::F32 => {
                Self::to_f32_internal::<f32>(FromBytes::ref_from_bytes(src).unwrap(), src_format)
            }
        }
    }

    pub const fn color_model(self) -> ColorModel {
        self.target_definition().color_model()
    }

    #[inline]
    fn to_f32_internal<T: ChannelValue>(
        source_channels: &[T],
        source_format: Self,
    ) -> NoramlizedPixel {
        match source_format.swizzle() {
            NormalizeSwizzle::Rgba(swizzle) => {
                let mut channels_f32 = [0.; 4];
                for (n, channel) in channels_f32.iter_mut().enumerate() {
                    *channel = swizzle[n].take(source_channels).to_f32_normed();
                }

                if source_format.is_premultiplied() && channels_f32[3] > 0. {
                    channels_f32[0] /= channels_f32[3];
                    channels_f32[1] /= channels_f32[3];
                    channels_f32[2] /= channels_f32[3];
                }

                NoramlizedPixel::Rgba(channels_f32)
            }
            NormalizeSwizzle::Ga(swizzle) => {
                let mut channels_f32 = [0.; 2];
                for (n, channel) in channels_f32.iter_mut().enumerate() {
                    *channel = swizzle[n].take(source_channels).to_f32_normed();
                }

                NoramlizedPixel::Ga(channels_f32)
            }
            NormalizeSwizzle::Cmyka(swizzle) => {
                let mut channels_f32 = [0.; 5];
                for (n, channel) in channels_f32.iter_mut().enumerate() {
                    *channel = swizzle[n].take(source_channels).to_f32_normed();
                }

                NoramlizedPixel::Cmyka(channels_f32)
            }
        }
    }

    #[inline]
    pub(crate) fn from_pixel(pixel: NoramlizedPixel, target_format: Self, target: &mut [u8]) {
        match target_format.channel_type() {
            ChannelType::U8 => Self::from_pixel_internal::<u8>(pixel, target_format, target),
            ChannelType::U16 => Self::from_pixel_internal::<u16>(pixel, target_format, target),
            ChannelType::F16 => {
                Self::from_pixel_internal::<half::f16>(pixel, target_format, target)
            }
            ChannelType::F32 => Self::from_pixel_internal::<f32>(pixel, target_format, target),
        }
    }

    #[inline]
    fn from_pixel_internal<T: ChannelValue>(
        pixel: NoramlizedPixel,
        target_format: Self,
        target: &mut [u8],
    ) {
        let target_channel_size = target_format.channel_type().size() as usize;
        let target_definition = target_format.target_definition();

        let mut pixel = pixel.to_color_model_internal(target_definition.color_model());
        let target_definition = target_definition.into_iter_usize();

        if target_format.is_premultiplied() {
            // Premultiply color channels
            pixel[0] *= pixel[3];
            pixel[1] *= pixel[3];
            pixel[2] *= pixel[3];
        }

        for (def, chunk) in target_definition.zip(target.chunks_exact_mut(target_channel_size)) {
            let new_channel = T::from_f32_normed(pixel[def]);
            chunk.copy_from_slice(new_channel.as_bytes_wrapper());
        }
    }

    pub fn try_from_str(s: &str) -> Option<Self> {
        Some(match s {
            "B8g8r8a8Premultiplied" => Self::B8g8r8a8Premultiplied,
            "A8r8g8b8Premultiplied" => Self::A8r8g8b8Premultiplied,
            "R8g8b8a8Premultiplied" => Self::R8g8b8a8Premultiplied,
            "B8g8r8a8" => Self::B8g8r8a8,
            "A8r8g8b8" => Self::A8r8g8b8,
            "R8g8b8a8" => Self::R8g8b8a8,
            "A8b8g8r8" => Self::A8b8g8r8,
            "R8g8b8" => Self::R8g8b8,
            "B8g8r8" => Self::B8g8r8,
            "R16g16b16" => Self::R16g16b16,
            "R16g16b16a16Premultiplied" => Self::R16g16b16a16Premultiplied,
            "R16g16b16a16" => Self::R16g16b16a16,
            "R16g16b16Float" => Self::R16g16b16Float,
            "R16g16b16a16Float" => Self::R16g16b16a16Float,
            "R32g32b32Float" => Self::R32g32b32Float,
            "R32g32b32a32FloatPremultiplied" => Self::R32g32b32a32FloatPremultiplied,
            "R32g32b32a32Float" => Self::R32g32b32a32Float,
            "G8a8Premultiplied" => Self::G8a8Premultiplied,
            "G8a8" => Self::G8a8,
            "G8" => Self::G8,
            "G16a16Premultiplied" => Self::G16a16Premultiplied,
            "G16a16" => Self::G16a16,
            "G16" => Self::G16,
            _ => return None,
        })
    }

    pub const fn display(&self) -> &'static str {
        match self {
            Self::B8g8r8a8Premultiplied => "BGRA8 Premultiplied",
            Self::A8r8g8b8Premultiplied => "ARGB8 Premultiplied",
            Self::R8g8b8a8Premultiplied => "RGBA8 Premultiplied",
            Self::B8g8r8a8 => "BGRA8",
            Self::A8r8g8b8 => "ARGB8",
            Self::R8g8b8a8 => "RGBA8",
            Self::A8b8g8r8 => "ABGR8",
            Self::R8g8b8 => "RGB8",
            Self::B8g8r8 => "BGR8",
            Self::R16g16b16 => "RGB16",
            Self::R16g16b16a16Premultiplied => "RGBA16 Premultiplied",
            Self::R16g16b16a16 => "RGBA16",
            Self::R16g16b16Float => "RGB16Float",
            Self::R16g16b16a16Float => "RGBA16Float",
            Self::R32g32b32Float => "RGB32Float",
            Self::R32g32b32a32FloatPremultiplied => "RGBA32Float Premultiplied",
            Self::R32g32b32a32Float => "RGBA32Float",
            Self::G8a8Premultiplied => "GA8 Premultiplied",
            Self::G8a8 => "GA8",
            Self::G8 => "G8",
            Self::G16a16Premultiplied => "GA16 Premultiplied",
            Self::G16a16 => "GA16",
            Self::G16 => "G16",
            Self::C8m8y8k8 => "CMYK8",
            Self::C8m8y8k8a8 => "CMYKA8",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ExtendedMemoryFormat {
    Basic(MemoryFormat),
    Y8Cb8Cr8,
    Y8Cb8Cr8K8,
}

impl MemoryFormatInfo for ExtendedMemoryFormat {
    fn n_bytes(self) -> MemoryFormatBytes {
        match self {
            Self::Basic(basic) => basic.n_bytes(),
            Self::Y8Cb8Cr8 => MemoryFormatBytes::B3,
            Self::Y8Cb8Cr8K8 => MemoryFormatBytes::B4,
        }
    }

    fn n_channels(self) -> u8 {
        match self {
            Self::Basic(basic) => basic.n_channels(),
            Self::Y8Cb8Cr8 => 3,
            Self::Y8Cb8Cr8K8 => 4,
        }
    }
}

trait ChannelValue:
    Default + Copy + std::ops::Add<Output = Self> + std::ops::Sub<Output = Self>
{
    const MAX: Self;
    fn from_f32_normed(value: f32) -> Self;
    fn to_f32_normed(self) -> f32;
    fn as_bytes_wrapper(&self) -> &[u8];
}

impl ChannelValue for u8 {
    const MAX: Self = Self::MAX;

    fn from_f32_normed(value: f32) -> Self {
        (value * Self::MAX as f32).round() as Self
    }

    fn to_f32_normed(self) -> f32 {
        (self as f32) / Self::MAX as f32
    }

    fn as_bytes_wrapper(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl ChannelValue for u16 {
    const MAX: Self = Self::MAX;

    fn from_f32_normed(value: f32) -> Self {
        (value * Self::MAX as f32).round() as Self
    }

    fn to_f32_normed(self) -> f32 {
        (self as f32) / Self::MAX as f32
    }

    fn as_bytes_wrapper(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl ChannelValue for half::f16 {
    const MAX: Self = Self::from_f32_const(1.);

    fn from_f32_normed(value: f32) -> Self {
        Self::from_f32(value)
    }

    fn to_f32_normed(self) -> f32 {
        self.into()
    }

    fn as_bytes_wrapper(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl ChannelValue for f32 {
    const MAX: Self = 1.;

    fn from_f32_normed(value: f32) -> Self {
        value
    }

    fn to_f32_normed(self) -> f32 {
        self
    }

    fn as_bytes_wrapper(&self) -> &[u8] {
        self.as_bytes()
    }
}

#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChannelType {
    U8,
    U16,
    F16,
    F32,
}

impl ChannelType {
    pub const fn size(self) -> u8 {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::F16 => 2,
            Self::F32 => 4,
        }
    }
}

impl From<MemoryFormat> for ExtendedMemoryFormat {
    fn from(value: MemoryFormat) -> Self {
        Self::Basic(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum MemoryFormatBytes {
    B1 = 1,
    B2 = 2,
    B3 = 3,
    B4 = 4,
    B5 = 5,
    B6 = 6,
    B8 = 8,
    B12 = 12,
    B16 = 16,
}

// TODO: Convert to From trait impls
impl MemoryFormatBytes {
    pub fn u8(self) -> u8 {
        self as u8
    }

    pub fn u32(self) -> u32 {
        self as u32
    }

    pub fn u64(self) -> u64 {
        self as u64
    }

    pub fn usize(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum ColorModel {
    Rgb,
    /// Grayscale
    G,
    Cmyk,
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
/// Values are not premultiplied and 0.0 to 1.0
pub enum NoramlizedPixel {
    Rgba([f32; 4]),
    Ga([f32; 2]),
    Cmyka([f32; 5]),
}

impl NoramlizedPixel {
    #[inline]
    fn to_color_model_internal(self, target: ColorModel) -> [f32; 5] {
        match (self, target) {
            (Self::Rgba([r, g, b, a]), ColorModel::Rgb) => [r, g, b, a, 0.],
            (Self::Rgba([r, g, b, a]), ColorModel::G) => {
                let avg = (r + g + b) / 3.;
                [avg, a, 0., 0., 0.]
            }
            (Self::Rgba([r, g, b, a]), ColorModel::Cmyk) => {
                let max_rgb = [r, g, b].into_iter().reduce(f32::max).unwrap();

                if max_rgb <= f32::EPSILON {
                    [0., 0., 0., 1., a]
                } else {
                    let inv_max = 1.0 / max_rgb;

                    let c = (max_rgb - r) * inv_max;
                    let m = (max_rgb - g) * inv_max;
                    let y = (max_rgb - b) * inv_max;
                    let k = 1.0 - max_rgb;
                    [c, m, y, k, a]
                }
            }

            (Self::Ga([g, a]), ColorModel::Rgb) => [g, g, g, a, 0.],
            (Self::Ga([g, a]), ColorModel::G) => [g, a, 0., 0., 0.],
            (Self::Ga([g, a]), ColorModel::Cmyk) => {
                Self::Rgba([g, g, g, a]).to_color_model_internal(ColorModel::Cmyk)
            }

            (Self::Cmyka([c, m, y, k, a]), ColorModel::Rgb) => {
                let k_inv = 1. - k;
                let r = (1. - c) * k_inv;
                let g = (1. - m) * k_inv;
                let b = (1. - y) * k_inv;

                [r, g, b, a, 0.]
            }
            (cmyk @ Self::Cmyka(_), ColorModel::G) => {
                let [r, g, b, a, _] = cmyk.to_color_model_internal(ColorModel::Rgb);
                let avg = (r + g + b) / 3.;

                [avg, a, 0., 0., 0.]
            }
            (Self::Cmyka([c, m, y, k, a]), ColorModel::Cmyk) => [c, m, y, k, a],
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub enum SwizzleChannel {
    _0,
    _1,
    _2,
    _3,
    _4,
    ConstMax = 1000,
}

impl SwizzleChannel {
    #[inline]
    fn take<T: ChannelValue>(self, channels: &[T]) -> T {
        let channel = self as usize;
        if channel == Self::ConstMax as usize {
            T::MAX
        } else {
            channels[channel]
        }
    }
}

#[derive(Debug)]
pub enum NormalizeSwizzle {
    Rgba([SwizzleChannel; 4]),
    Ga([SwizzleChannel; 2]),
    Cmyka([SwizzleChannel; 5]),
}

impl NormalizeSwizzle {
    pub fn into_iter_usize(&self) -> impl Iterator<Item = &'_ SwizzleChannel> {
        match self {
            Self::Rgba(x) => x.iter(),
            Self::Ga(x) => x.iter(),
            Self::Cmyka(x) => x.iter(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(usize)]
pub enum TargetRgba {
    R = 0,
    G = 1,
    B = 2,
    A = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(usize)]
pub enum TargetCmyka {
    C = 0,
    M = 1,
    Y = 2,
    K = 3,
    A = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(usize)]
pub enum TargetGa {
    G = 0,
    A = 1,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum TargetSwizzle<'a> {
    Rgba(&'a [TargetRgba]),
    Ga(&'a [TargetGa]),
    Cmyk(&'a [TargetCmyka]),
}

impl<'a> TargetSwizzle<'a> {
    pub const fn color_model(&self) -> ColorModel {
        match self {
            Self::Rgba(_) => ColorModel::Rgb,
            Self::Ga(_) => ColorModel::G,
            Self::Cmyk(_) => ColorModel::Cmyk,
        }
    }

    pub fn into_iter_usize(&self) -> TargetSwizzleIter<'a> {
        match self {
            Self::Rgba(x) => TargetSwizzleIter::Rgba(x.iter()),
            Self::Cmyk(x) => TargetSwizzleIter::Cmyk(x.iter()),
            Self::Ga(x) => TargetSwizzleIter::Ga(x.iter()),
        }
    }
}

pub enum TargetSwizzleIter<'a> {
    Rgba(std::slice::Iter<'a, TargetRgba>),
    Ga(std::slice::Iter<'a, TargetGa>),
    Cmyk(std::slice::Iter<'a, TargetCmyka>),
}

impl<'a> Iterator for TargetSwizzleIter<'a> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Rgba(it) => it.next().map(|x| *x as usize),
            Self::Ga(it) => it.next().map(|x| *x as usize),
            Self::Cmyk(it) => it.next().map(|x| *x as usize),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Rgba(it) => it.size_hint(),
            Self::Cmyk(it) => it.size_hint(),
            Self::Ga(it) => it.size_hint(),
        }
    }
}

impl<'a> ExactSizeIterator for TargetSwizzleIter<'a> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        let target = &mut [0; 4];

        MemoryFormat::transform(
            MemoryFormat::R8g8b8,
            &[255, 85, 127],
            MemoryFormat::B8g8r8a8,
            target,
        );

        assert_eq!(*target, [127, 85, 255, 255]);
    }

    #[test]
    fn grayscale() {
        let target = &mut [0; 1];

        MemoryFormat::transform(
            MemoryFormat::R8g8b8,
            &[255, 0, 127],
            MemoryFormat::G8,
            target,
        );

        assert_eq!(*target, [127]);
    }

    #[test]
    fn u16() {
        let target = &mut [0; 6];

        MemoryFormat::transform(
            MemoryFormat::R8g8b8,
            &[255, 0, 127],
            MemoryFormat::R16g16b16,
            target,
        );

        assert_eq!(*target, [255, 255, 0, 0, 127, 127]);
    }

    #[test]
    fn cmyk_to_rgb_black1() {
        let target = &mut [0; 3];

        MemoryFormat::transform(
            MemoryFormat::C8m8y8k8,
            &[0, 0, 0, 5],
            MemoryFormat::R8g8b8,
            target,
        );

        assert_eq!(*target, [250, 250, 250]);
    }

    #[test]
    fn cmyk_to_rgb_black2() {
        let target = &mut [0; 3];

        MemoryFormat::transform(
            MemoryFormat::C8m8y8k8,
            &[5, 5, 5, 0],
            MemoryFormat::R8g8b8,
            target,
        );

        assert_eq!(*target, [250, 250, 250]);
    }

    #[test]
    fn cmyk_to_rgb_black3() {
        let target = &mut [0; 3];

        MemoryFormat::transform(
            MemoryFormat::C8m8y8k8,
            &[5, 5, 5, 5],
            MemoryFormat::R8g8b8,
            target,
        );

        assert_eq!(*target, [245, 245, 245]);
    }
}
