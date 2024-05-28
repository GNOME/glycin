use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

#[derive(Deserialize, Serialize, Type, Debug, Clone, Copy)]
pub enum MemoryFormat {
    B8g8r8a8Premultiplied,
    A8r8g8b8Premultiplied,
    R8g8b8a8Premultiplied,
    B8g8r8a8,
    A8r8g8b8,
    R8g8b8a8,
    A8b8g8r8,
    R8g8b8,
    B8g8r8,
    R16g16b16,
    R16g16b16a16Premultiplied,
    R16g16b16a16,
    R16g16b16Float,
    R16g16b16a16Float,
    R32g32b32Float,
    R32g32b32a32FloatPremultiplied,
    R32g32b32a32Float,
    G8a8Premultiplied,
    G8a8,
    G8,
    G16a16Premultiplied,
    G16a16,
    G16,
}

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
            MemoryFormat::B8g8r8a8Premultiplied => 4,
            MemoryFormat::A8r8g8b8Premultiplied => 4,
            MemoryFormat::R8g8b8a8Premultiplied => 4,
            MemoryFormat::B8g8r8a8 => 4,
            MemoryFormat::A8r8g8b8 => 4,
            MemoryFormat::R8g8b8a8 => 4,
            MemoryFormat::A8b8g8r8 => 4,
            MemoryFormat::R8g8b8 => 3,
            MemoryFormat::B8g8r8 => 3,
            MemoryFormat::R16g16b16 => 3,
            MemoryFormat::R16g16b16a16Premultiplied => 4,
            MemoryFormat::R16g16b16a16 => 4,
            MemoryFormat::R16g16b16Float => 3,
            MemoryFormat::R16g16b16a16Float => 4,
            MemoryFormat::R32g32b32Float => 3,
            MemoryFormat::R32g32b32a32FloatPremultiplied => 4,
            MemoryFormat::R32g32b32a32Float => 4,
            MemoryFormat::G8a8Premultiplied => 2,
            MemoryFormat::G8a8 => 2,
            MemoryFormat::G8 => 1,
            MemoryFormat::G16a16Premultiplied => 2,
            MemoryFormat::G16a16 => 2,
            MemoryFormat::G16 => 1,
        }
    }

    pub const fn has_alpha(self) -> bool {
        match self {
            MemoryFormat::B8g8r8a8Premultiplied => true,
            MemoryFormat::A8r8g8b8Premultiplied => true,
            MemoryFormat::R8g8b8a8Premultiplied => true,
            MemoryFormat::B8g8r8a8 => true,
            MemoryFormat::A8r8g8b8 => true,
            MemoryFormat::R8g8b8a8 => true,
            MemoryFormat::A8b8g8r8 => true,
            MemoryFormat::R8g8b8 => false,
            MemoryFormat::B8g8r8 => false,
            MemoryFormat::R16g16b16 => false,
            MemoryFormat::R16g16b16a16Premultiplied => true,
            MemoryFormat::R16g16b16a16 => true,
            MemoryFormat::R16g16b16Float => false,
            MemoryFormat::R16g16b16a16Float => true,
            MemoryFormat::R32g32b32Float => false,
            MemoryFormat::R32g32b32a32FloatPremultiplied => true,
            MemoryFormat::R32g32b32a32Float => true,
            MemoryFormat::G8a8Premultiplied => true,
            MemoryFormat::G8a8 => true,
            MemoryFormat::G8 => false,
            MemoryFormat::G16a16Premultiplied => true,
            MemoryFormat::G16a16 => true,
            MemoryFormat::G16 => false,
        }
    }

    pub const fn is_premultiplied(self) -> bool {
        match self {
            MemoryFormat::B8g8r8a8Premultiplied => true,
            MemoryFormat::A8r8g8b8Premultiplied => true,
            MemoryFormat::R8g8b8a8Premultiplied => true,
            MemoryFormat::B8g8r8a8 => false,
            MemoryFormat::A8r8g8b8 => false,
            MemoryFormat::R8g8b8a8 => false,
            MemoryFormat::A8b8g8r8 => false,
            MemoryFormat::R8g8b8 => false,
            MemoryFormat::B8g8r8 => false,
            MemoryFormat::R16g16b16 => false,
            MemoryFormat::R16g16b16a16Premultiplied => true,
            MemoryFormat::R16g16b16a16 => false,
            MemoryFormat::R16g16b16Float => false,
            MemoryFormat::R16g16b16a16Float => false,
            MemoryFormat::R32g32b32Float => false,
            MemoryFormat::R32g32b32a32FloatPremultiplied => true,
            MemoryFormat::R32g32b32a32Float => false,
            MemoryFormat::G8a8Premultiplied => true,
            MemoryFormat::G8a8 => false,
            MemoryFormat::G8 => false,
            MemoryFormat::G16a16Premultiplied => true,
            MemoryFormat::G16a16 => false,
            MemoryFormat::G16 => false,
        }
    }
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
