use std::ffi::c_char;
use std::ptr;

use gio::ffi::{GAsyncReadyCallback, GAsyncResult, GTask};
use gio::prelude::*;
use glib::ffi::{gpointer, GError, GStrv, GType};
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
    let frame_request = gobject::GlyFrameRequest::new();
    gly_image_get_specific_frame(image, frame_request.to_glib_none().0, g_error)
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_next_frame_async(
    image: *mut GlyImage,
    cancellable: *mut gio::ffi::GCancellable,
    callback: GAsyncReadyCallback,
    user_data: gpointer,
) {
    let frame_request = gobject::GlyFrameRequest::new();
    gly_image_get_specific_frame_async(
        image,
        frame_request.to_glib_none().0,
        cancellable,
        callback,
        user_data,
    );
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_next_frame_finish(
    image: *mut GlyImage,
    res: *mut GAsyncResult,
    error: *mut *mut GError,
) -> *mut GlyFrame {
    gly_image_get_specific_frame_finish(image, res, error)
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_get_specific_frame(
    image: *mut GlyImage,
    frame_request: *mut GlyFrameRequest,
    g_error: *mut *mut GError,
) -> *mut GlyFrame {
    let obj = gobject::GlyImage::from_glib_ptr_borrow(&image);
    let frame_request: glycin::FrameRequest =
        gobject::GlyFrameRequest::from_glib_ptr_borrow(&frame_request).frame_request();

    let result = async_global_executor::block_on(obj.specific_frame(frame_request));

    match result {
        Ok(frame) => frame.into_glib_ptr(),
        Err(err) => {
            set_context_error(g_error, &err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_get_specific_frame_async(
    image: *mut GlyImage,
    frame_request: *mut GlyFrameRequest,
    cancellable: *mut gio::ffi::GCancellable,
    callback: GAsyncReadyCallback,
    user_data: gpointer,
) {
    let obj = gobject::GlyImage::from_glib_none(image);
    let frame_request: glycin::FrameRequest =
        gobject::GlyFrameRequest::from_glib_ptr_borrow(&frame_request).frame_request();
    let cancellable: Option<gio::Cancellable> = from_glib_none(cancellable);
    let callback: GAsyncReadyCallbackSend = GAsyncReadyCallbackSend::new(callback, user_data);
    let cancel_signal = if let Some(cancellable) = &cancellable {
        cancellable.connect_cancelled(glib::clone!(
            #[weak]
            obj,
            move |_| obj.cancellable().cancel()
        ))
    } else {
        None
    };
    let cancellable_ = cancellable.clone();
    let closure = move |task: gio::Task<gobject::GlyFrame>, obj: Option<&gobject::GlyImage>| {
        if let (Some(cancel_signal), Some(cancellable)) = (cancel_signal, cancellable) {
            cancellable.disconnect_cancelled(cancel_signal);
        }

        let result = task.upcast_ref::<gio::AsyncResult>().as_ptr();
        callback.call(obj.unwrap(), result);
    };
    let task = gio::Task::new(Some(&obj), cancellable_.as_ref(), closure);
    async_global_executor::spawn(async move {
        let res = obj
            .specific_frame(frame_request)
            .await
            .map_err(|x| glib_context_error(&x));
        task.return_result(res);
    })
    .detach();
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_get_specific_frame_finish(
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
    let image = gobject::GlyImage::from_glib_ptr_borrow(&image);
    image.mime_type().as_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_get_width(image: *mut GlyImage) -> u32 {
    let image = gobject::GlyImage::from_glib_ptr_borrow(&image);
    image.image_info().width()
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_get_height(image: *mut GlyImage) -> u32 {
    let image = gobject::GlyImage::from_glib_ptr_borrow(&image);
    image.image_info().height()
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_get_metadata_key_value(
    image: *mut GlyImage,
    key: *const c_char,
) -> *const c_char {
    let image = gobject::GlyImage::from_glib_ptr_borrow(&image);
    let key = glib::GStr::from_ptr_checked(key).unwrap().as_str();

    let image_info = image.image_info();
    let value = image_info
        .metadata_key_value()
        .as_ref()
        .and_then(|x| x.get(key));

    value.to_glib_full()
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_get_metadata_keys(image: *mut GlyImage) -> GStrv {
    let image = gobject::GlyImage::from_glib_ptr_borrow(&image);

    image
        .image_info()
        .metadata_key_value()
        .as_ref()
        .map(|x| glib::StrV::from_iter(x.keys().map(|x| glib::GString::from(x))))
        .unwrap_or_default()
        .into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn gly_image_get_transformation_orientation(image: *mut GlyImage) -> u16 {
    let image = gobject::GlyImage::from_glib_ptr_borrow(&image);
    image.image().transformation_orientation().into()
}
