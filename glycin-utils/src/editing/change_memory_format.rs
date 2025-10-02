use std::sync::Arc;

use glycin_common::{ChannelType, MemoryFormatInfo, Source, Target};
use gufo_common::math::Checked;
use rayon::iter::IntoParallelIterator;
use rayon::prelude::*;

use crate::{editing, Frame, ImgBuf, MemoryFormat};
pub fn change_memory_format(
    mut img_buf: ImgBuf,
    mut frame: Frame,
    target_format: MemoryFormat,
) -> Result<(Frame, ImgBuf), editing::Error> {
    let src_format = frame.memory_format;

    if src_format == target_format {
        log::debug!("Same image format {src_format:?}, no need for transformation");
        return Ok((frame, img_buf));
    }

    log::debug!("Starting to transform image format from {src_format:?} to {target_format:?}");
    let start_instant = std::time::Instant::now();

    let src_format = frame.memory_format;
    let src_data = img_buf.as_mut_slice();
    let src_pixel_n_bytes = src_format.n_bytes().usize();

    let target_pixel_n_bytes = target_format.n_bytes().usize();
    let new_stride = (Checked::new(frame.width) * target_format.n_bytes().u32()).check()?;
    let new_total_size: usize =
        (Checked::new(frame.height as usize) * new_stride as usize).check()?;

    let mut new_data = vec![0; new_total_size];

    // Target rows for parralel processing
    let mut target_rows = Vec::new();
    (0..frame.height as usize).fold(new_data.as_mut_slice(), |x, y| {
        let (row, rest) = x.split_at_mut(new_stride as usize);
        target_rows.push((y, row));
        rest
    });

    rayon::ThreadPoolBuilder::new()
        .thread_name(|i| format!("gly-rayon-{i}"))
        .build()
        .map_err(Arc::new)?
        .install(|| {
            if src_format.channel_type() == target_format.channel_type()
                && src_format.is_premultiplied() == target_format.is_premultiplied()
                && (!src_format.source_definition().contains(&Source::Opaque)
                    || !target_format.target_definition().contains(&Target::A))
                && !target_format.target_definition().contains(&Target::RgbAvg)
            {
                let mut source_target_index_map = [0; 4];
                for (n, target) in target_format.target_definition().iter().enumerate() {
                    source_target_index_map[n] =
                        src_format.source_definition()[*target as usize] as usize;
                }

                let target_n_channels = target_format.n_channels();

                target_rows.into_par_iter().for_each(|(y, new_row)| {
                    for x in 0..frame.width as usize {
                        let x_ = x * src_pixel_n_bytes;

                        // src bytes for pixel
                        let i0 = x_ + y * frame.stride as usize;

                        // target bytes for pixel
                        let k0 = x * target_pixel_n_bytes;

                        for channel_byte in 0..target_format.channel_type().size() {
                            for i in 0..target_n_channels as usize {
                                new_row[k0 + i + channel_byte] =
                                    src_data[i0 + source_target_index_map[i] + channel_byte];
                            }
                        }
                    }
                });
            } else if src_format.channel_type() == ChannelType::U16
                && target_format.channel_type() == ChannelType::U8
                && src_format.is_premultiplied() == target_format.is_premultiplied()
                && (!src_format.source_definition().contains(&Source::Opaque)
                    || !target_format.target_definition().contains(&Target::A))
                && !target_format.target_definition().contains(&Target::RgbAvg)
            {
                let mut source_target_index_map = [0; 4];
                for (n, target) in target_format.target_definition().iter().enumerate() {
                    source_target_index_map[n] =
                        src_format.source_definition()[*target as usize] as usize;
                }

                let target_n_channels = target_format.n_channels();
                let source_channel_size = src_format.channel_type().size();

                target_rows.into_par_iter().for_each(|(y, new_row)| {
                    for x in 0..frame.width as usize {
                        let x_ = x * src_pixel_n_bytes;

                        // src bytes for pixel
                        let i0 = x_ + y * frame.stride as usize;

                        // target bytes for pixel
                        let k0 = x * target_pixel_n_bytes;

                        for i in 0..target_n_channels as usize {
                            new_row[k0 + i] = (u16::from_ne_bytes([
                                src_data[i0 + source_target_index_map[i] * source_channel_size],
                                src_data[i0 + source_target_index_map[i] * source_channel_size + 1],
                            ])
                            .saturating_add(128)
                                >> 8) as u8;
                        }
                    }
                });
            } else {
                target_rows.into_par_iter().for_each(|(y, new_row)| {
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
            }
        });

    frame.stride = new_stride;
    frame.memory_format = target_format;

    log::debug!(
        "Transformation completed after {:?}",
        start_instant.elapsed()
    );

    Ok((frame, ImgBuf::Vec(new_data)))
}

#[cfg(test)]
mod test {
    use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};

    use glycin_common::BinaryData;

    use super::*;

    #[test]
    fn u16_to_u8() {
        let (a, _) = std::os::unix::net::UnixStream::pair().unwrap();
        let texture = BinaryData::from(unsafe { OwnedFd::from_raw_fd(a.into_raw_fd()) });
        let img_buf = ImgBuf::Vec(if cfg!(target_endian = "little") {
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
        let frame = Frame::new(2, 2, crate::MemoryFormat::R16g16b16, texture).unwrap();
        let x = change_memory_format(img_buf, frame, MemoryFormat::R8g8b8)
            .unwrap()
            .1;
        assert_eq!(x.as_slice(), &[0, 1, 2, 3, 4, 5, 6, 7, 8, 253, 254, 255]);
    }

    #[test]
    fn u8alpha_to_u8reversed() {
        let (a, _) = std::os::unix::net::UnixStream::pair().unwrap();
        let texture = BinaryData::from(unsafe { OwnedFd::from_raw_fd(a.into_raw_fd()) });
        let img_buf = ImgBuf::Vec(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let frame = Frame::new(2, 2, crate::MemoryFormat::R8g8b8a8, texture).unwrap();
        let x = change_memory_format(img_buf, frame, MemoryFormat::B8g8r8)
            .unwrap()
            .1;
        assert_eq!(x.as_slice(), &[3, 2, 1, 7, 6, 5, 11, 10, 9, 15, 14, 13]);
    }

    #[test]
    fn u8premultiplied_to_u8() {
        let (a, _) = std::os::unix::net::UnixStream::pair().unwrap();
        let texture = BinaryData::from(unsafe { OwnedFd::from_raw_fd(a.into_raw_fd()) });
        let img_buf = ImgBuf::Vec(vec![127, 63, 0, 127, 127, 63, 0, 255]);
        let frame = Frame::new(1, 2, crate::MemoryFormat::R8g8b8a8Premultiplied, texture).unwrap();
        let x = change_memory_format(img_buf, frame, MemoryFormat::R8g8b8a8)
            .unwrap()
            .1;
        assert_eq!(x.as_slice(), &[255, 126, 0, 127, 127, 63, 0, 255]);
    }
}
