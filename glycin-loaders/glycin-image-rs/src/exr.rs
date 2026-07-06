use std::io::Cursor;

use exr::block::samples::{FromNativeSample, Sample};
use exr::image::read::image::ReadLayers;
use exr::image::read::layers::ReadChannels;
use exr::meta::attribute::SampleType;
use glycin_utils::*;
use gufo_common::math::ToU32;

struct Pixels<B: ByteData> {
    width: usize,
    height: usize,
    pixels: B,
    f16: bool,
    alpha: bool,
    channel_size: usize,
    num_channels: usize,
}

pub fn metadata<B: ByteData>(data: &[u8]) -> Result<ImageDetails<B>, ProcessError> {
    let metadata = exr::meta::MetaData::read_from_buffered(data, false).expected_error()?;
    let header = metadata.headers.first().expected_error()?;

    Ok(ImageDetails::new(
        header.layer_size.width().u32().expected_error()?,
        header.layer_size.height().u32().expected_error()?,
    ))
}

pub fn frame<B: ByteData>(data: &[u8]) -> Result<Frame<B>, ProcessError> {
    let image = exr::image::read::read()
        .no_deep_data()
        .largest_resolution_level()
        .rgba_channels(
            |resolution, (r, _, _, a)| {
                let sample_type = r.sample_type;

                let width = resolution.width();
                let height = resolution.height();

                let alpha = a.is_some();
                let num_channels = if alpha { 4 } else { 3 };
                let channel_size = sample_type.bytes_per_sample();

                Pixels {
                    width,
                    height,
                    pixels: B::new(
                        width as u64 * height as u64 * channel_size as u64 * num_channels as u64,
                    )
                    .unwrap(),
                    f16: matches!(sample_type, SampleType::F16),
                    alpha,
                    channel_size,
                    num_channels,
                }
            },
            |pixels, v, (r, g, b, a): (Sample, Sample, Sample, Sample)| {
                // We have to use coordinates, since the pixels are not necressarily returned in
                // the order they appear in the texture
                let index = v.x() * pixels.channel_size * pixels.num_channels
                    + v.y() * pixels.width * pixels.channel_size * pixels.num_channels;

                append(r, pixels.f16, &mut pixels.pixels, index);
                append(
                    g,
                    pixels.f16,
                    &mut pixels.pixels,
                    index + pixels.channel_size,
                );
                append(
                    b,
                    pixels.f16,
                    &mut pixels.pixels,
                    index + pixels.channel_size * 2,
                );
                if pixels.alpha {
                    append(
                        a,
                        pixels.f16,
                        &mut pixels.pixels,
                        index + pixels.channel_size * 3,
                    );
                }
            },
        )
        .first_valid_layer()
        .all_attributes()
        .from_buffered(Cursor::new(data))
        .expected_error()?;

    let pixels = image.layer_data.channel_data.pixels;

    let width = pixels.width as u32;
    let height = pixels.height as u32;

    let memory_format = match (pixels.alpha, pixels.f16) {
        (true, true) => MemoryFormat::R16g16b16a16Float,
        (false, true) => MemoryFormat::R16g16b16Float,
        (true, false) => MemoryFormat::R32g32b32a32Float,
        (false, false) => MemoryFormat::R32g32b32Float,
    };

    let mut frame = Frame::new(width, height, memory_format, pixels.pixels)?;

    if pixels.alpha {
        frame.details.info_alpha_channel = Some(pixels.alpha)
    }

    frame.details.info_bit_depth = Some(if pixels.f16 { 16 } else { 32 });

    Ok(frame)
}

pub fn append<B: ByteData>(from: Sample, to_f16: bool, data: &mut B, index: usize) {
    match from {
        Sample::F16(x) => match to_f16 {
            true => {
                data[index..index + 2].copy_from_slice(&x.to_ne_bytes());
            }
            false => data[index..index + 4].copy_from_slice(&x.to_f32().to_ne_bytes()),
        },
        Sample::F32(x) => match to_f16 {
            true => data[index..index + 2]
                .copy_from_slice(&exr::prelude::f16::from_f32(x).to_ne_bytes()),
            false => data[index..index + 4].copy_from_slice(&x.to_ne_bytes()),
        },
        Sample::U32(x) => match to_f16 {
            true => data[index..index + 2].copy_from_slice(
                &(exr::prelude::f16::from_u32(x) / exr::prelude::f16::from_u32(u32::MAX))
                    .to_ne_bytes(),
            ),
            false => {
                data[index..index + 4].copy_from_slice(&(x as f32 / u32::MAX as f32).to_ne_bytes())
            }
        },
    }
}
