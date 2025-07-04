use glycin_common::{ExtendedMemoryFormat, MemoryFormatInfo};
use gufo_common::orientation::{Orientation, Rotation};

use super::EditingFrame;
use crate::{Frame, ImgBuf};

pub trait FrameDimensions {
    fn width(&self) -> u32;
    fn set_width(&mut self, width: u32);
    fn height(&self) -> u32;
    fn set_height(&mut self, height: u32);
    fn stride(&self) -> u32;
    fn set_stride(&mut self, stride: u32);
    fn memory_format(&self) -> ExtendedMemoryFormat;
}

impl FrameDimensions for EditingFrame {
    fn width(&self) -> u32 {
        self.width
    }

    fn set_width(&mut self, width: u32) {
        self.width = width;
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn set_height(&mut self, height: u32) {
        self.height = height;
    }

    fn stride(&self) -> u32 {
        self.stride
    }

    fn set_stride(&mut self, stride: u32) {
        self.stride = stride;
    }

    fn memory_format(&self) -> ExtendedMemoryFormat {
        self.memory_format
    }
}

impl FrameDimensions for Frame {
    fn width(&self) -> u32 {
        self.width
    }

    fn set_width(&mut self, width: u32) {
        self.width = width;
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn set_height(&mut self, height: u32) {
        self.height = height;
    }

    fn stride(&self) -> u32 {
        self.stride
    }

    fn set_stride(&mut self, stride: u32) {
        self.stride = stride;
    }

    fn memory_format(&self) -> ExtendedMemoryFormat {
        self.memory_format.into()
    }
}

#[allow(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]
pub fn change_orientation(
    mut img_buf: ImgBuf,
    frame: &mut impl FrameDimensions,
    transformation: Orientation,
) -> ImgBuf {
    let stride = frame.stride() as usize;
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    let pixel_size = frame.memory_format().n_bytes().usize();

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
            frame.set_width(height as u32);
            frame.set_height(width as u32);
            frame.set_stride((height * pixel_size) as u32);

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
            frame.set_width(height as u32);
            frame.set_height(width as u32);
            frame.set_stride((height * pixel_size) as u32);

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
