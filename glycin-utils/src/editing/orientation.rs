use glycin_common::{ExtendedMemoryFormat, MemoryFormatInfo};
use gufo_common::orientation::{Orientation, Rotation};

use super::EditingFrame;
use crate::{ByteData, Frame, FungibleMemory, editing};

pub trait BasicFrame<B: ByteData> {
    fn width(&self) -> u32;
    fn set_width(&mut self, width: u32);
    fn height(&self) -> u32;
    fn set_height(&mut self, height: u32);
    fn stride(&self) -> u32;
    fn set_stride(&mut self, stride: u32);
    fn memory_format(&self) -> ExtendedMemoryFormat;
    fn texture(&self) -> &B;
    fn texture_mut(&mut self) -> &mut B;
    fn set_texture(&mut self, texture: B);
}

impl<B: ByteData> BasicFrame<B> for EditingFrame<B> {
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

    fn texture(&self) -> &B {
        &self.texture
    }

    fn set_texture(&mut self, texture: B) {
        self.texture = texture;
    }

    fn texture_mut(&mut self) -> &mut B {
        &mut self.texture
    }
}

impl<B: ByteData> BasicFrame<B> for Frame<B> {
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

    fn texture(&self) -> &B {
        &self.texture
    }

    fn texture_mut(&mut self) -> &mut B {
        &mut self.texture
    }

    fn set_texture(&mut self, texture: B) {
        self.texture = texture;
    }
}

#[allow(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]
pub fn change_orientation<F: BasicFrame<FungibleMemory>>(
    mut frame: F,
    transformation: Orientation,
) -> Result<F, editing::Error> {
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
                    frame.texture_mut().swap(p0, p1);
                }
            }
        }
    }

    match transformation.rotate() {
        Rotation::_0 => Ok(frame),
        Rotation::_270 => {
            let mut target = FungibleMemory::new(n_bytes as u64)?;
            frame.set_width(height as u32);
            frame.set_height(width as u32);
            frame.set_stride((height * pixel_size) as u32);

            let src = frame.texture_mut();

            for x in 0..width {
                for y in 0..height {
                    for i in 0..pixel_size {
                        let p0 = x * pixel_size + y * stride + i;
                        let p1 = x * height * pixel_size + (height - 1 - y) * pixel_size + i;
                        target[p1] = src[p0];
                    }
                }
            }

            frame.set_texture(target);

            Ok(frame)
        }
        Rotation::_90 => {
            let mut target = FungibleMemory::new(n_bytes as u64)?;
            frame.set_width(height as u32);
            frame.set_height(width as u32);
            frame.set_stride((height * pixel_size) as u32);

            let src = frame.texture_mut();

            for x in 0..width {
                for y in 0..height {
                    for i in 0..pixel_size {
                        let p0 = x * pixel_size + y * stride + i;
                        let p1 = (width - 1 - x) * height * pixel_size + y * pixel_size + i;
                        target[p1] = src[p0];
                    }
                }
            }

            frame.set_texture(target);

            Ok(frame)
        }
        Rotation::_180 => {
            let mid_col = width / 2;
            let uneven_cols = width % 2 == 1;

            let src = frame.texture_mut();

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

                        src.swap(p0, p1);
                    }
                }
            }

            Ok(frame)
        }
    }
}
