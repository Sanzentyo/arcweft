//! Canonical private and public View-part identities.

use arcweft_id::{IdError, PublicId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Private implementation name declared by `.part(name)` inside one View.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewLocalPartName(PublicId);

/// Public capability name used by a caller-side Style selector.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPartName(PublicId);

/// Compact owner-local identity used by one checked View program.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPartId(pub u32);

/// Node-producing instruction kind that owns a local part target.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewPartInstructionKind {
    Element,
    Text,
    Image,
    Custom,
    ViewCall,
}

/// One public export owned by its containing View program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPartExport {
    id: ViewPartId,
    public_name: ViewPartName,
}

/// Failure to construct one internally consistent View program.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewProgramBuildError {
    #[error("View program instruction count {actual} exceeds the u32 index space")]
    InstructionOverflow { actual: usize },
    #[error("local View part {part:?} is attached to more than one static instruction")]
    DuplicateLocalTarget { part: ViewPartId },
    #[error("View part export target {part:?} is already exported")]
    DuplicateExportTarget { part: ViewPartId },
    #[error("public View part name {name:?} is already exported")]
    DuplicatePublicName { name: ViewPartName },
    #[error("View part export references unknown local target {part:?}")]
    UnknownExportTarget { part: ViewPartId },
    #[error("View part {part:?} labels a nested View call and cannot be re-exported")]
    UnsupportedViewCallExport { part: ViewPartId },
}

impl ViewLocalPartName {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdError> {
        PublicId::try_new(value).map(Self)
    }

    pub const fn from_public_id(id: PublicId) -> Self {
        Self(id)
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.0
    }

    /// Compares a same-owner private name with selector spelling without
    /// granting a conversion into the public export namespace.
    pub fn matches_selector(&self, selector: &ViewPartName) -> bool {
        self.0 == selector.0
    }
}

impl ViewPartName {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdError> {
        PublicId::try_new(value).map(Self)
    }

    pub const fn from_public_id(id: PublicId) -> Self {
        Self(id)
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.0
    }
}

impl ViewPartExport {
    pub const fn new(id: ViewPartId, public_name: ViewPartName) -> Self {
        Self { id, public_name }
    }

    pub const fn id(&self) -> ViewPartId {
        self.id
    }

    pub const fn public_name(&self) -> &ViewPartName {
        &self.public_name
    }
}

impl Serialize for ViewLocalPartName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.public_id().as_str())
    }
}

impl<'de> Deserialize<'de> for ViewLocalPartName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

impl Serialize for ViewPartName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.public_id().as_str())
    }
}

impl<'de> Deserialize<'de> for ViewPartName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}
