//! Closed diagnostics emitted by the final manifest decoder and admission path.

use arcweft_source::SourceSpan;
use thiserror::Error;

/// Stable code for one final manifest failure class.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ManifestDiagnosticCode {
    TomlSyntax,
    SchemaMissing,
    SchemaUnsupported,
    RequiredPackage,
    UnknownRootKey,
    UnknownTable,
    UnknownField,
    ValueType,
    ValueMissing,
    DuplicateRootKey,
    DuplicateTable,
    DuplicateField,
    DuplicateMapId,
    DuplicateArrayId,
    DuplicateActivityBinding,
    IdInvalid,
    VersionInvalid,
    PathInvalid,
    DigestInvalid,
    EnumInvalid,
    EntityRefInvalid,
    ListenInvalid,
    PureWorkersInvalid,
    PureThresholdInvalid,
    PlayerViewportInvalid,
    InlinePolicyInvalid,
    CharacterNameLocaleInvalid,
    CharacterNameFallbackDuplicate,
    CharacterNameFallbackLimit,
    ProfileNone,
    ProfileMissing,
    ProfileDefaultMissing,
    ReferenceEntryFamily,
    ReferenceExternalModuleMissing,
    ReferenceActivityImplementationMissing,
    ReferenceActivityModuleNotSelected,
    ReferenceContentUnitMissing,
}

impl ManifestDiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TomlSyntax => "manifest.toml.syntax",
            Self::SchemaMissing => "manifest.schema.missing",
            Self::SchemaUnsupported => "manifest.schema.unsupported",
            Self::RequiredPackage => "manifest.required.package",
            Self::UnknownRootKey => "manifest.unknown.root-key",
            Self::UnknownTable => "manifest.unknown.table",
            Self::UnknownField => "manifest.unknown.field",
            Self::ValueType => "manifest.value.type",
            Self::ValueMissing => "manifest.value.missing",
            Self::DuplicateRootKey => "manifest.duplicate.root-key",
            Self::DuplicateTable => "manifest.duplicate.table",
            Self::DuplicateField => "manifest.duplicate.field",
            Self::DuplicateMapId => "manifest.duplicate.map-id",
            Self::DuplicateArrayId => "manifest.duplicate.array-id",
            Self::DuplicateActivityBinding => "manifest.duplicate.activity-binding",
            Self::IdInvalid => "manifest.id.invalid",
            Self::VersionInvalid => "manifest.version.invalid",
            Self::PathInvalid => "manifest.path.invalid",
            Self::DigestInvalid => "manifest.digest.invalid",
            Self::EnumInvalid => "manifest.enum.invalid",
            Self::EntityRefInvalid => "manifest.entity-ref.invalid",
            Self::ListenInvalid => "manifest.listen.invalid",
            Self::PureWorkersInvalid => "manifest.pure.workers-invalid",
            Self::PureThresholdInvalid => "manifest.pure.threshold-invalid",
            Self::PlayerViewportInvalid => "manifest.player.viewport-invalid",
            Self::InlinePolicyInvalid => "manifest.inline-policy.invalid",
            Self::CharacterNameLocaleInvalid => "CHAR_NAME_008_INVALID_LOCALE",
            Self::CharacterNameFallbackDuplicate => "CHAR_NAME_009_DUPLICATE_FALLBACK",
            Self::CharacterNameFallbackLimit => "manifest.character-name.fallback-limit",
            Self::ProfileNone => "manifest.profile.none",
            Self::ProfileMissing => "manifest.profile.missing",
            Self::ProfileDefaultMissing => "manifest.profile.default-missing",
            Self::ReferenceEntryFamily => "manifest.reference.entry-family",
            Self::ReferenceExternalModuleMissing => "manifest.reference.external-module-missing",
            Self::ReferenceActivityImplementationMissing => {
                "manifest.reference.activity-implementation-missing"
            }
            Self::ReferenceActivityModuleNotSelected => {
                "manifest.reference.activity-module-not-selected"
            }
            Self::ReferenceContentUnitMissing => "manifest.reference.content-unit-missing",
        }
    }
}

/// One related range that explains a manifest diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestRelatedSpan {
    label: String,
    span: SourceSpan,
}

impl ManifestRelatedSpan {
    pub(crate) fn new(label: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            label: label.into(),
            span,
        }
    }

    fn sort_key(&self) -> (usize, usize, String) {
        let range = self.span.range();
        (range.start(), range.end(), self.label.clone())
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("manifest diagnostic spans must share one exact source identity")]
pub(crate) struct ManifestDiagnosticSourceMismatch;

/// One revision-bound manifest diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestDiagnostic {
    code: ManifestDiagnosticCode,
    message: String,
    primary: SourceSpan,
    related: Vec<ManifestRelatedSpan>,
}

impl ManifestDiagnostic {
    pub(crate) fn new(
        code: ManifestDiagnosticCode,
        message: impl Into<String>,
        primary: SourceSpan,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            primary,
            related: Vec::new(),
        }
    }

    pub(crate) fn try_new(
        code: ManifestDiagnosticCode,
        message: impl Into<String>,
        primary: SourceSpan,
        mut related: Vec<ManifestRelatedSpan>,
    ) -> Result<Self, ManifestDiagnosticSourceMismatch> {
        if related
            .iter()
            .any(|related| related.span.source() != primary.source())
        {
            return Err(ManifestDiagnosticSourceMismatch);
        }
        related.sort_by_key(ManifestRelatedSpan::sort_key);
        Ok(Self {
            code,
            message: message.into(),
            primary,
            related,
        })
    }

    pub const fn code(&self) -> ManifestDiagnosticCode {
        self.code
    }

    pub const fn primary(&self) -> &SourceSpan {
        &self.primary
    }

    pub fn related(&self) -> &[ManifestRelatedSpan] {
        &self.related
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

/// A non-empty, deterministic set of decoder or lowering failures.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("manifest validation failed")]
pub struct ManifestReport(Vec<ManifestDiagnostic>);

impl ManifestReport {
    pub(crate) fn single(diagnostic: ManifestDiagnostic) -> Self {
        Self(vec![diagnostic])
    }

    pub(crate) fn from_first(
        first: ManifestDiagnostic,
        rest: impl IntoIterator<Item = ManifestDiagnostic>,
    ) -> Self {
        let mut diagnostics = vec![first];
        diagnostics.extend(rest);
        diagnostics.sort_by(|left, right| {
            let left_range = left.primary.range();
            let right_range = right.primary.range();
            (left_range.start(), left_range.end(), left.code.as_str())
                .cmp(&(right_range.start(), right_range.end(), right.code.as_str()))
                .then_with(|| {
                    left.related
                        .iter()
                        .map(ManifestRelatedSpan::sort_key)
                        .cmp(right.related.iter().map(ManifestRelatedSpan::sort_key))
                })
        });
        Self(diagnostics)
    }

    pub fn diagnostics(&self) -> &[ManifestDiagnostic] {
        &self.0
    }
}
