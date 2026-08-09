//! Publication of final-HIR module, callable, nominal, and external declarations.

use std::collections::BTreeMap;

use arcweft_id::{DeclarationIdentityFamily, DeclarationName, PublicId};
use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModulePathRoot, ModuleSegment},
    symbol_path::{ProjectSymbolPath, ProjectSymbolSegment},
};
use arcweft_source::SourceSpan;

use crate::identity::ItemId;
use crate::item::{
    HirCapabilityMember, HirFlowIdentity, HirImplMember, HirItem, HirItemKind, HirItemPrefix,
    HirTraitMember, HirVisibility,
};
use crate::leaf::HirIdRef;
use crate::module::HirModuleStatus;
use crate::project::HirProjectView;
use crate::source_index::{
    HirCallableSourceOwner, HirCallableSourceRole, HirDeclarationSourceRole, HirFlowSourceRole,
    HirItemSourceRole,
};

use super::super::nominal::ProjectNominalDeclarationError;
use super::nominal::{self, NominalHir, NominalModuleView};
use crate::symbol::{
    CallableDeclarationId, CallableDeclarationKey, CallableDeclarationOwner, CallableSymbol,
    ExternalDeclarationId, ExternalDeclarationSeedId, ExternalSymbol, FlowDeclarationId,
    FlowPublicationKind, ImplDeclarationId, ImplMethodDeclarationId, ImplMethodKind,
    ProjectDeclarationId, ProjectExternalDeclarations, ProjectSymbol, ProjectSymbolLimitKind,
    ProjectSymbolLimits, ProjectSymbolLinkError, ProjectSymbolTable, ProjectSymbolTargetId,
    TraitDeclarationId, TraitMethodRequirementId,
};

use super::{ProjectSymbolModuleView, ScopeBinding, is_reserved_type_name};

impl ProjectSymbolTable {
    pub(super) fn insert_module_bindings(&mut self, project: HirProjectView<'_>) {
        for (module, hir) in project.modules() {
            let Some(name) = module.last_segment() else {
                continue;
            };
            let owner = module
                .parent()
                .unwrap_or_else(CanonicalModulePath::crate_root);
            let site = hir
                .provenance()
                .document()
                .span(arcweft_source::SourceRange::new(0, 0))
                .expect("zero-width module binding site is in bounds");
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
                    ProjectSymbolTargetId::Module(module.clone()),
                    Some(Visibility::Public),
                    owner.clone(),
                    [site],
                ),
            );
        }
    }

    pub(super) fn insert_callables(
        &mut self,
        project: HirProjectView<'_>,
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
        work: &mut u64,
    ) {
        let mut impl_ordinals = BTreeMap::<CanonicalModulePath, u32>::new();
        for item_ref in project.items() {
            let module_path = item_ref.module_path();
            let impl_ordinal = impl_ordinals.entry(module_path.clone()).or_default();
            if !self.insert_item_callables(
                module_path,
                ProjectSymbolModuleView::Published(item_ref.module()),
                item_ref.id(),
                item_ref.item(),
                impl_ordinal,
                item_ref.module().status() == HirModuleStatus::Clean,
                diagnostics,
                work,
            ) {
                return;
            }
        }
    }

    /// Publishes every callable identity owned by one already-lowered source
    /// item. Both the paused Proof-return header view and the final project
    /// view use this exact traversal, so final registration cannot silently
    /// gain callable families that were absent from the pre-publication
    /// symbol authority.
    #[allow(
        clippy::too_many_arguments,
        reason = "one item publication binds its exact module/source identity and shared accounting transaction"
    )]
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive publication pass assigns every callable family from the same typed item owner"
    )]
    pub(super) fn insert_item_callables(
        &mut self,
        module_path: &CanonicalModulePath,
        module: ProjectSymbolModuleView<'_, '_>,
        source_item: ItemId,
        item: &HirItem,
        impl_ordinal: &mut u32,
        executable_module: bool,
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
        work: &mut u64,
    ) -> bool {
        if let HirItemKind::Flow(flow) = item.kind()
            && !self.insert_flow_symbol(
                module_path,
                module,
                source_item,
                item,
                flow.identity(),
                executable_module && !item.is_poisoned(),
                diagnostics,
                work,
            )
        {
            return false;
        }

        let ordinary = match item.kind() {
            HirItemKind::Function(function) => Some((
                CallableDeclarationOwner::Function,
                function.name(),
                has_fx_attribute(item),
            )),
            HirItemKind::Predicate(predicate) => Some((
                CallableDeclarationOwner::Predicate,
                predicate.name(),
                has_fx_attribute(item),
            )),
            HirItemKind::Proof(proof) => Some((
                CallableDeclarationOwner::Proof,
                proof.name(),
                has_fx_attribute(item),
            )),
            _ => None,
        };
        if let Some((declaration_owner, name, fx)) = ordinary
            && let Some(name) = name.resolved()
        {
            let whole = declaration_span(module, source_item, HirDeclarationSourceRole::Whole);
            let name_site = declaration_span(module, source_item, HirDeclarationSourceRole::Name);
            let path = [ProjectSymbolSegment::try_new(name.as_str())
                .expect("resolved HIR names are project symbol segments")];
            if !self.insert_callable_symbol(
                module_path,
                module,
                source_item,
                HirCallableSourceOwner::Item,
                declaration_owner,
                std::iter::empty(),
                name.as_str(),
                ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, path)
                    .expect("one resolved callable segment is a valid binding"),
                visibility(item),
                fx,
                whole.clone(),
                name_site,
                executable_module && !item.is_poisoned(),
                diagnostics,
                work,
            ) {
                return false;
            }
        }

        match item.kind() {
            HirItemKind::Trait(declaration) => {
                if let Some(name) = declaration.name().resolved() {
                    let trait_declaration = TraitDeclarationId::new(
                        self.world.package().clone(),
                        module_path.clone(),
                        ModuleSegment::new(name.as_str())
                            .expect("resolved Trait names are module segments"),
                    );
                    for (position, member) in declaration.members().iter().enumerate() {
                        let HirTraitMember::Function(function) = member else {
                            continue;
                        };
                        let member = u16::try_from(position).expect(
                            "accepted declaration-member limit fits callable source ordinals",
                        );
                        let Some(method) = function.name().resolved() else {
                            continue;
                        };
                        let source_owner = HirCallableSourceOwner::TraitFunction { member };
                        let key = CallableDeclarationKey::TraitRequirement(
                            TraitMethodRequirementId::new(
                                trait_declaration.clone(),
                                ModuleSegment::new(method.as_str())
                                    .expect("resolved Trait method names are module segments"),
                            ),
                        );
                        if !self.insert_method_callable_symbol(
                            module_path,
                            module,
                            source_item,
                            source_owner,
                            key,
                            visibility(item),
                            has_fx_attribute_prefix(function.prefix()),
                            callable_span(
                                module,
                                source_item,
                                HirCallableSourceRole::Signature {
                                    owner: source_owner,
                                },
                            ),
                            callable_span(
                                module,
                                source_item,
                                HirCallableSourceRole::Name {
                                    owner: source_owner,
                                },
                            ),
                            executable_module
                                && !item.is_poisoned()
                                && !function.name().is_recovered(),
                            diagnostics,
                            work,
                        ) {
                            return false;
                        }
                    }
                }
            }
            HirItemKind::Impl(declaration) => {
                let implementation = ImplDeclarationId::new(
                    self.world.package().clone(),
                    module_path.clone(),
                    *impl_ordinal,
                );
                *impl_ordinal = impl_ordinal
                    .checked_add(1)
                    .expect("accepted Impl declaration limit fits u32");
                let kind = if declaration.trait_ref().is_some() {
                    ImplMethodKind::Trait
                } else {
                    ImplMethodKind::Inherent
                };
                for (position, member) in declaration.members().iter().enumerate() {
                    let HirImplMember::Function(function) = member else {
                        continue;
                    };
                    let member = u16::try_from(position)
                        .expect("accepted declaration-member limit fits callable source ordinals");
                    let Some(method) = function.name().resolved() else {
                        continue;
                    };
                    let source_owner = HirCallableSourceOwner::ImplFunction { member };
                    let key = CallableDeclarationKey::ImplMethod(ImplMethodDeclarationId::new(
                        implementation.clone(),
                        kind,
                        ModuleSegment::new(method.as_str())
                            .expect("resolved Impl method names are module segments"),
                    ));
                    if !self.insert_method_callable_symbol(
                        module_path,
                        module,
                        source_item,
                        source_owner,
                        key,
                        None,
                        has_fx_attribute_prefix(function.prefix()),
                        callable_span(
                            module,
                            source_item,
                            HirCallableSourceRole::Signature {
                                owner: source_owner,
                            },
                        ),
                        callable_span(
                            module,
                            source_item,
                            HirCallableSourceRole::Name {
                                owner: source_owner,
                            },
                        ),
                        executable_module && !item.is_poisoned() && !function.name().is_recovered(),
                        diagnostics,
                        work,
                    ) {
                        return false;
                    }
                }
            }
            _ => {}
        }

        let HirItemKind::ExternCapability(capability) = item.kind() else {
            return true;
        };
        let Some(capability_name) = capability.name().resolved() else {
            return true;
        };
        let capability_segment = ModuleSegment::new(capability_name.as_str())
            .expect("resolved HIR names are module segments");
        for (position, member) in capability.members().iter().enumerate() {
            let HirCapabilityMember::Function(function) = member else {
                continue;
            };
            let member = u16::try_from(position)
                .expect("accepted declaration-member limit fits callable source member ordinals");
            let source_owner = HirCallableSourceOwner::ExternCapabilityFunction { member };
            let Some(name) = function.name().resolved() else {
                continue;
            };
            let name_site = callable_span(
                module,
                source_item,
                HirCallableSourceRole::Name {
                    owner: source_owner,
                },
            );
            let signature = callable_span(
                module,
                source_item,
                HirCallableSourceRole::Signature {
                    owner: source_owner,
                },
            );
            let path = ProjectSymbolPath::new(
                ModulePathRoot::ImplicitCrate,
                [
                    ProjectSymbolSegment::try_new(capability_name.as_str())
                        .expect("resolved capability names are project symbol segments"),
                    ProjectSymbolSegment::try_new(name.as_str())
                        .expect("resolved function names are project symbol segments"),
                ],
            )
            .expect("capability function path is a valid project binding");
            if !self.insert_callable_symbol(
                module_path,
                module,
                source_item,
                source_owner,
                CallableDeclarationOwner::ExternCapability,
                [capability_segment.clone()],
                name.as_str(),
                path,
                prefix_visibility(function.prefix()).or_else(|| visibility(item)),
                has_fx_attribute_prefix(function.prefix()),
                signature,
                name_site,
                executable_module && !item.is_poisoned() && !function.name().is_recovered(),
                diagnostics,
                work,
            ) {
                return false;
            }
        }
        true
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Flow structural publication binds the accepted identity and exact HIR owner atomically"
    )]
    fn insert_flow_symbol(
        &mut self,
        module_path: &CanonicalModulePath,
        module: ProjectSymbolModuleView<'_, '_>,
        source_item: ItemId,
        item: &HirItem,
        identity: &HirFlowIdentity,
        executable: bool,
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
        work: &mut u64,
    ) -> bool {
        let Some((public_id, publication, identity_role)) = accepted_flow_identity(identity) else {
            return true;
        };
        let declaration_span = module
            .item_source(
                source_item,
                HirItemSourceRole::Flow(HirFlowSourceRole::Whole),
            )
            .expect("clean final Flow items retain their authored whole source");
        if let Err(error) = Self::charge(work, 1, Some(declaration_span.clone())) {
            diagnostics.push(error);
            return false;
        }
        let identity_span = module
            .item_source(source_item, HirItemSourceRole::Flow(identity_role))
            .expect("accepted Flow identities retain their selected source component");
        let declaration = CallableDeclarationKey::Flow(FlowDeclarationId::new(
            self.world.package().clone(),
            module_path.clone(),
            public_id.clone(),
            publication,
        ));

        if publication == FlowPublicationKind::AuthoredAbsolute
            && let Some(first) = self.callable_symbols().find(|symbol| {
                matches!(
                    symbol.declaration(),
                    CallableDeclarationKey::Flow(existing)
                        if existing.publication() == FlowPublicationKind::AuthoredAbsolute
                            && existing.public_id() == &public_id
                )
            })
        {
            diagnostics.push(ProjectSymbolLinkError::DuplicatePublicId {
                public_id,
                first: first.name_span().clone(),
                duplicate: identity_span,
            });
            return true;
        }

        let Some(path) = flow_symbol_path(&public_id) else {
            return true;
        };
        let lookup_key = path.to_string();
        if let Some(first) = self
            .scopes
            .get(module_path)
            .and_then(|scope| scope.get(&lookup_key))
            .and_then(|bindings| bindings.first())
            .and_then(|binding| binding.sites.first())
            .cloned()
        {
            diagnostics.push(ProjectSymbolLinkError::duplicate_declaration(
                module_path.clone(),
                lookup_key,
                first,
                identity_span,
            ));
            return true;
        }

        let visibility = visibility(item);
        self.insert_scope_binding(
            module_path,
            ScopeBinding::new(
                path,
                ProjectSymbolTargetId::StructuralCallable(declaration.clone()),
                visibility,
                module_path.clone(),
                [identity_span.clone()],
            ),
        );
        self.symbols.insert(
            ProjectDeclarationId::Callable(declaration.clone()),
            ProjectSymbol::Callable(CallableSymbol {
                declaration,
                visibility,
                fx: false,
                source_snapshot: module.snapshot_id(),
                source_item,
                source_owner: HirCallableSourceOwner::Item,
                declaration_span,
                name_span: identity_span,
                executable,
            }),
        );
        true
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "symbol publication binds every final callable identity and source component atomically"
    )]
    pub(super) fn insert_callable_symbol(
        &mut self,
        module_path: &CanonicalModulePath,
        module: ProjectSymbolModuleView<'_, '_>,
        source_item: ItemId,
        source_owner: HirCallableSourceOwner,
        owner: CallableDeclarationOwner,
        owner_path: impl IntoIterator<Item = ModuleSegment>,
        name: &str,
        path: ProjectSymbolPath,
        visibility: Option<Visibility>,
        fx: bool,
        declaration_span: SourceSpan,
        name_span: SourceSpan,
        executable: bool,
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
        work: &mut u64,
    ) -> bool {
        if let Err(error) = Self::charge(work, 1, Some(declaration_span.clone())) {
            diagnostics.push(error);
            return false;
        }
        if is_reserved_type_name(name) {
            diagnostics.push(ProjectSymbolLinkError::ReservedTypeName {
                module: module_path.clone(),
                name: name.to_owned(),
                source: name_span,
            });
            return true;
        }
        let declaration = match CallableDeclarationId::try_new_in_owner_path(
            self.world.package().clone(),
            module_path.clone(),
            owner,
            owner_path,
            name,
        ) {
            Ok(declaration) => declaration,
            Err(reason) => {
                diagnostics.push(ProjectSymbolLinkError::InvalidDeclaration {
                    source: declaration_span,
                    reason,
                });
                return true;
            }
        };
        let lookup_key = path.to_string();
        if let Some(first) = self
            .scopes
            .get(module_path)
            .and_then(|scope| scope.get(&lookup_key))
            .and_then(|bindings| bindings.first())
            .and_then(|binding| binding.sites.first())
            .cloned()
        {
            diagnostics.push(ProjectSymbolLinkError::duplicate_declaration(
                module_path.clone(),
                lookup_key,
                first,
                name_span,
            ));
            return true;
        }
        let declaration = CallableDeclarationKey::Existing(declaration);
        let target = ProjectSymbolTargetId::Callable(declaration.clone());
        self.insert_scope_binding(
            module_path,
            ScopeBinding::new(
                path,
                target,
                visibility,
                module_path.clone(),
                [name_span.clone()],
            ),
        );
        self.symbols.insert(
            ProjectDeclarationId::Callable(declaration.clone()),
            ProjectSymbol::Callable(CallableSymbol {
                declaration,
                visibility,
                fx,
                source_snapshot: module.snapshot_id(),
                source_item,
                source_owner,
                declaration_span,
                name_span,
                executable,
            }),
        );
        true
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "method publication binds structural identity and exact final-HIR source atomically"
    )]
    fn insert_method_callable_symbol(
        &mut self,
        module_path: &CanonicalModulePath,
        module: ProjectSymbolModuleView<'_, '_>,
        source_item: ItemId,
        source_owner: HirCallableSourceOwner,
        declaration: CallableDeclarationKey,
        visibility: Option<Visibility>,
        fx: bool,
        declaration_span: SourceSpan,
        name_span: SourceSpan,
        executable: bool,
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
        work: &mut u64,
    ) -> bool {
        if let Err(error) = Self::charge(work, 1, Some(declaration_span.clone())) {
            diagnostics.push(error);
            return false;
        }
        let declaration_id = ProjectDeclarationId::Callable(declaration.clone());
        if let Some(ProjectSymbol::Callable(first)) = self.symbols.get(&declaration_id) {
            diagnostics.push(ProjectSymbolLinkError::duplicate_declaration(
                module_path.clone(),
                declaration.name().to_owned(),
                first.name_span().clone(),
                name_span,
            ));
            return true;
        }
        self.symbols.insert(
            declaration_id,
            ProjectSymbol::Callable(CallableSymbol {
                declaration,
                visibility,
                fx,
                source_snapshot: module.snapshot_id(),
                source_item,
                source_owner,
                declaration_span,
                name_span,
                executable,
            }),
        );
        true
    }

    pub(super) fn insert_nominals(
        &mut self,
        project: HirProjectView<'_>,
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
        work: &mut u64,
    ) {
        let mut world_count = 0_u64;
        let mut module_count = BTreeMap::<CanonicalModulePath, u64>::new();
        for item_ref in project.items() {
            let hir = match item_ref.item().kind() {
                HirItemKind::Struct(item) => NominalHir::Struct(item),
                HirItemKind::Enum(item) => NominalHir::Enum(item),
                HirItemKind::TypeAlias(item) => NominalHir::TypeAlias(item),
                _ => continue,
            };
            let source = declaration_span(
                ProjectSymbolModuleView::Published(item_ref.module()),
                item_ref.id(),
                HirDeclarationSourceRole::Whole,
            );
            let count = module_count
                .entry(item_ref.module_path().clone())
                .or_default();
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
            if let Err(error) = Self::charge(
                work,
                hir.link_work_units(NominalModuleView::Published(item_ref.module())),
                Some(source.clone()),
            ) {
                diagnostics.push(error);
                return;
            }
            self.insert_nominal_declaration(
                item_ref.module_path(),
                NominalModuleView::Published(item_ref.module()),
                item_ref.id(),
                item_ref.item(),
                hir,
                source,
                diagnostics,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn insert_nominal_declaration(
        &mut self,
        module_path: &CanonicalModulePath,
        module: NominalModuleView<'_, '_>,
        owner: ItemId,
        item: &HirItem,
        hir: NominalHir<'_>,
        source: SourceSpan,
        diagnostics: &mut Vec<ProjectSymbolLinkError>,
    ) {
        let nominal = match nominal::build_nominal_declaration(
            owner,
            hir,
            visibility(item),
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
            diagnostics.push(ProjectSymbolLinkError::duplicate_declaration(
                module_path.clone(),
                name.to_owned(),
                first,
                nominal.source().name().clone(),
            ));
            return;
        }
        let id = nominal.id().clone();
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
                ProjectSymbolTargetId::Nominal(id.clone()),
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
                    diagnostics.push(ProjectSymbolLinkError::duplicate_declaration(
                        binding.module().clone(),
                        binding_name.to_owned(),
                        first,
                        binding.source().clone(),
                    ));
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

fn accepted_flow_identity(
    identity: &HirFlowIdentity,
) -> Option<(PublicId, FlowPublicationKind, HirFlowSourceRole)> {
    match identity {
        HirFlowIdentity::Name { name } => {
            let name = DeclarationName::try_new(name.as_str()).ok()?;
            let public_id = DeclarationIdentityFamily::Flow
                .derive_public_id(&name)
                .ok()?;
            Some((
                public_id,
                FlowPublicationKind::ModuleScoped,
                HirFlowSourceRole::Name,
            ))
        }
        HirFlowIdentity::PublicId { public_id } => {
            let (public_id, publication) = accepted_flow_public_id(public_id)?;
            Some((public_id, publication, HirFlowSourceRole::PublicId))
        }
        HirFlowIdentity::PublicIdAndName { public_id, name } => {
            let (public_id, publication) = accepted_flow_public_id(public_id)?;
            if public_id.as_str().rsplit('.').next() != Some(name.as_str()) {
                return None;
            }
            Some((public_id, publication, HirFlowSourceRole::Name))
        }
        HirFlowIdentity::Missing => None,
    }
}

fn accepted_flow_public_id(reference: &HirIdRef) -> Option<(PublicId, FlowPublicationKind)> {
    let (text, publication) = match reference {
        HirIdRef::Absolute(reference) => (
            reference.as_str().to_owned(),
            FlowPublicationKind::AuthoredAbsolute,
        ),
        HirIdRef::Relative(relative) if relative.parent_depth() == 0 => (
            format!(
                "{}.{}",
                DeclarationIdentityFamily::Flow.prefix(),
                relative.suffix().as_str()
            ),
            FlowPublicationKind::ModuleScoped,
        ),
        HirIdRef::FamilyRelative(relative)
            if relative.relative().parent_depth() == 0
                && relative.family().as_str() == DeclarationIdentityFamily::Flow.prefix() =>
        {
            (
                format!(
                    "{}.{}",
                    DeclarationIdentityFamily::Flow.prefix(),
                    relative.relative().suffix().as_str()
                ),
                FlowPublicationKind::ModuleScoped,
            )
        }
        HirIdRef::Relative(_) | HirIdRef::FamilyRelative(_) => return None,
    };
    let public_id = PublicId::try_new(text).ok()?;
    DeclarationIdentityFamily::Flow
        .validate_public_id(&public_id)
        .ok()?;
    Some((public_id, publication))
}

fn flow_symbol_path(public_id: &PublicId) -> Option<ProjectSymbolPath> {
    let segments = public_id
        .as_str()
        .split('.')
        .map(ProjectSymbolSegment::try_new)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, segments).ok()
}

pub(super) fn visibility(item: &HirItem) -> Option<Visibility> {
    prefix_visibility(item.prefix())
}

pub(super) fn prefix_visibility(prefix: &HirItemPrefix) -> Option<Visibility> {
    prefix.visibility().map(|visibility| match visibility {
        HirVisibility::Public => Visibility::Public,
        HirVisibility::Crate => Visibility::Crate,
        HirVisibility::Super => Visibility::Super,
    })
}

pub(super) fn has_fx_attribute(item: &HirItem) -> bool {
    has_fx_attribute_prefix(item.prefix())
}

pub(super) fn has_fx_attribute_prefix(prefix: &HirItemPrefix) -> bool {
    prefix.attributes().iter().any(|attribute| {
        attribute
            .path()
            .segments()
            .last()
            .is_some_and(|segment| match segment {
                crate::leaf::HirPathSegment::Identifier(name) => name.as_str() == "fx",
                crate::leaf::HirPathSegment::ProjectSymbol(_) => false,
            })
    })
}

fn callable_span(
    module: ProjectSymbolModuleView<'_, '_>,
    owner: ItemId,
    role: HirCallableSourceRole,
) -> SourceSpan {
    module
        .item_source(owner, HirItemSourceRole::Callable(role))
        .expect("project callable identity requires authored source")
}

fn declaration_span(
    module: ProjectSymbolModuleView<'_, '_>,
    owner: ItemId,
    role: HirDeclarationSourceRole,
) -> SourceSpan {
    module
        .item_source(owner, HirItemSourceRole::Declaration(role))
        .expect("project callable/nominal declarations require authored source spans")
}
