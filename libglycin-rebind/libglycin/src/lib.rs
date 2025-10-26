#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::needless_doctest_main)]
//! # Rust Glycin bindings
//!
//! This library contains safe Rust bindings for [Glycin](https://gitlab.gnome.org/GNOME/glycin).

// Re-export the -sys bindings
pub use {ffi, gio, glib};

macro_rules! assert_initialized_main_thread {
    () => {};
}

macro_rules! skip_assert_initialized {
    () => {};
}

#[allow(unused_imports)]
#[allow(clippy::let_and_return)]
#[allow(clippy::type_complexity)]
mod auto;

pub use auto::*;
