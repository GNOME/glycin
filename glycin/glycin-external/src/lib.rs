//! The `glycin-external` crate just exports the [glycin-core](glycin-core)
//! crate with the `external` feature enabled. You usually want to use the
//! [`glycin`](glycin) crate which automatically selects the right compilation
//! method (builtin/external) for the target OS.
//!
//! You can use this crate if you want to explicitly use external loaders, which
//! will only work on Linux.

pub use glycin_core::*;
