use gio::prelude::*;
use glib::ffi::{GBytes, GType};
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject;

pub type GlyEncodedImage =
    <gobject::encoded_image::imp::GlyEncodedImage as ObjectSubclass>::Instance;

#[no_mangle]
pub extern "C" fn gly_encoded_image_get_type() -> GType {
    <gobject::GlyEncodedImage as StaticType>::static_type().into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_encoded_image_get_data(
    encoded_image: *mut GlyEncodedImage,
) -> *mut GBytes {
    let encoded_image = gobject::GlyEncodedImage::from_glib_ptr_borrow(&encoded_image);
    encoded_image.data().into_glib_ptr()
}
