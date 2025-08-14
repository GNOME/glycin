use glycin_utils::{Frame, ImgBuf};

use crate::Image;

pub fn apply_exif_orientation(img_buf: ImgBuf, frame: &mut Frame, image: &Image) -> ImgBuf {
    if image.details().transformation_ignore_exif() {
        img_buf
    } else {
        let orientation = image.transformation_orientation();
        glycin_utils::editing::change_orientation(img_buf, frame, orientation)
    }
}
