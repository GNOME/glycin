use gio::prelude::*;
use glib::ffi::GType;
use glib::translate::*;
pub use glycin::gobject::GlyColorMode;

#[unsafe(no_mangle)]
pub extern "C" fn gly_color_mode_get_type() -> GType {
    <GlyColorMode as StaticType>::static_type().into_glib()
}
