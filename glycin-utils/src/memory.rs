use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

#[cfg(feature = "external")]
pub use zbus::zvariant;

mod fungible;
mod local;
#[cfg(feature = "external")]
mod shared;

pub use fungible::*;
pub use local::*;
#[cfg(feature = "external")]
pub use shared::*;

#[derive(Debug)]
pub struct MemoryAllocationError(pub(crate) String);

impl std::fmt::Display for MemoryAllocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MemoryAllocationError {}

pub trait ByteData: Sized + Deref<Target = [u8]> + DerefMut + Debug + 'static {
    fn new(size: u64) -> std::io::Result<Self>;
    fn into_fungible(self) -> FungibleMemory;
    fn into_other<O: ByteData>(self) -> Result<O, MemoryAllocationError>;
    #[cfg(feature = "external")]
    fn from_shared(shared: SharedMemory) -> Self;
    fn try_from_vec(vec: Vec<u8>) -> Result<Self, MemoryAllocationError>;
    fn try_from_slice(slice: &[u8]) -> Result<Self, MemoryAllocationError>;
    fn initial_seal(
        &mut self,
    ) -> impl std::future::Future<Output = Result<(), MemoryAllocationError>> + Send;
    fn final_seal(
        &mut self,
    ) -> impl std::future::Future<Output = Result<(), MemoryAllocationError>> + Send;
    #[cfg(feature = "glib")]
    fn into_gbytes(self) -> Result<glib::Bytes, MemoryAllocationError>;
}
