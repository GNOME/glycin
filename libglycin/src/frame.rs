use gio::glib;
use gio::prelude::*;
use glib::ffi::GType;
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject;
pub use glycin::MemoryFormat as GlyMemoryFormat;

pub type GlyFrame = <gobject::frame::imp::GlyFrame as ObjectSubclass>::Instance;

#[no_mangle]
pub extern "C" fn gly_frame_get_type() -> GType {
    <gobject::GlyFrame as StaticType>::static_type().into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_get_delay(frame: *mut GlyFrame) -> i64 {
    let frame = gobject::GlyFrame::from_glib_ptr_borrow(&(frame as *const _));
    frame.frame().delay().unwrap_or_default().as_micros() as i64
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_get_width(frame: *mut GlyFrame) -> u32 {
    let frame = gobject::GlyFrame::from_glib_ptr_borrow(&(frame as *const _));
    frame.frame().width()
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_get_height(frame: *mut GlyFrame) -> u32 {
    let frame = gobject::GlyFrame::from_glib_ptr_borrow(&(frame as *const _));
    frame.frame().height()
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_get_stride(frame: *mut GlyFrame) -> u32 {
    let frame = gobject::GlyFrame::from_glib_ptr_borrow(&(frame as *const _));
    frame.frame().stride()
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_get_buf_bytes(frame: *mut GlyFrame) -> *mut glib::ffi::GBytes {
    let frame = gobject::GlyFrame::from_glib_ptr_borrow(&(frame as *const _));
    frame.frame().buf_bytes().to_glib_none().0
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_get_memory_format(frame: *mut GlyFrame) -> i32 {
    let frame = gobject::GlyFrame::from_glib_ptr_borrow(&(frame as *const _));
    frame.frame().memory_format().into_glib()
}

#[no_mangle]
pub extern "C" fn gly_memory_format_get_type() -> GType {
    <GlyMemoryFormat as StaticType>::static_type().into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_memory_format_has_alpha(memory_format: i32) -> bool {
    let format = glycin::MemoryFormat::try_from(memory_format).unwrap();
    format.has_alpha()
}

#[no_mangle]
pub unsafe extern "C" fn gly_memory_format_is_premultiplied(memory_format: i32) -> bool {
    let format = glycin::MemoryFormat::try_from(memory_format).unwrap();
    format.is_premultiplied()
}
