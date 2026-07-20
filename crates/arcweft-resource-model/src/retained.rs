//! Closed retained-identity references used by configured resource values.

use crate::{identity::ResourceFieldId, value::ResourceConstValue};
use arcweft_id::{EntityId, PublicId};
use arcweft_source::SourceSpan;

/// Exact retained identity family accepted by resource descriptor fields.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RetainedIdentityKind {
    Character,
    View,
    Action,
    Layer,
    Signal,
    PresentationTarget,
    ScrollRegion,
}

/// Stable scope of an accepted presentation target.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PresentationTargetScope {
    Global,
    View { owner_view_entity_id: EntityId },
}

/// Canonical retained target after accepted-project resolution.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResolvedRetainedIdentityRef {
    Character {
        entity_id: EntityId,
    },
    View {
        entity_id: EntityId,
    },
    Action {
        entity_id: EntityId,
    },
    Layer {
        entity_id: EntityId,
    },
    Signal {
        entity_id: EntityId,
    },
    PresentationTarget {
        scope: PresentationTargetScope,
        target_id: PublicId,
    },
    ScrollRegion {
        owner_view_entity_id: EntityId,
        region_id: PublicId,
    },
}

/// Canonical map key embedded in a retained dependency value path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceConstMapKey(ResourceConstValue);

/// One stable segment of a resource value's dependency path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceValuePathSegment {
    Field(ResourceFieldId),
    RecordField(ResourceFieldId),
    ListIndex(u32),
    MapKey(ResourceConstMapKey),
}

/// Canonical descriptor-relative path of one resource value occurrence.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceValuePath(Vec<ResourceValuePathSegment>);

/// One source-aware dependency from a resource to a retained identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedIdentityDependency {
    from_resource: EntityId,
    value_path: ResourceValuePath,
    target: ResolvedRetainedIdentityRef,
    source: SourceSpan,
}

impl RetainedIdentityKind {
    pub const ALL: [Self; 7] = [
        Self::Character,
        Self::View,
        Self::Action,
        Self::Layer,
        Self::Signal,
        Self::PresentationTarget,
        Self::ScrollRegion,
    ];

    /// Canonical closed manifest token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Character => "character",
            Self::View => "view",
            Self::Action => "action",
            Self::Layer => "layer",
            Self::Signal => "signal",
            Self::PresentationTarget => "presentation_target",
            Self::ScrollRegion => "scroll_region",
        }
    }

    /// Parses one canonical closed manifest token.
    pub fn from_manifest_token(value: &str) -> Option<Self> {
        match value {
            "character" => Some(Self::Character),
            "view" => Some(Self::View),
            "action" => Some(Self::Action),
            "layer" => Some(Self::Layer),
            "signal" => Some(Self::Signal),
            "presentation_target" => Some(Self::PresentationTarget),
            "scroll_region" => Some(Self::ScrollRegion),
            _ => None,
        }
    }
}

impl ResolvedRetainedIdentityRef {
    /// Exact retained family of this resolved identity.
    pub const fn kind(&self) -> RetainedIdentityKind {
        match self {
            Self::Character { .. } => RetainedIdentityKind::Character,
            Self::View { .. } => RetainedIdentityKind::View,
            Self::Action { .. } => RetainedIdentityKind::Action,
            Self::Layer { .. } => RetainedIdentityKind::Layer,
            Self::Signal { .. } => RetainedIdentityKind::Signal,
            Self::PresentationTarget { .. } => RetainedIdentityKind::PresentationTarget,
            Self::ScrollRegion { .. } => RetainedIdentityKind::ScrollRegion,
        }
    }

    /// Canonical global declaration identity, when this target is global.
    pub const fn entity_id(&self) -> Option<&EntityId> {
        match self {
            Self::Character { entity_id }
            | Self::View { entity_id }
            | Self::Action { entity_id }
            | Self::Layer { entity_id }
            | Self::Signal { entity_id } => Some(entity_id),
            Self::PresentationTarget { .. } | Self::ScrollRegion { .. } => None,
        }
    }

    /// View owner contributed by a View or View-scoped presentation identity.
    pub const fn effective_view_owner(&self) -> Option<&EntityId> {
        match self {
            Self::View { entity_id } => Some(entity_id),
            Self::PresentationTarget {
                scope:
                    PresentationTargetScope::View {
                        owner_view_entity_id,
                    },
                ..
            }
            | Self::ScrollRegion {
                owner_view_entity_id,
                ..
            } => Some(owner_view_entity_id),
            Self::Character { .. }
            | Self::Action { .. }
            | Self::Layer { .. }
            | Self::Signal { .. }
            | Self::PresentationTarget {
                scope: PresentationTargetScope::Global,
                ..
            } => None,
        }
    }
}

impl ResourceConstMapKey {
    pub const fn new(value: ResourceConstValue) -> Self {
        Self(value)
    }

    pub const fn value(&self) -> &ResourceConstValue {
        &self.0
    }
}

impl ResourceValuePath {
    pub fn new(segments: impl IntoIterator<Item = ResourceValuePathSegment>) -> Self {
        Self(segments.into_iter().collect())
    }

    pub fn segments(&self) -> &[ResourceValuePathSegment] {
        &self.0
    }
}

impl RetainedIdentityDependency {
    pub const fn new(
        from_resource: EntityId,
        value_path: ResourceValuePath,
        target: ResolvedRetainedIdentityRef,
        source: SourceSpan,
    ) -> Self {
        Self {
            from_resource,
            value_path,
            target,
            source,
        }
    }

    pub const fn from_resource(&self) -> &EntityId {
        &self.from_resource
    }

    pub const fn value_path(&self) -> &ResourceValuePath {
        &self.value_path
    }

    pub const fn target(&self) -> &ResolvedRetainedIdentityRef {
        &self.target
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }
}
