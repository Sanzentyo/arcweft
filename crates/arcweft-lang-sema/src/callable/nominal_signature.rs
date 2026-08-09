//! Final-HIR nominal resolution for accepted project callable signatures.

use arcweft_lang_hir::{
    identity::TypeId,
    item::{
        HirCapabilityMember, HirGenericParameter, HirImplMember, HirItemKind, HirMethodParameter,
        HirTraitMember, HirWherePredicate,
    },
    module::HirModule,
    project::HirProjectView,
    source_index::{
        HirCallableSourceOwner, HirSourcePresence, HirSourceQuery, HirSourceSite, HirTypeSourceRole,
    },
    symbol::{CallableDeclarationOwner, CallableSymbol, ProjectSymbolTable},
};
use arcweft_source::SourceSpan;

use crate::{
    nominal::{
        CheckedTypeReferenceCache, GenericTypeBinding, GenericTypeScope, NominalResolutionIndex,
        NominalResolutionLimits, ResolvedTypeRefOutcome, SelfTypeScope, TypeResolutionInput,
        TypeSourceEvidence,
    },
    registration::AcceptedNominalWorld,
    types::{GenericTypeOwnerId, GenericTypeParameterId, TypeKind},
};

use super::CallableCatalogBuildError;

pub(super) struct ResolvedProjectSignature {
    pub(super) parameter_types: Vec<Vec<TypeKind>>,
    pub(super) return_type: TypeKind,
}

pub(super) struct ProjectSignatureResolver<'a> {
    project: HirProjectView<'a>,
    symbols: &'a ProjectSymbolTable,
    nominal_world: &'a AcceptedNominalWorld,
    resolutions: &'a mut NominalResolutionIndex,
    cache: &'a mut CheckedTypeReferenceCache,
}

impl<'a> ProjectSignatureResolver<'a> {
    pub(super) fn new(
        project: HirProjectView<'a>,
        symbols: &'a ProjectSymbolTable,
        nominal_world: &'a AcceptedNominalWorld,
        resolutions: &'a mut NominalResolutionIndex,
        cache: &'a mut CheckedTypeReferenceCache,
    ) -> Self {
        Self {
            project,
            symbols,
            nominal_world,
            resolutions,
            cache,
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the exhaustive final-HIR callable-owner matrix keeps each owner/source/item triple adjacent to its typed signature projection"
    )]
    pub(super) fn resolve_project_signature(
        &mut self,
        symbol: &CallableSymbol,
    ) -> Result<ResolvedProjectSignature, CallableCatalogBuildError> {
        let declaration = symbol.declaration();
        let module = self.project.module(declaration.module()).ok_or_else(|| {
            CallableCatalogBuildError::MissingProjectModuleSource {
                module: declaration.module().clone(),
            }
        })?;
        if symbol.source_snapshot() != module.snapshot_id() {
            return Err(CallableCatalogBuildError::ProjectIdentityMismatch {
                declaration: declaration.clone(),
            });
        }
        let item = module.resolve_item(symbol.source_item()).map_err(|_| {
            CallableCatalogBuildError::ProjectIdentityMismatch {
                declaration: declaration.clone(),
            }
        })?;

        let owner = GenericTypeOwnerId::Callable(declaration.clone());
        match (declaration.owner(), symbol.source_owner(), item.kind()) {
            (
                CallableDeclarationOwner::Flow,
                HirCallableSourceOwner::Item,
                HirItemKind::Flow(flow),
            ) => self.resolve_signature_types(
                module,
                symbol.declaration_span(),
                flow.generic_parameters(),
                flow.where_predicates(),
                vec![
                    flow.parameters()
                        .iter()
                        .map(arcweft_lang_hir::item::HirParameter::ty)
                        .collect(),
                ],
                flow.result().authored_type(),
                &owner,
            ),
            (
                CallableDeclarationOwner::Function,
                HirCallableSourceOwner::Item,
                HirItemKind::Function(function),
            ) => {
                let parameters = function
                    .parameter_groups()
                    .iter()
                    .map(|group| {
                        group
                            .parameters()
                            .iter()
                            .map(arcweft_lang_hir::item::HirParameter::ty)
                            .collect()
                    })
                    .collect();
                self.resolve_signature_types(
                    module,
                    symbol.declaration_span(),
                    function.generic_parameters(),
                    function.where_predicates(),
                    parameters,
                    function.return_type(),
                    &owner,
                )
            }
            (
                CallableDeclarationOwner::Predicate,
                HirCallableSourceOwner::Item,
                HirItemKind::Predicate(predicate),
            ) => self.resolve_signature_types(
                module,
                symbol.declaration_span(),
                predicate.generic_parameters(),
                predicate.where_predicates(),
                vec![
                    predicate
                        .parameters()
                        .iter()
                        .map(arcweft_lang_hir::item::HirParameter::ty)
                        .collect(),
                ],
                Some(predicate.return_type()),
                &owner,
            ),
            (
                CallableDeclarationOwner::Proof,
                HirCallableSourceOwner::Item,
                HirItemKind::Proof(proof),
            ) => self.resolve_signature_types(
                module,
                symbol.declaration_span(),
                proof.generic_parameters(),
                proof.where_predicates(),
                vec![
                    proof
                        .parameters()
                        .iter()
                        .map(arcweft_lang_hir::item::HirParameter::ty)
                        .collect(),
                ],
                Some(proof.return_type()),
                &owner,
            ),
            (
                CallableDeclarationOwner::ExternCapability,
                HirCallableSourceOwner::ExternCapabilityFunction { member },
                HirItemKind::ExternCapability(capability),
            ) => {
                let Some(HirCapabilityMember::Function(function)) =
                    capability.members().get(usize::from(member))
                else {
                    return Err(CallableCatalogBuildError::ProjectIdentityMismatch {
                        declaration: declaration.clone(),
                    });
                };
                self.resolve_signature_types(
                    module,
                    symbol.declaration_span(),
                    function.generic_parameters(),
                    &[],
                    function
                        .parameter_groups()
                        .iter()
                        .map(|group| {
                            group
                                .parameters()
                                .iter()
                                .map(arcweft_lang_hir::item::HirParameter::ty)
                                .collect()
                        })
                        .collect(),
                    function.return_type(),
                    &owner,
                )
            }
            (
                CallableDeclarationOwner::TraitRequirement,
                HirCallableSourceOwner::TraitFunction { member },
                HirItemKind::Trait(trait_item),
            ) => {
                let Some(HirTraitMember::Function(function)) =
                    trait_item.members().get(usize::from(member))
                else {
                    return Err(CallableCatalogBuildError::ProjectIdentityMismatch {
                        declaration: declaration.clone(),
                    });
                };
                self.resolve_method_signature(
                    module,
                    symbol,
                    trait_item.generic_parameters(),
                    trait_item.where_predicates(),
                    None,
                    function.generic_parameters(),
                    function.where_predicates(),
                    function
                        .parameter_groups()
                        .iter()
                        .map(arcweft_lang_hir::item::HirMethodParameterGroup::parameters),
                    function.return_type(),
                    &owner,
                )
            }
            (
                CallableDeclarationOwner::TraitImplementation
                | CallableDeclarationOwner::InherentMethod,
                HirCallableSourceOwner::ImplFunction { member },
                HirItemKind::Impl(impl_item),
            ) => {
                let Some(HirImplMember::Function(function)) =
                    impl_item.members().get(usize::from(member))
                else {
                    return Err(CallableCatalogBuildError::ProjectIdentityMismatch {
                        declaration: declaration.clone(),
                    });
                };
                self.resolve_method_signature(
                    module,
                    symbol,
                    impl_item.generic_parameters(),
                    impl_item.where_predicates(),
                    Some(impl_item.target()),
                    function.generic_parameters(),
                    function.where_predicates(),
                    function
                        .parameter_groups()
                        .iter()
                        .map(arcweft_lang_hir::item::HirMethodParameterGroup::parameters),
                    function.return_type(),
                    &owner,
                )
            }
            _ => Err(CallableCatalogBuildError::ProjectIdentityMismatch {
                declaration: declaration.clone(),
            }),
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "method signature resolution joins its inline Trait/Impl owner and member exactly once"
    )]
    fn resolve_method_signature<'method>(
        &mut self,
        module: &HirModule,
        symbol: &CallableSymbol,
        owner_generics: &[HirGenericParameter],
        owner_where: &[HirWherePredicate],
        impl_target: Option<TypeId>,
        method_generics: &[HirGenericParameter],
        method_where: &[HirWherePredicate],
        parameter_groups: impl IntoIterator<Item = &'method [HirMethodParameter]>,
        return_type: Option<TypeId>,
        owner: &GenericTypeOwnerId,
    ) -> Result<ResolvedProjectSignature, CallableCatalogBuildError> {
        let generics = owner_generics
            .iter()
            .chain(method_generics)
            .cloned()
            .collect::<Vec<_>>();
        let where_predicates = owner_where
            .iter()
            .chain(method_where)
            .cloned()
            .collect::<Vec<_>>();
        let generic_scope = Self::generic_scope(&generics, owner, symbol.declaration_span())?;
        for parameter in &generics {
            for bound in parameter.bounds() {
                self.resolve_type(
                    module,
                    *bound,
                    &generic_scope,
                    SelfTypeScope::Absent,
                    symbol.declaration_span(),
                )?;
            }
        }
        for predicate in &where_predicates {
            self.resolve_type(
                module,
                predicate.subject(),
                &generic_scope,
                SelfTypeScope::Absent,
                symbol.declaration_span(),
            )?;
            for bound in predicate.bounds() {
                self.resolve_type(
                    module,
                    *bound,
                    &generic_scope,
                    SelfTypeScope::Absent,
                    symbol.declaration_span(),
                )?;
            }
        }
        let self_type = if let Some(target) = impl_target {
            self.resolve_type(
                module,
                target,
                &generic_scope,
                SelfTypeScope::Absent,
                symbol.declaration_span(),
            )?
        } else {
            let ordinal = u16::try_from(generics.len())
                .map_err(|_| CallableCatalogBuildError::WorkOverflow)?;
            TypeKind::GenericParam(GenericTypeParameterId::new(owner.clone(), ordinal))
        };
        let self_scope = SelfTypeScope::Known(self_type.clone());
        let parameter_types = parameter_groups
            .into_iter()
            .map(|group| {
                group
                    .iter()
                    .map(|parameter| match parameter {
                        HirMethodParameter::Receiver(_) => Ok(self_type.clone()),
                        HirMethodParameter::Typed(parameter) => self.resolve_type(
                            module,
                            parameter.ty(),
                            &generic_scope,
                            self_scope.clone(),
                            symbol.declaration_span(),
                        ),
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let return_type = return_type.map_or(Ok(TypeKind::Unit), |ty| {
            self.resolve_type(
                module,
                ty,
                &generic_scope,
                self_scope,
                symbol.declaration_span(),
            )
        })?;
        Ok(ResolvedProjectSignature {
            parameter_types,
            return_type,
        })
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the final callable signature boundary passes every HIR-owned component explicitly"
    )]
    fn resolve_signature_types(
        &mut self,
        module: &HirModule,
        declaration_span: &SourceSpan,
        generic_parameters: &[HirGenericParameter],
        where_predicates: &[HirWherePredicate],
        parameter_groups: Vec<Vec<TypeId>>,
        return_type: Option<TypeId>,
        owner: &GenericTypeOwnerId,
    ) -> Result<ResolvedProjectSignature, CallableCatalogBuildError> {
        let generics = Self::generic_scope(generic_parameters, owner, declaration_span)?;
        for parameter in generic_parameters {
            for bound in parameter.bounds() {
                self.resolve_type(
                    module,
                    *bound,
                    &generics,
                    SelfTypeScope::Absent,
                    declaration_span,
                )?;
            }
        }
        for predicate in where_predicates {
            self.resolve_type(
                module,
                predicate.subject(),
                &generics,
                SelfTypeScope::Absent,
                declaration_span,
            )?;
            for bound in predicate.bounds() {
                self.resolve_type(
                    module,
                    *bound,
                    &generics,
                    SelfTypeScope::Absent,
                    declaration_span,
                )?;
            }
        }
        let parameter_types = parameter_groups
            .into_iter()
            .map(|group| {
                group
                    .into_iter()
                    .map(|ty| {
                        self.resolve_type(
                            module,
                            ty,
                            &generics,
                            SelfTypeScope::Absent,
                            declaration_span,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let return_type = return_type.map_or(Ok(TypeKind::Unit), |ty| {
            self.resolve_type(
                module,
                ty,
                &generics,
                SelfTypeScope::Absent,
                declaration_span,
            )
        })?;
        Ok(ResolvedProjectSignature {
            parameter_types,
            return_type,
        })
    }

    fn generic_scope(
        parameters: &[HirGenericParameter],
        owner: &GenericTypeOwnerId,
        declaration_span: &SourceSpan,
    ) -> Result<GenericTypeScope, CallableCatalogBuildError> {
        let bindings = parameters
            .iter()
            .filter_map(|parameter| match parameter {
                HirGenericParameter::Type { name, .. } => name.resolved(),
                HirGenericParameter::Lifetime { .. } => None,
            })
            .enumerate()
            .map(|(ordinal, name)| {
                let ordinal =
                    u16::try_from(ordinal).map_err(|_| CallableCatalogBuildError::WorkOverflow)?;
                Ok(GenericTypeBinding::new(
                    GenericTypeParameterId::new(owner.clone(), ordinal),
                    arcweft_lang_syntax::ast::module_path::ModuleSegment::new(name.as_str())
                        .expect("resolved HIR names are valid module segments"),
                    TypeSourceEvidence::accepted(
                        declaration_span.range(),
                        declaration_span.clone(),
                    ),
                ))
            })
            .collect::<Result<Vec<_>, CallableCatalogBuildError>>()?;
        GenericTypeScope::try_new(bindings).map_err(|error| {
            CallableCatalogBuildError::InvalidProjectSignatureSource {
                span: error
                    .duplicate()
                    .project()
                    .expect("accepted callable generic bindings retain project source")
                    .clone(),
            }
        })
    }

    fn resolve_type(
        &mut self,
        module: &HirModule,
        root: TypeId,
        generics: &GenericTypeScope,
        self_scope: SelfTypeScope,
        declaration_span: &SourceSpan,
    ) -> Result<TypeKind, CallableCatalogBuildError> {
        let source = type_source(module, root).unwrap_or_else(|| declaration_span.clone());
        let input = TypeResolutionInput::accepted(
            root,
            module,
            self.project,
            self.symbols,
            self.nominal_world,
            generics,
            self_scope,
            NominalResolutionLimits::PRODUCTION,
        )
        .map_err(
            |reason| CallableCatalogBuildError::ProjectSignatureResolutionInput {
                span: source.clone(),
                reason: Box::new(reason),
            },
        )?;
        let report = self.cache.resolve(&input).map_err(|reason| {
            CallableCatalogBuildError::ProjectSignatureResolutionInput {
                span: source.clone(),
                reason: Box::new(reason),
            }
        })?;
        let resolved = match report.outcome() {
            ResolvedTypeRefOutcome::Complete(product) => product.recovered().clone(),
            ResolvedTypeRefOutcome::Poisoned(poisoned) => poisoned.product().recovered().clone(),
            ResolvedTypeRefOutcome::Detached(_) => {
                return Err(
                    CallableCatalogBuildError::InvalidProjectSignatureTypeOutcome { owner: root },
                );
            }
        };
        self.resolutions
            .record(report.as_ref().clone())
            .map_err(
                |reason| CallableCatalogBuildError::ProjectSignatureResolutionIndex {
                    span: source,
                    reason: Box::new(reason),
                },
            )?;
        Ok(resolved)
    }
}

fn type_source(module: &HirModule, owner: TypeId) -> Option<SourceSpan> {
    let lookup = module
        .source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Type {
                owner,
                role: HirTypeSourceRole::Whole,
            },
        )
        .ok()?;
    match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => Some(span.clone()),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => None,
    }
}
