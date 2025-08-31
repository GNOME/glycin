use std::num::TryFromIntError;
use std::sync::Arc;

#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Remote error: {0}")]
    Io(Arc<std::io::Error>),
    #[error("OutOfMemory")]
    OutOfMemory,
    #[error("TryFromIntError")]
    TryFromIntError(#[from] TryFromIntError),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        if err.kind() == std::io::ErrorKind::OutOfMemory {
            Self::OutOfMemory
        } else {
            Self::Io(Arc::new(err))
        }
    }
}
