//! Typed product identities and provenance for exported View parts.

use crate::resource_codec::SourceRangeRef;
use arcweft_id::{IdError, PublicId};
use arcweft_view::{ViewId, ViewPartLocalName, ViewPartName};
use core::fmt;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Canonical product reference to one View definition.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewDefinitionRef(PublicId);

/// Owner-qualified private part target in one View definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewOwnedPartRef {
    pub view: ViewDefinitionRef,
    pub part: ViewPartLocalName,
}

/// Exact source ranges retained for one authored export declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewPartExportSourceRef {
    pub declaration: SourceRangeRef,
    pub local_name: SourceRangeRef,
    pub public_name: SourceRangeRef,
}

/// One public View-part capability with typed ownership and provenance.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewExportedPart {
    pub target: ViewOwnedPartRef,
    pub public_name: ViewPartName,
    pub source: ViewPartExportSourceRef,
}

impl ViewDefinitionRef {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdError> {
        PublicId::try_new(value).map(Self)
    }

    /// Constructs a product reference for an engine-owned reserved View.
    pub fn try_new_engine_owned(value: impl Into<String>) -> Result<Self, IdError> {
        PublicId::try_new_engine_owned(value).map(Self)
    }

    pub const fn from_public_id(id: PublicId) -> Self {
        Self(id)
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Projects this accepted product definition owner into its semantic View identity.
    pub fn to_view_id(&self) -> ViewId {
        ViewId::from_public_id(self.0.clone())
    }
}

impl ViewOwnedPartRef {
    pub const fn new(view: ViewDefinitionRef, part: ViewPartLocalName) -> Self {
        Self { view, part }
    }
}

impl fmt::Display for ViewDefinitionRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl ViewPartExportSourceRef {
    pub const fn ranges(&self) -> [&SourceRangeRef; 3] {
        [&self.declaration, &self.local_name, &self.public_name]
    }

    pub fn ranges_mut(&mut self) -> [&mut SourceRangeRef; 3] {
        [
            &mut self.declaration,
            &mut self.local_name,
            &mut self.public_name,
        ]
    }
}

impl Serialize for ViewDefinitionRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.public_id().as_str())
    }
}

impl<'de> Deserialize<'de> for ViewDefinitionRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new_engine_owned(value).map_err(serde::de::Error::custom)
    }
}
