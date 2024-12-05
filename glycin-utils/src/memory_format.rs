use std::io::{Read, Write};

use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;
use zerocopy::{FromBytes, IntoBytes};

gufo_common::maybe_convertible_enum!(
    #[repr(i32)]
    #[derive(Deserialize, Serialize, Type, Debug, Clone, Copy)]
    #[cfg_attr(feature = "gobject", derive(glib::Enum))]
    #[cfg_attr(feature = "gobject", enum_type(name = "GlyMemoryFormat"))]
    #[zvariant(signature = "u")]
    /// Describes the formats the image data can have.
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
    }
);

impl MemoryFormat {
    pub const fn n_bytes(self) -> MemoryFormatBytes {
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
        }
    }

    pub const fn n_channels(self) -> u8 {
        match self {
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
            | MemoryFormat::R32g32b32a32Float => 4,
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
            | MemoryFormat::G8 => ChannelType::U8,
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
            | MemoryFormat::G16a16 => true,
            MemoryFormat::R8g8b8
            | MemoryFormat::B8g8r8
            | MemoryFormat::R16g16b16
            | MemoryFormat::R16g16b16Float
            | MemoryFormat::R32g32b32Float
            | MemoryFormat::G8
            | MemoryFormat::G16 => false,
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
            | MemoryFormat::G16 => false,
        }
    }

    const fn source_definition(self) -> [Source; 4] {
        match self {
            MemoryFormat::B8g8r8a8Premultiplied | MemoryFormat::B8g8r8a8 => {
                [Source::C2, Source::C1, Source::C0, Source::C3]
            }
            MemoryFormat::A8r8g8b8Premultiplied | MemoryFormat::A8r8g8b8 => {
                [Source::C1, Source::C2, Source::C3, Source::C0]
            }
            MemoryFormat::R8g8b8a8Premultiplied
            | MemoryFormat::R8g8b8a8
            | MemoryFormat::R16g16b16a16Premultiplied
            | MemoryFormat::R16g16b16a16
            | MemoryFormat::R16g16b16a16Float
            | MemoryFormat::R32g32b32a32FloatPremultiplied
            | MemoryFormat::R32g32b32a32Float => [Source::C0, Source::C1, Source::C2, Source::C3],
            MemoryFormat::A8b8g8r8 => [Source::C1, Source::C2, Source::C3, Source::C0],
            MemoryFormat::R8g8b8
            | MemoryFormat::R16g16b16
            | MemoryFormat::R16g16b16Float
            | MemoryFormat::R32g32b32Float => [Source::C0, Source::C1, Source::C2, Source::Opaque],
            MemoryFormat::B8g8r8 => [Source::C2, Source::C1, Source::C0, Source::Opaque],
            MemoryFormat::G8a8Premultiplied
            | MemoryFormat::G8a8
            | MemoryFormat::G16a16Premultiplied
            | MemoryFormat::G16a16 => [Source::C0, Source::C0, Source::C0, Source::C1],
            MemoryFormat::G8 | MemoryFormat::G16 => {
                [Source::C0, Source::C0, Source::C0, Source::Opaque]
            }
        }
    }

    const fn target_definition(self) -> &'static [Target] {
        match self {
            MemoryFormat::B8g8r8a8Premultiplied | MemoryFormat::B8g8r8a8 => {
                &[Target::B, Target::G, Target::R, Target::A]
            }
            MemoryFormat::A8r8g8b8Premultiplied | MemoryFormat::A8r8g8b8 => {
                &[Target::A, Target::R, Target::G, Target::B]
            }
            MemoryFormat::R8g8b8a8Premultiplied
            | MemoryFormat::R8g8b8a8
            | MemoryFormat::R16g16b16a16Premultiplied
            | MemoryFormat::R16g16b16a16
            | MemoryFormat::R16g16b16a16Float
            | MemoryFormat::R32g32b32a32FloatPremultiplied
            | MemoryFormat::R32g32b32a32Float => &[Target::R, Target::G, Target::B, Target::A],
            MemoryFormat::A8b8g8r8 => &[Target::A, Target::B, Target::G, Target::R],
            MemoryFormat::R8g8b8
            | MemoryFormat::R16g16b16
            | MemoryFormat::R16g16b16Float
            | MemoryFormat::R32g32b32Float => &[Target::R, Target::G, Target::B],
            MemoryFormat::B8g8r8 => &[Target::B, Target::G, Target::R],
            MemoryFormat::G8a8Premultiplied
            | MemoryFormat::G8a8
            | MemoryFormat::G16a16Premultiplied
            | MemoryFormat::G16a16 => &[Target::RgbAvg, Target::A],
            MemoryFormat::G8 | MemoryFormat::G16 => &[Target::RgbAvg],
        }
    }

    pub fn transform(
        src_format: Self,
        mut src: &[u8],
        target_format: Self,
        mut target: &mut [u8],
    ) -> Option<()> {
        let channels_f32 = match src_format.channel_type() {
            ChannelType::U8 => Self::to_f32::<u8>(FromBytes::ref_from_bytes(src).ok()?, src_format),
            ChannelType::U16 => {
                Self::to_f32::<u16>(FromBytes::ref_from_bytes(src).ok()?, src_format)
            }
            ChannelType::F16 => {
                let bytes = &mut [0; 2];
                let mut f16_data = Vec::new();
                while let Ok(()) = src.read_exact(bytes) {
                    f16_data.push(half::f16::from_ne_bytes(*bytes));
                }
                Self::to_f32::<half::f16>(&f16_data, src_format)
            }
            ChannelType::F32 => {
                Self::to_f32::<f32>(FromBytes::ref_from_bytes(src).ok()?, src_format)
            }
        }?;

        match target_format.channel_type() {
            ChannelType::U8 => Self::from_f32::<u8>(channels_f32, target_format)?
                .get(0..target_format.n_channels().into())?
                .write_to(target)
                .ok()?,
            ChannelType::U16 => Self::from_f32::<u16>(channels_f32, target_format)?
                .get(..target_format.n_channels().into())?
                .write_to(target)
                .ok()?,
            ChannelType::F16 => {
                let f16_data = Self::from_f32::<half::f16>(channels_f32, target_format)?
                    .get(..target_format.n_channels().into())?
                    .iter()
                    .flat_map(|x| x.to_ne_bytes())
                    .collect::<Vec<u8>>();
                target.write_all(&f16_data).ok()?;
            }
            ChannelType::F32 => Self::from_f32::<f32>(channels_f32, target_format)?
                .get(..target_format.n_channels().into())?
                .write_to(target)
                .ok()?,
        };

        Some(())
    }

    #[allow(clippy::get_first)]
    fn to_f32<T: ChannelValue>(source_channels: &[T], source_format: Self) -> Option<[f32; 4]> {
        let mut channels_f32 = [0.0_f32; 4];

        let source_definition = source_format.source_definition();

        for (n, channel) in channels_f32.iter_mut().enumerate() {
            *channel = match source_definition.get(n)? {
                Source::C0 => (*source_channels.get(0)?).to_f32_normed(),
                Source::C1 => (*source_channels.get(1)?).to_f32_normed(),
                Source::C2 => (*source_channels.get(2)?).to_f32_normed(),
                Source::C3 => (*source_channels.get(3)?).to_f32_normed(),
                Source::Opaque => 1.,
            };
        }

        if source_format.is_premultiplied() {
            channels_f32[0] /= channels_f32[3];
            channels_f32[1] /= channels_f32[3];
            channels_f32[2] /= channels_f32[3];
        }

        Some(channels_f32)
    }

    fn from_f32<T: ChannelValue>(channels_f32: [f32; 4], target_format: Self) -> Option<[T; 4]> {
        let mut out = [T::default(); 4];

        let premultiply = if target_format.is_premultiplied() {
            channels_f32[3]
        } else {
            1.
        };

        for (n, def) in target_format.target_definition().iter().enumerate() {
            *out.get_mut(n)? = match def {
                Target::R => T::from_f32_normed(channels_f32[0] * premultiply),
                Target::G => T::from_f32_normed(channels_f32[1] * premultiply),
                Target::B => T::from_f32_normed(channels_f32[2] * premultiply),
                Target::A => T::from_f32_normed(channels_f32[3]),
                Target::RgbAvg => {
                    T::from_f32_normed((channels_f32[0] + channels_f32[1] + channels_f32[2]) / 3.)
                }
            };
        }

        Some(out)
    }
}

trait ChannelValue: Default + Copy {
    fn from_f32_normed(value: f32) -> Self;
    fn to_f32_normed(self) -> f32;
}

impl ChannelValue for u8 {
    fn from_f32_normed(value: f32) -> Self {
        #![allow(clippy::cast_possible_truncation)]
        (value * Self::MAX as f32).round() as Self
    }

    fn to_f32_normed(self) -> f32 {
        (self as f32) / Self::MAX as f32
    }
}

impl ChannelValue for u16 {
    fn from_f32_normed(value: f32) -> Self {
        #![allow(clippy::cast_possible_truncation)]
        (value * Self::MAX as f32).round() as Self
    }

    fn to_f32_normed(self) -> f32 {
        (self as f32) / Self::MAX as f32
    }
}

impl ChannelValue for half::f16 {
    fn from_f32_normed(value: f32) -> Self {
        Self::from_f32(value)
    }

    fn to_f32_normed(self) -> f32 {
        self.into()
    }
}

impl ChannelValue for f32 {
    fn from_f32_normed(value: f32) -> Self {
        value
    }

    fn to_f32_normed(self) -> f32 {
        self
    }
}

enum Target {
    R,
    G,
    B,
    A,
    RgbAvg,
}

enum Source {
    C0,
    C1,
    C2,
    C3,
    Opaque,
}

pub enum ChannelType {
    U8,
    U16,
    F16,
    F32,
}

pub enum MemoryFormatBytes {
    B1 = 1,
    B2 = 2,
    B3 = 3,
    B4 = 4,
    B6 = 6,
    B8 = 8,
    B12 = 12,
    B16 = 16,
}

// TODO: Convert to From trait impls
impl MemoryFormatBytes {
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
        )
        .unwrap();

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
        )
        .unwrap();

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
        )
        .unwrap();

        assert_eq!(*target, [255, 255, 0, 0, 127, 127]);
    }
}
