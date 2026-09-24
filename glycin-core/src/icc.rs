use std::sync::Arc;

use glycin_common::{ChannelType, ColorModel, MemoryFormat, MemoryFormatInfo};
use glycin_utils::{ByteData, FungibleMemory, MemoryFormatSelection};

use crate::{ColorState, Error};

pub fn apply_transformation(
    icc_profile: &[u8],
    mut frame: glycin_utils::Frame<FungibleMemory>,
    final_memory_format: MemoryFormat,
) -> (
    glycin_utils::Frame<FungibleMemory>,
    Result<ColorState, Error>,
) {
    match transform(icc_profile, &mut frame, final_memory_format) {
        Err(err) => (frame, Err(err)),
        Ok(color_state) => (frame, Ok(color_state)),
    }
}

type InPlaceTransformExectuor<T> = Arc<dyn moxcms::InPlaceTransformExecutor<T> + Send + Sync>;
type TransformExectuor<T> = Arc<dyn moxcms::TransformExecutor<T> + Send + Sync>;

#[derive(Clone)]
enum Transform {
    NewPlace {
        transform: TransformNewPlace,
        final_memory_format: MemoryFormat,
    },
    InPlace(TransformInPlace),
}

#[derive(Clone)]
enum TransformNewPlace {
    U8(TransformExectuor<u8>),
}

#[derive(Clone)]
enum TransformInPlace {
    U8(InPlaceTransformExectuor<u8>),
    U16(InPlaceTransformExectuor<u16>),
    F32(InPlaceTransformExectuor<f32>),
}

impl TransformNewPlace {
    fn transform(&self, src: &mut [u8], dst: &mut [u8]) -> Result<(), Error> {
        match self {
            Self::U8(executor) => executor.transform(src, dst),
        }
        .map_err(Into::into)
    }
}

impl TransformInPlace {
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
    memory_format: &MemoryFormat,
    final_memory_format: MemoryFormat,
) -> std::result::Result<Transform, moxcms::CmsError> {
    tracing::debug!("Converting to sRGB via ICC profile");

    let layout = pixel_layout(memory_format);
    let src_profile = moxcms::ColorProfile::new_from_slice(icc_profile)?;

    let target_profile = match memory_format.color_model() {
        ColorModel::Rgb => moxcms::ColorProfile::new_srgb(),
        ColorModel::G => moxcms::ColorProfile::new_gray_with_gamma(2.2),
        ColorModel::Cmyk if final_memory_format.color_model() == ColorModel::Rgb => {
            moxcms::ColorProfile::new_srgb()
        }
        ColorModel::Cmyk if final_memory_format.color_model() == ColorModel::Cmyk => {
            // Needs CMYK format
            moxcms::ColorProfile::new_srgb()
        }
        model => {
            tracing::error!("Color model not explitly supported: {model:?}");
            moxcms::ColorProfile::new_srgb()
        }
    };

    match memory_format.channel_type() {
        ChannelType::U8
            if *memory_format == MemoryFormat::C8m8y8k8
                && final_memory_format.color_model() == ColorModel::Rgb =>
        {
            Ok(Transform::NewPlace {
                transform: TransformNewPlace::U8(src_profile.create_transform_8bit(
                    layout,
                    &moxcms::ColorProfile::new_srgb(),
                    moxcms::Layout::Rgb,
                    moxcms::TransformOptions::default(),
                )?),
                final_memory_format: MemoryFormat::R8g8b8,
            })
        }
        ChannelType::U8 => Ok(Transform::InPlace(TransformInPlace::U8(
            src_profile.create_in_place_transform_8bit(
                layout,
                &target_profile,
                moxcms::TransformOptions::default(),
            )?,
        ))),
        ChannelType::U16 => Ok(Transform::InPlace(TransformInPlace::U16(
            src_profile.create_in_place_transform_16bit(
                layout,
                &target_profile,
                moxcms::TransformOptions::default(),
            )?,
        ))),
        ChannelType::F16 => {
            // Previously transformed to different format
            unreachable!()
        }
        ChannelType::F32 => Ok(Transform::InPlace(TransformInPlace::F32(
            src_profile.create_in_place_transform_f32(
                layout,
                &target_profile,
                moxcms::TransformOptions::default(),
            )?,
        ))),
        _ => unreachable!(),
    }
}

fn transform(
    icc_profile: &[u8],
    frame: &mut glycin_utils::Frame<FungibleMemory>,
    final_memory_format: MemoryFormat,
) -> std::result::Result<ColorState, Error> {
    // moxcms does not support different RGB orders or f16.
    // Hence we transform to a format that moxcms supports.
    let supported_formats = MemoryFormatSelection::R8g8b8
        | MemoryFormatSelection::R16g16b16
        | MemoryFormatSelection::R32g32b32Float
        | MemoryFormatSelection::R8g8b8a8
        | MemoryFormatSelection::R16g16b16a16
        | MemoryFormatSelection::R32g32b32a32Float
        | MemoryFormatSelection::G8
        | MemoryFormatSelection::G16
        | MemoryFormatSelection::G8a8
        | MemoryFormatSelection::G16a16
        | MemoryFormatSelection::C8m8y8k8
        | MemoryFormatSelection::C8m8y8k8a8;

    let best_format = supported_formats.best_format_for(frame.memory_format);
    if let Some(best_format) = best_format
        && best_format != frame.memory_format
    {
        glycin_utils::editing::change_memory_format(frame, best_format)?;
    }

    let memory_format = &mut frame.memory_format;
    let transform = transformation(icc_profile, memory_format, final_memory_format)?;

    let multiple = std::thread::available_parallelism().map_or(2, |x| x.get());
    tracing::trace!("Applying ICC profiles while using {multiple} threads");

    let stride = frame.stride;
    let width = frame.width;
    let height = frame.width;
    let buf = &mut frame.texture;

    let rows_per_thread = (buf.len() / stride as usize).div_ceil(multiple);
    let chunk_size = rows_per_thread * stride as usize;
    let row_length = width as usize * frame.memory_format.n_bytes().usize();

    match transform {
        Transform::InPlace(transform) => std::thread::scope(|s| {
            tracing::debug!("Doing in-place transformation");

            for chunk in buf.chunks_mut(chunk_size) {
                let transform = transform.clone();
                s.spawn(move || {
                    for row in chunk.chunks_mut(stride as usize) {
                        transform.transform(&mut row[0..row_length])?
                    }
                    Ok::<(), Error>(())
                });
            }
        }),
        Transform::NewPlace {
            transform,
            final_memory_format,
        } => {
            tracing::debug!("Transforming to new place");

            let dst_stride = width * final_memory_format.n_bytes().u32();
            let dst_size = dst_stride as usize * height as usize;

            let mut dst_buf = FungibleMemory::new(dst_size as u64)?;

            let dst_chunk_size = rows_per_thread * dst_stride as usize;

            std::thread::scope(|s| {
                let src_chunks = buf.chunks_mut(chunk_size);
                let dst_chunks = dst_buf.chunks_mut(dst_chunk_size);

                for (chunk, dst_chunk) in src_chunks.zip(dst_chunks) {
                    let transform = transform.clone();
                    s.spawn(move || {
                        let src_iter = chunk.chunks_mut(stride as usize);
                        let dst_iter = dst_chunk.chunks_mut(dst_stride as usize);

                        for (src, dst) in src_iter.zip(dst_iter) {
                            transform.transform(&mut src[0..row_length], dst)?;
                        }
                        Ok::<(), Error>(())
                    });
                }
                Ok::<(), Error>(())
            })?;

            // TODO: Check for errors from threads?
            std::mem::swap(&mut frame.texture, &mut dst_buf);
            frame.memory_format = final_memory_format;
            frame.stride = dst_stride;
        }
    }

    Ok(ColorState::Srgb)
}

const fn pixel_layout(format: &MemoryFormat) -> moxcms::Layout {
    match format {
        MemoryFormat::R8g8b8 | MemoryFormat::R16g16b16 | MemoryFormat::R32g32b32Float => {
            moxcms::Layout::Rgb
        }
        MemoryFormat::R8g8b8a8 | MemoryFormat::R16g16b16a16 | MemoryFormat::R32g32b32a32Float => {
            moxcms::Layout::Rgba
        }
        MemoryFormat::G8 | MemoryFormat::G16 => moxcms::Layout::Gray,
        MemoryFormat::G8a8 | MemoryFormat::G16a16 => moxcms::Layout::GrayAlpha,
        MemoryFormat::C8m8y8k8 => {
            // Per documentation CMYK uses RGBA layout
            moxcms::Layout::Rgba
        }
        MemoryFormat::C8m8y8k8a8 => moxcms::Layout::Cmyka,
        _ => unreachable!(),
    }
}
