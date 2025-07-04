use glycin_common::{Operation, Operations};
use gufo_common::orientation::{Orientation, Rotation};

use super::{EditingFrame, Error};
use crate::{editing, ImgBuf};

pub fn apply_operations(
    mut buf: Vec<u8>,
    simple_frame: &mut EditingFrame,
    operations: &Operations,
) -> Result<Vec<u8>, Error> {
    for operation in operations.operations() {
        match operation {
            Operation::Rotate(rotation) => {
                buf = editing::change_orientation(
                    ImgBuf::Vec(buf),
                    simple_frame,
                    Orientation::new(false, *rotation),
                )
                .into_vec();
            }
            Operation::MirrorHorizontally => {
                buf = editing::change_orientation(
                    ImgBuf::Vec(buf),
                    simple_frame,
                    Orientation::new(true, Rotation::_0),
                )
                .into_vec();
            }
            Operation::MirrorVertically => {
                buf = editing::change_orientation(
                    ImgBuf::Vec(buf),
                    simple_frame,
                    Orientation::new(true, Rotation::_180),
                )
                .into_vec();
            }
            Operation::Clip(clip) => {
                buf = editing::clip(buf, simple_frame, *clip)?;
            }
            op => return Err(Error::UnknownOperation(op.id())),
        }
    }

    Ok(buf)
}
