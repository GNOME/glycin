// Copyright (c) 2024 GNOME Foundation Inc.

use std::io::{Cursor, Seek, SeekFrom, Write};
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
    pub fn for_operations(operations: &Operations) -> Result<Self, RemoteError> {
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
    pub info: EditorOutputInfo,
}

impl SparseEditorOutput {
    pub fn bit_changes(changes: &[(u64, u8)]) -> Self {
        let bit_changes = BitChanges::from_slice(changes);

        SparseEditorOutput {
            bit_changes: Some(bit_changes),
            data: None,
            info: Default::default(),
        }
    }

    pub fn data(data: BinaryData) -> Self {
        Self {
            bit_changes: None,
            data: Some(data),
            info: Default::default(),
        }
    }

    pub fn from_complete(complete: CompleteEditorOutput) -> Self {
        let sparse = Self::data(complete.data);

        sparse
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

impl BitChanges {
    pub fn from_slice(changes: &[(u64, u8)]) -> Self {
        BitChanges {
            changes: changes
                .iter()
                .map(|(offset, new_value)| BitChange {
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
            cur.write(&[change.new_value]).unwrap();
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
}

pub enum EditorOuput {
    Sparse(BitChanges, Vec<u8>, EditorOutputInfo),
    Complete(BinaryData, EditorOutputInfo),
}

impl EditorOuput {
    pub fn complete(data: BinaryData) -> Self {
        Self::Complete(data, Default::default())
    }

    pub fn sparse(changes: &[(u64, u8)]) -> Self {
        let bit_changes = BitChanges::from_slice(changes);
        Self::Sparse(bit_changes, Vec::new(), Default::default())
    }

    pub fn into_complete(self) -> CompleteEditorOutput {
        match self {
            Self::Sparse(bit_changes, mut buf, info) => {
                bit_changes.apply(&mut buf);
                let data = BinaryData::from_data(buf).unwrap();
                CompleteEditorOutput { data, info }
            }
            Self::Complete(data, info) => CompleteEditorOutput { data, info },
        }
    }

    pub fn into_sparse(self) -> SparseEditorOutput {
        match self {
            Self::Sparse(bit_changes, _, info) => SparseEditorOutput {
                bit_changes: Some(bit_changes),
                info,
                data: None,
            },
            EditorOuput::Complete(data, info) => SparseEditorOutput {
                data: Some(data),
                info,
                bit_changes: None,
            },
        }
    }
}

#[derive(DeserializeDict, SerializeDict, Type, Debug, Default, Clone)]
#[zvariant(signature = "dict")]
#[non_exhaustive]
pub struct EditorOutputInfo {
    /// Operation is considered to be lossless
    ///
    /// Operations are considered lossless when all metadata are kept, no image
    /// data is los, and no image quality is lost.
    pub lossless: bool,
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
            .apply_sparse(
                stream,
                init_request.mime_type,
                init_request.details,
                operations,
            )
            .map_err(|x| x.into_editor_error())?;

        Ok(image_info)
    }

    /// Same as [`Self::apply()`] but without potential to return sparse changes
    async fn apply_complete(
        &self,
        init_request: InitRequest,
        edit_request: EditRequest,
    ) -> Result<CompleteEditorOutput, RemoteError> {
        let fd: OwnedFd = OwnedFd::from(init_request.fd);
        let stream = UnixStream::from(fd);
        let operations = edit_request.operations()?;

        let image_info = self
            .get_editor()?
            .apply_complete(
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
    fn apply_sparse(
        &self,
        stream: UnixStream,
        mime_type: String,
        details: InitializationDetails,
        operations: Operations,
    ) -> Result<SparseEditorOutput, ProcessError> {
        let complete = self.apply_complete(stream, mime_type, details, operations)?;

        Ok(SparseEditorOutput::from_complete(complete))
    }

    fn apply_complete(
        &self,
        stream: UnixStream,
        mime_type: String,
        details: InitializationDetails,
        operations: Operations,
    ) -> Result<CompleteEditorOutput, ProcessError>;
}

/// Give a `None` for a non-existent `EditorImplementation`
pub fn void_editor_none() -> Option<impl EditorImplementation> {
    enum Void {}

    impl EditorImplementation for Void {
        fn apply_sparse(
            &self,
            _stream: UnixStream,
            _mime_type: String,
            _details: InitializationDetails,
            _operations: Operations,
        ) -> Result<SparseEditorOutput, ProcessError> {
            match *self {}
        }

        fn apply_complete(
            &self,
            _stream: UnixStream,
            _mime_type: String,
            _details: InitializationDetails,
            _operations: Operations,
        ) -> Result<CompleteEditorOutput, ProcessError> {
            match *self {}
        }
    }

    None::<Void>
}
