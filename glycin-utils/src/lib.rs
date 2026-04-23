//! Utilities for building glycin decoders

#[cfg(all(not(feature = "async-io"), not(feature = "tokio")))]
mod error_message {
    compile_error!(
        "\"async-io\" (default) or \"tokio\" must be enabled to provide an async runtime for zbus."
    );
}

mod api;
#[cfg(feature = "builtin")]
mod builtin;
#[cfg(feature = "external")]
mod dbus_editor_api;
#[cfg(feature = "external")]
mod dbus_loader_api;
pub mod editing;
pub mod error;
#[cfg(feature = "image-rs")]
pub mod image_rs;
//mod img_buf;
#[cfg(all(feature = "loader-utils", feature = "external"))]
pub mod instruction_handler;
mod memory;
pub mod safe_math;

use std::panic::UnwindSafe;

//pub use img_buf::ImgBuf;
pub use api::*;
#[cfg(feature = "builtin")]
pub use builtin::Builtin;
#[cfg(feature = "external")]
pub use dbus_editor_api::*;
#[cfg(feature = "external")]
pub use dbus_loader_api::*;
pub use error::*;
pub use glycin_common::{
    ExtendedMemoryFormat, MemoryFormat, MemoryFormatInfo, MemoryFormatSelection, Operation,
    Operations,
};
#[cfg(all(feature = "loader-utils", feature = "external"))]
pub use instruction_handler::*;
pub use memory::*;

fn catch_unwind<R, F: FnOnce() -> R + UnwindSafe>(f: F) -> Result<R, RemoteError> {
    std::panic::catch_unwind(f).map_err(|_| RemoteError::Panic)
}
