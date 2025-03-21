use std::sync::Mutex;

use gio::glib;
use glib::prelude::*;
use glib::subclass::prelude::*;

use super::GlyImage;
use crate::{Loader, SandboxSelector};

static_assertions::assert_impl_all!(GlyLoader: Send, Sync);

pub mod imp {
    use glycin_utils::MemoryFormatSelection;

    use super::*;

    #[derive(Default, Debug, glib::Properties)]
    #[properties(wrapper_type = super::GlyLoader)]
    pub struct GlyLoader {
        #[property(get, construct_only)]
        file: Mutex<Option<gio::File>>,
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
    impl ObjectImpl for GlyLoader {}
}

glib::wrapper! {
    /// GObject wrapper for [`Loader`]
    pub struct GlyLoader(ObjectSubclass<imp::GlyLoader>);
}

impl GlyLoader {
    pub fn new(file: &gio::File) -> Self {
        glib::Object::builder().property("file", file).build()
    }

    pub async fn load(&self) -> Result<GlyImage, crate::ErrorCtx> {
        let mut loader = Loader::new(self.file().unwrap());

        loader.sandbox_selector = self.sandbox_selector();
        loader.memory_format_selection = self.memory_format_selection();
        loader.cancellable(self.cancellable());

        let image = loader.load().await?;

        Ok(GlyImage::new(image))
    }
}
