//! Publication of admitted module, callable, nominal, and external declarations.

use std::collections::BTreeMap;

use crate::{
    model::{HirModule, HirTopLevelDecl},
    project::HirProject,
};
use arcweft_lang_syntax::ast::{
    common::{TextRange, Visibility},
    module_path::{CanonicalModulePath, ModulePathRoot},
    symbol_path::{ProjectSymbolPath, ProjectSymbolSegment},
};

use super::super::nominal::ProjectNominalDeclarationError;
use super::nominal;
use super::{
    CallableDeclarationId, CallableSymbol, ExternalDeclarationId, ExternalDeclarationSeedId,
    ExternalSymbol, ProjectDeclarationId, ProjectExternalDeclarations, ProjectSymbol,
    ProjectSymbolLimitKind, ProjectSymbolLimits, ProjectSymbolLinkError, ProjectSymbolTable,
    ProjectSymbolTargetId, ScopeBinding, is_reserved_type_name, source_span,
};

impl ProjectSymbolTable {
    pub(super) fn insert_module_bindings(&mut self, project: &HirProject) {
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

    pub(super) fn insert_callables(
        &mut self,
        project: &HirProject,
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
        work: &mut u64,
    ) {
        for (module_path, module) in project.modules() {
            for function in module.functions() {
                let site = source_span(project, module_path, *function.range());
                let name_site =
                    source_span(project, module_path, function.signature_source().name());
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
                if is_reserved_type_name(&name) {
                    diagnostics.push(ProjectSymbolLinkError::ReservedTypeName {
                        module: module_path.clone(),
                        name,
                        source: name_site,
                    });
                    continue;
                }
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
                        duplicate: name_site,
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
                        [name_site],
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

    pub(super) fn insert_nominals(
        &mut self,
        project: &HirProject,
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
        work: &mut u64,
    ) {
        let mut world_count = 0_u64;
        for (module_path, module) in project.modules() {
            let mut module_count = 0_u64;
            for declaration in module.declarations() {
                let syntax = match declaration {
                    HirTopLevelDecl::Struct(item) => nominal::NominalSyntax::Struct(item),
                    HirTopLevelDecl::Enum(item) => nominal::NominalSyntax::Enum(item),
                    HirTopLevelDecl::TypeAlias(item) => nominal::NominalSyntax::TypeAlias(item),
                    HirTopLevelDecl::Trait(_)
                    | HirTopLevelDecl::Impl(_)
                    | HirTopLevelDecl::EntityDecl(_)
                    | HirTopLevelDecl::Entry(_)
                    | HirTopLevelDecl::ExternCapability(_)
                    | HirTopLevelDecl::Proof(_)
                    | HirTopLevelDecl::Test(_)
                    | HirTopLevelDecl::Bench(_)
                    | HirTopLevelDecl::Source(_)
                    | HirTopLevelDecl::Style(_) => continue,
                };
                let source = source_span(project, module_path, syntax.range());
                module_count = module_count.saturating_add(1);
                world_count = world_count.saturating_add(1);
                for (kind, observed, maximum) in [
                    (
                        ProjectSymbolLimitKind::NominalDeclarationsPerModule,
                        module_count,
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
                if module_count > ProjectSymbolLimits::PRODUCTION.nominal_declarations_per_module()
                    || world_count
                        > ProjectSymbolLimits::PRODUCTION.nominal_declarations_per_world()
                {
                    continue;
                }
                if let Err(error) =
                    Self::charge(work, syntax.link_work_units(), Some(source.clone()))
                {
                    diagnostics.push(error);
                    return;
                }
                self.insert_nominal_declaration(module_path, module, syntax, source, diagnostics);
            }
        }
    }

    fn insert_nominal_declaration(
        &mut self,
        module_path: &CanonicalModulePath,
        module: &HirModule,
        syntax: nominal::NominalSyntax<'_>,
        source: arcweft_source::SourceSpan,
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
    ) {
        let nominal = match nominal::build_nominal_declaration(
            syntax,
            module,
            module_path,
            self.world.clone(),
            self.revision,
        ) {
            Ok(nominal) => nominal,
            Err(ProjectNominalDeclarationError::Limit {
                kind,
                observed,
                maximum,
                source,
            }) => {
                diagnostics.push(ProjectSymbolLinkError::Limit {
                    kind,
                    observed,
                    maximum,
                    source: Some(source),
                });
                return;
            }
            Err(reason) => {
                diagnostics.push(ProjectSymbolLinkError::InvalidNominalDeclaration {
                    source,
                    reason: Box::new(reason),
                });
                return;
            }
        };
        let name = nominal.id().name().as_str();
        if is_reserved_type_name(name) {
            diagnostics.push(ProjectSymbolLinkError::ReservedTypeName {
                module: module_path.clone(),
                name: name.to_owned(),
                source: nominal.source().name().clone(),
            });
            return;
        }
        if let Some(first) = self
            .scopes
            .get(module_path)
            .and_then(|scope| scope.get(name))
            .and_then(|bindings| bindings.first())
            .and_then(|binding| binding.sites.first())
            .cloned()
        {
            diagnostics.push(ProjectSymbolLinkError::DuplicateDeclaration {
                module: module_path.clone(),
                name: name.to_owned(),
                first,
                duplicate: nominal.source().name().clone(),
            });
            return;
        }
        let id = nominal.id().clone();
        let target = ProjectSymbolTargetId::Nominal(id.clone());
        let path = ProjectSymbolPath::new(
            ModulePathRoot::ImplicitCrate,
            [ProjectSymbolSegment::try_new(name)
                .expect("nominal names are valid project symbol segments")],
        )
        .expect("one nominal name is a valid implicit project binding");
        self.insert_scope_binding(
            module_path,
            ScopeBinding::new(
                path,
                target,
                nominal.visibility(),
                module_path.clone(),
                [nominal.source().name().clone()],
            ),
        );
        self.nominal_ids.insert(id.clone());
        self.symbols.insert(
            ProjectDeclarationId::Nominal(id),
            ProjectSymbol::Nominal(Box::new(nominal)),
        );
    }

    pub(super) fn insert_externals(
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
                let binding_name = binding.path().last_segment().as_str();
                if binding.path().segments().len() == 1 && is_reserved_type_name(binding_name) {
                    diagnostics.push(ProjectSymbolLinkError::ReservedTypeName {
                        module: binding.module().clone(),
                        name: binding_name.to_owned(),
                        source: binding.source().clone(),
                    });
                    continue;
                }
                if let Some(first) = self
                    .scopes
                    .get(binding.module())
                    .and_then(|scope| scope.get(&binding.path().to_string()))
                    .and_then(|bindings| {
                        bindings.iter().find(|existing| {
                            existing.target != ProjectSymbolTargetId::External(declaration)
                        })
                    })
                    .and_then(|binding| binding.sites.first())
                    .cloned()
                {
                    diagnostics.push(ProjectSymbolLinkError::DuplicateDeclaration {
                        module: binding.module().clone(),
                        name: binding_name.to_owned(),
                        first,
                        duplicate: binding.source().clone(),
                    });
                    continue;
                }
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
}
