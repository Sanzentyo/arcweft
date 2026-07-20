//! Revision-bound source-map keys for the final manifest decoder.

use arcweft_manifest_model::{
    ActivityImplementationId, ContentUnitId, ExternalModuleImportId, ProfileId,
};
use arcweft_source::{SourceDocument, SourceSpan, SourceSpanValidationError};
use std::{collections::BTreeMap, sync::Arc};

/// Exact source locations published only with an accepted manifest document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ManifestSourceMap {
    document: Arc<SourceDocument>,
    entries: BTreeMap<ManifestSourceKey, SourceSpan>,
}

impl ManifestSourceMap {
    pub(crate) fn try_new(
        document: Arc<SourceDocument>,
        entries: BTreeMap<ManifestSourceKey, SourceSpan>,
    ) -> Result<Self, SourceSpanValidationError> {
        for span in entries.values() {
            span.validate_for(&document)?;
        }
        Ok(Self { document, entries })
    }

    pub(crate) const fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    pub(crate) fn get(&self, key: &ManifestSourceKey) -> Option<&SourceSpan> {
        self.entries.get(key)
    }
}

/// A typed location within one accepted manifest document.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ManifestSourceKey {
    pub(crate) path: ManifestPath,
    pub(crate) slot: ManifestSourceSlot,
}

/// The syntactic role occupied by a source-map key.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ManifestSourceSlot {
    TableHeader,
    MapKey,
    FieldKey,
    ScalarValue,
    ArrayElement { index: u32 },
}

/// Closed typed path to one manifest field or collection member.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ManifestPath(Box<[ManifestPathSegment]>);

impl ManifestPath {
    pub(crate) fn new(segments: impl Into<Box<[ManifestPathSegment]>>) -> Self {
        Self(segments.into())
    }

    #[cfg(test)]
    pub(crate) fn segments(&self) -> &[ManifestPathSegment] {
        &self.0
    }
}

/// One semantic path segment in the final schema-1 document.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ManifestPathSegment {
    Root(ManifestRootField),
    Package(PackageField),
    Build(BuildField),
    ContentUnit(ContentUnitId),
    ContentUnitField(ContentUnitField),
    ExternalModule(ExternalModuleImportId),
    ExternalModuleField(ExternalModuleField),
    ActivityImplementation(ActivityImplementationId),
    ActivityImplementationField(ActivityImplementationField),
    Profile(ProfileId),
    ProfileField(ProfileField),
    Dialogue(DialogueField),
    InlineFailure(InlineFailureField),
    InlineFallback(InlineFallbackField),
    FallbackStyle(FallbackStyleField),
    Pure(PureField),
    Player(PlayerField),
    Viewport(ViewportField),
    ProfileContent(ContentUnitId),
    ProfileContentField(ProfileContentField),
    ActivityBinding(u32),
    ActivityBindingField(ActivityBindingField),
    Index(u32),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ManifestRootField {
    Schema,
    Package,
    Build,
    ContentUnits,
    ExternalModules,
    ActivityImplementations,
    DefaultProfile,
    Profiles,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum PackageField {
    Id,
    Version,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum BuildField {
    SourceDir,
    TargetDir,
    Incremental,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ContentUnitField {
    Roots,
    Visibility,
    Demand,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ExternalModuleField {
    Mount,
    Metadata,
    MetadataHash,
    ExpectedPackage,
    ExpectedVersion,
    ExpectedModule,
    ExpectedFamily,
    ExpectedAbiHash,
    Visibility,
    Demand,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ActivityImplementationField {
    Module,
    Export,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ProfileField {
    Kind,
    Source,
    Entry,
    Adapter,
    ExternalModules,
    ActivityBindings,
    Dialogue,
    Listen,
    Pure,
    Content,
    Player,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum DialogueField {
    View,
    Style,
    InlineFailure,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum InlineFailureField {
    Kind,
    Fallback,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum InlineFallbackField {
    Kind,
    Text,
    Style,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum FallbackStyleField {
    Kind,
    Styles,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum PureField {
    Backend,
    MathBackend,
    MathWgpuMinElements,
    Workers,
    BatchMinLen,
    ObjectArtifacts,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum PlayerField {
    Viewport,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ViewportField {
    DesignWidth,
    DesignHeight,
    Fit,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ProfileContentField {
    Residency,
    Placement,
    Compression,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ActivityBindingField {
    Activity,
    Implementation,
}
