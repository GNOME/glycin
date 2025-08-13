use std::sync::Mutex;

use gio::glib;
use glib::prelude::*;
use glib::subclass::prelude::*;
use glycin_common::MemoryFormatSelection;

use super::GlyImage;
use crate::error::ResultExt;
use crate::{Error, GInputStreamSend, SandboxSelector, Source};

static_assertions::assert_impl_all!(GlyLoader: Send, Sync);
use super::init;

pub mod imp {
    use super::*;

    #[derive(Default, Debug, glib::Properties)]
    #[properties(wrapper_type = super::GlyLoader)]
    pub struct GlyLoader {
        #[property(get, construct_only)]
        pub(super) file: Mutex<Option<gio::File>>,
        #[property(get=Self::stream, set=Self::set_stream, construct_only, type=Option<gio::InputStream>)]
        pub(super) stream: Mutex<Option<GInputStreamSend>>,
        #[property(get, construct_only)]
        pub(super) bytes: Mutex<Option<glib::Bytes>>,

        #[property(get, set)]
        cancellable: Mutex<gio::Cancellable>,
        #[property(get, set, builder(SandboxSelector::default()))]
        sandbox_selector: Mutex<SandboxSelector>,
        #[property(get, set)]
        memory_format_selection: Mutex<MemoryFormatSelection>,
        #[property(get, set)]
        apply_transformation: Mutex<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlyLoader {
        const NAME: &'static str = "GlyLoader";
        type Type = super::GlyLoader;
    }

    #[glib::derived_properties]
    impl ObjectImpl for GlyLoader {
        fn constructed(&self) {
            self.parent_constructed();

            init();

            let obj = self.obj();

            *self.apply_transformation.lock().unwrap() = true;

            if obj.file().is_some() as u8
                + obj.stream().is_some() as u8
                + obj.bytes().is_some() as u8
                != 1
            {
                glib::g_critical!("glycin", "A loader needs to be initialized with exactly one of 'file', 'stream', or 'bytes'.");
            }
        }
    }

    impl GlyLoader {
        fn stream(&self) -> Option<gio::InputStream> {
            self.stream.lock().unwrap().as_ref().map(|x| x.stream())
        }

        fn set_stream(&self, stream: Option<&gio::InputStream>) {
            let stream = unsafe { stream.map(|x| GInputStreamSend::new(x.clone())) };
            *self.stream.lock().unwrap() = stream;
        }
    }
}

glib::wrapper! {
    /// GObject wrapper for [`Loader`]
    pub struct GlyLoader(ObjectSubclass<imp::GlyLoader>);
}

impl GlyLoader {
    pub fn new(file: &gio::File) -> Self {
        glib::Object::builder().property("file", file).build()
    }

    pub fn for_stream(stream: &gio::InputStream) -> Self {
        glib::Object::builder().property("stream", stream).build()
    }

    pub fn for_bytes(bytes: &glib::Bytes) -> Self {
        glib::Object::builder().property("bytes", bytes).build()
    }

    pub async fn load(&self) -> Result<GlyImage, crate::ErrorCtx> {
        let mut loader = if let Some(file) = std::mem::take(&mut *self.imp().file.lock().unwrap()) {
            crate::Loader::new(file)
        } else if let Some(stream) = std::mem::take(&mut *self.imp().stream.lock().unwrap()) {
            crate::Loader::new_source(Source::Stream(stream))
        } else if let Some(bytes) = std::mem::take(&mut *self.imp().bytes.lock().unwrap()) {
            crate::Loader::new_bytes(bytes)
        } else {
            return Err(Error::LoaderUsedTwice).err_no_context(&self.cancellable());
        };

        loader.sandbox_selector = self.sandbox_selector();
        loader.memory_format_selection = self.memory_format_selection();
        loader.apply_transformations = self.apply_transformation();
        loader.cancellable(self.cancellable());

        let image = loader.load().await?;

        Ok(GlyImage::new(image))
    }
}
