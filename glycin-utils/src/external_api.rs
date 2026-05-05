//! D-Bus API for external processors

mod editor;
mod loader;

use std::panic::UnwindSafe;

pub use editor::*;
pub use loader::*;

use crate::RemoteError;

fn catch_unwind<R, F: FnOnce() -> R + UnwindSafe>(f: F) -> Result<R, RemoteError> {
    std::panic::catch_unwind(f).map_err(|_| RemoteError::Panic)
}
