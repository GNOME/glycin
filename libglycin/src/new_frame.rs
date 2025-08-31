use gio::prelude::*;
use glib::ffi::{GBytes, GType};
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject;

pub type GlyNewFrame = <gobject::new_frame::imp::GlyNewFrame as ObjectSubclass>::Instance;

#[no_mangle]
pub extern "C" fn gly_new_frame_get_type() -> GType {
    <gobject::GlyNewFrame as StaticType>::static_type().into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_new_frame_set_color_icc_profile(
    new_frame: *mut GlyNewFrame,
    icc_profile: *mut GBytes,
) -> glib::ffi::gboolean {
    let new_frame = gobject::GlyNewFrame::from_glib_ptr_borrow(&new_frame);

    if icc_profile.is_null() {
        new_frame
            .new_frame()
            .set_color_icc_profile(None)
            .is_ok()
            .into_glib()
    } else {
        let icc_profile = glib::Bytes::from_glib_ptr_borrow(&icc_profile);

        new_frame
            .new_frame()
            .set_color_icc_profile(Some(icc_profile.to_vec()))
            .is_ok()
            .into_glib()
    }
}
