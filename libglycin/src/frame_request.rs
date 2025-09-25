use std::ffi::c_int;

use gio::prelude::*;
use glib::ffi::GType;
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject;

pub type GlyFrameRequest =
    <gobject::frame_request::imp::GlyFrameRequest as ObjectSubclass>::Instance;

#[no_mangle]
pub extern "C" fn gly_frame_request_get_type() -> GType {
    <gobject::GlyFrameRequest as StaticType>::static_type().into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_request_new() -> *mut GlyFrameRequest {
    gobject::GlyFrameRequest::new().into_glib_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_request_set_scale(
    frame_request: *mut GlyFrameRequest,
    width: u32,
    height: u32,
) {
    let frame_request = gobject::GlyFrameRequest::from_glib_ptr_borrow(&frame_request);
    frame_request.set_scale(width, height);
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_request_set_loop_animation(
    frame_request: *mut GlyFrameRequest,
    loop_animation: c_int,
) {
    let frame_request = gobject::GlyFrameRequest::from_glib_ptr_borrow(&frame_request);
    frame_request.set_loop_animation(loop_animation != 0);
}
