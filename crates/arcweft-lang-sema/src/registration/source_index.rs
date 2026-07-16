//! Immutable descriptor-to-declaration provenance for a registered character world.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use super::model::{ProjectRegistrationFacts, RegisteredTypeCheckEnv};
use arcweft_character::{
    id::{CharacterLookId, CharacterPartId, CharacterVariantId},
    manifest::registration::{
        CharacterManifestDeclarationError, CharacterManifestTokenPath,
        SourceBackedCharacterManifest,
    },
    symbol::CharacterSymbolDescriptor,
};
use arcweft_lang_hir::symbol::{ProjectSymbolRevision, ProjectSymbolTable, ProjectSymbolWorldId};
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceDocumentIdentity, SourceRange, SourceSetRevision,
    SourceSpan,
};

/// Inclusive production bounds for character definition indexing and queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CharacterDefinitionLimits {
    indexed_manifests: u64,
    descriptors: u64,
    documents: u64,
    declaration_sources_per_descriptor: u64,
    aliases_consulted: u64,
    candidates: u64,
    diagnostics: u64,
    source_bytes: u64,
    build_work: u64,
    query_work: u64,
}

impl CharacterDefinitionLimits {
    pub const PRODUCTION: Self = Self {
        indexed_manifests: 1_024,
        descriptors: 262_144,
        documents: 4_096,
        declaration_sources_per_descriptor: 64,
        aliases_consulted: 256,
        candidates: 256,
        diagnostics: 128,
        source_bytes: 8_388_608,
        build_work: 1_048_576,
        query_work: 4_096,
    };

    pub const fn indexed_manifests(self) -> u64 {
        self.indexed_manifests
    }

    pub const fn descriptors(self) -> u64 {
        self.descriptors
    }

    pub const fn documents(self) -> u64 {
        self.documents
    }

    pub const fn declaration_sources_per_descriptor(self) -> u64 {
        self.declaration_sources_per_descriptor
    }

    pub const fn aliases_consulted(self) -> u64 {
        self.aliases_consulted
    }

    pub const fn candidates(self) -> u64 {
        self.candidates
    }

    pub const fn diagnostics(self) -> u64 {
        self.diagnostics
    }

    pub const fn source_bytes(self) -> u64 {
        self.source_bytes
    }

    pub const fn build_work(self) -> u64 {
        self.build_work
    }

    pub const fn query_work(self) -> u64 {
        self.query_work
    }
}

/// Resource counter reported by definition indexing and queries.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterDefinitionLimitKind {
    IndexedManifests,
    Descriptors,
    Documents,
    DeclarationSourcesPerDescriptor,
    AliasesConsulted,
    Candidates,
    Diagnostics,
    SourceBytes,
    BuildWork,
    QueryWork,
}

/// Stable typed code for an index construction failure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterDefinitionIndexCode {
    StaleWorld,
    StaleSymbolRevision,
    MissingDocument,
    ConflictingDocument,
    Projection,
    MissingToken,
    NonStringToken,
    SpanMismatch,
    InvalidSpan,
    DuplicateSourceFact,
    InconsistentSourceFact,
    DescriptorSetMismatch,
    Limit,
    ArithmeticOverflow,
}

impl CharacterDefinitionIndexCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaleWorld => "aw.character.definition.index.stale_world",
            Self::StaleSymbolRevision => "aw.character.definition.index.stale_symbol_revision",
            Self::MissingDocument => "aw.character.definition.index.missing_document",
            Self::ConflictingDocument => "aw.character.definition.index.conflicting_document",
            Self::Projection => "aw.character.definition.index.projection",
            Self::MissingToken => "aw.character.definition.index.missing_token",
            Self::NonStringToken => "aw.character.definition.index.non_string_token",
            Self::SpanMismatch => "aw.character.definition.index.span_mismatch",
            Self::InvalidSpan => "aw.character.definition.index.invalid_span",
            Self::DuplicateSourceFact => "aw.character.definition.index.duplicate_source_fact",
            Self::InconsistentSourceFact => {
                "aw.character.definition.index.inconsistent_source_fact"
            }
            Self::DescriptorSetMismatch => "aw.character.definition.index.descriptor_set_mismatch",
            Self::Limit => "aw.character.definition.limit",
            Self::ArithmeticOverflow => "aw.character.definition.arithmetic_overflow",
        }
    }
}

/// Reason one declaration span cannot be admitted to the immutable index.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterDefinitionSpanError {
    Reversed,
    OutOfBounds,
    NotUtf8Boundary,
    DifferentDocuments,
    SelectionOutsideValue,
    SelectionIncludesQuote,
}

/// Exact declaration provenance for one accepted manifest occurrence.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterDeclarationSource {
    token_path: CharacterManifestTokenPath,
    value_span: SourceSpan,
    selection_span: SourceSpan,
}

impl CharacterDeclarationSource {
    pub const fn token_path(&self) -> &CharacterManifestTokenPath {
        &self.token_path
    }

    pub const fn value_span(&self) -> &SourceSpan {
        &self.value_span
    }

    pub const fn selection_span(&self) -> &SourceSpan {
        &self.selection_span
    }
}

/// Ordered, non-empty declaration sources for one nominal descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDeclarationSet {
    sources: Vec<CharacterDeclarationSource>,
}

impl CharacterDeclarationSet {
    pub fn sources(&self) -> impl ExactSizeIterator<Item = &CharacterDeclarationSource> {
        self.sources.iter()
    }
}

#[derive(Clone, Debug, Default)]
struct CharacterMemberSpellingIndex {
    looks: BTreeMap<CharacterLookId, Vec<CharacterSymbolDescriptor>>,
    parts: BTreeMap<CharacterPartId, Vec<CharacterSymbolDescriptor>>,
    variants: BTreeMap<CharacterVariantId, Vec<CharacterSymbolDescriptor>>,
}

/// Immutable source index published as part of one registered semantic world.
#[derive(Clone, Debug)]
pub struct CharacterDefinitionIndex {
    world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    source_revision: SourceSetRevision,
    manifest_count: u64,
    documents: BTreeMap<SourceDocumentId, Arc<SourceDocument>>,
    declarations: BTreeMap<CharacterSymbolDescriptor, CharacterDeclarationSet>,
    members: CharacterMemberSpellingIndex,
}

impl CharacterDefinitionIndex {
    pub const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }

    pub const fn symbol_revision(&self) -> &ProjectSymbolRevision {
        &self.symbol_revision
    }

    pub const fn source_revision(&self) -> SourceSetRevision {
        self.source_revision
    }

    pub const fn manifest_count(&self) -> u64 {
        self.manifest_count
    }

    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    pub fn declaration(
        &self,
        descriptor: &CharacterSymbolDescriptor,
    ) -> Option<&CharacterDeclarationSet> {
        self.declarations.get(descriptor)
    }

    pub fn document(&self, identity: &SourceDocumentIdentity) -> Option<&Arc<SourceDocument>> {
        self.documents
            .get(identity.id())
            .filter(|document| document.identity() == identity)
    }

    pub fn documents(&self) -> impl ExactSizeIterator<Item = &Arc<SourceDocument>> {
        self.documents.values()
    }

    pub(crate) fn look_candidates(&self, id: &CharacterLookId) -> &[CharacterSymbolDescriptor] {
        self.members.looks.get(id).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn part_candidates(&self, id: &CharacterPartId) -> &[CharacterSymbolDescriptor] {
        self.members.parts.get(id).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn variant_candidates(
        &self,
        id: &CharacterVariantId,
    ) -> &[CharacterSymbolDescriptor] {
        self.members.variants.get(id).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn try_build(
        facts: &ProjectRegistrationFacts,
        symbols: &ProjectSymbolTable,
        environment: &RegisteredTypeCheckEnv,
    ) -> Result<Self, CharacterDefinitionIndexBuildReport> {
        Self::try_build_with_limits(
            facts,
            symbols,
            environment,
            CharacterDefinitionLimits::PRODUCTION,
        )
    }

    fn try_build_with_limits(
        facts: &ProjectRegistrationFacts,
        symbols: &ProjectSymbolTable,
        environment: &RegisteredTypeCheckEnv,
        limits: CharacterDefinitionLimits,
    ) -> Result<Self, CharacterDefinitionIndexBuildReport> {
        IndexBuilder::new(facts, symbols, environment, limits).build()
    }
}

/// Fail-closed index construction error.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CharacterDefinitionIndexBuildError {
    StaleWorld {
        expected: ProjectSymbolWorldId,
        actual: ProjectSymbolWorldId,
    },
    StaleSymbolRevision {
        expected: ProjectSymbolRevision,
        actual: ProjectSymbolRevision,
    },
    MissingDocument {
        identity: SourceDocumentIdentity,
    },
    ConflictingDocument {
        id: SourceDocumentId,
        first: SourceDocumentIdentity,
        conflicting: SourceDocumentIdentity,
    },
    Projection {
        descriptor: CharacterSymbolDescriptor,
        source: SourceDocumentIdentity,
        error: CharacterManifestDeclarationError,
    },
    MissingTokenProvenance {
        descriptor: CharacterSymbolDescriptor,
        path: CharacterManifestTokenPath,
        source: SourceDocumentIdentity,
    },
    NonStringDeclaration {
        descriptor: CharacterSymbolDescriptor,
        path: CharacterManifestTokenPath,
        source: SourceDocumentIdentity,
    },
    SpanSourceMismatch {
        descriptor: CharacterSymbolDescriptor,
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    InvalidSpan {
        descriptor: CharacterSymbolDescriptor,
        span: SourceSpan,
        reason: CharacterDefinitionSpanError,
    },
    DuplicateSourceFact {
        descriptor: CharacterSymbolDescriptor,
        source: CharacterDeclarationSource,
    },
    InconsistentSourceFact {
        descriptor: CharacterSymbolDescriptor,
        first: CharacterDeclarationSource,
        conflicting: CharacterDeclarationSource,
    },
    DescriptorSetMismatch {
        missing: Vec<CharacterSymbolDescriptor>,
        unexpected: Vec<CharacterSymbolDescriptor>,
    },
    Limit {
        kind: CharacterDefinitionLimitKind,
        observed: u64,
        maximum: u64,
    },
    ArithmeticOverflow {
        counter: CharacterDefinitionLimitKind,
    },
}

impl fmt::Display for CharacterDefinitionIndexBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code().as_str())
    }
}

impl std::error::Error for CharacterDefinitionIndexBuildError {}

impl CharacterDefinitionIndexBuildError {
    pub const fn code(&self) -> CharacterDefinitionIndexCode {
        match self {
            Self::StaleWorld { .. } => CharacterDefinitionIndexCode::StaleWorld,
            Self::StaleSymbolRevision { .. } => CharacterDefinitionIndexCode::StaleSymbolRevision,
            Self::MissingDocument { .. } => CharacterDefinitionIndexCode::MissingDocument,
            Self::ConflictingDocument { .. } => CharacterDefinitionIndexCode::ConflictingDocument,
            Self::Projection { error, .. } => match error {
                CharacterManifestDeclarationError::MissingToken { .. } => {
                    CharacterDefinitionIndexCode::MissingToken
                }
                CharacterManifestDeclarationError::NonStringToken { .. } => {
                    CharacterDefinitionIndexCode::NonStringToken
                }
                _ => CharacterDefinitionIndexCode::Projection,
            },
            Self::MissingTokenProvenance { .. } => CharacterDefinitionIndexCode::MissingToken,
            Self::NonStringDeclaration { .. } => CharacterDefinitionIndexCode::NonStringToken,
            Self::SpanSourceMismatch { .. } => CharacterDefinitionIndexCode::SpanMismatch,
            Self::InvalidSpan { .. } => CharacterDefinitionIndexCode::InvalidSpan,
            Self::DuplicateSourceFact { .. } => CharacterDefinitionIndexCode::DuplicateSourceFact,
            Self::InconsistentSourceFact { .. } => {
                CharacterDefinitionIndexCode::InconsistentSourceFact
            }
            Self::DescriptorSetMismatch { .. } => {
                CharacterDefinitionIndexCode::DescriptorSetMismatch
            }
            Self::Limit { .. } => CharacterDefinitionIndexCode::Limit,
            Self::ArithmeticOverflow { .. } => CharacterDefinitionIndexCode::ArithmeticOverflow,
        }
    }

    pub(crate) fn primary_span(&self) -> Option<&SourceSpan> {
        match self {
            Self::InvalidSpan { span, .. } => Some(span),
            Self::DuplicateSourceFact { source, .. } => Some(source.selection_span()),
            Self::InconsistentSourceFact { conflicting, .. } => Some(conflicting.selection_span()),
            _ => None,
        }
    }
}

/// Deterministically ordered bounded index construction failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDefinitionIndexBuildReport {
    errors: Vec<CharacterDefinitionIndexBuildError>,
    omitted_errors: u64,
}

impl CharacterDefinitionIndexBuildReport {
    fn new(mut errors: Vec<CharacterDefinitionIndexBuildError>, maximum: u64) -> Self {
        errors.sort();
        errors.dedup();
        let maximum = usize::try_from(maximum).unwrap_or(usize::MAX);
        let omitted_errors =
            u64::try_from(errors.len().saturating_sub(maximum)).unwrap_or(u64::MAX);
        errors.truncate(maximum);
        Self {
            errors,
            omitted_errors,
        }
    }

    pub fn errors(&self) -> &[CharacterDefinitionIndexBuildError] {
        &self.errors
    }

    pub const fn omitted_errors(&self) -> u64 {
        self.omitted_errors
    }
}

struct IndexBuilder<'a> {
    facts: &'a ProjectRegistrationFacts,
    symbols: &'a ProjectSymbolTable,
    environment: &'a RegisteredTypeCheckEnv,
    limits: CharacterDefinitionLimits,
    manifest_count: u64,
    source_bytes: u64,
    work: u64,
    build_exhausted: bool,
    documents: BTreeMap<SourceDocumentId, Arc<SourceDocument>>,
    declarations: BTreeMap<CharacterSymbolDescriptor, Vec<CharacterDeclarationSource>>,
    errors: Vec<CharacterDefinitionIndexBuildError>,
}

impl<'a> IndexBuilder<'a> {
    fn new(
        facts: &'a ProjectRegistrationFacts,
        symbols: &'a ProjectSymbolTable,
        environment: &'a RegisteredTypeCheckEnv,
        limits: CharacterDefinitionLimits,
    ) -> Self {
        Self {
            facts,
            symbols,
            environment,
            limits,
            manifest_count: 0,
            source_bytes: 0,
            work: 0,
            build_exhausted: false,
            documents: BTreeMap::new(),
            declarations: BTreeMap::new(),
            errors: Vec::new(),
        }
    }

    fn build(mut self) -> Result<CharacterDefinitionIndex, CharacterDefinitionIndexBuildReport> {
        self.audit_world();
        'catalogs: for catalog in self.facts.catalogs() {
            for manifest in catalog.manifests() {
                if self.build_exhausted {
                    break 'catalogs;
                }
                if self.charge_counter(
                    CharacterDefinitionLimitKind::IndexedManifests,
                    self.manifest_count,
                    1,
                    self.limits.indexed_manifests,
                ) {
                    self.manifest_count += 1;
                    self.admit_manifest(manifest);
                }
            }
        }

        if !self.build_exhausted {
            let expected = descriptors_from_environment(self.environment);
            let actual = self.declarations.keys().cloned().collect::<BTreeSet<_>>();
            for _ in &actual {
                if !self.charge_work(1) {
                    break;
                }
            }
            if !self.build_exhausted && expected != actual {
                self.record_error(CharacterDefinitionIndexBuildError::DescriptorSetMismatch {
                    missing: expected.difference(&actual).cloned().collect(),
                    unexpected: actual.difference(&expected).cloned().collect(),
                });
            }
        }

        if !self.errors.is_empty() {
            return Err(CharacterDefinitionIndexBuildReport::new(
                self.errors,
                self.limits.diagnostics,
            ));
        }

        let members = member_index(self.declarations.keys());
        for descriptor in members
            .looks
            .values()
            .chain(members.parts.values())
            .chain(members.variants.values())
            .flatten()
        {
            if !self.charge_work(1) {
                break;
            }
            if !self.declarations.contains_key(descriptor) {
                let error = CharacterDefinitionIndexBuildError::DescriptorSetMismatch {
                    missing: Vec::new(),
                    unexpected: vec![descriptor.clone()],
                };
                self.record_error(error);
            }
        }
        if !self.errors.is_empty() {
            return Err(CharacterDefinitionIndexBuildReport::new(
                self.errors,
                self.limits.diagnostics,
            ));
        }

        let source_revision = SourceSetRevision::try_for_identities(
            self.documents.values().map(|document| document.identity()),
        )
        .expect("admitted source documents have unique exact identities");
        let declarations = self
            .declarations
            .into_iter()
            .map(|(descriptor, mut sources)| {
                sources.sort_by(declaration_source_order);
                (descriptor, CharacterDeclarationSet { sources })
            })
            .collect::<BTreeMap<_, _>>();

        Ok(CharacterDefinitionIndex {
            world: self.facts.world().clone(),
            symbol_revision: *self.facts.symbol_revision(),
            source_revision,
            manifest_count: self.manifest_count,
            documents: self.documents,
            declarations,
            members,
        })
    }

    fn audit_world(&mut self) {
        for actual in [self.symbols.world(), self.environment.world()] {
            if actual != self.facts.world() {
                self.record_error(CharacterDefinitionIndexBuildError::StaleWorld {
                    expected: self.facts.world().clone(),
                    actual: actual.clone(),
                });
            }
        }
        for actual in [self.symbols.revision(), self.environment.symbol_revision()] {
            if actual != self.facts.symbol_revision() {
                self.record_error(CharacterDefinitionIndexBuildError::StaleSymbolRevision {
                    expected: *self.facts.symbol_revision(),
                    actual: *actual,
                });
            }
        }
    }

    fn admit_manifest(&mut self, manifest: &SourceBackedCharacterManifest) {
        if !self.charge_work(1) {
            return;
        }
        let identity = manifest.source_map().document();
        let Some(document) = self.facts.document_arc(identity.id()).cloned() else {
            self.record_error(CharacterDefinitionIndexBuildError::MissingDocument {
                identity: identity.clone(),
            });
            return;
        };
        if document.identity() != identity {
            self.record_error(CharacterDefinitionIndexBuildError::ConflictingDocument {
                id: identity.id().clone(),
                first: document.identity().clone(),
                conflicting: identity.clone(),
            });
            return;
        }
        if !self.documents.contains_key(identity.id()) {
            let observed = u64::try_from(self.documents.len())
                .ok()
                .and_then(|count| count.checked_add(1));
            let Some(observed) = observed else {
                self.record_error(CharacterDefinitionIndexBuildError::ArithmeticOverflow {
                    counter: CharacterDefinitionLimitKind::Documents,
                });
                return;
            };
            if observed > self.limits.documents {
                self.record_error(CharacterDefinitionIndexBuildError::Limit {
                    kind: CharacterDefinitionLimitKind::Documents,
                    observed,
                    maximum: self.limits.documents,
                });
                return;
            }
            let Some(source_bytes) = self.source_bytes.checked_add(identity.source_len()) else {
                self.record_error(CharacterDefinitionIndexBuildError::ArithmeticOverflow {
                    counter: CharacterDefinitionLimitKind::SourceBytes,
                });
                return;
            };
            if source_bytes > self.limits.source_bytes {
                self.record_error(CharacterDefinitionIndexBuildError::Limit {
                    kind: CharacterDefinitionLimitKind::SourceBytes,
                    observed: source_bytes,
                    maximum: self.limits.source_bytes,
                });
                return;
            }
            self.source_bytes = source_bytes;
            self.documents.insert(identity.id().clone(), document);
        }

        for descriptor in descriptors_from_manifest(manifest.manifest()) {
            if self.build_exhausted {
                break;
            }
            self.admit_descriptor(manifest, descriptor);
        }
    }

    fn admit_descriptor(
        &mut self,
        manifest: &SourceBackedCharacterManifest,
        descriptor: CharacterSymbolDescriptor,
    ) {
        if !self.charge_work(1) {
            return;
        }
        let is_new = !self.declarations.contains_key(&descriptor);
        if is_new {
            let observed = u64::try_from(self.declarations.len())
                .ok()
                .and_then(|count| count.checked_add(1));
            let Some(observed) = observed else {
                self.record_error(CharacterDefinitionIndexBuildError::ArithmeticOverflow {
                    counter: CharacterDefinitionLimitKind::Descriptors,
                });
                return;
            };
            if observed > self.limits.descriptors {
                self.record_error(CharacterDefinitionIndexBuildError::Limit {
                    kind: CharacterDefinitionLimitKind::Descriptors,
                    observed,
                    maximum: self.limits.descriptors,
                });
                return;
            }
        }

        let (token_path, token) = match manifest.declaration_token(&descriptor) {
            Ok(projected) => projected,
            Err(error) => {
                self.record_error(CharacterDefinitionIndexBuildError::Projection {
                    descriptor,
                    source: manifest.source_map().document().clone(),
                    error,
                });
                return;
            }
        };
        let source = CharacterDeclarationSource {
            token_path,
            value_span: token.value().clone(),
            selection_span: token
                .string_content()
                .expect("declaration_token requires a JSON string")
                .clone(),
        };
        if let Err(error) = validate_declaration_source(
            &descriptor,
            manifest.source_map().document(),
            self.documents
                .get(manifest.source_map().document().id())
                .expect("manifest document was admitted"),
            &source,
        ) {
            self.record_error(*error);
            return;
        }

        let existing = self.declarations.get(&descriptor);
        if let Some(first) = existing.into_iter().flatten().find(|first| {
            first.token_path == source.token_path
                && first.value_span.source().id() == source.value_span.source().id()
        }) {
            let error = if first == &source {
                CharacterDefinitionIndexBuildError::DuplicateSourceFact { descriptor, source }
            } else {
                CharacterDefinitionIndexBuildError::InconsistentSourceFact {
                    descriptor,
                    first: first.clone(),
                    conflicting: source,
                }
            };
            self.record_error(error);
            return;
        }
        let existing_len = existing.map_or(0, Vec::len);
        let observed = u64::try_from(existing_len)
            .ok()
            .and_then(|count| count.checked_add(1));
        let Some(observed) = observed else {
            self.record_error(CharacterDefinitionIndexBuildError::ArithmeticOverflow {
                counter: CharacterDefinitionLimitKind::DeclarationSourcesPerDescriptor,
            });
            return;
        };
        if observed > self.limits.declaration_sources_per_descriptor {
            self.record_error(CharacterDefinitionIndexBuildError::Limit {
                kind: CharacterDefinitionLimitKind::DeclarationSourcesPerDescriptor,
                observed,
                maximum: self.limits.declaration_sources_per_descriptor,
            });
            return;
        }
        if existing_len != 0 && !self.charge_work(1) {
            return;
        }
        self.declarations
            .entry(descriptor)
            .or_default()
            .push(source);
    }

    fn record_error(&mut self, error: CharacterDefinitionIndexBuildError) {
        let _ = self.charge_work(1);
        self.errors.push(error);
    }

    fn charge_counter(
        &mut self,
        kind: CharacterDefinitionLimitKind,
        current: u64,
        amount: u64,
        maximum: u64,
    ) -> bool {
        let Some(observed) = current.checked_add(amount) else {
            self.record_error(CharacterDefinitionIndexBuildError::ArithmeticOverflow {
                counter: kind,
            });
            return false;
        };
        if observed > maximum {
            self.record_error(CharacterDefinitionIndexBuildError::Limit {
                kind,
                observed,
                maximum,
            });
            return false;
        }
        self.charge_work(1)
    }

    fn charge_work(&mut self, amount: u64) -> bool {
        if self.build_exhausted {
            return false;
        }
        let Some(observed) = self.work.checked_add(amount) else {
            self.errors
                .push(CharacterDefinitionIndexBuildError::ArithmeticOverflow {
                    counter: CharacterDefinitionLimitKind::BuildWork,
                });
            self.build_exhausted = true;
            return false;
        };
        if observed > self.limits.build_work {
            self.errors.push(CharacterDefinitionIndexBuildError::Limit {
                kind: CharacterDefinitionLimitKind::BuildWork,
                observed,
                maximum: self.limits.build_work,
            });
            self.build_exhausted = true;
            return false;
        }
        self.work = observed;
        true
    }
}

fn descriptors_from_manifest(
    manifest: &arcweft_character::manifest::CharacterManifest,
) -> Vec<CharacterSymbolDescriptor> {
    let character = manifest.character().clone();
    std::iter::once(CharacterSymbolDescriptor::Owner {
        character: character.clone(),
    })
    .chain(
        manifest
            .looks()
            .iter()
            .map(|look| CharacterSymbolDescriptor::Look {
                character: character.clone(),
                look: look.id().clone(),
            }),
    )
    .chain(
        manifest
            .parts()
            .iter()
            .map(|part| CharacterSymbolDescriptor::Part {
                character: character.clone(),
                part: part.id().clone(),
            }),
    )
    .chain(manifest.parts().iter().flat_map(|part| {
        let character = character.clone();
        part.variants()
            .iter()
            .map(move |variant| CharacterSymbolDescriptor::Variant {
                character: character.clone(),
                part: part.id().clone(),
                variant: variant.id().clone(),
            })
    }))
    .collect()
}

fn descriptors_from_environment(
    environment: &RegisteredTypeCheckEnv,
) -> BTreeSet<CharacterSymbolDescriptor> {
    environment
        .characters()
        .flat_map(|(_, manifest)| descriptors_from_manifest(manifest))
        .collect()
}

fn validate_declaration_source(
    descriptor: &CharacterSymbolDescriptor,
    expected: &SourceDocumentIdentity,
    document: &SourceDocument,
    source: &CharacterDeclarationSource,
) -> Result<(), Box<CharacterDefinitionIndexBuildError>> {
    for span in [source.value_span(), source.selection_span()] {
        if span.source() != expected {
            return Err(Box::new(
                CharacterDefinitionIndexBuildError::SpanSourceMismatch {
                    descriptor: descriptor.clone(),
                    expected: expected.clone(),
                    actual: span.source().clone(),
                },
            ));
        }
        let range = span.range();
        let reason = if range.start() > range.end() {
            Some(CharacterDefinitionSpanError::Reversed)
        } else if range.end() > document.text().len() {
            Some(CharacterDefinitionSpanError::OutOfBounds)
        } else if !document.text().is_char_boundary(range.start())
            || !document.text().is_char_boundary(range.end())
        {
            Some(CharacterDefinitionSpanError::NotUtf8Boundary)
        } else {
            None
        };
        if let Some(reason) = reason {
            return Err(Box::new(CharacterDefinitionIndexBuildError::InvalidSpan {
                descriptor: descriptor.clone(),
                span: span.clone(),
                reason,
            }));
        }
    }

    let value = source.value_span().range();
    let selection = source.selection_span().range();
    if selection.start() < value.start() || selection.end() > value.end() {
        return Err(Box::new(CharacterDefinitionIndexBuildError::InvalidSpan {
            descriptor: descriptor.clone(),
            span: source.selection_span().clone(),
            reason: CharacterDefinitionSpanError::SelectionOutsideValue,
        }));
    }
    if value.end().saturating_sub(value.start()) < 2
        || selection != SourceRange::new(value.start() + 1, value.end() - 1)
    {
        return Err(Box::new(CharacterDefinitionIndexBuildError::InvalidSpan {
            descriptor: descriptor.clone(),
            span: source.selection_span().clone(),
            reason: CharacterDefinitionSpanError::SelectionIncludesQuote,
        }));
    }
    Ok(())
}

fn declaration_source_order(
    left: &CharacterDeclarationSource,
    right: &CharacterDeclarationSource,
) -> Ordering {
    left.selection_span
        .source()
        .cmp(right.selection_span.source())
        .then_with(|| {
            left.selection_span
                .range()
                .cmp(&right.selection_span.range())
        })
        .then_with(|| left.value_span.range().cmp(&right.value_span.range()))
        .then_with(|| left.token_path.cmp(&right.token_path))
}

fn member_index<'a>(
    declarations: impl Iterator<Item = &'a CharacterSymbolDescriptor>,
) -> CharacterMemberSpellingIndex {
    let mut members = CharacterMemberSpellingIndex::default();
    for descriptor in declarations {
        match descriptor {
            CharacterSymbolDescriptor::Owner { .. } => {}
            CharacterSymbolDescriptor::Look { look, .. } => members
                .looks
                .entry(look.clone())
                .or_default()
                .push(descriptor.clone()),
            CharacterSymbolDescriptor::Part { part, .. } => members
                .parts
                .entry(part.clone())
                .or_default()
                .push(descriptor.clone()),
            CharacterSymbolDescriptor::Variant { variant, .. } => members
                .variants
                .entry(variant.clone())
                .or_default()
                .push(descriptor.clone()),
        }
    }
    for candidates in members
        .looks
        .values_mut()
        .chain(members.parts.values_mut())
        .chain(members.variants.values_mut())
    {
        candidates.sort();
        candidates.dedup();
    }
    members
}
