//! Temporary bindings until gdk4 crate supports them

use gdk::glib::translate::*;
use gdk::{ffi, glib, ColorState, MemoryFormat, Texture};

glib::wrapper! {
    #[doc(alias = "GdkMemoryTextureBuilder")]
    pub struct MemoryTextureBuilder(Object<ffi::GdkMemoryTextureBuilder, ffi::GdkMemoryTextureBuilderClass>);

    match fn {
        type_ => || ffi::gdk_memory_texture_builder_get_type(),
    }
}

impl MemoryTextureBuilder {
    pub fn new() -> MemoryTextureBuilder {
        unsafe { from_glib_full(ffi::gdk_memory_texture_builder_new()) }
    }

    pub fn build(&self) -> Texture {
        unsafe { from_glib_full(ffi::gdk_memory_texture_builder_build(self.to_glib_none().0)) }
    }

    pub fn set_bytes(&self, bytes: Option<&glib::Bytes>) {
        unsafe {
            ffi::gdk_memory_texture_builder_set_bytes(
                self.to_glib_none().0,
                bytes.to_glib_none().0,
            );
        }
    }

    pub fn set_color_state(&self, color_state: Option<&ColorState>) {
        unsafe {
            ffi::gdk_memory_texture_builder_set_color_state(
                self.to_glib_none().0,
                color_state.to_glib_none().0,
            );
        }
    }

    pub fn set_format(&self, format: MemoryFormat) {
        unsafe {
            ffi::gdk_memory_texture_builder_set_format(self.to_glib_none().0, format.into_glib());
        }
    }

    pub fn set_height(&self, height: i32) {
        unsafe {
            ffi::gdk_memory_texture_builder_set_height(self.to_glib_none().0, height);
        }
    }

    pub fn set_stride(&self, stride: usize) {
        unsafe {
            ffi::gdk_memory_texture_builder_set_stride(self.to_glib_none().0, stride);
        }
    }

    pub fn set_width(&self, width: i32) {
        unsafe {
            ffi::gdk_memory_texture_builder_set_width(self.to_glib_none().0, width);
        }
    }
}
