use std::any::Any;

#[derive(zbus::DBusError, Debug, Clone)]
#[zbus(prefix = "org.gnome.glycin.Error")]
#[non_exhaustive]
/// Error within the remote process.
///
/// Errors that appear within the loader or editor.
pub enum RemoteError {
    #[zbus(error)]
    ZBus(zbus::Error),
    LoadingError(String),
    InternalLoaderError(String),
    EditingError(String),
    InternalEditorError(String),
    UnsupportedImageFormat(String),
    ConversionTooLargerError,
    OutOfMemory(String),
    Aborted,
    NoMoreFrames,
}

type Location = std::panic::Location<'static>;

impl ProcessError {
    pub fn into_loader_error(self) -> RemoteError {
        match self {
            err @ ProcessError::ExpectedError { .. } => RemoteError::LoadingError(err.to_string()),
            err @ ProcessError::InternalError { .. } => {
                RemoteError::InternalLoaderError(err.to_string())
            }
            ProcessError::UnsupportedImageFormat(msg) => RemoteError::UnsupportedImageFormat(msg),
            ProcessError::ConversionTooLargerError => RemoteError::ConversionTooLargerError,
            err @ ProcessError::OutOfMemory { .. } => RemoteError::OutOfMemory(err.to_string()),
            ProcessError::NoMoreFrames => RemoteError::NoMoreFrames,
        }
    }

    pub fn into_editor_error(self) -> RemoteError {
        match self {
            err @ ProcessError::ExpectedError { .. } => RemoteError::EditingError(err.to_string()),
            err @ ProcessError::InternalError { .. } => {
                RemoteError::InternalEditorError(err.to_string())
            }
            ProcessError::UnsupportedImageFormat(msg) => RemoteError::UnsupportedImageFormat(msg),
            ProcessError::ConversionTooLargerError => RemoteError::ConversionTooLargerError,
            err @ ProcessError::OutOfMemory { .. } => RemoteError::OutOfMemory(err.to_string()),
            ProcessError::NoMoreFrames => RemoteError::NoMoreFrames,
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum ProcessError {
    #[error("{location}: {err}")]
    ExpectedError { err: String, location: Location },
    #[error("{location}: Internal error: {err}")]
    InternalError { err: String, location: Location },
    #[error("Unsupported image format: {0}")]
    UnsupportedImageFormat(String),
    #[error("Dimension too large for system")]
    ConversionTooLargerError,
    #[error("{location}: Not enough memory available")]
    OutOfMemory { location: Location },
    #[error("No more frames available")]
    NoMoreFrames,
}

impl ProcessError {
    #[track_caller]
    pub fn expected(err: &impl ToString) -> Self {
        Self::ExpectedError {
            err: err.to_string(),
            location: *Location::caller(),
        }
    }

    #[track_caller]
    pub fn out_of_memory() -> Self {
        Self::OutOfMemory {
            location: *Location::caller(),
        }
    }
}

impl From<DimensionTooLargerError> for ProcessError {
    fn from(err: DimensionTooLargerError) -> Self {
        eprintln!("Decoding error: {err:?}");
        Self::ConversionTooLargerError
    }
}

pub trait GenericContexts<T> {
    fn expected_error(self) -> Result<T, ProcessError>;
    fn internal_error(self) -> Result<T, ProcessError>;
}

impl<T, E> GenericContexts<T> for Result<T, E>
where
    E: std::error::Error + Any,
{
    #[track_caller]
    fn expected_error(self) -> Result<T, ProcessError> {
        match self {
            Ok(x) => Ok(x),
            Err(err) => Err(
                if let Some(err) = ((&err) as &dyn Any).downcast_ref::<ProcessError>() {
                    if matches!(err, ProcessError::OutOfMemory { .. }) {
                        ProcessError::out_of_memory()
                    } else {
                        ProcessError::expected(err)
                    }
                } else if let Some(err) =
                    ((&err) as &dyn Any).downcast_ref::<glycin_common::Error>()
                {
                    if matches!(err, glycin_common::Error::OutOfMemory) {
                        ProcessError::out_of_memory()
                    } else {
                        ProcessError::expected(err)
                    }
                } else {
                    ProcessError::expected(&err)
                },
            ),
        }
    }

    #[track_caller]
    fn internal_error(self) -> Result<T, ProcessError> {
        match self {
            Ok(x) => Ok(x),
            Err(err) => Err(ProcessError::InternalError {
                err: err.to_string(),
                location: *Location::caller(),
            }),
        }
    }
}

impl<T> GenericContexts<T> for Option<T> {
    #[track_caller]
    fn expected_error(self) -> Result<T, ProcessError> {
        match self {
            Some(x) => Ok(x),
            None => Err(ProcessError::ExpectedError {
                err: String::from("None"),
                location: *Location::caller(),
            }),
        }
    }

    #[track_caller]
    fn internal_error(self) -> Result<T, ProcessError> {
        match self {
            Some(x) => Ok(x),
            None => Err(ProcessError::InternalError {
                err: String::from("None"),
                location: *Location::caller(),
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DimensionTooLargerError;

impl std::fmt::Display for DimensionTooLargerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("Dimension too large for system")
    }
}

impl std::error::Error for DimensionTooLargerError {}
