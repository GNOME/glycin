use std::ops::{Deref, DerefMut};

use crate::{ByteData, FungibleMemory, MemoryAllocationError};

#[cfg(unix)]
const MMAP_THRESHOLD: usize = 128 * 1024;

#[derive(Debug)]
pub enum LocalMemory {
    Vec(Vec<u8>),
    #[cfg(unix)]
    Mmap(memmap::MmapMut),
}

impl ByteData for LocalMemory {
    fn new(size: u64) -> Result<Self, MemoryAllocationError> {
        #[cfg(unix)]
        {
            Ok(if size > MMAP_THRESHOLD as u64 {
                Self::Mmap(
                    memmap::MmapMut::map_anon(size as usize)
                        .map_err(|x| MemoryAllocationError(x.to_string()))?,
                )
            } else {
                Self::Vec(vec![0; size as usize])
            })
        }
        #[cfg(not(unix))]
        Ok(Self::Vec(vec![0; size as usize]))
    }

    fn into_fungible(self) -> FungibleMemory {
        FungibleMemory::LocalMemory(self)
    }

    #[cfg(feature = "external")]
    fn from_shared(shared: crate::SharedMemory) -> Self {
        Self::try_from_slice(&shared).unwrap()
    }

    fn from_local(local: crate::LocalMemory) -> Self {
        local
    }

    fn into_other<O: ByteData>(self) -> Result<O, MemoryAllocationError> {
        Ok(O::from_local(self))
    }

    fn try_from_vec(value: Vec<u8>) -> Result<Self, MemoryAllocationError> {
        Ok(Self::Vec(value))
    }

    fn try_from_slice(value: &[u8]) -> Result<Self, MemoryAllocationError> {
        #[cfg(unix)]
        {
            if value.len() > MMAP_THRESHOLD {
                let mut local = Self::new(value.len() as u64)
                    .map_err(|x| MemoryAllocationError(x.to_string()))?;
                local.copy_from_slice(value);
                return Ok(local);
            }
        }

        Ok(Self::Vec(value.to_vec()))
    }

    async fn final_seal(&mut self) -> Result<(), MemoryAllocationError> {
        Ok(())
    }

    async fn initial_seal(&mut self) -> Result<(), MemoryAllocationError> {
        Ok(())
    }

    #[cfg(feature = "glib")]
    fn into_gbytes(self) -> Result<glib::Bytes, MemoryAllocationError> {
        match self {
            #[cfg(unix)]
            Self::Mmap(mmap) => Ok(glib::Bytes::from_owned(mmap)),
            Self::Vec(vec) => Ok(glib::Bytes::from_owned(vec)),
        }
    }
}

impl Deref for LocalMemory {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            #[cfg(unix)]
            Self::Mmap(mmap) => mmap,
            Self::Vec(mmap) => mmap,
        }
    }
}

impl DerefMut for LocalMemory {
    fn deref_mut(&mut self) -> &mut [u8] {
        match self {
            #[cfg(unix)]
            Self::Mmap(mmap) => mmap.as_mut(),
            Self::Vec(mmap) => mmap.as_mut(),
        }
    }
}

impl Clone for LocalMemory {
    fn clone(&self) -> Self {
        match self {
            #[cfg(unix)]
            Self::Mmap(mmap) => Self::try_from_slice(mmap).unwrap(),
            Self::Vec(vec) => Self::try_from_slice(vec).unwrap(),
        }
    }
}
