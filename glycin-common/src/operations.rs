use std::io::Read;
use std::str::FromStr;

use gufo_common::orientation::{Orientation, Rotation};
use serde::de::{value, IntoDeserializer};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub enum Operation {
    Clip((u32, u32, u32, u32)),
    MirrorHorizontally,
    MirrorVertically,
    /// Counter-clockwise rotation
    Rotate(gufo_common::orientation::Rotation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub enum OperationId {
    Clip,
    MirrorHorizontally,
    MirrorVertically,
    Rotate,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(from = "OperationsIntermediate")]
pub struct Operations {
    operations: Vec<Operation>,
    #[serde(skip)]
    unknown_operations: Vec<String>,
}

impl Operations {
    pub fn new(operations: Vec<Operation>) -> Operations {
        Self {
            operations,
            unknown_operations: vec![],
        }
    }

    /// Creates new operations that apply the specified orientation
    pub fn new_orientation(orientation: Orientation) -> Operations {
        let mut operations = Vec::new();

        if orientation.mirror() {
            operations.push(Operation::MirrorHorizontally);
        }

        let rotate = orientation.rotate();
        if rotate != Rotation::_0 {
            operations.push(Operation::Rotate(rotate));
        }

        Self {
            operations,
            unknown_operations: Vec::new(),
        }
    }

    /// Prepend operations
    ///
    /// ```
    /// # use glycin_common::{Operation, Operations};
    /// # use gufo_common::orientation::{Orientation, Rotation};
    /// let mut ops = Operations::new(vec![Operation::MirrorVertically]);
    /// ops.prepend(Operations::new_orientation(Orientation::Rotation90));
    ///
    /// assert_eq!(
    ///     ops.operations(),
    ///     &[
    ///         Operation::Rotate(Rotation::_90),
    ///         Operation::MirrorVertically
    ///     ]
    /// );
    /// ```
    pub fn prepend(&mut self, mut operations: Operations) {
        std::mem::swap(self, &mut operations);
        self.operations.append(&mut operations.operations);
    }

    pub fn from_read(reader: impl Read) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::decode::from_read(reader)
    }

    pub fn from_slice(slice: impl AsRef<[u8]>) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::decode::from_slice(slice.as_ref())
    }

    pub fn to_message_pack(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        let mut buf = Vec::new();
        self.serialize(&mut rmp_serde::Serializer::new(&mut buf).with_human_readable())?;

        Ok(buf)
    }

    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    pub fn operation_ids(&self) -> Vec<OperationId> {
        self.operations.iter().map(|x| x.id()).collect()
    }

    /// Returns information about all operations that were unknown when
    /// deserializing
    pub fn unknown_operations(&self) -> &[String] {
        &self.unknown_operations
    }

    /// Returns an [`Orientation`] if all operations can be reduced to that
    ///
    /// ```
    /// # use glycin_common::{Operations, Operation};
    /// # use gufo_common::orientation::{Rotation, Orientation};
    /// assert_eq!(
    ///     Operations::new(vec![
    ///         Operation::Rotate(Rotation::_180),
    ///         Operation::Rotate(Rotation::_270)
    ///     ])
    ///     .orientation(),
    ///     Some(Orientation::Rotation90)
    /// );
    ///
    /// assert_eq!(
    ///     Operations::new(vec![
    ///         Operation::Rotate(Rotation::_90),
    ///         Operation::MirrorHorizontally
    ///     ])
    ///     .orientation(),
    ///     Some(Orientation::MirroredRotation270)
    /// );
    ///
    /// assert_eq!(
    ///     Operations::new(vec![
    ///         Operation::MirrorHorizontally,
    ///         Operation::MirrorVertically,
    ///         Operation::Rotate(Rotation::_270),
    ///         Operation::MirrorHorizontally,
    ///     ])
    ///     .orientation(),
    ///     Some(Orientation::MirroredRotation270)
    /// );
    /// ```
    pub fn orientation(&self) -> Option<Orientation> {
        let mut orientation = Orientation::Id;

        for operation in &self.operations {
            match operation {
                Operation::MirrorHorizontally => {
                    orientation = orientation.add_mirror_horizontally();
                }
                Operation::MirrorVertically => {
                    orientation = orientation.add_mirror_vertically();
                }
                Operation::Rotate(rotation) => {
                    orientation = orientation.add_rotation(*rotation);
                }
                _ => return None,
            }
        }

        Some(orientation)
    }
}

impl From<OperationsIntermediate> for Operations {
    fn from(operations: OperationsIntermediate) -> Operations {
        Operations {
            operations: operations
                .operations
                .iter()
                .filter_map(|x| x.operation().cloned())
                .collect(),

            unknown_operations: operations
                .operations
                .iter()
                .filter_map(|x| x.unknown())
                .collect(),
        }
    }
}

/// Decoding format that allows to decode without failing for unknown operations
#[derive(Debug, PartialEq, Deserialize)]
struct OperationsIntermediate {
    operations: Vec<MaybeOperation>,
}

#[derive(Debug, PartialEq)]
enum MaybeOperation {
    Operation(Operation),
    Unknown(String),
}

impl MaybeOperation {
    fn operation(&self) -> Option<&Operation> {
        match self {
            Self::Operation(operation) => Some(operation),
            Self::Unknown(_) => None,
        }
    }

    fn unknown(&self) -> Option<String> {
        match self {
            Self::Operation(_) => None,
            Self::Unknown(s) => Some(s.to_string()),
        }
    }
}

impl<'de> Deserialize<'de> for MaybeOperation {
    fn deserialize<D>(deserializer: D) -> Result<MaybeOperation, D::Error>
    where
        D: Deserializer<'de>,
    {
        match serde::Deserialize::deserialize(deserializer) {
            Ok(val) => Ok(Self::Operation(val)),
            Err(err) => Ok(Self::Unknown(err.to_string())),
        }
    }
}

impl Operation {
    pub fn id(&self) -> OperationId {
        match self {
            Self::Clip(_) => OperationId::Clip,
            Self::MirrorHorizontally => OperationId::MirrorHorizontally,
            Self::MirrorVertically => OperationId::MirrorVertically,
            Self::Rotate(_) => OperationId::Rotate,
        }
    }
}

impl FromStr for OperationId {
    type Err = value::Error;

    /// ```
    /// # use glycin_common::OperationId;
    /// # use std::str::FromStr;
    /// let id = OperationId::from_str("Clip").unwrap();
    /// assert_eq!(id, OperationId::Clip)
    /// ```
    fn from_str(slice: &str) -> Result<Self, value::Error> {
        Self::deserialize(slice.into_deserializer())
    }
}
