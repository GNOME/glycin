#![deny(clippy::arithmetic_side_effects)]
#![deny(clippy::cast_possible_truncation)]
#![deny(clippy::cast_possible_wrap)]

//! Utilities for building glycin decoders

#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[cfg(all(not(feature = "async-io"), not(feature = "tokio")))]
mod error_message {
    compile_error!(
        "\"async-io\" (default) or \"tokio\" must be enabled to provide an async runtime for zbus."
    );
}

pub mod dbus;
pub mod dbus_editor_api;
pub mod dbus_loader_api;
pub mod editing;
pub mod error;
#[cfg(feature = "image-rs")]
pub mod image_rs;
#[cfg(feature = "loader-utils")]
pub mod instruction_handler;
pub mod memory_format;
pub mod save_math;
pub mod shared_memory;

#[cfg(feature = "loader-utils")]
#[doc(no_inline)]
pub use std::os::unix::net::UnixStream;

pub mod operations;

pub use dbus::*;
pub use dbus_editor_api::*;
pub use dbus_loader_api::*;
pub use error::*;
#[cfg(feature = "loader-utils")]
pub use instruction_handler::*;
pub use memory_format::MemoryFormat;
pub use save_math::*;
pub use shared_memory::*;
