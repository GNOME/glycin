use glycin_utils::{Frame, FungibleMemory, editing};

use crate::Image;

pub fn apply_exif_orientation(
    frame: Frame<FungibleMemory>,
    image: &Image,
) -> Result<Frame<FungibleMemory>, editing::Error> {
    if image.details().transformation_ignore_exif() {
        Ok(frame)
    } else {
        let orientation = image.transformation_orientation();
        glycin_utils::editing::change_orientation(frame, orientation)
    }
}
