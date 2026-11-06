use std::fmt::{Debug, Display};
use std::sync::Arc;

use crate::{DimensionTooLargerError, FungibleMemory, LocalMemory};

mod change_memory_format;
mod clip;
mod operations;
mod orientation;

pub use change_memory_format::change_memory_format;
pub use clip::clip;
use glycin_common::{ExtendedMemoryFormat, OperationId};
use gufo_common::math::MathError;
use gufo_common::read::ReadError;
pub use operations::apply_operations;
pub use orientation::change_orientation;

use crate::ByteData;

#[derive(Debug, Clone)]
pub struct EditingFrame<B: ByteData> {
    pub width: u32,
    pub height: u32,
    /// Line stride
    pub stride: u32,
    pub memory_format: ExtendedMemoryFormat,
    pub texture: B,
}

impl EditingFrame<LocalMemory> {
    pub fn into_funglible(self) -> EditingFrame<FungibleMemory> {
        EditingFrame {
            width: self.width,
            height: self.height,
            stride: self.stride,
            memory_format: self.memory_format,
            texture: self.texture.into_fungible(),
        }
    }
}

#[derive(Debug, thiserror::Error, Clone)]
#[non_exhaustive]
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
    #[error("Unknown operation: {0:?}")]
    UnknownOperation(OperationId),
    #[error("Failed to build rayon thread pool: {0}")]
    ThreadPoolBuildError(#[from] Arc<rayon::ThreadPoolBuildError>),
    #[error("Failed to allocate memory: {0}")]
    MemoryAllocationError(crate::memory::MemoryAllocationError),
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

impl From<crate::memory::MemoryAllocationError> for Error {
    fn from(value: crate::memory::MemoryAllocationError) -> Self {
        Self::MemoryAllocationError(value)
    }
}
