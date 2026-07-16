//! Immutable source, module, and HIR authority retained by one accepted profile generation.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use arcweft_lang_hir::{
    model::HirModule,
    project::HirProject,
    symbol::{ProjectSymbolRevision, ProjectSymbolWorldId},
};
use arcweft_lang_sema::registration::{CharacterRegistrationLimits, RegisteredSemanticWorld};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceDocumentIdentity, SourceSetRevision,
    SourceSetRevisionError,
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

    #[allow(dead_code, reason = "retained for bounded accepted-project metrics")]
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
    sources: AcceptedSourceDocuments,
    module_by_source: BTreeMap<SourceDocumentIdentity, CanonicalModulePath>,
    #[allow(dead_code, reason = "retained for bounded accepted-project metrics")]
    footprint: AcceptedProjectFootprint,
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
    SourceSet(SourceSetRevisionError),
}

impl std::fmt::Display for AcceptedProjectSnapshotError {
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
        let sources = source_builder.finish(
            symbols.world().clone(),
            *symbols.revision(),
            character_source_revision,
        );
        Ok(Self {
            hir,
            sources,
            module_by_source,
            footprint,
        })
    }

    pub(crate) const fn hir_project(&self) -> &Arc<HirProject> {
        &self.hir
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
mod tests {
    use super::*;
    use arcweft_lang_hir::{
        lower::lower_document_to_hir,
        project::HirProjectModule,
        symbol::{CallablePackageId, ProjectSymbolWorldId},
    };
    use arcweft_lang_sema::{
        env::TypeCheckEnv,
        registration::{
            CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts,
        },
    };
    use arcweft_lang_syntax::{ast::module_path::ModuleSegment, parser::parse_source};
    use arcweft_source::SourceName;

    fn document(id: &str, text: &str) -> Arc<SourceDocument> {
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(id).expect("document ID"),
                SourceName::path(id),
                text,
            )
            .expect("source document"),
        )
    }

    fn module(path: CanonicalModulePath, document: &Arc<SourceDocument>) -> HirProjectModule {
        let parsed = parse_source(document.text());
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let hir = lower_document_to_hir(document, parsed.typed_tree()).expect("lowered HIR");
        HirProjectModule::try_new(path, document.identity().clone(), hir)
            .expect("source-bound module")
    }

    fn project_and_world(
        modules: &[(CanonicalModulePath, Arc<SourceDocument>)],
    ) -> (Arc<HirProject>, Arc<RegisteredSemanticWorld>) {
        let root = Arc::clone(&modules[0].1);
        let documents = modules
            .iter()
            .map(|(_, document)| Arc::clone(document))
            .collect::<Vec<_>>();
        let project = Arc::new(
            HirProject::new(
                "accepted-project-tests",
                modules
                    .iter()
                    .map(|(path, document)| module(path.clone(), document)),
            )
            .expect("HIR project"),
        );
        let world = ProjectSymbolWorldId::try_new(
            CallablePackageId::try_new("accepted-project-tests").expect("package"),
            root.identity().id().clone(),
            "test",
        )
        .expect("world");
        let facts = ProjectRegistrationFacts::try_new(world, documents, Vec::new(), Vec::new())
            .expect("registration facts");
        let registered = Arc::new(
            CharacterRegistrar::register(CharacterRegistrationRequest::new(
                Arc::new(TypeCheckEnv::standard()),
                project.as_ref(),
                &facts,
                None,
            ))
            .expect("registered world"),
        );
        (project, registered)
    }

    fn seed(document: Arc<SourceDocument>, uri: &str) -> AcceptedSourceDocumentSeed {
        AcceptedSourceDocumentSeed::new(
            document,
            AcceptedSourceLocator::Uri {
                uri: uri.parse::<Uri>().expect("URI"),
            },
            AcceptedSourceOwnership::Workspace,
            AcceptedSourceAccess::Writable,
        )
    }

    #[test]
    fn exact_root_dependency_and_declaration_free_hir_are_retained() {
        let root = document(
            "arcweft-project://accepted/root.arcw",
            "flow @flow.main main { return \"ok\" }\n",
        );
        let dependency = document("arcweft-project://accepted/empty.arcw", "\n");
        let dependency_path =
            CanonicalModulePath::from_segments([
                ModuleSegment::new("dependency").expect("dependency segment")
            ]);
        let (hir, world) = project_and_world(&[
            (CanonicalModulePath::crate_root(), Arc::clone(&root)),
            (dependency_path.clone(), Arc::clone(&dependency)),
        ]);
        let snapshot = AcceptedProjectSnapshot::try_new(
            Arc::clone(&hir),
            world.as_ref(),
            vec![
                seed(Arc::clone(&root), "file:///accepted/root.arcw"),
                AcceptedSourceDocumentSeed::new(
                    Arc::clone(&dependency),
                    AcceptedSourceLocator::Uri {
                        uri: "arcweft-dependency:///empty.arcw"
                            .parse::<Uri>()
                            .expect("dependency URI"),
                    },
                    AcceptedSourceOwnership::Dependency,
                    AcceptedSourceAccess::ReadOnly,
                ),
            ],
        )
        .expect("accepted snapshot");

        assert!(Arc::ptr_eq(snapshot.hir_project(), &hir));
        let root_key = snapshot
            .module_key(root.identity())
            .expect("root module key");
        assert_eq!(root_key.module(), &CanonicalModulePath::crate_root());
        assert_eq!(
            snapshot.hir(&root_key).expect("root HIR").source_document(),
            Some(root.as_ref())
        );
        let dependency_key = snapshot
            .module_key(dependency.identity())
            .expect("dependency module key");
        assert_eq!(dependency_key.module(), &dependency_path);
        assert_eq!(
            snapshot
                .source(dependency.identity())
                .expect("dependency source")
                .ownership(),
            AcceptedSourceOwnership::Dependency
        );
        assert_eq!(
            snapshot
                .source(dependency.identity())
                .expect("dependency source")
                .access(),
            AcceptedSourceAccess::ReadOnly
        );
        assert_eq!(snapshot.footprint().documents(), 2);
        assert_eq!(snapshot.footprint().modules(), 2);
        assert_eq!(
            snapshot.footprint().source_bytes(),
            (root.text().len() + dependency.text().len()) as u64
        );
    }

    #[test]
    fn duplicate_identity_and_uri_are_rejected_without_overwrite() {
        let root = document(
            "arcweft-project://accepted/duplicate.arcw",
            "flow @flow.main main {}\n",
        );
        let (hir, world) =
            project_and_world(&[(CanonicalModulePath::crate_root(), Arc::clone(&root))]);
        let duplicate = AcceptedProjectSnapshot::try_new(
            Arc::clone(&hir),
            world.as_ref(),
            vec![
                seed(Arc::clone(&root), "file:///accepted/duplicate.arcw"),
                seed(Arc::clone(&root), "file:///accepted/duplicate-again.arcw"),
            ],
        );
        assert!(matches!(
            duplicate,
            Err(AcceptedProjectSnapshotError::DuplicateSourceIdentity { .. })
        ));

        let extra = document("arcweft-generated://accepted/extra.arcw", "\n");
        let duplicate_uri = AcceptedProjectSnapshot::try_new(
            hir,
            world.as_ref(),
            vec![
                seed(root, "file:///accepted/shared.arcw"),
                AcceptedSourceDocumentSeed::new(
                    extra,
                    AcceptedSourceLocator::Uri {
                        uri: "file:///accepted/shared.arcw".parse::<Uri>().expect("URI"),
                    },
                    AcceptedSourceOwnership::Generated,
                    AcceptedSourceAccess::ReadOnly,
                ),
            ],
        );
        assert!(matches!(
            duplicate_uri,
            Err(AcceptedProjectSnapshotError::DuplicateUri { .. })
        ));
    }

    #[test]
    fn accepted_generated_source_without_module_is_not_forged_into_hir() {
        let root = document(
            "arcweft-project://accepted/main.arcw",
            "flow @flow.main main {}\n",
        );
        let generated = document("arcweft-generated://accepted/index.arcw", "\n");
        let (hir, world) =
            project_and_world(&[(CanonicalModulePath::crate_root(), Arc::clone(&root))]);
        let snapshot = AcceptedProjectSnapshot::try_new(
            hir,
            world.as_ref(),
            vec![
                seed(root, "file:///accepted/main.arcw"),
                AcceptedSourceDocumentSeed::new(
                    Arc::clone(&generated),
                    AcceptedSourceLocator::Uri {
                        uri: "arcweft-generated:///index.arcw"
                            .parse::<Uri>()
                            .expect("generated URI"),
                    },
                    AcceptedSourceOwnership::Generated,
                    AcceptedSourceAccess::ReadOnly,
                ),
            ],
        )
        .expect("accepted generated source");
        assert!(snapshot.source(generated.identity()).is_some());
        assert!(snapshot.module_key(generated.identity()).is_none());
        assert_eq!(snapshot.sources().documents().len(), 2);
    }
}
