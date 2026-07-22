//! Import binding and visibility resolution for the project symbol table.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::ast::{
    common::{UseItem, UseTreeKind, Visibility},
    module_path::{CanonicalModulePath, ModulePath, ModulePathRoot, ModuleSegment},
    symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
};

use crate::project::HirProject;

use super::{
    ImportResolutionError, LinkedProjectSymbolPath, ProjectSymbolLimitKind, ProjectSymbolLimits,
    ProjectSymbolLinkError, ProjectSymbolTable, ProjectSymbolTargetId, ScopeBinding,
    append_leaf_qualifier, coalesce_bindings, sort_spans, source_span, use_counts,
};

impl ProjectSymbolTable {
    pub(super) fn check_import_limits(
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

    pub(super) fn import_bindings(
        &self,
        project: &HirProject,
        importer: &CanonicalModulePath,
        import: &UseItem,
    ) -> Result<Vec<ScopeBinding>, ImportResolutionError> {
        match import.tree().kind() {
            UseTreeKind::Path { path, alias } => {
                let reference_site = path
                    .segment_ranges()
                    .last()
                    .copied()
                    .map(|range| source_span(project, importer, range));
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
                Self::bind_named_targets(
                    project,
                    importer,
                    import,
                    &binding_path,
                    targets,
                    reference_site.as_ref(),
                )
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
                                    None,
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
                    let targets = self.targets_for_symbol_path(importer, &path)?;
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
                    let reference_site = source_span(project, importer, selected.name_range());
                    bindings.extend(Self::bind_named_targets(
                        project,
                        importer,
                        import,
                        &binding_path,
                        targets,
                        Some(&reference_site),
                    )?);
                }
                (!bindings.is_empty())
                    .then_some(bindings)
                    .ok_or(ImportResolutionError::Unknown)
            }
        }
    }

    pub(super) fn bind_named_targets(
        project: &HirProject,
        importer: &CanonicalModulePath,
        import: &UseItem,
        path: &ProjectSymbolPath,
        targets: Vec<ScopeBinding>,
        reference_site: Option<&arcweft_source::SourceSpan>,
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
            .map(|target| {
                target.rebound(
                    path.clone(),
                    importer,
                    import.visibility(),
                    site.clone(),
                    reference_site.cloned(),
                )
            })
            .collect())
    }

    pub(super) fn targets_for_symbol_path(
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
            let inaccessible = self
                .scopes
                .get(&module)
                .and_then(|scope| scope.get(path.leaf()))
                .into_iter()
                .flatten()
                .cloned()
                .collect();
            Err(ImportResolutionError::Inaccessible(inaccessible))
        } else {
            Err(ImportResolutionError::Unknown)
        }
    }

    pub(super) fn qualifier_module(
        requester: &CanonicalModulePath,
        path: &SymbolPath,
    ) -> Result<CanonicalModulePath, ImportResolutionError> {
        ModulePath::new(path.root(), path.qualifiers().iter().cloned())
            .map_err(ImportResolutionError::InvalidPath)?
            .resolve_from(requester)
            .map_err(ImportResolutionError::InvalidPath)
    }

    pub(super) fn module_for_symbol_path(
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

    pub(super) fn visible_bindings(
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

    pub(super) fn binding_visible_from(
        binding: &ScopeBinding,
        requester: &CanonicalModulePath,
    ) -> bool {
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

    pub(super) fn can_reexport(target: Option<Visibility>, requested: Option<Visibility>) -> bool {
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

    pub(super) fn insert_scope_binding(
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

    pub(super) fn import_error(
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
            (Ok(import), ImportResolutionError::Inaccessible(_)) => {
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
            (Ok(import), ImportResolutionError::Unknown) => ProjectSymbolLinkError::UnknownImport {
                module: module.clone(),
                import,
                source,
            },
            (Err(_), _) => unreachable!("validated syntax paths fail only with ModulePathError"),
        }
    }
}
