use arcweft_character::{
    id::{CharacterId, CharacterPartId},
    manifest::{
        CharacterManifestFingerprint,
        diagnostic::{CharacterIdentifierDomain, JsonStructuralErrorKind},
        registration::{CharacterManifestTokenPath, JsonObjectPath},
    },
};
use arcweft_lang_hir::symbol::{
    ExternalDeclarationId, ProjectSymbolDiagnosticCode, ProjectSymbolLinkError,
    ProjectSymbolRevision, ProjectSymbolTargetId, ProjectSymbolWorldId,
};
use arcweft_lang_syntax::ast::symbol_path::SymbolPath;
use arcweft_source::{
    Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceDocumentId, SourceDocumentIdError,
    SourceRange, SourceRevision, SourceSpan, SourceSpanError,
};

use crate::{callable::CallableDiagnosticCode, env::nominal::AcceptedNominalCatalogError};

use super::{
    limits::{CharacterRegistrationLimitKind, CharacterRegistrationLimits},
    model::{
        CharacterInventoryDigest, CharacterInventoryRevision, RegisteredExternalOwner,
        RegisteredExternalOwnerKind,
    },
    source_index::{CharacterDefinitionIndexBuildError, CharacterDefinitionIndexCode},
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterRegistrationCode {
    InvalidDocumentId,
    SourceRange,
    ManifestBytesLimit,
    DuplicateJsonKey,
    ManifestSyntax,
    InvalidIdentifier,
    DuplicateCatalogOwner,
    UnknownPart,
    ConflictingManifest,
    UnknownOwner,
    AliasCollision,
    MissingProvenance,
    WrongDocument,
    WrongRevision,
    StaleSource,
    SourceDigestCollision,
    Project(ProjectSymbolDiagnosticCode),
    CallableCatalog(CallableDiagnosticCode),
    AcceptedNominalCatalog,
    ExternalUnknown,
    ExternalDuplicate,
    ExternalConflict,
    ExternalWrongKind,
    ExternalStale,
    Limit,
    ArithmeticOverflow,
    DigestCollision,
    DescriptorTamper,
    RevisionOverflow,
    WorkOverflow,
    DefinitionIndex(CharacterDefinitionIndexCode),
}

impl CharacterRegistrationCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidDocumentId => "aw.character.source.invalid_document_id",
            Self::SourceRange => "aw.character.source.range",
            Self::ManifestBytesLimit => "aw.character.manifest.bytes_limit",
            Self::DuplicateJsonKey => "aw.character.manifest.duplicate_key",
            Self::ManifestSyntax => "aw.character.manifest.syntax",
            Self::InvalidIdentifier => "aw.character.manifest.invalid_identifier",
            Self::DuplicateCatalogOwner => "aw.character.catalog.duplicate_owner",
            Self::UnknownPart => "aw.character.manifest.unknown_part",
            Self::ConflictingManifest => "aw.character.registration.conflicting_manifest",
            Self::UnknownOwner => "aw.character.registration.unknown_owner",
            Self::AliasCollision => "aw.character.registration.alias_collision",
            Self::MissingProvenance => "aw.character.registration.missing_provenance",
            Self::WrongDocument => "aw.character.registration.wrong_document",
            Self::WrongRevision => "aw.character.registration.wrong_revision",
            Self::StaleSource => "aw.character.registration.stale_source",
            Self::SourceDigestCollision => "aw.character.source.digest_collision",
            Self::Project(code) => code.as_str(),
            Self::CallableCatalog(_) => "aw.callable.catalog.registration",
            Self::AcceptedNominalCatalog => "aw.nominal.catalog.registration",
            Self::ExternalUnknown => "aw.character.registration.external_unknown",
            Self::ExternalDuplicate => "aw.character.registration.external_duplicate",
            Self::ExternalConflict => "aw.character.registration.external_conflict",
            Self::ExternalWrongKind => "aw.character.registration.external_wrong_kind",
            Self::ExternalStale => "aw.character.registration.external_stale",
            Self::Limit => "aw.character.registration.limit",
            Self::ArithmeticOverflow => "aw.character.registration.arithmetic_overflow",
            Self::DigestCollision => "aw.character.registration.digest_collision",
            Self::DescriptorTamper => "aw.character.registration.descriptor_tamper",
            Self::RevisionOverflow => "aw.character.registration.revision_overflow",
            Self::WorkOverflow => "aw.character.registration.work_overflow",
            Self::DefinitionIndex(code) => code.as_str(),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RequiredCharacterToken {
    Manifest(CharacterManifestTokenPath),
    CatalogDeclaration,
    LaunchCharacterManifest { index: usize },
    ExternalDeclaration,
    ExternalOwner,
    DirectBinding { index: usize },
    ImportPath,
    ImportAlias,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CharacterRegistrationDiagnosticKind {
    InvalidDocumentId {
        value: String,
        reason: SourceDocumentIdError,
    },
    SourceRange {
        reason: SourceSpanError,
    },
    ManifestBytesLimit {
        observed: u64,
        maximum: u64,
    },
    DuplicateJsonKey {
        object: JsonObjectPath,
        key: String,
    },
    ManifestSyntax {
        kind: JsonStructuralErrorKind,
    },
    InvalidIdentifier {
        domain: CharacterIdentifierDomain,
        value: String,
    },
    DuplicateCatalogOwner {
        owner: CharacterId,
    },
    UnknownPart {
        owner: CharacterId,
        part: CharacterPartId,
    },
    ConflictingManifest {
        owner: CharacterId,
        first: CharacterManifestFingerprint,
        conflicting: CharacterManifestFingerprint,
    },
    UnknownOwner {
        owner: RegisteredExternalOwner,
    },
    AliasCollision {
        spelling: SymbolPath,
        expected: ExternalDeclarationId,
        conflicting: Vec<ProjectSymbolTargetId>,
    },
    MissingProvenance {
        token: RequiredCharacterToken,
    },
    WrongDocument {
        expected: SourceDocumentId,
        actual: SourceDocumentId,
    },
    WrongRevision {
        expected: SourceRevision,
        actual: SourceRevision,
    },
    StaleSource {
        expected: ProjectSymbolRevision,
        actual: ProjectSymbolRevision,
    },
    SourceDigestCollision {
        id: SourceDocumentId,
        revision: SourceRevision,
    },
    ProjectSymbol {
        error: ProjectSymbolLinkError,
    },
    CallableCatalog {
        code: CallableDiagnosticCode,
    },
    AcceptedNominalCatalog {
        error: AcceptedNominalCatalogError,
    },
    ExternalUnknown {
        declaration: ExternalDeclarationId,
    },
    ExternalDuplicate {
        declaration: ExternalDeclarationId,
        owner: RegisteredExternalOwner,
    },
    ExternalConflict {
        declaration: ExternalDeclarationId,
        first: RegisteredExternalOwner,
        conflicting: RegisteredExternalOwner,
    },
    ExternalWrongKind {
        declaration: ExternalDeclarationId,
        expected: RegisteredExternalOwnerKind,
        actual: RegisteredExternalOwnerKind,
    },
    ExternalStale {
        expected_world: ProjectSymbolWorldId,
        actual_world: ProjectSymbolWorldId,
        expected_revision: ProjectSymbolRevision,
        actual_revision: ProjectSymbolRevision,
    },
    Limit {
        kind: CharacterRegistrationLimitKind,
        observed: u64,
        maximum: u64,
    },
    ArithmeticOverflow {
        counter: CharacterRegistrationLimitKind,
    },
    DigestCollision {
        owner: CharacterId,
        digest: CharacterManifestFingerprint,
    },
    DescriptorTamper {
        expected: CharacterInventoryDigest,
        actual: CharacterInventoryDigest,
    },
    RevisionOverflow {
        previous: CharacterInventoryRevision,
    },
    WorkOverflow {
        attempted: u64,
        maximum: u64,
    },
    DefinitionIndex {
        error: CharacterDefinitionIndexBuildError,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterRegistrationDiagnostic {
    kind: CharacterRegistrationDiagnosticKind,
    primary: SourceSpan,
    secondary: Vec<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterRegistrationReport {
    diagnostics: Vec<CharacterRegistrationDiagnostic>,
    omitted_diagnostics: u64,
}

impl CharacterRegistrationDiagnosticKind {
    pub const fn code(&self) -> CharacterRegistrationCode {
        match self {
            CharacterRegistrationDiagnosticKind::InvalidDocumentId { .. } => {
                CharacterRegistrationCode::InvalidDocumentId
            }
            CharacterRegistrationDiagnosticKind::SourceRange { .. } => {
                CharacterRegistrationCode::SourceRange
            }
            CharacterRegistrationDiagnosticKind::ManifestBytesLimit { .. } => {
                CharacterRegistrationCode::ManifestBytesLimit
            }
            CharacterRegistrationDiagnosticKind::DuplicateJsonKey { .. } => {
                CharacterRegistrationCode::DuplicateJsonKey
            }
            CharacterRegistrationDiagnosticKind::ManifestSyntax { .. } => {
                CharacterRegistrationCode::ManifestSyntax
            }
            CharacterRegistrationDiagnosticKind::InvalidIdentifier { .. } => {
                CharacterRegistrationCode::InvalidIdentifier
            }
            CharacterRegistrationDiagnosticKind::DuplicateCatalogOwner { .. } => {
                CharacterRegistrationCode::DuplicateCatalogOwner
            }
            CharacterRegistrationDiagnosticKind::UnknownPart { .. } => {
                CharacterRegistrationCode::UnknownPart
            }
            CharacterRegistrationDiagnosticKind::ConflictingManifest { .. } => {
                CharacterRegistrationCode::ConflictingManifest
            }
            CharacterRegistrationDiagnosticKind::UnknownOwner { .. } => {
                CharacterRegistrationCode::UnknownOwner
            }
            CharacterRegistrationDiagnosticKind::AliasCollision { .. } => {
                CharacterRegistrationCode::AliasCollision
            }
            CharacterRegistrationDiagnosticKind::MissingProvenance { .. } => {
                CharacterRegistrationCode::MissingProvenance
            }
            CharacterRegistrationDiagnosticKind::WrongDocument { .. } => {
                CharacterRegistrationCode::WrongDocument
            }
            CharacterRegistrationDiagnosticKind::WrongRevision { .. } => {
                CharacterRegistrationCode::WrongRevision
            }
            CharacterRegistrationDiagnosticKind::StaleSource { .. } => {
                CharacterRegistrationCode::StaleSource
            }
            CharacterRegistrationDiagnosticKind::SourceDigestCollision { .. } => {
                CharacterRegistrationCode::SourceDigestCollision
            }
            CharacterRegistrationDiagnosticKind::ProjectSymbol { error } => {
                CharacterRegistrationCode::Project(error.code())
            }
            CharacterRegistrationDiagnosticKind::CallableCatalog { code } => {
                CharacterRegistrationCode::CallableCatalog(*code)
            }
            CharacterRegistrationDiagnosticKind::AcceptedNominalCatalog { .. } => {
                CharacterRegistrationCode::AcceptedNominalCatalog
            }
            CharacterRegistrationDiagnosticKind::ExternalUnknown { .. } => {
                CharacterRegistrationCode::ExternalUnknown
            }
            CharacterRegistrationDiagnosticKind::ExternalDuplicate { .. } => {
                CharacterRegistrationCode::ExternalDuplicate
            }
            CharacterRegistrationDiagnosticKind::ExternalConflict { .. } => {
                CharacterRegistrationCode::ExternalConflict
            }
            CharacterRegistrationDiagnosticKind::ExternalWrongKind { .. } => {
                CharacterRegistrationCode::ExternalWrongKind
            }
            CharacterRegistrationDiagnosticKind::ExternalStale { .. } => {
                CharacterRegistrationCode::ExternalStale
            }
            CharacterRegistrationDiagnosticKind::Limit { .. } => CharacterRegistrationCode::Limit,
            CharacterRegistrationDiagnosticKind::ArithmeticOverflow { .. } => {
                CharacterRegistrationCode::ArithmeticOverflow
            }
            CharacterRegistrationDiagnosticKind::DigestCollision { .. } => {
                CharacterRegistrationCode::DigestCollision
            }
            CharacterRegistrationDiagnosticKind::DescriptorTamper { .. } => {
                CharacterRegistrationCode::DescriptorTamper
            }
            CharacterRegistrationDiagnosticKind::RevisionOverflow { .. } => {
                CharacterRegistrationCode::RevisionOverflow
            }
            CharacterRegistrationDiagnosticKind::WorkOverflow { .. } => {
                CharacterRegistrationCode::WorkOverflow
            }
            CharacterRegistrationDiagnosticKind::DefinitionIndex { error } => {
                CharacterRegistrationCode::DefinitionIndex(error.code())
            }
        }
    }
}

impl CharacterRegistrationDiagnostic {
    pub(crate) fn new(
        kind: CharacterRegistrationDiagnosticKind,
        primary: SourceSpan,
        secondary: impl IntoIterator<Item = SourceSpan>,
    ) -> Self {
        let mut secondary = secondary.into_iter().collect::<Vec<_>>();
        sort_spans(&mut secondary);
        secondary.dedup();
        Self {
            kind,
            primary,
            secondary,
        }
    }

    pub const fn kind(&self) -> &CharacterRegistrationDiagnosticKind {
        &self.kind
    }

    pub const fn code(&self) -> CharacterRegistrationCode {
        self.kind.code()
    }

    pub const fn primary(&self) -> &SourceSpan {
        &self.primary
    }

    pub fn secondary(&self) -> &[SourceSpan] {
        &self.secondary
    }

    pub fn diagnostic(&self) -> Diagnostic {
        self.secondary.iter().fold(
            Diagnostic::new(DiagnosticSeverity::Error, format!("{:?}", self.kind))
                .with_code(self.code().as_str())
                .with_label(DiagnosticLabel::primary(self.primary.clone(), None)),
            |diagnostic, span| {
                diagnostic.with_label(DiagnosticLabel::secondary(span.clone(), None))
            },
        )
    }
}

impl CharacterRegistrationReport {
    pub(crate) fn from_diagnostics(mut diagnostics: Vec<CharacterRegistrationDiagnostic>) -> Self {
        diagnostics.sort_by(|left, right| {
            span_key(left.primary())
                .cmp(&span_key(right.primary()))
                .then_with(|| left.code().cmp(&right.code()))
                .then_with(|| left.kind().cmp(right.kind()))
        });
        diagnostics.dedup();
        let maximum = usize::try_from(CharacterRegistrationLimits::PRODUCTION.diagnostics())
            .expect("diagnostic limit fits usize");
        let omitted_diagnostics =
            u64::try_from(diagnostics.len().saturating_sub(maximum)).unwrap_or(u64::MAX);
        diagnostics.truncate(maximum);
        Self {
            diagnostics,
            omitted_diagnostics,
        }
    }

    pub(crate) fn with_omitted(mut self, additional: u64) -> Self {
        self.omitted_diagnostics = self.omitted_diagnostics.saturating_add(additional);
        self
    }

    pub fn diagnostics(&self) -> &[CharacterRegistrationDiagnostic] {
        &self.diagnostics
    }

    pub const fn omitted_diagnostics(&self) -> u64 {
        self.omitted_diagnostics
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty() && self.omitted_diagnostics == 0
    }
}

fn sort_spans(spans: &mut [SourceSpan]) {
    spans.sort_by_key(span_key);
}

fn span_key(span: &SourceSpan) -> (SourceDocumentId, SourceRevision, SourceRange) {
    (
        span.source().id().clone(),
        span.source().revision(),
        span.range(),
    )
}
