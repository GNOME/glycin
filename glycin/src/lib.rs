#![deny(clippy::arithmetic_side_effects)]
#![deny(clippy::cast_possible_truncation)]
#![deny(clippy::cast_possible_wrap)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::expect_used)]
#![deny(clippy::unwrap_used)]

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
//! # use glycin::*;
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
//! Glycin requires the libraries *libglib2.0*, *liblcms2*, *libfontconfig*, and
//! *libseccomp* packages to be installed. For the `gdk4` feature, *libgtk-4* is
//! required as well. To actually work with images, loaders for the respective
//! formats have to be installed. Glycin provides [loaders] for many formats
//! that are packaged with many distributions. When working in the default
//! sandbox mode, the `bwrap` binary of *bubblewrap* is required as well. The
//! required dependencies can usually be installed through commands like
//!
//! ```sh
//! $ apt install libgtk-4-dev liblcms2-dev libfontconfig-dev libseccomp-dev glycin-loaders bubblewrap
//! ```
//!
//! on Debian/Ubuntu or
//!
//! ```sh
//! $ dnf install gtk4-devel lcms2-devel fontconfig-devel libseccomp-devel glycin-loaders bubblewrap
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

#[cfg(all(not(feature = "async-io"), not(feature = "tokio")))]
mod error_message {
    compile_error!(
        "\"async-io\" (default) or \"tokio\" must be enabled to provide an async runtime."
    );
}

mod api_common;
mod api_creator;
mod api_editor;
mod api_loader;
#[cfg(feature = "unstable-config")]
pub mod config;
#[cfg(not(feature = "unstable-config"))]
mod config;
mod dbus;
mod error;
mod fontconfig;
mod icc;
mod orientation;
mod pool;
mod sandbox;
mod util;

#[cfg(feature = "gobject")]
pub mod gobject;

pub use api_common::*;
pub use api_creator::*;
pub use api_editor::*;
pub use api_loader::*;
pub use config::COMPAT_VERSION;
pub use error::{Error, ErrorCtx};
pub use glycin_common::{
    BinaryData, MemoryFormat, MemoryFormatSelection, Operation, OperationId, Operations,
};
pub use gufo_common::cicp::Cicp;
pub use pool::{Pool, PoolConfig};
#[cfg(feature = "gdk4")]
pub use util::gdk_memory_format;
