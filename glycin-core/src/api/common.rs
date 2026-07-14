#[cfg(feature = "builtin")]
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(feature = "gobject")]
use gio::glib;
use gio::prelude::*;

use crate::config::{Config, ImageEditorConfig, ImageLoaderConfig};
#[cfg(feature = "external")]
use crate::dbus::ZbusProxy;
use crate::dbus::{EditorProxy, LoaderProxy};
#[cfg(feature = "external")]
use crate::pool::{PooledProcess, UsageTracker};
use crate::source::SourceTransmission;
use crate::util::RunEnvironment;
use crate::{Error, ErrorKind, MimeType, Pool, config};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
/// Sandboxing mechanism for image loading and editing
pub enum SandboxMechanism {
    Bwrap,
    FlatpakSpawn,
    NotSandboxed,
}

impl SandboxMechanism {
    pub async fn detect() -> Self {
        match RunEnvironment::cached().await {
            RunEnvironment::SandboxForceDisabled => Self::NotSandboxed,
            RunEnvironment::FlatpakDevel => Self::NotSandboxed,
            RunEnvironment::Flatpak => Self::FlatpakSpawn,
            RunEnvironment::Host => Self::Bwrap,
            RunEnvironment::HostBwrapSyscallsBlocked => Self::NotSandboxed,
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
/// Method by which the [`SandboxMechanism`] is selected
pub enum SandboxSelector {
    #[default]
    /// This mode selects `bwrap` outside of Flatpaks and usually
    /// `flatpak-spawn` inside of Flatpaks. The sandbox is disabled
    /// automatically inside of Flatpak development environments. See
    /// details below.
    ///
    /// Inside of Flatpaks, `flatpak-spawn` is used to create the sandbox. This
    /// mechanism starts an installed Flatpak with the same app id. For
    /// development, Flatpak are usually not installed and the sandbox can
    /// therefore not be used. If the sandbox has been started via
    /// `flatpak-builder --run` (i.e. without installed Flatpak) and the app id
    /// ends with `Devel`, the sandbox is disabled.
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

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ColorState {
    Srgb,
    Cicp(crate::Cicp),
}

/// A version of an input stream that can be sent.
///
/// Using the stream from multiple threads is UB. Therefore the `new` function
/// is unsafe.
#[derive(Debug, Clone)]
pub(crate) struct GInputStreamSend(gio::InputStream);

unsafe impl Send for GInputStreamSend {}
unsafe impl Sync for GInputStreamSend {}

impl GInputStreamSend {
    pub(crate) unsafe fn new(stream: gio::InputStream) -> Self {
        Self(stream)
    }

    pub(crate) fn display(&self) -> String {
        self.0.type_().name().to_string()
    }
}

/// Image source for a loader/editor
#[derive(Debug, Clone)]
pub(crate) enum Source {
    File(gio::File),
    Stream(GInputStreamSend),
    TransferredStream,
}

impl Source {
    pub fn file(&self) -> Option<gio::File> {
        match self {
            Self::File(file) => Some(file.clone()),
            _ => None,
        }
    }

    pub async fn to_stream(&self) -> Result<gio::InputStream, Error> {
        match self {
            Self::File(file) => file
                .read_future(glib::Priority::DEFAULT)
                .await
                .map(|x| x.upcast())
                .map_err(|e| ErrorKind::ImageSource(e).err()),
            Self::Stream(stream) => Ok(stream.0.clone()),
            Self::TransferredStream => Err(ErrorKind::TransferredStream.into()),
        }
    }

    /// Get a [`Source`] for sending to [`GFileWorker`]
    ///
    /// This will remove the stored stream from `self` to avoid it getting used
    /// anywhere else than the [`GFileWorker`] it has been sent to.
    pub fn send(&mut self) -> Self {
        let new = self
            .file()
            .map(Self::File)
            .unwrap_or(Self::TransferredStream);

        std::mem::replace(self, new)
    }

    pub fn display(&self) -> String {
        match self {
            Self::File(file) => {
                if let Some(path) = file.path() {
                    path.display().to_string()
                } else {
                    file.uri().to_string()
                }
            }
            Self::Stream(stream) => {
                format!("Stream({})", stream.display())
            }
            Self::TransferredStream => String::from("TransferredStream"),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ProcessorContext<T: GetConfig, S> {
    pub mime_type: MimeType,
    pub sandbox_mechanism: SandboxMechanism,
    pub config_entry: T,
    pub g_file_worker: S,
    pub base_dir: Option<PathBuf>,
}

pub trait GetConfig {
    fn config_entry<'a>(config: &'a Config, mime_type: &'a MimeType) -> Result<&'a Self, Error>;
    fn expose_base_dir(&self) -> bool;
    fn guess_mime_type(config: &Config, path: Option<&Path>, head: &[u8]) -> Option<MimeType>;
}

impl GetConfig for ImageLoaderConfig {
    fn config_entry<'a>(
        config: &'a Config,
        mime_type: &'a MimeType,
    ) -> Result<&'a ImageLoaderConfig, Error> {
        config.loader(mime_type)
    }

    fn expose_base_dir(&self) -> bool {
        self.expose_base_dir
    }

    fn guess_mime_type(config: &Config, path: Option<&Path>, head: &[u8]) -> Option<MimeType> {
        Config::guess_mime_type(config, path, head, false)
    }
}

impl GetConfig for ImageEditorConfig {
    fn config_entry<'a>(
        config: &'a Config,
        mime_type: &'a MimeType,
    ) -> Result<&'a ImageEditorConfig, Error> {
        config.editor(mime_type)
    }

    fn expose_base_dir(&self) -> bool {
        self.expose_base_dir
    }

    fn guess_mime_type(config: &Config, path: Option<&Path>, head: &[u8]) -> Option<MimeType> {
        Config::guess_mime_type(config, path, head, true)
    }
}

impl<T: GetConfig + Clone> ProcessorContext<T, SourceTransmission> {
    /// Determines mime-type, relevant config entry, and sandboxing mode
    ///
    /// Also spawns the file worker since we need to read from the file for
    /// detecting the mime type
    pub(crate) async fn new(
        source: Source,
        use_expose_base_dir: bool,
        sandbox_selector: &SandboxSelector,
    ) -> Result<ProcessorContext<T, SourceTransmission>, Error> {
        let file = source.file();

        let source_transmission = SourceTransmission::init(source).await?;
        let config = config::Config::cached().await;

        let mime_type = T::guess_mime_type(
            &config,
            source_transmission.file().and_then(|x| x.path()).as_deref(),
            source_transmission.first_bytes(),
        );

        let mime_type = if let Some(mime_type) = mime_type {
            mime_type
        } else {
            guess_mime_type(
                source_transmission.file(),
                source_transmission.first_bytes(),
            )
            .await?
        };

        let config_entry = T::config_entry(&config, &mime_type)?.clone();

        let base_dir = if use_expose_base_dir && config_entry.expose_base_dir() {
            file.and_then(|x| x.parent()).and_then(|x| x.path())
        } else {
            None
        };

        let sandbox_mechanism = sandbox_selector.determine_sandbox_mechanism().await;

        Ok(ProcessorContext {
            config_entry,
            base_dir,
            mime_type,
            sandbox_mechanism,
            g_file_worker: source_transmission,
        })
    }
}

impl<T: GetConfig + Clone> ProcessorContext<T, ()> {
    pub async fn new_sourceless(
        mime_type: MimeType,
        sandbox_selector: &SandboxSelector,
    ) -> Result<ProcessorContext<T, ()>, Error> {
        let config = Config::cached().await;
        let config_entry = T::config_entry(&config, &mime_type)?.clone();
        let sandbox_mechanism = sandbox_selector.determine_sandbox_mechanism().await;

        Ok(Self {
            mime_type,
            base_dir: None,
            config_entry,
            sandbox_mechanism,
            g_file_worker: (),
        })
    }
}

impl<S> ProcessorContext<ImageLoaderConfig, S> {
    pub async fn loader(
        self,
        pool: Arc<Pool>,
        cancellable: &gio::Cancellable,
    ) -> Result<Processor<LoaderProxy<'static>, S>, Error> {
        match self.config_entry.processor {
            #[cfg(feature = "external")]
            config::Processor::Binary(_) => self
                .spin_up_loader(pool, cancellable)
                .await
                .map(Processor::Binary),
            #[cfg(feature = "builtin")]
            config::Processor::Builtin(builtin) => Ok(Processor::Builtin(BuiltinProcessor {
                builtin,
                source_transmission: self.g_file_worker,
                mime_type: self.mime_type,
                _phantom_data: Default::default(),
            })),
        }
    }

    #[cfg(feature = "external")]
    async fn spin_up_loader(
        self,
        pool: Arc<Pool>,
        cancellable: &gio::Cancellable,
    ) -> Result<ExternalProcessor<LoaderProxy<'static>, S>, Error> {
        let (process, usage_tracker) = pool
            .clone()
            .get_loader(
                self.config_entry,
                self.sandbox_mechanism,
                self.base_dir,
                cancellable,
            )
            .await?;

        Ok(ExternalProcessor {
            process,
            usage_tracker,
            source_transmission: self.g_file_worker,
            mime_type: self.mime_type,
            sandbox_mechanism: self.sandbox_mechanism,
        })
    }
}

impl<S> ProcessorContext<ImageEditorConfig, S> {
    pub async fn editor(
        self,
        pool: Arc<Pool>,
        cancellable: &gio::Cancellable,
    ) -> Result<Processor<EditorProxy<'static>, S>, Error> {
        match self.config_entry.processor {
            #[cfg(feature = "external")]
            config::Processor::Binary(_) => self
                .spin_up_editor(pool, cancellable)
                .await
                .map(Processor::Binary),
            #[cfg(feature = "builtin")]
            config::Processor::Builtin(builtin) => Ok(Processor::Builtin(BuiltinProcessor {
                builtin,
                source_transmission: self.g_file_worker,
                mime_type: self.mime_type,
                _phantom_data: Default::default(),
            })),
        }
    }

    #[cfg(feature = "external")]
    async fn spin_up_editor(
        self,
        pool: Arc<Pool>,
        cancellable: &gio::Cancellable,
    ) -> Result<ExternalProcessor<EditorProxy<'static>, S>, Error> {
        let (process, usage_tracker) = pool
            .clone()
            .get_editor(
                self.config_entry,
                self.sandbox_mechanism,
                self.base_dir,
                cancellable,
            )
            .await?;

        Ok(ExternalProcessor {
            process,
            usage_tracker,
            source_transmission: self.g_file_worker,
            mime_type: self.mime_type,
            sandbox_mechanism: self.sandbox_mechanism,
        })
    }
}
#[cfg(feature = "external")]
pub trait DBusProxy: ZbusProxy<'static> + 'static {}
#[cfg(not(feature = "external"))]
pub trait DBusProxy: 'static {}

impl DBusProxy for LoaderProxy<'static> {}
impl DBusProxy for EditorProxy<'static> {}

//impl DBusProxy for () {}

pub(crate) enum Processor<P: DBusProxy, S> {
    #[cfg(feature = "external")]
    Binary(ExternalProcessor<P, S>),
    #[cfg(feature = "builtin")]
    Builtin(BuiltinProcessor<P, S>),
}

#[cfg(feature = "external")]
pub(crate) struct ExternalProcessor<P: DBusProxy, S> {
    pub process: Arc<PooledProcess<P>>,
    pub source_transmission: S,
    pub mime_type: MimeType,
    pub sandbox_mechanism: SandboxMechanism,
    pub usage_tracker: Arc<UsageTracker>,
}

#[cfg(feature = "builtin")]
pub(crate) struct BuiltinProcessor<T, S> {
    pub builtin: config::BuiltinProcessor,
    pub mime_type: MimeType,
    pub source_transmission: S,
    _phantom_data: PhantomData<T>,
}

#[cfg(feature = "external")]
impl<P: DBusProxy, S> ExternalProcessor<P, S> {
    pub fn use_process(&self) -> Arc<crate::dbus::RemoteProcess<P>> {
        self.process.use_()
    }
}

pub(crate) async fn guess_mime_type(
    file: Option<&gio::File>,
    head: &[u8],
) -> Result<MimeType, Error> {
    fn guess_mime_type_(
        filename: Option<PathBuf>,
        data: &[u8],
    ) -> (Result<glib::GString, Error>, bool) {
        let (content_type, unsure) = gio::content_type_guess(filename, data);

        let mime_type = gio::content_type_get_mime_type(&content_type)
            .ok_or_else(|| ErrorKind::UnknownContentType(content_type.to_string()).into());

        (mime_type, unsure)
    }

    let (mime_type, unsure) = guess_mime_type_(None, head);

    // Prefer file extension for TIFF since it can be a RAW format as well
    let is_tiff = mime_type.clone().ok() == Some("image/tiff".into());

    // Prefer file extension for XML since long comment between `<?xml` and `<svg>`
    // can falsely guess XML instead of SVG
    let is_xml = mime_type.clone().ok() == Some("application/xml".into());

    // Prefer file extension for gzip since it might be an SVGZ
    let is_gzip = mime_type.clone().ok() == Some("application/gzip".into());

    // Prefer file extension for text since it might be an XBM
    let is_text = mime_type.clone().ok() == Some("text/plain".into());

    let mime_type = if (unsure || is_tiff || is_xml || is_gzip || is_text)
        && let Some(filename) = file.and_then(|x| x.basename())
    {
        guess_mime_type_(Some(filename), head).0?
    } else {
        mime_type?
    };

    tracing::trace!("Mimetype is: '{mime_type}'");

    Ok(MimeType::new(mime_type.to_string()))
}
