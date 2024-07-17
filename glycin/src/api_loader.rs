use gio::glib;
use gio::prelude::*;
#[cfg(feature = "gdk4")]
use glycin_utils::save_math::*;
use glycin_utils::ImageInfo;
pub use glycin_utils::{FrameDetails, MemoryFormat};

use crate::api_common::*;
pub use crate::config::MimeType;
use crate::dbus::*;
use crate::error::ResultExt;
use crate::{config, ErrorCtx};

/// Image request builder
#[derive(Debug)]
pub struct Loader {
    file: gio::File,
    cancellable: gio::Cancellable,
    pub(crate) apply_transformations: bool,
    pub(crate) sandbox_selector: SandboxSelector,
}

static_assertions::assert_impl_all!(Loader: Send, Sync);

impl Loader {
    /// Create a new loader
    pub fn new(file: gio::File) -> Self {
        Self {
            file,
            cancellable: gio::Cancellable::new(),
            apply_transformations: true,
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
    pub async fn load<'a>(self) -> Result<Image<'a>, ErrorCtx> {
        let process_context = spin_up(&self.file, &self.cancellable, &self.sandbox_selector)
            .await
            .err_no_context()?;

        let process = process_context.process;
        let info = process
            .init(process_context.gfile_worker, process_context.base_dir)
            .await
            .err_context(&process)?;

        Ok(Image {
            process,
            info,
            loader: self,
            mime_type: process_context.mime_type,
            active_sandbox_mechanism: process_context.sandbox_mechanism,
        })
    }
}

/// Image handle containing metadata and allowing frame requests
#[derive(Debug)]
pub struct Image<'a> {
    pub(crate) loader: Loader,
    process: RemoteProcess<'a, LoaderProxy<'a>>,
    info: ImageInfo,
    mime_type: MimeType,
    active_sandbox_mechanism: SandboxMechanism,
}

static_assertions::assert_impl_all!(Image: Send, Sync);

impl<'a> Image<'a> {
    /// Loads next frame
    ///
    /// Loads texture and information of the next frame. For single still
    /// images, this can only be called once. For animated images, this
    /// function will loop to the first frame, when the last frame is reached.
    pub async fn next_frame(&self) -> Result<Frame, ErrorCtx> {
        self.process
            .request_frame(glycin_utils::FrameRequest::default(), self)
            .await
            .map_err(Into::into)
            .err_context(&self.process)
    }

    /// Loads a specific frame
    ///
    /// Loads a specific frame from the file. Loaders can ignore parts of the
    /// instructions in the `FrameRequest`.
    pub async fn specific_frame(&self, frame_request: FrameRequest) -> Result<Frame, ErrorCtx> {
        self.process
            .request_frame(frame_request.request, self)
            .await
            .map_err(Into::into)
            .err_context(&self.process)
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

    /// Active sandbox mechanism
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
pub async fn supported_loader_mime_types() -> Vec<MimeType> {
    config::Config::cached()
        .await
        .image_loader
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
