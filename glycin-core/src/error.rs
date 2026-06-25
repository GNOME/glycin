use std::any::Any;
use std::fmt::Display;
use std::process::ExitStatus;
use std::sync::Arc;
use std::time::Duration;

use futures_channel::oneshot;
use gio::glib;
use glycin_utils::{DimensionTooLargerError, MemoryAllocationError, RemoteError};

#[cfg(feature = "external")]
use crate::dbus::RemoteProcess;
use crate::{DBusProxy, FeatureNotSupported, MAX_TEXTURE_SIZE, config};

#[derive(Debug, Clone, Default)]
pub struct ErrorContext {
    stderr: Option<String>,
    stdout: Option<String>,
}

impl std::fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(stderr) = &self.stderr
            && !stderr.is_empty()
        {
            f.write_str("\n\nstderr:\n")?;
            f.write_str(stderr)?;
        }

        if let Some(stdout) = &self.stdout
            && !stdout.is_empty()
        {
            f.write_str("\n\nstdout:\n")?;
            f.write_str(stdout)?;
        }

        Ok(())
    }
}

pub trait ResultExt<T> {
    #[cfg(feature = "external")]
    fn err_context<S: DBusProxy>(self, process: &RemoteProcess<S>) -> Result<T, Error>;
}

impl<T, E: Into<Error>> ResultExt<T> for Result<T, E> {
    #[cfg(feature = "external")]
    fn err_context<S: DBusProxy>(self, process: &RemoteProcess<S>) -> Result<T, Error> {
        match self {
            Ok(x) => Ok(x),
            Err(err) => {
                let mut err = err.into();

                let stderr = process.stderr_content.lock().ok().map(|x| x.clone());
                let stdout = process.stdout_content.lock().ok().map(|x| x.clone());

                err.context = Some(ErrorContext { stderr, stdout });

                Err(err)
            }
        }
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Error {
    kind: Box<ErrorKind>,
    context: Option<ErrorContext>,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.kind.to_string())
    }
}

impl std::error::Error for Error {}

impl Error {
    fn from_kind(kind: ErrorKind) -> Self {
        Self {
            kind: Box::new(kind),
            context: None,
        }
    }

    #[cfg(feature = "unstable")]
    pub fn kind(self) -> ErrorKind {
        *self.kind
    }

    /// Returns if the error is related to unsupported formats.
    ///
    /// Return the mime type of the unsupported format or [`None`] if the error
    /// is unrelated to unsupported formats.
    pub fn unsupported_format(&self) -> Option<String> {
        match &*self.kind {
            ErrorKind::UnknownImageFormat(mime_type, _) => Some(mime_type.to_string()),
            ErrorKind::RemoteError(RemoteError::UnsupportedImageFormat(msg)) => Some(msg.clone()),
            _ => None,
        }
    }

    pub fn failed_image_source(&self) -> Option<glib::Error> {
        if let ErrorKind::ImageSource(err) = &*self.kind {
            Some(err.clone())
        } else {
            None
        }
    }

    pub fn has_no_processor_configured(&self) -> bool {
        matches!(*self.kind, ErrorKind::NoLoadersConfigured(_))
    }

    pub fn is_out_of_memory(&self) -> bool {
        matches!(
            *self.kind,
            ErrorKind::RemoteError(RemoteError::OutOfMemory(_))
        )
    }

    pub fn has_no_more_frames(&self) -> bool {
        matches!(
            *self.kind,
            ErrorKind::RemoteError(RemoteError::NoMoreFrames)
        )
    }

    pub fn is_panic(&self) -> bool {
        matches!(
            *self.kind,
            ErrorKind::ThreadPanic(_) | ErrorKind::RemoteError(RemoteError::Panic)
        )
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(*self.kind, ErrorKind::Canceled(_))
    }

    pub fn is_timeout(&self) -> bool {
        matches!(*self.kind, ErrorKind::Timeout(_))
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum ErrorKind {
    #[error("Remote error: {0}")]
    RemoteError(#[from] RemoteError),
    #[error("GLib error: {0}")]
    GLibError(#[from] glib::Error),
    #[error("Failed to load file/stream: {0}")]
    ImageSource(glib::Error),
    #[cfg(feature = "external")]
    #[error("Libc error: {0}")]
    NixError(#[from] nix::errno::Errno),
    #[error("IO error: {err} {info}")]
    StdIoError {
        err: Arc<std::io::Error>,
        info: String,
    },
    #[error("D-Bus error: {0}")]
    #[cfg(feature = "external")]
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
    #[error("Could not spawn the following command. Is the used binary available? `{cmd}`: {err}")]
    SpawnErrorNotFound {
        cmd: String,
        err: Arc<std::io::Error>,
    },
    #[error("Texture is only {texture_size} but was announced differently: {frame}")]
    TextureWrongSize { texture_size: usize, frame: String },
    #[error("Texture size exceeds hardcoded limit of {MAX_TEXTURE_SIZE} bytes")]
    TextureTooLarge,
    #[error("Stride is smaller than possible: {0}")]
    StrideTooSmall(String),
    #[error("Width or height is zero: {0}")]
    WidgthOrHeightZero(String),
    #[cfg(feature = "external")]
    #[error("Seccomp: {0}")]
    Seccomp(Arc<libseccomp::error::SeccompError>),
    #[error("ICC profile: {0}")]
    IccProfile(#[from] lcms2::Error),
    #[error("Operation was explicitly canceled.\nOriginal error: {0:?}")]
    Canceled(Option<String>),
    #[error("Editing: {0}")]
    Editing(#[from] glycin_utils::editing::Error),
    #[error("Trying to access already transferred GInputStream")]
    TransferredStream,
    #[cfg(feature = "gobject")]
    #[error("A loader can only be used once")]
    LoaderUsedTwice,
    #[error("Math error: {0}")]
    MathError(#[from] gufo_common::math::MathError),
    #[error("Glycin common error: {0}")]
    CommonError(#[from] glycin_common::Error),
    #[error("Tried to use builtin processor in binary context")]
    ExpectedBinaryProcessor,
    #[error("Failed to allocate memory: {0}")]
    MemoryAllocationError(String),
    #[error("GLib thread failed: {0}")]
    JoinError(String),
    #[error("Thread panic: {0:?}")]
    ThreadPanic(Option<String>),
    #[error("Feature not supported: {0}")]
    FeatureNotSupported(#[from] FeatureNotSupported),
    #[error("Operation did not complete in supplied limit of {0:?}")]
    Timeout(Duration),
    #[error("This state should never have been reached: {0}:{1}")]
    Unreachable(&'static str, u32),
}

impl ErrorKind {
    pub fn err(self) -> Error {
        Error::from_kind(self)
    }

    pub fn panic(any: Box<dyn Any>) -> ErrorKind {
        let s = any
            .downcast_ref::<&str>()
            .map(|x| x.to_string())
            .or_else(|| any.downcast_ref::<String>().map(|x| x.to_string()));

        ErrorKind::ThreadPanic(s)
    }

    #[track_caller]
    pub(crate) fn unreachable() -> ErrorKind {
        Self::Unreachable(std::file!(), std::line!())
    }
}

impl From<std::io::Error> for ErrorKind {
    fn from(err: std::io::Error) -> Self {
        Self::StdIoError {
            err: Arc::new(err),
            info: String::new(),
        }
    }
}

impl From<Arc<std::io::Error>> for ErrorKind {
    fn from(err: Arc<std::io::Error>) -> Self {
        Self::StdIoError {
            err,
            info: String::new(),
        }
    }
}

#[cfg(feature = "external")]
impl From<libseccomp::error::SeccompError> for ErrorKind {
    fn from(err: libseccomp::error::SeccompError) -> Self {
        Self::Seccomp(Arc::new(err))
    }
}

impl From<oneshot::Canceled> for ErrorKind {
    fn from(_err: oneshot::Canceled) -> Self {
        Self::InternalCommunicationCanceled
    }
}

impl From<DimensionTooLargerError> for ErrorKind {
    fn from(_err: DimensionTooLargerError) -> Self {
        Self::ConversionTooLargerError
    }
}

impl From<MemoryAllocationError> for ErrorKind {
    fn from(value: MemoryAllocationError) -> Self {
        Self::MemoryAllocationError(value.to_string())
    }
}

impl From<glib::JoinError> for ErrorKind {
    fn from(value: glib::JoinError) -> Self {
        Self::JoinError(value.to_string())
    }
}

impl<T> From<T> for Error
where
    T: Into<ErrorKind>,
{
    fn from(t: T) -> Self {
        Error::from_kind(t.into())
    }
}
