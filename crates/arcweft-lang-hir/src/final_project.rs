//! Module-preserving project owner for the final arena HIR.

use std::collections::BTreeMap;
use std::sync::Arc;

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
use crate::symbol::CallablePackageId;

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
}

/// Immutable package project that preserves every module-local HIR identity.
pub struct HirProject {
    package: CallablePackageId,
    database: HirDatabaseId,
    modules: BTreeMap<CanonicalModulePath, HirProjectModule>,
}

/// Invalid final-HIR project generation.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirProjectError {
    #[error("HIR project contains {observed} modules, maximum is {maximum}")]
    ModuleLimit { observed: usize, maximum: usize },
    #[error("HIR project contains duplicate module `{module}`")]
    DuplicateModule { module: CanonicalModulePath },
    #[error("HIR project maps source document {document:?} to both `{first}` and `{second}`")]
    DuplicateSourceDocument {
        document: SourceDocumentId,
        first: CanonicalModulePath,
        second: CanonicalModulePath,
    },
    #[error("HIR project does not contain the crate root module")]
    MissingRootModule,
    #[error("HIR module `{module}` belongs to package {actual:?}, expected {expected:?}")]
    WrongPackage {
        module: CanonicalModulePath,
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
}

impl HirProject {
    pub fn try_new(
        database: &HirDatabase,
        package: CallablePackageId,
        modules: impl IntoIterator<Item = HirProjectModule>,
    ) -> Result<Self, HirProjectError> {
        Self::try_new_with_limit(
            database,
            package,
            modules,
            HirLimit::ModulesPerDatabase.maximum(),
        )
    }

    fn try_new_with_limit(
        database: &HirDatabase,
        package: CallablePackageId,
        modules: impl IntoIterator<Item = HirProjectModule>,
        maximum: usize,
    ) -> Result<Self, HirProjectError> {
        let mut module_map = BTreeMap::new();
        for module in modules {
            let observed = module_map.len().saturating_add(1);
            if observed > maximum {
                return Err(HirProjectError::ModuleLimit { observed, maximum });
            }
            let path = module.path().clone();
            if module_map.insert(path.clone(), module).is_some() {
                return Err(HirProjectError::DuplicateModule { module: path });
            }
        }

        let root_path = CanonicalModulePath::crate_root();
        if !module_map.contains_key(&root_path) {
            return Err(HirProjectError::MissingRootModule);
        }
        let database_id = database.database_id();

        let mut source_paths = BTreeMap::new();
        for (path, module) in &module_map {
            if module.package() != &package {
                return Err(HirProjectError::WrongPackage {
                    module: path.clone(),
                    expected: package.clone(),
                    actual: module.package().clone(),
                });
            }
            let actual_database = module.module().module_id().database();
            if actual_database != database_id {
                return Err(HirProjectError::WrongDatabase {
                    module: path.clone(),
                    expected: database_id,
                    actual: actual_database,
                });
            }
            let Some(current) = database.current_lineage(module.module().key()) else {
                return Err(HirProjectError::MissingAcceptedModule {
                    module: path.clone(),
                });
            };
            if !Arc::ptr_eq(&current, module.module()) {
                return Err(HirProjectError::StaleModuleLease {
                    module: path.clone(),
                    current: current.snapshot_id(),
                    supplied: module.module().snapshot_id(),
                });
            }
            let document = module.source().id().clone();
            if let Some(first) = source_paths.insert(document.clone(), path.clone()) {
                return Err(HirProjectError::DuplicateSourceDocument {
                    document,
                    first,
                    second: path.clone(),
                });
            }
        }

        Ok(Self {
            package,
            database: database_id,
            modules: module_map,
        })
    }

    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }

    pub const fn database_id(&self) -> HirDatabaseId {
        self.database
    }

    pub fn module(&self, path: &CanonicalModulePath) -> Option<&HirProjectModule> {
        self.modules.get(path)
    }

    pub fn view(&self) -> HirProjectView<'_> {
        HirProjectView { project: self }
    }

    pub fn executable_view(
        &self,
    ) -> Result<HirExecutableProjectView<'_>, HirProjectExecutionError> {
        for (path, module) in &self.modules {
            if module.module().status() == HirModuleStatus::Recovered {
                return Err(HirProjectExecutionError::RecoveredModule {
                    module: path.clone(),
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
        &self.project.package
    }

    pub fn modules(
        self,
    ) -> impl ExactSizeIterator<Item = (&'project CanonicalModulePath, &'project Arc<HirModule>)>
    + 'project {
        self.project
            .modules
            .iter()
            .map(|(path, module)| (path, module.module()))
    }

    pub fn module(self, path: &CanonicalModulePath) -> Option<&'project Arc<HirModule>> {
        self.project.modules.get(path).map(HirProjectModule::module)
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

impl<'project> HirExecutableProjectView<'project> {
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
