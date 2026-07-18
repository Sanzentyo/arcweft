//! Unified project-symbol table and deterministic fixed-point linker.
//!
//! Binding insertion, ambiguity resolution, work charging, and bounded link
//! reporting stay together because they share one monotone transaction and its
//! ordering invariants. The module remains below the production warning gate.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::ast::{
    common::{TextRange, UseItem, UseTreeKind, Visibility},
    module_path::{
        CanonicalModulePath, ModulePath, ModulePathError, ModulePathRoot, ModuleSegment,
    },
    symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
};
use arcweft_source::{SourceDocumentIdentity, SourceSpan};

use crate::project::HirProject;

use super::{
    CallableDeclarationId, CallableSymbol, ExternalDeclarationId, ExternalDeclarationSeedId,
    ExternalSymbol, ProjectDeclarationId, ProjectExternalDeclarations, ProjectSymbol,
    ProjectSymbolLinkError, ProjectSymbolLinkReport, ProjectSymbolResolutionError,
    ProjectSymbolRevision, ProjectSymbolWorldId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectSymbolLimitKind {
    AliasesPerModule,
    AliasesPerWorld,
    Imports,
    Diagnostics,
    Work,
}

pub struct ProjectSymbolLimits {
    aliases_per_module: u64,
    aliases_per_world: u64,
    imports: u64,
    diagnostics: u64,
    work: u64,
}

impl ProjectSymbolLimits {
    pub const PRODUCTION: Self = Self {
        aliases_per_module: 256,
        aliases_per_world: 8_192,
        imports: 32_768,
        diagnostics: 128,
        work: 262_144,
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
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectSymbolTargetId {
    Callable(CallableDeclarationId),
    External(ExternalDeclarationId),
    Module(CanonicalModulePath),
}

#[derive(Debug)]
pub enum ResolvedProjectSymbol<'a> {
    Callable(&'a CallableSymbol),
    External(&'a ExternalSymbol),
    Module(&'a CanonicalModulePath),
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
    pub(super) scopes: BTreeMap<CanonicalModulePath, BTreeMap<String, Vec<ScopeBinding>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ScopeBinding {
    pub(super) path: ProjectSymbolPath,
    pub(super) target: ProjectSymbolTargetId,
    pub(super) visibility: Option<Visibility>,
    pub(super) owner: CanonicalModulePath,
    pub(super) sites: Vec<SourceSpan>,
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
    Inaccessible,
    VisibilityEscalation,
    Ambiguous(Vec<ProjectSymbolTargetId>),
    InvalidPath(ModulePathError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LinkedProjectSymbolPath {
    reference: SymbolPath,
    unaliased_binding: ProjectSymbolPath,
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
        };
        let mut diagnostics = Vec::new();
        let mut work = 0_u64;

        if let Err(error) = Self::charge(&mut work, 1, None) {
            diagnostics.push(error);
        }
        table.insert_module_bindings(project);
        table.insert_callables(project, &mut diagnostics, &mut work);
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
            for (module, import) in &imports {
                match table.import_bindings(project, module, import) {
                    Ok(_) | Err(ImportResolutionError::Unknown) => {}
                    Err(error) => {
                        diagnostics.push(Self::import_error(project, module, import, error));
                    }
                }
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
            ProjectSymbol::External(_) => None,
        })
    }

    pub fn external_symbols(&self) -> impl Iterator<Item = &ExternalSymbol> {
        self.symbols.values().filter_map(|symbol| match symbol {
            ProjectSymbol::External(external) => Some(external),
            ProjectSymbol::Callable(_) => None,
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
            ProjectSymbol::External(_) => None,
        }
    }

    pub fn external(&self, id: ExternalDeclarationId) -> Option<&ExternalSymbol> {
        match self.symbols.get(&ProjectDeclarationId::External(id))? {
            ProjectSymbol::External(symbol) => Some(symbol),
            ProjectSymbol::Callable(_) => None,
        }
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
                | ImportResolutionError::Inaccessible
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

    fn insert_module_bindings(&mut self, project: &HirProject) {
        for module in self.modules.clone() {
            let Some(name) = module.last_segment() else {
                continue;
            };
            let owner = module
                .parent()
                .unwrap_or_else(CanonicalModulePath::crate_root);
            let site = source_span(project, &module, TextRange::new(0, 0));
            let path = ProjectSymbolPath::new(
                ModulePathRoot::ImplicitCrate,
                [ProjectSymbolSegment::try_new(name)
                    .expect("module segments are valid project symbol segments")],
            )
            .expect("one module segment is a valid implicit project binding");
            self.insert_scope_binding(
                &owner,
                ScopeBinding::new(
                    path,
                    ProjectSymbolTargetId::Module(module),
                    Some(Visibility::Public),
                    owner.clone(),
                    [site],
                ),
            );
        }
    }

    fn insert_callables(
        &mut self,
        project: &HirProject,
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
        work: &mut u64,
    ) {
        for (module_path, module) in project.modules() {
            for function in module.functions() {
                let site = source_span(project, module_path, *function.range());
                if let Err(error) = Self::charge(work, 1, Some(site.clone())) {
                    diagnostics.push(error);
                    return;
                }
                let declaration =
                    match CallableDeclarationId::for_function(self.world.package(), function) {
                        Ok(declaration) => declaration,
                        Err(reason) => {
                            diagnostics.push(ProjectSymbolLinkError::InvalidDeclaration {
                                source: site,
                                reason,
                            });
                            continue;
                        }
                    };
                let name = function.name().to_owned();
                if let Some(first) = self
                    .scopes
                    .get(module_path)
                    .and_then(|scope| scope.get(&name))
                    .and_then(|bindings| bindings.first())
                    .and_then(|binding| binding.sites.first())
                    .cloned()
                {
                    diagnostics.push(ProjectSymbolLinkError::DuplicateDeclaration {
                        module: module_path.clone(),
                        name,
                        first,
                        duplicate: site,
                    });
                    continue;
                }
                let target = ProjectSymbolTargetId::Callable(declaration.clone());
                let path = ProjectSymbolPath::new(
                    ModulePathRoot::ImplicitCrate,
                    [ProjectSymbolSegment::try_new(function.name())
                        .expect("callable declaration names are valid project symbol segments")],
                )
                .expect("one callable name is a valid implicit project binding");
                self.insert_scope_binding(
                    module_path,
                    ScopeBinding::new(
                        path,
                        target,
                        function.visibility(),
                        module_path.clone(),
                        [site.clone()],
                    ),
                );
                self.symbols.insert(
                    ProjectDeclarationId::Callable(declaration.clone()),
                    ProjectSymbol::Callable(CallableSymbol {
                        declaration,
                        visibility: function.visibility(),
                        fx: function.has_attribute("fx"),
                        source: site,
                    }),
                );
            }
        }
    }

    fn insert_externals(
        &mut self,
        externals: &ProjectExternalDeclarations,
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
        work: &mut u64,
    ) -> BTreeMap<ExternalDeclarationSeedId, ExternalDeclarationId> {
        let mut mapping = BTreeMap::new();
        for (seed_id, seed) in externals.declarations() {
            let source = seed.declaration().clone();
            if let Err(error) = Self::charge(work, 1, Some(source)) {
                diagnostics.push(error);
                break;
            }
            let declaration = ExternalDeclarationId::from_index(seed_id.index());
            mapping.insert(seed_id, declaration);
            self.symbols.insert(
                ProjectDeclarationId::External(declaration),
                ProjectSymbol::External(ExternalSymbol::new(declaration, seed)),
            );
            for binding in seed.direct_bindings() {
                self.scopes.entry(binding.module().clone()).or_default();
                self.insert_scope_binding(
                    binding.module(),
                    ScopeBinding::new(
                        binding.path().clone(),
                        ProjectSymbolTargetId::External(declaration),
                        binding.visibility(),
                        binding.module().clone(),
                        [binding.source().clone()],
                    ),
                );
            }
        }
        mapping
    }

    fn check_import_limits(
        project: &HirProject,
        imports: &[(CanonicalModulePath, &UseItem)],
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
    ) {
        let mut aliases_world = 0_u64;
        let mut import_count = 0_u64;
        let mut aliases_by_module = BTreeMap::<CanonicalModulePath, u64>::new();
        for (module, import) in imports {
            let (imports_in_tree, aliases_in_tree) = use_counts(import);
            import_count = import_count.saturating_add(imports_in_tree);
            aliases_world = aliases_world.saturating_add(aliases_in_tree);
            let module_aliases = aliases_by_module.entry(module.clone()).or_default();
            *module_aliases = module_aliases
                .checked_add(aliases_in_tree)
                .unwrap_or(u64::MAX);
            let source = source_span(project, module, *import.range());
            if *module_aliases > ProjectSymbolLimits::PRODUCTION.aliases_per_module() {
                diagnostics.push(ProjectSymbolLinkError::Limit {
                    kind: ProjectSymbolLimitKind::AliasesPerModule,
                    observed: *module_aliases,
                    maximum: ProjectSymbolLimits::PRODUCTION.aliases_per_module(),
                    source: Some(source.clone()),
                });
            }
            if aliases_world > ProjectSymbolLimits::PRODUCTION.aliases_per_world() {
                diagnostics.push(ProjectSymbolLinkError::Limit {
                    kind: ProjectSymbolLimitKind::AliasesPerWorld,
                    observed: aliases_world,
                    maximum: ProjectSymbolLimits::PRODUCTION.aliases_per_world(),
                    source: Some(source.clone()),
                });
            }
            if import_count > ProjectSymbolLimits::PRODUCTION.imports() {
                diagnostics.push(ProjectSymbolLinkError::Limit {
                    kind: ProjectSymbolLimitKind::Imports,
                    observed: import_count,
                    maximum: ProjectSymbolLimits::PRODUCTION.imports(),
                    source: Some(source),
                });
            }
        }
    }

    fn import_bindings(
        &self,
        project: &HirProject,
        importer: &CanonicalModulePath,
        import: &UseItem,
    ) -> Result<Vec<ScopeBinding>, ImportResolutionError> {
        match import.tree().kind() {
            UseTreeKind::Path { path, alias } => {
                let path = LinkedProjectSymbolPath::try_new(path.path())?;
                let targets = self.targets_for_symbol_path(importer, path.reference())?;
                let binding_path = alias.as_ref().map_or_else(
                    || path.unaliased_binding().clone(),
                    |alias| {
                        ProjectSymbolPath::new(
                            ModulePathRoot::ImplicitCrate,
                            [ProjectSymbolSegment::try_new(alias.name().as_str())
                                .expect("use aliases are valid project symbol segments")],
                        )
                        .expect("one use alias is a valid implicit project binding")
                    },
                );
                Self::bind_named_targets(project, importer, import, &binding_path, targets)
            }
            UseTreeKind::Glob { module } => {
                let path = LinkedProjectSymbolPath::try_new(module.path())?;
                let module_path = self.module_for_symbol_path(importer, path.reference())?;
                let site = source_span(project, importer, module.range());
                let mut bindings = Vec::new();
                if let Some(scope) = self.scopes.get(&module_path) {
                    for candidates in scope.values() {
                        for candidate in candidates
                            .iter()
                            .filter(|binding| Self::binding_visible_from(binding, importer))
                        {
                            if Self::can_reexport(candidate.visibility, import.visibility()) {
                                bindings.push(candidate.rebound(
                                    candidate.path.clone(),
                                    importer,
                                    import.visibility(),
                                    site.clone(),
                                ));
                            }
                        }
                    }
                }
                (!bindings.is_empty())
                    .then_some(bindings)
                    .ok_or(ImportResolutionError::Unknown)
            }
            UseTreeKind::Group { module, names } => {
                let module = LinkedProjectSymbolPath::try_new(module.path())?;
                let mut bindings = Vec::new();
                for selected in names {
                    let path = append_leaf_qualifier(module.reference(), selected.name())?;
                    let targets = match self.targets_for_symbol_path(importer, &path) {
                        Ok(targets) => targets,
                        Err(ImportResolutionError::Unknown) => continue,
                        Err(error) => return Err(error),
                    };
                    let binding_path = selected.alias().map_or_else(
                        || {
                            ProjectSymbolPath::new(
                                ModulePathRoot::ImplicitCrate,
                                [selected.name().clone()],
                            )
                            .expect("one selected name is a valid implicit project binding")
                        },
                        |alias| {
                            ProjectSymbolPath::new(
                                ModulePathRoot::ImplicitCrate,
                                [ProjectSymbolSegment::try_new(alias.name().as_str())
                                    .expect("use aliases are valid project symbol segments")],
                            )
                            .expect("one use alias is a valid implicit project binding")
                        },
                    );
                    bindings.extend(Self::bind_named_targets(
                        project,
                        importer,
                        import,
                        &binding_path,
                        targets,
                    )?);
                }
                (!bindings.is_empty())
                    .then_some(bindings)
                    .ok_or(ImportResolutionError::Unknown)
            }
        }
    }

    fn bind_named_targets(
        project: &HirProject,
        importer: &CanonicalModulePath,
        import: &UseItem,
        path: &ProjectSymbolPath,
        targets: Vec<ScopeBinding>,
    ) -> Result<Vec<ScopeBinding>, ImportResolutionError> {
        let distinct = targets
            .iter()
            .map(|binding| binding.target.clone())
            .collect::<BTreeSet<_>>();
        if distinct.len() > 1 {
            return Err(ImportResolutionError::Ambiguous(
                distinct.into_iter().collect(),
            ));
        }
        if targets
            .iter()
            .any(|target| !Self::can_reexport(target.visibility, import.visibility()))
        {
            return Err(ImportResolutionError::VisibilityEscalation);
        }
        let site = source_span(project, importer, *import.range());
        Ok(targets
            .into_iter()
            .map(|target| target.rebound(path.clone(), importer, import.visibility(), site.clone()))
            .collect())
    }

    fn targets_for_symbol_path(
        &self,
        requester: &CanonicalModulePath,
        path: &SymbolPath,
    ) -> Result<Vec<ScopeBinding>, ImportResolutionError> {
        if matches!(path.root(), ModulePathRoot::ImplicitCrate) && path.qualifiers().is_empty() {
            let targets = self.visible_bindings(requester, requester, path.leaf());
            return (!targets.is_empty())
                .then_some(targets)
                .ok_or(ImportResolutionError::Unknown);
        }

        if matches!(path.root(), ModulePathRoot::ImplicitCrate) {
            let canonical = path.canonical_string();
            let root_targets =
                self.visible_bindings(requester, &CanonicalModulePath::crate_root(), &canonical);
            if !root_targets.is_empty() {
                return Ok(root_targets);
            }
        }

        let module = Self::qualifier_module(requester, path)?;
        let mut targets = self.visible_bindings(requester, &module, path.leaf());
        if let Ok(segment) = ModuleSegment::new(path.leaf()) {
            let child = module.join(segment);
            if self.modules.contains(&child) {
                let binding_path = ProjectSymbolPath::new(
                    ModulePathRoot::ImplicitCrate,
                    [ProjectSymbolSegment::try_new(path.leaf())
                        .expect("module leaves are valid project symbol segments")],
                )
                .expect("one module leaf is a valid implicit project binding");
                targets.push(ScopeBinding::new(
                    binding_path,
                    ProjectSymbolTargetId::Module(child),
                    Some(Visibility::Public),
                    module.clone(),
                    [],
                ));
            }
        }
        if !targets.is_empty() {
            return Ok(coalesce_bindings(targets));
        }
        let exists = self
            .scopes
            .get(&module)
            .and_then(|scope| scope.get(path.leaf()))
            .is_some_and(|bindings| !bindings.is_empty());
        if exists {
            Err(ImportResolutionError::Inaccessible)
        } else {
            Err(ImportResolutionError::Unknown)
        }
    }

    fn qualifier_module(
        requester: &CanonicalModulePath,
        path: &SymbolPath,
    ) -> Result<CanonicalModulePath, ImportResolutionError> {
        ModulePath::new(path.root(), path.qualifiers().iter().cloned())
            .map_err(ImportResolutionError::InvalidPath)?
            .resolve_from(requester)
            .map_err(ImportResolutionError::InvalidPath)
    }

    fn module_for_symbol_path(
        &self,
        requester: &CanonicalModulePath,
        path: &SymbolPath,
    ) -> Result<CanonicalModulePath, ImportResolutionError> {
        let leaf = ModuleSegment::new(path.leaf()).map_err(ImportResolutionError::InvalidPath)?;
        let module = ModulePath::new(path.root(), path.qualifiers().iter().cloned().chain([leaf]))
            .map_err(ImportResolutionError::InvalidPath)?
            .resolve_from(requester)
            .map_err(ImportResolutionError::InvalidPath)?;
        self.modules
            .contains(&module)
            .then_some(module)
            .ok_or(ImportResolutionError::Unknown)
    }

    fn visible_bindings(
        &self,
        requester: &CanonicalModulePath,
        module: &CanonicalModulePath,
        name: &str,
    ) -> Vec<ScopeBinding> {
        self.scopes
            .get(module)
            .and_then(|scope| scope.get(name))
            .into_iter()
            .flatten()
            .filter(|binding| Self::binding_visible_from(binding, requester))
            .cloned()
            .collect()
    }

    fn binding_visible_from(binding: &ScopeBinding, requester: &CanonicalModulePath) -> bool {
        if requester == &binding.owner {
            return true;
        }
        match binding.visibility {
            Some(Visibility::Public | Visibility::Crate) => true,
            Some(Visibility::Super) => {
                let parent = binding
                    .owner
                    .parent()
                    .unwrap_or_else(CanonicalModulePath::crate_root);
                requester.segments().starts_with(parent.segments())
            }
            None => false,
        }
    }

    fn can_reexport(target: Option<Visibility>, requested: Option<Visibility>) -> bool {
        match requested {
            None => true,
            Some(Visibility::Public) => matches!(target, Some(Visibility::Public)),
            Some(Visibility::Crate) => {
                matches!(target, Some(Visibility::Public | Visibility::Crate))
            }
            Some(Visibility::Super) => matches!(
                target,
                Some(Visibility::Public | Visibility::Crate | Visibility::Super)
            ),
        }
    }

    fn insert_scope_binding(
        &mut self,
        module: &CanonicalModulePath,
        binding: ScopeBinding,
    ) -> bool {
        let lookup_key = binding.path.to_string();
        let bindings = self
            .scopes
            .entry(module.clone())
            .or_default()
            .entry(lookup_key)
            .or_default();
        let changed = if let Some(existing) = bindings.iter_mut().find(|existing| {
            existing.path == binding.path
                && existing.target == binding.target
                && existing.visibility == binding.visibility
                && existing.owner == binding.owner
        }) {
            let old_len = existing.sites.len();
            existing.sites.extend(binding.sites);
            sort_spans(&mut existing.sites);
            existing.sites.dedup();
            existing.sites.len() != old_len
        } else {
            bindings.push(binding);
            true
        };
        bindings.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.target.cmp(&right.target))
                .then_with(|| left.visibility.cmp(&right.visibility))
                .then_with(|| left.owner.cmp(&right.owner))
        });
        changed
    }

    fn import_error(
        project: &HirProject,
        module: &CanonicalModulePath,
        import: &UseItem,
        error: ImportResolutionError,
    ) -> ProjectSymbolLinkError {
        let source = source_span(project, module, *import.range());
        let import_path = match import.tree().kind() {
            UseTreeKind::Path { path, .. } => {
                LinkedProjectSymbolPath::try_new(path.path()).map(|path| path.reference().clone())
            }
            UseTreeKind::Glob { module } | UseTreeKind::Group { module, .. } => {
                LinkedProjectSymbolPath::try_new(module.path()).map(|path| path.reference().clone())
            }
        };
        match (import_path, error) {
            (Err(ImportResolutionError::InvalidPath(reason)), _)
            | (_, ImportResolutionError::InvalidPath(reason)) => {
                ProjectSymbolLinkError::InvalidImportPath {
                    module: module.clone(),
                    source,
                    reason,
                }
            }
            (Ok(import), ImportResolutionError::Inaccessible) => {
                ProjectSymbolLinkError::InaccessibleImport {
                    module: module.clone(),
                    import,
                    source,
                }
            }
            (Ok(import), ImportResolutionError::VisibilityEscalation) => {
                ProjectSymbolLinkError::VisibilityEscalation {
                    module: module.clone(),
                    import,
                    source,
                }
            }
            (Ok(import), ImportResolutionError::Ambiguous(mut candidates)) => {
                candidates.sort();
                candidates.dedup();
                ProjectSymbolLinkError::AmbiguousImport {
                    module: module.clone(),
                    import,
                    source,
                    candidates,
                }
            }
            (Ok(_), ImportResolutionError::Unknown) => {
                unreachable!("unknown imports are deliberately omitted from link reports")
            }
            (Err(_), _) => unreachable!("validated syntax paths fail only with ModulePathError"),
        }
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
        }
    }

    fn rebound(
        &self,
        path: ProjectSymbolPath,
        owner: &CanonicalModulePath,
        visibility: Option<Visibility>,
        site: SourceSpan,
    ) -> Self {
        Self::new(path, self.target.clone(), visibility, owner.clone(), [site])
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
