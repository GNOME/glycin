use std::ops::{Deref, DerefMut};
use std::os::fd::{AsRawFd, OwnedFd};

use crate::{BinaryData, Error};

#[derive(Debug)]
pub struct SharedMemory {
    memfd: OwnedFd,
    pub mmap: memmap::MmapMut,
}

impl SharedMemory {
    pub fn new(size: u64) -> Result<Self, Error> {
        let memfd = nix::sys::memfd::memfd_create(
            c"glycin-frame",
            nix::sys::memfd::MFdFlags::MFD_CLOEXEC | nix::sys::memfd::MFdFlags::MFD_ALLOW_SEALING,
        )
        .expect("Failed to create memfd");

        nix::unistd::ftruncate(&memfd, size.try_into().expect("Required memory too large"))
            .expect("Failed to set memfd size");

        let raw_fd = memfd.as_raw_fd();
        let mmap = unsafe { memmap::MmapMut::map_mut(raw_fd) }?;

        Ok(Self { mmap, memfd })
    }

    pub fn into_binary_data(self) -> BinaryData {
        BinaryData::from(self.memfd)
    }
}

impl SharedMemory {
    fn from_data(value: impl AsRef<[u8]>) -> Result<Self, Error> {
        let mut shared_memory = SharedMemory::new(u64::try_from(value.as_ref().len())?)?;

        shared_memory.copy_from_slice(value.as_ref());

        Ok(shared_memory)
    }
}

impl BinaryData {
    pub fn from_data(value: impl AsRef<[u8]>) -> Result<Self, Error> {
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
