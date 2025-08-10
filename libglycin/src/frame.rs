use gio::prelude::*;
use glib::ffi::GType;
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject::{self, GlyCicp};

pub type GlyFrame = <gobject::frame::imp::GlyFrame as ObjectSubclass>::Instance;

#[no_mangle]
pub extern "C" fn gly_frame_get_type() -> GType {
    <gobject::GlyFrame as StaticType>::static_type().into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_get_delay(frame: *mut GlyFrame) -> i64 {
    let frame = gobject::GlyFrame::from_glib_ptr_borrow(&frame);
    frame.frame().delay().unwrap_or_default().as_micros() as i64
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_get_width(frame: *mut GlyFrame) -> u32 {
    let frame = gobject::GlyFrame::from_glib_ptr_borrow(&frame);
    frame.frame().width()
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_get_height(frame: *mut GlyFrame) -> u32 {
    let frame = gobject::GlyFrame::from_glib_ptr_borrow(&frame);
    frame.frame().height()
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_get_stride(frame: *mut GlyFrame) -> u32 {
    let frame = gobject::GlyFrame::from_glib_ptr_borrow(&frame);
    frame.frame().stride()
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_get_buf_bytes(frame: *mut GlyFrame) -> *mut glib::ffi::GBytes {
    let frame = gobject::GlyFrame::from_glib_ptr_borrow(&frame);
    frame.frame().buf_bytes().to_glib_none().0
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_get_memory_format(frame: *mut GlyFrame) -> i32 {
    let frame = gobject::GlyFrame::from_glib_ptr_borrow(&frame);
    frame.frame().memory_format().into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_frame_get_color_cicp(frame: *mut GlyFrame) -> *const GlyCicp {
    let frame = gobject::GlyFrame::from_glib_ptr_borrow(&frame);
    match frame.color_cicp() {
        Some(cicp) => gobject::GlyCicp {
            color_primaries: cicp.color_primaries.into(),
            transfer_characteristics: cicp.transfer_characteristics.into(),
            matrix_coefficients: cicp.matrix_coefficients.into(),
            video_full_range_flag: cicp.video_full_range_flag.into(),
        }
        .into_glib_ptr(),
        None => std::ptr::null(),
    }
}

#[no_mangle]
pub extern "C" fn gly_cicp_get_type() -> GType {
    <GlyCicp as StaticType>::static_type().into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_cicp_copy(cicp: *mut GlyCicp) -> *mut GlyCicp {
    GlyCicp::from_glib_none(cicp).clone().into_glib_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn gly_cicp_free(cicp: *mut GlyCicp) {
    drop(GlyCicp::from_glib_full(cicp));
}
