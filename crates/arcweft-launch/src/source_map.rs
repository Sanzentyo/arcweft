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

/// Revision-bound semantic coordinate in one accepted manifest document.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ManifestTokenPath {
    ProfileCharacterNamesTable { profile: ProfileId },
    ProfileCharacterNamesActive { profile: ProfileId },
    ProfileCharacterNamesFallback { profile: ProfileId, ordinal: u16 },
}

/// Source-token role requested for a [`ManifestTokenPath`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ManifestTokenSlot {
    TableHeader,
    FieldKey,
    Value,
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

impl ManifestTokenPath {
    pub(crate) fn source_key(&self, slot: ManifestTokenSlot) -> Option<ManifestSourceKey> {
        let (path, source_slot) = match self {
            Self::ProfileCharacterNamesTable { profile } => (
                character_names_path(profile, []),
                match slot {
                    ManifestTokenSlot::TableHeader => ManifestSourceSlot::TableHeader,
                    ManifestTokenSlot::FieldKey | ManifestTokenSlot::Value => return None,
                },
            ),
            Self::ProfileCharacterNamesActive { profile } => {
                let path = character_names_path(
                    profile,
                    [ManifestPathSegment::CharacterNames(
                        CharacterNamesField::Active,
                    )],
                );
                let source_slot = match slot {
                    ManifestTokenSlot::FieldKey => ManifestSourceSlot::FieldKey,
                    ManifestTokenSlot::Value => ManifestSourceSlot::ScalarValue,
                    ManifestTokenSlot::TableHeader => return None,
                };
                (path, source_slot)
            }
            Self::ProfileCharacterNamesFallback { profile, ordinal } => {
                if slot != ManifestTokenSlot::Value {
                    return None;
                }
                (
                    character_names_path(
                        profile,
                        [ManifestPathSegment::CharacterNames(
                            CharacterNamesField::Fallbacks,
                        )],
                    ),
                    ManifestSourceSlot::ArrayElement {
                        index: u32::from(*ordinal),
                    },
                )
            }
        };
        Some(ManifestSourceKey {
            path,
            slot: source_slot,
        })
    }
}

fn character_names_path(
    profile: &ProfileId,
    tail: impl IntoIterator<Item = ManifestPathSegment>,
) -> ManifestPath {
    let mut segments = vec![
        ManifestPathSegment::Root(ManifestRootField::Profiles),
        ManifestPathSegment::Profile(profile.clone()),
        ManifestPathSegment::ProfileField(ProfileField::Localization),
        ManifestPathSegment::Localization(LocalizationField::CharacterNames),
    ];
    segments.extend(tail);
    ManifestPath::new(segments)
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
    Localization(LocalizationField),
    CharacterNames(CharacterNamesField),
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
    Localization,
    Listen,
    Pure,
    Content,
    Player,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum LocalizationField {
    CharacterNames,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CharacterNamesField {
    Active,
    Fallbacks,
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
