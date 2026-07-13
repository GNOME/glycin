use gio::prelude::*;
use glib::ffi::{GBytes, GType};
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject;
use glycin::gobject::GlyPhysicalDimensionUnit;

use crate::GlyPixelDensity;

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
    pixel_density: *mut GlyPixelDensity,
) -> bool {
    unsafe {
        let new_frame = gobject::GlyNewFrame::from_glib_ptr_borrow(&new_frame);
        let pixel_density =
            from_glib_borrow::<_, Option<gobject::GlyPixelDensity>>(pixel_density).to_owned();

        new_frame.set_pixel_density(pixel_density);

        true
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gly_physical_dimension_unit_get_type() -> GType {
    <GlyPhysicalDimensionUnit as StaticType>::static_type().into_glib()
}
