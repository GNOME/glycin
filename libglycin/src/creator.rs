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

pub type GlyCreator = <gobject::creator::imp::GlyCreator as ObjectSubclass>::Instance;

#[no_mangle]
pub unsafe extern "C" fn gly_creator_new(mime_type: *const c_char) -> *mut GlyCreator {
    let mime_type = glib::GStr::from_ptr_checked(mime_type).unwrap().to_string();
    gobject::GlyCreator::new(mime_type).into_glib_ptr()
}

/*
#[no_mangle]
pub unsafe extern "C" fn gly_creator_set_sandbox_selector(
    loader: *mut GlyLoader,
    sandbox_selector: i32,
) {
    let sandbox_selector = GlySandboxSelector::from_glib(sandbox_selector);
    let obj = gobject::GlyLoader::from_glib_ptr_borrow(&loader);

    obj.set_sandbox_selector(sandbox_selector);
}
 */

#[no_mangle]
pub unsafe extern "C" fn gly_creator_create(
    creator: *mut GlyCreator,
    new_image: *mut GlyNewImage,
    g_error: *mut *mut GError,
) -> *mut GlyEncodedImage {
    let obj = gobject::GlyCreator::from_glib_ptr_borrow(&creator);

    let new_image = gobject::GlyNewImage::from_glib_ptr_borrow(&new_image);

    let result = async_io::block_on(async move {
        // TODO unwrap
        let new_image = new_image.new_image().await.unwrap();
        obj.create(new_image).await
    });

    match result {
        Ok(image) => image.into_glib_ptr(),
        Err(err) => {
            set_error(g_error, &err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn gly_creator_create_async(
    creator: *mut GlyCreator,
    new_image: *mut GlyNewImage,
    cancellable: *mut gio::ffi::GCancellable,
    callback: GAsyncReadyCallback,
    user_data: gpointer,
) {
    let obj = gobject::GlyCreator::from_glib_none(creator);
    let cancellable: Option<gio::Cancellable> = from_glib_none(cancellable);
    let callback = GAsyncReadyCallbackSend::new(callback, user_data);

    let new_image = gobject::GlyNewImage::from_glib_none(new_image);

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

    async_io::block_on(async move {
        // TODO unwrap
        let new_image = new_image.new_image().await.unwrap();
        let res = obj.create(new_image).await.map_err(|x| glib_error(&x));
        task.return_result(res);
    });
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
pub extern "C" fn gly_creator_get_type() -> GType {
    <gobject::GlyCreator as StaticType>::static_type().into_glib()
}
