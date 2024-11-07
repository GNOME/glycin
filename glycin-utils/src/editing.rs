use crate::MemoryFormat;

mod clip;
mod operations;
mod orientation;

pub use clip::clip;
use gufo_common::math::MathError;
pub use orientation::change_orientation;

pub struct SimpleFrame {
    pub width: u32,
    pub height: u32,
    /// Line stride
    pub stride: u32,
    pub memory_format: MemoryFormat,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Math Error: {0}")]
    Math(#[from] MathError),
}
