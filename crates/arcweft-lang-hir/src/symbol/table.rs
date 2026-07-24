//! Unified project-symbol table and deterministic fixed-point linker.
//!
//! Binding insertion, ambiguity resolution, work charging, and bounded link
//! reporting stay together because they share one monotone transaction and its
//! ordering invariants. The module remains below the production warning gate.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::ast::{
    common::{TextRange, UseItem, UseTreeKind, Visibility},
    module_path::{CanonicalModulePath, ModulePathError, ModulePathRoot, ModuleSegment},
    symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
};
use arcweft_lang_syntax::types::TypePath;
use arcweft_source::{SourceDocumentIdentity, SourceSpan};

use crate::project::HirProject;

use super::nominal::{ProjectNominalDeclaration, ProjectNominalDeclarationId};
use super::{
    CallableDeclarationId, CallableSymbol, ExternalDeclarationId, ExternalDeclarationSeedId,
    ExternalSymbol, ProjectDeclarationId, ProjectExternalDeclarations, ProjectSymbol,
    ProjectSymbolLinkError, ProjectSymbolLinkReport, ProjectSymbolResolutionError,
    ProjectSymbolRevision, ProjectSymbolWorldId,
};

mod import_graph;
mod imports;
mod nominal;
mod publication;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectSymbolLimitKind {
    AliasesPerModule,
    AliasesPerWorld,
    Imports,
    Diagnostics,
    Work,
    NominalDeclarationsPerModule,
    NominalDeclarationsPerWorld,
    NominalMembersPerDeclaration,
    NominalTypeParameters,
    NominalTypeNodesPerDeclaration,
}

pub struct ProjectSymbolLimits {
    aliases_per_module: u64,
    aliases_per_world: u64,
    imports: u64,
    diagnostics: u64,
    work: u64,
    nominal_declarations_per_module: u64,
    nominal_declarations_per_world: u64,
    nominal_members_per_declaration: u64,
    nominal_type_parameters: u64,
    nominal_type_nodes_per_declaration: u64,
}

impl ProjectSymbolLimits {
    pub const PRODUCTION: Self = Self {
        aliases_per_module: 256,
        aliases_per_world: 8_192,
        imports: 32_768,
        diagnostics: 128,
        work: 262_144,
        nominal_declarations_per_module: 1_024,
        nominal_declarations_per_world: 16_384,
        nominal_members_per_declaration: 4_096,
        nominal_type_parameters: 64,
        nominal_type_nodes_per_declaration: 16_384,
    };

    pub const fn aliases_per_module(&self) -> u64 {
        self.aliases_per_module
    }

    pub const fn aliases_per_world(&self) -> u64 {
        self.aliases_per_world
    }

    pub const fn imports(&self) -> u64 {
        self.imports
    }

    pub const fn diagnostics(&self) -> u64 {
        self.diagnostics
    }

    pub const fn work(&self) -> u64 {
        self.work
    }

    pub const fn nominal_declarations_per_module(&self) -> u64 {
        self.nominal_declarations_per_module
    }

    pub const fn nominal_declarations_per_world(&self) -> u64 {
        self.nominal_declarations_per_world
    }

    pub const fn nominal_members_per_declaration(&self) -> u64 {
        self.nominal_members_per_declaration
    }

    pub const fn nominal_type_parameters(&self) -> u64 {
        self.nominal_type_parameters
    }

    pub const fn nominal_type_nodes_per_declaration(&self) -> u64 {
        self.nominal_type_nodes_per_declaration
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectSymbolTargetId {
    Callable(CallableDeclarationId),
    External(ExternalDeclarationId),
    Nominal(ProjectNominalDeclarationId),
    Module(CanonicalModulePath),
}

#[derive(Debug)]
pub enum ResolvedProjectSymbol<'a> {
    Callable(&'a CallableSymbol),
    External(&'a ExternalSymbol),
    Nominal(&'a ProjectNominalDeclaration),
    Module(&'a CanonicalModulePath),
}

/// One deterministically ordered project candidate retained in a type lookup failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTypeCandidate {
    target: ProjectSymbolTargetId,
    declaration: Option<SourceSpan>,
    binding_sites: Box<[SourceSpan]>,
}

/// A project declaration that may legally occupy type position.
#[derive(Clone, Copy, Debug)]
pub enum ProjectTypeTarget<'a> {
    Nominal(&'a ProjectNominalDeclaration),
    External(&'a ExternalSymbol),
}

/// Typed result of looking up one project path in value position.
///
/// Nominals, modules, externals, and an unknown path are deliberately
/// represented by `Absent`: they do not occupy the callable value namespace.
/// Ambiguous or inaccessible callable bindings are returned as terminal typed
/// errors so a later type-position lookup cannot silently override them.
#[derive(Clone, Copy, Debug)]
pub enum ProjectValueLookup<'a> {
    Present(&'a CallableSymbol),
    Absent,
}

/// Authoritative project value-namespace lookup failure.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProjectValueLookupError {
    #[error("project value reference is ambiguous")]
    Ambiguous {
        module: CanonicalModulePath,
        reference: SymbolPath,
        reference_source: SourceSpan,
        candidates: Box<[ProjectSymbolTargetId]>,
    },
    #[error("project value reference is inaccessible")]
    Inaccessible {
        module: CanonicalModulePath,
        reference: SymbolPath,
        reference_source: SourceSpan,
        candidates: Box<[ProjectSymbolTargetId]>,
    },
    #[error("project value reference path is invalid")]
    InvalidPath {
        reference_source: SourceSpan,
        reason: ModulePathError,
    },
    #[error("project value lookup reached a missing accepted callable")]
    Poisoned {
        reference_source: SourceSpan,
        target: ProjectSymbolTargetId,
    },
}

/// Authoritative project type-target lookup failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectTypeLookupError {
    Unknown {
        module: CanonicalModulePath,
        reference: TypePath,
        source: SourceSpan,
    },
    Ambiguous {
        module: CanonicalModulePath,
        reference: TypePath,
        source: SourceSpan,
        candidates: Box<[ProjectTypeCandidate]>,
    },
    Inaccessible {
        module: CanonicalModulePath,
        reference: TypePath,
        source: SourceSpan,
        candidates: Box<[ProjectTypeCandidate]>,
    },
    WrongKind {
        reference: TypePath,
        source: SourceSpan,
        actual: Box<ProjectTypeCandidate>,
    },
    InvalidPath {
        source: SourceSpan,
        reason: ModulePathError,
    },
}

/// One source spelling visible to type completion in a project module.
#[derive(Clone, Copy, Debug)]
pub struct VisibleProjectTypeBinding<'a> {
    spelling: &'a ProjectSymbolPath,
    target: ProjectTypeTarget<'a>,
    visibility: Option<Visibility>,
    binding_sites: &'a [SourceSpan],
    reference_sites: &'a [SourceSpan],
}

/// One scope spelling whose binding set contains the expected target and at
/// least one different target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSymbolBindingCollision {
    module: CanonicalModulePath,
    path: ProjectSymbolPath,
    expected: ProjectSymbolTargetId,
    conflicting: Vec<ProjectSymbolTargetId>,
    expected_sites: Vec<SourceSpan>,
    conflicting_sites: Vec<SourceSpan>,
}

impl ProjectSymbolBindingCollision {
    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn path(&self) -> &ProjectSymbolPath {
        &self.path
    }

    pub const fn expected(&self) -> &ProjectSymbolTargetId {
        &self.expected
    }

    pub fn conflicting(&self) -> &[ProjectSymbolTargetId] {
        &self.conflicting
    }

    pub fn expected_sites(&self) -> &[SourceSpan] {
        &self.expected_sites
    }

    pub fn conflicting_sites(&self) -> &[SourceSpan] {
        &self.conflicting_sites
    }
}

#[derive(Clone, Debug)]
pub struct ProjectSymbolTable {
    world: ProjectSymbolWorldId,
    revision: ProjectSymbolRevision,
    modules: BTreeSet<CanonicalModulePath>,
    source_identities: BTreeMap<CanonicalModulePath, SourceDocumentIdentity>,
    symbols: BTreeMap<ProjectDeclarationId, ProjectSymbol>,
    nominal_ids: BTreeSet<ProjectNominalDeclarationId>,
    pub(super) scopes: BTreeMap<CanonicalModulePath, BTreeMap<String, Vec<ScopeBinding>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ScopeBinding {
    pub(super) path: ProjectSymbolPath,
    pub(super) target: ProjectSymbolTargetId,
    pub(super) visibility: Option<Visibility>,
    pub(super) owner: CanonicalModulePath,
    pub(super) sites: Vec<SourceSpan>,
    pub(super) reference_sites: Vec<SourceSpan>,
}

#[derive(Clone, Debug)]
pub struct ProjectSymbolLinkOutput {
    table: ProjectSymbolTable,
    seed_declarations: BTreeMap<ExternalDeclarationSeedId, ExternalDeclarationId>,
    work_charged: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ImportResolutionError {
    Unknown,
    Inaccessible(Vec<ScopeBinding>),
    VisibilityEscalation,
    Ambiguous(Vec<ProjectSymbolTargetId>),
    InvalidPath(ModulePathError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LinkedProjectSymbolPath {
    reference: SymbolPath,
    unaliased_binding: ProjectSymbolPath,
}

impl ProjectTypeCandidate {
    pub const fn target(&self) -> &ProjectSymbolTargetId {
        &self.target
    }

    pub const fn declaration(&self) -> Option<&SourceSpan> {
        self.declaration.as_ref()
    }

    pub fn binding_sites(&self) -> &[SourceSpan] {
        &self.binding_sites
    }
}

impl<'a> VisibleProjectTypeBinding<'a> {
    pub const fn spelling(&self) -> &ProjectSymbolPath {
        self.spelling
    }

    pub const fn target(&self) -> ProjectTypeTarget<'a> {
        self.target
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn binding_sites(&self) -> &[SourceSpan] {
        self.binding_sites
    }

    /// Exact imported-name tokens that refer to this target.
    pub const fn reference_sites(&self) -> &[SourceSpan] {
        self.reference_sites
    }
}

impl ProjectSymbolLinkOutput {
    pub const fn table(&self) -> &ProjectSymbolTable {
        &self.table
    }

    pub fn seed_declaration(
        &self,
        seed: ExternalDeclarationSeedId,
    ) -> Option<ExternalDeclarationId> {
        self.seed_declarations.get(&seed).copied()
    }

    pub fn seed_declarations(
        &self,
    ) -> impl ExactSizeIterator<Item = (ExternalDeclarationSeedId, ExternalDeclarationId)> + '_
    {
        self.seed_declarations.iter().map(|(seed, id)| (*seed, *id))
    }

    pub const fn work_charged(&self) -> u64 {
        self.work_charged
    }

    pub fn into_table(self) -> ProjectSymbolTable {
        self.table
    }
}

impl ProjectSymbolTable {
    pub fn link(
        project: &HirProject,
        externals: &ProjectExternalDeclarations,
    ) -> Result<ProjectSymbolLinkOutput, ProjectSymbolLinkReport> {
        let modules = project
            .modules()
            .map(|(path, _)| path.clone())
            .collect::<BTreeSet<_>>();
        let source_identities = project
            .source_identities()
            .map(|(path, source)| (path.clone(), source.clone()))
            .collect();
        let mut table = Self {
            world: externals.world().clone(),
            revision: *externals.revision(),
            scopes: modules
                .iter()
                .cloned()
                .map(|module| (module, BTreeMap::new()))
                .collect(),
            modules,
            source_identities,
            symbols: BTreeMap::new(),
            nominal_ids: BTreeSet::new(),
        };
        let mut diagnostics = Vec::new();
        let mut work = 0_u64;

        if let Err(error) = Self::charge(&mut work, 1, None) {
            diagnostics.push(error);
        }
        table.insert_module_bindings(project);
        table.insert_callables(project, &mut diagnostics, &mut work);
        table.insert_nominals(project, &mut diagnostics, &mut work);
        let seed_declarations = table.insert_externals(externals, &mut diagnostics, &mut work);

        let imports = project
            .modules()
            .flat_map(|(module, hir)| {
                hir.uses()
                    .iter()
                    .map(|item| (module.clone(), item))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        Self::check_import_limits(project, &imports, &mut diagnostics);

        if diagnostics.is_empty() {
            loop {
                let mut changed = false;
                for (module, import) in &imports {
                    let source = source_span(project, module, *import.range());
                    if let Err(error) = Self::charge(&mut work, 1, Some(source)) {
                        diagnostics.push(error);
                        break;
                    }
                    if let Ok(bindings) = table.import_bindings(project, module, import) {
                        for binding in bindings {
                            changed |= table.insert_scope_binding(module, binding);
                        }
                    }
                }
                if !diagnostics.is_empty() || !changed {
                    break;
                }
            }
        }

        if diagnostics.is_empty() {
            let mut unresolved = Vec::new();
            for (index, (module, import)) in imports.iter().enumerate() {
                match table.import_bindings(project, module, import) {
                    Ok(_) => {}
                    Err(ImportResolutionError::Unknown) => unresolved.push(index),
                    Err(error) => {
                        diagnostics.push(Self::import_error(project, module, import, error));
                    }
                }
            }
            match import_graph::classify_unresolved_imports(
                project,
                &table,
                &imports,
                &unresolved,
                &mut work,
            ) {
                Ok(errors) => diagnostics.extend(errors),
                Err(error) => diagnostics.push(*error),
            }
        }

        if diagnostics.is_empty() {
            Ok(ProjectSymbolLinkOutput {
                table,
                seed_declarations,
                work_charged: work,
            })
        } else {
            Err(link_report(diagnostics, work))
        }
    }

    pub const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }

    pub const fn revision(&self) -> &ProjectSymbolRevision {
        &self.revision
    }

    pub fn modules(&self) -> impl ExactSizeIterator<Item = &CanonicalModulePath> {
        self.modules.iter()
    }

    /// Exact source-document revision for one project module.
    pub fn source_identity(&self, module: &CanonicalModulePath) -> Option<&SourceDocumentIdentity> {
        self.source_identities.get(module)
    }

    pub fn symbols(&self) -> impl ExactSizeIterator<Item = &ProjectSymbol> {
        self.symbols.values()
    }

    pub fn callable_symbols(&self) -> impl Iterator<Item = &CallableSymbol> {
        self.symbols.values().filter_map(|symbol| match symbol {
            ProjectSymbol::Callable(callable) => Some(callable),
            ProjectSymbol::External(_) | ProjectSymbol::Nominal(_) => None,
        })
    }

    pub fn external_symbols(&self) -> impl Iterator<Item = &ExternalSymbol> {
        self.symbols.values().filter_map(|symbol| match symbol {
            ProjectSymbol::External(external) => Some(external),
            ProjectSymbol::Callable(_) | ProjectSymbol::Nominal(_) => None,
        })
    }

    /// Published nominal declarations in deterministic identity order.
    ///
    /// # Panics
    ///
    /// Panics only if the table's private nominal-ID inventory and symbol map
    /// become inconsistent. Atomic publication maintains this invariant.
    pub fn nominal_symbols(&self) -> impl ExactSizeIterator<Item = &ProjectNominalDeclaration> {
        self.nominal_ids.iter().map(|id| {
            self.nominal(id)
                .expect("nominal ID inventory is table-owned")
        })
    }

    /// Every typed binding installed in a project module scope.
    ///
    /// Order is module, rendered private lookup key, then typed scope-row order.
    /// Multiple source sites for the same target remain coalesced into one row;
    /// callers consume the binding identity rather than its diagnostic sites.
    pub fn scope_bindings(
        &self,
    ) -> impl Iterator<
        Item = (
            &CanonicalModulePath,
            &ProjectSymbolPath,
            &ProjectSymbolTargetId,
        ),
    > {
        self.scopes.iter().flat_map(|(module, scope)| {
            scope.values().flat_map(move |bindings| {
                bindings
                    .iter()
                    .map(move |binding| (module, &binding.path, &binding.target))
            })
        })
    }

    /// Returns every deterministic scope collision that contains `expected`.
    ///
    /// This is a domain-neutral projection used by registrars that require a
    /// particular external declaration to remain unambiguous. The underlying
    /// scope records and same-target provenance remain private to HIR.
    pub fn binding_collisions_for(
        &self,
        expected: &ProjectSymbolTargetId,
    ) -> Vec<ProjectSymbolBindingCollision> {
        let mut collisions = Vec::new();
        for (module, scope) in &self.scopes {
            for bindings in scope.values() {
                let Some(expected_binding) =
                    bindings.iter().find(|binding| &binding.target == expected)
                else {
                    continue;
                };
                let path = expected_binding.path.clone();
                let expected_bindings = bindings
                    .iter()
                    .filter(|binding| &binding.target == expected)
                    .collect::<Vec<_>>();
                let conflicting_bindings = bindings
                    .iter()
                    .filter(|binding| &binding.target != expected)
                    .collect::<Vec<_>>();
                if conflicting_bindings.is_empty() {
                    continue;
                }

                let mut conflicting = conflicting_bindings
                    .iter()
                    .map(|binding| binding.target.clone())
                    .collect::<Vec<_>>();
                conflicting.sort();
                conflicting.dedup();
                let mut expected_sites = expected_bindings
                    .into_iter()
                    .flat_map(|binding| binding.sites.iter().cloned())
                    .collect::<Vec<_>>();
                sort_spans(&mut expected_sites);
                expected_sites.dedup();
                let mut conflicting_sites = conflicting_bindings
                    .into_iter()
                    .flat_map(|binding| binding.sites.iter().cloned())
                    .collect::<Vec<_>>();
                sort_spans(&mut conflicting_sites);
                conflicting_sites.dedup();
                collisions.push(ProjectSymbolBindingCollision {
                    module: module.clone(),
                    path,
                    expected: expected.clone(),
                    conflicting,
                    expected_sites,
                    conflicting_sites,
                });
            }
        }
        collisions
    }

    pub fn callable(&self, id: CallableDeclarationId) -> Option<&CallableSymbol> {
        match self.symbols.get(&ProjectDeclarationId::Callable(id))? {
            ProjectSymbol::Callable(symbol) => Some(symbol),
            ProjectSymbol::External(_) | ProjectSymbol::Nominal(_) => None,
        }
    }

    pub fn external(&self, id: ExternalDeclarationId) -> Option<&ExternalSymbol> {
        match self.symbols.get(&ProjectDeclarationId::External(id))? {
            ProjectSymbol::External(symbol) => Some(symbol),
            ProjectSymbol::Callable(_) | ProjectSymbol::Nominal(_) => None,
        }
    }

    pub fn nominal(&self, id: &ProjectNominalDeclarationId) -> Option<&ProjectNominalDeclaration> {
        match self
            .symbols
            .get(&ProjectDeclarationId::Nominal(id.clone()))?
        {
            ProjectSymbol::Nominal(symbol) => Some(symbol.as_ref()),
            ProjectSymbol::Callable(_) | ProjectSymbol::External(_) => None,
        }
    }

    pub fn resolve_type_target(
        &self,
        module: &CanonicalModulePath,
        path: &TypePath,
        source: SourceSpan,
    ) -> Result<ProjectTypeTarget<'_>, ProjectTypeLookupError> {
        let reference = SymbolPath::try_from(path.path()).map_err(|reason| {
            ProjectTypeLookupError::InvalidPath {
                source: source.clone(),
                reason,
            }
        })?;
        let bindings = match self.targets_for_symbol_path(module, &reference) {
            Ok(bindings) => bindings,
            Err(ImportResolutionError::Inaccessible(bindings)) => {
                return Err(ProjectTypeLookupError::Inaccessible {
                    module: module.clone(),
                    reference: path.clone(),
                    source,
                    candidates: self.type_candidates(bindings),
                });
            }
            Err(ImportResolutionError::InvalidPath(reason)) => {
                return Err(ProjectTypeLookupError::InvalidPath { source, reason });
            }
            Err(
                ImportResolutionError::Unknown
                | ImportResolutionError::Ambiguous(_)
                | ImportResolutionError::VisibilityEscalation,
            ) => {
                return Err(ProjectTypeLookupError::Unknown {
                    module: module.clone(),
                    reference: path.clone(),
                    source,
                });
            }
        };
        let candidates = self.type_candidates(bindings);
        if candidates.is_empty() {
            return Err(ProjectTypeLookupError::Unknown {
                module: module.clone(),
                reference: path.clone(),
                source,
            });
        }
        if candidates.len() > 1 {
            return Err(ProjectTypeLookupError::Ambiguous {
                module: module.clone(),
                reference: path.clone(),
                source,
                candidates,
            });
        }
        let Some(actual) = candidates.into_vec().pop() else {
            return Err(ProjectTypeLookupError::Unknown {
                module: module.clone(),
                reference: path.clone(),
                source,
            });
        };
        match actual.target() {
            ProjectSymbolTargetId::Nominal(id) => self
                .nominal(id)
                .map(ProjectTypeTarget::Nominal)
                .ok_or_else(|| ProjectTypeLookupError::Unknown {
                    module: module.clone(),
                    reference: path.clone(),
                    source,
                }),
            ProjectSymbolTargetId::External(id) => self
                .external(*id)
                .map(ProjectTypeTarget::External)
                .ok_or_else(|| ProjectTypeLookupError::Unknown {
                    module: module.clone(),
                    reference: path.clone(),
                    source,
                }),
            ProjectSymbolTargetId::Callable(_) | ProjectSymbolTargetId::Module(_) => {
                Err(ProjectTypeLookupError::WrongKind {
                    reference: path.clone(),
                    source,
                    actual: Box::new(actual),
                })
            }
        }
    }

    pub fn visible_type_bindings(
        &self,
        module: &CanonicalModulePath,
    ) -> impl Iterator<Item = VisibleProjectTypeBinding<'_>> {
        self.scopes
            .get(module)
            .into_iter()
            .flat_map(|scope| scope.values())
            .flatten()
            .filter(|binding| Self::binding_visible_from(binding, module))
            .filter_map(|binding| {
                let target = match &binding.target {
                    ProjectSymbolTargetId::Nominal(id) => {
                        ProjectTypeTarget::Nominal(self.nominal(id)?)
                    }
                    ProjectSymbolTargetId::External(id) => {
                        ProjectTypeTarget::External(self.external(*id)?)
                    }
                    ProjectSymbolTargetId::Callable(_) | ProjectSymbolTargetId::Module(_) => {
                        return None;
                    }
                };
                Some(VisibleProjectTypeBinding {
                    spelling: &binding.path,
                    target,
                    visibility: binding.visibility,
                    binding_sites: &binding.sites,
                    reference_sites: &binding.reference_sites,
                })
            })
    }

    fn type_candidates(&self, bindings: Vec<ScopeBinding>) -> Box<[ProjectTypeCandidate]> {
        let mut sites_by_target = BTreeMap::<ProjectSymbolTargetId, Vec<SourceSpan>>::new();
        for binding in bindings {
            sites_by_target
                .entry(binding.target)
                .or_default()
                .extend(binding.sites);
        }
        sites_by_target
            .into_iter()
            .map(|(target, mut binding_sites)| {
                sort_spans(&mut binding_sites);
                binding_sites.dedup();
                let declaration = match &target {
                    ProjectSymbolTargetId::Callable(id) => self
                        .callable(id.clone())
                        .map(|symbol| symbol.source().clone()),
                    ProjectSymbolTargetId::External(id) => self
                        .external(*id)
                        .map(|symbol| symbol.declaration_span().clone()),
                    ProjectSymbolTargetId::Nominal(id) => self
                        .nominal(id)
                        .map(|symbol| symbol.source().name().clone()),
                    ProjectSymbolTargetId::Module(_) => None,
                };
                ProjectTypeCandidate {
                    target,
                    declaration,
                    binding_sites: binding_sites.into_boxed_slice(),
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    /// Resolves a structured project path in the callable value namespace.
    ///
    /// This lookup intentionally filters nominal/module/external targets before
    /// deciding ambiguity. A callable and a same-spelling type therefore select
    /// the callable in value position, while two callable targets remain a
    /// terminal ambiguity. No source label is parsed or reconstructed here.
    #[allow(
        clippy::result_large_err,
        reason = "value lookup errors retain typed module, path, source, and target evidence"
    )]
    pub fn resolve_value_target(
        &self,
        module: &CanonicalModulePath,
        reference: &SymbolPath,
        source: SourceSpan,
    ) -> Result<ProjectValueLookup<'_>, ProjectValueLookupError> {
        let bindings = match self.targets_for_symbol_path(module, reference) {
            Ok(bindings) => bindings,
            Err(ImportResolutionError::Unknown) => return Ok(ProjectValueLookup::Absent),
            Err(ImportResolutionError::InvalidPath(reason)) => {
                return Err(ProjectValueLookupError::InvalidPath {
                    reference_source: source,
                    reason,
                });
            }
            Err(ImportResolutionError::VisibilityEscalation) => {
                return Err(ProjectValueLookupError::Inaccessible {
                    module: module.clone(),
                    reference: reference.clone(),
                    reference_source: source,
                    candidates: Box::new([]),
                });
            }
            Err(ImportResolutionError::Inaccessible(bindings)) => {
                let targets = bindings
                    .into_iter()
                    .map(|binding| binding.target)
                    .collect::<Vec<_>>();
                let callables = Self::callable_value_targets(targets);
                if callables.is_empty() {
                    return Ok(ProjectValueLookup::Absent);
                }
                return Err(ProjectValueLookupError::Inaccessible {
                    module: module.clone(),
                    reference: reference.clone(),
                    reference_source: source,
                    candidates: callables.into_boxed_slice(),
                });
            }
            Err(ImportResolutionError::Ambiguous(targets)) => {
                let callables = Self::callable_value_targets(targets);
                match callables.as_slice() {
                    [] => return Ok(ProjectValueLookup::Absent),
                    [ProjectSymbolTargetId::Callable(id)] => {
                        return self.callable(id.clone()).map_or_else(
                            || {
                                Err(ProjectValueLookupError::Poisoned {
                                    reference_source: source,
                                    target: ProjectSymbolTargetId::Callable(id.clone()),
                                })
                            },
                            |callable| Ok(ProjectValueLookup::Present(callable)),
                        );
                    }
                    _ => {
                        return Err(ProjectValueLookupError::Ambiguous {
                            module: module.clone(),
                            reference: reference.clone(),
                            reference_source: source,
                            candidates: callables.into_boxed_slice(),
                        });
                    }
                }
            }
        };

        let callables = Self::callable_value_targets(
            bindings.into_iter().map(|binding| binding.target).collect(),
        );
        match callables.as_slice() {
            [] => {
                let inaccessible = Self::callable_value_targets(
                    self.inaccessible_bindings_for_symbol_path(module, reference)
                        .into_iter()
                        .map(|binding| binding.target)
                        .collect(),
                );
                if inaccessible.is_empty() {
                    Ok(ProjectValueLookup::Absent)
                } else {
                    Err(ProjectValueLookupError::Inaccessible {
                        module: module.clone(),
                        reference: reference.clone(),
                        reference_source: source,
                        candidates: inaccessible.into_boxed_slice(),
                    })
                }
            }
            [ProjectSymbolTargetId::Callable(id)] => self.callable(id.clone()).map_or_else(
                || {
                    Err(ProjectValueLookupError::Poisoned {
                        reference_source: source,
                        target: ProjectSymbolTargetId::Callable(id.clone()),
                    })
                },
                |callable| Ok(ProjectValueLookup::Present(callable)),
            ),
            _ => Err(ProjectValueLookupError::Ambiguous {
                module: module.clone(),
                reference: reference.clone(),
                reference_source: source,
                candidates: callables.into_boxed_slice(),
            }),
        }
    }

    fn callable_value_targets(targets: Vec<ProjectSymbolTargetId>) -> Vec<ProjectSymbolTargetId> {
        let mut targets = targets
            .into_iter()
            .filter(|target| matches!(target, ProjectSymbolTargetId::Callable(_)))
            .collect::<Vec<_>>();
        targets.sort();
        targets.dedup();
        targets
    }

    #[allow(
        clippy::result_large_err,
        reason = "resolution errors retain typed module, path, source, and target evidence"
    )]
    pub fn resolve(
        &self,
        module: &CanonicalModulePath,
        reference: &SymbolPath,
        source: &SourceSpan,
    ) -> Result<ResolvedProjectSymbol<'_>, ProjectSymbolResolutionError> {
        let mut candidates = self
            .targets_for_symbol_path(module, reference)
            .map_err(|error| match error {
                ImportResolutionError::InvalidPath(reason) => {
                    ProjectSymbolResolutionError::InvalidPath {
                        source: source.clone(),
                        reason,
                    }
                }
                ImportResolutionError::Ambiguous(candidates) => {
                    ProjectSymbolResolutionError::Ambiguous {
                        module: module.clone(),
                        reference: reference.clone(),
                        source: source.clone(),
                        candidates,
                    }
                }
                ImportResolutionError::Unknown
                | ImportResolutionError::Inaccessible(_)
                | ImportResolutionError::VisibilityEscalation => {
                    ProjectSymbolResolutionError::Unknown {
                        module: module.clone(),
                        reference: reference.clone(),
                        source: source.clone(),
                    }
                }
            })?
            .into_iter()
            .map(|binding| binding.target)
            .collect::<Vec<_>>();
        candidates.sort();
        candidates.dedup();
        match candidates.as_slice() {
            [ProjectSymbolTargetId::Callable(id)] => self
                .callable(id.clone())
                .map(ResolvedProjectSymbol::Callable)
                .ok_or_else(|| ProjectSymbolResolutionError::Unknown {
                    module: module.clone(),
                    reference: reference.clone(),
                    source: source.clone(),
                }),
            [ProjectSymbolTargetId::External(id)] => self
                .external(*id)
                .map(ResolvedProjectSymbol::External)
                .ok_or_else(|| ProjectSymbolResolutionError::Unknown {
                    module: module.clone(),
                    reference: reference.clone(),
                    source: source.clone(),
                }),
            [ProjectSymbolTargetId::Nominal(id)] => self
                .nominal(id)
                .map(ResolvedProjectSymbol::Nominal)
                .ok_or_else(|| ProjectSymbolResolutionError::Unknown {
                    module: module.clone(),
                    reference: reference.clone(),
                    source: source.clone(),
                }),
            [ProjectSymbolTargetId::Module(path)] => self
                .modules
                .get(path)
                .map(ResolvedProjectSymbol::Module)
                .ok_or_else(|| ProjectSymbolResolutionError::Unknown {
                    module: module.clone(),
                    reference: reference.clone(),
                    source: source.clone(),
                }),
            [] => Err(ProjectSymbolResolutionError::Unknown {
                module: module.clone(),
                reference: reference.clone(),
                source: source.clone(),
            }),
            candidates => Err(ProjectSymbolResolutionError::Ambiguous {
                module: module.clone(),
                reference: reference.clone(),
                source: source.clone(),
                candidates: candidates.to_vec(),
            }),
        }
    }

    #[allow(
        clippy::result_large_err,
        reason = "resolution errors retain typed module, path, source, and target evidence"
    )]
    pub fn resolve_callable(
        &self,
        module: &CanonicalModulePath,
        reference: &SymbolPath,
        source: &SourceSpan,
    ) -> Result<&CallableSymbol, ProjectSymbolResolutionError> {
        match self.resolve(module, reference, source)? {
            ResolvedProjectSymbol::Callable(callable) => Ok(callable),
            ResolvedProjectSymbol::External(external) => {
                Err(ProjectSymbolResolutionError::NotCallable {
                    reference: reference.clone(),
                    source: source.clone(),
                    actual: ProjectSymbolTargetId::External(external.declaration()),
                })
            }
            ResolvedProjectSymbol::Nominal(nominal) => {
                Err(ProjectSymbolResolutionError::NotCallable {
                    reference: reference.clone(),
                    source: source.clone(),
                    actual: ProjectSymbolTargetId::Nominal(nominal.id().clone()),
                })
            }
            ResolvedProjectSymbol::Module(module) => {
                Err(ProjectSymbolResolutionError::NotCallable {
                    reference: reference.clone(),
                    source: source.clone(),
                    actual: ProjectSymbolTargetId::Module(module.clone()),
                })
            }
        }
    }

    #[allow(
        clippy::result_large_err,
        reason = "link errors retain typed source and target evidence"
    )]
    pub(super) fn charge(
        work: &mut u64,
        units: u64,
        source: Option<SourceSpan>,
    ) -> Result<(), ProjectSymbolLinkError> {
        let attempted =
            work.checked_add(units)
                .ok_or_else(|| ProjectSymbolLinkError::WorkOverflow {
                    attempted: u64::MAX,
                    maximum: ProjectSymbolLimits::PRODUCTION.work(),
                    source: source.clone(),
                })?;
        if attempted > ProjectSymbolLimits::PRODUCTION.work() {
            return Err(ProjectSymbolLinkError::WorkOverflow {
                attempted,
                maximum: ProjectSymbolLimits::PRODUCTION.work(),
                source,
            });
        }
        *work = attempted;
        Ok(())
    }
}

impl ScopeBinding {
    fn new(
        path: ProjectSymbolPath,
        target: ProjectSymbolTargetId,
        visibility: Option<Visibility>,
        owner: CanonicalModulePath,
        sites: impl IntoIterator<Item = SourceSpan>,
    ) -> Self {
        assert_eq!(
            path.root(),
            ModulePathRoot::ImplicitCrate,
            "scope-local project bindings must use the implicit root"
        );
        let mut sites = sites.into_iter().collect::<Vec<_>>();
        sort_spans(&mut sites);
        sites.dedup();
        Self {
            path,
            target,
            visibility,
            owner,
            sites,
            reference_sites: Vec::new(),
        }
    }

    fn rebound(
        &self,
        path: ProjectSymbolPath,
        owner: &CanonicalModulePath,
        visibility: Option<Visibility>,
        site: SourceSpan,
        reference_site: Option<SourceSpan>,
    ) -> Self {
        let sites = self.sites.iter().cloned().chain([site]);
        let mut binding = Self::new(path, self.target.clone(), visibility, owner.clone(), sites);
        binding.reference_sites = self
            .reference_sites
            .iter()
            .cloned()
            .chain(reference_site)
            .collect();
        sort_spans(&mut binding.reference_sites);
        binding.reference_sites.dedup();
        binding
    }
}

impl LinkedProjectSymbolPath {
    fn try_new(path: &ProjectSymbolPath) -> Result<Self, ImportResolutionError> {
        let reference = SymbolPath::try_from(path).map_err(ImportResolutionError::InvalidPath)?;
        let non_leaf_segments = &path.segments()[..path.segments().len() - 1];
        let unaliased_binding = if non_leaf_segments
            .iter()
            .all(|segment| segment.try_as_module_segment().is_ok())
        {
            ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, [path.last_segment().clone()])
                .expect("one source path segment is a valid implicit project binding")
        } else {
            debug_assert_eq!(path.root(), ModulePathRoot::ImplicitCrate);
            path.clone()
        };
        Ok(Self {
            reference,
            unaliased_binding,
        })
    }

    const fn reference(&self) -> &SymbolPath {
        &self.reference
    }

    const fn unaliased_binding(&self) -> &ProjectSymbolPath {
        &self.unaliased_binding
    }
}

fn append_leaf_qualifier(
    path: &SymbolPath,
    leaf: &ProjectSymbolSegment,
) -> Result<SymbolPath, ImportResolutionError> {
    let qualifier = ModuleSegment::new(path.leaf()).map_err(ImportResolutionError::InvalidPath)?;
    SymbolPath::try_new(
        path.root(),
        path.qualifiers()
            .iter()
            .cloned()
            .chain([qualifier])
            .collect(),
        leaf.as_str(),
    )
    .map_err(|_| ImportResolutionError::Unknown)
}

#[allow(
    clippy::result_large_err,
    reason = "unresolved-import classification preserves complete typed link evidence"
)]
fn use_counts(import: &UseItem) -> (u64, u64) {
    match import.tree().kind() {
        UseTreeKind::Path { alias, .. } => (1, u64::from(alias.is_some())),
        UseTreeKind::Glob { .. } => (1, 0),
        UseTreeKind::Group { names, .. } => (
            u64::try_from(names.len()).unwrap_or(u64::MAX),
            u64::try_from(names.iter().filter(|name| name.alias().is_some()).count())
                .unwrap_or(u64::MAX),
        ),
    }
}

fn is_reserved_type_name(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "f32"
            | "f64"
            | "String"
            | "char"
            | "Bytes"
            | "Unit"
            | "Never"
            | "Vec"
            | "Slice"
            | "Seq"
            | "Option"
            | "Probe"
            | "ThreadHandle"
            | "Shared"
            | "Array"
            | "OrderedMap"
            | "SortedMap"
            | "BTreeMap"
            | "Result"
            | "Need"
            | "Stream"
            | "Source"
            | "Ref"
            | "Speaker"
            | "SpeakerPreset"
    )
}

fn coalesce_bindings(mut bindings: Vec<ScopeBinding>) -> Vec<ScopeBinding> {
    bindings.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.target.cmp(&right.target))
            .then_with(|| left.visibility.cmp(&right.visibility))
            .then_with(|| left.owner.cmp(&right.owner))
    });
    let mut coalesced: Vec<ScopeBinding> = Vec::with_capacity(bindings.len());
    for binding in bindings {
        if let Some(existing) = coalesced.last_mut()
            && existing.path == binding.path
            && existing.target == binding.target
            && existing.visibility == binding.visibility
            && existing.owner == binding.owner
        {
            existing.sites.extend(binding.sites);
            sort_spans(&mut existing.sites);
            existing.sites.dedup();
            existing.reference_sites.extend(binding.reference_sites);
            sort_spans(&mut existing.reference_sites);
            existing.reference_sites.dedup();
        } else {
            coalesced.push(binding);
        }
    }
    coalesced
}

fn source_span(project: &HirProject, module: &CanonicalModulePath, range: TextRange) -> SourceSpan {
    project
        .module(module)
        .expect("known module has HIR")
        .source_span(range)
        .expect("project-relevant HIR ranges were bound during lowering")
}

fn sort_spans(spans: &mut [SourceSpan]) {
    spans.sort_by(|left, right| {
        left.source()
            .id()
            .cmp(right.source().id())
            .then_with(|| left.source().revision().cmp(&right.source().revision()))
            .then_with(|| left.range().cmp(&right.range()))
    });
}

fn link_report(
    mut diagnostics: Vec<ProjectSymbolLinkError>,
    work_charged: u64,
) -> ProjectSymbolLinkReport {
    diagnostics.sort_by(|left, right| {
        left.source()
            .map_or((None, None, None), |source| {
                (
                    Some(source.source().id()),
                    Some(source.source().revision()),
                    Some(source.range()),
                )
            })
            .cmp(&right.source().map_or((None, None, None), |source| {
                (
                    Some(source.source().id()),
                    Some(source.source().revision()),
                    Some(source.range()),
                )
            }))
            .then_with(|| left.code().cmp(&right.code()))
            .then_with(|| left.cmp(right))
    });
    diagnostics.dedup();
    let maximum = usize::try_from(ProjectSymbolLimits::PRODUCTION.diagnostics())
        .expect("project-symbol diagnostic maximum fits usize");
    let omitted_diagnostics = diagnostics.len().saturating_sub(maximum) as u64;
    diagnostics.truncate(maximum);
    ProjectSymbolLinkReport {
        diagnostics,
        omitted_diagnostics,
        work_charged,
    }
}
