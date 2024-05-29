use std::ffi::c_char;
use std::ptr;

use gio::ffi::{GAsyncReadyCallback, GAsyncResult, GTask};
use gio::glib;
use gio::prelude::*;
use glib::ffi::{gpointer, GError, GType};
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject;

use crate::common::*;
use crate::*;

pub type GlyImage = <gobject::image::imp::GlyImage as ObjectSubclass>::Instance;

#[no_mangle]
pub extern "C" fn gly_image_get_type() -> GType {
    <gobject::GlyImage as StaticType>::static_type().into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_next_frame(
    image: *mut GlyImage,
    g_error: *mut *mut GError,
) -> *mut GlyFrame {
    let obj = gobject::GlyImage::from_glib_ptr_borrow(&(image as *const _));

    let result = async_io::block_on(obj.next_frame());

    match result {
        Ok(frame) => frame.into_glib_ptr(),
        Err(err) => {
            set_error(g_error, &err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_next_frame_async(
    image: *mut GlyImage,
    cancellable: *mut gio::ffi::GCancellable,
    callback: GAsyncReadyCallback,
    user_data: gpointer,
) {
    let obj = gobject::GlyImage::from_glib_none(image);
    let cancellable = (!cancellable.is_null())
        .then(|| gio::Cancellable::from_glib_ptr_borrow(&(cancellable as *const _)));
    let callback: GAsyncReadyCallbackSend = GAsyncReadyCallbackSend::new(callback, user_data);

    let cancel_signal = if let Some(cancellable) = cancellable {
        cancellable
            .connect_cancelled(glib::clone!(@weak obj => move |_| obj.cancellable().cancel()))
    } else {
        None
    };

    let closure = move |task: gio::Task<gobject::GlyFrame>, obj: Option<&gobject::GlyImage>| {
        if let (Some(cancel_signal), Some(cancellable)) = (cancel_signal, cancellable) {
            cancellable.disconnect_cancelled(cancel_signal);
        }

        let result = task.upcast_ref::<gio::AsyncResult>().as_ptr();
        callback.call(obj.unwrap(), result);
    };

    let task = gio::Task::new(Some(&obj), cancellable, closure);

    glib::MainContext::ref_thread_default().spawn_local(async move {
        let res = obj.next_frame().await.map_err(|x| glib_error(&x));
        task.return_result(res);
    });
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_next_frame_finish(
    _image: *mut GlyImage,
    res: *mut GAsyncResult,
    error: *mut *mut GError,
) -> *mut GlyFrame {
    let task = gio::Task::<gobject::GlyFrame>::from_glib_none(res as *mut GTask);

    match task.propagate() {
        Ok(frame) => frame.into_glib_ptr(),
        Err(e) => {
            if !error.is_null() {
                *error = e.into_glib_ptr();
            }
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_get_mime_type(image: *mut GlyImage) -> *const c_char {
    let image = gobject::GlyImage::from_glib_ptr_borrow(&(image as *const _));
    image.mime_type().as_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_get_width(image: *mut GlyImage) -> u32 {
    let image = gobject::GlyImage::from_glib_ptr_borrow(&(image as *const _));
    image.image_info().width
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_get_height(image: *mut GlyImage) -> u32 {
    let image = gobject::GlyImage::from_glib_ptr_borrow(&(image as *const _));
    image.image_info().height
}
