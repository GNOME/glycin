use gufo_common::math::Checked;
use gufo_common::read::*;

use crate::memory_format::MemoryFormatInfo;
use crate::{editing, Frame, ImgBuf, MemoryFormat};

pub fn change_memory_format(
    mut img_buf: ImgBuf,
    frame: &mut Frame,
    target_format: MemoryFormat,
) -> Result<ImgBuf, editing::Error> {
    let src_format = frame.memory_format;
    let src_pixel_n_bytes = src_format.n_bytes().usize();

    let target_pixel_n_bytes = target_format.n_bytes().usize();

    if src_format == target_format {
        return Ok(img_buf);
    }

    let src_data = img_buf.as_mut_slice();

    let mut new_data;

    let new_stride = (Checked::new(frame.width) * target_format.n_bytes().u32()).check()?;
    let new_total_size = (Checked::new(frame.height as usize) * new_stride as usize).check()?;

    // Check if we need a new allocation because otherwise we would overwrite the
    // data that we are reading
    let new_alloc = new_total_size > frame.n_bytes()?;

    if new_alloc {
        new_data = vec![0; new_total_size];
    } else {
        new_data = Vec::new();
    }

    for x in 0..frame.width as usize {
        let x_ = Checked::new(x) * src_pixel_n_bytes;
        for y in 0..frame.height as usize {
            let i0 = x_ + Checked::new(y) * frame.stride as usize;
            let i1 = i0 + src_pixel_n_bytes;

            let k0 = Checked::new(x) * target_pixel_n_bytes + Checked::new(y) * new_stride as usize;
            let k1 = k0 + target_pixel_n_bytes;

            if new_alloc {
                MemoryFormat::transform(
                    src_format,
                    &src_data.e_get(i0.check()?..i1.check()?)?,
                    target_format,
                    new_data.e_get_mut(k0.check()?..k1.check()?)?,
                )?;
            } else {
                let channels_f32 =
                    MemoryFormat::to_f32(src_format, src_data.e_get(i0.check()?..i1.check()?)?)?;

                MemoryFormat::from_f32(
                    channels_f32,
                    target_format,
                    src_data.e_get_mut(k0.check()?..k1.check()?)?,
                )?;
            }
        }
    }

    frame.stride = new_stride;
    frame.memory_format = target_format;

    // TODO: potentially reduce size of mmap

    if new_alloc {
        Ok(ImgBuf::Vec(new_data))
    } else {
        Ok(img_buf)
    }
}
