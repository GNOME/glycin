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
pub mod editing;
pub mod error;
#[cfg(feature = "external")]
mod external_api;
#[cfg(feature = "image-rs")]
pub mod image_rs;
#[cfg(all(feature = "loader-utils", feature = "external"))]
pub mod instruction_handler;
mod memory;
pub mod safe_math;

pub use api::*;
#[cfg(feature = "builtin")]
pub use builtin::Builtin;
pub use error::*;
#[cfg(feature = "external")]
pub use external_api::*;
pub use glycin_common::{
    ExtendedMemoryFormat, MemoryFormat, MemoryFormatInfo, MemoryFormatSelection, Operation,
    Operations,
};
#[cfg(all(feature = "loader-utils", feature = "external"))]
pub use instruction_handler::*;
pub use memory::*;
