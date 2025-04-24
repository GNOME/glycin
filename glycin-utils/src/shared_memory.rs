use std::ffi::CString;
use std::ops::{Deref, DerefMut};
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::Arc;

use zbus::zvariant;

use crate::{BinaryData, ProcessError, SafeConversion};

#[derive(Debug)]
pub struct SharedMemory {
    memfd: OwnedFd,
    pub mmap: memmap::MmapMut,
}

impl SharedMemory {
    pub fn new(size: u64) -> Result<Self, ProcessError> {
        let memfd = nix::sys::memfd::memfd_create(
            &CString::new("glycin-frame").unwrap(),
            nix::sys::memfd::MemFdCreateFlag::MFD_CLOEXEC
                | nix::sys::memfd::MemFdCreateFlag::MFD_ALLOW_SEALING,
        )
        .expect("Failed to create memfd");

        nix::unistd::ftruncate(&memfd, size.try_into().expect("Required memory too large"))
            .expect("Failed to set memfd size");

        let raw_fd = memfd.as_raw_fd();
        let mmap = unsafe { memmap::MmapMut::map_mut(raw_fd) }.map_err(|err| {
            if err.kind() == std::io::ErrorKind::OutOfMemory {
                ProcessError::out_of_memory()
            } else {
                ProcessError::expected(&err)
            }
        })?;

        Ok(Self { mmap, memfd })
    }

    pub fn into_binary_data(self) -> BinaryData {
        let owned_fd = zvariant::OwnedFd::from(self.memfd);
        BinaryData {
            memfd: Arc::new(owned_fd),
        }
    }
}

impl SharedMemory {
    fn from_data(value: impl AsRef<[u8]>) -> Result<Self, ProcessError> {
        let mut shared_memory = SharedMemory::new(value.as_ref().len().try_u64()?)?;

        shared_memory.copy_from_slice(value.as_ref());

        Ok(shared_memory)
    }
}

impl BinaryData {
    pub fn from_data(value: impl AsRef<[u8]>) -> Result<Self, ProcessError> {
        Ok(SharedMemory::from_data(value)?.into_binary_data())
    }
}

impl Deref for SharedMemory {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.mmap.deref()
    }
}

impl DerefMut for SharedMemory {
    fn deref_mut(&mut self) -> &mut [u8] {
        self.mmap.deref_mut()
    }
}
