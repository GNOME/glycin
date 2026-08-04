//! The `glycin-core` crate contains the implementation for the
//! [`glycin`](glycin) crate, which you usually want to use. The
//! [`glycin`](glycin) crate contains the detailled documentation and
//! automatically selects the right compilation method for the target OS.
//!
//! You can use this crate if you want to explicitly combine builtin and
//! external loaders.

#![cfg_attr(
    feature = "builtin",
    allow(dead_code, unused_variables, unused_imports)
)]

#[cfg(all(not(feature = "async-io"), not(feature = "tokio")))]
mod error_message {
    compile_error!(
        "Feature 'async-io' (default) or 'tokio' must be enabled to provide an async runtime."
    );
}

#[cfg(all(feature = "async-io", feature = "tokio"))]
mod error_message {
    compile_error!(
        "Features 'async-io' (default) or and 'tokio' cannot be enabled at the same time."
    );
}

#[cfg(all(not(feature = "external"), not(feature = "builtin")))]
mod error_message {
    compile_error!(
        "Feature 'external' or 'builtin' must be enabled to provide a way to load images."
    );
}

#[cfg(all(
    feature = "builtin",
    not(any(feature = "builtin-image-rs", feature = "builtin-test"))
))]
compile_error!(
    "At least one builtin loader feature like 'builtin-image-rs' has to be enabled if 'builtin' is enabled."
);

mod api;
pub mod config;
#[cfg(feature = "external")]
mod dbus;
#[cfg(not(feature = "external"))]
mod dbus_shim;
mod error;
#[cfg(feature = "external")]
mod fontconfig;
mod icc;
mod main_context;
mod orientation;
#[cfg(feature = "external")]
mod pool;
#[cfg(not(feature = "external"))]
mod pool_shim;
#[cfg(feature = "external")]
mod sandbox;
mod source;
mod util;

#[cfg(feature = "gobject")]
pub mod gobject;

/// Max texture size 8 GB in bytes
pub(crate) const MAX_TEXTURE_SIZE: u64 = 8 * 10u64.pow(9);

pub const COMPAT_VERSION: u8 = 2;

pub use api::*;
#[cfg(not(feature = "external"))]
use dbus_shim as dbus;
pub use error::{Error, ErrorContext};
pub use glycin_common::{
    ColorProfilePreference, MemoryFormat, MemoryFormatSelection, Operation, OperationId, Operations,
};
pub use gufo_common::cicp::Cicp;
pub use main_context::MainContextSelector;
pub use pool::{Pool, PoolConfig};
#[cfg(not(feature = "external"))]
use pool_shim as pool;
#[cfg(feature = "gdk4")]
pub use util::gdk_memory_format;
