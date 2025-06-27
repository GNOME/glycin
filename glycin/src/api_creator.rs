use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use glib::object::IsA;
use glib::prelude::*;
use glycin_utils::{BinaryData, MemoryFormat};

use crate::config::{Config, ImageEditorConfig};
use crate::error::ResultExt;
use crate::pool::Pool;
use crate::{spin_up_encoder, Error, ErrorCtx, MimeType, SandboxSelector};

#[derive(Debug)]
pub struct Creator {
    mime_type: MimeType,
    config: ImageEditorConfig,
    pool: Arc<Pool>,
    pub(crate) cancellable: gio::Cancellable,
    pub(crate) sandbox_selector: SandboxSelector,
    encoding_options: glycin_utils::EncodingOptions,
    new_image: glycin_utils::NewImage,

    new_frames: Vec<Arc<NewFrame>>,
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
    pub async fn new(mime_type: MimeType) -> Result<Creator, Error> {
        let config = Config::cached().await.editor(&mime_type)?.clone();

        Ok(Self {
            mime_type,
            config,
            pool: Pool::global(),
            cancellable: gio::Cancellable::new(),
            sandbox_selector: SandboxSelector::default(),
            encoding_options: glycin_utils::EncodingOptions::default(),
            new_image: glycin_utils::NewImage::new(glycin_utils::ImageInfo::new(1, 1), vec![]),
            new_frames: vec![],
        })
    }

    pub fn add_frame(
        &mut self,
        width: u32,
        height: u32,
        memory_format: MemoryFormat,
        texture: impl AsRef<[u8]>,
    ) -> Arc<NewFrame> {
        let new_frame = Arc::new(NewFrame::new(
            self.config.clone(),
            width,
            height,
            memory_format,
            texture.as_ref().to_vec(),
        ));

        self.new_frames.push(new_frame.clone());

        new_frame
    }

    /// Encode an image
    pub async fn create(self) -> Result<EncodedImage, ErrorCtx> {
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

        let mut new_image = self.new_image;

        for frame in self.new_frames {
            new_image
                .frames
                .push((frame).frame().err_no_context(&self.cancellable)?);
        }

        Ok(EncodedImage::new(
            process
                .create(&self.mime_type, new_image, self.encoding_options)
                .await
                .err_context(&process, &self.cancellable)?,
        ))
    }

    pub fn set_encoding_quality(&mut self, quality: u8) -> Result<(), FeatureNotSupported> {
        self.encoding_options.quality = Some(quality);
        Ok(())
    }

    /// Set compression level
    ///
    /// This sets the lossless compression level. The range is from 0 (no
    /// compression) to 100 (highest compression).
    pub fn set_encoding_compression(&mut self, compression: u8) -> Result<(), FeatureNotSupported> {
        self.encoding_options.compression = Some(compression);
        Ok(())
    }

    pub fn set_metadata_key_value(
        &mut self,
        key_value: BTreeMap<String, String>,
    ) -> Result<(), FeatureNotSupported> {
        self.new_image.image_info.key_value = Some(key_value);
        Ok(())
    }

    pub fn add_metadata_key_value(
        &mut self,
        key: String,
        value: String,
    ) -> Result<(), FeatureNotSupported> {
        let mut key_value = self
            .new_image
            .image_info
            .key_value
            .clone()
            .unwrap_or_default();
        key_value.insert(key, value);
        self.new_image.image_info.key_value = Some(key_value);
        Ok(())
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
}

#[derive(Debug)]
pub struct NewFrame {
    //config: ImageEditorConfig,
    width: u32,
    height: u32,
    //stride: Option<u32>,
    memory_format: MemoryFormat,
    texture: Vec<u8>,
    //delay: Option<Duration>,
    details: glycin_utils::FrameDetails,
    icc_profile: Mutex<Option<Vec<u8>>>,
}

impl NewFrame {
    fn new(
        config: ImageEditorConfig,
        width: u32,
        height: u32,
        memory_format: MemoryFormat,
        texture: Vec<u8>,
    ) -> NewFrame {
        Self {
            //config,
            width,
            height,
            memory_format,
            texture,
            //stride: None,
            //delay: None,
            details: Default::default(),
            icc_profile: Default::default(),
        }
    }

    pub fn set_color_icc_profile(&self, icc_profile: Option<Vec<u8>>) {
        *self.icc_profile.lock().unwrap() = icc_profile;
    }

    fn frame(&self) -> Result<glycin_utils::Frame, Error> {
        // TODO fix unwrap
        let texture = BinaryData::from_data(&self.texture).unwrap();
        let mut frame =
            glycin_utils::Frame::new(self.width, self.height, self.memory_format, texture)?;

        frame.details = self.details.clone();

        if let Some(icc_profile) = self.icc_profile.lock().unwrap().as_ref() {
            // TODO unwrap
            let icc_profile = BinaryData::from_data(icc_profile).unwrap();
            frame.details.iccp = Some(icc_profile);
        }

        Ok(frame)
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
