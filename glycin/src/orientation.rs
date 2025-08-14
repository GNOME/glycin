use glycin_utils::{Frame, ImgBuf};

use crate::ImageDetails;

pub fn apply_exif_orientation(
    img_buf: ImgBuf,
    frame: &mut Frame,
    image_info: &ImageDetails,
) -> ImgBuf {
    if image_info.transformation_ignore_exif() {
        img_buf
    } else if let Some(orientation) = image_info.transformation_orientation() {
        glycin_utils::editing::change_orientation(img_buf, frame, orientation)
    } else if let Some(exif_data) = image_info
        .metadata_exif()
        .as_ref()
        .and_then(|x| x.get_full().ok())
    {
        match gufo_exif::Exif::new(exif_data) {
            Err(err) => {
                eprintln!("exif: Failed to parse data: {err:?}");
                img_buf
            }
            Ok(data) => {
                if let Some(orientation) = data.orientation() {
                    glycin_utils::editing::change_orientation(img_buf, frame, orientation)
                } else {
                    img_buf
                }
            }
        }
    } else {
        img_buf
    }
}
