use std::path::Path;
use std::sync::OnceLock;

use async_global_executor::spawn_blocking;
use gio::glib;
use gio::prelude::*;
#[cfg(feature = "gdk4")]
use glycin_utils::save_math::*;
use glycin_utils::ImageInfo;
pub use glycin_utils::{FrameDetails, MemoryFormat};

pub use crate::config::MimeType;
use crate::dbus::*;
use crate::{config, Error};

static IS_FLATPAKED: OnceLock<bool> = OnceLock::new();

pub type Result<T> = std::result::Result<T, Error>;

async fn is_flatpaked() -> bool {
    if let Some(result) = IS_FLATPAKED.get() {
        *result
    } else {
        let flatpaked = spawn_blocking(|| Path::new("/.flatpak-info").is_file()).await;
        *IS_FLATPAKED.get_or_init(|| flatpaked)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SandboxMechanism {
    Bwrap,
    FlatpakSpawn,
    NotSandboxed,
}

impl SandboxMechanism {
    pub async fn detect() -> Self {
        if is_flatpaked().await {
            Self::FlatpakSpawn
        } else {
            Self::Bwrap
        }
    }

    pub fn into_selector(self) -> SandboxSelector {
        match self {
            Self::Bwrap => SandboxSelector::Bwrap,
            Self::FlatpakSpawn => SandboxSelector::FlatpakSpawn,
            Self::NotSandboxed => SandboxSelector::NotSandboxed,
        }
    }
}

#[derive(Debug, Copy, Clone, Default)]
#[cfg_attr(feature = "gobject", derive(gio::glib::Enum))]
#[cfg_attr(feature = "gobject", enum_type(name = "GlySandboxSelector"))]
#[repr(i32)]
pub enum SandboxSelector {
    #[default]
    Auto,
    Bwrap,
    FlatpakSpawn,
    NotSandboxed,
}

impl SandboxSelector {
    pub async fn determine_sandbox_mechanism(self) -> SandboxMechanism {
        match self {
            Self::Auto => SandboxMechanism::detect().await,
            Self::Bwrap => SandboxMechanism::Bwrap,
            Self::FlatpakSpawn => SandboxMechanism::FlatpakSpawn,
            Self::NotSandboxed => SandboxMechanism::NotSandboxed,
        }
    }
}

/// Image request builder
#[derive(Debug)]
pub struct Loader {
    file: gio::File,
    cancellable: gio::Cancellable,
    pub(crate) apply_transformations: bool,
    pub(crate) sandbox_mechanism: SandboxSelector,
}

impl Loader {
    /// Create a new loader
    pub fn new(file: gio::File) -> Self {
        Self {
            file,
            cancellable: gio::Cancellable::new(),
            apply_transformations: true,
            sandbox_mechanism: SandboxSelector::default(),
        }
    }

    /// Change the sandbox mechanism
    ///
    /// The default without calling this function is to automatically select a
    /// sandbox mechanism. The sandbox is never disabled automatically.
    pub fn sandbox_mechanism(&mut self, sandbox_mechanism: Option<SandboxMechanism>) -> &mut Self {
        self.sandbox_mechanism =
            sandbox_mechanism.map_or(SandboxSelector::Auto, |x| x.into_selector());
        self
    }

    /// Set [`Cancellable`](gio::Cancellable) to cancel any loader operations
    pub fn cancellable(&mut self, cancellable: impl IsA<gio::Cancellable>) -> &mut Self {
        self.cancellable = cancellable.upcast();
        self
    }

    /// Set whether to apply transformations to texture
    ///
    /// When enabled, transformations like image orientation are applied to the
    /// texture data.
    ///
    /// This option is enabled by default.
    pub fn apply_transformations(&mut self, apply_transformations: bool) -> &mut Self {
        self.apply_transformations = apply_transformations;
        self
    }

    /// Load basic image information and enable further operations
    pub async fn load<'a>(self) -> Result<Image<'a>> {
        let config = config::Config::cached().await;

        let gfile_worker = GFileWorker::spawn(self.file.clone(), self.cancellable.clone());
        let mime_type = Self::guess_mime_type(&gfile_worker).await?;
        let decoder_config = config.get(&mime_type)?;

        let sandbox_mechanism = self.sandbox_mechanism.determine_sandbox_mechanism().await;

        let base_dir = if decoder_config.expose_base_dir {
            self.file.parent().and_then(|x| x.path())
        } else {
            None
        };

        let process = DecoderProcess::new(
            &mime_type,
            config,
            sandbox_mechanism,
            &self.file,
            self.cancellable.as_ref(),
        )
        .await?;

        let info = process.init(gfile_worker, base_dir).await?;

        Ok(Image {
            process,
            info,
            loader: self,
            mime_type,
            active_sandbox_mechanism: sandbox_mechanism,
        })
    }

    async fn guess_mime_type(gfile_worker: &GFileWorker) -> Result<String> {
        let head = gfile_worker.head().await?;
        let (content_type, unsure) = gio::content_type_guess(None::<String>, &head);
        let mime_type = gio::content_type_get_mime_type(&content_type)
            .ok_or_else(|| Error::UnknownImageFormat(content_type.to_string()));

        // Prefer file extension for TIFF since it can be a RAW format as well
        let is_tiff = mime_type.clone().ok() == Some("image/tiff".into());

        // Prefer file extension for XML since long comment between `<?xml` and `<svg>`
        // can falsely guess XML instead of SVG
        let is_xml = mime_type.clone().ok() == Some("application/xml".into());

        if unsure || is_tiff || is_xml {
            if let Some(filename) = gfile_worker.file().basename() {
                let content_type_fn = gio::content_type_guess(Some(filename), &head).0;
                return gio::content_type_get_mime_type(&content_type_fn)
                    .ok_or_else(|| Error::UnknownImageFormat(content_type_fn.to_string()))
                    .map(|x| x.to_string());
            }
        }

        mime_type.map(|x| x.to_string())
    }
}

/// Image handle containing metadata and allowing frame requests
#[derive(Debug)]
pub struct Image<'a> {
    pub(crate) loader: Loader,
    process: DecoderProcess<'a>,
    info: ImageInfo,
    mime_type: MimeType,
    active_sandbox_mechanism: SandboxMechanism,
}

impl<'a> Image<'a> {
    /// Loads next frame
    ///
    /// Loads texture and information of the next frame. For single still
    /// images, this can only be called once. For animated images, this
    /// function will loop to the first frame, when the last frame is reached.
    pub async fn next_frame(&self) -> Result<Frame> {
        self.process
            .request_frame(glycin_utils::FrameRequest::default(), self)
            .await
            .map_err(Into::into)
    }

    /// Loads a specific frame
    ///
    /// Loads a specific frame from the file. Loaders can ignore parts of the
    /// instructions in the `FrameRequest`.
    pub async fn specific_frame(&self, frame_request: FrameRequest) -> Result<Frame> {
        self.process
            .request_frame(frame_request.request, self)
            .await
            .map_err(Into::into)
    }

    /// Returns already obtained info
    pub fn info(&self) -> &ImageInfo {
        &self.info
    }

    /// Returns detected MIME type of the file
    pub fn mime_type(&self) -> MimeType {
        self.mime_type.clone()
    }

    /// A textual representation of the image format
    pub fn format_name(&self) -> Option<String> {
        self.info().details.format_name.as_ref().cloned()
    }

    /// File the image was loaded from
    pub fn file(&self) -> gio::File {
        self.loader.file.clone()
    }

    /// [`Cancellable`](gio::Cancellable) to cancel operations within this image
    pub fn cancellable(&self) -> gio::Cancellable {
        self.loader.cancellable.clone()
    }

    /// Active sandbox mechanis
    pub fn active_sandbox_mechanism(&self) -> SandboxMechanism {
        self.active_sandbox_mechanism
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        self.cancellable.cancel();
    }
}

/// A frame of an image often being the complete image
#[derive(Debug, Clone)]
pub struct Frame {
    pub(crate) buffer: glib::Bytes,
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Line stride
    pub(crate) stride: u32,
    pub(crate) memory_format: MemoryFormat,
    pub(crate) delay: Option<std::time::Duration>,
    pub(crate) details: FrameDetails,
}

impl Frame {
    pub fn buf_bytes(&self) -> glib::Bytes {
        self.buffer.clone()
    }

    pub fn buf_slice(&self) -> &[u8] {
        self.buffer.as_ref()
    }

    /// Width in pixels
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Line stride in bytes
    pub fn stride(&self) -> u32 {
        self.stride
    }

    pub fn memory_format(&self) -> MemoryFormat {
        self.memory_format
    }

    /// Duration to show frame for animations.
    ///
    /// If the value is not set, the image is not animated.
    pub fn delay(&self) -> Option<std::time::Duration> {
        self.delay
    }

    pub fn details(&self) -> &FrameDetails {
        &self.details
    }

    #[cfg(feature = "gdk4")]
    pub fn texture(&self) -> gdk::Texture {
        // Use unwraps here since the compatibility was checked before
        gdk::MemoryTexture::new(
            self.width().try_i32().unwrap(),
            self.height().try_i32().unwrap(),
            crate::util::gdk_memory_format(self.memory_format()),
            &self.buffer,
            self.stride().try_usize().unwrap(),
        )
        .upcast()
    }
}

#[derive(Default, Debug)]
#[must_use]
/// Request information to get a specific frame
pub struct FrameRequest {
    request: glycin_utils::FrameRequest,
}

impl FrameRequest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scale(mut self, width: u32, height: u32) -> Self {
        self.request.scale = Some((width, height));
        self
    }

    pub fn clip(mut self, x: u32, y: u32, width: u32, height: u32) -> Self {
        self.request.clip = Some((x, y, width, height));
        self
    }
}

/// Returns a list of mime types for which loaders are configured
pub async fn supported_mime_types() -> Vec<MimeType> {
    config::Config::cached()
        .await
        .image_decoders
        .keys()
        .cloned()
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    #[allow(dead_code)]
    fn ensure_futures_are_send() {
        gio::glib::spawn_future(async {
            let loader = Loader::new(gio::File::for_uri("invalid"));
            let image = loader.load().await.unwrap();
            image.next_frame().await.unwrap();
        });
    }
}
