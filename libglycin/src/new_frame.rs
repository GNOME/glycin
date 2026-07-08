use gio::prelude::*;
use glib::ffi::{GBytes, GType};
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject;
use glycin::gobject::new_frame::GlyPhysicalDimensionUnit;

pub type GlyNewFrame = <gobject::new_frame::imp::GlyNewFrame as ObjectSubclass>::Instance;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gly_new_frame_get_type() -> GType {
    <gobject::GlyNewFrame as StaticType>::static_type().into_glib()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gly_new_frame_set_color_icc_profile(
    new_frame: *mut GlyNewFrame,
    icc_profile: *mut GBytes,
) -> glib::ffi::gboolean {
    unsafe {
        let new_frame = gobject::GlyNewFrame::from_glib_ptr_borrow(&new_frame);

        if icc_profile.is_null() {
            new_frame.set_color_icc_profile(None::<&glib::Bytes>);

            true.into_glib()
        } else {
            let icc_profile = glib::Bytes::from_glib_ptr_borrow(&icc_profile);

            new_frame.set_color_icc_profile(Some(icc_profile.clone()));

            true.into_glib()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gly_new_frame_set_pixel_density(
    new_frame: *mut GlyNewFrame,
    x_density: f64,
    x_unit: i32,
    y_density: f64,
    y_unit: i32,
) {
    unsafe {
        let new_frame = gobject::GlyNewFrame::from_glib_ptr_borrow(&new_frame);

        if x_unit == 0 || y_unit == 0 {
            new_frame.set_pixel_density(None);
        } else {
            let x_unit = GlyPhysicalDimensionUnit::from_glib(x_unit);
            let y_unit = GlyPhysicalDimensionUnit::from_glib(y_unit);

            new_frame.set_pixel_density(Some((x_density, x_unit, y_density, y_unit)));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gly_physical_dimension_unit_get_type() -> GType {
    <GlyPhysicalDimensionUnit as StaticType>::static_type().into_glib()
}
