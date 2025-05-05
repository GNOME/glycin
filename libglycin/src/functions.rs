use gio::glib::{
    self,
    ffi::{gpointer, GStrv},
};

type GlyLoaderGetMimeTypesDoneFunc = unsafe extern "C" fn(GStrv, gpointer);

#[no_mangle]
pub extern "C" fn gly_loader_get_mime_types() -> GStrv {
    let mime_types = glib::StrV::from_iter(
        glib::MainContext::default()
            .block_on(glycin::Loader::supported_mime_types())
            .into_iter()
            .map(|x| glib::GString::from(x.as_str())),
    );

    mime_types.into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn gly_loader_get_mime_types_async(
    callback: GlyLoaderGetMimeTypesDoneFunc,
    data: gpointer,
) {
    glib::MainContext::default().spawn_local(async move {
        let mime_types = glycin::Loader::supported_mime_types().await;
        let strv = glib::StrV::from_iter(
            mime_types
                .into_iter()
                .map(|x| glib::GString::from(x.as_str())),
        );
        callback(strv.into_raw(), data);
    });
}
