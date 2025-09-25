use gio::prelude::*;
use glib::error::ErrorDomain;
use glib::ffi::GError;
use glib::translate::*;

#[derive(Debug, Copy, Clone, glib::Enum, glib::ErrorDomain)]
#[error_domain(name = "gly-loader-error")]
#[repr(C)]
#[enum_type(name = "GlyLoaderError")]
#[non_exhaustive]
pub enum GlyLoaderError {
    Failed = 0,
    UnknownImageFormat = 1,
    NoMoreFrames = 2,
}

impl From<&glycin::Error> for GlyLoaderError {
    fn from(value: &glycin::Error) -> Self {
        if value.is_no_more_frames() {
            Self::NoMoreFrames
        } else if value.unsupported_format().is_some() {
            Self::UnknownImageFormat
        } else {
            Self::Failed
        }
    }
}

#[no_mangle]
pub extern "C" fn gly_loader_error_quark() -> glib::ffi::GQuark {
    GlyLoaderError::domain().into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_loader_error_get_type() -> glib::ffi::GType {
    GlyLoaderError::static_type().into_glib()
}

pub unsafe fn set_context_error(g_error: *mut *mut GError, err: &glycin::ErrorCtx) {
    if !g_error.is_null() {
        *g_error = glib_context_error(err).into_glib_ptr();
    }
}

pub unsafe fn set_error(g_error: *mut *mut GError, err: &glycin::Error) {
    if !g_error.is_null() {
        *g_error = glib_error(err).into_glib_ptr();
    }
}

pub fn glib_context_error(err: &glycin::ErrorCtx) -> glib::Error {
    let gly_error: GlyLoaderError = err.error().into();
    glib::Error::new(gly_error, &err.to_string())
}

pub fn glib_error(err: &glycin::Error) -> glib::Error {
    let gly_error: GlyLoaderError = err.into();
    glib::Error::new(gly_error, &err.to_string())
}
