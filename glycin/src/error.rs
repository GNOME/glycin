use std::ops::Deref;
use std::process::ExitStatus;
use std::sync::Arc;

use futures_channel::oneshot;
use gio::glib;
use glycin_utils::{DimensionTooLargerError, RemoteError};
use libseccomp::error::SeccompError;

use crate::config;
use crate::dbus::{RemoteProcess, ZbusProxy, MAX_TEXTURE_SIZE};

#[derive(Debug, Clone)]

pub struct ErrorCtx {
    error: Error,
    stderr: Option<String>,
    stdout: Option<String>,
}

impl Deref for ErrorCtx {
    type Target = Error;

    fn deref(&self) -> &Self::Target {
        self.error()
    }
}

impl std::error::Error for ErrorCtx {}

impl std::fmt::Display for ErrorCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.error.to_string())?;

        if let Some(stderr) = &self.stderr {
            if !stderr.is_empty() {
                f.write_str("\n\nstderr:\n")?;
                f.write_str(&stderr)?;
            }
        }

        if let Some(stdout) = &self.stdout {
            if !stdout.is_empty() {
                f.write_str("\n\nstdout:\n")?;
                f.write_str(&stdout)?;
            }
        }

        Ok(())
    }
}

impl ErrorCtx {
    pub fn from_error(kind: Error) -> Self {
        ErrorCtx {
            error: kind,
            stderr: None,
            stdout: None,
        }
    }

    pub fn error(&self) -> &Error {
        &self.error
    }
}

pub trait ResultExt<T> {
    fn err_context<'a, S: ZbusProxy<'a>>(
        self,
        process: &RemoteProcess<'a, S>,
    ) -> Result<T, ErrorCtx>;
    fn err_no_context(self) -> Result<T, ErrorCtx>;
}

impl<T> ResultExt<T> for Result<T, Error> {
    fn err_context<'a, S: ZbusProxy<'a>>(
        self,
        process: &RemoteProcess<'a, S>,
    ) -> Result<T, ErrorCtx> {
        match self {
            Ok(x) => Ok(x),
            Err(kind) => {
                let stderr = process.stderr_content.lock().ok().map(|x| x.clone());
                let stdout = process.stdout_content.lock().ok().map(|x| x.clone());

                Err(ErrorCtx {
                    error: kind,
                    stderr,
                    stdout,
                })
            }
        }
    }

    fn err_no_context(self) -> Result<T, ErrorCtx> {
        self.map_err(ErrorCtx::from_error)
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Remote error: {0}")]
    RemoteError(#[from] RemoteError),
    #[error("GLib error: {0}")]
    GLibError(#[from] glib::Error),
    #[error("Libc error: {0}")]
    NixError(#[from] nix::errno::Errno),
    #[error("IO error: {err} {info}")]
    StdIoError {
        err: Arc<std::io::Error>,
        info: String,
    },
    #[error("D-Bus error: {0}")]
    DbusError(#[from] zbus::Error),
    #[error("Internal communication was unexpectedly canceled")]
    InternalCommunicationCanceled,
    #[error(
        "No image loaders are configured. You might need to install a package like glycin-loaders.\nUsed config: {0:#?}"
    )]
    NoLoadersConfigured(config::Config),
    #[error("Unknown image format: {0}\nUsed config: {1:#?}")]
    UnknownImageFormat(String, config::Config),
    #[error("Unknown content type: {0}")]
    UnknownContentType(String),
    #[error("Loader process exited early with status '{}'Command:\n {cmd}", .status.code().unwrap_or_default())]
    PrematureExit { status: ExitStatus, cmd: String },
    #[error("Conversion too large")]
    ConversionTooLargerError,
    #[error("Could not spawn `{cmd}`: {err}")]
    SpawnError {
        cmd: String,
        err: Arc<std::io::Error>,
    },
    #[error("Texture is only {texture_size} but was announced differently: {frame}")]
    TextureTooSmall { texture_size: usize, frame: String },
    #[error("Texture size exceeds hardcoded limit of {MAX_TEXTURE_SIZE} bytes")]
    TextureTooLarge,
    #[error("Stride is smaller than possible: {0}")]
    StrideTooSmall(String),
    #[error("Width or height is zero: {0}")]
    WidgthOrHeightZero(String),
    #[error("Memfd: {0}")]
    MemFd(Arc<memfd::Error>),
    #[error("Seccomp: {0}")]
    Seccomp(Arc<SeccompError>),
    #[error("ICC profile: {0}")]
    IccProfile(#[from] lcms2::Error),
}

impl Error {
    /// Returns if the error is related to unsupported formats.
    ///
    /// Return the mime type of the unsupported format or [`None`] if the error
    /// is unrelated to unsupported formats.
    pub fn unsupported_format(&self) -> Option<String> {
        match self {
            Self::UnknownImageFormat(mime_type, _) => Some(mime_type.to_string()),
            Self::RemoteError(RemoteError::UnsupportedImageFormat(msg)) => Some(msg.clone()),
            _ => None,
        }
    }

    pub fn is_out_of_memory(&self) -> bool {
        matches!(self, Self::RemoteError(RemoteError::OutOfMemory(_)))
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::StdIoError {
            err: Arc::new(err),
            info: String::new(),
        }
    }
}

impl From<Arc<std::io::Error>> for Error {
    fn from(err: Arc<std::io::Error>) -> Self {
        Self::StdIoError {
            err,
            info: String::new(),
        }
    }
}

impl From<memfd::Error> for Error {
    fn from(err: memfd::Error) -> Self {
        Self::MemFd(Arc::new(err))
    }
}

impl From<SeccompError> for Error {
    fn from(err: SeccompError) -> Self {
        Self::Seccomp(Arc::new(err))
    }
}

impl From<oneshot::Canceled> for Error {
    fn from(_err: oneshot::Canceled) -> Self {
        Self::InternalCommunicationCanceled
    }
}

impl From<DimensionTooLargerError> for Error {
    fn from(_err: DimensionTooLargerError) -> Self {
        Self::ConversionTooLargerError
    }
}
