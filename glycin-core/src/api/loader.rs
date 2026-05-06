use std::pin::Pin;
use std::sync::{Arc, Mutex};

#[cfg(feature = "builtin")]
use futures_util::FutureExt;
use gio::glib;
use gio::prelude::*;
pub use glycin_common::MemoryFormat;
use glycin_common::{MemoryFormatInfo, MemoryFormatSelection};
#[cfg(feature = "builtin")]
use glycin_utils::LoaderImplementation;
use glycin_utils::safe_math::*;
use glycin_utils::{ByteData, FungibleMemory};
use gufo_common::cicp::Cicp;
use gufo_common::orientation::{Orientation, Rotation};
use util::{CancellableFuture, ShortcutErrorFuture, TimeoutFuture};
#[cfg(feature = "external")]
use zbus::zvariant::OwnedObjectPath;

use crate::api::*;
pub use crate::config::MimeType;
#[cfg(feature = "external")]
use crate::dbus::*;
use crate::error::ResultExt;
use crate::main_context::{MainContextSelector, ProvidesMainContext};
#[cfg(feature = "external")]
use crate::pool::{PooledProcess, UsageTracker};
use crate::source::SourceTransmission;
use crate::util::spawn_blocking;
use crate::{Error, ErrorKind, MAX_TEXTURE_SIZE, Pool, config, icc, orientation, util};

/// Image request builder
#[derive(Debug)]
pub struct Loader {
    source: Source,
    pool: Arc<Pool>,
    pub(crate) cancellable: gio::Cancellable,
    use_expose_base_dir: bool,
    pub(crate) apply_transformations: bool,
    pub(crate) sandbox_selector: SandboxSelector,
    pub(crate) memory_format_selection: MemoryFormatSelection,
    pub(crate) limits: Limits,
    pub(crate) main_context_selector: MainContextSelector,
}

static_assertions::assert_impl_all!(Loader: Send, Sync);

impl Loader {
    /// Create a loader with a [`gio::File`] as source
    pub fn new(file: gio::File) -> Self {
        Self::new_source(Source::File(file))
    }

    /// Create a loader with a [`gio::InputStream`] as source
    pub unsafe fn new_stream(stream: impl IsA<gio::InputStream>) -> Self {
        unsafe { Self::new_source(Source::Stream(GInputStreamSend::new(stream.upcast()))) }
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
            limits: Limits::default(),
            main_context_selector: MainContextSelector::Auto,
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
    /// different base directories, which has a noticeable performance impact
    /// when loading many small SVGs from many different directories.
    pub fn use_expose_base_dir(&mut self, use_epose_base_dir: bool) -> &mut Self {
        self.use_expose_base_dir = use_epose_base_dir;
        self
    }

    pub fn pool(&mut self, pool: Arc<Pool>) -> &mut Self {
        self.pool = pool;
        self
    }

    pub fn limits(&mut self, limits: Limits) -> &mut Self {
        self.limits = limits;
        self
    }

    pub fn main_context_selector(&mut self, selector: MainContextSelector) -> &mut Self {
        self.main_context_selector = selector;
        self
    }

    /// Load basic image information and enable further operations
    pub fn load(mut self) -> Pin<Box<dyn Future<Output = Result<Image, Error>> + Send>> {
        Box::pin(async {
            tracing::debug!(image = self.source.display(), "Loading image");

            let source = self.source.send();
            let main_context = self.main_context();
            let cancellable = self.cancellable.clone();
            let timeout = self.limits.inner.timeout;

            let f = move || {
                async move { self.load_internal(source).await }
                    .make_cancellable(cancellable)
                    .enforce_timeout(timeout)
            };

            main_context.spawn_from_within(f).await?
        })
    }

    async fn load_internal(self, source: Source) -> Result<Image, Error> {
        let loader_context =
            ProcessorContext::new(source, self.use_expose_base_dir, &self.sandbox_selector).await?;

        let loader = loader_context
            .loader(self.pool.clone(), &self.cancellable)
            .await?;

        match loader {
            #[cfg(feature = "external")]
            Processor::Binary(binary_loader) => self.load_internal_external(binary_loader).await,
            #[cfg(feature = "builtin")]
            Processor::Builtin(builtin) => self.load_internal_builtin(builtin).await,
        }
    }

    #[cfg(feature = "external")]
    async fn load_internal_external(
        self,
        binary_loader: ExternalProcessor<LoaderProxy<'static>, SourceTransmission>,
    ) -> Result<Image, Error> {
        tracing::debug!("Using external loader");

        let process = binary_loader.use_process();
        let (remote_reader, file_read_future) =
            binary_loader.source_transmission.spawn_external()?;

        let remote_image_future = process.init(&binary_loader.mime_type, remote_reader);

        // Drive reading the image source in parallel and shortcut if it errors
        let mut remote_image = remote_image_future
            .join_abort_on_error(file_read_future)
            .await
            .err_context(&process)?;

        remote_image.final_seal().await?;

        let mut details = remote_image.details.into_fungible();

        if self.apply_transformations {
            match Image::transformation_orientation_internal(&details).rotate() {
                Rotation::_90 | Rotation::_270 => {
                    std::mem::swap(&mut details.width, &mut details.height);
                }
                _ => {}
            }
        }

        let path = remote_image.frame_request.clone();
        self.cancellable.connect_cancelled(glib::clone!(
            #[strong(rename_to=process)]
            binary_loader.process,
            move |_| {
                tracing::debug!("Terminating loader");
                util::spawn_detached(process.use_().done(path))
            }
        ));

        let mime_type = binary_loader.mime_type.clone();

        let image_loader = ImageLoader::Binary(ImageExternalLoader {
            process: binary_loader.process,
            active_sandbox_mechanism: binary_loader.sandbox_mechanism,
            usage_tracker: Mutex::new(Some(binary_loader.usage_tracker)),
            frame_request: remote_image.frame_request,
        });

        Ok(Image {
            image_loader,
            details: Arc::new(details),
            loader: self,
            mime_type,
        })
    }

    #[cfg(feature = "builtin")]
    async fn load_internal_builtin<P: DBusProxy>(
        self,
        builtin: BuiltinProcessor<P, SourceTransmission>,
    ) -> Result<Image, Error> {
        tracing::debug!("Using builtin loader '{}'", builtin.builtin.common().name());

        let init_function: Box<dyn Fn(_, _, _) -> _ + Send>;

        match builtin.builtin {
            #[cfg(feature = "builtin-image-rs")]
            config::BuiltinProcessor::ImageRs(_) => {
                init_function = Box::new(|stream, mime_type, details| {
                    glycin_image_rs::ImgDecoder::load(stream, mime_type, details).map(
                        |(decoder, details)| {
                            (
                                ImageBuiltinLoader::ImageRs(Arc::new(Mutex::new(decoder))),
                                details,
                            )
                        },
                    )
                });
            }
            #[cfg(feature = "builtin-test")]
            config::BuiltinProcessor::Test(_) => {
                init_function = Box::new(|stream, mime_type, details| {
                    glycin_test::ImgDecoder::load(stream, mime_type, details).map(
                        |(decoder, details)| {
                            (
                                ImageBuiltinLoader::Test(Arc::new(Mutex::new(decoder))),
                                details,
                            )
                        },
                    )
                });
            }
        }

        let mime_type = builtin.mime_type.clone();

        let (source_reader, file_read_future) = builtin.source_transmission.spawn_builtin();

        let remote_image_future = gio::spawn_blocking(move || {
            init_function(
                source_reader,
                builtin.mime_type.to_string(),
                // TODO: That should be something different?
                glycin_utils::InitializationDetails::default(),
            )
            .map_err(|e| Error::from(e.into_loader_error()))
        })
        .map(|x| x.map_err(|_| ErrorKind::ThreadPanic.err()));

        let (image_loader, image_details) = remote_image_future
            .join_abort_on_error(file_read_future)
            .await??;

        Ok(Image {
            image_loader: ImageLoader::Builtin(image_loader),
            details: Arc::new(image_details),
            loader: self,
            mime_type,
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
        "image/x-win-bitmap",
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
    image_loader: ImageLoader,
    details: Arc<glycin_utils::ImageDetails<FungibleMemory>>,
    mime_type: MimeType,
}

static_assertions::assert_impl_all!(Image: Send, Sync);

impl Drop for Image {
    fn drop(&mut self) {
        #[cfg(feature = "external")]
        #[allow(irrefutable_let_patterns)]
        if let ImageLoader::Binary(image_loader) = &self.image_loader {
            let process = image_loader.process.clone();
            let path = self.frame_request_path();
            let loader_alive = std::mem::take(&mut *image_loader.usage_tracker.lock().unwrap());
            util::spawn_detached(async move {
                if let Err(err) = process.use_().done(path).await {
                    tracing::warn!("Failed to tear down loader: {err}")
                }

                drop(loader_alive);
            });
        }
    }
}

impl Image {
    /// Loads next frame
    ///
    /// Loads texture and information of the next frame. For single still
    /// images, this can only be called once. For animated images, this
    /// function will loop to the first frame, when the last frame is reached.
    pub fn next_frame<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<Frame, Error>> + 'a + Send>> {
        self.specific_frame(FrameRequest::default())
    }

    /// Loads a specific frame
    ///
    /// Loads a specific frame from the file. Loaders can ignore parts of the
    /// instructions in the `FrameRequest`.
    pub fn specific_frame<'a>(
        &'a self,
        frame_request: FrameRequest,
    ) -> Pin<Box<dyn Future<Output = Result<Frame, Error>> + 'a + Send>> {
        Box::pin(async move {
            let cancellable = self.loader.cancellable.clone();

            self.specific_frame_internal(frame_request)
                .make_cancellable(cancellable)
                .enforce_timeout(self.loader.limits.inner.timeout)
                .await
        })
    }

    async fn specific_frame_internal(&self, frame_request: FrameRequest) -> Result<Frame, Error> {
        let frame_request = frame_request.request;

        match &self.image_loader {
            #[cfg(feature = "external")]
            ImageLoader::Binary(image_loader) => {
                let process = image_loader.process.use_();

                let frame = process
                    .request_frame(frame_request, self)
                    .await
                    .err_context(&process)?;

                Frame::from_loader(frame, self).await
            }
            #[cfg(feature = "builtin")]
            ImageLoader::Builtin(builtin) => {
                use glycin_utils::LocalMemory;

                let editor_function: Box<dyn FnOnce() -> _ + Send>;

                match builtin {
                    #[cfg(feature = "builtin-image-rs")]
                    ImageBuiltinLoader::ImageRs(loader) => {
                        let loader: Arc<Mutex<glycin_image_rs::ImgDecoder>> = loader.to_owned();
                        editor_function = Box::new(move || {
                            loader
                                .lock()
                                .unwrap()
                                .specific_frame::<LocalMemory>(frame_request)
                        });
                    }
                    #[cfg(feature = "builtin-test")]
                    ImageBuiltinLoader::Test(editor) => {
                        let editor = editor.to_owned();
                        editor_function = Box::new(move || {
                            editor
                                .lock()
                                .unwrap()
                                .specific_frame::<LocalMemory>(frame_request)
                        });
                    }
                }

                let frame = gio::spawn_blocking(|| {
                    editor_function().map_err(|e| Error::from(e.into_loader_error()))
                })
                .await
                .map_err(|_| ErrorKind::ThreadPanic)??;

                Frame::from_loader(frame, self).await
            }
        }
    }

    /// Returns already obtained info
    pub fn details(&self) -> ImageDetails {
        ImageDetails::new(self.details.clone())
    }

    /// Returns already obtained info
    #[cfg(feature = "external")]
    pub(crate) fn frame_request_path(&self) -> OwnedObjectPath {
        #[allow(irrefutable_let_patterns)]
        if let ImageLoader::Binary(image_loader) = &self.image_loader {
            image_loader.frame_request.clone()
        } else {
            todo!()
        }
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
        match &self.image_loader {
            #[cfg(feature = "external")]
            ImageLoader::Binary(image_loader) => image_loader.active_sandbox_mechanism,
            #[cfg(feature = "builtin")]
            ImageLoader::Builtin(_) => SandboxMechanism::NotSandboxed,
        }
    }

    /// Tramsformations to be applied to orient image correctly
    ///
    /// If the [`Loader::apply_transformations`] has ben set to `false`, these
    /// transformations have to be applied to display the image correctly.
    /// Otherwise, they are applied automatically to the image after loading it.
    pub fn transformation_orientation(&self) -> Orientation {
        Self::transformation_orientation_internal(&self.details)
    }

    fn transformation_orientation_internal(
        details: &glycin_utils::ImageDetails<FungibleMemory>,
    ) -> Orientation {
        if let Some(orientation) = details.transformation_orientation {
            orientation
        } else if !details.transformation_ignore_exif {
            details
                .metadata_exif
                .as_ref()
                .map(|x| x.to_vec())
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

#[derive(Debug)]
enum ImageLoader {
    #[cfg(feature = "external")]
    Binary(ImageExternalLoader),
    #[cfg(feature = "builtin")]
    Builtin(ImageBuiltinLoader),
}

#[cfg(feature = "external")]
#[derive(Debug)]
struct ImageExternalLoader {
    process: Arc<PooledProcess<LoaderProxy<'static>>>,
    active_sandbox_mechanism: SandboxMechanism,
    usage_tracker: Mutex<Option<Arc<UsageTracker>>>,
    frame_request: OwnedObjectPath,
}

#[cfg(feature = "builtin")]
#[derive(Clone)]
enum ImageBuiltinLoader {
    #[cfg(feature = "builtin-image-rs")]
    ImageRs(Arc<Mutex<glycin_image_rs::ImgDecoder>>),
    #[cfg(feature = "builtin-test")]
    Test(Arc<Mutex<glycin_test::ImgDecoder>>),
}

#[cfg(feature = "builtin")]
impl std::fmt::Debug for ImageBuiltinLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ImageBuiltinLoader")
    }
}

#[derive(Debug, Clone)]
pub struct ImageDetails {
    inner: Arc<glycin_utils::ImageDetails<FungibleMemory>>,
}

static_assertions::assert_impl_all!(ImageDetails: Send, Sync);

impl ImageDetails {
    fn new(inner: Arc<glycin_utils::ImageDetails<FungibleMemory>>) -> Self {
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

    pub fn metadata_exif(&self) -> Option<&[u8]> {
        self.inner.metadata_exif.as_deref()
    }

    pub fn transformation_orientation(&self) -> Option<Orientation> {
        self.inner.transformation_orientation
    }

    pub fn metadata_xmp(&self) -> Option<&[u8]> {
        self.inner.metadata_xmp.as_deref()
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
    pub(crate) details: Arc<glycin_utils::FrameDetails<FungibleMemory>>,
    pub(crate) color_state: ColorState,
}

static_assertions::assert_impl_all!(Frame: Send, Sync);

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

    pub(crate) async fn from_loader<B: ByteData>(
        mut frame: glycin_utils::Frame<B>,
        image: &Image,
    ) -> Result<Self, Error> {
        frame.initial_seal().await?;

        validate_frame(&frame, &image.loader.limits)?;

        let frame = if image.loader.apply_transformations {
            orientation::apply_exif_orientation(frame.into_fungible(), image)
        } else {
            frame.into_fungible()
        };

        let mut color_state = ColorState::Srgb;

        let frame = if let Some(cicp) = frame
            .details
            .color_cicp
            .and_then(|x| Cicp::from_bytes(&x).ok())
        {
            color_state = ColorState::Cicp(cicp);
            frame
        } else if let Some(icc_profile) =
            frame.details.color_icc_profile.as_ref().map(|x| x.to_vec())
        {
            let (frame, icc_result) =
                spawn_blocking(move || icc::apply_transformation(&icc_profile, frame)).await;

            match icc_result {
                Err(err) => {
                    tracing::warn!("Failed to apply ICC profile: {err}");
                }
                Ok(new_color_state) => {
                    color_state = new_color_state;
                }
            }

            frame
        } else {
            frame
        };

        let mut frame = if let Some(target_format) = image
            .loader
            .memory_format_selection
            .best_format_for(frame.memory_format)
            && frame.memory_format != target_format
        {
            util::spawn_blocking(move || {
                glycin_utils::editing::change_memory_format(frame.into_fungible(), target_format)
            })
            .await?
        } else {
            frame.into_fungible()
        };

        frame.final_seal().await?;

        Ok(Self {
            buffer: frame.texture.into_gbytes()?,
            width: frame.width,
            height: frame.height,
            stride: frame.stride,
            memory_format: frame.memory_format,
            delay: frame.delay.into(),
            details: Arc::new(frame.details.into_other()?),
            color_state,
        })
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

fn validate_frame<B: ByteData>(
    frame: &glycin_utils::Frame<B>,
    limits: &Limits,
) -> Result<(), Error> {
    let img_buf = &frame.texture;

    if img_buf.len() < frame.n_bytes()? {
        return Err(ErrorKind::TextureWrongSize {
            texture_size: img_buf.len(),
            frame: format!("{:?}", frame.desc()),
        }
        .err());
    }

    if frame.stride < frame.width.smul(frame.memory_format.n_bytes().u32())? {
        return Err(ErrorKind::StrideTooSmall(format!("{:?}", frame.desc())).err());
    }

    if frame.width < 1 || frame.height < 1 {
        return Err(ErrorKind::WidgthOrHeightZero(format!("{:?}", frame.desc())).err());
    }

    if (frame.stride as u64).smul(frame.height as u64)? > MAX_TEXTURE_SIZE {
        return Err(ErrorKind::TextureTooLarge.err());
    }

    if frame.width > limits.inner.max_dimensions.0 {
        return Err(ErrorKind::TextureTooLarge.err());
    }

    if frame.height > limits.inner.max_dimensions.1 {
        return Err(ErrorKind::TextureTooLarge.err());
    }

    // Ensure
    frame.width.try_i32()?;
    frame.height.try_i32()?;
    frame.stride.try_usize()?;

    Ok(())
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
    inner: Arc<glycin_utils::FrameDetails<FungibleMemory>>,
}

impl FrameDetails {
    fn new(inner: Arc<glycin_utils::FrameDetails<FungibleMemory>>) -> Self {
        Self { inner }
    }

    pub fn color_cicp(&self) -> Option<crate::Cicp> {
        self.inner
            .color_cicp
            .and_then(|x| crate::Cicp::from_bytes(&x).ok())
    }

    pub fn color_icc_profile(&self) -> Option<&[u8]> {
        self.inner.color_icc_profile.as_deref()
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
