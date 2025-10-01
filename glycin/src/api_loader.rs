use std::sync::{Arc, Mutex};

use gio::glib;
use gio::prelude::*;
pub use glycin_common::MemoryFormat;
use glycin_common::{BinaryData, MemoryFormatSelection};
#[cfg(feature = "gdk4")]
use glycin_utils::safe_math::*;
use gufo_common::orientation::{Orientation, Rotation};
use zbus::zvariant::OwnedObjectPath;

use crate::api_common::*;
pub use crate::config::MimeType;
use crate::dbus::*;
use crate::error::ResultExt;
use crate::pool::{Pool, PooledProcess, UsageTracker};
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

    pub fn pool(&mut self, pool: Arc<Pool>) -> &mut Self {
        self.pool = pool;
        self
    }

    /// Load basic image information and enable further operations
    pub async fn load(mut self) -> Result<Image, ErrorCtx> {
        let source = self.source.send();

        let process_basics = spin_up_loader(
            source,
            self.use_expose_base_dir,
            self.pool.clone(),
            &self.cancellable,
            &self.sandbox_selector,
        )
        .await
        .err_no_context(&self.cancellable)?;

        let process = process_basics.process.use_();
        let mut remote_image = process
            .init(
                process_basics.g_file_worker.unwrap(),
                &process_basics.mime_type,
            )
            .await
            .err_context(&process, &self.cancellable)?;

        if self.apply_transformations {
            match Image::transformation_orientation_internal(&remote_image.details).rotate() {
                Rotation::_90 | Rotation::_270 => {
                    std::mem::swap(
                        &mut remote_image.details.width,
                        &mut remote_image.details.height,
                    );
                }
                _ => {}
            }
        }

        let path = remote_image.frame_request.clone();
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
            frame_request: remote_image.frame_request,
            details: Arc::new(remote_image.details),
            loader: self,
            mime_type: process_basics.mime_type,
            active_sandbox_mechanism: process_basics.sandbox_mechanism,
            usage_tracker: Mutex::new(Some(process_basics.usage_tracker)),
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
    usage_tracker: Mutex<Option<Arc<UsageTracker>>>,
}

static_assertions::assert_impl_all!(Image: Send, Sync);

impl Drop for Image {
    fn drop(&mut self) {
        let process = self.process.clone();
        let path = self.frame_request_path();
        let loader_alive = std::mem::take(&mut *self.usage_tracker.lock().unwrap());
        spawn_detached(async move {
            if let Err(err) = process.use_().done(path).await {
                tracing::warn!("Failed to tear down loader: {err}")
            }

            drop(loader_alive);
        });
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

        let mut frame_request = glycin_utils::FrameRequest::default();
        frame_request.loop_animation = true;

        process
            .request_frame(frame_request, self)
            .await
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

    /// Tramsformations to be applied to orient image correctly
    ///
    /// If the [`Loader::apply_transformations`] has ben set to `false`, these
    /// transformations have to be applied to display the image correctly.
    /// Otherwise, they are applied automatically to the image after loading it.
    pub fn transformation_orientation(&self) -> Orientation {
        Self::transformation_orientation_internal(&self.details)
    }

    fn transformation_orientation_internal(details: &glycin_utils::ImageDetails) -> Orientation {
        if let Some(orientation) = details.transformation_orientation {
            orientation
        } else if !details.transformation_ignore_exif {
            details
                .metadata_exif
                .as_ref()
                .and_then(|x| x.get_full().ok())
                .and_then(|x| match gufo_exif::Exif::new(x) {
                    Err(err) => {
                        tracing::warn!("exif: Failed to parse data: {err:?}");
                        None
                    }
                    Ok(x) => x.orientation(),
                })
                .unwrap_or(Orientation::Id)
        } else {
            Orientation::Id
        }
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
        self.inner.info_format_name.as_deref()
    }

    pub fn info_dimensions_text(&self) -> Option<&str> {
        self.inner.info_dimensions_text.as_deref()
    }

    pub fn metadata_exif(&self) -> Option<BinaryData> {
        self.inner.metadata_exif.clone()
    }

    pub fn transformation_orientation(&self) -> Option<Orientation> {
        self.inner.transformation_orientation
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
        let color_state = crate::util::gdk_color_state(&self.color_state).unwrap_or_else(|_| {
            tracing::warn!("Unsupported color state: {:?}", self.color_state);
            gdk::ColorState::srgb()
        });

        gdk::MemoryTextureBuilder::new()
            .set_bytes(Some(&self.buffer))
            // Use unwraps here since the compatibility was checked before
            .set_width(self.width().try_i32().unwrap())
            .set_height(self.height().try_i32().unwrap())
            .set_stride(self.stride().try_usize().unwrap())
            .set_format(crate::util::gdk_memory_format(self.memory_format()))
            .set_color_state(&color_state)
            .build()
    }
}

#[derive(Debug, Clone)]
#[must_use]
/// Request information to get a specific frame
pub struct FrameRequest {
    pub(crate) request: glycin_utils::FrameRequest,
}

impl Default for FrameRequest {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameRequest {
    pub fn new() -> Self {
        let mut request = glycin_utils::FrameRequest::default();
        request.loop_animation = true;

        Self { request }
    }

    pub fn scale(mut self, width: u32, height: u32) -> Self {
        self.request.scale = Some((width, height));
        self
    }

    pub fn clip(mut self, x: u32, y: u32, width: u32, height: u32) -> Self {
        self.request.clip = Some((x, y, width, height));
        self
    }

    /// Controls if first frame is returned after last frame
    ///
    /// By default, this option is set to `true`, returning the first frame, if
    /// the previously requested frame was the last frame.
    pub fn loop_animation(mut self, loop_animation: bool) -> Self {
        self.request.loop_animation = loop_animation;
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

    pub fn color_cicp(&self) -> Option<crate::Cicp> {
        self.inner
            .color_cicp
            .and_then(|x| crate::Cicp::from_bytes(&x).ok())
    }

    pub fn color_icc_profile(&self) -> Option<BinaryData> {
        self.inner.color_icc_profile.clone()
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
