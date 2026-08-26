//! Immutable source, module, and HIR authority retained by one accepted profile generation.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use arcweft_compiler::project::{CompiledProject, ProjectToolingLease};
use arcweft_lang_hir::{
    expr::HirExprKind,
    item::{HirItemKind, HirUseBindingKind},
    leaf::HirIdRef,
    module::HirModule,
    project::HirProject,
    source_index::{
        HirExprSourceRole, HirIdRefSourcePart, HirItemSourceRole, HirSourcePresence,
        HirSourceQuery, HirSourceSite, HirUseBindingSourcePart, HirUseSourceRole,
    },
    symbol::{
        CallableDeclarationKey, ProjectSymbolRevision, ProjectSymbolTable, ProjectSymbolWorldId,
        ProjectValueLookup,
    },
};
use arcweft_lang_sema::{
    callable::{CheckedCallableLookupError, ResolvedCallableOrigin},
    entry::CheckedEntryId,
    final_analysis::{CheckedExpressionResolution, CheckedValueResolution, FinalSemanticAnalysis},
    project_index::ProjectSemanticIndex,
    registration::{CharacterRegistrationLimits, RegisteredSemanticWorld},
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::incremental::ParsedSource;
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
    character_source_revision: Option<SourceSetRevision>,
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

/// One immutable HIR/source/module carrier published from an accepted tooling lease.
#[derive(Debug)]
pub(crate) struct AcceptedProjectSnapshot {
    tooling: Arc<ProjectToolingLease>,
    callable_references: Arc<[AcceptedCallableReference]>,
    entry_references: Arc<[AcceptedEntryReference]>,
    sources: AcceptedSourceDocuments,
    module_by_source: BTreeMap<SourceDocumentIdentity, CanonicalModulePath>,
    #[allow(dead_code, reason = "retained for bounded accepted-project metrics")]
    footprint: AcceptedProjectFootprint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedCallableReference {
    declaration: CallableDeclarationKey,
    source: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedEntryReference {
    entry: CheckedEntryId,
    source: SourceSpan,
}

impl AcceptedCallableReference {
    pub(crate) const fn declaration(&self) -> &CallableDeclarationKey {
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
    CompiledToolingLeaseMismatch,
    CompiledSemanticGenerationMismatch,
    CompiledCheckedCatalogLeaseMismatch,
    CompiledCheckedCatalogAuthority(CheckedCallableLookupError),
    CompiledModuleInventoryMismatch {
        project_only: Box<[CanonicalModulePath]>,
        compiled_only: Box<[CanonicalModulePath]>,
    },
    MissingCompiledModule {
        module: CanonicalModulePath,
    },
    MissingModuleDocument {
        module: CanonicalModulePath,
        source: SourceDocumentIdentity,
    },
    CompiledModuleLeaseMismatch {
        module: CanonicalModulePath,
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

#[derive(Debug)]
enum CompiledSemanticAuthorityError {
    GenerationMismatch,
    CatalogLeaseMismatch,
    CatalogAuthority(CheckedCallableLookupError),
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
            Self::CompiledToolingLeaseMismatch => write!(
                formatter,
                "compiled project does not retain the accepted tooling lease allocation"
            ),
            Self::CompiledSemanticGenerationMismatch => write!(
                formatter,
                "compiled semantic report does not match the retained HIR generation"
            ),
            Self::CompiledCheckedCatalogLeaseMismatch => write!(
                formatter,
                "compiled semantic report and project index do not retain the same checked callable catalog allocation"
            ),
            Self::CompiledCheckedCatalogAuthority(error) => write!(
                formatter,
                "compiled checked callable catalog is not admitted by the accepted semantic world: {error:?}"
            ),
            Self::CompiledModuleInventoryMismatch {
                project_only,
                compiled_only,
            } => write!(
                formatter,
                "compiled/HIR project module inventory mismatch: project-only {project_only:?}, compiled-only {compiled_only:?}"
            ),
            Self::MissingCompiledModule { module } => {
                write!(
                    formatter,
                    "compiled module is absent from the accepted HIR project: {module:?}"
                )
            }
            Self::MissingModuleDocument { module, source } => write!(
                formatter,
                "module source is absent from accepted documents: {module:?} -> {source:?}"
            ),
            Self::CompiledModuleLeaseMismatch { module } => write!(
                formatter,
                "compiled module does not retain the HIR project's exact Arc lease: {module:?}"
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

impl From<CompiledSemanticAuthorityError> for AcceptedProjectSnapshotError {
    fn from(error: CompiledSemanticAuthorityError) -> Self {
        match error {
            CompiledSemanticAuthorityError::GenerationMismatch => {
                Self::CompiledSemanticGenerationMismatch
            }
            CompiledSemanticAuthorityError::CatalogLeaseMismatch => {
                Self::CompiledCheckedCatalogLeaseMismatch
            }
            CompiledSemanticAuthorityError::CatalogAuthority(error) => {
                Self::CompiledCheckedCatalogAuthority(error)
            }
        }
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
        actual: SourceDocumentIdentity,
    },
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
        character_source_revision: Option<SourceSetRevision>,
    ) -> AcceptedSourceDocuments {
        AcceptedSourceDocuments {
            world,
            symbol_revision,
            character_source_revision,
            by_identity: self.by_identity,
            by_uri: self.by_uri,
        }
    }
}

#[allow(
    clippy::result_large_err,
    reason = "the accepted-HIR invariant reports the exact module and source identity"
)]
fn validate_bound_hir_source(
    module: &CanonicalModulePath,
    expected_source: &SourceDocumentIdentity,
    bound_source: &SourceDocumentIdentity,
    bound_text: &str,
    accepted_text: &str,
) -> Result<(), AcceptedProjectSnapshotError> {
    if bound_source != expected_source || bound_text != accepted_text {
        return Err(AcceptedProjectSnapshotError::HirTextMismatch {
            module: module.clone(),
            source: expected_source.clone(),
        });
    }
    Ok(())
}

fn validate_compiled_semantic_authority(
    compiled: &CompiledProject,
) -> Result<(), CompiledSemanticAuthorityError> {
    let analysis = compiled.final_analysis();
    let project = compiled
        .hir_project()
        .executable_view()
        .map_err(|_| CompiledSemanticAuthorityError::GenerationMismatch)?;
    analysis
        .validate_generation(project, compiled.project_symbols())
        .map_err(|_| CompiledSemanticAuthorityError::GenerationMismatch)?;
    let checked = analysis.checked_callables();
    if !Arc::ptr_eq(checked, compiled.semantic_index().checked_callables()) {
        return Err(CompiledSemanticAuthorityError::CatalogLeaseMismatch);
    }
    analysis
        .validate_registered_callable_authority(
            compiled.registered_world().environment().callable_catalog(),
        )
        .map_err(CompiledSemanticAuthorityError::CatalogAuthority)
}

impl AcceptedProjectSnapshot {
    #[allow(
        clippy::result_large_err,
        clippy::too_many_lines,
        reason = "one admission boundary validates and preserves the exact HIR, symbol, source, and document identity tuple"
    )]
    pub(crate) fn try_new(
        tooling: Arc<ProjectToolingLease>,
        executable: Option<&CompiledProject>,
        source_seeds: Vec<AcceptedSourceDocumentSeed>,
    ) -> Result<Self, AcceptedProjectSnapshotError> {
        if executable.is_some_and(|compiled| !Arc::ptr_eq(compiled.tooling_lease(), &tooling)) {
            return Err(AcceptedProjectSnapshotError::CompiledToolingLeaseMismatch);
        }
        if let Some(compiled) = executable {
            validate_compiled_semantic_authority(compiled)
                .map_err(AcceptedProjectSnapshotError::from)?;
        }
        let mut source_builder = AcceptedSourceRegistryBuilder::default();
        for seed in source_seeds {
            source_builder.insert(seed)?;
        }
        let character_source_revision = executable
            .map(CompiledProject::registered_world)
            .map(|world| source_builder.validate_world(world))
            .transpose()?;

        let symbols = tooling.project_symbols();
        let hir = tooling.hir_project();
        let hir_modules = hir
            .view()
            .modules()
            .map(|(module, _)| module.clone())
            .collect::<BTreeSet<_>>();
        let compiled_modules = tooling
            .modules()
            .iter()
            .map(|module| module.module().clone())
            .collect::<BTreeSet<_>>();
        if hir_modules != compiled_modules {
            return Err(
                AcceptedProjectSnapshotError::CompiledModuleInventoryMismatch {
                    project_only: hir_modules
                        .difference(&compiled_modules)
                        .cloned()
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    compiled_only: compiled_modules
                        .difference(&hir_modules)
                        .cloned()
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                },
            );
        }

        let module_count = u64::try_from(compiled_modules.len()).map_err(|_| {
            AcceptedProjectSnapshotError::ArithmeticOverflow {
                counter: AcceptedProjectLimitKind::Modules,
            }
        })?;
        let document_count = u64::try_from(source_builder.by_identity.len()).map_err(|_| {
            AcceptedProjectSnapshotError::ArithmeticOverflow {
                counter: AcceptedProjectLimitKind::Documents,
            }
        })?;
        let mut module_by_source = BTreeMap::new();
        for compiled_module in tooling.modules() {
            let module = compiled_module.module();
            let hir_module = hir.view().module(module).ok_or_else(|| {
                AcceptedProjectSnapshotError::MissingCompiledModule {
                    module: module.clone(),
                }
            })?;
            if !Arc::ptr_eq(hir_module, compiled_module.hir()) {
                return Err(AcceptedProjectSnapshotError::CompiledModuleLeaseMismatch {
                    module: module.clone(),
                });
            }
            let hir_source = hir_module.provenance().source_identity().clone();
            let accepted = source_builder
                .by_identity
                .get_mut(&hir_source)
                .ok_or_else(|| AcceptedProjectSnapshotError::MissingModuleDocument {
                    module: module.clone(),
                    source: hir_source.clone(),
                })?;
            let bound_document = hir_module.provenance().document();
            let parsed_document = compiled_module.parsed().document_lease();
            validate_bound_hir_source(
                module,
                &hir_source,
                bound_document.identity(),
                bound_document.text(),
                parsed_document.text(),
            )?;
            validate_bound_hir_source(
                module,
                &hir_source,
                parsed_document.identity(),
                parsed_document.text(),
                accepted.document.text(),
            )?;
            accepted.document = Arc::clone(parsed_document);
            if let Some(first) = module_by_source.insert(hir_source.clone(), module.clone()) {
                return Err(AcceptedProjectSnapshotError::ConflictingModuleMapping {
                    source: hir_source,
                    first,
                    conflicting: module.clone(),
                });
            }
        }
        if module_count > document_count {
            return Err(AcceptedProjectSnapshotError::Limit {
                kind: AcceptedProjectLimitKind::Modules,
                observed: module_count,
                maximum: document_count,
            });
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
        let mut callable_references = import_callable_references(hir.as_ref(), symbols, &sources);
        if let Some(compiled) = executable {
            callable_references.extend(final_call_references(
                hir.as_ref(),
                compiled.final_analysis(),
                &sources,
            ));
        }
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
        let mut entry_references = executable.map_or_else(Vec::new, |compiled| {
            final_entry_references(
                hir.as_ref(),
                compiled.final_analysis(),
                compiled.semantic_index(),
                &sources,
            )
        });
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
            tooling,
            callable_references: callable_references.into(),
            entry_references: entry_references.into(),
            sources,
            module_by_source,
            footprint,
        })
    }

    pub(crate) fn hir_project(&self) -> &Arc<HirProject> {
        self.tooling.hir_project()
    }

    pub(crate) const fn tooling_lease(&self) -> &Arc<ProjectToolingLease> {
        &self.tooling
    }

    /// Exact project-symbol authority published with this accepted generation.
    pub(crate) fn project_symbols(&self) -> &ProjectSymbolTable {
        self.tooling.project_symbols()
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

    /// Returns the compiler-retained grammar lease for one accepted module key.
    pub(crate) fn parsed_source(&self, key: &AcceptedModuleKey) -> Option<&ParsedSource> {
        self.tooling
            .modules()
            .iter()
            .find(|module| module.module() == key.module())
            .filter(|module| module.source() == key.source())
            .map(arcweft_compiler::project::CompiledProjectModule::parsed)
    }

    #[allow(
        clippy::result_large_err,
        reason = "lookup failures retain the exact accepted module and source key"
    )]
    pub(crate) fn hir(
        &self,
        key: &AcceptedModuleKey,
    ) -> Result<&Arc<HirModule>, AcceptedHirLookupError> {
        let hir = self
            .tooling
            .hir_project()
            .view()
            .module(key.module())
            .ok_or_else(|| AcceptedHirLookupError::MissingModule { key: key.clone() })?;
        if hir.provenance().source_identity() != key.source() {
            return Err(AcceptedHirLookupError::SourceIdentityMismatch {
                key: key.clone(),
                actual: hir.provenance().source_identity().clone(),
            });
        }
        let document = hir.provenance().document();
        if document.identity() != key.source() {
            return Err(AcceptedHirLookupError::SourceDocumentMismatch {
                key: key.clone(),
                actual: document.identity().clone(),
            });
        }
        Ok(hir)
    }

    /// Returns the exact accepted HIR lease for one accepted open-document lease.
    ///
    /// LSP feature readers must not reparse the overlay or reconstruct a
    /// detached HIR. Equal bytes in a copied source document are not accepted;
    /// the live store must own the compiler-retained document lease.
    pub(crate) fn hir_for_open_document(
        &self,
        uri: &Uri,
        document: &Arc<SourceDocument>,
    ) -> Option<&Arc<HirModule>> {
        let source = self.sources.by_uri(uri)?;
        if !Arc::ptr_eq(&source.document, document) {
            return None;
        }
        let key = self.module_key(source.document.identity())?;
        self.hir(&key).ok()
    }

    #[allow(dead_code, reason = "retained for bounded accepted-project metrics")]
    pub(crate) const fn footprint(&self) -> AcceptedProjectFootprint {
        self.footprint
    }
}

fn final_entry_references(
    project: &HirProject,
    analysis: &FinalSemanticAnalysis,
    semantic_index: &ProjectSemanticIndex,
    sources: &AcceptedSourceDocuments,
) -> Vec<AcceptedEntryReference> {
    let mut references = Vec::new();
    for (owner, checked) in analysis.expressions() {
        let CheckedExpressionResolution::Value(CheckedValueResolution::Entry(reference)) =
            checked.resolution()
        else {
            continue;
        };
        let Some(entry) = semantic_index
            .entry_records()
            .keys()
            .find(|entry| entry.public_id() == reference.diagnostic_public_id())
        else {
            continue;
        };
        let Some((_, module)) = project
            .view()
            .modules()
            .find(|(_, module)| module.module_id() == owner.module())
        else {
            continue;
        };
        let Ok(expression) = module.resolve_expr(owner) else {
            continue;
        };
        let HirExprKind::EntityReference(reference) = expression.kind() else {
            continue;
        };
        let Some(HirIdRef::Absolute(reference)) = reference.as_resolved() else {
            continue;
        };
        let Some(last_ordinal) = reference.segment_count().checked_sub(1) else {
            continue;
        };
        let Ok(last_ordinal) = u32::try_from(last_ordinal) else {
            continue;
        };
        let Some(first) = final_hir_source_span(
            module,
            HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment {
                    ordinal: 0,
                }),
            },
        ) else {
            continue;
        };
        let Some(last) = final_hir_source_span(
            module,
            HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment {
                    ordinal: last_ordinal,
                }),
            },
        ) else {
            continue;
        };
        if first.source() != last.source() {
            continue;
        }
        let Some(source) = sources.get(first.source()) else {
            continue;
        };
        let Ok(source) = source
            .document()
            .span(SourceRange::new(first.range().start(), last.range().end()))
        else {
            continue;
        };
        references.push(AcceptedEntryReference {
            entry: entry.clone(),
            source,
        });
    }
    references
}

fn final_call_references(
    project: &HirProject,
    analysis: &FinalSemanticAnalysis,
    sources: &AcceptedSourceDocuments,
) -> Vec<AcceptedCallableReference> {
    let mut references = Vec::new();
    for (owner, call) in analysis.calls() {
        let Some(application) = call.selected_application() else {
            continue;
        };
        let ResolvedCallableOrigin::Project { declaration, .. } =
            application.core().candidates().selected().origin()
        else {
            continue;
        };
        let Some((_, module)) = project
            .view()
            .modules()
            .find(|(_, module)| module.module_id() == owner.module())
        else {
            continue;
        };
        let identity = module.provenance().source_identity();
        if sources.get(identity).is_none() {
            continue;
        }
        let Some(source) = final_hir_source_span(
            module,
            HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::CallCallee,
            },
        ) else {
            continue;
        };
        references.push(AcceptedCallableReference {
            declaration: declaration.clone(),
            source,
        });
    }
    references
}

fn import_callable_references(
    project: &HirProject,
    symbols: &ProjectSymbolTable,
    sources: &AcceptedSourceDocuments,
) -> Vec<AcceptedCallableReference> {
    let mut references = Vec::new();
    for (module_path, module) in project.view().modules() {
        let identity = module.provenance().source_identity();
        if sources.get(identity).is_none() {
            continue;
        }
        for owner in module.source_ordered_items() {
            let Ok(item) = module.resolve_item(*owner) else {
                continue;
            };
            let HirItemKind::Use(import) = item.kind() else {
                continue;
            };
            for (ordinal, binding) in import.bindings().iter().enumerate() {
                if binding.kind() != HirUseBindingKind::Item {
                    continue;
                }
                let Some(path) = binding.path().as_resolved() else {
                    continue;
                };
                let Ok(ordinal) = u32::try_from(ordinal) else {
                    continue;
                };
                let Some(source) = final_hir_source_span(
                    module,
                    HirSourceQuery::Item {
                        owner: *owner,
                        role: HirItemSourceRole::Use(HirUseSourceRole::Binding {
                            ordinal,
                            part: HirUseBindingSourcePart::TerminalReference,
                        }),
                    },
                ) else {
                    continue;
                };
                let Ok(ProjectValueLookup::Present(callable)) =
                    symbols.resolve_hir_value_target(module_path, path, source.clone())
                else {
                    continue;
                };
                references.push(AcceptedCallableReference {
                    declaration: callable.declaration().clone(),
                    source,
                });
            }
        }
    }
    references
}

fn final_hir_source_span(module: &HirModule, query: HirSourceQuery) -> Option<SourceSpan> {
    let lookup = module
        .source_site(module.provenance().source_identity(), query)
        .ok()?;
    match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => Some(span.clone()),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => None,
    }
}

impl AcceptedSourceDocuments {
    pub(crate) const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }

    pub(crate) const fn symbol_revision(&self) -> &ProjectSymbolRevision {
        &self.symbol_revision
    }

    pub(crate) const fn character_source_revision(&self) -> Option<SourceSetRevision> {
        self.character_source_revision
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
pub(crate) mod stamp_test_support;
#[cfg(test)]
mod tests;
