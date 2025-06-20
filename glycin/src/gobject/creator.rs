use std::sync::Mutex;

use gio::glib;
use glib::prelude::*;
use glib::subclass::prelude::*;

use crate::error::ResultExt;
use crate::gobject;
use crate::{Creator, Error, MimeType, NewImage, SandboxSelector};

static_assertions::assert_impl_all!(GlyCreator: Send, Sync);
use super::init;

pub mod imp {
    use super::*;

    #[derive(Default, Debug, glib::Properties)]
    #[properties(wrapper_type = super::GlyCreator)]
    pub struct GlyCreator {
        #[property(get, set, builder(SandboxSelector::default()))]
        sandbox_selector: Mutex<SandboxSelector>,

        pub(super) creator: Mutex<Option<Creator>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlyCreator {
        const NAME: &'static str = "GlyCreator";
        type Type = super::GlyCreator;
    }

    #[glib::derived_properties]
    impl ObjectImpl for GlyCreator {
        fn constructed(&self) {
            self.parent_constructed();

            *self.creator.lock().unwrap() = Some(Creator::new());

            init();
        }
    }

    impl GlyCreator {}
}

glib::wrapper! {
    /// GObject wrapper for [`Loader`]
    pub struct GlyCreator(ObjectSubclass<imp::GlyCreator>);
}

impl GlyCreator {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn cancellable(&self) -> gio::Cancellable {
        self.imp()
            .creator
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .cancellable
            .clone()
    }

    pub async fn create(
        &self,
        new_image: NewImage,
        mime_type: MimeType,
    ) -> Result<gobject::GlyEncodedImage, crate::ErrorCtx> {
        if let Some(creator) = std::mem::take(&mut *self.imp().creator.lock().unwrap()) {
            let encoded_image = creator.create(new_image, mime_type).await?;
            Ok(gobject::GlyEncodedImage::new(encoded_image))
        } else {
            Err(Error::LoaderUsedTwice).err_no_context(&self.cancellable())
        }
    }
}
