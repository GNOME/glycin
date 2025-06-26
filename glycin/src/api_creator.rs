use std::collections::BTreeMap;
use std::sync::Arc;

use glib::object::IsA;
use glib::prelude::*;
use glycin_utils::{BinaryData, EncodingOptions, Frame, ImageInfo, MemoryFormat};

use crate::error::ResultExt;
use crate::pool::Pool;
use crate::{spin_up_encoder, Error, ErrorCtx, MimeType, SandboxSelector};

#[derive(Debug)]
pub struct Creator {
    mime_type: MimeType,
    pool: Arc<Pool>,
    pub(crate) cancellable: gio::Cancellable,
    pub(crate) sandbox_selector: SandboxSelector,
    encoding_options: EncodingOptions,
}

static_assertions::assert_impl_all!(Creator: Send, Sync);

#[derive(Debug)]
pub struct FeatureNotSupported;

impl std::fmt::Display for FeatureNotSupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Feature not supported by this image format.")
    }
}

impl std::error::Error for FeatureNotSupported {}

impl Creator {
    /// Create an encoder.
    pub fn new(mime_type: MimeType) -> Self {
        Self {
            mime_type,
            pool: Pool::global(),
            cancellable: gio::Cancellable::new(),
            sandbox_selector: SandboxSelector::default(),
            encoding_options: EncodingOptions::default(),
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
    pub async fn create(self, new_image: NewImage) -> Result<EncodedImage, ErrorCtx> {
        let process_context = spin_up_encoder(
            self.mime_type.clone(),
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
                .create(
                    &self.mime_type,
                    new_image.into_inner().err_no_context(&self.cancellable)?,
                    self.encoding_options,
                )
                .await
                .err_context(&process, &self.cancellable)?,
        ))
    }

    pub fn set_quality(&mut self, quality: u8) -> Result<(), FeatureNotSupported> {
        self.encoding_options.quality = Some(quality);
        Ok(())
    }
}

#[derive(Debug)]
pub struct NewImage {
    pub(crate) inner: glycin_utils::NewImage,
    icc_profile: Option<Vec<u8>>,
}

impl NewImage {
    pub fn new(
        width: u32,
        height: u32,
        memory_format: MemoryFormat,
        texture: impl AsRef<[u8]>,
    ) -> Result<Self, Error> {
        let mut image_info = ImageInfo::default();
        image_info.width = width;
        image_info.height = height;

        let texture = BinaryData::from_data(texture).map_err(|x| x.into_editor_error())?;
        let frame = Frame::new(width, height, memory_format, texture)?;

        let frames = vec![frame];

        Ok(Self {
            inner: glycin_utils::NewImage::new(image_info, frames),
            icc_profile: None,
        })
    }

    fn into_inner(mut self) -> Result<glycin_utils::NewImage, Error> {
        // TODO unwrap
        if let Some(icc_profile) = self
            .icc_profile
            .as_ref()
            .map(|x| BinaryData::from_data(x))
            .transpose()
            .unwrap()
        {
            for frame in &mut self.inner.frames {
                frame.details.iccp = Some(icc_profile.clone());
            }
        }

        Ok(self.inner)
    }

    pub fn set_key_value(&mut self, key_value: BTreeMap<String, String>) {
        self.inner.image_info.key_value = Some(key_value);
    }

    /// Sets the ICC profile for all frames
    ///
    /// If this is set to `None`, which is the default, the ICC profile of the
    /// frames will be left untouched.
    pub fn set_color_icc_profile(&mut self, icc_profile: Option<Vec<u8>>) {
        self.icc_profile = icc_profile;
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
