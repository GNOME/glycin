pub enum ImgBuf {
    MMap(memmap::MmapMut),
    Vec(Vec<u8>),
}

impl ImgBuf {
    pub fn as_slice(&self) -> &[u8] {
        match self {
            Self::MMap(mmap) => mmap.as_ref(),
            Self::Vec(v) => v.as_slice(),
        }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        match self {
            Self::MMap(mmap) => mmap.as_mut(),
            Self::Vec(v) => v.as_mut_slice(),
        }
    }

    pub fn into_vec(self) -> Vec<u8> {
        match self {
            Self::Vec(vec) => vec,
            Self::MMap(_) => self.to_vec(),
        }
    }
}

impl std::ops::Deref for ImgBuf {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl std::ops::DerefMut for ImgBuf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}
