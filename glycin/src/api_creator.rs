use std::sync::Arc;

use glib::object::IsA;
use glib::prelude::*;
use glycin_utils::{BinaryData, Frame, ImageInfo, MemoryFormat};

use crate::error::ResultExt;
use crate::pool::Pool;
use crate::{spin_up_encoder, Error, ErrorCtx, MimeType, SandboxSelector};

#[derive(Debug)]
pub struct Creator {
    pool: Arc<Pool>,
    pub(crate) cancellable: gio::Cancellable,
    pub(crate) sandbox_selector: SandboxSelector,
}

static_assertions::assert_impl_all!(Creator: Send, Sync);

impl Default for Creator {
    fn default() -> Self {
        Self::new()
    }
}

impl Creator {
    /// Create an encoder.
    pub fn new() -> Self {
        Self {
            pool: Pool::global(),
            cancellable: gio::Cancellable::new(),
            sandbox_selector: SandboxSelector::default(),
        }
    }

    /// Sets the method by which the sandbox mechanism is selected.
    ///
    /// The default without calling this function is [`SandboxSelector::Auto`].
    pub fn sandbox_selector(&mut self, sandbox_selector: SandboxSelector) -> &mut Self {
        self.sandbox_selector = sandbox_selector;
        self
    }

    /// Set [`Cancellable`](gio::Cancellable) to cancel any editing operations.
    pub fn cancellable(&mut self, cancellable: impl IsA<gio::Cancellable>) -> &mut Self {
        self.cancellable = cancellable.upcast();
        self
    }

    /// Encode an image
    pub async fn create(
        self,
        new_image: NewImage,
        mime_type: MimeType,
    ) -> Result<EncodedImage, ErrorCtx> {
        let process_context = spin_up_encoder(
            mime_type.clone(),
            &self.pool,
            &self.cancellable,
            &self.sandbox_selector,
            Arc::downgrade(&Arc::new(())),
        )
        .await
        .err_no_context(&self.cancellable)?;

        let process = process_context.process.use_();

        Ok(EncodedImage::new(
            process
                .create(new_image.into_inner(mime_type))
                .await
                .err_context(&process, &self.cancellable)?,
        ))
    }
}

#[derive(Debug)]
pub struct NewImage {
    pub(crate) inner: glycin_utils::NewImage,
}

impl NewImage {
    pub fn new(
        width: u32,
        height: u32,
        memory_format: MemoryFormat,
        texture: impl AsRef<[u8]>,
    ) -> Result<Self, Error> {
        let mime_type = String::new();

        let mut image_info = ImageInfo::default();
        image_info.width = width;
        image_info.height = height;

        let texture = BinaryData::from_data(texture).map_err(|x| x.into_editor_error())?;
        let frame = Frame::new(width, height, memory_format, texture)?;

        let frames = vec![frame];

        Ok(Self {
            inner: glycin_utils::NewImage::new(mime_type, image_info, frames),
        })
    }

    fn into_inner(mut self, mime_type: MimeType) -> glycin_utils::NewImage {
        self.inner.mime_type = mime_type.to_string();
        self.inner
    }
}

#[derive(Debug)]
pub struct EncodedImage {
    pub(crate) inner: glycin_utils::EncodedImage,
}

impl EncodedImage {
    pub fn new(inner: glycin_utils::EncodedImage) -> Self {
        Self { inner }
    }

    pub fn data_ref(&self) -> Result<glycin_utils::BinaryDataRef, std::io::Error> {
        self.inner.data.get()
    }

    pub fn data_full(&self) -> Result<Vec<u8>, std::io::Error> {
        self.inner.data.get_full()
    }
}
