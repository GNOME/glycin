// Copyright (c) 2024 GNOME Foundation Inc.

use std::marker::PhantomData;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex, MutexGuard};

use futures_util::FutureExt;
use zbus::zvariant::OwnedObjectPath;

use crate::dbus_types::*;
use crate::error::*;

pub trait LoaderImplementation: Send + Sync + Sized + 'static {
    fn init(
        stream: UnixStream,
        mime_type: String,
        details: InitializationDetails,
    ) -> Result<(Self, ImageDetails), ProcessError>;

    fn frame(&mut self, frame_request: FrameRequest) -> Result<Frame, ProcessError>;
}

pub struct Loader<T: LoaderImplementation> {
    pub loader: PhantomData<T>,
    pub image_id: Mutex<u64>,
}

#[zbus::interface(name = "org.gnome.glycin.Loader")]
impl<T: LoaderImplementation> Loader<T> {
    async fn init(
        &self,
        init_request: InitRequest,
        #[zbus(connection)] dbus_connection: &zbus::Connection,
    ) -> Result<RemoteImage, RemoteError> {
        let fd = OwnedFd::from(init_request.fd);
        let stream = UnixStream::from(fd);

        let (loader_state, image_info) =
            T::init(stream, init_request.mime_type, init_request.details)
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

        let dbus_image = RemoteImage::new(image_info, path.clone());

        dbus_connection
            .object_server()
            .at(
                &path,
                Image {
                    loader_implementation: Arc::new(Mutex::new(Box::new(loader_state))),
                    path: path.clone(),
                    dropped: Default::default(),
                },
            )
            .await
            .internal_error()
            .map_err(|x| x.into_loader_error())?;

        Ok(dbus_image)
    }
}

pub struct Image<T: LoaderImplementation> {
    pub loader_implementation: Arc<Mutex<Box<T>>>,
    pub path: OwnedObjectPath,
    dropped: async_lock::OnceCell<()>,
}

impl<T: LoaderImplementation> Image<T> {
    pub fn get_loader_state(&self) -> Result<MutexGuard<'_, Box<T>>, RemoteError> {
        self.loader_implementation.lock().map_err(|err| {
            RemoteError::InternalLoaderError(format!(
                "Failed to lock loader state for operation: {err}"
            ))
        })
    }
}

#[zbus::interface(name = "org.gnome.glycin.Image")]
impl<T: LoaderImplementation> Image<T> {
    async fn frame(&self, frame_request: FrameRequest) -> Result<Frame, RemoteError> {
        let loader_implementation = self.loader_implementation.clone();
        let mut frame_request = blocking::unblock(move || {
            let mut loader_implementation = loader_implementation.lock().map_err(|err| {
                RemoteError::InternalLoaderError(format!(
                    "Failed to lock loader state for operation: {err}"
                ))
            })?;

            loader_implementation
                .frame(frame_request)
                .map_err(|x| x.into_loader_error())
        })
        .fuse();

        futures_util::select! {
            result = frame_request => result,
            _ = self.dropped.wait().fuse() => Err(RemoteError::Aborted),
        }
    }

    async fn done(
        &self,
        #[zbus(object_server)] object_server: &zbus::ObjectServer,
    ) -> Result<(), RemoteError> {
        log::debug!("Disconnecting {}", self.path);
        let removed = object_server.remove::<Image<T>, _>(&self.path).await?;
        if removed {
            log::debug!("Removed {}", self.path);
        } else {
            log::error!("Failed to remove {}", self.path);
        }
        let _ = self.dropped.set(()).await;
        Ok(())
    }
}
