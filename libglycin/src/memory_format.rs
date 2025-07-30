use gio::prelude::*;
use glib::ffi::GType;
use glib::translate::*;
pub use glycin::MemoryFormat as GlyMemoryFormat;

#[no_mangle]
pub extern "C" fn gly_memory_format_get_type() -> GType {
    <GlyMemoryFormat as StaticType>::static_type().into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_memory_format_has_alpha(memory_format: i32) -> glib::ffi::gboolean {
    let format = glycin::MemoryFormat::try_from(memory_format).unwrap();
    format.has_alpha().into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_memory_format_is_premultiplied(
    memory_format: i32,
) -> glib::ffi::gboolean {
    let format = glycin::MemoryFormat::try_from(memory_format).unwrap();
    format.is_premultiplied().into_glib()
}
