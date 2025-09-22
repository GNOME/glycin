use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use glib::object::IsA;
use glib::prelude::*;
use glycin_common::{BinaryData, MemoryFormatInfo};
use glycin_utils::{DimensionTooLargerError, MemoryFormat};

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
            new_image: glycin_utils::NewImage::new(glycin_utils::ImageDetails::new(1, 1), vec![]),
            new_frames: vec![],
        })
    }

    pub fn add_frame(
        &mut self,
        width: u32,
        height: u32,
        memory_format: MemoryFormat,
        texture: Vec<u8>,
    ) -> Result<Arc<NewFrame>, Error> {
        let stride = memory_format
            .n_bytes()
            .u32()
            .checked_mul(width)
            .ok_or(DimensionTooLargerError)?;

        let new_frame =
            self.add_frame_with_stride(width, height, stride, memory_format, texture)?;

        Ok(new_frame)
    }

    pub fn add_frame_with_stride(
        &mut self,
        width: u32,
        height: u32,
        stride: u32,
        memory_format: MemoryFormat,
        mut texture: Vec<u8>,
    ) -> Result<Arc<NewFrame>, Error> {
        let pixel_size = memory_format.n_bytes().u32();

        let smallest_stride = pixel_size
            .checked_mul(width)
            .ok_or(DimensionTooLargerError)?;

        if smallest_stride > stride {
            return Err(Error::StrideTooSmall(format!(
                "Stride is {stride} but must be at least {smallest_stride}"
            )));
        }

        // Allow that last row doesn't have the complete stride length
        if texture.len() < stride as usize * (height - 1) as usize + smallest_stride as usize {
            return Err(Error::TextureWrongSize {
                texture_size: texture.len(),
                frame: format!("Stride size: {stride} Image size: {width} x {height}"),
            });
        }

        if smallest_stride != stride {
            let old_stride = stride as usize;
            let new_stride = smallest_stride as usize;

            let height_ = height as usize;
            let mut source = vec![0; new_stride];

            for row in 0..height_ {
                let old_row_begin = row * old_stride;
                let old_row_end = old_row_begin + new_stride;
                source.copy_from_slice(&texture[old_row_begin..old_row_end]);

                let new_row_begin = row * new_stride;
                let new_row_end = new_row_begin + new_stride;
                texture[new_row_begin..new_row_end].copy_from_slice(&source);
            }

            texture.resize(new_stride * height_, 0);
        };

        let new_frame = Arc::new(NewFrame::new(
            self.config.clone(),
            width,
            height,
            memory_format,
            texture,
        ));

        self.new_frames.push(new_frame.clone());

        Ok(new_frame)
    }

    /// Encode an image
    pub async fn create(self) -> Result<EncodedImage, ErrorCtx> {
        let process_context = spin_up_encoder(
            self.mime_type.clone(),
            self.pool.clone(),
            &self.cancellable,
            &self.sandbox_selector,
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
        if !self.config.creator_encoding_quality {
            return Err(FeatureNotSupported);
        }

        self.encoding_options.quality = Some(quality);
        Ok(())
    }

    /// Set compression level
    ///
    /// This sets the lossless compression level. The range is from 0 (no
    /// compression) to 100 (highest compression).
    pub fn set_encoding_compression(&mut self, compression: u8) -> Result<(), FeatureNotSupported> {
        if !self.config.creator_encoding_compression {
            return Err(FeatureNotSupported);
        }

        self.encoding_options.compression = Some(compression);
        Ok(())
    }

    pub fn set_metadata_key_value(
        &mut self,
        key_value: BTreeMap<String, String>,
    ) -> Result<(), FeatureNotSupported> {
        if !self.config.creator_metadata_key_value {
            return Err(FeatureNotSupported);
        }

        self.new_image.image_info.metadata_key_value = Some(key_value);
        Ok(())
    }

    pub fn add_metadata_key_value(
        &mut self,
        key: String,
        value: String,
    ) -> Result<(), FeatureNotSupported> {
        if !self.config.creator_metadata_key_value {
            return Err(FeatureNotSupported);
        }

        let mut key_value = self
            .new_image
            .image_info
            .metadata_key_value
            .clone()
            .unwrap_or_default();
        key_value.insert(key, value);
        self.new_image.image_info.metadata_key_value = Some(key_value);
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
    config: ImageEditorConfig,
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
            config,
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

    pub fn set_color_icc_profile(
        &self,
        icc_profile: Option<Vec<u8>>,
    ) -> Result<(), FeatureNotSupported> {
        if !self.config.creator_color_icc_profile {
            return Err(FeatureNotSupported);
        }

        *self.icc_profile.lock().unwrap() = icc_profile;
        Ok(())
    }

    fn frame(&self) -> Result<glycin_utils::Frame, Error> {
        let texture = BinaryData::from_data(&self.texture)?;
        let mut frame =
            glycin_utils::Frame::new(self.width, self.height, self.memory_format, texture)?;

        frame.details = self.details.clone();

        if let Some(icc_profile) = self.icc_profile.lock().unwrap().as_ref() {
            let icc_profile = BinaryData::from_data(icc_profile)?;
            frame.details.color_icc_profile = Some(icc_profile);
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

    pub fn data_ref(&self) -> Result<glycin_common::BinaryDataRef, std::io::Error> {
        self.inner.data.get()
    }

    pub fn data_full(&self) -> Result<Vec<u8>, std::io::Error> {
        self.inner.data.get_full()
    }
}
