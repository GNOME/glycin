// Copyright (c) 2024 GNOME Foundation Inc.

use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::sync::{Mutex, MutexGuard};

use zbus::zvariant::OwnedObjectPath;

use crate::dbus_types::*;
use crate::error::*;

pub trait LoaderState: Send + 'static {
    fn frame(&self, frame_request: FrameRequest) -> Result<Frame, ProcessError>;
}

pub trait LoaderImplementation<T>: Send {
    fn init(
        &self,
        stream: UnixStream,
        mime_type: String,
        details: InitializationDetails,
    ) -> Result<(ImageInfo, T), ProcessError>;
    fn frame(&self, loader_state: T, frame_request: FrameRequest) -> Result<Frame, ProcessError>;
}

pub struct Loader<T: LoaderState> {
    pub loader: Mutex<Box<dyn LoaderImplementation<T>>>,
    pub image_id: Mutex<u64>,
}

impl<T: LoaderState> Loader<T> {
    pub fn get_loader(&self) -> Result<MutexGuard<Box<dyn LoaderImplementation<T>>>, RemoteError> {
        self.loader.lock().map_err(|err| {
            RemoteError::InternalLoaderError(format!("Failed to lock decoder for operation: {err}"))
        })
    }
}

#[zbus::interface(name = "org.gnome.glycin.Loader")]
impl<T: LoaderState> Loader<T> {
    async fn init(
        &self,
        init_request: InitRequest,
        #[zbus(connection)] dbus_connection: &zbus::Connection,
    ) -> Result<ImageInfo, RemoteError> {
        let fd = OwnedFd::from(init_request.fd);
        let stream = UnixStream::from(fd);

        let (image_info, loader_state) = self
            .get_loader()?
            .init(stream, init_request.mime_type, init_request.details)
            .map_err(|x| x.into_loader_error())?;

        let image_id = {
            let lock = self.image_id.lock();
            let mut image_id = match lock {
                Ok(id) => id,
                Err(err) => return Err(RemoteError::InternalLoaderError(err.to_string())),
            };
            let id = *image_id;
            *image_id = id + 1;
            id
        };

        let path = OwnedObjectPath::try_from(format!("/org/gnome/glycin/image/{image_id}"))
            .internal_error()
            .map_err(|x| x.into_loader_error())?;

        dbus_connection
            .object_server()
            .at(
                &path,
                Image {
                    loader_state: Mutex::new(Box::new(loader_state)),
                    path: path.clone(),
                },
            )
            .await
            .internal_error()
            .map_err(|x| x.into_loader_error())?;

        Ok(image_info)
    }

    async fn frame(&self, frame_request: FrameRequest) -> Result<Frame, RemoteError> {
        self.get_loader()?
            .frame(todo!(), frame_request)
            .map_err(|x| x.into_loader_error())
    }
}

pub struct Image<T: LoaderState> {
    pub loader_state: Mutex<Box<T>>,
    pub path: OwnedObjectPath,
}

impl<T: LoaderState> Image<T> {
    pub fn get_loader_state(&self) -> Result<MutexGuard<Box<T>>, RemoteError> {
        self.loader_state.lock().map_err(|err| {
            RemoteError::InternalLoaderError(format!(
                "Failed to lock loader state for operation: {err}"
            ))
        })
    }
}

#[zbus::interface(name = "org.gnome.glycin.Image")]
impl<T: LoaderState> Image<T> {
    async fn frame(&self, frame_request: FrameRequest) -> Result<Frame, RemoteError> {
        self.get_loader_state()?
            .frame(frame_request)
            .map_err(|x| x.into_loader_error())
    }

    async fn done(
        &self,
        #[zbus(object_server)] object_server: &zbus::ObjectServer,
    ) -> Result<(), RemoteError> {
        object_server.remove::<Image<T>, _>(&self.path).await?;

        Ok(())
    }
}
