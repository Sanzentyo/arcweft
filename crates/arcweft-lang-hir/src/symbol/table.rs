//! Unified project-symbol table and deterministic fixed-point linker.
//!
//! Binding insertion, ambiguity resolution, work charging, and bounded link
//! reporting stay together because they share one monotone transaction and its
//! ordering invariants. The module remains below the production warning gate.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arcweft_id::PublicId;
use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModulePathError, ModulePathRoot},
    symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
};
use arcweft_source::{SourceDocumentIdentity, SourceSpan};

use crate::identity::{HirSnapshotId, ItemId};
use crate::item::{HirItemKind, HirUseBinding, HirVisibility};
use crate::leaf::HirPath;
use crate::module::HirModule;
use crate::project::HirProjectView;
use crate::proof_return::{HirProofReturnHeaderModuleView, HirProofReturnHeaderProjectView};
use crate::source_index::{
    HirCallableSourceOwner, HirDeclarationSourceRole, HirItemSourceRole, HirSourcePresence,
    HirSourceQuery, HirSourceSite, HirUseBindingSourcePart, HirUseSourceRole,
};

use super::nominal::{ProjectNominalDeclaration, ProjectNominalDeclarationId};
use super::{
    CallableDeclarationKey, CallableDeclarationOwner, CallableSymbol, ExternalDeclarationId,
    ExternalDeclarationSeedId, ExternalSymbol, ProjectDeclarationId, ProjectExternalDeclarations,
    ProjectRetainedSymbol, ProjectSymbol, ProjectSymbolLinkError, ProjectSymbolLinkReport,
    ProjectSymbolResolutionError, ProjectSymbolRevision, ProjectSymbolWorldId, ProofArtifactId,
    ProofArtifactIdentityError,
};

mod import_graph;
mod imports;
mod nominal;
mod publication;
mod retained;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectSymbolLimitKind {
    AliasesPerModule,
    AliasesPerWorld,
    Imports,
    Diagnostics,
    Work,
    NominalDeclarationsPerModule,
    NominalDeclarationsPerWorld,
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

    pub const fn nominal_type_parameters(&self) -> u64 {
        self.nominal_type_parameters
    }

    pub const fn nominal_type_nodes_per_declaration(&self) -> u64 {
        self.nominal_type_nodes_per_declaration
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectSymbolTargetId {
    Callable(CallableDeclarationKey),
    /// Structural execution owner retained in the callable authority without
    /// entering the ordinary callable value namespace.
    StructuralCallable(CallableDeclarationKey),
    External(ExternalDeclarationId),
    Nominal(ProjectNominalDeclarationId),
    Retained(PublicId),
    Module(CanonicalModulePath),
}

#[derive(Debug)]
pub enum ResolvedProjectSymbol<'a> {
    Callable(&'a CallableSymbol),
    StructuralCallable(&'a CallableSymbol),
    External(&'a ExternalSymbol),
    Nominal(&'a ProjectNominalDeclaration),
    Retained(&'a ProjectRetainedSymbol),
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
    #[error("final HIR value reference cannot be projected into the accepted project path domain")]
    InvalidHirPath {
        reference: HirPath,
        reference_source: SourceSpan,
    },
    #[error("project value lookup reached a missing accepted callable")]
    Poisoned {
        reference_source: SourceSpan,
        target: ProjectSymbolTargetId,
    },
}

/// Failure to project one final-HIR path through the accepted project symbol
/// table without reconstructing a syntax path or source spelling.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProjectHirSymbolLookupError {
    #[error("final HIR symbol path is invalid in the accepted module context")]
    InvalidPath {
        reference: HirPath,
        site: SourceSpan,
        reason: ModulePathError,
    },
    #[error("final HIR symbol path cannot be projected into the accepted project path domain")]
    InvalidHirPath {
        reference: HirPath,
        site: SourceSpan,
    },
    #[error(transparent)]
    Symbol(#[from] ProjectSymbolResolutionError),
}

/// Authoritative project type-target lookup failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectTypeLookupError {
    Unknown {
        module: CanonicalModulePath,
        reference: HirPath,
        source: SourceSpan,
    },
    Ambiguous {
        module: CanonicalModulePath,
        reference: HirPath,
        source: SourceSpan,
        candidates: Box<[ProjectTypeCandidate]>,
    },
    Inaccessible {
        module: CanonicalModulePath,
        reference: HirPath,
        source: SourceSpan,
        candidates: Box<[ProjectTypeCandidate]>,
    },
    WrongKind {
        reference: HirPath,
        source: SourceSpan,
        actual: Box<ProjectTypeCandidate>,
    },
    InvalidPath {
        reference: HirPath,
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
    callable_sources:
        BTreeMap<(HirSnapshotId, ItemId, HirCallableSourceOwner), Option<CallableDeclarationKey>>,
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

#[derive(Clone, Copy)]
pub(super) enum ProjectSymbolModuleView<'project, 'source> {
    Published(&'project Arc<HirModule>),
    ProofHeader(HirProofReturnHeaderModuleView<'project, 'source>),
}

impl ProjectSymbolModuleView<'_, '_> {
    pub(super) fn snapshot_id(self) -> crate::identity::HirSnapshotId {
        match self {
            Self::Published(module) => module.snapshot_id(),
            Self::ProofHeader(module) => module.snapshot_id(),
        }
    }

    pub(super) fn item_source(self, owner: ItemId, role: HirItemSourceRole) -> Option<SourceSpan> {
        match self {
            Self::Published(module) => {
                let lookup = module
                    .source_site(
                        module.provenance().source_identity(),
                        HirSourceQuery::Item { owner, role },
                    )
                    .ok()?;
                match lookup.presence() {
                    HirSourcePresence::Present(HirSourceSite::Span(span)) => Some(span.clone()),
                    HirSourcePresence::Present(HirSourceSite::Insertion(_))
                    | HirSourcePresence::AbsentOptional => None,
                }
            }
            Self::ProofHeader(module) => match module.item_source_site(owner, role)? {
                HirSourceSite::Span(span) => Some(span.clone()),
                HirSourceSite::Insertion(_) => None,
            },
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct ProjectImportRef<'project> {
    module_path: &'project CanonicalModulePath,
    module: ProjectSymbolModuleView<'project, 'project>,
    owner: ItemId,
    ordinal: u32,
    visibility: Option<Visibility>,
    binding: &'project HirUseBinding,
}

impl ProjectImportRef<'_> {
    pub(super) fn whole_source(self) -> SourceSpan {
        self.source(HirUseSourceRole::Whole)
            .expect("final use item owns an authored whole span")
    }

    pub(super) fn path_source(self) -> SourceSpan {
        self.source(HirUseSourceRole::Binding {
            ordinal: self.ordinal,
            part: HirUseBindingSourcePart::TerminalReference,
        })
        .expect("final use binding owns a terminal reference span")
    }

    fn source(self, role: HirUseSourceRole) -> Option<SourceSpan> {
        self.module
            .item_source(self.owner, HirItemSourceRole::Use(role))
    }
}

const fn hir_visibility(visibility: HirVisibility) -> Visibility {
    match visibility {
        HirVisibility::Public => Visibility::Public,
        HirVisibility::Crate => Visibility::Crate,
        HirVisibility::Super => Visibility::Super,
    }
}

fn authored_span(site: &HirSourceSite) -> Option<SourceSpan> {
    match site {
        HirSourceSite::Span(span) => Some(span.clone()),
        HirSourceSite::Insertion(_) => None,
    }
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
    /// Links the sole project symbol table directly from the immutable view of
    /// one paused final-HIR project transaction. No provisional module is
    /// published and no declaration is reconstructed from source text.
    ///
    /// # Panics
    ///
    /// Panics only if an accepted resolved HIR name or bounded declaration
    /// ordinal violates its construction invariant.
    #[allow(
        clippy::too_many_lines,
        reason = "one atomic linker pass owns module inventory, declarations, imports, fixed-point resolution, diagnostics, and accounting"
    )]
    pub fn link_proof_return_headers(
        project: HirProofReturnHeaderProjectView<'_, '_>,
        externals: &ProjectExternalDeclarations,
    ) -> Result<ProjectSymbolLinkOutput, ProjectSymbolLinkReport> {
        let modules = project
            .modules()
            .map(|(path, _)| path.clone())
            .collect::<BTreeSet<_>>();
        let source_identities = project
            .modules()
            .map(|(path, module)| (path.clone(), module.source_identity().clone()))
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
            callable_sources: BTreeMap::new(),
            nominal_ids: BTreeSet::new(),
        };
        let mut diagnostics = Vec::new();
        let mut work = 0_u64;
        if let Err(error) = Self::charge(&mut work, 1, None) {
            diagnostics.push(error);
        }

        for (path, module) in project.modules() {
            let Some(name) = path.last_segment() else {
                continue;
            };
            let owner = path
                .parent()
                .unwrap_or_else(CanonicalModulePath::crate_root);
            let site = module
                .document()
                .span(arcweft_source::SourceRange::new(0, 0))
                .expect("zero-width module binding is in bounds");
            let binding = ProjectSymbolPath::new(
                ModulePathRoot::ImplicitCrate,
                [ProjectSymbolSegment::try_new(name)
                    .expect("module segment is a project symbol segment")],
            )
            .expect("one module segment is a valid project binding");
            table.insert_scope_binding(
                &owner,
                ScopeBinding::new(
                    binding,
                    ProjectSymbolTargetId::Module(path.clone()),
                    Some(Visibility::Public),
                    owner.clone(),
                    [site],
                ),
            );
        }

        table.insert_retained_header_declarations(project, &mut diagnostics, &mut work);

        let authored_return_items = project
            .authored_proof_returns()
            .map(|proof| (proof.module().key().path().clone(), proof.item()))
            .collect::<BTreeSet<_>>();
        let mut impl_ordinals = BTreeMap::<CanonicalModulePath, u32>::new();
        for item_ref in project.items() {
            let module = item_ref.module();
            let module_path = module.key().path();
            if matches!(item_ref.item().kind(), HirItemKind::Proof(_))
                && authored_return_items.contains(&(module_path.clone(), item_ref.id()))
            {
                // An authored-return Proof has a dedicated pending/header row.
                // Retained modules expose the finalized item as well, so the
                // two typed views must remain mutually exclusive here.
                continue;
            }
            let impl_ordinal = impl_ordinals.entry(module_path.clone()).or_default();
            // Every non-pending item in this header view has completed final
            // lowering. The transaction still withholds authored Proof bodies,
            // but the immutable symbol table is also the one consumed after
            // atomic publication, so clean sibling declarations must retain
            // their final executable state here. Pending Proofs are excluded
            // above and inserted explicitly as non-executable below.
            if !table.insert_item_callables(
                module_path,
                ProjectSymbolModuleView::ProofHeader(module),
                item_ref.id(),
                item_ref.item(),
                impl_ordinal,
                true,
                &mut diagnostics,
                &mut work,
            ) {
                break;
            }
        }
        for proof in project.authored_proof_returns() {
            let Some(name) = proof.name().resolved() else {
                continue;
            };
            let module = proof.module();
            let path = ProjectSymbolPath::new(
                ModulePathRoot::ImplicitCrate,
                [ProjectSymbolSegment::try_new(name.as_str())
                    .expect("resolved Proof name is a symbol segment")],
            )
            .expect("one Proof name is a valid binding");
            if !table.insert_callable_symbol(
                module.key().path(),
                ProjectSymbolModuleView::ProofHeader(module),
                proof.item(),
                HirCallableSourceOwner::Item,
                CallableDeclarationOwner::Proof,
                std::iter::empty(),
                name.as_str(),
                path,
                publication::prefix_visibility(proof.prefix()),
                publication::has_fx_attribute_prefix(proof.prefix()),
                proof.declaration_source().clone(),
                proof.name_source().clone(),
                false,
                &mut diagnostics,
                &mut work,
            ) {
                break;
            }
        }

        let mut world_count = 0_u64;
        let mut module_count = BTreeMap::<CanonicalModulePath, u64>::new();
        for item_ref in project.items() {
            let hir = match item_ref.item().kind() {
                HirItemKind::Struct(item) => nominal::NominalHir::Struct(item),
                HirItemKind::Enum(item) => nominal::NominalHir::Enum(item),
                HirItemKind::TypeAlias(item) => nominal::NominalHir::TypeAlias(item),
                _ => continue,
            };
            let module = item_ref.module();
            let Some(source) = module
                .item_source_site(
                    item_ref.id(),
                    HirItemSourceRole::Declaration(HirDeclarationSourceRole::Whole),
                )
                .and_then(authored_span)
            else {
                continue;
            };
            let count = module_count.entry(module.key().path().clone()).or_default();
            *count = count.saturating_add(1);
            world_count = world_count.saturating_add(1);
            for (kind, observed, maximum) in [
                (
                    ProjectSymbolLimitKind::NominalDeclarationsPerModule,
                    *count,
                    ProjectSymbolLimits::PRODUCTION.nominal_declarations_per_module(),
                ),
                (
                    ProjectSymbolLimitKind::NominalDeclarationsPerWorld,
                    world_count,
                    ProjectSymbolLimits::PRODUCTION.nominal_declarations_per_world(),
                ),
            ] {
                if observed > maximum {
                    diagnostics.push(ProjectSymbolLinkError::Limit {
                        kind,
                        observed,
                        maximum,
                        source: Some(source.clone()),
                    });
                }
            }
            if *count > ProjectSymbolLimits::PRODUCTION.nominal_declarations_per_module()
                || world_count > ProjectSymbolLimits::PRODUCTION.nominal_declarations_per_world()
            {
                continue;
            }
            let module_view = nominal::NominalModuleView::ProofHeader(module);
            if let Err(error) = Self::charge(
                &mut work,
                hir.link_work_units(module_view),
                Some(source.clone()),
            ) {
                diagnostics.push(error);
                break;
            }
            table.insert_nominal_declaration(
                module.key().path(),
                module_view,
                item_ref.id(),
                item_ref.item(),
                hir,
                source,
                &mut diagnostics,
            );
        }

        table.rebuild_callable_source_index();
        let seed_declarations = table.insert_externals(externals, &mut diagnostics, &mut work);
        let imports = project
            .items()
            .filter_map(|item_ref| match item_ref.item().kind() {
                HirItemKind::Use(declaration) => Some((item_ref, declaration)),
                _ => None,
            })
            .flat_map(|(item_ref, declaration)| {
                let visibility = item_ref.item().prefix().visibility().map(hir_visibility);
                declaration
                    .bindings()
                    .iter()
                    .enumerate()
                    .filter(|(_, binding)| binding.path().as_resolved().is_some())
                    .map(move |(ordinal, binding)| ProjectImportRef {
                        module_path: item_ref.module().key().path(),
                        module: ProjectSymbolModuleView::ProofHeader(item_ref.module()),
                        owner: item_ref.id(),
                        ordinal: u32::try_from(ordinal)
                            .expect("accepted use binding count fits u32"),
                        visibility,
                        binding,
                    })
            })
            .collect::<Vec<_>>();
        Self::check_import_limits(&imports, &mut diagnostics);
        if diagnostics.is_empty() {
            loop {
                let mut changed = false;
                for import in &imports {
                    let source = import.whole_source();
                    if let Err(error) = Self::charge(&mut work, 1, Some(source)) {
                        diagnostics.push(error);
                        break;
                    }
                    if let Ok(bindings) = table.import_bindings(*import) {
                        for binding in bindings {
                            changed |= table.insert_scope_binding(import.module_path, binding);
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
            for (index, import) in imports.iter().enumerate() {
                match table.import_bindings(*import) {
                    Ok(_) => {}
                    Err(ImportResolutionError::Unknown) => unresolved.push(index),
                    Err(error) => diagnostics.push(Self::import_error(*import, error)),
                }
            }
            match import_graph::classify_unresolved_imports(
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

    /// Links the sole project symbol table from one accepted final-HIR project.
    ///
    /// # Panics
    ///
    /// Panics only if an accepted resolved HIR name or bounded declaration
    /// ordinal violates its construction invariant.
    #[allow(
        clippy::too_many_lines,
        reason = "one atomic linker pass owns the accepted module and import inventories through final publication"
    )]
    pub fn link(
        project: HirProjectView<'_>,
        externals: &ProjectExternalDeclarations,
    ) -> Result<ProjectSymbolLinkOutput, ProjectSymbolLinkReport> {
        let modules = project
            .modules()
            .map(|(path, _)| path.clone())
            .collect::<BTreeSet<_>>();
        let source_identities = project
            .modules()
            .map(|(path, module)| (path.clone(), module.provenance().source_identity().clone()))
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
            callable_sources: BTreeMap::new(),
            nominal_ids: BTreeSet::new(),
        };
        let mut diagnostics = Vec::new();
        let mut work = 0_u64;

        if let Err(error) = Self::charge(&mut work, 1, None) {
            diagnostics.push(error);
        }
        table.insert_module_bindings(project);
        table.insert_retained_declarations(project, &mut diagnostics, &mut work);
        table.insert_callables(project, &mut diagnostics, &mut work);
        table.rebuild_callable_source_index();
        table.insert_nominals(project, &mut diagnostics, &mut work);
        let seed_declarations = table.insert_externals(externals, &mut diagnostics, &mut work);

        let imports = project
            .items()
            .filter_map(|item_ref| match item_ref.item().kind() {
                HirItemKind::Use(declaration) => Some((item_ref, declaration)),
                _ => None,
            })
            .flat_map(|(item_ref, declaration)| {
                let visibility = item_ref.item().prefix().visibility().map(hir_visibility);
                declaration
                    .bindings()
                    .iter()
                    .enumerate()
                    .filter(|(_, binding)| binding.path().as_resolved().is_some())
                    .map(move |(ordinal, binding)| ProjectImportRef {
                        module_path: item_ref.module_path(),
                        module: ProjectSymbolModuleView::Published(item_ref.module()),
                        owner: item_ref.id(),
                        ordinal: u32::try_from(ordinal)
                            .expect("accepted use binding count fits u32"),
                        visibility,
                        binding,
                    })
            })
            .collect::<Vec<_>>();
        Self::check_import_limits(&imports, &mut diagnostics);

        if diagnostics.is_empty() {
            loop {
                let mut changed = false;
                for import in &imports {
                    let source = import.whole_source();
                    if let Err(error) = Self::charge(&mut work, 1, Some(source)) {
                        diagnostics.push(error);
                        break;
                    }
                    if let Ok(bindings) = table.import_bindings(*import) {
                        for binding in bindings {
                            changed |= table.insert_scope_binding(import.module_path, binding);
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
            for (index, import) in imports.iter().enumerate() {
                match table.import_bindings(*import) {
                    Ok(_) => {}
                    Err(ImportResolutionError::Unknown) => unresolved.push(index),
                    Err(error) => {
                        diagnostics.push(Self::import_error(*import, error));
                    }
                }
            }
            match import_graph::classify_unresolved_imports(
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
            ProjectSymbol::External(_) | ProjectSymbol::Nominal(_) | ProjectSymbol::Retained(_) => {
                None
            }
        })
    }

    /// Returns the sole structural Flow symbol for one exact HIR item owner.
    ///
    /// Downstream project consumers use this projection instead of rebuilding
    /// a Flow public identity from the item surface.
    pub fn flow_symbol_for_item(&self, owner: ItemId) -> Option<&CallableSymbol> {
        self.callable_symbols().find(|symbol| {
            symbol.source_item() == owner && symbol.owner() == CallableDeclarationOwner::Flow
        })
    }

    pub fn external_symbols(&self) -> impl Iterator<Item = &ExternalSymbol> {
        self.symbols.values().filter_map(|symbol| match symbol {
            ProjectSymbol::External(external) => Some(external),
            ProjectSymbol::Callable(_) | ProjectSymbol::Nominal(_) | ProjectSymbol::Retained(_) => {
                None
            }
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

    pub fn callable(&self, id: &CallableDeclarationKey) -> Option<&CallableSymbol> {
        match self
            .symbols
            .get(&ProjectDeclarationId::Callable(id.clone()))?
        {
            ProjectSymbol::Callable(symbol) => Some(symbol),
            ProjectSymbol::External(_) | ProjectSymbol::Nominal(_) | ProjectSymbol::Retained(_) => {
                None
            }
        }
    }

    fn rebuild_callable_source_index(&mut self) {
        self.callable_sources.clear();
        let rows = self
            .callable_symbols()
            .map(|symbol| {
                (
                    (
                        symbol.source_snapshot(),
                        symbol.source_item(),
                        symbol.source_owner(),
                    ),
                    symbol.declaration().clone(),
                )
            })
            .collect::<Vec<_>>();
        for (key, declaration) in rows {
            match self.callable_sources.entry(key) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(Some(declaration));
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    if entry.get().as_ref() != Some(&declaration) {
                        entry.insert(None);
                    }
                }
            }
        }
    }

    /// Returns the unique callable published at one exact HIR source owner.
    ///
    /// Source coordinates are a join key, not a filter that callers may
    /// rebuild independently. Ambiguous or absent rows fail closed.
    pub fn callable_at_source(
        &self,
        snapshot: HirSnapshotId,
        item: crate::identity::ItemId,
        owner: crate::source_index::HirCallableSourceOwner,
    ) -> Option<&CallableSymbol> {
        let declaration = self
            .callable_sources
            .get(&(snapshot, item, owner))?
            .as_ref()?;
        self.callable(declaration)
    }

    /// Derives the sole session-only proof identity from the registered symbol
    /// and the exact final-HIR snapshot retained by the supplied project.
    pub fn proof_artifact(
        &self,
        project: HirProjectView<'_>,
        id: &super::CallableDeclarationId,
    ) -> Result<ProofArtifactId, ProofArtifactIdentityError> {
        let key = CallableDeclarationKey::Existing(id.clone());
        let symbol =
            self.callable(&key)
                .ok_or_else(|| ProofArtifactIdentityError::UnknownDeclaration {
                    declaration: id.clone(),
                })?;
        if symbol.owner() != super::CallableDeclarationOwner::Proof {
            return Err(ProofArtifactIdentityError::NotProof {
                declaration: id.clone(),
                actual: symbol.owner(),
            });
        }
        let snapshot = symbol.source_snapshot();
        let Some(module) = project
            .module(id.module())
            .filter(|module| module.snapshot_id() == snapshot)
        else {
            return Err(ProofArtifactIdentityError::SnapshotUnavailable { snapshot });
        };
        let item = symbol.source_item();
        let proof = module
            .resolve_item(item)
            .ok()
            .and_then(|item| match item.kind() {
                HirItemKind::Proof(proof) => Some(proof),
                _ => None,
            });
        let Some(proof) = proof else {
            return Err(ProofArtifactIdentityError::ItemMismatch { snapshot, item });
        };
        if symbol.source_owner() != crate::source_index::HirCallableSourceOwner::Item
            || symbol.declaration() != &key
            || proof.name().resolved().map(crate::leaf::HirName::as_str) != Some(id.name())
        {
            return Err(ProofArtifactIdentityError::RegistrationMismatch {
                declaration: id.clone(),
            });
        }
        Ok(ProofArtifactId::new(id.clone(), snapshot, item))
    }

    pub fn external(&self, id: ExternalDeclarationId) -> Option<&ExternalSymbol> {
        match self.symbols.get(&ProjectDeclarationId::External(id))? {
            ProjectSymbol::External(symbol) => Some(symbol),
            ProjectSymbol::Callable(_) | ProjectSymbol::Nominal(_) | ProjectSymbol::Retained(_) => {
                None
            }
        }
    }

    pub fn nominal(&self, id: &ProjectNominalDeclarationId) -> Option<&ProjectNominalDeclaration> {
        match self
            .symbols
            .get(&ProjectDeclarationId::Nominal(id.clone()))?
        {
            ProjectSymbol::Nominal(symbol) => Some(symbol.as_ref()),
            ProjectSymbol::Callable(_)
            | ProjectSymbol::External(_)
            | ProjectSymbol::Retained(_) => None,
        }
    }

    pub fn retained(&self, public_id: &PublicId) -> Option<&ProjectRetainedSymbol> {
        match self
            .symbols
            .get(&ProjectDeclarationId::Retained(public_id.clone()))?
        {
            ProjectSymbol::Retained(symbol) => Some(symbol),
            ProjectSymbol::Callable(_) | ProjectSymbol::External(_) | ProjectSymbol::Nominal(_) => {
                None
            }
        }
    }

    pub fn retained_symbols(&self) -> impl Iterator<Item = &ProjectRetainedSymbol> {
        self.symbols.values().filter_map(|symbol| match symbol {
            ProjectSymbol::Retained(retained) => Some(retained),
            ProjectSymbol::Callable(_) | ProjectSymbol::External(_) | ProjectSymbol::Nominal(_) => {
                None
            }
        })
    }

    /// Resolves one final-HIR path in type position from an accepted module context.
    ///
    /// The path root and segments are consumed directly. There is deliberately
    /// no syntax-path overload: semantic consumers must not reconstruct or
    /// reparse source spelling after final HIR publication.
    pub fn resolve_hir_type_target(
        &self,
        module: &CanonicalModulePath,
        path: &HirPath,
        source: SourceSpan,
    ) -> Result<ProjectTypeTarget<'_>, ProjectTypeLookupError> {
        let reference = imports::linked_path(path)
            .map_err(|error| match error {
                ImportResolutionError::InvalidPath(reason) => ProjectTypeLookupError::InvalidPath {
                    reference: path.clone(),
                    source: source.clone(),
                    reason,
                },
                ImportResolutionError::Unknown
                | ImportResolutionError::Inaccessible(_)
                | ImportResolutionError::VisibilityEscalation
                | ImportResolutionError::Ambiguous(_) => ProjectTypeLookupError::Unknown {
                    module: module.clone(),
                    reference: path.clone(),
                    source: source.clone(),
                },
            })?
            .reference;
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
                return Err(ProjectTypeLookupError::InvalidPath {
                    reference: path.clone(),
                    source,
                    reason,
                });
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
            ProjectSymbolTargetId::Callable(_)
            | ProjectSymbolTargetId::StructuralCallable(_)
            | ProjectSymbolTargetId::Retained(_)
            | ProjectSymbolTargetId::Module(_) => Err(ProjectTypeLookupError::WrongKind {
                reference: path.clone(),
                source,
                actual: Box::new(actual),
            }),
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
                    ProjectSymbolTargetId::Callable(_)
                    | ProjectSymbolTargetId::StructuralCallable(_)
                    | ProjectSymbolTargetId::Retained(_)
                    | ProjectSymbolTargetId::Module(_) => {
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
                        .callable(id)
                        .map(|symbol| symbol.declaration_span().clone()),
                    ProjectSymbolTargetId::StructuralCallable(id) => self
                        .callable(id)
                        .map(|symbol| symbol.declaration_span().clone()),
                    ProjectSymbolTargetId::External(id) => self
                        .external(*id)
                        .map(|symbol| symbol.declaration_span().clone()),
                    ProjectSymbolTargetId::Nominal(id) => self
                        .nominal(id)
                        .map(|symbol| symbol.source().name().clone()),
                    ProjectSymbolTargetId::Retained(id) => self
                        .retained(id)
                        .map(|symbol| symbol.declaration_span().clone()),
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
                        return self.callable(id).map_or_else(
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
            [ProjectSymbolTargetId::Callable(id)] => self.callable(id).map_or_else(
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

    /// Resolves one final-HIR path in the callable value namespace.
    ///
    /// This is the value-position counterpart of
    /// [`Self::resolve_hir_type_target`]. It preserves the final HIR root and
    /// segments and delegates to the same accepted project scope table used by
    /// all other value lookups; semantic consumers never rebuild a syntax path
    /// or reparse source spelling.
    #[allow(
        clippy::result_large_err,
        reason = "value lookup errors retain typed module, path, source, and target evidence"
    )]
    pub fn resolve_hir_value_target(
        &self,
        module: &CanonicalModulePath,
        path: &HirPath,
        source: SourceSpan,
    ) -> Result<ProjectValueLookup<'_>, ProjectValueLookupError> {
        let reference = imports::linked_path(path)
            .map_err(|error| match error {
                ImportResolutionError::InvalidPath(reason) => {
                    ProjectValueLookupError::InvalidPath {
                        reference_source: source.clone(),
                        reason,
                    }
                }
                ImportResolutionError::Unknown
                | ImportResolutionError::Inaccessible(_)
                | ImportResolutionError::VisibilityEscalation
                | ImportResolutionError::Ambiguous(_) => ProjectValueLookupError::InvalidHirPath {
                    reference: path.clone(),
                    reference_source: source.clone(),
                },
            })?
            .reference;
        self.resolve_value_target(module, &reference, source)
    }

    /// Resolves one root-preserving final-HIR path through the sole accepted
    /// project symbol table.
    ///
    /// Unlike [`Self::resolve_hir_value_target`], this projection retains all
    /// project symbol families. Consumers that require a particular family
    /// must treat every other returned family as a typed terminal mismatch;
    /// they must not retry through a fallback namespace.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "the terminal typed lookup consumes one exact source span for structured error evidence"
    )]
    #[allow(
        clippy::result_large_err,
        reason = "lookup failures preserve complete typed path, source, and candidate evidence"
    )]
    pub fn resolve_hir_symbol_target(
        &self,
        module: &CanonicalModulePath,
        path: &HirPath,
        source: SourceSpan,
    ) -> Result<ResolvedProjectSymbol<'_>, ProjectHirSymbolLookupError> {
        let reference = imports::linked_path(path)
            .map_err(|error| match error {
                ImportResolutionError::InvalidPath(reason) => {
                    ProjectHirSymbolLookupError::InvalidPath {
                        reference: path.clone(),
                        site: source.clone(),
                        reason,
                    }
                }
                ImportResolutionError::Unknown
                | ImportResolutionError::Inaccessible(_)
                | ImportResolutionError::VisibilityEscalation
                | ImportResolutionError::Ambiguous(_) => {
                    ProjectHirSymbolLookupError::InvalidHirPath {
                        reference: path.clone(),
                        site: source.clone(),
                    }
                }
            })?
            .reference;
        self.resolve(module, &reference, &source)
            .map_err(ProjectHirSymbolLookupError::from)
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
            [target] => {
                self.resolve_target(target)
                    .ok_or_else(|| ProjectSymbolResolutionError::Unknown {
                        module: module.clone(),
                        reference: reference.clone(),
                        source: source.clone(),
                    })
            }
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

    /// Projects one already-selected table target through the sole symbol
    /// storage owner.  Entity-reference and ordinary symbol resolution share
    /// this projection so neither can grow a second target reader.
    pub(super) fn resolve_target(
        &self,
        target: &ProjectSymbolTargetId,
    ) -> Option<ResolvedProjectSymbol<'_>> {
        match target {
            ProjectSymbolTargetId::Callable(id) => {
                self.callable(id).map(ResolvedProjectSymbol::Callable)
            }
            ProjectSymbolTargetId::StructuralCallable(id) => self
                .callable(id)
                .map(ResolvedProjectSymbol::StructuralCallable),
            ProjectSymbolTargetId::External(id) => {
                self.external(*id).map(ResolvedProjectSymbol::External)
            }
            ProjectSymbolTargetId::Nominal(id) => {
                self.nominal(id).map(ResolvedProjectSymbol::Nominal)
            }
            ProjectSymbolTargetId::Retained(id) => {
                self.retained(id).map(ResolvedProjectSymbol::Retained)
            }
            ProjectSymbolTargetId::Module(path) => {
                self.modules.get(path).map(ResolvedProjectSymbol::Module)
            }
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
            ResolvedProjectSymbol::StructuralCallable(callable) => {
                Err(ProjectSymbolResolutionError::NotCallable {
                    reference: reference.clone(),
                    source: source.clone(),
                    actual: ProjectSymbolTargetId::StructuralCallable(
                        callable.declaration().clone(),
                    ),
                })
            }
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
            ResolvedProjectSymbol::Retained(retained) => {
                Err(ProjectSymbolResolutionError::NotCallable {
                    reference: reference.clone(),
                    source: source.clone(),
                    actual: ProjectSymbolTargetId::Retained(retained.public_id().clone()),
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
        assert_eq!(
            path.root(),
            ModulePathRoot::ImplicitCrate,
            "scope-local project bindings must use the implicit root"
        );
        let mut sites = self.sites.clone();
        extend_sorted_unique_spans(&mut sites, [site]);
        let mut reference_sites = self.reference_sites.clone();
        extend_sorted_unique_spans(&mut reference_sites, reference_site);
        Self {
            path,
            target: self.target.clone(),
            visibility,
            owner: owner.clone(),
            sites,
            reference_sites,
        }
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
            | "Ref"
    )
}

fn coalesce_bindings(mut bindings: Vec<ScopeBinding>) -> Vec<ScopeBinding> {
    bindings.sort_by(compare_scope_binding_identity);
    let mut coalesced: Vec<ScopeBinding> = Vec::with_capacity(bindings.len());
    for binding in bindings {
        if let Some(existing) = coalesced.last_mut()
            && existing.path == binding.path
            && existing.target == binding.target
            && existing.visibility == binding.visibility
            && existing.owner == binding.owner
        {
            extend_sorted_unique_spans(&mut existing.sites, binding.sites);
            extend_sorted_unique_spans(&mut existing.reference_sites, binding.reference_sites);
        } else {
            coalesced.push(binding);
        }
    }
    coalesced
}

fn compare_scope_binding_identity(
    left: &ScopeBinding,
    right: &ScopeBinding,
) -> core::cmp::Ordering {
    left.path
        .cmp(&right.path)
        .then_with(|| left.target.cmp(&right.target))
        .then_with(|| left.visibility.cmp(&right.visibility))
        .then_with(|| left.owner.cmp(&right.owner))
}

fn sort_spans(spans: &mut [SourceSpan]) {
    spans.sort_by(compare_spans);
}

/// Extends canonical provenance without re-sorting the complete accumulated
/// evidence after every import. Authored import order normally takes the fast
/// append path; re-exported evidence retains deterministic ordered insertion.
fn extend_sorted_unique_spans(
    spans: &mut Vec<SourceSpan>,
    additions: impl IntoIterator<Item = SourceSpan>,
) -> bool {
    let mut changed = false;
    for addition in additions {
        if spans
            .last()
            .is_none_or(|last| compare_spans(last, &addition).is_lt())
        {
            spans.push(addition);
            changed = true;
            continue;
        }
        if let Err(index) = spans.binary_search_by(|span| compare_spans(span, &addition)) {
            spans.insert(index, addition);
            changed = true;
        }
    }
    changed
}

fn compare_spans(left: &SourceSpan, right: &SourceSpan) -> core::cmp::Ordering {
    left.source()
        .id()
        .cmp(right.source().id())
        .then_with(|| left.source().revision().cmp(&right.source().revision()))
        .then_with(|| left.range().cmp(&right.range()))
}

fn link_report(
    diagnostics: Vec<ProjectSymbolLinkError>,
    work_charged: u64,
) -> ProjectSymbolLinkReport {
    let mut diagnostics = coalesce_duplicate_declarations(diagnostics);
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

fn coalesce_duplicate_declarations(
    diagnostics: Vec<ProjectSymbolLinkError>,
) -> Vec<ProjectSymbolLinkError> {
    let mut grouped = BTreeMap::<(CanonicalModulePath, String), Vec<SourceSpan>>::new();
    let mut retained = Vec::with_capacity(diagnostics.len());
    for diagnostic in diagnostics {
        match diagnostic {
            ProjectSymbolLinkError::DuplicateDeclaration {
                module,
                name,
                sites,
            } => grouped.entry((module, name)).or_default().extend(sites),
            other => retained.push(other),
        }
    }
    for ((module, name), mut sites) in grouped {
        sort_spans(&mut sites);
        sites.dedup();
        debug_assert!(
            sites.len() >= 2,
            "duplicate-declaration publication always observes at least two source sites"
        );
        retained.push(ProjectSymbolLinkError::DuplicateDeclaration {
            module,
            name,
            sites: sites.into_boxed_slice(),
        });
    }
    retained
}
