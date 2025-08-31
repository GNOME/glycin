use std::ffi::c_char;
use std::ptr;

use gio::ffi::{GAsyncReadyCallback, GAsyncResult, GTask};
use gio::glib;
use gio::prelude::*;
use glib::ffi::{gpointer, GBytes, GError, GType};
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject::{self};
use glycin::SandboxSelector as GlySandboxSelector;

use crate::common::*;
use crate::*;

pub type GlyCreator = <gobject::creator::imp::GlyCreator as ObjectSubclass>::Instance;

#[no_mangle]
pub unsafe extern "C" fn gly_creator_new(
    mime_type: *const c_char,
    g_error: *mut *mut GError,
) -> *mut GlyCreator {
    let mime_type = glib::GStr::from_ptr_checked(mime_type).unwrap().to_string();

    let creator = async_global_executor::block_on(gobject::GlyCreator::new(mime_type));

    match creator {
        Ok(creator) => creator.into_glib_ptr(),
        Err(err) => {
            set_error(g_error, &err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn gly_creator_set_sandbox_selector(
    loader: *mut GlyLoader,
    sandbox_selector: i32,
) {
    let sandbox_selector = GlySandboxSelector::from_glib(sandbox_selector);
    let obj = gobject::GlyLoader::from_glib_ptr_borrow(&loader);

    obj.set_sandbox_selector(sandbox_selector);
}

#[no_mangle]
pub unsafe extern "C" fn gly_creator_create(
    creator: *mut GlyCreator,
    g_error: *mut *mut GError,
) -> *mut GlyEncodedImage {
    let obj = gobject::GlyCreator::from_glib_ptr_borrow(&creator);

    let result = async_global_executor::block_on(async move { obj.create().await });

    match result {
        Ok(image) => image.into_glib_ptr(),
        Err(err) => {
            set_context_error(g_error, &err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn gly_creator_add_frame(
    creator: *mut GlyCreator,
    width: u32,
    height: u32,
    memory_format: i32,
    data: *mut GBytes,
    g_error: *mut *mut GError,
) -> *mut GlyNewFrame {
    let obj = gobject::GlyCreator::from_glib_ptr_borrow(&creator);
    let memory_format = glycin::MemoryFormat::try_from(memory_format).unwrap();
    let data = glib::Bytes::from_glib_ptr_borrow(&data).clone();

    let new_frame: Result<gobject::GlyNewFrame, glycin::Error> =
        obj.add_frame(width, height, memory_format, data.to_vec());

    match new_frame {
        Ok(new_frame) => new_frame.into_glib_ptr(),
        Err(err) => {
            set_error(g_error, &err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn gly_creator_add_frame_with_stride(
    creator: *mut GlyCreator,
    width: u32,
    height: u32,
    stride: u32,
    memory_format: i32,
    data: *mut GBytes,
    g_error: *mut *mut GError,
) -> *mut GlyNewFrame {
    let obj = gobject::GlyCreator::from_glib_ptr_borrow(&creator);
    let memory_format = glycin::MemoryFormat::try_from(memory_format).unwrap();
    let data = glib::Bytes::from_glib_ptr_borrow(&data).clone();

    let new_frame: Result<gobject::GlyNewFrame, glycin::Error> =
        obj.add_frame_with_stride(width, height, stride, memory_format, data.to_vec());

    match new_frame {
        Ok(new_frame) => new_frame.into_glib_ptr(),
        Err(err) => {
            set_error(g_error, &err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn gly_creator_create_async(
    creator: *mut GlyCreator,
    cancellable: *mut gio::ffi::GCancellable,
    callback: GAsyncReadyCallback,
    user_data: gpointer,
) {
    let obj = gobject::GlyCreator::from_glib_none(creator);
    let cancellable: Option<gio::Cancellable> = from_glib_none(cancellable);
    let callback = GAsyncReadyCallbackSend::new(callback, user_data);

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
    let closure = move |task: gio::Task<gobject::GlyEncodedImage>,
                        obj: Option<&gobject::GlyCreator>| {
        if let (Some(cancel_signal), Some(cancellable)) = (cancel_signal, cancellable) {
            cancellable.disconnect_cancelled(cancel_signal);
        }

        let result = task.upcast_ref::<gio::AsyncResult>().as_ptr();
        callback.call(obj.unwrap(), result);
    };

    let task = gio::Task::new(Some(&obj), cancellable_.as_ref(), closure);

    async_global_executor::spawn(async move {
        let res = obj.create().await.map_err(|x| glib_context_error(&x));
        task.return_result(res);
    })
    .detach();
}

#[no_mangle]
pub unsafe extern "C" fn gly_creator_create_finish(
    _creator: *mut GlyCreator,
    res: *mut GAsyncResult,
    error: *mut *mut GError,
) -> *mut GlyEncodedImage {
    let task = gio::Task::<gobject::GlyEncodedImage>::from_glib_none(res as *mut GTask);

    match task.propagate() {
        Ok(image) => image.into_glib_ptr(),
        Err(e) => {
            if !error.is_null() {
                *error = e.into_glib_ptr();
            }
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn gly_creator_add_metadata_key_value(
    creator: *mut GlyCreator,
    key: *const c_char,
    value: *const c_char,
) -> glib::ffi::gboolean {
    let key = glib::GStr::from_ptr_checked(key).unwrap().as_str();
    let value = glib::GStr::from_ptr_checked(value).unwrap().as_str();
    let creator = gobject::GlyCreator::from_glib_ptr_borrow(&creator);

    creator
        .metadata_add_key_value(key.to_string(), value.to_string())
        .is_ok()
        .into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_creator_set_encoding_quality(
    creator: *mut GlyCreator,
    quality: u8,
) -> glib::ffi::gboolean {
    let creator = gobject::GlyCreator::from_glib_ptr_borrow(&creator);

    creator.set_encoding_quality(quality).is_ok().into_glib()
}

#[no_mangle]
pub unsafe extern "C" fn gly_creator_set_encoding_compression(
    creator: *mut GlyCreator,
    compression: u8,
) -> glib::ffi::gboolean {
    let creator = gobject::GlyCreator::from_glib_ptr_borrow(&creator);

    creator
        .set_encoding_compression(compression)
        .is_ok()
        .into_glib()
}

#[no_mangle]
pub extern "C" fn gly_creator_get_type() -> GType {
    <gobject::GlyCreator as StaticType>::static_type().into_glib()
}
