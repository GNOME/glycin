use std::path::PathBuf;
use std::sync::Arc;

#[cfg(feature = "gobject")]
use gio::glib;
use gio::prelude::*;

use crate::config::{Config, ImageEditorConfig, ImageLoaderConfig};
use crate::dbus::{EditorProxy, GFileWorker, LoaderProxy, ZbusProxy};
use crate::pool::{Pool, PooledProcess, UsageTracker};
use crate::util::RunEnvironment;
use crate::{config, Error, MimeType};

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

pub(crate) struct RemoteProcessContext<P: ZbusProxy<'static> + 'static> {
    pub process: Arc<PooledProcess<P>>,
    pub g_file_worker: Option<GFileWorker>,
    pub mime_type: MimeType,
    pub sandbox_mechanism: SandboxMechanism,
    pub usage_tracker: Arc<UsageTracker>,
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

    #[cfg(feature = "gobject")]
    pub(crate) fn stream(&self) -> gio::InputStream {
        self.0.clone()
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

    pub fn to_stream(&self, cancellable: &gio::Cancellable) -> Result<gio::InputStream, Error> {
        match self {
            Self::File(file) => file
                .read(Some(cancellable))
                .map(|x| x.upcast())
                .map_err(Into::into),
            Self::Stream(stream) => Ok(stream.0.clone()),
            Self::TransferredStream => Err(Error::TransferredStream),
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
}

#[derive(Debug)]
pub(crate) struct ProcessBasics<T> {
    pub mime_type: MimeType,
    pub sandbox_mechanism: SandboxMechanism,
    pub config_entry: T,
    pub g_file_worker: Option<GFileWorker>,
    pub base_dir: Option<PathBuf>,
}

pub trait GetConfig {
    fn config_entry<'a>(config: &'a Config, mime_type: &'a MimeType) -> Result<&'a Self, Error>;
    fn expose_base_dir(&self) -> bool;
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
}

pub(crate) async fn spin_up<T: GetConfig + Clone>(
    source: Source,
    use_expose_base_dir: bool,
    cancellable: &gio::Cancellable,
    sandbox_selector: &SandboxSelector,
) -> Result<ProcessBasics<T>, Error> {
    let file = source.file();

    let g_file_worker: GFileWorker = GFileWorker::spawn(source, cancellable.clone());
    let mime_type = guess_mime_type(&g_file_worker).await?;

    let config = config::Config::cached().await;
    let config_entry = T::config_entry(&config, &mime_type)?.clone().clone();

    let base_dir = if use_expose_base_dir && config_entry.expose_base_dir() {
        file.and_then(|x| x.parent()).and_then(|x| x.path())
    } else {
        None
    };

    let sandbox_mechanism = sandbox_selector.determine_sandbox_mechanism().await;

    Ok(ProcessBasics {
        config_entry,
        base_dir,
        mime_type,
        sandbox_mechanism,
        g_file_worker: Some(g_file_worker),
    })
}

pub(crate) async fn spin_up_editor<'a>(
    source: Source,
    pool: Arc<Pool>,
    cancellable: &gio::Cancellable,
    sandbox_selector: &SandboxSelector,
) -> Result<RemoteProcessContext<EditorProxy<'static>>, Error> {
    let process_basics =
        spin_up::<ImageEditorConfig>(source, false, cancellable, sandbox_selector).await?;

    let (process, usage_tracker) = pool
        .get_editor(
            process_basics.config_entry,
            process_basics.sandbox_mechanism,
            process_basics.base_dir,
            cancellable,
        )
        .await?;

    Ok(RemoteProcessContext {
        process,
        g_file_worker: process_basics.g_file_worker,
        mime_type: process_basics.mime_type,
        sandbox_mechanism: process_basics.sandbox_mechanism,
        usage_tracker,
    })
}

pub(crate) async fn spin_up_encoder<'a>(
    mime_type: MimeType,
    pool: Arc<Pool>,
    cancellable: &gio::Cancellable,
    sandbox_selector: &SandboxSelector,
) -> Result<RemoteProcessContext<EditorProxy<'static>>, Error> {
    let config_entry = Config::cached().await.editor(&mime_type)?.clone();
    let sandbox_mechanism = sandbox_selector.determine_sandbox_mechanism().await;

    let (process, usage_tracker) = pool
        .get_editor(config_entry, sandbox_mechanism, None, cancellable)
        .await?;

    Ok(RemoteProcessContext {
        process,
        g_file_worker: None,
        mime_type,
        sandbox_mechanism,
        usage_tracker,
    })
}

pub(crate) async fn spin_up_loader<'a>(
    source: Source,
    use_expose_base_dir: bool,
    pool: Arc<Pool>,
    cancellable: &gio::Cancellable,
    sandbox_selector: &SandboxSelector,
) -> Result<RemoteProcessContext<LoaderProxy<'static>>, Error> {
    let process_basics =
        spin_up(source, use_expose_base_dir, cancellable, sandbox_selector).await?;

    let (process, usage_tracker) = pool
        .clone()
        .get_loader(
            process_basics.config_entry,
            process_basics.sandbox_mechanism,
            process_basics.base_dir,
            cancellable,
        )
        .await?;

    Ok(RemoteProcessContext {
        process,
        usage_tracker,
        g_file_worker: process_basics.g_file_worker,
        mime_type: process_basics.mime_type,
        sandbox_mechanism: process_basics.sandbox_mechanism,
    })
}

pub(crate) async fn guess_mime_type(gfile_worker: &GFileWorker) -> Result<MimeType, Error> {
    let head = gfile_worker.head().await?;
    let (content_type, unsure) = gio::content_type_guess(None::<String>, head.as_slice());
    let mime_type = gio::content_type_get_mime_type(&content_type)
        .ok_or_else(|| Error::UnknownContentType(content_type.to_string()));

    // Prefer file extension for TIFF since it can be a RAW format as well
    let is_tiff = mime_type.clone().ok() == Some("image/tiff".into());

    // Prefer file extension for XML since long comment between `<?xml` and `<svg>`
    // can falsely guess XML instead of SVG
    let is_xml = mime_type.clone().ok() == Some("application/xml".into());

    // Prefer file extension for gzip since it might be an SVGZ
    let is_gzip = mime_type.clone().ok() == Some("application/gzip".into());

    if unsure || is_tiff || is_xml || is_gzip {
        if let Some(filename) = gfile_worker.file().and_then(|x| x.basename()) {
            let content_type_fn = gio::content_type_guess(Some(filename), head.as_slice()).0;
            return gio::content_type_get_mime_type(&content_type_fn)
                .ok_or_else(|| Error::UnknownContentType(content_type_fn.to_string()))
                .map(|x| MimeType::new(x.to_string()));
        }
    }

    mime_type.map(|x| MimeType::new(x.to_string()))
}
