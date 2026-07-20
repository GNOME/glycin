//! Glycin allows to decode images into [`gdk::Texture`]s and to extract image
//! metadata. The decoding happens in sandboxed modular image loaders that have
//! to be provided as binaries. The [`glycin-utils`](glycin_utils) for more
//! details.
//!
//! # Example
//!
//! You need to enable the `gdk4` feature for this example to work.
//!
//! ```no_run
//! # use glycin_core::*;
//! # async {
//! let file = gio::File::for_path("image.jpg");
//! let image = Loader::new(file).load().await?;
//!
//! let height = image.details().height();
//! let texture = image.next_frame().await?.texture();
//! # Ok::<(), ErrorCtx>(()) };
//! ```
//!
//! You can pass the [`texture`](Frame#structfield.texture) of a [`Frame`] to
//! [`gtk4::Image::from_paintable()`] to display the image.
//!
//! # External Dependencies
//!
//! Glycin requires the libraries *libglib2.0*, *libfontconfig*, and
//! *libseccomp* packages to be installed. For the `gdk4` feature, *libgtk-4* is
//! required as well. To actually work with images, loaders for the respective
//! formats have to be installed. Glycin provides [loaders] for many formats
//! that are packaged with many distributions. When working in the default
//! sandbox mode, the `bwrap` binary of *bubblewrap* is required as well. The
//! required dependencies can usually be installed through commands like
//!
//! ```sh
//! $ apt install libgtk-4-dev libfontconfig-dev libseccomp-dev glycin-loaders bubblewrap
//! ```
//!
//! on Debian/Ubuntu or
//!
//! ```sh
//! $ dnf install gtk4-devel fontconfig-devel libseccomp-devel glycin-loaders bubblewrap
//! ```
//!
//! on Fedora.
//!
//! # Features
//!
//! - `gdk4` --- Enables interoperability with [`gdk4`](gdk) by enabling to get
//!   a [`gdk::Texture`] directly.
//! - `tokio` --- Makes glycin compatible with [`zbus`] using [`tokio`].
//!
//! [`gtk4::Image::from_paintable()`]: https://gtk-rs.org/gtk4-rs/git/docs/gtk4/struct.Image.html#method.from_paintable
//! [loaders]: https://gitlab.gnome.org/GNOME/glycin#supported-image-formats

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
pub use error::{Error, ErrorContext, ErrorKind};
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
