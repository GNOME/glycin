use gio::prelude::*;
use glib::ffi::GType;
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject;

pub type GlyFrameDetails =
    <gobject::frame_details::imp::GlyFrameDetails as ObjectSubclass>::Instance;

#[unsafe(no_mangle)]
pub extern "C" fn gly_frame_details_get_type() -> GType {
    <gobject::GlyFrameDetails as StaticType>::static_type().into_glib()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gly_frame_details_get_pixel_density(
    frame: *mut GlyFrameDetails,
) -> *mut crate::GlyPixelDensity {
    unsafe {
        let frame = gobject::GlyFrameDetails::from_glib_ptr_borrow(&frame);
        frame.pixel_density().into_glib_ptr()
    }
}
