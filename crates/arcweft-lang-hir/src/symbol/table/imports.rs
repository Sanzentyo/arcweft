//! Import binding and visibility resolution from final `HirUseBinding` rows.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModulePath, ModulePathRoot, ModuleSegment},
    symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
};

use crate::item::HirUseBindingKind;
use crate::leaf::{HirPath, HirPathRoot, HirPathSegment};

use super::{
    ImportResolutionError, LinkedProjectSymbolPath, ProjectImportRef, ProjectSymbolLimitKind,
    ProjectSymbolLimits, ProjectSymbolLinkError, ProjectSymbolTable, ProjectSymbolTargetId,
    ScopeBinding, coalesce_bindings,
};

impl ProjectSymbolTable {
    pub(super) fn check_import_limits(
        imports: &[ProjectImportRef<'_>],
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
    ) {
        let mut aliases_world = 0_u64;
        let mut aliases_by_module = BTreeMap::<CanonicalModulePath, u64>::new();
        for (index, import) in imports.iter().enumerate() {
            let alias = u64::from(import.binding.alias().is_some());
            aliases_world = aliases_world.saturating_add(alias);
            let module_aliases = aliases_by_module
                .entry(import.module_path.clone())
                .or_default();
            *module_aliases = module_aliases.saturating_add(alias);
            let source = import.whole_source();
            for (kind, observed, maximum) in [
                (
                    ProjectSymbolLimitKind::AliasesPerModule,
                    *module_aliases,
                    ProjectSymbolLimits::PRODUCTION.aliases_per_module(),
                ),
                (
                    ProjectSymbolLimitKind::AliasesPerWorld,
                    aliases_world,
                    ProjectSymbolLimits::PRODUCTION.aliases_per_world(),
                ),
                (
                    ProjectSymbolLimitKind::Imports,
                    u64::try_from(index + 1).unwrap_or(u64::MAX),
                    ProjectSymbolLimits::PRODUCTION.imports(),
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
        }
    }

    pub(super) fn import_bindings(
        &self,
        import: ProjectImportRef<'_>,
    ) -> Result<Vec<ScopeBinding>, ImportResolutionError> {
        let path = linked_path(
            import
                .binding
                .path()
                .as_resolved()
                .ok_or(ImportResolutionError::Unknown)?,
        )?;
        match import.binding.kind() {
            HirUseBindingKind::Item => {
                let targets = self.targets_for_symbol_path(import.module_path, path.reference())?;
                let binding_path = import.binding.alias().map_or_else(
                    || path.unaliased_binding().clone(),
                    |alias| {
                        ProjectSymbolPath::new(
                            ModulePathRoot::ImplicitCrate,
                            [ProjectSymbolSegment::try_new(alias.as_str())
                                .expect("final HIR aliases retain parser-valid names")],
                        )
                        .expect("one use alias is a valid implicit project binding")
                    },
                );
                Self::bind_named_targets(import, &binding_path, targets)
            }
            HirUseBindingKind::Glob => {
                let module_path =
                    self.module_for_symbol_path(import.module_path, path.reference())?;
                let mut bindings = Vec::new();
                if let Some(scope) = self.scopes.get(&module_path) {
                    for candidates in scope.values() {
                        for candidate in candidates.iter().filter(|binding| {
                            Self::binding_visible_from(binding, import.module_path)
                        }) {
                            if Self::can_reexport(candidate.visibility, import.visibility) {
                                bindings.push(candidate.rebound(
                                    candidate.path.clone(),
                                    import.module_path,
                                    import.visibility,
                                    import.whole_source(),
                                    Some(import.path_source()),
                                ));
                            }
                        }
                    }
                }
                (!bindings.is_empty())
                    .then_some(bindings)
                    .ok_or(ImportResolutionError::Unknown)
            }
        }
    }

    fn bind_named_targets(
        import: ProjectImportRef<'_>,
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
            .any(|target| !Self::can_reexport(target.visibility, import.visibility))
        {
            return Err(ImportResolutionError::VisibilityEscalation);
        }
        let site = import.whole_source();
        let reference = import.path_source();
        Ok(targets
            .into_iter()
            .map(|target| {
                target.rebound(
                    path.clone(),
                    import.module_path,
                    import.visibility,
                    site.clone(),
                    Some(reference.clone()),
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

    pub(super) fn inaccessible_bindings_for_symbol_path(
        &self,
        requester: &CanonicalModulePath,
        path: &SymbolPath,
    ) -> Vec<ScopeBinding> {
        if matches!(path.root(), ModulePathRoot::ImplicitCrate) && path.qualifiers().is_empty() {
            return Vec::new();
        }
        let Ok(module) = Self::qualifier_module(requester, path) else {
            return Vec::new();
        };
        self.scopes
            .get(&module)
            .and_then(|scope| scope.get(path.leaf()))
            .into_iter()
            .flatten()
            .filter(|binding| !Self::binding_visible_from(binding, requester))
            .cloned()
            .collect()
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
        match bindings
            .binary_search_by(|existing| super::compare_scope_binding_identity(existing, &binding))
        {
            Ok(index) => {
                let existing = &mut bindings[index];
                let sites_changed =
                    super::extend_sorted_unique_spans(&mut existing.sites, binding.sites);
                let references_changed = super::extend_sorted_unique_spans(
                    &mut existing.reference_sites,
                    binding.reference_sites,
                );
                sites_changed || references_changed
            }
            Err(index) => {
                bindings.insert(index, binding);
                true
            }
        }
    }

    pub(super) fn import_error(
        import: ProjectImportRef<'_>,
        error: ImportResolutionError,
    ) -> ProjectSymbolLinkError {
        let source = import.whole_source();
        let reference = import
            .binding
            .path()
            .as_resolved()
            .expect("recovered use bindings are excluded from the symbol inventory");
        let reference = match linked_path(reference) {
            Ok(path) => path.reference,
            Err(ImportResolutionError::InvalidPath(reason)) => {
                return ProjectSymbolLinkError::InvalidImportPath {
                    module: import.module_path.clone(),
                    source,
                    reason,
                };
            }
            Err(_) => unreachable!("accepted final-HIR use paths have valid symbol segments"),
        };
        match error {
            ImportResolutionError::InvalidPath(reason) => {
                ProjectSymbolLinkError::InvalidImportPath {
                    module: import.module_path.clone(),
                    source,
                    reason,
                }
            }
            ImportResolutionError::Inaccessible(_) => ProjectSymbolLinkError::InaccessibleImport {
                module: import.module_path.clone(),
                import: reference,
                source,
            },
            ImportResolutionError::VisibilityEscalation => {
                ProjectSymbolLinkError::VisibilityEscalation {
                    module: import.module_path.clone(),
                    import: reference,
                    source,
                }
            }
            ImportResolutionError::Ambiguous(mut candidates) => {
                candidates.sort();
                candidates.dedup();
                ProjectSymbolLinkError::AmbiguousImport {
                    module: import.module_path.clone(),
                    import: reference,
                    source,
                    candidates,
                }
            }
            ImportResolutionError::Unknown => ProjectSymbolLinkError::UnknownImport {
                module: import.module_path.clone(),
                import: reference,
                source,
            },
        }
    }
}

pub(super) fn linked_path(
    path: &HirPath,
) -> Result<LinkedProjectSymbolPath, ImportResolutionError> {
    let root = match path.root() {
        HirPathRoot::ImplicitCrate => ModulePathRoot::ImplicitCrate,
        HirPathRoot::Crate => ModulePathRoot::Crate,
        HirPathRoot::SelfModule => ModulePathRoot::SelfModule,
        HirPathRoot::Super { depth } => ModulePathRoot::Super(depth),
    };
    let segments = path
        .segments()
        .iter()
        .map(|segment| match segment {
            HirPathSegment::Identifier(name) => ProjectSymbolSegment::try_new(name.as_str()),
            HirPathSegment::ProjectSymbol(segment) => {
                ProjectSymbolSegment::try_new(segment.as_str())
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ImportResolutionError::Unknown)?;
    let path =
        ProjectSymbolPath::new(root, segments).map_err(|_| ImportResolutionError::Unknown)?;
    LinkedProjectSymbolPath::try_new(&path)
}
