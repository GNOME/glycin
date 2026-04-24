use std::any::Any;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::panic::RefUnwindSafe;

use glycin_common::Operations;
#[cfg(feature = "external")]
use zbus::zvariant::{self, DeserializeDict, SerializeDict, Type, as_value};

use crate::{
    ByteData, EncodedImage, EncodingOptions, FungibleMemory, GenericContexts,
    InitializationDetails, MemoryAllocationError, NewImage, ProcessError,
};

/// Implement this trait to create an image editor
pub trait EditorImplementation: Send + Sync + Sized + RefUnwindSafe + 'static {
    const USEABLE: bool = true;

    fn edit<S: Read + Any>(
        stream: S,
        mime_type: String,
        details: InitializationDetails,
    ) -> Result<Self, ProcessError>;

    fn create<B: ByteData>(
        mime_type: String,
        new_image: NewImage<B>,
        encoding_options: EncodingOptions,
    ) -> Result<EncodedImage<B>, ProcessError>;

    fn apply_sparse<B: ByteData>(
        &self,
        operations: Operations,
    ) -> Result<SparseEditorOutput<B>, ProcessError> {
        let complete = Self::apply_complete(self, operations)?;

        Ok(SparseEditorOutput::from(complete))
    }

    fn apply_complete<B: ByteData>(
        &self,
        operations: Operations,
    ) -> Result<CompleteEditorOutput<B>, ProcessError>;
}

#[cfg(feature = "external")]
/// Editable image
#[derive(serde::Deserialize, serde::Serialize, Type, Debug, Clone)]
pub struct RemoteEditableImage {
    pub edit_request: zvariant::OwnedObjectPath,
}

#[cfg(feature = "external")]
impl RemoteEditableImage {
    pub fn new(frame_request: zvariant::OwnedObjectPath) -> Self {
        Self {
            edit_request: frame_request,
        }
    }
}

/// Result of a sparse editor operation
///
/// This either contains `byte_changes` or `data`, depending on whether a sparse
/// application of the operations was possible.
#[derive(Debug)]
#[cfg_attr(
    feature = "external",
    derive(Type, serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "external", zvariant(signature = "dict"))]
#[cfg_attr(
    feature = "external",
    serde(bound(
        serialize = "B: ByteData + serde::Serialize + zbus::zvariant::Type + 'static",
        deserialize = "B: ByteData + serde::de::DeserializeOwned + zbus::zvariant::Type + 'static"
    ))
)]
#[non_exhaustive]
pub struct SparseEditorOutput<B: ByteData> {
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub byte_changes: Option<ByteChanges>,
    #[cfg_attr(
        feature = "external",
        serde(
            with = "as_value::optional",
            skip_serializing_if = "Option::is_none",
            default
        )
    )]
    pub data: Option<B>,
    #[cfg_attr(feature = "external", serde(with = "as_value"))]
    pub info: EditorOutputInfo,
}

impl<B: ByteData> SparseEditorOutput<B> {
    pub fn byte_changes(byte_changes: ByteChanges) -> Self {
        SparseEditorOutput {
            byte_changes: Some(byte_changes),
            data: None,
            info: EditorOutputInfo { lossless: true },
        }
    }

    pub fn into_fungible(self) -> SparseEditorOutput<FungibleMemory> {
        SparseEditorOutput {
            byte_changes: self.byte_changes,
            data: self.data.map(|x| x.into_fungible()),
            info: self.info,
        }
    }

    pub async fn initial_seal(&mut self) -> Result<(), MemoryAllocationError> {
        if let Some(data) = &mut self.data {
            data.initial_seal().await?;
        }

        Ok(())
    }

    pub async fn final_seal(&mut self) -> Result<(), MemoryAllocationError> {
        if let Some(data) = &mut self.data {
            data.final_seal().await?;
        }

        Ok(())
    }
}

impl<B: ByteData> From<CompleteEditorOutput<B>> for SparseEditorOutput<B> {
    fn from(value: CompleteEditorOutput<B>) -> Self {
        Self {
            byte_changes: None,
            data: Some(value.data),
            info: value.info,
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "external", derive(DeserializeDict, SerializeDict, Type))]
#[cfg_attr(feature = "external", zvariant(signature = "dict"))]
#[non_exhaustive]
pub struct ByteChanges {
    pub changes: Vec<ByteChange>,
}

#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "external",
    derive(serde::Deserialize, serde::Serialize, Type)
)]
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

    pub fn apply(&self, data: &mut [u8]) -> std::io::Result<()> {
        let mut cur = Cursor::new(data);
        for change in self.changes.iter() {
            cur.seek(SeekFrom::Start(change.offset))?;
            cur.write_all(&[change.new_value])?;
        }
        Ok(())
    }
}

#[derive(Debug)]
#[cfg_attr(
    feature = "external",
    derive(Type, serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "external", zvariant(signature = "dict"))]
#[cfg_attr(
    feature = "external",
    serde(bound(
        serialize = "B: ByteData + serde::Serialize + zbus::zvariant::Type + 'static",
        deserialize = "B: ByteData + serde::de::DeserializeOwned + zbus::zvariant::Type + 'static"
    ))
)]
#[non_exhaustive]
pub struct CompleteEditorOutput<B: ByteData> {
    #[cfg_attr(feature = "external", serde(with = "as_value"))]
    pub data: B,
    #[cfg_attr(feature = "external", serde(with = "as_value"))]
    pub info: EditorOutputInfo,
}

/*
#[cfg(feature = "external")]
impl zvariant::Type for CompleteEditorOutput<crate::SharedMemory> {
    const SIGNATURE: &'static zvariant::Signature = &zvariant::Signature::Dict {
        key: zvariant::signature::Child::Static {
            child: &zvariant::Signature::Str,
        },
        value: zvariant::signature::Child::Static {
            child: &zvariant::Signature::Variant,
        },
    };
}
    */

impl<B: ByteData> CompleteEditorOutput<B> {
    pub fn new(data: B) -> Self {
        Self {
            data,
            info: Default::default(),
        }
    }

    pub fn new_lossless(data: Vec<u8>) -> Result<Self, ProcessError> {
        let data = B::try_from_vec(data).expected_error()?;
        let info = EditorOutputInfo { lossless: true };
        Ok(Self { data, info })
    }

    pub fn into_fungible(self) -> CompleteEditorOutput<FungibleMemory> {
        CompleteEditorOutput {
            data: self.data.into_fungible(),
            info: self.info,
        }
    }

    pub async fn initial_seal(&mut self) -> Result<(), MemoryAllocationError> {
        self.data.initial_seal().await
    }

    pub async fn final_seal(&mut self) -> Result<(), MemoryAllocationError> {
        self.data.final_seal().await
    }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "external", derive(DeserializeDict, SerializeDict, Type))]
#[cfg_attr(feature = "external", zvariant(signature = "dict"))]
#[non_exhaustive]
pub struct EditorOutputInfo {
    /// Operation is considered to be lossless
    ///
    /// Operations are considered lossless when all metadata are kept, no image
    /// data is lost, and no image quality is lost.
    pub lossless: bool,
}
