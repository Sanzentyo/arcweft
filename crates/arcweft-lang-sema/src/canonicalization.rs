//! Narrow, source-revision-bound semantic evidence for source canonicalization.

use std::collections::BTreeMap;
use std::fmt;

use arcweft_lang_hir::symbol::{CallableDeclarationId, CallablePackageId};
use arcweft_lang_syntax::ast::{
    common::TextRange, dialogue::SpeakerLineSurface, module_path::CanonicalModulePath,
};
use thiserror::Error;

use crate::types::{EntityKind, TypeKind};

/// Adapter-provided source document identity, usually a canonical URI or path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticDocumentId(String);

impl SemanticDocumentId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SemanticDocumentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// BLAKE3 of the exact UTF-8 source bytes checked by sema.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticSourceRevision([u8; 32]);

impl SemanticSourceRevision {
    #[must_use]
    pub fn from_source(source: &str) -> Self {
        Self(*blake3::hash(source.as_bytes()).as_bytes())
    }

    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for SemanticSourceRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for SemanticSourceRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .iter()
            .try_for_each(|byte| write!(formatter, "{byte:02x}"))
    }
}

/// Exact project/document/module snapshot represented by one inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticSourceIdentity {
    project: CallablePackageId,
    document: SemanticDocumentId,
    module: CanonicalModulePath,
    revision: SemanticSourceRevision,
    source_len: usize,
}

impl SemanticSourceIdentity {
    #[must_use]
    pub fn from_source(
        project: CallablePackageId,
        document: SemanticDocumentId,
        module: CanonicalModulePath,
        source: &str,
    ) -> Self {
        Self {
            project,
            document,
            module,
            revision: SemanticSourceRevision::from_source(source),
            source_len: source.len(),
        }
    }

    #[must_use]
    pub fn from_revision(
        project: CallablePackageId,
        document: SemanticDocumentId,
        module: CanonicalModulePath,
        revision: SemanticSourceRevision,
        source_len: usize,
    ) -> Self {
        Self {
            project,
            document,
            module,
            revision,
            source_len,
        }
    }

    #[must_use]
    pub const fn project(&self) -> &CallablePackageId {
        &self.project
    }

    #[must_use]
    pub const fn document(&self) -> &SemanticDocumentId {
        &self.document
    }

    #[must_use]
    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    #[must_use]
    pub const fn revision(&self) -> SemanticSourceRevision {
        self.revision
    }

    #[must_use]
    pub const fn source_len(&self) -> usize {
        self.source_len
    }
}

/// Invalid exact-source inventory supplied before checking begins.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CanonicalizationSourceSetError {
    #[error("canonicalization source set mixes project `{expected}` with `{actual}`")]
    MixedProject {
        expected: CallablePackageId,
        actual: CallablePackageId,
    },
    #[error("canonicalization source set contains duplicate module `{module}")]
    DuplicateModule { module: CanonicalModulePath },
}

/// Exact source identities supplied to one project check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalizationSourceSet {
    project: CallablePackageId,
    sources: BTreeMap<CanonicalModulePath, SemanticSourceIdentity>,
}

impl CanonicalizationSourceSet {
    pub fn try_new(
        project: CallablePackageId,
        sources: impl IntoIterator<Item = SemanticSourceIdentity>,
    ) -> Result<Self, CanonicalizationSourceSetError> {
        let mut source_map = BTreeMap::new();
        for source in sources {
            if source.project() != &project {
                return Err(CanonicalizationSourceSetError::MixedProject {
                    expected: project,
                    actual: source.project().clone(),
                });
            }
            let module = source.module().clone();
            if source_map.insert(module.clone(), source).is_some() {
                return Err(CanonicalizationSourceSetError::DuplicateModule { module });
            }
        }
        Ok(Self {
            project,
            sources: source_map,
        })
    }

    #[must_use]
    pub const fn project(&self) -> &CallablePackageId {
        &self.project
    }

    #[must_use]
    pub fn source(&self, module: &CanonicalModulePath) -> Option<&SemanticSourceIdentity> {
        self.sources.get(module)
    }

    pub fn sources(&self) -> impl ExactSizeIterator<Item = &SemanticSourceIdentity> {
        self.sources.values()
    }

    #[must_use]
    pub fn first_document(&self) -> Option<&SemanticDocumentId> {
        self.sources
            .values()
            .next()
            .map(SemanticSourceIdentity::document)
    }
}

/// Unique lexical scope inside one type-check report.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticScopeId(pub(crate) u32);

/// Unique lexical binding inside one type-check report.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticBindingId(pub(crate) u32);

/// Stable syntax identity inside one exact source revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeakerLineSyntaxId {
    module: CanonicalModulePath,
    head_range: TextRange,
}

impl SpeakerLineSyntaxId {
    pub(crate) fn new(module: CanonicalModulePath, head_range: TextRange) -> Self {
        Self { module, head_range }
    }

    #[must_use]
    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    #[must_use]
    pub const fn head_range(&self) -> TextRange {
        self.head_range
    }
}

/// Canonical semantic identity of the value used by a speaker line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticSymbolIdentity {
    Local {
        scope: SemanticScopeId,
        binding: SemanticBindingId,
        name: String,
    },
    Callable {
        declaration: CallableDeclarationId,
    },
    ModuleValue {
        module: CanonicalModulePath,
        name: String,
    },
    EnvironmentValue {
        name: String,
    },
    EntityLiteral {
        kind: EntityKind,
        canonical_name: String,
    },
}

/// Classification captured while the normal checker owns the resolved type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpeakerLineOutcome {
    Preset { entity_kind: EntityKind },
    Speaker { entity_kind: EntityKind },
    NonSpeaker,
    Unresolved,
    Erroneous,
}

/// One authored speaker line and the exact semantic proof available for it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedSpeakerLine {
    id: SpeakerLineSyntaxId,
    surface: SpeakerLineSurface,
    scope: SemanticScopeId,
    reference: String,
    symbol: Option<SemanticSymbolIdentity>,
    resolved_type: Option<TypeKind>,
    outcome: SpeakerLineOutcome,
}

impl CheckedSpeakerLine {
    pub(crate) fn new(
        id: SpeakerLineSyntaxId,
        surface: SpeakerLineSurface,
        scope: SemanticScopeId,
        reference: String,
        symbol: Option<SemanticSymbolIdentity>,
        resolved_type: Option<TypeKind>,
        outcome: SpeakerLineOutcome,
    ) -> Self {
        Self {
            id,
            surface,
            scope,
            reference,
            symbol,
            resolved_type,
            outcome,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &SpeakerLineSyntaxId {
        &self.id
    }

    #[must_use]
    pub const fn surface(&self) -> &SpeakerLineSurface {
        &self.surface
    }

    #[must_use]
    pub const fn scope(&self) -> SemanticScopeId {
        self.scope
    }

    #[must_use]
    pub fn reference(&self) -> &str {
        &self.reference
    }

    #[must_use]
    pub const fn symbol(&self) -> Option<&SemanticSymbolIdentity> {
        self.symbol.as_ref()
    }

    #[must_use]
    pub const fn resolved_type(&self) -> Option<&TypeKind> {
        self.resolved_type.as_ref()
    }

    #[must_use]
    pub const fn outcome(&self) -> &SpeakerLineOutcome {
        &self.outcome
    }
}

/// Narrow sema-owned input consumed by tooling for one document/module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCanonicalizationInventory {
    source: SemanticSourceIdentity,
    speaker_lines: Vec<CheckedSpeakerLine>,
}

impl CheckedCanonicalizationInventory {
    pub(crate) fn new(
        source: SemanticSourceIdentity,
        mut speaker_lines: Vec<CheckedSpeakerLine>,
    ) -> Self {
        speaker_lines.sort_by_key(|line| {
            let range = line.id().head_range();
            (range.start(), range.end(), line.reference().to_owned())
        });
        Self {
            source,
            speaker_lines,
        }
    }

    #[must_use]
    pub const fn source(&self) -> &SemanticSourceIdentity {
        &self.source
    }

    #[must_use]
    pub fn speaker_lines(&self) -> &[CheckedSpeakerLine] {
        &self.speaker_lines
    }
}

/// Explicit semantic-unavailability result. It is not a parse error.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("semantic data is unavailable for `{document}`: {reason}")]
pub struct SemanticDataUnavailable {
    document: SemanticDocumentId,
    reason: String,
}

impl SemanticDataUnavailable {
    #[must_use]
    pub fn new(document: SemanticDocumentId, reason: impl Into<String>) -> Self {
        Self {
            document,
            reason: reason.into(),
        }
    }

    #[must_use]
    pub const fn document(&self) -> &SemanticDocumentId {
        &self.document
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}
