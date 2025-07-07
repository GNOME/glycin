// Copyright (c) 2024 GNOME Foundation Inc.

use std::io::{Cursor, Seek, SeekFrom, Write};
use std::marker::PhantomData;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};

use futures_util::FutureExt;
use glycin_common::{BinaryData, Operations};
use serde::{Deserialize, Serialize};
use zbus::zvariant::{DeserializeDict, OwnedObjectPath, SerializeDict, Type};

use crate::dbus_types::{self, *};
use crate::error::*;

#[derive(DeserializeDict, SerializeDict, Type, Debug)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct EditRequest {
    pub operations: BinaryData,
}

impl EditRequest {
    pub fn for_operations(operations: &Operations) -> Result<Self, RemoteError> {
        let operations = operations
            .to_message_pack()
            .expected_error()
            .map_err(|x| x.into_editor_error())?;
        let operations = BinaryData::from_data(operations)
            .expected_error()
            .map_err(|x| x.into_editor_error())?;
        Ok(Self { operations })
    }

    pub fn operations(&self) -> Result<Operations, RemoteError> {
        let binary_data = self
            .operations
            .get()
            .expected_error()
            .map_err(|x| x.into_editor_error())?;

        let operations = Operations::from_slice(&binary_data)
            .expected_error()
            .map_err(|x| x.into_editor_error())?;

        Ok(operations)
    }
}

/// Result of a sparse editor operation
///
/// This either contains `byte_changes` or `data`, depending on whether a sparse
/// application of the operations was possible.
#[derive(DeserializeDict, SerializeDict, Type, Debug, Clone)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct SparseEditorOutput {
    pub byte_changes: Option<ByteChanges>,
    pub data: Option<BinaryData>,
    pub info: EditorOutputInfo,
}

impl SparseEditorOutput {
    pub fn byte_changes(byte_changes: ByteChanges) -> Self {
        SparseEditorOutput {
            byte_changes: Some(byte_changes),
            data: None,
            info: EditorOutputInfo { lossless: true },
        }
    }
}

impl From<CompleteEditorOutput> for SparseEditorOutput {
    fn from(value: CompleteEditorOutput) -> Self {
        Self {
            byte_changes: None,
            data: Some(value.data),
            info: value.info,
        }
    }
}

#[derive(DeserializeDict, SerializeDict, Type, Debug, Clone)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct ByteChanges {
    pub changes: Vec<ByteChange>,
}

#[derive(Deserialize, Serialize, Type, Debug, Clone)]
pub struct ByteChange {
    pub offset: u64,
    pub new_value: u8,
}

impl ByteChanges {
    pub fn from_slice(changes: &[(u64, u8)]) -> Self {
        ByteChanges {
            changes: changes
                .iter()
                .map(|(offset, new_value)| ByteChange {
                    offset: *offset,
                    new_value: *new_value,
                })
                .collect(),
        }
    }

    pub fn apply(&self, data: &mut [u8]) {
        let mut cur = Cursor::new(data);
        for change in self.changes.iter() {
            cur.seek(SeekFrom::Start(change.offset)).unwrap();
            cur.write_all(&[change.new_value]).unwrap();
        }
    }
}

#[derive(DeserializeDict, SerializeDict, Type, Debug, Clone)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct CompleteEditorOutput {
    pub data: BinaryData,
    pub info: EditorOutputInfo,
}

impl CompleteEditorOutput {
    pub fn new(data: BinaryData) -> Self {
        Self {
            data,
            info: Default::default(),
        }
    }

    pub fn new_lossless(data: Vec<u8>) -> Result<Self, ProcessError> {
        let data = BinaryData::from_data(data).expected_error()?;
        let info = EditorOutputInfo { lossless: true };
        Ok(Self { data, info })
    }
}

#[derive(DeserializeDict, SerializeDict, Type, Debug, Default, Clone)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct EditorOutputInfo {
    /// Operation is considered to be lossless
    ///
    /// Operations are considered lossless when all metadata are kept, no image
    /// data is lost, and no image quality is lost.
    pub lossless: bool,
}

pub struct Editor<E: EditorImplementation> {
    pub editor: PhantomData<E>,
    pub image_id: Mutex<u64>,
}

/// D-Bus interface for image editors
#[zbus::interface(name = "org.gnome.glycin.Editor")]
impl<E: EditorImplementation> Editor<E> {
    async fn create(
        &self,
        mime_type: String,
        new_image: NewImage,
        encoding_options: EncodingOptions,
    ) -> Result<EncodedImage, RemoteError> {
        E::create(mime_type, new_image, encoding_options).map_err(|x| x.into_editor_error())
    }

    async fn edit(
        &self,
        init_request: InitRequest,
        #[zbus(connection)] dbus_connection: &zbus::Connection,
    ) -> Result<dbus_types::RemoteEditableImage, RemoteError> {
        let fd = OwnedFd::from(init_request.fd);
        let stream = UnixStream::from(fd);

        let editor_state = E::edit(stream, init_request.mime_type, init_request.details)
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

        let path =
            OwnedObjectPath::try_from(format!("/org/gnome/glycin/editable_image/{image_id}"))
                .internal_error()
                .map_err(|x| x.into_loader_error())?;

        let dbus_image = dbus_types::RemoteEditableImage::new(path.clone());

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

pub struct EditableImage<E: EditorImplementation> {
    pub editor_implementation: Arc<Box<E>>,
    pub path: OwnedObjectPath,
    dropped: async_lock::OnceCell<()>,
}

#[zbus::interface(name = "org.gnome.glycin.EditableImage")]
impl<E: EditorImplementation> EditableImage<E> {
    async fn apply_sparse(
        &self,
        edit_request: EditRequest,
    ) -> Result<SparseEditorOutput, RemoteError> {
        let operations = edit_request.operations()?;

        let editor_implementation = self.editor_implementation.clone();
        let mut editor_output = blocking::unblock(move || {
            editor_implementation
                .apply_sparse(operations)
                .map_err(|x| x.into_loader_error())
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
        edit_request: EditRequest,
    ) -> Result<CompleteEditorOutput, RemoteError> {
        let operations = edit_request.operations()?;

        let editor_implementation = self.editor_implementation.clone();
        let mut editor_output = blocking::unblock(move || {
            editor_implementation
                .apply_complete(operations)
                .map_err(|x| x.into_loader_error())
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

/// Implement this trait to create an image editor
pub trait EditorImplementation: Send + Sync + Sized + 'static {
    const USEABLE: bool = true;

    fn edit(
        stream: UnixStream,
        mime_type: String,
        details: InitializationDetails,
    ) -> Result<Self, ProcessError>;

    fn create(
        mime_type: String,
        new_image: NewImage,
        encoding_options: EncodingOptions,
    ) -> Result<EncodedImage, ProcessError>;

    fn apply_sparse(&self, operations: Operations) -> Result<SparseEditorOutput, ProcessError> {
        let complete = Self::apply_complete(self, operations)?;

        Ok(SparseEditorOutput::from(complete))
    }

    fn apply_complete(&self, operations: Operations) -> Result<CompleteEditorOutput, ProcessError>;
}

/// Give a `None` for a non-existent `EditorImplementation`
pub enum VoidEditorImplementation {}

impl EditorImplementation for VoidEditorImplementation {
    const USEABLE: bool = false;

    fn edit(
        _stream: UnixStream,
        _mime_type: String,
        _details: InitializationDetails,
    ) -> Result<Self, ProcessError> {
        unreachable!()
    }

    fn create(
        _mime_type: String,
        _new_image: NewImage,
        _encoding_options: EncodingOptions,
    ) -> Result<EncodedImage, ProcessError> {
        unreachable!()
    }

    fn apply_complete(
        &self,
        _operations: Operations,
    ) -> Result<CompleteEditorOutput, ProcessError> {
        unreachable!()
    }
}
