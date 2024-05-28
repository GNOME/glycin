use gdk::ffi::GdkTexture;
use gdk::{gio, glib};
use gio::prelude::*;
use glib::ffi::GType;
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject;

pub type GlyFrame = <gobject::frame::imp::GlyFrame as ObjectSubclass>::Instance;

#[no_mangle]
pub extern "C" fn gly_frame_get_type() -> GType {
    <gobject::GlyFrame as StaticType>::static_type().into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_get_texture(frame: *mut GlyFrame) -> *mut GdkTexture {
    let frame = gobject::GlyFrame::from_glib_ptr_borrow(&(frame as *const _));
    frame.frame().texture().unwrap().into_glib_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_get_delay(frame: *mut GlyFrame) -> i64 {
    let frame = gobject::GlyFrame::from_glib_ptr_borrow(&(frame as *const _));
    frame.frame().delay().unwrap_or_default().as_micros() as i64
}
