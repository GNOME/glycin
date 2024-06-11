// Copyright (c) 2024 GNOME Foundation Inc.

use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::sync::{Mutex, MutexGuard};

use crate::dbus::*;
use crate::error::*;

pub trait LoaderImplementation: Send {
    fn init(
        &self,
        stream: UnixStream,
        mime_type: String,
        details: InitializationDetails,
    ) -> Result<ImageInfo, LoaderError>;
    fn frame(&self, frame_request: FrameRequest) -> Result<Frame, LoaderError>;
}

pub struct Loader {
    pub loader: Mutex<Box<dyn LoaderImplementation>>,
}

impl Loader {
    pub fn get_loader(&self) -> Result<MutexGuard<Box<dyn LoaderImplementation>>, RemoteError> {
        self.loader.lock().map_err(|err| {
            RemoteError::InternalLoaderError(format!("Failed to lock decoder for operation: {err}"))
        })
    }
}

#[zbus::interface(name = "org.gnome.glycin.Loader")]
impl Loader {
    async fn init(&self, init_request: InitRequest) -> Result<ImageInfo, RemoteError> {
        let fd = OwnedFd::from(init_request.fd);
        let stream = UnixStream::from(fd);

        let image_info =
            self.get_loader()?
                .init(stream, init_request.mime_type, init_request.details)?;

        Ok(image_info)
    }

    async fn frame(&self, frame_request: FrameRequest) -> Result<Frame, RemoteError> {
        self.get_loader()?.frame(frame_request).map_err(Into::into)
    }
}
