use gio::prelude::*;
use glib::ffi::GType;
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject::{self, GlyPhysicalDimensionUnit};

pub type GlyPixelDensity =
    <gobject::pixel_density::imp::GlyPixelDensity as ObjectSubclass>::Instance;

#[unsafe(no_mangle)]
pub extern "C" fn gly_pixel_density_get_type() -> GType {
    <gobject::GlyPixelDensity as StaticType>::static_type().into_glib()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gly_pixel_density_new(
    x_value: f64,
    x_unit: GlyPhysicalDimensionUnit,
    y_value: f64,
    y_unit: GlyPhysicalDimensionUnit,
) -> *mut GlyPixelDensity {
    gobject::GlyPixelDensity::for_values(x_value, x_unit, y_value, y_unit).into_glib_ptr()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gly_pixel_density_get_x_value(pixel_density: *mut GlyPixelDensity) -> f64 {
    unsafe {
        let pixel_density = gobject::GlyPixelDensity::from_glib_ptr_borrow(&pixel_density);
        pixel_density.x_value()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gly_pixel_density_get_x_unit(
    pixel_density: *mut GlyPixelDensity,
) -> GlyPhysicalDimensionUnit {
    unsafe {
        let pixel_density = gobject::GlyPixelDensity::from_glib_ptr_borrow(&pixel_density);
        pixel_density.x_unit()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gly_pixel_density_get_y_value(pixel_density: *mut GlyPixelDensity) -> f64 {
    unsafe {
        let pixel_density = gobject::GlyPixelDensity::from_glib_ptr_borrow(&pixel_density);
        pixel_density.y_value()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gly_pixel_density_get_y_unit(
    pixel_density: *mut GlyPixelDensity,
) -> GlyPhysicalDimensionUnit {
    unsafe {
        let pixel_density = gobject::GlyPixelDensity::from_glib_ptr_borrow(&pixel_density);
        pixel_density.y_unit()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gly_pixel_density_convert(
    pixel_density: *mut GlyPixelDensity,
    unit: i32,
) -> *mut GlyPixelDensity {
    unsafe {
        let pixel_density = gobject::GlyPixelDensity::from_glib_ptr_borrow(&pixel_density);
        pixel_density
            .convert(GlyPhysicalDimensionUnit::from_glib(unit))
            .into_glib_ptr()
    }
}
