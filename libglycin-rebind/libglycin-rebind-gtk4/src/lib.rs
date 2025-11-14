#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::needless_doctest_main)]
//! # Rust GlycinGtk4 bindings
//!
//! This library contains safe Rust bindings for [Glycin](https://gitlab.gnome.org/GNOME/glycin).

// Re-export the -sys bindings
pub use {ffi, gdk, gio, gly, gtk};

/// Asserts that this is the main thread and `gtk::init` has been called.
macro_rules! assert_initialized_main_thread {
    () => {
        if !::gtk::is_initialized_main_thread() {
            if ::gtk::is_initialized() {
                panic!("libadwaita may only be used from the main thread.");
            } else {
                panic!("Gtk has to be initialized before using libadwaita.");
            }
        }
    };
}

#[allow(unused_imports)]
#[allow(clippy::let_and_return)]
#[allow(clippy::type_complexity)]
mod auto;

pub use auto::functions::*;
