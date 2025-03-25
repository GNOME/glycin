use std::sync::Mutex;

use gio::glib;
use glib::prelude::*;
use glib::subclass::prelude::*;
use glycin_utils::MemoryFormatSelection;

use super::GlyImage;
use crate::error::ResultExt;
use crate::{Error, GInputStreamSend, Loader, SandboxSelector, Source};

static_assertions::assert_impl_all!(GlyLoader: Send, Sync);

pub mod imp {
    use super::*;

    #[derive(Default, Debug, glib::Properties)]
    #[properties(wrapper_type = super::GlyLoader)]
    pub struct GlyLoader {
        #[property(get, construct_only)]
        file: Mutex<Option<gio::File>>,
        pub(super) source: Mutex<Option<Source>>,

        #[property(get, set)]
        cancellable: Mutex<gio::Cancellable>,
        #[property(get, set, builder(SandboxSelector::default()))]
        sandbox_selector: Mutex<SandboxSelector>,
        #[property(get, set)]
        memory_format_selection: Mutex<MemoryFormatSelection>,
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

            if let Some(file) = self.obj().file() {
                self.set_source(Source::File(file.clone()));
            }
        }
    }

    impl GlyLoader {
        pub(super) fn set_source(&self, source: Source) {
            *self.source.lock().unwrap() = Some(source);
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
        let obj = glib::Object::builder::<GlyLoader>().build();
        let stream = unsafe { GInputStreamSend::new(stream.clone()) };
        obj.imp().set_source(Source::Stream(stream));
        obj
    }

    pub fn for_bytes(bytes: &glib::Bytes) -> Self {
        let obj = glib::Object::builder::<GlyLoader>().build();
        let stream =
            unsafe { GInputStreamSend::new(gio::MemoryInputStream::from_bytes(&bytes).upcast()) };
        obj.imp().set_source(Source::Stream(stream));
        obj
    }

    pub async fn load(&self) -> Result<GlyImage, crate::ErrorCtx> {
        let Some(source) = std::mem::take(&mut *self.imp().source.lock().unwrap()) else {
            return Err(Error::LoaderUsedTwice).err_no_context(&self.cancellable());
        };

        let mut loader = Loader::with_source(source);

        loader.sandbox_selector = self.sandbox_selector();
        loader.memory_format_selection = self.memory_format_selection();
        loader.cancellable(self.cancellable());

        let image = loader.load().await?;

        Ok(GlyImage::new(image))
    }
}
