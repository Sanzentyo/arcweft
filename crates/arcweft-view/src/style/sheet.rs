//! Typed identities and applications for native and inline View Style.

use arcweft_id::{IdError, PublicId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewStyleSheetId(PublicId);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewStyleTokenId(PublicId);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ViewStylePatchId(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ViewStyleSourceId(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewStyleScopeId(u64);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewStyleApplicationTarget {
    Named { sheet: ViewStyleSheetId },
    Inline { patch: ViewStylePatchId },
}

/// Boundary facts recorded where one style application enters a View scope.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewStyleBoundaryFacts {
    nested_view_boundary: bool,
    exported_part: bool,
    inherited_root: bool,
}

/// One ordered sheet or inline-patch application in a retained View scope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewStyleApplication {
    target: ViewStyleApplicationTarget,
    scope: ViewStyleScopeId,
    scope_depth: u16,
    application_order: u32,
    boundary: ViewStyleBoundaryFacts,
}

impl ViewStyleSheetId {
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

impl ViewStyleTokenId {
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

impl ViewStylePatchId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

impl ViewStyleSourceId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

impl ViewStyleScopeId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ViewStyleApplicationTarget {
    pub const fn named(sheet: ViewStyleSheetId) -> Self {
        Self::Named { sheet }
    }

    pub const fn inline(patch: ViewStylePatchId) -> Self {
        Self::Inline { patch }
    }
}

impl ViewStyleBoundaryFacts {
    pub const SAME_VIEW: Self = Self {
        nested_view_boundary: false,
        exported_part: false,
        inherited_root: false,
    };

    pub const fn nested_view(exported_part: bool, inherited_root: bool) -> Self {
        Self {
            nested_view_boundary: true,
            exported_part,
            inherited_root,
        }
    }

    pub const fn is_nested_view_boundary(self) -> bool {
        self.nested_view_boundary
    }

    pub const fn is_exported_part(self) -> bool {
        self.exported_part
    }

    pub const fn allows_inherited_root(self) -> bool {
        self.inherited_root
    }

    pub const fn allows_selector_traversal(self) -> bool {
        !self.nested_view_boundary || self.exported_part
    }
}

impl ViewStyleApplication {
    pub const fn new(
        target: ViewStyleApplicationTarget,
        scope: ViewStyleScopeId,
        scope_depth: u16,
        application_order: u32,
        boundary: ViewStyleBoundaryFacts,
    ) -> Self {
        Self {
            target,
            scope,
            scope_depth,
            application_order,
            boundary,
        }
    }

    pub const fn target(&self) -> &ViewStyleApplicationTarget {
        &self.target
    }

    pub const fn scope(&self) -> ViewStyleScopeId {
        self.scope
    }

    pub const fn scope_depth(&self) -> u16 {
        self.scope_depth
    }

    pub const fn application_order(&self) -> u32 {
        self.application_order
    }

    pub const fn boundary(&self) -> ViewStyleBoundaryFacts {
        self.boundary
    }
}

impl Serialize for ViewStyleSheetId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for ViewStyleSheetId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

impl Serialize for ViewStyleTokenId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for ViewStyleTokenId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

impl Serialize for ViewStyleScopeId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(self.0)
    }
}

impl<'de> Deserialize<'de> for ViewStyleScopeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        u64::deserialize(deserializer).map(Self)
    }
}
