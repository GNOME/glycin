use std::ffi::c_char;

use gio::prelude::*;
use glib::ffi::{GBytes, GType};
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject;

pub type GlyNewImage = <gobject::new_image::imp::GlyNewImage as ObjectSubclass>::Instance;

#[no_mangle]
pub extern "C" fn gly_new_image_get_type() -> GType {
    <gobject::GlyNewImage as StaticType>::static_type().into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_new_image_new(
    width: u32,
    height: u32,
    memory_format: i32,
    data: *mut GBytes,
) -> *mut GlyNewImage {
    let memory_format = glycin::MemoryFormat::try_from(memory_format).unwrap();
    let data = glib::Bytes::from_glib_ptr_borrow(&data).clone();
    gobject::GlyNewImage::new(width, height, memory_format, data).into_glib_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn gly_new_image_add_metadata_key_value(
    new_image: *mut GlyNewImage,
    key: *const c_char,
    value: *const c_char,
) {
    let key = glib::GStr::from_ptr_checked(key).unwrap().as_str();
    let value = glib::GStr::from_ptr_checked(value).unwrap().as_str();
    let new_image = gobject::GlyNewImage::from_glib_ptr_borrow(&new_image);

    new_image.add_key_value(key.to_string(), value.to_string());
}
