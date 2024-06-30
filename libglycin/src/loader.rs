use std::ptr;

use gio::ffi::{GAsyncReadyCallback, GAsyncResult, GTask};
use gio::glib;
use gio::prelude::*;
use glib::ffi::{gpointer, GError, GType};
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject;
pub use glycin::SandboxSelector as GlySandboxSelector;

use crate::common::*;
use crate::*;

pub type GlyLoader = <gobject::loader::imp::GlyLoader as ObjectSubclass>::Instance;

#[no_mangle]
pub unsafe extern "C" fn gly_loader_new(file: *mut gio::ffi::GFile) -> *mut GlyLoader {
    let file = gio::File::from_glib_ptr_borrow(&file);
    gobject::GlyLoader::new(&file).into_glib_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn gly_loader_set_sandbox_selector(
    loader: *mut GlyLoader,
    sandbox_selector: i32,
) {
    let sandbox_selector = GlySandboxSelector::from_glib(sandbox_selector);
    let obj = gobject::GlyLoader::from_glib_ptr_borrow(&loader);

    obj.set_sandbox_selector(sandbox_selector);
}

#[no_mangle]
pub unsafe extern "C" fn gly_loader_load(
    loader: *mut GlyLoader,
    g_error: *mut *mut GError,
) -> *mut GlyImage {
    let obj = gobject::GlyLoader::from_glib_ptr_borrow(&loader);

    let result = async_io::block_on(obj.load());

    match result {
        Ok(image) => image.into_glib_ptr(),
        Err(err) => {
            set_error(g_error, &err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn gly_loader_load_async(
    loader: *mut GlyLoader,
    cancellable: *mut gio::ffi::GCancellable,
    callback: GAsyncReadyCallback,
    user_data: gpointer,
) {
    let obj = gobject::GlyLoader::from_glib_none(loader);
    let cancellable =
        (!cancellable.is_null()).then(|| gio::Cancellable::from_glib_none(cancellable));
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
    let closure = move |task: gio::Task<gobject::GlyImage>, obj: Option<&gobject::GlyLoader>| {
        if let (Some(cancel_signal), Some(cancellable)) = (cancel_signal, cancellable) {
            cancellable.disconnect_cancelled(cancel_signal);
        }

        let result = task.upcast_ref::<gio::AsyncResult>().as_ptr();
        callback.call(obj.unwrap(), result);
    };

    let task = gio::Task::new(Some(&obj), cancellable_.as_ref(), closure);

    glib::MainContext::ref_thread_default().spawn_local(async move {
        let res = obj.load().await.map_err(|x| glib_error(&x));
        task.return_result(res);
    });
}

#[no_mangle]
pub unsafe extern "C" fn gly_loader_load_finish(
    _loader: *mut GlyLoader,
    res: *mut GAsyncResult,
    error: *mut *mut GError,
) -> *mut GlyImage {
    let task = gio::Task::<gobject::GlyImage>::from_glib_none(res as *mut GTask);

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
pub extern "C" fn gly_loader_get_type() -> GType {
    <gobject::GlyLoader as StaticType>::static_type().into_glib()
}

#[no_mangle]
pub extern "C" fn gly_sandbox_selector_get_type() -> GType {
    <GlySandboxSelector as StaticType>::static_type().into_glib()
}
