//! Immutable source, module, and HIR authority retained by one accepted profile generation.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use arcweft_lang_hir::{
    model::HirModule,
    project::HirProject,
    symbol::{
        CallableDeclarationId, ProjectSymbolRevision, ProjectSymbolTable, ProjectSymbolWorldId,
    },
};
use arcweft_lang_sema::{
    check::{TypeCheckReport, analyze_registered_project_types},
    entry::{CheckedEntryId, check_project_entries},
    project_index::{ProjectSemanticIndex, project_semantic_index_from_checked_project},
    registration::{CharacterRegistrationLimits, RegisteredSemanticWorld},
};
use arcweft_lang_syntax::ast::{
    common::UseTreeKind,
    module_path::CanonicalModulePath,
    symbol_path::{ProjectSymbolPath, SymbolPath},
};
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceDocumentIdentity, SourceRange, SourceSetRevision,
    SourceSetRevisionError, SourceSpan,
};
use lsp_types::Uri;
use thiserror::Error;

use crate::{
    positions::{LineIndex, PositionEncoding},
    uri_key::LspUriKey,
};

/// One canonical module proven to carry one exact accepted source revision.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AcceptedModuleKey {
    module: CanonicalModulePath,
    source: SourceDocumentIdentity,
}

impl AcceptedModuleKey {
    pub(crate) const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub(crate) const fn source(&self) -> &SourceDocumentIdentity {
        &self.source
    }
}

/// Exact retained size of one accepted project snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedProjectFootprint {
    documents: u64,
    modules: u64,
    source_bytes: u64,
}

impl AcceptedProjectFootprint {
    #[allow(dead_code, reason = "retained for bounded accepted-project metrics")]
    pub(crate) const fn documents(self) -> u64 {
        self.documents
    }

    #[allow(dead_code, reason = "retained for bounded accepted-project metrics")]
    pub(crate) const fn modules(self) -> u64 {
        self.modules
    }

    pub(crate) const fn source_bytes(self) -> u64 {
        self.source_bytes
    }
}

/// Bounded counter owned by accepted-project construction.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum AcceptedProjectLimitKind {
    Documents,
    Modules,
    SourceBytes,
}

impl AcceptedProjectLimitKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Documents => "documents",
            Self::Modules => "modules",
            Self::SourceBytes => "source bytes",
        }
    }
}

/// Immutable source adapter registry owned by one accepted project.
#[derive(Debug)]
pub(crate) struct AcceptedSourceDocuments {
    world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    all_source_revision: SourceSetRevision,
    by_identity: BTreeMap<SourceDocumentIdentity, AcceptedSourceDocument>,
    by_uri: BTreeMap<LspUriKey, SourceDocumentIdentity>,
}

/// One exact source document and its explicit navigation metadata.
#[derive(Debug)]
pub(crate) struct AcceptedSourceDocument {
    document: Arc<SourceDocument>,
    locator: AcceptedSourceLocator,
    #[allow(
        dead_code,
        reason = "exact topology ownership is retained for later LSP policy"
    )]
    ownership: AcceptedSourceOwnership,
    #[allow(
        dead_code,
        reason = "exact topology access is retained for later LSP policy"
    )]
    access: AcceptedSourceAccess,
    line_index: LineIndex,
}

/// Explicit location authority; no display/source-name inverse parsing is allowed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AcceptedSourceLocator {
    File {
        path: PathBuf,
        uri: Uri,
    },
    #[allow(
        dead_code,
        reason = "non-file accepted adapters use typed URI provenance"
    )]
    Uri {
        uri: Uri,
    },
    Unavailable,
}

/// Ownership category for one accepted source.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum AcceptedSourceOwnership {
    Workspace,
    Dependency,
    Generated,
}

/// Mutability category retained for edit policy, not query eligibility.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum AcceptedSourceAccess {
    Writable,
    ReadOnly,
    Unknown,
}

/// One explicit source record supplied to accepted-project construction.
#[derive(Clone, Debug)]
pub(crate) struct AcceptedSourceDocumentSeed {
    document: Arc<SourceDocument>,
    locator: AcceptedSourceLocator,
    ownership: AcceptedSourceOwnership,
    access: AcceptedSourceAccess,
}

#[derive(Default)]
struct AcceptedSourceRegistryBuilder {
    identities_by_id: BTreeMap<SourceDocumentId, SourceDocumentIdentity>,
    by_identity: BTreeMap<SourceDocumentIdentity, AcceptedSourceDocument>,
    by_uri: BTreeMap<LspUriKey, SourceDocumentIdentity>,
    source_bytes: u64,
}

/// One immutable HIR/source/module carrier published with an accepted world.
#[derive(Debug)]
pub(crate) struct AcceptedProjectSnapshot {
    hir: Arc<HirProject>,
    typecheck: Arc<TypeCheckReport>,
    semantic_index: Arc<ProjectSemanticIndex>,
    callable_references: Arc<[AcceptedCallableReference]>,
    entry_references: Arc<[AcceptedEntryReference]>,
    sources: AcceptedSourceDocuments,
    module_by_source: BTreeMap<SourceDocumentIdentity, CanonicalModulePath>,
    #[allow(dead_code, reason = "retained for bounded accepted-project metrics")]
    footprint: AcceptedProjectFootprint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedCallableReference {
    declaration: CallableDeclarationId,
    source: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedEntryReference {
    entry: CheckedEntryId,
    source: SourceSpan,
}

impl AcceptedCallableReference {
    pub(crate) const fn declaration(&self) -> &CallableDeclarationId {
        &self.declaration
    }

    pub(crate) const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

impl AcceptedEntryReference {
    pub(crate) const fn entry(&self) -> &CheckedEntryId {
        &self.entry
    }

    pub(crate) const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

#[derive(Debug)]
pub(crate) enum AcceptedProjectSnapshotError {
    DuplicateSourceIdentity {
        source: SourceDocumentIdentity,
    },
    ConflictingSourceId {
        id: SourceDocumentId,
        first: SourceDocumentIdentity,
        conflicting: SourceDocumentIdentity,
    },
    DuplicateUri {
        uri: LspUriKey,
        first: SourceDocumentIdentity,
        conflicting: SourceDocumentIdentity,
    },
    Limit {
        kind: AcceptedProjectLimitKind,
        observed: u64,
        maximum: u64,
    },
    ArithmeticOverflow {
        counter: AcceptedProjectLimitKind,
    },
    WorldMismatch {
        expected: ProjectSymbolWorldId,
        actual: ProjectSymbolWorldId,
    },
    SymbolRevisionMismatch {
        expected: ProjectSymbolRevision,
        actual: ProjectSymbolRevision,
    },
    CharacterSourceRevisionMismatch {
        expected: SourceSetRevision,
        actual: SourceSetRevision,
    },
    ModuleInventoryMismatch {
        hir_only: Box<[CanonicalModulePath]>,
        symbol_only: Box<[CanonicalModulePath]>,
    },
    MissingProjectSource {
        module: CanonicalModulePath,
    },
    MissingSymbolSource {
        module: CanonicalModulePath,
    },
    MissingModuleDocument {
        module: CanonicalModulePath,
        source: SourceDocumentIdentity,
    },
    MissingHirSource {
        module: CanonicalModulePath,
    },
    ModuleSourceMismatch {
        module: CanonicalModulePath,
        project: SourceDocumentIdentity,
        hir: SourceDocumentIdentity,
        symbols: SourceDocumentIdentity,
    },
    HirTextMismatch {
        module: CanonicalModulePath,
        source: SourceDocumentIdentity,
    },
    ConflictingModuleMapping {
        source: SourceDocumentIdentity,
        first: CanonicalModulePath,
        conflicting: CanonicalModulePath,
    },
    TypeCheck(String),
    EntryBinding(String),
    SemanticIndex(String),
    SourceSet(SourceSetRevisionError),
}

impl std::fmt::Display for AcceptedProjectSnapshotError {
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive formatter preserves stable messages for every accepted-project invariant"
    )]
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateSourceIdentity { source } => {
                write!(formatter, "duplicate accepted source identity: {source:?}")
            }
            Self::ConflictingSourceId {
                id,
                first,
                conflicting,
            } => write!(
                formatter,
                "conflicting accepted revisions for {id:?}: {first:?} versus {conflicting:?}"
            ),
            Self::DuplicateUri {
                uri,
                first,
                conflicting,
            } => write!(
                formatter,
                "duplicate accepted URI {uri}: {first:?} versus {conflicting:?}"
            ),
            Self::Limit {
                kind,
                observed,
                maximum,
            } => write!(
                formatter,
                "accepted project {} limit exceeded: observed {observed}, maximum {maximum}",
                kind.as_str()
            ),
            Self::ArithmeticOverflow { counter } => {
                write!(
                    formatter,
                    "accepted project {} counter overflowed",
                    counter.as_str()
                )
            }
            Self::WorldMismatch { expected, actual } => write!(
                formatter,
                "accepted source world mismatch: expected {expected:?}, actual {actual:?}"
            ),
            Self::SymbolRevisionMismatch { expected, actual } => write!(
                formatter,
                "accepted symbol revision mismatch: expected {expected:?}, actual {actual:?}"
            ),
            Self::CharacterSourceRevisionMismatch { expected, actual } => write!(
                formatter,
                "accepted character source revision mismatch: expected {expected:?}, actual {actual:?}"
            ),
            Self::ModuleInventoryMismatch {
                hir_only,
                symbol_only,
            } => write!(
                formatter,
                "HIR/symbol module inventory mismatch: HIR-only {hir_only:?}, symbol-only {symbol_only:?}"
            ),
            Self::MissingProjectSource { module } => {
                write!(
                    formatter,
                    "HIR project module has no source identity: {module:?}"
                )
            }
            Self::MissingSymbolSource { module } => {
                write!(
                    formatter,
                    "symbol module has no source identity: {module:?}"
                )
            }
            Self::MissingModuleDocument { module, source } => write!(
                formatter,
                "module source is absent from accepted documents: {module:?} -> {source:?}"
            ),
            Self::MissingHirSource { module } => {
                write!(formatter, "module HIR has no retained source: {module:?}")
            }
            Self::ModuleSourceMismatch {
                module,
                project,
                hir,
                symbols,
            } => write!(
                formatter,
                "module source mismatch for {module:?}: project {project:?}, HIR {hir:?}, symbols {symbols:?}"
            ),
            Self::HirTextMismatch { module, source } => write!(
                formatter,
                "module HIR text differs from accepted source: {module:?} -> {source:?}"
            ),
            Self::ConflictingModuleMapping {
                source,
                first,
                conflicting,
            } => write!(
                formatter,
                "accepted source maps to multiple modules: {source:?} -> {first:?}, {conflicting:?}"
            ),
            Self::TypeCheck(message) => {
                write!(
                    formatter,
                    "accepted project type checking failed: {message}"
                )
            }
            Self::EntryBinding(message) => {
                write!(formatter, "accepted entry binding failed: {message}")
            }
            Self::SemanticIndex(message) => {
                write!(formatter, "accepted semantic index failed: {message}")
            }
            Self::SourceSet(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for AcceptedProjectSnapshotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceSet(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SourceSetRevisionError> for AcceptedProjectSnapshotError {
    fn from(error: SourceSetRevisionError) -> Self {
        Self::SourceSet(error)
    }
}

/// Typed failure to retrieve HIR through a validated accepted-module key.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum AcceptedHirLookupError {
    #[error("accepted HIR module is absent")]
    MissingModule { key: AcceptedModuleKey },
    #[error("accepted HIR source identity differs from its key")]
    SourceIdentityMismatch {
        key: AcceptedModuleKey,
        actual: Option<SourceDocumentIdentity>,
    },
    #[error("accepted HIR source document is absent")]
    MissingSourceDocument { key: AcceptedModuleKey },
    #[error("accepted HIR source document differs from its key")]
    SourceDocumentMismatch {
        key: AcceptedModuleKey,
        actual: SourceDocumentIdentity,
    },
}

impl AcceptedSourceDocumentSeed {
    pub(crate) fn new(
        document: Arc<SourceDocument>,
        locator: AcceptedSourceLocator,
        ownership: AcceptedSourceOwnership,
        access: AcceptedSourceAccess,
    ) -> Self {
        Self {
            document,
            locator,
            ownership,
            access,
        }
    }

    pub(crate) const fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }
}

impl AcceptedSourceRegistryBuilder {
    #[allow(
        clippy::result_large_err,
        reason = "rejection retains both exact source identities for deterministic admission diagnostics"
    )]
    fn insert(
        &mut self,
        seed: AcceptedSourceDocumentSeed,
    ) -> Result<(), AcceptedProjectSnapshotError> {
        let identity = seed.document.identity().clone();
        if self.by_identity.contains_key(&identity) {
            return Err(AcceptedProjectSnapshotError::DuplicateSourceIdentity { source: identity });
        }
        if let Some(first) = self.identities_by_id.get(identity.id()) {
            return Err(AcceptedProjectSnapshotError::ConflictingSourceId {
                id: identity.id().clone(),
                first: first.clone(),
                conflicting: identity,
            });
        }

        let observed_documents = u64::try_from(self.by_identity.len())
            .map_err(|_| AcceptedProjectSnapshotError::ArithmeticOverflow {
                counter: AcceptedProjectLimitKind::Documents,
            })?
            .checked_add(1)
            .ok_or(AcceptedProjectSnapshotError::ArithmeticOverflow {
                counter: AcceptedProjectLimitKind::Documents,
            })?;
        let maximum_documents = CharacterRegistrationLimits::PRODUCTION.documents();
        if observed_documents > maximum_documents {
            return Err(AcceptedProjectSnapshotError::Limit {
                kind: AcceptedProjectLimitKind::Documents,
                observed: observed_documents,
                maximum: maximum_documents,
            });
        }

        let document_bytes = u64::try_from(seed.document.text().len()).map_err(|_| {
            AcceptedProjectSnapshotError::ArithmeticOverflow {
                counter: AcceptedProjectLimitKind::SourceBytes,
            }
        })?;
        let source_bytes = self.source_bytes.checked_add(document_bytes).ok_or(
            AcceptedProjectSnapshotError::ArithmeticOverflow {
                counter: AcceptedProjectLimitKind::SourceBytes,
            },
        )?;
        let maximum_source_bytes = CharacterRegistrationLimits::PRODUCTION.source_bytes();
        if source_bytes > maximum_source_bytes {
            return Err(AcceptedProjectSnapshotError::Limit {
                kind: AcceptedProjectLimitKind::SourceBytes,
                observed: source_bytes,
                maximum: maximum_source_bytes,
            });
        }

        if let Some(uri) = seed.locator.uri() {
            let uri = LspUriKey::from_uri(uri);
            if let Some(first) = self.by_uri.get(&uri) {
                return Err(AcceptedProjectSnapshotError::DuplicateUri {
                    uri,
                    first: first.clone(),
                    conflicting: identity,
                });
            }
            self.by_uri.insert(uri, identity.clone());
        }
        self.identities_by_id
            .insert(identity.id().clone(), identity.clone());
        self.by_identity.insert(
            identity,
            AcceptedSourceDocument {
                line_index: LineIndex::new(
                    seed.document.text().to_owned(),
                    PositionEncoding::default(),
                ),
                document: seed.document,
                locator: seed.locator,
                ownership: seed.ownership,
                access: seed.access,
            },
        );
        self.source_bytes = source_bytes;
        Ok(())
    }

    #[allow(
        clippy::result_large_err,
        reason = "rejection retains exact world and revision identities for deterministic admission diagnostics"
    )]
    fn validate_world(
        &self,
        world: &RegisteredSemanticWorld,
    ) -> Result<SourceSetRevision, AcceptedProjectSnapshotError> {
        let symbols = world.symbols();
        let environment = world.environment();
        let index = world.character_definition_index();
        if environment.world() != symbols.world() || index.world() != symbols.world() {
            let actual = match (
                environment.world() == symbols.world(),
                index.world() == symbols.world(),
            ) {
                (false, _) => environment.world().clone(),
                (true, false) => index.world().clone(),
                (true, true) => unreachable!("world mismatch was already established"),
            };
            return Err(AcceptedProjectSnapshotError::WorldMismatch {
                expected: symbols.world().clone(),
                actual,
            });
        }
        if environment.symbol_revision() != symbols.revision()
            || index.symbol_revision() != symbols.revision()
        {
            let actual = match (
                environment.symbol_revision() == symbols.revision(),
                index.symbol_revision() == symbols.revision(),
            ) {
                (false, _) => *environment.symbol_revision(),
                (true, false) => *index.symbol_revision(),
                (true, true) => unreachable!("symbol revision mismatch was already established"),
            };
            return Err(AcceptedProjectSnapshotError::SymbolRevisionMismatch {
                expected: *symbols.revision(),
                actual,
            });
        }

        let mut indexed_identities = Vec::with_capacity(index.documents().len());
        for indexed in index.documents() {
            let identity = indexed.identity();
            let Some(accepted) = self.by_identity.get(identity) else {
                return Err(AcceptedProjectSnapshotError::MissingModuleDocument {
                    module: CanonicalModulePath::crate_root(),
                    source: identity.clone(),
                });
            };
            if accepted.document.text() != indexed.text() {
                return Err(AcceptedProjectSnapshotError::HirTextMismatch {
                    module: CanonicalModulePath::crate_root(),
                    source: identity.clone(),
                });
            }
            indexed_identities.push(identity.clone());
        }
        let actual = SourceSetRevision::try_for_identities(indexed_identities.iter())?;
        if actual != index.source_revision() {
            return Err(
                AcceptedProjectSnapshotError::CharacterSourceRevisionMismatch {
                    expected: index.source_revision(),
                    actual,
                },
            );
        }
        Ok(actual)
    }

    fn finish(
        self,
        world: ProjectSymbolWorldId,
        symbol_revision: ProjectSymbolRevision,
        character_source_revision: SourceSetRevision,
    ) -> AcceptedSourceDocuments {
        AcceptedSourceDocuments {
            world,
            symbol_revision,
            all_source_revision: character_source_revision,
            by_identity: self.by_identity,
            by_uri: self.by_uri,
        }
    }
}

impl AcceptedProjectSnapshot {
    #[allow(
        clippy::result_large_err,
        clippy::too_many_lines,
        reason = "one admission boundary validates and preserves the exact HIR, symbol, source, and document identity tuple"
    )]
    pub(crate) fn try_new(
        hir: Arc<HirProject>,
        world: &RegisteredSemanticWorld,
        source_seeds: Vec<AcceptedSourceDocumentSeed>,
    ) -> Result<Self, AcceptedProjectSnapshotError> {
        let mut source_builder = AcceptedSourceRegistryBuilder::default();
        for seed in source_seeds {
            source_builder.insert(seed)?;
        }
        let character_source_revision = source_builder.validate_world(world)?;

        let symbols = world.symbols();
        let hir_modules = hir
            .modules()
            .map(|(module, _)| module.clone())
            .collect::<BTreeSet<_>>();
        let symbol_modules = symbols.modules().cloned().collect::<BTreeSet<_>>();
        if hir_modules != symbol_modules {
            return Err(AcceptedProjectSnapshotError::ModuleInventoryMismatch {
                hir_only: hir_modules
                    .difference(&symbol_modules)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                symbol_only: symbol_modules
                    .difference(&hir_modules)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            });
        }

        let module_count = u64::try_from(hir_modules.len()).map_err(|_| {
            AcceptedProjectSnapshotError::ArithmeticOverflow {
                counter: AcceptedProjectLimitKind::Modules,
            }
        })?;
        let document_count = u64::try_from(source_builder.by_identity.len()).map_err(|_| {
            AcceptedProjectSnapshotError::ArithmeticOverflow {
                counter: AcceptedProjectLimitKind::Documents,
            }
        })?;
        if module_count > document_count {
            return Err(AcceptedProjectSnapshotError::Limit {
                kind: AcceptedProjectLimitKind::Modules,
                observed: module_count,
                maximum: document_count,
            });
        }

        let mut module_by_source = BTreeMap::new();
        for module in hir_modules {
            let project_source = hir.source(&module).cloned().ok_or_else(|| {
                AcceptedProjectSnapshotError::MissingProjectSource {
                    module: module.clone(),
                }
            })?;
            let hir_module = hir
                .module(&module)
                .expect("module inventory was collected from this HIR project");
            let hir_source = hir_module.source_identity().cloned().ok_or_else(|| {
                AcceptedProjectSnapshotError::MissingHirSource {
                    module: module.clone(),
                }
            })?;
            let symbol_source = symbols.source_identity(&module).cloned().ok_or_else(|| {
                AcceptedProjectSnapshotError::MissingSymbolSource {
                    module: module.clone(),
                }
            })?;
            if project_source != hir_source || project_source != symbol_source {
                return Err(AcceptedProjectSnapshotError::ModuleSourceMismatch {
                    module,
                    project: project_source,
                    hir: hir_source,
                    symbols: symbol_source,
                });
            }
            let accepted = source_builder
                .by_identity
                .get(&project_source)
                .ok_or_else(|| AcceptedProjectSnapshotError::MissingModuleDocument {
                    module: module.clone(),
                    source: project_source.clone(),
                })?;
            let bound_document = hir_module.source_document().ok_or_else(|| {
                AcceptedProjectSnapshotError::MissingHirSource {
                    module: module.clone(),
                }
            })?;
            if bound_document.identity() != &project_source
                || bound_document.text() != accepted.document.text()
            {
                return Err(AcceptedProjectSnapshotError::HirTextMismatch {
                    module,
                    source: project_source,
                });
            }
            if let Some(first) = module_by_source.insert(project_source.clone(), module.clone()) {
                return Err(AcceptedProjectSnapshotError::ConflictingModuleMapping {
                    source: project_source,
                    first,
                    conflicting: module,
                });
            }
        }

        let footprint = AcceptedProjectFootprint {
            documents: document_count,
            modules: module_count,
            source_bytes: source_builder.source_bytes,
        };
        let linked = hir.linked_module();
        let typecheck = analyze_registered_project_types(&linked, world);
        if !typecheck.diagnostics.is_empty() {
            return Err(AcceptedProjectSnapshotError::TypeCheck(
                typecheck
                    .diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
            ));
        }
        let checked_entries = check_project_entries(
            hir.as_ref(),
            world.symbols(),
            world.environment().callable_catalog(),
            &typecheck,
        )
        .map_err(|diagnostics| {
            AcceptedProjectSnapshotError::EntryBinding(
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message().to_owned())
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        })?;
        let source_set_revision = symbols.revision().as_source_set().as_bytes().iter().fold(
            String::with_capacity(64),
            |mut output, byte| {
                write!(&mut output, "{byte:02x}")
                    .expect("formatting a byte into a String cannot fail");
                output
            },
        );
        let semantic_index = Arc::new(
            project_semantic_index_from_checked_project(
                hir.as_ref(),
                symbols,
                &typecheck,
                arcweft_lang_sema::project_index::ProgramHash::new(format!(
                    "lsp:source-set-v1:{source_set_revision}"
                )),
                &checked_entries,
            )
            .map_err(|error| AcceptedProjectSnapshotError::SemanticIndex(error.to_string()))?,
        );
        let sources = source_builder.finish(
            symbols.world().clone(),
            *symbols.revision(),
            character_source_revision,
        );
        let mut callable_references = typecheck
            .project_callable_references
            .iter()
            .filter_map(|reference| {
                let identity = hir.source(reference.module())?;
                let source = sources.get(identity)?;
                let start = reference.range().start();
                let end = reference.range().end();
                let span = source.document().span(SourceRange::new(start, end)).ok()?;
                Some(AcceptedCallableReference {
                    declaration: reference.declaration().clone(),
                    source: span,
                })
            })
            .collect::<Vec<_>>();
        callable_references.extend(import_callable_references(&hir, symbols, &sources));
        callable_references.sort_by(|left, right| {
            left.source
                .source()
                .id()
                .as_str()
                .cmp(right.source.source().id().as_str())
                .then_with(|| {
                    left.source
                        .range()
                        .start()
                        .cmp(&right.source.range().start())
                })
                .then_with(|| left.declaration.cmp(&right.declaration))
        });
        callable_references.dedup();
        let mut entry_references = Vec::new();
        for reference in &typecheck.project_entity_references {
            let Some(identity) = hir.source(reference.module()) else {
                continue;
            };
            let Some(source) = sources.get(identity) else {
                continue;
            };
            let Some(entry) = semantic_index
                .entry_records()
                .keys()
                .find(|entry| entry.public_id().as_str() == reference.name())
            else {
                continue;
            };
            let Ok(span) = source.document().span(SourceRange::new(
                reference.range().start(),
                reference.range().end(),
            )) else {
                continue;
            };
            entry_references.push(AcceptedEntryReference {
                entry: entry.clone(),
                source: span,
            });
        }
        entry_references.sort_by(|left, right| {
            left.source
                .source()
                .id()
                .as_str()
                .cmp(right.source.source().id().as_str())
                .then_with(|| {
                    left.source
                        .range()
                        .start()
                        .cmp(&right.source.range().start())
                })
                .then_with(|| left.entry.cmp(&right.entry))
        });
        entry_references.dedup();
        Ok(Self {
            hir,
            typecheck: Arc::new(typecheck),
            semantic_index,
            callable_references: callable_references.into(),
            entry_references: entry_references.into(),
            sources,
            module_by_source,
            footprint,
        })
    }

    pub(crate) const fn hir_project(&self) -> &Arc<HirProject> {
        &self.hir
    }

    pub(crate) const fn typecheck(&self) -> &Arc<TypeCheckReport> {
        &self.typecheck
    }

    pub(crate) const fn semantic_index(&self) -> &Arc<ProjectSemanticIndex> {
        &self.semantic_index
    }

    pub(crate) fn callable_references(&self) -> &[AcceptedCallableReference] {
        &self.callable_references
    }

    pub(crate) fn entry_references(&self) -> &[AcceptedEntryReference] {
        &self.entry_references
    }

    pub(crate) const fn sources(&self) -> &AcceptedSourceDocuments {
        &self.sources
    }

    pub(crate) fn source_identity_by_uri(
        &self,
        uri: &LspUriKey,
    ) -> Option<&SourceDocumentIdentity> {
        self.sources.source_identity_by_uri(uri)
    }

    pub(crate) fn source(
        &self,
        identity: &SourceDocumentIdentity,
    ) -> Option<&AcceptedSourceDocument> {
        self.sources.get(identity)
    }

    pub(crate) fn module_key(&self, source: &SourceDocumentIdentity) -> Option<AcceptedModuleKey> {
        self.module_by_source
            .get(source)
            .cloned()
            .map(|module| AcceptedModuleKey {
                module,
                source: source.clone(),
            })
    }

    #[allow(
        clippy::result_large_err,
        reason = "lookup failures retain the exact accepted module and source key"
    )]
    pub(crate) fn hir(
        &self,
        key: &AcceptedModuleKey,
    ) -> Result<&HirModule, AcceptedHirLookupError> {
        let hir = self
            .hir
            .module(key.module())
            .ok_or_else(|| AcceptedHirLookupError::MissingModule { key: key.clone() })?;
        if hir.source_identity() != Some(key.source()) {
            return Err(AcceptedHirLookupError::SourceIdentityMismatch {
                key: key.clone(),
                actual: hir.source_identity().cloned(),
            });
        }
        let document = hir
            .source_document()
            .ok_or_else(|| AcceptedHirLookupError::MissingSourceDocument { key: key.clone() })?;
        if document.identity() != key.source() {
            return Err(AcceptedHirLookupError::SourceDocumentMismatch {
                key: key.clone(),
                actual: document.identity().clone(),
            });
        }
        Ok(hir)
    }

    #[allow(dead_code, reason = "retained for bounded accepted-project metrics")]
    pub(crate) const fn footprint(&self) -> AcceptedProjectFootprint {
        self.footprint
    }
}

fn import_callable_references(
    hir: &HirProject,
    symbols: &ProjectSymbolTable,
    sources: &AcceptedSourceDocuments,
) -> Vec<AcceptedCallableReference> {
    let mut references = Vec::new();
    for (module, hir_module) in hir.modules() {
        let Some(identity) = hir.source(module) else {
            continue;
        };
        let Some(source) = sources.get(identity) else {
            continue;
        };
        for import in hir_module.uses() {
            match import.tree().kind() {
                UseTreeKind::Path { path, .. } => {
                    let Some(range) = path.segment_ranges().last().copied() else {
                        continue;
                    };
                    let Ok(reference) = SymbolPath::try_from(path.path()) else {
                        continue;
                    };
                    push_import_reference(
                        &mut references,
                        symbols,
                        module,
                        source,
                        &reference,
                        range,
                    );
                }
                UseTreeKind::Group {
                    module: prefix,
                    names,
                } => {
                    for name in names {
                        let Ok(path) = ProjectSymbolPath::from_str(&format!(
                            "{}.{}",
                            prefix.path(),
                            name.name()
                        )) else {
                            continue;
                        };
                        let Ok(reference) = SymbolPath::try_from(&path) else {
                            continue;
                        };
                        push_import_reference(
                            &mut references,
                            symbols,
                            module,
                            source,
                            &reference,
                            name.name_range(),
                        );
                    }
                }
                UseTreeKind::Glob { .. } => {}
            }
        }
    }
    references
}

fn push_import_reference(
    references: &mut Vec<AcceptedCallableReference>,
    symbols: &ProjectSymbolTable,
    module: &CanonicalModulePath,
    source: &AcceptedSourceDocument,
    reference: &SymbolPath,
    range: arcweft_lang_syntax::ast::common::TextRange,
) {
    let Ok(span) = source
        .document()
        .span(SourceRange::new(range.start(), range.end()))
    else {
        return;
    };
    let Ok(callable) = symbols.resolve_callable(module, reference, &span) else {
        return;
    };
    references.push(AcceptedCallableReference {
        declaration: callable.declaration().clone(),
        source: span,
    });
}

impl AcceptedSourceDocuments {
    pub(crate) const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }

    pub(crate) const fn symbol_revision(&self) -> &ProjectSymbolRevision {
        &self.symbol_revision
    }

    pub(crate) const fn all_source_revision(&self) -> SourceSetRevision {
        self.all_source_revision
    }

    pub(crate) fn get(&self, identity: &SourceDocumentIdentity) -> Option<&AcceptedSourceDocument> {
        self.by_identity.get(identity)
    }

    pub(crate) fn source_identity_by_uri(
        &self,
        uri: &LspUriKey,
    ) -> Option<&SourceDocumentIdentity> {
        self.by_uri.get(uri)
    }

    pub(crate) fn by_uri(&self, uri: &Uri) -> Option<&AcceptedSourceDocument> {
        self.source_identity_by_uri(&LspUriKey::from_uri(uri))
            .and_then(|identity| self.by_identity.get(identity))
    }

    #[allow(
        dead_code,
        reason = "retained exact source inventory supports lifecycle metrics"
    )]
    pub(crate) fn documents(&self) -> impl ExactSizeIterator<Item = &AcceptedSourceDocument> {
        self.by_identity.values()
    }
}

impl AcceptedSourceDocument {
    pub(crate) const fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    pub(crate) const fn locator(&self) -> &AcceptedSourceLocator {
        &self.locator
    }

    #[allow(
        dead_code,
        reason = "exact topology ownership is retained for later LSP policy"
    )]
    pub(crate) const fn ownership(&self) -> AcceptedSourceOwnership {
        self.ownership
    }

    #[allow(
        dead_code,
        reason = "exact topology access is retained for later LSP policy"
    )]
    pub(crate) const fn access(&self) -> AcceptedSourceAccess {
        self.access
    }

    pub(crate) const fn line_index(&self) -> &LineIndex {
        &self.line_index
    }
}

impl AcceptedSourceLocator {
    pub(crate) fn uri(&self) -> Option<&Uri> {
        match self {
            Self::File { uri, .. } | Self::Uri { uri } => Some(uri),
            Self::Unavailable => None,
        }
    }

    pub(crate) fn path(&self) -> Option<&Path> {
        match self {
            Self::File { path, .. } => Some(path),
            Self::Uri { .. } | Self::Unavailable => None,
        }
    }
}

#[cfg(test)]
mod tests;
