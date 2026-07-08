use std::sync::Arc;

use glycin_common::{ChannelType, MemoryFormat, MemoryFormatInfo};
use glycin_utils::{FungibleMemory, MemoryFormatSelection};

use crate::{ColorState, Error};

pub fn apply_transformation(
    icc_profile: &[u8],
    mut frame: glycin_utils::Frame<FungibleMemory>,
) -> (
    glycin_utils::Frame<FungibleMemory>,
    Result<ColorState, Error>,
) {
    match transform(icc_profile, &mut frame) {
        Err(err) => (frame, Err(err.into())),
        Ok(color_state) => (frame, Ok(color_state)),
    }
}

type TransformExectuor<T> = Arc<dyn moxcms::InPlaceTransformExecutor<T> + Send + Sync>;

enum Transform {
    U8(TransformExectuor<u8>),
    U16(TransformExectuor<u16>),
    F32(TransformExectuor<f32>),
}

impl Transform {
    fn transform(&self, in_out: &mut [u8]) -> Result<(), Error> {
        match self {
            Self::U8(executor) => executor.transform(in_out),
            Self::U16(executor) => {
                let in_out = bytemuck::try_cast_slice_mut(in_out)?;
                executor.transform(in_out)
            }
            Self::F32(executor) => {
                let in_out = bytemuck::try_cast_slice_mut(in_out)?;
                executor.transform(in_out)
            }
        }
        .map_err(Into::into)
    }
}

fn transformation(
    icc_profile: &[u8],
    memory_format: MemoryFormat,
) -> std::result::Result<Transform, moxcms::CmsError> {
    tracing::debug!("Converting to sRGB via ICC profile");

    let layout = pixel_layout(memory_format);
    let src_profile = moxcms::ColorProfile::new_from_slice(icc_profile)?;

    let target_profile = if memory_format.n_channels() > 2 {
        moxcms::ColorProfile::new_srgb()
    } else {
        moxcms::ColorProfile::new_gray_with_gamma(2.2)
    };

    match memory_format.channel_type() {
        ChannelType::U8 => Ok(Transform::U8(src_profile.create_in_place_transform_8bit(
            layout,
            &target_profile,
            moxcms::TransformOptions::default(),
        )?)),
        ChannelType::U16 => Ok(Transform::U16(
            src_profile.create_in_place_transform_16bit(
                layout,
                &target_profile,
                moxcms::TransformOptions::default(),
            )?,
        )),
        ChannelType::F16 => unreachable!(),
        ChannelType::F32 => Ok(Transform::F32(src_profile.create_in_place_transform_f32(
            layout,
            &target_profile,
            moxcms::TransformOptions::default(),
        )?)),
    }
}

fn transform(
    icc_profile: &[u8],
    frame: &mut glycin_utils::Frame<FungibleMemory>,
) -> std::result::Result<ColorState, Error> {
    let multiple = std::thread::available_parallelism().map_or(2, |x| x.get());
    tracing::trace!("Applying ICC profiles while using {multiple} threads");

    let supported_formats = MemoryFormatSelection::R8g8b8
        | MemoryFormatSelection::R16g16b16
        | MemoryFormatSelection::R32g32b32Float
        | MemoryFormatSelection::R8g8b8a8
        | MemoryFormatSelection::R16g16b16a16
        | MemoryFormatSelection::R32g32b32a32Float
        | MemoryFormatSelection::G8
        | MemoryFormatSelection::G16
        | MemoryFormatSelection::G8a8
        | MemoryFormatSelection::G16a16;

    let best_format = supported_formats.best_format_for(frame.memory_format);

    if let Some(best_format) = best_format
        && best_format != frame.memory_format
    {
        glycin_utils::editing::change_memory_format(frame, best_format)?;
    }

    let stride = frame.stride;
    let width = frame.width;
    let buf = &mut frame.texture;
    let memory_format = frame.memory_format;

    let transform = transformation(icc_profile, memory_format)?;

    let chunk_size = (buf.len() / stride as usize).div_ceil(multiple) * stride as usize;
    let row_length = width as usize * memory_format.n_bytes().usize();

    std::thread::scope(|s| {
        for chunk in buf.chunks_mut(chunk_size) {
            s.spawn(|| {
                for row in chunk.chunks_mut(stride as usize) {
                    transform.transform(&mut row[0..row_length])?;
                }
                Ok::<(), Error>(())
            });
        }
    });

    Ok(ColorState::Srgb)
}

const fn pixel_layout(format: MemoryFormat) -> moxcms::Layout {
    match format {
        MemoryFormat::R8g8b8 | MemoryFormat::R16g16b16 | MemoryFormat::R32g32b32Float => {
            moxcms::Layout::Rgb
        }
        MemoryFormat::R8g8b8a8 | MemoryFormat::R16g16b16a16 | MemoryFormat::R32g32b32a32Float => {
            moxcms::Layout::Rgba
        }
        MemoryFormat::G8 | MemoryFormat::G16 => moxcms::Layout::Gray,
        MemoryFormat::G8a8 | MemoryFormat::G16a16 => moxcms::Layout::GrayAlpha,
        _ => unreachable!(),
    }
}
