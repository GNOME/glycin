use std::sync::Arc;

use glycin_common::{ChannelType, MemoryFormatInfo};
use gufo_common::math::Checked;
use rayon::prelude::*;

use crate::{Frame, FungibleMemory, MemoryFormat, editing};
pub fn change_memory_format(
    frame: &mut Frame<FungibleMemory>,
    target_format: MemoryFormat,
) -> Result<(), editing::Error> {
    let src_format = frame.memory_format;

    if src_format == target_format {
        log::debug!("Same image format {src_format:?}, no need for transformation");
        return Ok(());
    }

    log::debug!("Starting to transform image format from {src_format:?} to {target_format:?}");
    let start_instant = std::time::Instant::now();

    let src_format = frame.memory_format;
    let src_data = &mut frame.texture;
    let src_pixel_n_bytes = src_format.n_bytes().usize();
    let src_stride = frame.stride as usize;
    let src_width = frame.width as usize;
    let src_correct_stride = src_width * src_pixel_n_bytes;
    let src_n_channels = frame.memory_format.n_channels() as usize;
    let src_channel_bytes = frame.memory_format.channel_type().size() as usize;

    let target_pixel_n_bytes = target_format.n_bytes().usize();
    let new_stride = (Checked::new(frame.width) * target_format.n_bytes().u32()).check()?;
    let new_total_size: usize =
        (Checked::new(frame.height as usize) * new_stride as usize).check()?;

    let mut new_data_x = None;

    rayon::ThreadPoolBuilder::new()
        .thread_name(|i| format!("gly-rayon-{i}"))
        .build()
        .map_err(Arc::new)?
        .install(|| {
            if src_format.color_model() == target_format.color_model()
                && src_format.channel_type() == target_format.channel_type()
                && src_format.is_premultiplied() == target_format.is_premultiplied()
                && src_format.has_alpha() == target_format.has_alpha()
            {
                // Fast path for pure shuffling of indices

                let mut source_target_index_map = [0; 4];
                for (n, target) in target_format
                    .target_definition()
                    .into_iter_usize()
                    .enumerate()
                {
                    source_target_index_map[n] =
                        *src_format.swizzle().into_iter_usize().collect::<Vec<_>>()[target]
                            as usize;
                }

                src_data
                    .chunks_exact_mut(src_stride)
                    .par_bridge()
                    .for_each(|new_row| {
                        let mut pixel_buffer = vec![0; src_pixel_n_bytes];

                        for pixel in
                            new_row[..src_correct_stride].chunks_exact_mut(src_pixel_n_bytes)
                        {
                            pixel_buffer.clone_from_slice(&pixel);
                            for (target_channel, src_channel) in source_target_index_map
                                .iter()
                                .take(src_n_channels)
                                .enumerate()
                            {
                                for channel_byte in 0..src_channel_bytes {
                                    pixel[target_channel + channel_byte] =
                                        pixel_buffer[src_channel + channel_byte];
                                }
                            }
                        }
                    });
            } else if src_format == MemoryFormat::B8g8r8a8Premultiplied
                && target_format == MemoryFormat::R8g8b8a8
            {
                // Specialize for librsvg format

                src_data
                    .chunks_exact_mut(src_stride)
                    .par_bridge()
                    .for_each(|new_row| {
                        for pixel in new_row[..src_correct_stride].chunks_exact_mut(4) {
                            // Swap red and blue
                            pixel.swap(0, 2);

                            if pixel[3] > 0 {
                                let unpremultiply = 255. / pixel[3] as f32;
                                pixel[0] = (pixel[0] as f32 * unpremultiply) as u8;
                                pixel[1] = (pixel[1] as f32 * unpremultiply) as u8;
                                pixel[2] = (pixel[2] as f32 * unpremultiply) as u8;
                            }
                        }
                    });
            } else if src_format.channel_type() == ChannelType::U16
                && target_format.channel_type() == ChannelType::U8
                && src_format.color_model() == target_format.color_model()
                && src_format.is_premultiplied() == target_format.is_premultiplied()
                && (src_format.has_alpha() || !target_format.has_alpha())
            {
                // Fast path for u16 to u8 conversion

                let mut new_data = vec![0; new_total_size];
                let new_data_rows = new_data.chunks_exact_mut(new_stride as usize);

                let mut source_target_index_map = [0; 5];
                for (n, target) in target_format
                    .target_definition()
                    .into_iter_usize()
                    .enumerate()
                {
                    source_target_index_map[n] =
                        *src_format.swizzle().into_iter_usize().collect::<Vec<_>>()[target]
                            as usize;
                }

                let target_n_channels = target_format.n_channels();
                let source_channel_size = src_format.channel_type().size() as usize;

                new_data_rows
                    .enumerate()
                    .par_bridge()
                    .for_each(|(y, new_row)| {
                        for x in 0..frame.width as usize {
                            let x_ = x * src_pixel_n_bytes;

                            // src bytes for pixel
                            let i0 = x_ + y * frame.stride as usize;

                            // target bytes for pixel
                            let k0 = x * target_pixel_n_bytes;

                            for i in 0..target_n_channels as usize {
                                new_row[k0 + i] = (u16::from_ne_bytes([
                                    src_data[i0 + source_target_index_map[i] * source_channel_size],
                                    src_data
                                        [i0 + source_target_index_map[i] * source_channel_size + 1],
                                ])
                                .saturating_add(128)
                                    >> 8) as u8;
                            }
                        }
                    });

                new_data_x = Some(new_data);
            } else {
                // Slow generic path

                let mut new_data = vec![0; new_total_size];
                let new_data_rows = new_data.chunks_exact_mut(new_stride as usize);

                new_data_rows
                    .enumerate()
                    .par_bridge()
                    .for_each(|(y, new_row)| {
                        for x in 0..frame.width as usize {
                            let x_ = x * src_pixel_n_bytes;

                            // src bytes for pixel
                            let i0 = x_ + y * frame.stride as usize;
                            let i1 = i0 + src_pixel_n_bytes;

                            // target bytes for pixel
                            let k0 = x * target_pixel_n_bytes;
                            let k1 = k0 + target_pixel_n_bytes;

                            MemoryFormat::transform(
                                src_format,
                                &src_data[i0..i1],
                                target_format,
                                &mut new_row[k0..k1],
                            );
                        }
                    });

                new_data_x = Some(new_data);
            }
        });

    if let Some(new_data) = new_data_x {
        frame.stride = new_stride;
        frame.texture = FungibleMemory::from_vec(new_data);
    }
    frame.memory_format = target_format;

    log::debug!(
        "Transformation completed after {:?}",
        start_instant.elapsed()
    );

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn all_to_all() {
        for from in crate::MemoryFormat::ALL {
            for to in crate::MemoryFormat::ALL {
                let src = vec![0; from.n_bytes().usize()];
                let texture = FungibleMemory::from_vec(src);
                let mut frame = Frame::new(1, 1, *from, texture).unwrap();
                change_memory_format(&mut frame, *to).unwrap();
            }
        }
    }

    #[test]
    fn gray_to_all_to_all_to_gray() {
        for from in crate::MemoryFormat::ALL {
            for to in crate::MemoryFormat::ALL {
                let src = vec![255 / 3];
                let texture = FungibleMemory::from_vec(src);
                let mut frame = Frame::new(1, 1, MemoryFormat::G8, texture).unwrap();
                change_memory_format(&mut frame, *from).unwrap();
                change_memory_format(&mut frame, *to).unwrap();
                change_memory_format(&mut frame, MemoryFormat::G8).unwrap();
                assert!(
                    [255 / 3, 255 / 3 - 1].contains(&frame.texture[0]),
                    "Not matching after going through {from:?} to {to:?}: {} != {}",
                    frame.texture[0],
                    255 / 3
                );
            }
        }
    }

    #[test]
    fn g8_to_g16a16pre() {
        let src = vec![85];
        let texture = FungibleMemory::from_vec(src);
        let mut frame = Frame::new(1, 1, MemoryFormat::G8, texture).unwrap();
        change_memory_format(&mut frame, MemoryFormat::G16a16Premultiplied).unwrap();
        assert_eq!(&[85, 85, 255, 255], &frame.texture.as_ref());
    }

    #[test]
    fn rgba16_to_rgb16() {
        let src = vec![85, 85, 85, 85, 85, 85, 255, 255];
        let texture = FungibleMemory::from_vec(src);
        let mut frame = Frame::new(1, 1, MemoryFormat::R16g16b16a16, texture).unwrap();
        change_memory_format(&mut frame, MemoryFormat::R16g16b16).unwrap();
        assert_eq!(&[85, 85, 85, 85, 85, 85], &frame.texture.as_ref());
    }

    #[test]
    fn g16a16pre_to_g16a16() {
        let src = vec![85, 85, 255, 255];
        let texture = FungibleMemory::from_vec(src);
        let mut frame = Frame::new(1, 1, MemoryFormat::G16a16Premultiplied, texture).unwrap();
        change_memory_format(&mut frame, MemoryFormat::G16a16).unwrap();
        assert_eq!(&[85, 85, 255, 255], &frame.texture.as_ref());
    }

    #[test]
    fn u16_to_u8() {
        let texture = FungibleMemory::from_vec(if cfg!(target_endian = "little") {
            vec![
                127, 0, 128, 0, 127, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 127, 253, 128, 253,
                255, 255,
            ]
        } else {
            vec![
                0, 127, 0, 128, 2, 127, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 253, 127, 253, 128,
                255, 255,
            ]
        });
        let mut frame = Frame::new(2, 2, crate::MemoryFormat::R16g16b16, texture).unwrap();
        change_memory_format(&mut frame, MemoryFormat::R8g8b8).unwrap();
        assert_eq!(&*frame.texture, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 253, 254, 255]);
    }

    #[test]
    fn u8alpha_to_u8reversed() {
        let texture =
            FungibleMemory::from_vec(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let mut frame = Frame::new(2, 2, crate::MemoryFormat::R8g8b8a8, texture).unwrap();
        change_memory_format(&mut frame, MemoryFormat::B8g8r8).unwrap();
        assert_eq!(&*frame.texture, &[3, 2, 1, 7, 6, 5, 11, 10, 9, 15, 14, 13]);
    }

    #[test]
    fn u8premultiplied_to_u8() {
        let texture = FungibleMemory::from_vec(vec![127, 63, 0, 127, 127, 63, 0, 255]);
        let mut frame =
            Frame::new(1, 2, crate::MemoryFormat::R8g8b8a8Premultiplied, texture).unwrap();
        change_memory_format(&mut frame, MemoryFormat::R8g8b8a8).unwrap();
        assert_eq!(&*frame.texture, &[255, 126, 0, 127, 127, 63, 0, 255]);
    }

    #[test]
    fn u8_bgra_premultiplied_to_rgba() {
        let texture = FungibleMemory::from_vec(vec![127, 63, 0, 127, 127, 63, 0, 255]);
        let mut frame =
            Frame::new(1, 2, crate::MemoryFormat::B8g8r8a8Premultiplied, texture).unwrap();
        change_memory_format(&mut frame, MemoryFormat::R8g8b8a8).unwrap();
        assert_eq!(&*frame.texture, &[0, 126, 255, 127, 0, 63, 127, 255]);
    }

    #[test]
    fn u8_rgb_bgr() {
        let texture = FungibleMemory::from_vec(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        let mut frame = Frame::new(2, 2, crate::MemoryFormat::R8g8b8, texture).unwrap();
        change_memory_format(&mut frame, MemoryFormat::B8g8r8).unwrap();
        assert_eq!(&*frame.texture, &[3, 2, 1, 6, 5, 4, 9, 8, 7, 12, 11, 10]);
    }

    #[test]
    fn u8premultiplied_roundtrip() {
        let original = vec![127, 63, 0, 127, 127, 63, 0, 255];
        let texture = FungibleMemory::from_vec(original.clone());
        let mut frame =
            Frame::new(1, 2, crate::MemoryFormat::R8g8b8a8Premultiplied, texture).unwrap();
        change_memory_format(&mut frame, MemoryFormat::R8g8b8a8).unwrap();
        change_memory_format(&mut frame, MemoryFormat::R8g8b8a8Premultiplied).unwrap();
        assert_eq!(&*frame.texture, &original);
    }

    #[test]
    fn cmyk_to_rgb() {
        let texture = FungibleMemory::from_vec(vec![10, 20, 30, 5]);
        let mut frame = Frame::new(1, 1, crate::MemoryFormat::C8m8y8k8, texture).unwrap();
        change_memory_format(&mut frame, MemoryFormat::R8g8b8).unwrap();
        assert_eq!(&*frame.texture, &[240, 230, 221]);
    }

    #[test]
    fn cmyk_to_rgb_black() {
        let texture = FungibleMemory::from_vec(vec![0, 0, 0, 255]);
        let mut frame = Frame::new(1, 1, crate::MemoryFormat::C8m8y8k8, texture).unwrap();
        change_memory_format(&mut frame, MemoryFormat::R8g8b8).unwrap();
        assert_eq!(&*frame.texture, &[0, 0, 0]);
    }

    #[test]
    fn cmyk_to_rgb_black2() {
        let texture = FungibleMemory::from_vec(vec![255, 255, 255, 0]);
        let mut frame = Frame::new(1, 1, crate::MemoryFormat::C8m8y8k8, texture).unwrap();
        change_memory_format(&mut frame, MemoryFormat::R8g8b8).unwrap();
        assert_eq!(&*frame.texture, &[0, 0, 0]);
    }

    #[test]
    fn cmyk_to_rgb_white() {
        let texture = FungibleMemory::from_vec(vec![0, 0, 0, 0]);
        let mut frame = Frame::new(1, 1, crate::MemoryFormat::C8m8y8k8, texture).unwrap();
        change_memory_format(&mut frame, MemoryFormat::R8g8b8).unwrap();
        assert_eq!(&*frame.texture, &[255, 255, 255]);
    }

    #[test]
    fn rgb_to_cmyk_black() {
        let texture = FungibleMemory::from_vec(vec![0, 0, 0]);
        let mut frame = Frame::new(1, 1, crate::MemoryFormat::R8g8b8, texture).unwrap();
        change_memory_format(&mut frame, MemoryFormat::C8m8y8k8).unwrap();
        assert_eq!(&*frame.texture, &[0, 0, 0, 255]);
    }
}
