use std::path::PathBuf;

#[cfg(feature = "gobject")]
use gio::glib;
use gio::prelude::*;
use glycin_utils::MemoryFormatSelection;

use crate::dbus::{GFileWorker, RemoteProcess, ZbusProxy};
use crate::util::RunEnvironment;
use crate::{config, Error, MimeType};

#[derive(Debug, Copy, Clone)]
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
    /// ends with `.Devel`, the sandbox is disabled.
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

pub(crate) struct RemoteProcessContext<'a, P: ZbusProxy<'a>> {
    pub process: RemoteProcess<'a, P>,
    pub gfile_worker: GFileWorker,
    pub base_dir: Option<PathBuf>,
    pub mime_type: MimeType,
    pub sandbox_mechanism: SandboxMechanism,
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
            .map(|x| Self::File(x))
            .unwrap_or(Self::TransferredStream);

        std::mem::replace(self, new)
    }
}

pub(crate) async fn spin_up<'a, P: ZbusProxy<'a> + 'a>(
    source: Source,
    cancellable: &gio::Cancellable,
    sandbox_selector: &SandboxSelector,
    memory_format_selection: MemoryFormatSelection,
) -> Result<RemoteProcessContext<'a, P>, Error> {
    let config = config::Config::cached().await;

    let file = source.file();

    let gfile_worker = GFileWorker::spawn(source, cancellable.clone());
    let mime_type = guess_mime_type(&gfile_worker).await?;

    let sandbox_mechanism = sandbox_selector.determine_sandbox_mechanism().await;

    let process = RemoteProcess::new(
        &mime_type,
        config,
        sandbox_mechanism,
        file.clone(),
        cancellable,
        memory_format_selection,
    )
    .await?;

    let base_dir: Option<PathBuf> = if P::expose_base_dir(config, &mime_type)? {
        file.and_then(|x| x.parent()).and_then(|x| x.path())
    } else {
        None
    };

    Ok(RemoteProcessContext {
        process,
        gfile_worker,
        base_dir,
        mime_type,
        sandbox_mechanism,
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
                .map(|x| MimeType(x.to_string()));
        }
    }

    mime_type.map(|x| MimeType(x.to_string()))
}
