use gdk::ffi::GdkTexture;
use gdk::glib;
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject;

pub type GlyFrame = <gobject::frame::imp::GlyFrame as ObjectSubclass>::Instance;

extern "C" {
    pub fn gly_frame_get_width(frame: *mut GlyFrame) -> u32;
    pub fn gly_frame_get_height(frame: *mut GlyFrame) -> u32;
    pub fn gly_frame_get_memory_format(frame: *mut GlyFrame) -> i32;
    pub fn gly_frame_get_stride(frame: *mut GlyFrame) -> u32;
    pub fn gly_frame_get_buf_bytes(frame: *mut GlyFrame) -> *mut glib::ffi::GBytes;
}

#[no_mangle]
pub unsafe extern "C" fn gly_gtk_frame_get_texture(frame: *mut GlyFrame) -> *mut GdkTexture {
    let width = gly_frame_get_width(frame) as i32;
    let height = gly_frame_get_height(frame) as i32;
    let bytes = gly_frame_get_buf_bytes(frame);
    let stride = gly_frame_get_stride(frame) as usize;

    let gly_format = glycin::MemoryFormat::try_from(gly_frame_get_memory_format(frame)).unwrap();
    let gdk_format = glycin::gdk_memory_format(gly_format).into_glib();

    gdk::ffi::gdk_memory_texture_new(width, height, gdk_format, bytes, stride) as *mut GdkTexture
}
