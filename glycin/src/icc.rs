use glycin_common::{MemoryFormat, MemoryFormatInfo};

use crate::{ColorState, Error};

pub fn apply_transformation(
    icc_profile: &[u8],
    memory_format: MemoryFormat,
    mmap: &mut [u8],
) -> Result<ColorState, Error> {
    transform(icc_profile, memory_format, mmap).map_err(Into::into)
}

fn transform(
    icc_profile: &[u8],
    memory_format: MemoryFormat,
    buf: &mut [u8],
) -> std::result::Result<ColorState, lcms2::Error> {
    let icc_pixel_format = lcms_pixel_format(memory_format);
    let src_profile = lcms2::Profile::new_icc(icc_profile)?;

    let target_profile;
    let target_color_state;

    if memory_format.n_channels() > 2 {
        target_profile = lcms2::Profile::new_srgb();
        target_color_state = ColorState::Srgb;
    } else {
        target_profile =
            lcms2::Profile::new_gray(lcms2_sys::ffi::CIExyY::d50(), &lcms2::ToneCurve::new(2.2))?;
        target_color_state = ColorState::Srgb;
    };

    let transform = lcms2::Transform::new(
        &src_profile,
        icc_pixel_format,
        &target_profile,
        icc_pixel_format,
        lcms2::Intent::Perceptual,
    )?;

    transform.transform_in_place(buf);

    Ok(target_color_state)
}

const fn lcms_pixel_format(format: MemoryFormat) -> lcms2::PixelFormat {
    match format {
        MemoryFormat::B8g8r8a8Premultiplied => premul(lcms2::PixelFormat::BGRA_8),
        MemoryFormat::A8r8g8b8Premultiplied => premul(lcms2::PixelFormat::ARGB_8),
        MemoryFormat::R8g8b8a8Premultiplied => premul(lcms2::PixelFormat::RGBA_8),
        MemoryFormat::B8g8r8a8 => lcms2::PixelFormat::BGRA_8,
        MemoryFormat::A8r8g8b8 => lcms2::PixelFormat::ARGB_8,
        MemoryFormat::R8g8b8a8 => lcms2::PixelFormat::RGBA_8,
        MemoryFormat::A8b8g8r8 => lcms2::PixelFormat::ABGR_8,
        MemoryFormat::R8g8b8 => lcms2::PixelFormat::RGB_8,
        MemoryFormat::B8g8r8 => lcms2::PixelFormat::BGR_8,
        MemoryFormat::R16g16b16 => lcms2::PixelFormat::RGB_16,
        MemoryFormat::R16g16b16a16Premultiplied => premul(lcms2::PixelFormat::RGBA_16),
        MemoryFormat::R16g16b16a16 => lcms2::PixelFormat::RGBA_16,
        MemoryFormat::R16g16b16Float => lcms2::PixelFormat::RGB_HALF_FLT,
        MemoryFormat::R16g16b16a16Float => lcms2::PixelFormat::RGBA_HALF_FLT,
        MemoryFormat::R32g32b32Float => lcms2::PixelFormat::RGB_FLT,
        MemoryFormat::R32g32b32a32FloatPremultiplied => premul(lcms2::PixelFormat::RGBA_FLT),
        MemoryFormat::R32g32b32a32Float => lcms2::PixelFormat::RGBA_FLT,
        MemoryFormat::G8a8Premultiplied => premul(lcms2::PixelFormat::GRAYA_8),
        MemoryFormat::G8a8 => lcms2::PixelFormat::GRAYA_8,
        MemoryFormat::G8 => lcms2::PixelFormat::GRAY_8,
        MemoryFormat::G16a16Premultiplied => premul(lcms2::PixelFormat::GRAYA_16),
        MemoryFormat::G16a16 => lcms2::PixelFormat::GRAYA_16,
        MemoryFormat::G16 => lcms2::PixelFormat::GRAY_16,
    }
}

const fn premul(format: lcms2::PixelFormat) -> lcms2::PixelFormat {
    let mut bytes = format.0;
    bytes |= 0b1 << 23;
    lcms2::PixelFormat(bytes)
}

#[test]
fn premul_test() {
    assert!(!lcms2::PixelFormat::RGBA_8.premultiplied());
    assert!(premul(lcms2::PixelFormat::RGBA_8).premultiplied());
}
