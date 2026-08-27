//! Module-preserving project owner for the final arena HIR.

use std::sync::Arc;
use std::{collections::BTreeMap, fmt};

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::{SourceDocumentId, SourceDocumentIdentity};
use thiserror::Error;

use crate::database::HirDatabase;
use crate::identity::{HirDatabaseId, HirLimit, HirSnapshotId, ItemId};
use crate::item::{
    HirDeclarationMemberId, HirDeclarationMemberKind, HirItem, HirItemKind, HirStyleItem,
    HirViewExportMember,
};
use crate::module::{HirModule, HirModuleStatus};
use crate::symbol::{
    CallablePackageId, ProjectSymbolRevision, ProjectSymbolTable, ProjectSymbolWorldId,
};

#[path = "final_project/dialogue_lines.rs"]
mod dialogue_lines;
#[path = "final_project/runtime_semantic_owners.rs"]
mod runtime_semantic_owners;
#[path = "final_project/selected_expressions.rs"]
mod selected_expressions;
#[path = "final_project/semantic_paths.rs"]
mod semantic_paths;

pub use self::dialogue_lines::{
    AcceptedDialogueLine, AcceptedDialogueLineInventory, AcceptedDialogueLineSource,
    DialogueLineIndex, DialogueLineProjectFatal, DialogueLineProjectRejection,
};
pub use self::runtime_semantic_owners::{
    HirRuntimeEmissionMode, HirRuntimeExecutableOwner, HirRuntimeIteratorWitnessMethodRole,
    HirRuntimeReachabilityDigest, HirRuntimeReachabilityEdge, HirRuntimeReachabilityEdgeKind,
    HirRuntimeReachabilityError, HirRuntimeReachabilityIdentity, HirRuntimeReachabilityLimitFamily,
    HirRuntimeReachabilityPath, HirRuntimeReachabilityRoot, HirRuntimeReachabilityRootKind,
    HirRuntimeReachabilitySite, HirRuntimeSemanticReachability,
    HirRuntimeSemanticReachabilityInput,
};
pub use self::selected_expressions::{
    HirRuntimeCallCalleeDisposition, HirRuntimeExpressionTypeDisposition,
    HirSelectedCallExpressionDisposition, HirSelectedCallExpressionInventory,
    HirSelectedExpressionGraph, HirSelectedExpressionInventoryError,
};
pub use self::semantic_paths::{
    HirAcceptedItemFamily, HirBindingSite, HirCaptureEvaluationIndex, HirCaptureEvaluationRow,
    HirDeclarationBodyRoot, HirDeclarationBodyRootChild, HirDeclarationBodyRootRole,
    HirDeclarationBodyTopology, HirDeclarationContractRoot, HirDeclarationContractRootRole,
    HirDeclarationEvaluationPhase, HirDeclarationEvaluationView, HirDeclarationItemRootRole,
    HirDeclarationParameterRoot, HirDeclarationParameterRootChild, HirDeclarationParameterRootRole,
    HirExpressionBindingRole, HirExpressionCallableBoundary, HirExpressionEvaluationEdge,
    HirExpressionSemanticHop, HirExpressionUseIndex, HirExpressionUseRow,
    HirFlowContractRootFamily, HirImplicitCallableRegion, HirItemAttributeOwner,
    HirItemEvaluationEntry, HirItemEvaluationEntryRole, HirItemEvaluationRoot,
    HirItemRecoveryRootOwner, HirLayerExpressionRootField, HirLocalBindingOrigin,
    HirLocalBindingOriginIndex, HirLocalBindingStatementRole, HirLocalValueOrigin,
    HirMemberBindingRole, HirModuleEvaluationTopology, HirProjectEvaluationTopology,
    HirSemanticOwnerPath, HirSemanticPathError, HirSemanticPathIndex, HirSemanticPathLookupError,
    HirSemanticPathRoot, HirSemanticPathStep, HirStyleRootPath, HirStyleRootPathSegment,
};

/// Package-qualified canonical key for one project module.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirPackageModuleKey {
    package: CallablePackageId,
    path: CanonicalModulePath,
}

impl HirPackageModuleKey {
    pub(crate) fn new(package: CallablePackageId, path: CanonicalModulePath) -> Self {
        Self { package, path }
    }

    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }

    pub const fn path(&self) -> &CanonicalModulePath {
        &self.path
    }
}

impl From<&crate::lowering::HirModuleKey> for HirPackageModuleKey {
    fn from(key: &crate::lowering::HirModuleKey) -> Self {
        Self::new(key.package().clone(), key.path().clone())
    }
}

/// One exact current module lease admitted to a final-HIR project.
#[derive(Clone)]
pub struct HirProjectModule {
    module: Arc<HirModule>,
}

/// Failed binding of an expected package/path/source to an accepted HIR lease.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirProjectModuleError {
    #[error("HIR module package mismatch: expected {expected:?}, got {actual:?}")]
    WrongPackage {
        expected: CallablePackageId,
        actual: CallablePackageId,
    },
    #[error("HIR module path mismatch: expected {expected}, got {actual}")]
    WrongPath {
        expected: CanonicalModulePath,
        actual: CanonicalModulePath,
    },
    #[error("HIR module source mismatch for `{module}`")]
    WrongSource {
        module: CanonicalModulePath,
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    #[error("HIR module belongs to database {actual:?}, expected {expected:?}")]
    WrongDatabase {
        expected: HirDatabaseId,
        actual: HirDatabaseId,
    },
    #[error("HIR database has no accepted module for `{module}`")]
    MissingAcceptedModule { module: CanonicalModulePath },
    #[error(
        "HIR module `{module}` uses stale snapshot {supplied:?}; current snapshot is {current:?}"
    )]
    StaleModuleLease {
        module: CanonicalModulePath,
        current: HirSnapshotId,
        supplied: HirSnapshotId,
    },
}

impl HirProjectModule {
    /// Checks and retains the exact current `Arc<HirModule>` from one database.
    #[allow(
        clippy::result_large_err,
        reason = "project lease rejection preserves complete typed package, path, source, and snapshot evidence"
    )]
    pub fn try_new(
        database: &HirDatabase,
        expected_package: &CallablePackageId,
        expected_path: &CanonicalModulePath,
        expected_source: &SourceDocumentIdentity,
        module: Arc<HirModule>,
    ) -> Result<Self, HirProjectModuleError> {
        let actual_database = module.module_id().database();
        if actual_database != database.database_id() {
            return Err(HirProjectModuleError::WrongDatabase {
                expected: database.database_id(),
                actual: actual_database,
            });
        }
        if module.key().package() != expected_package {
            return Err(HirProjectModuleError::WrongPackage {
                expected: expected_package.clone(),
                actual: module.key().package().clone(),
            });
        }
        if module.key().path() != expected_path {
            return Err(HirProjectModuleError::WrongPath {
                expected: expected_path.clone(),
                actual: module.key().path().clone(),
            });
        }
        let actual_source = module.provenance().source_identity();
        if actual_source != expected_source {
            return Err(HirProjectModuleError::WrongSource {
                module: expected_path.clone(),
                expected: expected_source.clone(),
                actual: actual_source.clone(),
            });
        }

        let Some(current) = database.current_lineage(module.key()) else {
            return Err(HirProjectModuleError::MissingAcceptedModule {
                module: expected_path.clone(),
            });
        };
        if !Arc::ptr_eq(&current, &module) {
            return Err(HirProjectModuleError::StaleModuleLease {
                module: expected_path.clone(),
                current: current.snapshot_id(),
                supplied: module.snapshot_id(),
            });
        }
        Ok(Self { module })
    }

    pub fn package(&self) -> &CallablePackageId {
        self.module.key().package()
    }

    pub fn path(&self) -> &CanonicalModulePath {
        self.module.key().path()
    }

    pub fn source(&self) -> &SourceDocumentIdentity {
        self.module.provenance().source_identity()
    }

    pub const fn module(&self) -> &Arc<HirModule> {
        &self.module
    }

    pub fn key(&self) -> HirPackageModuleKey {
        HirPackageModuleKey::from(self.module.key())
    }
}

/// Immutable package project that preserves every module-local HIR identity.
pub struct HirProject {
    root_package: CallablePackageId,
    database: HirDatabaseId,
    modules: BTreeMap<HirPackageModuleKey, HirProjectModule>,
    dialogue_lines: AcceptedDialogueLineInventory,
}

/// Invalid final-HIR project generation.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirProjectBuildError {
    #[error("HIR project contains {observed} modules, maximum is {maximum}")]
    ModuleLimit { observed: usize, maximum: usize },
    #[error("HIR project contains duplicate module {key:?}")]
    DuplicateModule { key: HirPackageModuleKey },
    #[error("HIR project maps source document {document:?} to both `{first}` and `{second}`")]
    DuplicateSourceDocument {
        document: SourceDocumentId,
        first: CanonicalModulePath,
        second: CanonicalModulePath,
    },
    #[error("HIR project does not contain the crate root module for package {package:?}")]
    MissingRootModule { package: CallablePackageId },
    #[error("HIR module {key:?} belongs to package {actual:?}, expected {expected:?}")]
    ModulePackageMismatch {
        key: HirPackageModuleKey,
        expected: CallablePackageId,
        actual: CallablePackageId,
    },
    #[error("HIR module `{module}` belongs to database {actual:?}, expected {expected:?}")]
    WrongDatabase {
        module: CanonicalModulePath,
        expected: HirDatabaseId,
        actual: HirDatabaseId,
    },
    #[error("HIR database has no currently accepted module for `{module}`")]
    MissingAcceptedModule { module: CanonicalModulePath },
    #[error(
        "HIR project module `{module}` uses stale snapshot {supplied:?}; current snapshot is {current:?}"
    )]
    StaleModuleLease {
        module: CanonicalModulePath,
        current: HirSnapshotId,
        supplied: HirSnapshotId,
    },
    #[error(transparent)]
    DialogueLines(#[from] DialogueLineProjectRejection),
    #[error(transparent)]
    DialogueLineFatal(#[from] DialogueLineProjectFatal),
}

/// Sole transactional builder for one package-qualified final-HIR project.
pub struct HirProjectBuilder<'database> {
    database: &'database HirDatabase,
    root_package: CallablePackageId,
    modules: BTreeMap<HirPackageModuleKey, HirProjectModule>,
    maximum_modules: usize,
}

impl<'database> HirProjectBuilder<'database> {
    pub fn new(database: &'database HirDatabase, root_package: CallablePackageId) -> Self {
        Self {
            database,
            root_package,
            modules: BTreeMap::new(),
            maximum_modules: HirLimit::ModulesPerDatabase.maximum(),
        }
    }

    pub fn insert_module(&mut self, module: HirProjectModule) -> Result<(), HirProjectBuildError> {
        let observed =
            self.modules
                .len()
                .checked_add(1)
                .ok_or(HirProjectBuildError::ModuleLimit {
                    observed: usize::MAX,
                    maximum: self.maximum_modules,
                })?;
        if observed > self.maximum_modules {
            return Err(HirProjectBuildError::ModuleLimit {
                observed,
                maximum: self.maximum_modules,
            });
        }
        let key = module.key();
        if self.modules.contains_key(&key) {
            return Err(HirProjectBuildError::DuplicateModule { key });
        }
        self.modules.insert(key, module);
        Ok(())
    }

    pub fn finish(self) -> Result<HirProject, HirProjectBuildError> {
        if let Some((key, _)) = self
            .modules
            .iter()
            .find(|(key, _)| key.package() != &self.root_package)
        {
            return Err(HirProjectBuildError::ModulePackageMismatch {
                key: key.clone(),
                expected: self.root_package,
                actual: key.package().clone(),
            });
        }
        let root_key =
            HirPackageModuleKey::new(self.root_package.clone(), CanonicalModulePath::crate_root());
        if !self.modules.contains_key(&root_key) {
            return Err(HirProjectBuildError::MissingRootModule {
                package: self.root_package,
            });
        }
        let database_id = self.database.database_id();

        let mut source_paths = BTreeMap::new();
        for (key, module) in &self.modules {
            let actual_database = module.module().module_id().database();
            if actual_database != database_id {
                return Err(HirProjectBuildError::WrongDatabase {
                    module: key.path().clone(),
                    expected: database_id,
                    actual: actual_database,
                });
            }
            let Some(current) = self.database.current_lineage(module.module().key()) else {
                return Err(HirProjectBuildError::MissingAcceptedModule {
                    module: key.path().clone(),
                });
            };
            if !Arc::ptr_eq(&current, module.module()) {
                return Err(HirProjectBuildError::StaleModuleLease {
                    module: key.path().clone(),
                    current: current.snapshot_id(),
                    supplied: module.module().snapshot_id(),
                });
            }
            let document = module.source().id().clone();
            if let Some(first) = source_paths.insert(document.clone(), key.path().clone()) {
                return Err(HirProjectBuildError::DuplicateSourceDocument {
                    document,
                    first,
                    second: key.path().clone(),
                });
            }
        }

        let dialogue_lines = dialogue_lines::accept_dialogue_lines(self.modules.values())?;
        Ok(HirProject {
            root_package: self.root_package,
            database: database_id,
            modules: self.modules,
            dialogue_lines,
        })
    }

    #[cfg(test)]
    fn with_module_limit_for_test(mut self, maximum_modules: usize) -> Self {
        self.maximum_modules = maximum_modules;
        self
    }
}

impl HirProject {
    pub const fn package(&self) -> &CallablePackageId {
        &self.root_package
    }

    pub const fn root_package(&self) -> &CallablePackageId {
        &self.root_package
    }

    pub const fn database_id(&self) -> HirDatabaseId {
        self.database
    }

    pub fn module(&self, path: &CanonicalModulePath) -> Option<&HirProjectModule> {
        self.modules.get(&HirPackageModuleKey::new(
            self.root_package.clone(),
            path.clone(),
        ))
    }

    pub fn module_by_key(&self, key: &HirPackageModuleKey) -> Option<&HirProjectModule> {
        self.modules.get(key)
    }

    pub const fn dialogue_lines(&self) -> &AcceptedDialogueLineInventory {
        &self.dialogue_lines
    }

    pub fn view(&self) -> HirProjectView<'_> {
        HirProjectView { project: self }
    }

    pub fn executable_view(
        &self,
    ) -> Result<HirExecutableProjectView<'_>, HirProjectExecutionError> {
        for (key, module) in &self.modules {
            if module.module().status() == HirModuleStatus::Recovered {
                return Err(HirProjectExecutionError::RecoveredModule {
                    module: key.path().clone(),
                    snapshot: module.module().snapshot_id(),
                });
            }
        }
        Ok(HirExecutableProjectView {
            view: HirProjectView { project: self },
        })
    }
}

/// Tooling view over clean and recovered project modules.
#[derive(Clone, Copy)]
pub struct HirProjectView<'project> {
    project: &'project HirProject,
}

impl<'project> HirProjectView<'project> {
    pub const fn package(self) -> &'project CallablePackageId {
        &self.project.root_package
    }

    pub fn modules(
        self,
    ) -> impl ExactSizeIterator<Item = (&'project CanonicalModulePath, &'project Arc<HirModule>)>
    + 'project {
        self.project
            .modules
            .iter()
            .map(|(key, module)| (key.path(), module.module()))
    }

    pub fn module(self, path: &CanonicalModulePath) -> Option<&'project Arc<HirModule>> {
        self.project.module(path).map(HirProjectModule::module)
    }

    /// Returns the dialogue-line inventory accepted by this exact project
    /// generation. Consumers borrow this authority instead of reconstructing
    /// line identities from module source.
    pub const fn dialogue_lines(self) -> &'project AcceptedDialogueLineInventory {
        self.project.dialogue_lines()
    }

    pub fn items(&self) -> impl Iterator<Item = HirProjectItemRef<'project>> + 'project {
        self.project.modules.values().flat_map(|module| {
            module
                .module()
                .source_ordered_items()
                .iter()
                .copied()
                .map(move |id| HirProjectItemRef { module, id })
        })
    }
}

/// Iterates final View export members without flattening or rebasing their
/// module-local item/member identities.
///
/// # Panics
///
/// Panics only if a previously validated project lease no longer resolves one
/// of its source-ordered View items or export members.
pub fn exported_parts(
    project: HirProjectView<'_>,
) -> impl Iterator<Item = ProjectExportedPartRef<'_>> {
    project.project.modules.values().flat_map(|module| {
        module
            .module()
            .source_ordered_items()
            .iter()
            .copied()
            .filter_map(move |item| {
                let payload = module
                    .module()
                    .resolve_item(item)
                    .expect("validated project item identity resolves in its exact module lease");
                match payload.kind() {
                    HirItemKind::View(view) => Some((item, view.exports())),
                    _ => None,
                }
            })
            .flat_map(move |(item, exports)| {
                exports.iter().copied().map(move |member| {
                    let payload = module
                        .module()
                        .declaration_members()
                        .resolve(member)
                        .expect(
                            "validated View member identity resolves in its exact module lease",
                        );
                    let HirDeclarationMemberKind::ViewExport(part) = payload.kind() else {
                        unreachable!(
                            "validated View export identity resolved to another member family"
                        )
                    };
                    ProjectExportedPartRef {
                        module,
                        item,
                        member,
                        part,
                    }
                })
            })
    })
}

/// Iterates final Style items in canonical module and authored item order.
///
/// # Panics
///
/// Panics only if a previously validated project lease no longer resolves one
/// of its source-ordered Style items.
pub fn styles(project: HirProjectView<'_>) -> impl Iterator<Item = ProjectStyleRef<'_>> {
    project.project.modules.values().flat_map(|module| {
        module
            .module()
            .source_ordered_items()
            .iter()
            .copied()
            .filter_map(move |item| {
                let payload = module
                    .module()
                    .resolve_item(item)
                    .expect("validated project item identity resolves in its exact module lease");
                match payload.kind() {
                    HirItemKind::Style(style) => Some(ProjectStyleRef {
                        module,
                        item,
                        style,
                    }),
                    _ => None,
                }
            })
    })
}

/// One module-qualified View export retained by the final declaration-member
/// arena.
#[derive(Clone, Copy)]
pub struct ProjectExportedPartRef<'project> {
    module: &'project HirProjectModule,
    item: ItemId,
    member: HirDeclarationMemberId,
    part: &'project HirViewExportMember,
}

impl<'project> ProjectExportedPartRef<'project> {
    pub fn module_path(self) -> &'project CanonicalModulePath {
        self.module.path()
    }

    pub const fn item(self) -> ItemId {
        self.item
    }

    pub const fn member(self) -> HirDeclarationMemberId {
        self.member
    }

    pub const fn part(self) -> &'project HirViewExportMember {
        self.part
    }
}

/// One module-qualified Style item retained by the final item arena.
#[derive(Clone, Copy)]
pub struct ProjectStyleRef<'project> {
    module: &'project HirProjectModule,
    item: ItemId,
    style: &'project HirStyleItem,
}

impl<'project> ProjectStyleRef<'project> {
    pub fn module_path(self) -> &'project CanonicalModulePath {
        self.module.path()
    }

    pub const fn item(self) -> ItemId {
        self.item
    }

    pub const fn style(self) -> &'project HirStyleItem {
        self.style
    }
}

/// Executable-only project view, constructible only after full status checking.
#[derive(Clone, Copy)]
pub struct HirExecutableProjectView<'project> {
    view: HirProjectView<'project>,
}

/// One immutable module generation retained by an accepted project witness.
///
/// The row is identity-only: the module arena is borrowed from the executable
/// project view and is never copied into the generation token.
pub struct AcceptedHirModuleGeneration {
    canonical_path: CanonicalModulePath,
    module: crate::identity::HirModuleId,
    snapshot: HirSnapshotId,
    source: SourceDocumentIdentity,
}

impl fmt::Debug for AcceptedHirModuleGeneration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AcceptedHirModuleGeneration")
            .field("canonical_path", &self.canonical_path)
            .field("module", &self.module)
            .field("snapshot", &self.snapshot)
            .field("source", &self.source)
            .finish()
    }
}

impl PartialEq for AcceptedHirModuleGeneration {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_path == other.canonical_path
            && self.module == other.module
            && self.snapshot == other.snapshot
            && self.source == other.source
    }
}

impl Eq for AcceptedHirModuleGeneration {}

impl AcceptedHirModuleGeneration {
    pub const fn canonical_path(&self) -> &CanonicalModulePath {
        &self.canonical_path
    }

    pub const fn module(&self) -> crate::identity::HirModuleId {
        self.module
    }

    pub const fn snapshot(&self) -> HirSnapshotId {
        self.snapshot
    }

    pub const fn source(&self) -> &SourceDocumentIdentity {
        &self.source
    }
}

/// One immutable project generation retained by an accepted project witness.
///
/// The generation owns only identity rows in canonical path order. Its package
/// is derived from the symbol world, avoiding a duplicate package authority.
pub struct AcceptedHirProjectGeneration {
    symbol_world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    modules: Box<[Arc<AcceptedHirModuleGeneration>]>,
}

impl fmt::Debug for AcceptedHirProjectGeneration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AcceptedHirProjectGeneration")
            .field("symbol_world", &self.symbol_world)
            .field("symbol_revision", &self.symbol_revision)
            .field("modules", &self.modules)
            .finish()
    }
}

impl PartialEq for AcceptedHirProjectGeneration {
    fn eq(&self, other: &Self) -> bool {
        self.symbol_world == other.symbol_world
            && self.symbol_revision == other.symbol_revision
            && self.modules == other.modules
    }
}

impl Eq for AcceptedHirProjectGeneration {}

impl AcceptedHirProjectGeneration {
    pub const fn symbol_world(&self) -> &ProjectSymbolWorldId {
        &self.symbol_world
    }

    pub const fn symbol_revision(&self) -> ProjectSymbolRevision {
        self.symbol_revision
    }

    pub const fn package(&self) -> &CallablePackageId {
        self.symbol_world.package()
    }

    pub fn modules(&self) -> &[Arc<AcceptedHirModuleGeneration>] {
        &self.modules
    }

    pub fn module(&self, path: &CanonicalModulePath) -> Option<&Arc<AcceptedHirModuleGeneration>> {
        self.modules
            .binary_search_by(|value| value.canonical_path().cmp(path))
            .ok()
            .map(|index| &self.modules[index])
    }

    pub fn same_generation(&self, other: &Self) -> bool {
        self == other
    }

    /// Validates that this accepted generation is an exact lease for the
    /// supplied executable project view.
    ///
    /// Symbol identity is validated by the admission that minted this
    /// generation. This check closes the remaining project-owned portion of
    /// the lease without reconstructing a second generation token.
    pub fn validate_executable_lease(
        &self,
        project: HirExecutableProjectView<'_>,
    ) -> Result<(), AcceptedHirProjectLeaseError> {
        if self.package() != project.package() {
            return Err(AcceptedHirProjectLeaseError::PackageMismatch);
        }
        let mut project_modules = project.modules();
        let mut accepted_modules = self.modules.iter();
        loop {
            match (project_modules.next(), accepted_modules.next()) {
                (Some((project_path, module)), Some(accepted)) => {
                    match project_path.cmp(accepted.canonical_path()) {
                        std::cmp::Ordering::Less => {
                            return Err(AcceptedHirProjectLeaseError::MissingAcceptedModule {
                                module: project_path.clone(),
                            });
                        }
                        std::cmp::Ordering::Greater => {
                            return Err(AcceptedHirProjectLeaseError::ExtraAcceptedModule {
                                module: accepted.canonical_path().clone(),
                            });
                        }
                        std::cmp::Ordering::Equal => {}
                    }
                    if accepted.module() != module.module_id() {
                        return Err(AcceptedHirProjectLeaseError::ModuleMismatch {
                            module: project_path.clone(),
                        });
                    }
                    if accepted.snapshot() != module.snapshot_id() {
                        return Err(AcceptedHirProjectLeaseError::SnapshotMismatch {
                            module: project_path.clone(),
                        });
                    }
                    if accepted.source() != module.provenance().source_identity() {
                        return Err(AcceptedHirProjectLeaseError::SourceMismatch {
                            module: project_path.clone(),
                        });
                    }
                }
                (Some((project_path, _)), None) => {
                    return Err(AcceptedHirProjectLeaseError::MissingAcceptedModule {
                        module: project_path.clone(),
                    });
                }
                (None, Some(accepted)) => {
                    return Err(AcceptedHirProjectLeaseError::ExtraAcceptedModule {
                        module: accepted.canonical_path().clone(),
                    });
                }
                (None, None) => return Ok(()),
            }
        }
    }

    pub fn validate_module_lease(
        &self,
        module: &HirModule,
        symbols: &ProjectSymbolTable,
    ) -> Result<(), AcceptedHirModuleLeaseError> {
        if self.symbol_world != *symbols.world() {
            return Err(AcceptedHirModuleLeaseError::WorldMismatch);
        }
        if self.symbol_revision != *symbols.revision() {
            return Err(AcceptedHirModuleLeaseError::RevisionMismatch);
        }
        if self.symbol_world.package() != module.key().package() {
            return Err(AcceptedHirModuleLeaseError::PackageMismatch);
        }
        let Some(row) = self.module(module.key().path()) else {
            return Err(AcceptedHirModuleLeaseError::MissingModule);
        };
        if row.module() != module.module_id() {
            return Err(AcceptedHirModuleLeaseError::ModuleMismatch);
        }
        if row.snapshot() != module.snapshot_id() {
            return Err(AcceptedHirModuleLeaseError::SnapshotMismatch);
        }
        if row.source() != module.provenance().source_identity()
            || symbols.source_identity(module.key().path()) != Some(row.source())
        {
            return Err(AcceptedHirModuleLeaseError::SourceMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AcceptedHirProjectLeaseError {
    #[error("accepted HIR generation and executable project have different packages")]
    PackageMismatch,
    #[error("executable HIR module is absent from the accepted generation: {module}")]
    MissingAcceptedModule { module: CanonicalModulePath },
    #[error("accepted generation contains a module absent from executable HIR: {module}")]
    ExtraAcceptedModule { module: CanonicalModulePath },
    #[error("accepted generation module identity differs for `{module}`")]
    ModuleMismatch { module: CanonicalModulePath },
    #[error("accepted generation module snapshot differs for `{module}`")]
    SnapshotMismatch { module: CanonicalModulePath },
    #[error("accepted generation module source identity differs for `{module}`")]
    SourceMismatch { module: CanonicalModulePath },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AcceptedHirModuleLeaseError {
    #[error("accepted HIR project generation symbol world differs")]
    WorldMismatch,
    #[error("accepted HIR project generation symbol revision differs")]
    RevisionMismatch,
    #[error("accepted HIR project/module packages differ")]
    PackageMismatch,
    #[error("module is absent from the accepted project generation")]
    MissingModule,
    #[error("module identity differs from the accepted project generation")]
    ModuleMismatch,
    #[error("module snapshot differs from the accepted project generation")]
    SnapshotMismatch,
    #[error("module source identity differs from the accepted project generation")]
    SourceMismatch,
}

/// Exact symbol-generation witness for one executable HIR project.
///
/// The witness is move-only so consumers cannot retain an independently
/// reconstructed package/module/source join.  It is issued only after the
/// symbol table has been checked against every executable module lease.
pub struct AcceptedHirProjectSymbolGeneration<'project, 'symbols> {
    project: HirExecutableProjectView<'project>,
    symbols: &'symbols ProjectSymbolTable,
    generation: Arc<AcceptedHirProjectGeneration>,
}

impl<'project, 'symbols> AcceptedHirProjectSymbolGeneration<'project, 'symbols> {
    pub const fn project(&self) -> HirExecutableProjectView<'project> {
        self.project
    }

    pub const fn symbols(&self) -> &'symbols ProjectSymbolTable {
        self.symbols
    }

    pub fn generation(&self) -> &Arc<AcceptedHirProjectGeneration> {
        &self.generation
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AcceptedHirProjectSymbolGenerationError {
    #[error("HIR and symbol generations have different packages")]
    PackageMismatch,
    #[error("symbol generation contains a module absent from HIR: {module}")]
    ExtraSymbolModule { module: CanonicalModulePath },
    #[error("HIR module is absent from the symbol generation: {module}")]
    MissingSymbolModule { module: CanonicalModulePath },
    #[error("symbol generation source identity does not match HIR module `{module}`")]
    SourceIdentityMismatch { module: CanonicalModulePath },
}

impl<'project> HirExecutableProjectView<'project> {
    /// Mints the sole exact symbol-generation witness for this executable
    /// project.  Every accepted HIR module and every symbol module must join
    /// by canonical path and source-document identity.
    pub fn accept_symbol_generation<'symbols>(
        self,
        symbols: &'symbols ProjectSymbolTable,
    ) -> Result<
        AcceptedHirProjectSymbolGeneration<'project, 'symbols>,
        AcceptedHirProjectSymbolGenerationError,
    > {
        if symbols.world().package() != self.package() {
            return Err(AcceptedHirProjectSymbolGenerationError::PackageMismatch);
        }
        let mut project_modules = self.modules();
        let mut symbol_modules = symbols.modules();
        let mut accepted_modules = Vec::new();
        loop {
            match (project_modules.next(), symbol_modules.next()) {
                (Some((project_path, module)), Some(symbol_path)) => match project_path
                    .cmp(symbol_path)
                {
                    std::cmp::Ordering::Less => {
                        return Err(
                            AcceptedHirProjectSymbolGenerationError::MissingSymbolModule {
                                module: project_path.clone(),
                            },
                        );
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(AcceptedHirProjectSymbolGenerationError::ExtraSymbolModule {
                            module: symbol_path.clone(),
                        });
                    }
                    std::cmp::Ordering::Equal => {
                        if symbols.source_identity(project_path)
                            != Some(module.provenance().source_identity())
                        {
                            return Err(
                                AcceptedHirProjectSymbolGenerationError::SourceIdentityMismatch {
                                    module: project_path.clone(),
                                },
                            );
                        }
                        accepted_modules.push(Arc::new(AcceptedHirModuleGeneration {
                            canonical_path: project_path.clone(),
                            module: module.module_id(),
                            snapshot: module.snapshot_id(),
                            source: module.provenance().source_identity().clone(),
                        }));
                    }
                },
                (Some((project_path, _)), None) => {
                    return Err(
                        AcceptedHirProjectSymbolGenerationError::MissingSymbolModule {
                            module: project_path.clone(),
                        },
                    );
                }
                (None, Some(symbol_path)) => {
                    return Err(AcceptedHirProjectSymbolGenerationError::ExtraSymbolModule {
                        module: symbol_path.clone(),
                    });
                }
                (None, None) => break,
            }
        }
        let generation = Arc::new(AcceptedHirProjectGeneration {
            symbol_world: symbols.world().clone(),
            symbol_revision: *symbols.revision(),
            modules: accepted_modules.into_boxed_slice(),
        });
        Ok(AcceptedHirProjectSymbolGeneration {
            project: self,
            symbols,
            generation,
        })
    }

    /// Returns the exact tooling-capable view embedded by this executable
    /// admission without reopening or reconstructing the accepted project.
    pub const fn project_view(self) -> HirProjectView<'project> {
        self.view
    }

    /// Exact package identity admitted by this executable project generation.
    pub const fn package(self) -> &'project CallablePackageId {
        self.view.package()
    }

    pub fn modules(
        self,
    ) -> impl ExactSizeIterator<Item = (&'project CanonicalModulePath, &'project Arc<HirModule>)>
    + 'project {
        self.view.modules()
    }

    /// Resolves one exact executable module lease without reopening the
    /// tooling-capable project owner or reconstructing a module from its path.
    pub fn module(self, path: &CanonicalModulePath) -> Option<&'project Arc<HirModule>> {
        self.view.module(path)
    }

    /// Returns the line inventory owned by the admitted project generation.
    pub const fn dialogue_lines(self) -> &'project AcceptedDialogueLineInventory {
        self.view.dialogue_lines()
    }

    pub fn items(&self) -> impl Iterator<Item = HirProjectItemRef<'project>> + 'project {
        self.view.items()
    }
}

/// One module-qualified item identity in deterministic project iteration.
#[derive(Clone, Copy)]
pub struct HirProjectItemRef<'project> {
    module: &'project HirProjectModule,
    id: ItemId,
}

impl<'project> HirProjectItemRef<'project> {
    pub fn module_path(self) -> &'project CanonicalModulePath {
        self.module.path()
    }

    pub const fn id(self) -> ItemId {
        self.id
    }

    /// Exact accepted module lease that qualifies this item identity.
    pub const fn module(self) -> &'project Arc<HirModule> {
        self.module.module()
    }

    /// Resolves this qualified item from its exact accepted module lease.
    ///
    /// # Panics
    ///
    /// Panics only if the validated project item identity no longer resolves
    /// in the accepted module lease.
    pub fn item(self) -> &'project HirItem {
        self.module
            .module()
            .resolve_item(self.id)
            .expect("validated project item identity resolves in its exact module lease")
    }
}

/// Recovered modules remain visible to tooling but cannot enter execution.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirProjectExecutionError {
    #[error("HIR module `{module}` at {snapshot:?} is recovered and not executable")]
    RecoveredModule {
        module: CanonicalModulePath,
        snapshot: HirSnapshotId,
    },
}

#[cfg(test)]
#[path = "final_project/tests.rs"]
mod tests;
