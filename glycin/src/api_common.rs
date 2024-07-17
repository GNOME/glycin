use std::path::PathBuf;

#[cfg(feature = "gobject")]
use gio::glib;
use gio::prelude::*;

use crate::dbus::{GFileWorker, RemoteProcess, ZbusProxy};
use crate::error::ResultKind;
use crate::util::RunEnvironment;
use crate::{config, ErrorKind, MimeType};

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

pub(crate) struct RemoteProcessContext<'a, P: ZbusProxy<'a>> {
    pub process: RemoteProcess<'a, P>,
    pub gfile_worker: GFileWorker,
    pub base_dir: Option<PathBuf>,
    pub mime_type: MimeType,
    pub sandbox_mechanism: SandboxMechanism,
}

pub(crate) async fn spin_up<'a, P: ZbusProxy<'a> + 'a>(
    file: &gio::File,
    cancellable: &gio::Cancellable,
    sandbox_selector: &SandboxSelector,
) -> ResultKind<RemoteProcessContext<'a, P>> {
    let config = config::Config::cached().await;

    let gfile_worker = GFileWorker::spawn(file.clone(), cancellable.clone());
    let mime_type = guess_mime_type(&gfile_worker).await?;

    let sandbox_mechanism = sandbox_selector.determine_sandbox_mechanism().await;

    let process =
        RemoteProcess::new(&mime_type, config, sandbox_mechanism, file, cancellable).await?;

    let base_dir: Option<PathBuf> = if P::expose_base_dir(config, &mime_type)? {
        file.parent().and_then(|x| x.path())
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

pub(crate) async fn guess_mime_type(gfile_worker: &GFileWorker) -> ResultKind<MimeType> {
    let head = gfile_worker.head().await?;
    let (content_type, unsure) = gio::content_type_guess(None::<String>, &head);
    let mime_type = gio::content_type_get_mime_type(&content_type)
        .ok_or_else(|| ErrorKind::UnknownContentType(content_type.to_string()));

    // Prefer file extension for TIFF since it can be a RAW format as well
    let is_tiff = mime_type.clone().ok() == Some("image/tiff".into());

    // Prefer file extension for XML since long comment between `<?xml` and `<svg>`
    // can falsely guess XML instead of SVG
    let is_xml = mime_type.clone().ok() == Some("application/xml".into());

    if unsure || is_tiff || is_xml {
        if let Some(filename) = gfile_worker.file().basename() {
            let content_type_fn = gio::content_type_guess(Some(filename), &head).0;
            return gio::content_type_get_mime_type(&content_type_fn)
                .ok_or_else(|| ErrorKind::UnknownContentType(content_type_fn.to_string()))
                .map(|x| MimeType(x.to_string()));
        }
    }

    mime_type.map(|x| MimeType(x.to_string()))
}
