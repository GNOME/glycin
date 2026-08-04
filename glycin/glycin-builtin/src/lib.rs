//! The `glycin-builtin` crate just exports the [glycin-core](glycin-core) crate
//! with the `builtin` feature enabled. You usually want to use the
//! [`glycin`](glycin) crate which automatically selects the right compilation
//! method (builtin/external) for the target OS.
//!
//! You can use this crate if you want to explicitly use builtin  loaders.

pub use glycin_core::*;
