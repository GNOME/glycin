use std::ops::{Deref, DerefMut};
use std::os::fd::{AsRawFd, OwnedFd};

use log::warn;
use nix::fcntl;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zbus::zvariant;

use crate::{ByteData, FungibleMemory, MemoryAllocationError};

#[derive(Debug)]
pub struct SharedMemory {
    memfd: OwnedFd,
    mmap: Option<MMapOptions>,
}

#[derive(Debug)]
enum MMapOptions {
    Mutable(memmap::MmapMut),
    ReadOnly(memmap::Mmap),
}

impl Deref for MMapOptions {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Mutable(x) => x.deref(),
            Self::ReadOnly(x) => x.deref(),
        }
    }
}

impl DerefMut for MMapOptions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Mutable(x) => x.deref_mut(),
            Self::ReadOnly(_) => {
                panic!("Shared memory has been sealed and can't be written to anymore.")
            }
        }
    }
}

impl zvariant::Type for SharedMemory {
    const SIGNATURE: &'static zvariant::Signature = &zvariant::Signature::Fd;
}

impl Serialize for SharedMemory {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        zvariant::OwnedFd::from(self.memfd.try_clone().unwrap()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SharedMemory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let memfd = zvariant::OwnedFd::deserialize(deserializer)?.into();

        Ok(Self { memfd, mmap: None })
    }
}

impl Default for SharedMemory {
    fn default() -> Self {
        todo!()
    }
}

impl ByteData for SharedMemory {
    fn new(size: u64) -> std::io::Result<Self> {
        let (memfd, mmap) = Self::new_memfd(size)?;

        Ok(Self {
            memfd,
            mmap: Some(MMapOptions::Mutable(mmap)),
        })
    }

    fn into_fungible(self) -> FungibleMemory {
        FungibleMemory::SharedMemory(self)
    }

    fn from_shared(shared: SharedMemory) -> Self {
        shared
    }

    fn into_other<O: ByteData>(self) -> Result<O, MemoryAllocationError> {
        Ok(O::from_shared(self))
    }

    fn try_from_vec(value: Vec<u8>) -> Result<Self, MemoryAllocationError> {
        Self::try_from_slice(&value)
    }

    fn try_from_slice(value: &[u8]) -> Result<Self, MemoryAllocationError> {
        let (memfd, mut mmap) = Self::new_memfd(
            u64::try_from(value.len()).map_err(|err| MemoryAllocationError(err.to_string()))?,
        )
        .map_err(|err| MemoryAllocationError(err.to_string()))?;

        mmap.copy_from_slice(value.as_ref());

        Ok(Self {
            memfd,
            mmap: Some(MMapOptions::Mutable(mmap)),
        })
    }

    async fn initial_seal(&mut self) -> Result<(), MemoryAllocationError> {
        if self.mmap.is_some() {
            warn!("SharedMemory already got inital seal.");
            return Ok(());
        }

        self.seal(fcntl::SealFlag::F_SEAL_GROW | fcntl::SealFlag::F_SEAL_SHRINK)
            .await?;

        self.add_mut_memmap()?;

        Ok(())
    }

    async fn final_seal(&mut self) -> Result<(), MemoryAllocationError> {
        self.mmap = None;

        self.seal(
            fcntl::SealFlag::F_SEAL_GROW
                | fcntl::SealFlag::F_SEAL_SHRINK
                | fcntl::SealFlag::F_SEAL_WRITE
                | fcntl::SealFlag::F_SEAL_SEAL,
        )
        .await?;

        self.add_memmap()?;

        Ok(())
    }

    #[cfg(feature = "glib")]
    fn into_gbytes(self) -> Result<glib::Bytes, MemoryAllocationError> {
        if !matches!(self.mmap, Some(MMapOptions::ReadOnly(_))) {
            panic!("SharedMemory is lacking final seal.");
        }

        use std::os::fd::RawFd;

        pub unsafe fn gbytes_from_mmap(
            raw_fd: RawFd,
        ) -> Result<glib::Bytes, MemoryAllocationError> {
            unsafe {
                let mut error = std::ptr::null_mut();

                let mapped_file =
                    glib::ffi::g_mapped_file_new_from_fd(raw_fd, glib::ffi::GFALSE, &mut error);

                if !error.is_null() {
                    let err: glib::Error = glib::translate::from_glib_full(error);
                    return Err(MemoryAllocationError(err.to_string()));
                };

                let bytes = glib::translate::from_glib_full(glib::ffi::g_mapped_file_get_bytes(
                    mapped_file,
                ));

                glib::ffi::g_mapped_file_unref(mapped_file);

                Ok(bytes)
            }
        }

        unsafe { gbytes_from_mmap(self.memfd.as_raw_fd()) }
    }
}

impl SharedMemory {
    fn new_memfd(size: u64) -> std::io::Result<(OwnedFd, memmap::MmapMut)> {
        let memfd = nix::sys::memfd::memfd_create(
            c"glycin-frame",
            nix::sys::memfd::MFdFlags::MFD_CLOEXEC | nix::sys::memfd::MFdFlags::MFD_ALLOW_SEALING,
        )?;

        nix::unistd::ftruncate(&memfd, size.try_into().expect("Required memory too large"))?;

        let raw_fd = memfd.as_raw_fd();
        let mmap = unsafe { memmap::MmapMut::map_mut(raw_fd) }?;

        Ok((memfd, mmap))
    }

    fn add_mut_memmap(&mut self) -> Result<(), MemoryAllocationError> {
        let mmap: memmap::MmapMut = unsafe { memmap::MmapMut::map_mut(&self.memfd) }
            .map_err(|err| MemoryAllocationError(err.to_string()))?;

        self.mmap = Some(MMapOptions::Mutable(mmap));

        Ok(())
    }

    fn add_memmap(&mut self) -> Result<(), MemoryAllocationError> {
        let mmap: memmap::Mmap = unsafe { memmap::Mmap::map(&self.memfd) }
            .map_err(|err| MemoryAllocationError(err.to_string()))?;

        self.mmap = Some(MMapOptions::ReadOnly(mmap));

        Ok(())
    }

    async fn seal(&self, seals: fcntl::SealFlag) -> Result<(), MemoryAllocationError> {
        let start = std::time::Instant::now();

        // Sealing returns a ResourceBusy for SealWrite until no readable maps exist
        // anymore. Practically, we are waiting for the loader to close it's
        // mmap to the memfd.
        loop {
            // 🦭
            let seal = fcntl::fcntl(&self.memfd, fcntl::FcntlArg::F_ADD_SEALS(seals));

            match seal {
                Ok(_) => break,
                Err(err) if start.elapsed() > std::time::Duration::from_secs(10) => {
                    // Give up after some time and return the error
                    return Err(MemoryAllocationError(err.to_string()));
                }
                Err(_) => {
                    // Try again after short waiting time
                    futures_timer::Delay::new(std::time::Duration::from_millis(1)).await
                }
            }
        }

        Ok(())
    }
}

impl Deref for SharedMemory {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        self.mmap
            .as_ref()
            .expect("SharedMemory haven't been sealed before use.")
    }
}

impl DerefMut for SharedMemory {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        self.mmap
            .as_mut()
            .expect("SharedMemory haven't been sealed before use.")
    }
}
