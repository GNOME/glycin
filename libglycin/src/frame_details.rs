use gio::prelude::*;
use glib::ffi::GType;
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject::{self, GlyPhysicalDimensionUnit};

pub type GlyFrameDetails =
    <gobject::frame_details::imp::GlyFrameDetails as ObjectSubclass>::Instance;

#[unsafe(no_mangle)]
pub extern "C" fn gly_frame_details_get_type() -> GType {
    <gobject::GlyFrameDetails as StaticType>::static_type().into_glib()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gly_frame_details_get_pixel_density_x(frame: *mut GlyFrameDetails) -> f64 {
    unsafe {
        let frame = gobject::GlyFrameDetails::from_glib_ptr_borrow(&frame);
        frame.pixel_density_x()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gly_frame_details_get_pixel_density_x_unit(
    frame: *mut GlyFrameDetails,
) -> GlyPhysicalDimensionUnit {
    unsafe {
        let frame = gobject::GlyFrameDetails::from_glib_ptr_borrow(&frame);
        frame.pixel_density_x_unit()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gly_frame_details_get_pixel_density_y(frame: *mut GlyFrameDetails) -> f64 {
    unsafe {
        let frame = gobject::GlyFrameDetails::from_glib_ptr_borrow(&frame);
        frame.pixel_density_y()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gly_frame_details_get_pixel_density_y_unit(
    frame: *mut GlyFrameDetails,
) -> GlyPhysicalDimensionUnit {
    unsafe {
        let frame = gobject::GlyFrameDetails::from_glib_ptr_borrow(&frame);
        frame.pixel_density_y_unit()
    }
}
