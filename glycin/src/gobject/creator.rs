use std::sync::{Mutex, OnceLock};

use async_lock::MutexGuard;
use gio::glib;
use glib::prelude::*;
use glib::subclass::prelude::*;
use glycin_utils::MemoryFormat;

use crate::error::ResultExt;
use crate::gobject::GlyNewFrame;
use crate::util::AsyncMutex;
use crate::{gobject, Creator, Error, MimeType, SandboxSelector};

static_assertions::assert_impl_all!(GlyCreator: Send, Sync);
use super::init;

pub mod imp {

    use super::*;

    #[derive(Default, Debug, glib::Properties)]
    #[properties(wrapper_type = super::GlyCreator)]
    pub struct GlyCreator {
        #[property(get, set, builder(SandboxSelector::default()))]
        pub(super) sandbox_selector: Mutex<SandboxSelector>,
        #[property(get, construct_only)]
        mime_type: OnceLock<String>,

        pub(super) creator: AsyncMutex<Option<Creator>>,
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
            let obj = self.obj();

            let mut creator = self.creator.lock_blocking();

            if creator.is_none() {
                *creator = async_io::block_on(Creator::new(MimeType::new(obj.mime_type()))).ok();
            }

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
    pub async fn new(mime_type: String) -> Result<Self, Error> {
        let creator = Creator::new(MimeType::new(mime_type.clone())).await?;

        let obj = glib::Object::builder::<Self>()
            .property("mime-type", mime_type)
            .build();

        *obj.imp().creator.lock_blocking() = Some(creator);

        Ok(obj)
    }

    pub fn cancellable(&self) -> gio::Cancellable {
        self.imp()
            .creator
            .lock_blocking()
            .as_ref()
            .unwrap()
            .cancellable
            .clone()
    }

    pub fn creator(&self) -> MutexGuard<'_, Option<Creator>> {
        self.imp().creator.lock_blocking()
    }

    pub fn metadata_add_key_value(
        &self,
        key: String,
        value: String,
    ) -> Result<(), crate::FeatureNotSupported> {
        self.creator()
            .as_mut()
            .unwrap()
            .add_metadata_key_value(key, value)
    }

    pub fn set_encoding_quality(&self, quality: u8) -> Result<(), crate::FeatureNotSupported> {
        self.creator()
            .as_mut()
            .unwrap()
            .set_encoding_quality(quality)
    }

    pub fn set_encoding_compression(
        &self,
        compression: u8,
    ) -> Result<(), crate::FeatureNotSupported> {
        self.creator()
            .as_mut()
            .unwrap()
            .set_encoding_compression(compression)
    }

    pub fn add_frame(
        &self,
        width: u32,
        height: u32,
        memory_format: MemoryFormat,
        texture: Vec<u8>,
    ) -> Result<GlyNewFrame, Error> {
        let new_frame =
            self.creator()
                .as_mut()
                .unwrap()
                .add_frame(width, height, memory_format, texture)?;

        Ok(GlyNewFrame::new(new_frame))
    }

    pub fn add_frame_with_stride(
        &self,
        width: u32,
        height: u32,
        stride: u32,
        memory_format: MemoryFormat,
        texture: Vec<u8>,
    ) -> Result<GlyNewFrame, Error> {
        let new_frame = self.creator().as_mut().unwrap().add_frame_with_stride(
            width,
            height,
            stride,
            memory_format,
            texture,
        )?;

        Ok(GlyNewFrame::new(new_frame))
    }

    pub async fn create(&self) -> Result<gobject::GlyEncodedImage, crate::ErrorCtx> {
        if let Some(mut creator) = std::mem::take(&mut *self.imp().creator.lock_blocking()) {
            creator.sandbox_selector(self.sandbox_selector());
            let encoded_image: crate::EncodedImage = creator.create().await?;
            Ok(gobject::GlyEncodedImage::new(encoded_image))
        } else {
            Err(Error::LoaderUsedTwice).err_no_context(&self.cancellable())
        }
    }
}
