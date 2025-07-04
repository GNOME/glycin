use std::ops::Deref;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(zvariant::Type, Debug, Clone)]
#[zvariant(signature = "h")]
pub struct BinaryData {
    pub(crate) memfd: Arc<zvariant::OwnedFd>,
}

impl Serialize for BinaryData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.memfd.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BinaryData {
    fn deserialize<D>(deserializer: D) -> Result<BinaryData, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self {
            memfd: Arc::new(zvariant::OwnedFd::deserialize(deserializer)?),
        })
    }
}

impl AsRawFd for BinaryData {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.memfd.as_raw_fd()
    }
}

impl AsRawFd for &BinaryData {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.memfd.as_raw_fd()
    }
}

impl From<OwnedFd> for BinaryData {
    fn from(value: OwnedFd) -> Self {
        let owned_fd = zvariant::OwnedFd::from(value);
        BinaryData {
            memfd: Arc::new(owned_fd),
        }
    }
}

impl BinaryData {
    /// Get a copy of the binary data
    pub fn get_full(&self) -> std::io::Result<Vec<u8>> {
        Ok(self.get()?.to_vec())
    }

    /// Get a reference to the binary data
    pub fn get(&self) -> std::io::Result<BinaryDataRef> {
        Ok(BinaryDataRef {
            mmap: { unsafe { memmap::MmapOptions::new().map_copy_read_only(&self.memfd)? } },
        })
    }
}

#[derive(Debug)]
pub struct BinaryDataRef {
    mmap: memmap::Mmap,
}

impl Deref for BinaryDataRef {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.mmap.deref()
    }
}

impl AsRef<[u8]> for BinaryDataRef {
    fn as_ref(&self) -> &[u8] {
        self.mmap.deref()
    }
}
