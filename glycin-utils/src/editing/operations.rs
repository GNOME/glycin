use gufo_common::orientation::{Orientation, Rotation};

use super::{Error, SimpleFrame};
use crate::operations::{Operation, Operations};
use crate::{editing, ImgBuf};

impl Operations {
    pub fn apply(
        &self,
        mut buf: Vec<u8>,
        simple_frame: &mut SimpleFrame,
    ) -> Result<Vec<u8>, Error> {
        for operation in self.operations() {
            match operation {
                Operation::Rotate(rotation) => {
                    buf = editing::change_orientation(
                        ImgBuf::Vec(buf),
                        simple_frame,
                        Orientation::new(*rotation, false),
                    )
                    .into_vec();
                }
                Operation::MirrorHorizontally => {
                    buf = editing::change_orientation(
                        ImgBuf::Vec(buf),
                        simple_frame,
                        Orientation::new(Rotation::_0, true),
                    )
                    .into_vec();
                }
                Operation::Clip(clip) => {
                    buf = editing::clip(buf, simple_frame, *clip)?;
                }
            }
        }

        Ok(buf)
    }
}