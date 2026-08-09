//! Generation-bound semantic authority for authored Proof return types.
//!
//! HIR owns the staged header and fact identities. Semantic analysis owns the
//! classification value, and lowering may consume it only through a complete
//! fact set bound to the exact unpublished project generation.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::attachment::SyntaxSnapshotId;
use arcweft_source::{SourceDocumentIdentity, SourceSetRevisionError, SourceSpan};
use thiserror::Error;

use crate::identity::{HirDatabaseId, HirLimit, HirSnapshotId, ItemId, TypeId};
use crate::symbol::{CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId};

/// Semantic result used to choose the synthetic tail of one Proof body.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirProofReturnSemanticClass {
    /// The resolved return type is semantic Unit, including aliases to Unit.
    Unit,
    /// The resolved return type is authoritative and is not Unit.
    NonUnit,
    /// Resolution was poisoned or detached and the Proof is non-executable.
    Poisoned,
}

impl HirProofReturnSemanticClass {
    /// Whether an omitted Proof block tail is the clean implicit Unit value.
    pub const fn admits_implicit_unit_tail(self) -> bool {
        matches!(self, Self::Unit)
    }

    /// Whether semantic resolution failed to produce an executable type.
    pub const fn is_poisoned(self) -> bool {
        matches!(self, Self::Poisoned)
    }
}

/// Exact module/source lease admitted to one unpublished project generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirProofReturnModuleLease {
    package: CallablePackageId,
    path: CanonicalModulePath,
    hir_snapshot: HirSnapshotId,
    syntax_snapshot: SyntaxSnapshotId,
    source: SourceDocumentIdentity,
}

impl HirProofReturnModuleLease {
    pub fn new(
        package: CallablePackageId,
        path: CanonicalModulePath,
        hir_snapshot: HirSnapshotId,
        syntax_snapshot: SyntaxSnapshotId,
        source: SourceDocumentIdentity,
    ) -> Self {
        Self {
            package,
            path,
            hir_snapshot,
            syntax_snapshot,
            source,
        }
    }

    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }

    pub const fn path(&self) -> &CanonicalModulePath {
        &self.path
    }

    pub const fn hir_snapshot(&self) -> HirSnapshotId {
        self.hir_snapshot
    }

    pub const fn syntax_snapshot(&self) -> &SyntaxSnapshotId {
        &self.syntax_snapshot
    }

    pub const fn source(&self) -> &SourceDocumentIdentity {
        &self.source
    }
}

/// Immutable identity of the header snapshot consumed by semantic analysis.
///
/// Callers retain this value behind one `Arc`; facts and body lowering must
/// share that exact lease, not merely an equal reconstruction. Its revision
/// covers the full symbol-source inventory (including non-HIR registration
/// documents), while `modules` proves the exact HIR source subset.
#[derive(Debug)]
pub struct HirProofReturnProjectGeneration {
    database: HirDatabaseId,
    world: ProjectSymbolWorldId,
    revision: ProjectSymbolRevision,
    modules: BTreeMap<CanonicalModulePath, HirProofReturnModuleLease>,
    validation_work: u64,
}

/// Invalid project-generation identity supplied to Proof return analysis.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirProofReturnGenerationError {
    #[error("Proof return project generation contains no modules")]
    Empty,
    #[error("Proof return project generation contains {observed} modules, maximum is {maximum}")]
    ModuleLimit { observed: usize, maximum: usize },
    #[error("duplicate Proof return module lease for `{module}`")]
    DuplicateModule { module: CanonicalModulePath },
    #[error("source document `{document}` is owned by more than one module")]
    DuplicateSourceDocument {
        document: arcweft_source::SourceDocumentId,
    },
    #[error("module `{module}` belongs to package {actual:?}, expected {expected:?}")]
    WrongPackage {
        module: CanonicalModulePath,
        expected: CallablePackageId,
        actual: CallablePackageId,
    },
    #[error("module `{module}` belongs to HIR database {actual:?}, expected {expected:?}")]
    WrongDatabase {
        module: CanonicalModulePath,
        expected: HirDatabaseId,
        actual: HirDatabaseId,
    },
    #[error("project symbol revision does not match the exact generation source set")]
    WrongRevision {
        expected: ProjectSymbolRevision,
        actual: ProjectSymbolRevision,
    },
    #[error(
        "module `{module}` source document `{document}` is absent from the project symbol source set"
    )]
    MissingModuleSource {
        module: CanonicalModulePath,
        document: arcweft_source::SourceDocumentId,
    },
    #[error("module `{module}` source identity does not match the project symbol source identity")]
    ModuleSourceMismatch {
        module: CanonicalModulePath,
        expected: Box<SourceDocumentIdentity>,
        actual: Box<SourceDocumentIdentity>,
    },
    #[error("project root source document is absent from the generation")]
    MissingRootDocument,
    #[error("Proof return generation validation work overflowed u64")]
    WorkOverflow,
    #[error(transparent)]
    SourceRevision(#[from] SourceSetRevisionError),
}

impl HirProofReturnProjectGeneration {
    pub fn try_new<'source>(
        database: HirDatabaseId,
        world: ProjectSymbolWorldId,
        revision: ProjectSymbolRevision,
        symbol_sources: impl IntoIterator<Item = &'source SourceDocumentIdentity>,
        modules: impl IntoIterator<Item = HirProofReturnModuleLease>,
    ) -> Result<Arc<Self>, HirProofReturnGenerationError> {
        let symbol_sources = symbol_sources.into_iter().cloned().collect::<Vec<_>>();
        let actual = ProjectSymbolRevision::try_for_documents(symbol_sources.iter())?;
        if actual != revision {
            return Err(HirProofReturnGenerationError::WrongRevision {
                expected: revision,
                actual,
            });
        }
        let symbol_sources = symbol_sources
            .into_iter()
            .map(|source| (source.id().clone(), source))
            .collect::<BTreeMap<_, _>>();
        let mut by_path = BTreeMap::new();
        let mut documents = BTreeSet::new();
        let mut has_root = false;
        let maximum = HirLimit::ModulesPerDatabase.maximum();
        for lease in modules {
            let observed = by_path.len().saturating_add(1);
            if observed > maximum {
                return Err(HirProofReturnGenerationError::ModuleLimit { observed, maximum });
            }
            if lease.package() != world.package() {
                return Err(HirProofReturnGenerationError::WrongPackage {
                    module: lease.path().clone(),
                    expected: world.package().clone(),
                    actual: lease.package().clone(),
                });
            }
            let actual_database = lease.hir_snapshot().module().database();
            if actual_database != database {
                return Err(HirProofReturnGenerationError::WrongDatabase {
                    module: lease.path().clone(),
                    expected: database,
                    actual: actual_database,
                });
            }
            if !documents.insert(lease.source().id().clone()) {
                return Err(HirProofReturnGenerationError::DuplicateSourceDocument {
                    document: lease.source().id().clone(),
                });
            }
            let Some(symbol_source) = symbol_sources.get(lease.source().id()) else {
                return Err(HirProofReturnGenerationError::MissingModuleSource {
                    module: lease.path().clone(),
                    document: lease.source().id().clone(),
                });
            };
            if symbol_source != lease.source() {
                return Err(HirProofReturnGenerationError::ModuleSourceMismatch {
                    module: lease.path().clone(),
                    expected: Box::new(symbol_source.clone()),
                    actual: Box::new(lease.source().clone()),
                });
            }
            has_root |= lease.source().id() == world.root_document();
            let path = lease.path().clone();
            if by_path.insert(path.clone(), lease).is_some() {
                return Err(HirProofReturnGenerationError::DuplicateModule { module: path });
            }
        }
        if by_path.is_empty() {
            return Err(HirProofReturnGenerationError::Empty);
        }
        if !has_root {
            return Err(HirProofReturnGenerationError::MissingRootDocument);
        }
        let validation_work = u64::try_from(symbol_sources.len())
            .ok()
            .and_then(|sources| {
                u64::try_from(by_path.len())
                    .ok()
                    .and_then(|modules| sources.checked_add(modules))
            })
            .ok_or(HirProofReturnGenerationError::WorkOverflow)?;
        Ok(Arc::new(Self {
            database,
            world,
            revision,
            validation_work,
            modules: by_path,
        }))
    }

    pub const fn database(&self) -> HirDatabaseId {
        self.database
    }

    pub const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }

    pub const fn revision(&self) -> ProjectSymbolRevision {
        self.revision
    }

    pub fn modules(&self) -> impl ExactSizeIterator<Item = &HirProofReturnModuleLease> {
        self.modules.values()
    }

    pub fn module(&self, path: &CanonicalModulePath) -> Option<&HirProofReturnModuleLease> {
        self.modules.get(path)
    }

    /// Deterministic module-row validation work charged by construction.
    pub const fn validation_work(&self) -> u64 {
        self.validation_work
    }
}

/// Exact authored Proof return header emitted before any body allocation.
#[derive(Clone, Debug)]
pub struct HirProofReturnHeader {
    generation: Arc<HirProofReturnProjectGeneration>,
    module: CanonicalModulePath,
    item: ItemId,
    return_type: TypeId,
    source: SourceSpan,
}

/// Invalid generation/header/fact relationship.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirProofReturnAuthorityError {
    #[error("Proof return header references unknown module `{module}`")]
    UnknownModule { module: CanonicalModulePath },
    #[error("Proof return header HIR snapshot does not own its item and type")]
    WrongHirOwner {
        expected: crate::identity::HirModuleId,
        item: crate::identity::HirModuleId,
        return_type: crate::identity::HirModuleId,
    },
    #[error("Proof return header source does not match the generation module source")]
    WrongSource {
        expected: Box<SourceDocumentIdentity>,
        actual: Box<SourceDocumentIdentity>,
    },
    #[error("Proof return module transaction uses a different HIR snapshot")]
    WrongHirSnapshot {
        expected: HirSnapshotId,
        actual: HirSnapshotId,
    },
    #[error("Proof return module transaction uses a different syntax snapshot")]
    WrongSyntaxSnapshot {
        expected: Box<SyntaxSnapshotId>,
        actual: Box<SyntaxSnapshotId>,
    },
    #[error("Proof return module transaction uses a different package")]
    WrongPackage {
        expected: CallablePackageId,
        actual: CallablePackageId,
    },
    #[error("Proof return header belongs to another project-generation lease")]
    ForeignGeneration,
    #[error("duplicate Proof return header for item {item:?}")]
    DuplicateHeader { item: ItemId },
    #[error("duplicate Proof return semantic fact for item {item:?}")]
    DuplicateFact { item: ItemId },
    #[error("Proof return semantic fact is missing for item {item:?}")]
    MissingFact { item: ItemId },
    #[error("Proof return semantic fact has no staged header for item {item:?}")]
    UnexpectedFact { item: ItemId },
    #[error("Proof return semantic fact item/type/source binding is stale or foreign")]
    FactBindingMismatch {
        item: ItemId,
        expected_type: TypeId,
        actual_type: TypeId,
        expected_source: Box<SourceSpan>,
        actual_source: Box<SourceSpan>,
    },
    #[error("Proof return fact inventory contains {observed} headers, maximum is {maximum}")]
    HeaderLimit { observed: usize, maximum: usize },
    #[error("Proof return semantic work accounting overflowed u64")]
    WorkOverflow,
}

impl HirProofReturnHeader {
    pub fn try_new(
        generation: Arc<HirProofReturnProjectGeneration>,
        module: CanonicalModulePath,
        item: ItemId,
        return_type: TypeId,
        source: SourceSpan,
    ) -> Result<Self, HirProofReturnAuthorityError> {
        let lease = generation.module(&module).ok_or_else(|| {
            HirProofReturnAuthorityError::UnknownModule {
                module: module.clone(),
            }
        })?;
        let expected_module = lease.hir_snapshot().module();
        if item.module() != expected_module || return_type.module() != expected_module {
            return Err(HirProofReturnAuthorityError::WrongHirOwner {
                expected: expected_module,
                item: item.module(),
                return_type: return_type.module(),
            });
        }
        if source.source() != lease.source() {
            return Err(HirProofReturnAuthorityError::WrongSource {
                expected: Box::new(lease.source().clone()),
                actual: Box::new(source.source().clone()),
            });
        }
        Ok(Self {
            generation,
            module,
            item,
            return_type,
            source,
        })
    }

    pub const fn generation(&self) -> &Arc<HirProofReturnProjectGeneration> {
        &self.generation
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn item(&self) -> ItemId {
        self.item
    }

    pub const fn return_type(&self) -> TypeId {
        self.return_type
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    fn same_binding(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.generation, &other.generation)
            && self.module == other.module
            && self.item == other.item
            && self.return_type == other.return_type
            && self.source == other.source
    }
}

impl HirProofReturnProjectGeneration {
    /// Validates the exact unpublished module transaction before it may consume
    /// any semantic fact from this generation.
    pub fn validate_module_transaction(
        &self,
        package: &CallablePackageId,
        module: &CanonicalModulePath,
        hir_snapshot: HirSnapshotId,
        syntax_snapshot: &SyntaxSnapshotId,
        source: &SourceDocumentIdentity,
    ) -> Result<(), HirProofReturnAuthorityError> {
        let lease =
            self.module(module)
                .ok_or_else(|| HirProofReturnAuthorityError::UnknownModule {
                    module: module.clone(),
                })?;
        if lease.package() != package {
            return Err(HirProofReturnAuthorityError::WrongPackage {
                expected: lease.package().clone(),
                actual: package.clone(),
            });
        }
        if lease.hir_snapshot() != hir_snapshot {
            return Err(HirProofReturnAuthorityError::WrongHirSnapshot {
                expected: lease.hir_snapshot(),
                actual: hir_snapshot,
            });
        }
        if lease.syntax_snapshot() != syntax_snapshot {
            return Err(HirProofReturnAuthorityError::WrongSyntaxSnapshot {
                expected: Box::new(lease.syntax_snapshot().clone()),
                actual: Box::new(syntax_snapshot.clone()),
            });
        }
        if lease.source() != source {
            return Err(HirProofReturnAuthorityError::WrongSource {
                expected: Box::new(lease.source().clone()),
                actual: Box::new(source.clone()),
            });
        }
        Ok(())
    }
}

/// Sema-produced class bound to one exact staged Proof return header.
#[derive(Clone, Debug)]
pub struct HirProofReturnSemanticFact {
    header: HirProofReturnHeader,
    class: HirProofReturnSemanticClass,
    resolution_work: u64,
}

impl HirProofReturnSemanticFact {
    pub fn new(
        header: HirProofReturnHeader,
        class: HirProofReturnSemanticClass,
        resolution_work: u64,
    ) -> Self {
        Self {
            header,
            class,
            resolution_work,
        }
    }

    pub const fn header(&self) -> &HirProofReturnHeader {
        &self.header
    }

    pub const fn class(&self) -> HirProofReturnSemanticClass {
        self.class
    }

    /// Work charged by the sole nominal resolver for this return root.
    pub const fn resolution_work(&self) -> u64 {
        self.resolution_work
    }
}

/// Complete semantic fact authority installed before Proof body allocation.
#[derive(Debug)]
pub struct HirProofReturnSemanticFactSet {
    generation: Arc<HirProofReturnProjectGeneration>,
    headers: BTreeMap<ItemId, HirProofReturnHeader>,
    facts: BTreeMap<ItemId, HirProofReturnSemanticFact>,
    validation_work: u64,
    semantic_work: u64,
}

/// Opaque, unpublished multi-module HIR transaction paused after every
/// authored Proof return header has an exact typed identity and before any
/// changed Proof body has allocated a scope, statement, expression, or tail.
/// Exact current cache-hit modules may be retained in the same generation;
/// they are never cloned, rebased, or reconstructed.
pub struct HirProofReturnProjectTransaction<'source> {
    pub(crate) control: crate::lowering::HirLoweringControl,
    pub(crate) generation: Arc<HirProofReturnProjectGeneration>,
    pub(crate) headers: Box<[HirProofReturnHeader]>,
    pub(crate) modules: Vec<crate::final_lowering::ProofReturnProjectModuleTransaction<'source>>,
}

/// Read-only projection over the exact paused project transaction. It borrows
/// the sole mutable owner and therefore cannot outlive, clone, publish, or
/// mutate the staged HIR generation.
#[derive(Clone, Copy)]
pub struct HirProofReturnHeaderProjectView<'transaction, 'source> {
    pub(crate) modules:
        &'transaction [crate::final_lowering::ProofReturnProjectModuleTransaction<'source>],
}

#[derive(Clone, Copy)]
pub struct HirProofReturnHeaderModuleView<'transaction, 'source> {
    pub(crate) module:
        &'transaction crate::final_lowering::ProofReturnProjectModuleTransaction<'source>,
}

#[derive(Clone, Copy)]
pub struct HirProofReturnHeaderItemRef<'transaction, 'source> {
    pub(crate) module: HirProofReturnHeaderModuleView<'transaction, 'source>,
    pub(crate) id: ItemId,
    pub(crate) item: &'transaction crate::item::HirItem,
}

#[derive(Clone, Copy)]
pub struct HirProofReturnCallableHeaderRef<'transaction, 'source> {
    pub(crate) module: HirProofReturnHeaderModuleView<'transaction, 'source>,
    pub(crate) header: &'transaction crate::final_lowering::StagedProofReturnHeader,
}

impl HirProofReturnProjectTransaction<'_> {
    pub const fn generation(&self) -> &Arc<HirProofReturnProjectGeneration> {
        &self.generation
    }

    pub fn headers(&self) -> impl ExactSizeIterator<Item = &HirProofReturnHeader> {
        self.headers.iter()
    }
}

impl HirProofReturnSemanticFactSet {
    pub fn try_new(
        generation: Arc<HirProofReturnProjectGeneration>,
        headers: impl IntoIterator<Item = HirProofReturnHeader>,
        facts: impl IntoIterator<Item = HirProofReturnSemanticFact>,
    ) -> Result<Arc<Self>, HirProofReturnAuthorityError> {
        let mut by_item = BTreeMap::new();
        let maximum = generation
            .modules()
            .len()
            .saturating_mul(HirLimit::Items.maximum());
        let mut validation_work = 0_u64;
        for header in headers {
            let observed = by_item.len().saturating_add(1);
            if observed > maximum {
                return Err(HirProofReturnAuthorityError::HeaderLimit { observed, maximum });
            }
            validation_work = validation_work
                .checked_add(1)
                .ok_or(HirProofReturnAuthorityError::WorkOverflow)?;
            if !Arc::ptr_eq(header.generation(), &generation) {
                return Err(HirProofReturnAuthorityError::ForeignGeneration);
            }
            let item = header.item();
            if by_item.insert(item, header).is_some() {
                return Err(HirProofReturnAuthorityError::DuplicateHeader { item });
            }
        }

        let mut facts_by_item = BTreeMap::new();
        let mut semantic_work = 0_u64;
        for fact in facts {
            validation_work = validation_work
                .checked_add(1)
                .ok_or(HirProofReturnAuthorityError::WorkOverflow)?;
            semantic_work = semantic_work
                .checked_add(fact.resolution_work())
                .ok_or(HirProofReturnAuthorityError::WorkOverflow)?;
            if !Arc::ptr_eq(fact.header().generation(), &generation) {
                return Err(HirProofReturnAuthorityError::ForeignGeneration);
            }
            let item = fact.header().item();
            let Some(header) = by_item.get(&item) else {
                return Err(HirProofReturnAuthorityError::UnexpectedFact { item });
            };
            if !header.same_binding(fact.header()) {
                return Err(HirProofReturnAuthorityError::FactBindingMismatch {
                    item,
                    expected_type: header.return_type(),
                    actual_type: fact.header().return_type(),
                    expected_source: Box::new(header.source().clone()),
                    actual_source: Box::new(fact.header().source().clone()),
                });
            }
            if facts_by_item.insert(item, fact).is_some() {
                return Err(HirProofReturnAuthorityError::DuplicateFact { item });
            }
        }
        for item in by_item.keys().copied() {
            validation_work = validation_work
                .checked_add(1)
                .ok_or(HirProofReturnAuthorityError::WorkOverflow)?;
            if !facts_by_item.contains_key(&item) {
                return Err(HirProofReturnAuthorityError::MissingFact { item });
            }
        }

        Ok(Arc::new(Self {
            generation,
            headers: by_item,
            facts: facts_by_item,
            validation_work,
            semantic_work,
        }))
    }

    pub const fn generation(&self) -> &Arc<HirProofReturnProjectGeneration> {
        &self.generation
    }

    pub fn headers(&self) -> impl ExactSizeIterator<Item = &HirProofReturnHeader> {
        self.headers.values()
    }

    /// Deterministic header/fact/completeness rows validated at construction.
    pub const fn validation_work(&self) -> u64 {
        self.validation_work
    }

    /// Aggregate work already charged by semantic nominal resolution.
    pub const fn semantic_work(&self) -> u64 {
        self.semantic_work
    }

    pub fn class_for(
        &self,
        header: &HirProofReturnHeader,
    ) -> Result<HirProofReturnSemanticClass, HirProofReturnAuthorityError> {
        if !Arc::ptr_eq(header.generation(), &self.generation) {
            return Err(HirProofReturnAuthorityError::ForeignGeneration);
        }
        let expected = self.headers.get(&header.item()).ok_or(
            HirProofReturnAuthorityError::UnexpectedFact {
                item: header.item(),
            },
        )?;
        if !expected.same_binding(header) {
            return Err(HirProofReturnAuthorityError::FactBindingMismatch {
                item: header.item(),
                expected_type: expected.return_type(),
                actual_type: header.return_type(),
                expected_source: Box::new(expected.source().clone()),
                actual_source: Box::new(header.source().clone()),
            });
        }
        self.facts
            .get(&header.item())
            .map(HirProofReturnSemanticFact::class)
            .ok_or(HirProofReturnAuthorityError::MissingFact {
                item: header.item(),
            })
    }
}

#[cfg(test)]
mod tests;
