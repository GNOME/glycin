use glycin_utils::{Frame, ImageInfo};
use gufo_common::orientation::{Orientation, Rotation};

use crate::dbus::ImgBuf;

pub fn apply_exif_orientation(
    img_buf: ImgBuf,
    frame: &mut Frame,
    image_info: &ImageInfo,
) -> ImgBuf {
    if image_info.details.transformations_applied {
        img_buf
    } else if let Some(exif_data) = image_info
        .details
        .exif
        .as_ref()
        .and_then(|x| x.get_full().ok())
    {
        match gufo_exif::Exif::new(exif_data) {
            Err(err) => {
                eprintln!("exif: Failed to parse data: {err:?}");
                img_buf
            }
            Ok(data) => transform(img_buf, frame, data.orientation()),
        }
    } else {
        img_buf
    }
}

#[allow(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]
fn transform(mut img_buf: ImgBuf, frame: &mut Frame, transformation: Orientation) -> ImgBuf {
    let stride = frame.stride as usize;
    let width = frame.width as usize;
    let height = frame.height as usize;
    let pixel_size = frame.memory_format.n_bytes().usize();

    let n_bytes = width * height * pixel_size;

    if transformation.mirror() {
        for x in 0..width / 2 {
            for y in 0..height {
                for i in 0..pixel_size {
                    let p0 = x * pixel_size + y * stride + i;
                    let p1 = (width - 1 - x) * pixel_size + y * stride + i;
                    img_buf.swap(p0, p1);
                }
            }
        }
    }

    match transformation.rotate() {
        Rotation::_0 => img_buf,
        Rotation::_270 => {
            let mut v = vec![0; n_bytes];
            frame.width = height as u32;
            frame.height = width as u32;
            frame.stride = (height * pixel_size) as u32;

            for x in 0..width {
                for y in 0..height {
                    for i in 0..pixel_size {
                        let p0 = x * pixel_size + y * stride + i;
                        let p1 = x * height * pixel_size + (height - 1 - y) * pixel_size + i;
                        v[p1] = img_buf[p0];
                    }
                }
            }

            ImgBuf::Vec(v)
        }
        Rotation::_90 => {
            let mut v = vec![0; n_bytes];
            frame.width = height as u32;
            frame.height = width as u32;
            frame.stride = (height * pixel_size) as u32;

            for x in 0..width {
                for y in 0..height {
                    for i in 0..pixel_size {
                        let p0 = x * pixel_size + y * stride + i;
                        let p1 = (width - 1 - x) * height * pixel_size + y * pixel_size + i;
                        v[p1] = img_buf[p0];
                    }
                }
            }

            ImgBuf::Vec(v)
        }
        Rotation::_180 => {
            let mid_col = width / 2;
            let uneven_cols = width % 2 == 1;

            for x in 0..width.div_ceil(2) {
                let y_max = if uneven_cols && mid_col == x {
                    height / 2
                } else {
                    height
                };
                for y in 0..y_max {
                    for i in 0..pixel_size {
                        let p0 = x * pixel_size + y * stride + i;
                        let p1 = (width - 1 - x) * pixel_size + (height - 1 - y) * stride + i;

                        img_buf.swap(p0, p1);
                    }
                }
            }

            img_buf
        }
    }
}
