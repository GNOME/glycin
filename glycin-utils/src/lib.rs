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

mod dbus_editor_api;
mod dbus_loader_api;
mod dbus_types;
pub mod editing;
pub mod error;
#[cfg(feature = "image-rs")]
pub mod image_rs;
mod img_buf;
#[cfg(feature = "loader-utils")]
pub mod instruction_handler;
pub mod safe_math;

#[cfg(feature = "loader-utils")]
#[doc(no_inline)]
pub use std::os::unix::net::UnixStream;

pub use dbus_editor_api::*;
pub use dbus_loader_api::*;
pub use dbus_types::*;
pub use error::*;
pub use glycin_common::shared_memory::SharedMemory;
pub use glycin_common::{
    BinaryData, ExtendedMemoryFormat, MemoryFormat, MemoryFormatInfo, MemoryFormatSelection,
    Operation, Operations,
};
pub use img_buf::ImgBuf;
#[cfg(feature = "loader-utils")]
pub use instruction_handler::*;
