// Copyright (c) 2024 GNOME Foundation Inc.

use std::io::Read;
use std::marker::PhantomData;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};

use futures_util::FutureExt;
use glycin_common::Operations;
use zbus::zvariant::{DeserializeDict, OwnedObjectPath, SerializeDict, Type};

use crate::error::*;
use crate::{ByteData, MemoryAllocationError, SharedMemory, api};

#[derive(DeserializeDict, SerializeDict, Type, Debug)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct EditRequest {
    pub operations: SharedMemory,
}

impl EditRequest {
    pub fn for_operations(operations: &Operations) -> Result<Self, RemoteError> {
        let operations = operations
            .to_message_pack()
            .expected_error()
            .map_err(|x| x.into_editor_error())?;
        let operations = SharedMemory::try_from_vec(operations)?;
        Ok(Self { operations })
    }

    pub fn operations(&self) -> Result<Operations, RemoteError> {
        let operations = Operations::from_slice(&self.operations)
            .expected_error()
            .map_err(|x| x.into_editor_error())?;

        Ok(operations)
    }

    #[cfg(feature = "external")]
    pub async fn initial_seal(&mut self) -> Result<(), MemoryAllocationError> {
        self.operations.initial_seal().await
    }
}

pub struct Editor<E: api::EditorImplementation> {
    pub editor: PhantomData<E>,
    pub image_id: Mutex<u64>,
}

/// D-Bus interface for image editors
#[zbus::interface(name = "org.gnome.glycin.Editor")]
impl<E: api::EditorImplementation> Editor<E> {
    async fn create(
        &self,
        mime_type: String,
        mut new_image: api::NewImage<SharedMemory>,
        encoding_options: api::EncodingOptions,
    ) -> Result<api::EncodedImage<SharedMemory>, RemoteError> {
        new_image.initial_seal().await?;
        blocking::unblock(|| {
            crate::catch_unwind(|| {
                E::create(mime_type, new_image, encoding_options).map_err(|x| x.into_editor_error())
            })
            .flatten()
        })
        .await
    }

    async fn edit(
        &self,
        init_request: api::InitRequest,
        #[zbus(connection)] dbus_connection: &zbus::Connection,
    ) -> Result<api::RemoteEditableImage, RemoteError> {
        let fd = OwnedFd::from(init_request.fd);
        let stream = UnixStream::from(fd);

        let editor_state = blocking::unblock(|| {
            crate::catch_unwind(|| {
                E::edit(stream, init_request.mime_type, init_request.details)
                    .map_err(|x| x.into_loader_error())
            })
        })
        .await
        .flatten()?;

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

        let path =
            OwnedObjectPath::try_from(format!("/org/gnome/glycin/editable_image/{image_id}"))
                .internal_error()
                .map_err(|x| x.into_loader_error())?;

        let dbus_image = api::RemoteEditableImage::new(path.clone());

        dbus_connection
            .object_server()
            .at(
                &path,
                EditableImage {
                    editor_implementation: Arc::new(Box::new(editor_state)),
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

pub struct EditableImage<E: api::EditorImplementation> {
    pub editor_implementation: Arc<Box<E>>,
    pub path: OwnedObjectPath,
    dropped: async_lock::OnceCell<()>,
}

#[zbus::interface(name = "org.gnome.glycin.EditableImage")]
impl<E: api::EditorImplementation> EditableImage<E> {
    async fn apply_sparse(
        &self,
        mut edit_request: EditRequest,
    ) -> Result<api::SparseEditorOutput<SharedMemory>, RemoteError> {
        edit_request.initial_seal().await?;
        let operations = edit_request.operations()?;

        let editor_implementation = self.editor_implementation.clone();
        let mut editor_output = blocking::unblock(move || {
            crate::catch_unwind(|| {
                editor_implementation
                    .apply_sparse(operations)
                    .map_err(|x| x.into_loader_error())
            })
            .flatten()
        })
        .fuse();

        futures_util::select! {
            result = editor_output => result,
            _ = self.dropped.wait().fuse() => Err(RemoteError::Aborted),
        }
    }

    /// Same as [`Self::apply()`] but without potential to return sparse changes
    async fn apply_complete(
        &self,
        mut edit_request: EditRequest,
    ) -> Result<api::CompleteEditorOutput<SharedMemory>, RemoteError> {
        edit_request.initial_seal().await?;
        let operations = edit_request.operations()?;

        let editor_implementation = self.editor_implementation.clone();
        let mut editor_output = blocking::unblock(move || {
            crate::catch_unwind(move || {
                editor_implementation
                    .apply_complete(operations)
                    .map_err(|x| x.into_loader_error())
            })
            .flatten()
        })
        .fuse();

        futures_util::select! {
            result = editor_output => result,
            _ = self.dropped.wait().fuse() => Err(RemoteError::Aborted),
        }
    }

    async fn done(
        &self,
        #[zbus(object_server)] object_server: &zbus::ObjectServer,
    ) -> Result<(), RemoteError> {
        log::debug!("Disconnecting {}", self.path);
        let removed = object_server
            .remove::<EditableImage<E>, _>(&self.path)
            .await?;
        if removed {
            log::debug!("Removed {}", self.path);
        } else {
            log::error!("Failed to remove {}", self.path);
        }
        let _ = self.dropped.set(()).await;
        Ok(())
    }
}

/// Give a `None` for a non-existent `EditorImplementation`
pub enum VoidEditorImplementation {}

impl api::EditorImplementation for VoidEditorImplementation {
    const USEABLE: bool = false;

    fn edit<S: Read>(
        _stream: S,
        _mime_type: String,
        _details: api::InitializationDetails,
    ) -> Result<Self, ProcessError> {
        unreachable!()
    }

    fn create<B: ByteData>(
        _mime_type: String,
        _new_image: api::NewImage<B>,
        _encoding_options: api::EncodingOptions,
    ) -> Result<api::EncodedImage<B>, ProcessError> {
        unreachable!()
    }

    fn apply_complete<B: ByteData>(
        &self,
        _operations: Operations,
    ) -> Result<api::CompleteEditorOutput<B>, ProcessError> {
        unreachable!()
    }
}
