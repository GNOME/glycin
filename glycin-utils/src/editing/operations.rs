use glycin_common::{Operation, Operations};
use gufo_common::orientation::{Orientation, Rotation};

use super::{EditingFrame, Error};
use crate::{FungibleMemory, editing};

pub fn apply_operations(
    mut frame: EditingFrame<FungibleMemory>,
    operations: &Operations,
) -> Result<EditingFrame<FungibleMemory>, Error> {
    for operation in operations.operations() {
        match operation {
            Operation::Rotate(rotation) => {
                frame = editing::change_orientation(frame, Orientation::new(false, *rotation))?;
            }
            Operation::MirrorHorizontally => {
                frame = editing::change_orientation(frame, Orientation::new(true, Rotation::_0))?;
            }
            Operation::MirrorVertically => {
                frame = editing::change_orientation(frame, Orientation::new(true, Rotation::_180))?;
            }
            Operation::Clip(clip) => {
                frame = editing::clip(frame, *clip)?;
            }
            op => return Err(Error::UnknownOperation(op.id())),
        }
    }

    Ok(frame)
}
