use std::{
    fmt::{Debug, Display},
    sync::Arc,
};

use crate::{DimensionTooLargerError, MemoryFormat};

mod change_memory_format;
mod clip;
mod operations;
mod orientation;

pub use change_memory_format::change_memory_format;
pub use clip::clip;
use gufo_common::{math::MathError, read::ReadError};
pub use orientation::change_orientation;

pub struct SimpleFrame {
    pub width: u32,
    pub height: u32,
    /// Line stride
    pub stride: u32,
    pub memory_format: MemoryFormat,
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum Error {
    #[error("IO Error: {0}")]
    Io(#[from] Arc<std::io::Error>),
    #[error("Math Error: {0}")]
    Math(#[from] MathError),
    #[error("Read Error: {0}")]
    ReadError(#[from] ReadError),
    #[error("{0}")]
    DimensionTooLargerError(#[from] DimensionTooLargerError),
    #[error("Zerocopy: {0}")]
    ZerocopyConvertError(String),
}

impl<A: Display, S: Display, V: Display> From<zerocopy::ConvertError<A, S, V>> for Error {
    fn from(value: zerocopy::ConvertError<A, S, V>) -> Self {
        Self::ZerocopyConvertError(value.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Arc::new(value).into()
    }
}
