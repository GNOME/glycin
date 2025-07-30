use std::sync::{Arc, Mutex};

use gio::glib;
use gio::glib::clone::Downgrade;
use gio::prelude::*;
pub use glycin_common::MemoryFormat;
use glycin_common::{BinaryData, MemoryFormatSelection};
#[cfg(feature = "gdk4")]
use glycin_utils::safe_math::*;
use zbus::zvariant::OwnedObjectPath;

use crate::api_common::*;
pub use crate::config::MimeType;
use crate::dbus::*;
use crate::error::ResultExt;
use crate::pool::{Pool, PooledProcess};
use crate::util::spawn_detached;
use crate::{config, ErrorCtx};

/// Image request builder
#[derive(Debug)]
pub struct Loader {
    source: Source,
    pool: Arc<Pool>,
    cancellable: gio::Cancellable,
    use_expose_base_dir: bool,
    pub(crate) apply_transformations: bool,
    pub(crate) sandbox_selector: SandboxSelector,
    pub(crate) memory_format_selection: MemoryFormatSelection,
}

static_assertions::assert_impl_all!(Loader: Send, Sync);

impl Loader {
    /// Create a loader with a [`gio::File`] as source
    pub fn new(file: gio::File) -> Self {
        Self::new_source(Source::File(file))
    }

    /// Create a loader with a [`gio::InputStream`] as source
    pub unsafe fn new_stream(stream: impl IsA<gio::InputStream>) -> Self {
        Self::new_source(Source::Stream(GInputStreamSend::new(stream.upcast())))
    }

    /// Create a loader with [`glib::Bytes`] as source
    pub fn new_bytes(bytes: glib::Bytes) -> Self {
        let stream = gio::MemoryInputStream::from_bytes(&bytes);
        unsafe { Self::new_stream(stream) }
    }

    /// Create a loader with [`Vec<u8>`] as source
    pub fn new_vec(buf: Vec<u8>) -> Self {
        let bytes = glib::Bytes::from_owned(buf);
        Self::new_bytes(bytes)
    }

    pub(crate) fn new_source(source: Source) -> Self {
        Self {
            source,
            pool: Pool::global(),
            cancellable: gio::Cancellable::new(),
            apply_transformations: true,
            use_expose_base_dir: false,
            sandbox_selector: SandboxSelector::default(),
            memory_format_selection: MemoryFormatSelection::all(),
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

    /// Sets which memory formats can be returned by the loader
    ///
    /// If the memory format doesn't match one of the selected formats, the
    /// format will be transformed into the best suitable format selected.
    pub fn accepted_memory_formats(
        &mut self,
        memory_format_selection: MemoryFormatSelection,
    ) -> &mut Self {
        self.memory_format_selection = memory_format_selection;
        self
    }

    /// Sets if the file's directory can be exposed to loaders
    ///
    /// Some loaders have the `use_base_dir` option enabled to load external
    /// files. One example is SVGs which can display external images inside the
    /// picture. By default, `use_expose_base_dir` is set to `false`. You need
    /// to enable it for the `use_base_dir` option to have any effect. The
    /// downside of enabling it is that separate sandboxes are needed for
    /// different base directories, which has a noticable performance impact
    /// when loading many small SVGs from many different directories.
    pub fn use_expose_base_dir(&mut self, use_epose_base_dir: bool) -> &mut Self {
        self.use_expose_base_dir = use_epose_base_dir;
        self
    }

    /// Load basic image information and enable further operations
    pub async fn load(mut self) -> Result<Image, ErrorCtx> {
        let source = self.source.send();

        let loader_alive = Arc::new(());

        let process_basics = spin_up_loader(
            source,
            self.use_expose_base_dir,
            &self.pool,
            &self.cancellable,
            &self.sandbox_selector,
            loader_alive.downgrade(),
        )
        .await
        .err_no_context(&self.cancellable)?;

        let process = process_basics.process.use_();
        let info = process
            .init(
                process_basics.g_file_worker.unwrap(),
                &process_basics.mime_type,
            )
            .await
            .err_context(&process, &self.cancellable)?;

        let path = info.frame_request.clone();
        self.cancellable.connect_cancelled(glib::clone!(
            #[strong(rename_to=process)]
            process_basics.process,
            move |_| {
                tracing::debug!("Terminating loader");
                crate::util::spawn_detached(process.use_().done(path))
            }
        ));

        Ok(Image {
            process: process_basics.process,
            frame_request: info.frame_request,
            details: Arc::new(info.details),
            loader: self,
            mime_type: process_basics.mime_type,
            active_sandbox_mechanism: process_basics.sandbox_mechanism,
            loader_alive: Default::default(),
        })
    }

    /// Returns a list of mime types for which loaders are configured
    pub async fn supported_mime_types() -> Vec<MimeType> {
        config::Config::cached()
            .await
            .image_loader
            .keys()
            .cloned()
            .collect()
    }

    /// Formats that the default glycin loaders support
    pub const DEFAULT_MIME_TYPES: &'static [&'static str] = &[
        // image-rs
        "image/jpeg",
        "image/png",
        "image/gif",
        "image/webp",
        "image/tiff",
        "image/x-tga",
        "image/vnd-ms.dds",
        "image/x-dds",
        "image/bmp",
        "image/vnd.microsoft.icon",
        "image/vnd.radiance",
        "image/x-exr",
        "image/x-portable-bitmap",
        "image/x-portable-graymap",
        "image/x-portable-pixmap",
        "image/x-portable-anymap",
        "image/x-qoi",
        "image/qoi",
        // HEIF
        "image/avif",
        "image/heif",
        // JXL
        "image/jxl",
        // SVG
        "image/svg+xml",
        "image/svg+xml-compressed",
    ];
}

/// Image handle containing metadata and allowing frame requests
#[derive(Debug)]
pub struct Image {
    pub(crate) loader: Loader,
    pub(crate) process: Arc<PooledProcess<LoaderProxy<'static>>>,
    frame_request: OwnedObjectPath,
    details: Arc<glycin_utils::ImageDetails>,
    mime_type: MimeType,
    active_sandbox_mechanism: SandboxMechanism,
    loader_alive: Mutex<Arc<()>>,
}

static_assertions::assert_impl_all!(Image: Send, Sync);

impl Drop for Image {
    fn drop(&mut self) {
        self.process.use_().done_background(&self);
        *self.loader_alive.lock().unwrap() = Arc::new(());
        spawn_detached(self.loader.pool.clone().clean_loaders());
    }
}

impl Image {
    /// Loads next frame
    ///
    /// Loads texture and information of the next frame. For single still
    /// images, this can only be called once. For animated images, this
    /// function will loop to the first frame, when the last frame is reached.
    pub async fn next_frame(&self) -> Result<Frame, ErrorCtx> {
        let process = self.process.use_();

        process
            .request_frame(glycin_utils::FrameRequest::default(), self)
            .await
            .map_err(Into::into)
            .err_context(&process, &self.cancellable())
    }

    /// Loads a specific frame
    ///
    /// Loads a specific frame from the file. Loaders can ignore parts of the
    /// instructions in the `FrameRequest`.
    pub async fn specific_frame(&self, frame_request: FrameRequest) -> Result<Frame, ErrorCtx> {
        let process = self.process.use_();

        process
            .request_frame(frame_request.request, self)
            .await
            .map_err(Into::into)
            .err_context(&process, &self.cancellable())
    }

    /// Returns already obtained info
    pub fn details(&self) -> ImageDetails {
        ImageDetails::new(self.details.clone())
    }

    /// Returns already obtained info
    pub(crate) fn frame_request_path(&self) -> OwnedObjectPath {
        self.frame_request.clone()
    }

    /// Returns detected MIME type of the file
    pub fn mime_type(&self) -> MimeType {
        self.mime_type.clone()
    }

    /// File the image was loaded from
    ///
    /// Is `None` if the file was loaded from a stream or binary data.
    pub fn file(&self) -> Option<gio::File> {
        self.loader.source.file()
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

#[derive(Debug, Clone)]
pub struct ImageDetails {
    inner: Arc<glycin_utils::ImageDetails>,
}

impl ImageDetails {
    fn new(inner: Arc<glycin_utils::ImageDetails>) -> Self {
        Self { inner }
    }

    pub fn width(&self) -> u32 {
        self.inner.width
    }

    pub fn height(&self) -> u32 {
        self.inner.height
    }

    pub fn dimensions_inch(&self) -> Option<(f64, f64)> {
        self.inner.dimensions_inch
    }

    /// A textual representation of the image format
    pub fn info_format_name(&self) -> Option<&str> {
        self.inner.info_format_name.as_ref().map(|x| x.as_str())
    }

    pub fn info_dimensions_text(&self) -> Option<&str> {
        self.inner.info_dimensions_text.as_ref().map(|x| x.as_str())
    }

    pub fn metadata_exif(&self) -> Option<BinaryData> {
        self.inner.metadata_exif.clone()
    }

    pub fn metadata_orientation(&self) -> Option<u16> {
        self.inner.metadata_orientation
    }

    pub fn metadata_xmp(&self) -> Option<BinaryData> {
        self.inner.metadata_xmp.clone()
    }

    pub fn metadata_key_value(&self) -> Option<&std::collections::BTreeMap<String, String>> {
        self.inner.metadata_key_value.as_ref()
    }

    pub fn transformation_ignore_exif(&self) -> bool {
        self.inner.transformation_ignore_exif
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
    pub(crate) details: Arc<glycin_utils::FrameDetails>,
    pub(crate) color_state: ColorState,
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

    pub fn color_state(&self) -> &ColorState {
        &self.color_state
    }

    /// Duration to show frame for animations.
    ///
    /// If the value is not set, the image is not animated.
    pub fn delay(&self) -> Option<std::time::Duration> {
        self.delay
    }

    pub fn details(&self) -> FrameDetails {
        FrameDetails::new(self.details.clone())
    }

    #[cfg(feature = "gdk4")]
    pub fn texture(&self) -> gdk::Texture {
        use crate::memory_texture_builder::MemoryTextureBuilder;

        let builder = MemoryTextureBuilder::new();

        builder.set_bytes(Some(&self.buffer));

        // Use unwraps here since the compatibility was checked before
        builder.set_width(self.width().try_i32().unwrap());
        builder.set_height(self.height().try_i32().unwrap());
        builder.set_stride(self.stride().try_usize().unwrap());

        builder.set_format(crate::util::gdk_memory_format(self.memory_format()));

        let color_state = crate::util::gdk_color_state(&self.color_state).unwrap_or_else(|_| {
            tracing::warn!("Unsupported color state: {:?}", self.color_state);
            gdk::ColorState::srgb()
        });

        builder.set_color_state(Some(&color_state));

        builder.build()
    }
}

#[derive(Default, Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct FrameDetails {
    inner: Arc<glycin_utils::FrameDetails>,
}

impl FrameDetails {
    fn new(inner: Arc<glycin_utils::FrameDetails>) -> Self {
        Self { inner }
    }

    pub fn color_cicp(&self) -> Option<&[u8]> {
        self.inner.color_cicp.as_ref().map(|x| x.as_slice())
    }

    pub fn color_iccp(&self) -> Option<BinaryData> {
        self.inner.color_iccp.clone()
    }

    pub fn info_alpha_channel(&self) -> Option<bool> {
        self.inner.info_alpha_channel
    }

    pub fn info_bit_depth(&self) -> Option<u8> {
        self.inner.info_bit_depth
    }

    pub fn info_grayscale(&self) -> Option<bool> {
        self.inner.info_grayscale
    }

    pub fn n_frame(&self) -> Option<u64> {
        self.inner.n_frame
    }
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
