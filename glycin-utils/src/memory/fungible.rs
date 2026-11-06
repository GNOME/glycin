use std::ops::{Deref, DerefMut};

use crate::{ByteData, MemoryAllocationError};

#[derive(Debug)]
pub enum FungibleMemory {
    #[cfg(feature = "external")]
    SharedMemory(crate::SharedMemory),
    LocalMemory(crate::LocalMemory),
}

impl FungibleMemory {
    #[deprecated]
    pub fn from_vec(vec: Vec<u8>) -> Self {
        FungibleMemory::LocalMemory(crate::LocalMemory::try_from_vec(vec).unwrap())
    }
}

impl ByteData for FungibleMemory {
    fn new(size: u64) -> Result<Self, MemoryAllocationError> {
        Ok(Self::LocalMemory(crate::LocalMemory::new(size)?))
    }

    fn into_fungible(self) -> FungibleMemory {
        self
    }

    #[cfg(feature = "external")]
    fn from_shared(shared: crate::SharedMemory) -> Self {
        FungibleMemory::SharedMemory(shared)
    }

    fn from_local(local: crate::LocalMemory) -> Self {
        FungibleMemory::LocalMemory(local)
    }

    fn into_other<O: ByteData>(self) -> Result<O, MemoryAllocationError> {
        match self {
            Self::LocalMemory(local) => Ok(O::from_local(local)),
            #[cfg(feature = "external")]
            Self::SharedMemory(shared) => Ok(O::from_shared(shared)),
        }
    }

    fn try_from_vec(value: Vec<u8>) -> Result<Self, MemoryAllocationError> {
        Ok(Self::LocalMemory(crate::LocalMemory::try_from_vec(value)?))
    }

    fn try_from_slice(value: &[u8]) -> Result<Self, MemoryAllocationError> {
        Ok(Self::LocalMemory(crate::LocalMemory::try_from_slice(
            value,
        )?))
    }

    async fn initial_seal(&mut self) -> Result<(), MemoryAllocationError> {
        #[cfg(feature = "external")]
        if let Self::SharedMemory(shared) = self {
            shared.initial_seal().await?;
        }

        Ok(())
    }

    async fn final_seal(&mut self) -> Result<(), MemoryAllocationError> {
        #[cfg(feature = "external")]
        if let Self::SharedMemory(shared) = self {
            shared.final_seal().await?;
        }

        Ok(())
    }

    #[cfg(feature = "glib")]
    fn into_gbytes(self) -> Result<glib::Bytes, MemoryAllocationError> {
        match self {
            FungibleMemory::LocalMemory(local) => local.into_gbytes(),
            #[cfg(feature = "external")]
            FungibleMemory::SharedMemory(shared) => shared.into_gbytes(),
        }
    }
}

impl Deref for FungibleMemory {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Self::LocalMemory(local) => local,
            #[cfg(feature = "external")]
            Self::SharedMemory(shared) => shared,
        }
    }
}

impl DerefMut for FungibleMemory {
    fn deref_mut(&mut self) -> &mut [u8] {
        match self {
            Self::LocalMemory(local) => local,
            #[cfg(feature = "external")]
            Self::SharedMemory(shared) => shared,
        }
    }
}
