// Copyright (c) 2024 GNOME Foundation Inc.

use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use zbus::zvariant::{DeserializeDict, SerializeDict, Type};

use crate::dbus::*;
use crate::error::*;
use crate::operations::Operations;

#[derive(DeserializeDict, SerializeDict, Type, Debug)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct EditRequest {
    pub operations: BinaryData,
}

impl EditRequest {
    pub fn for_operations(operations: Operations) -> Result<Self, RemoteError> {
        let operations = operations
            .to_message_pack()
            .expected_error()
            .map_err(|x| x.into_editor_error())?;
        let operations = BinaryData::from_data(operations).map_err(|x| x.into_editor_error())?;
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

#[derive(DeserializeDict, SerializeDict, Type, Debug, Clone)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct SparseEditorOutput {
    pub bit_changes: Option<BitChanges>,
    pub data: Option<BinaryData>,
}

impl SparseEditorOutput {
    pub fn bit_changes(changes: &[(u64, u8)]) -> Self {
        let bit_changes = BitChanges {
            changes: changes
                .iter()
                .map(|(offset, new_value)| BitChange {
                    offset: *offset,
                    new_value: *new_value,
                })
                .collect(),
        };

        SparseEditorOutput {
            bit_changes: Some(bit_changes),
            data: None,
        }
    }
}

#[derive(DeserializeDict, SerializeDict, Type, Debug, Clone)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct BitChanges {
    pub changes: Vec<BitChange>,
}

#[derive(Deserialize, Serialize, Type, Debug, Clone)]
pub struct BitChange {
    pub offset: u64,
    pub new_value: u8,
}

pub struct Editor {
    pub editor: Mutex<Box<dyn EditorImplementation>>,
}

/// D-Bus interface for image editors
#[zbus::interface(name = "org.gnome.glycin.Editor")]
impl Editor {
    async fn apply(
        &self,
        init_request: InitRequest,
        edit_request: EditRequest,
    ) -> Result<SparseEditorOutput, RemoteError> {
        let fd: OwnedFd = OwnedFd::from(init_request.fd);
        let stream = UnixStream::from(fd);
        let operations = edit_request.operations()?;

        let image_info = self
            .get_editor()?
            .apply(
                stream,
                init_request.mime_type,
                init_request.details,
                operations,
            )
            .map_err(|x| x.into_editor_error())?;

        Ok(image_info)
    }
}

impl Editor {
    pub fn get_editor(&self) -> Result<MutexGuard<Box<dyn EditorImplementation>>, RemoteError> {
        self.editor.lock().map_err(|err| {
            RemoteError::InternalLoaderError(format!("Failed to lock editor for operation: {err}"))
        })
    }
}

/// Implement this trait to create an image editor
pub trait EditorImplementation: Send {
    fn apply(
        &self,
        stream: UnixStream,
        mime_type: String,
        details: InitializationDetails,
        operations: Operations,
    ) -> Result<SparseEditorOutput, ProcessError>;
}

pub fn void_editor_none() -> Option<impl EditorImplementation> {
    enum Void {}

    impl EditorImplementation for Void {
        fn apply(
            &self,
            _stream: UnixStream,
            _mime_type: String,
            _details: InitializationDetails,
            _operations: Operations,
        ) -> Result<SparseEditorOutput, ProcessError> {
            match *self {}
        }
    }

    None::<Void>
}
