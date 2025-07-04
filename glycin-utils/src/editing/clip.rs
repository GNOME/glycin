use std::io::{Cursor, Read, Seek};

use glycin_common::MemoryFormatInfo;
use gufo_common::math::{checked, Checked};

use super::{EditingFrame, Error};

pub fn clip(
    buf: Vec<u8>,
    frame: &mut EditingFrame,
    (x, y, width, height): (u32, u32, u32, u32),
) -> Result<Vec<u8>, Error> {
    let pixel_size = frame.memory_format.n_bytes().u32();

    checked![pixel_size, x, y];

    let max_width = (frame.width - x).check()?;
    let max_height = (frame.height - y).check()?;

    let width = u32::min(width, max_width);
    let height = u32::min(height, max_height);

    let new_stride = (width * pixel_size).check()?;
    let size = (Checked::new(height as usize) * new_stride as usize).check()?;
    let mut new = Vec::with_capacity(size);

    let stride = frame.stride as i64;
    let x_ = (x.i64() * pixel_size.i64()).check()?;
    let width_ = width as i64 * pixel_size.i64();

    checked![stride];

    let mut cur = Cursor::new(buf);
    let mut row = vec![0; (width as usize * pixel_size.usize()).check()?];

    cur.seek_relative((y.i64() * stride).check()?)?;

    for _ in 0..height {
        cur.seek_relative(x_)?;
        cur.read_exact(&mut row)?;
        new.extend_from_slice(&row);
        cur.seek_relative((stride - x_ - width_).check()?)?;
    }

    frame.width = width;
    frame.height = height;
    frame.stride = new_stride;

    Ok(new)
}
