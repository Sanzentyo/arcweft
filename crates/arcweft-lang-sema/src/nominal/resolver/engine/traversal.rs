use arcweft_lang_hir::{
    expr::HirBorrowKind,
    identity::TypeId,
    leaf::HirTypeRegion,
    type_ref::{HirFunctionType, HirGenericType, HirReferenceType, HirTraitBoundType, HirTypeKind},
};
use arcweft_lang_syntax::reference::BorrowKind;

use crate::{
    effect_row::EffectRow,
    effects::EffectSet,
    types::{EntityKind, LifetimeScopeKind, TypeKind},
};

use super::{
    BuiltinTypeConstructor, NodeValue, NominalResolutionLimitKind, ResolvedTypeNode, Resolver,
    SourceContext, StructuralTypeNodeKind, TypeArgumentExpectation, TypeNameResolution,
    TypePoisonOrigin, TypeResolutionFailure, TypeResolutionInputError,
};

impl Resolver<'_, '_> {
    pub(super) fn resolve_node(
        &mut self,
        context: &SourceContext<'_>,
        owner: TypeId,
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        if let Some(halted) = self.begin_node(context, owner, depth) {
            return Ok(halted);
        }
        let ty = context
            .module
            .resolve_type(owner)
            .expect("validated final-HIR type identity remains live");
        let kind = ty.kind().clone();
        let self_poison = ty.is_poisoned().then(|| {
            let poison = self.allocate_poison();
            self.record_poison(
                poison,
                TypePoisonOrigin::SyntaxTypeDiagnostic,
                context.evidence(owner, true),
                true,
            );
            poison
        });

        let mut value = match kind {
            HirTypeKind::Never => self.finish_node(
                context,
                owner,
                NodeValue::typed(TypeKind::Never, []),
                TypeNameResolution::Builtin(BuiltinTypeConstructor::Never),
            ),
            HirTypeKind::ConstInt(value) => self.finish_node(
                context,
                owner,
                NodeValue::constant(value),
                TypeNameResolution::Structural(StructuralTypeNodeKind::ConstInt),
            ),
            HirTypeKind::Path(path) => {
                let result = self.resolve_name(context, owner, &path, Vec::new(), depth)?;
                self.finish_node(context, owner, result.value, result.outcome)
            }
            HirTypeKind::Tuple(items) => self.resolve_tuple(context, owner, &items, depth)?,
            HirTypeKind::Function(function) => {
                self.resolve_function(context, owner, &function, depth)?
            }
            HirTypeKind::Choice(alternatives) => {
                self.resolve_choice(context, owner, &alternatives, depth)?
            }
            HirTypeKind::Generic(generic) => {
                self.resolve_generic(context, owner, &generic, depth)?
            }
            HirTypeKind::TraitBound(bound) => {
                self.resolve_trait_bound(context, owner, &bound, depth)?
            }
            HirTypeKind::Projection(projection) => self.resolve_projection(
                context,
                owner,
                projection.subject(),
                projection.associated().as_str(),
                depth,
            )?,
            HirTypeKind::Reference(reference) => {
                self.resolve_reference(context, owner, &reference, depth)?
            }
            HirTypeKind::Slice(item) => self.resolve_slice(context, owner, item, depth)?,
            HirTypeKind::Recovery(_) => {
                let poison = self_poison.unwrap_or_else(|| {
                    let poison = self.allocate_poison();
                    self.record_poison(
                        poison,
                        TypePoisonOrigin::SyntaxTypeDiagnostic,
                        context.evidence(owner, true),
                        true,
                    );
                    poison
                });
                return Ok(self.finish_node(
                    context,
                    owner,
                    NodeValue::error(poison, []),
                    TypeNameResolution::Poisoned(poison),
                ));
            }
        };
        if let Some(poison) = self_poison {
            value.causes = super::canonical_poisons(value.causes.into_iter().chain([poison]));
        }
        Ok(value)
    }

    fn resolve_tuple(
        &mut self,
        context: &SourceContext<'_>,
        owner: TypeId,
        items: &[TypeId],
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let mut recovered = Vec::with_capacity(items.len());
        let mut causes = Vec::new();
        for child in items.iter().copied() {
            let resolved = self.resolve_node(context, child, depth.saturating_add(1))?;
            causes.extend(&resolved.causes);
            recovered.push(self.require_type(context, child, resolved));
        }
        let semantic_type = if recovered.is_empty() {
            TypeKind::Unit
        } else {
            TypeKind::Tuple(recovered)
        };
        Ok(self.finish_node(
            context,
            owner,
            NodeValue::typed(semantic_type, causes),
            TypeNameResolution::Structural(StructuralTypeNodeKind::Tuple),
        ))
    }

    fn resolve_function(
        &mut self,
        context: &SourceContext<'_>,
        owner: TypeId,
        function: &HirFunctionType,
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let mut recovered = Vec::with_capacity(function.parameters().len());
        let mut causes = Vec::new();
        for child in function.parameters().iter().copied() {
            let resolved = self.resolve_node(context, child, depth.saturating_add(1))?;
            causes.extend(&resolved.causes);
            recovered.push(self.require_type(context, child, resolved));
        }
        let return_owner = function.return_type();
        let resolved_return = self.resolve_node(context, return_owner, depth.saturating_add(1))?;
        causes.extend(&resolved_return.causes);
        let return_type = self.require_type(context, return_owner, resolved_return);
        let effects = function
            .effects()
            .map_or_else(EffectRow::unknown, |effects| {
                EffectSet::from_labels(
                    effects
                        .effects()
                        .iter()
                        .map(arcweft_lang_hir::type_ref::HirEffectName::as_str),
                )
                .map_or_else(|_| EffectRow::unknown(), EffectRow::closed)
            });
        Ok(self.finish_node(
            context,
            owner,
            NodeValue::typed(
                TypeKind::Function {
                    params: recovered,
                    return_type: Box::new(return_type),
                    effects,
                },
                causes,
            ),
            TypeNameResolution::Structural(StructuralTypeNodeKind::Function),
        ))
    }

    fn resolve_choice(
        &mut self,
        context: &SourceContext<'_>,
        owner: TypeId,
        alternatives: &[TypeId],
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let mut recovered = Vec::with_capacity(alternatives.len());
        let mut causes = Vec::new();
        for child in alternatives.iter().copied() {
            let resolved = self.resolve_node(context, child, depth.saturating_add(1))?;
            causes.extend(&resolved.causes);
            recovered.push(self.require_type(context, child, resolved));
        }
        Ok(self.finish_node(
            context,
            owner,
            NodeValue::typed(TypeKind::Choice(recovered), causes),
            TypeNameResolution::Structural(StructuralTypeNodeKind::Choice),
        ))
    }

    fn resolve_generic(
        &mut self,
        context: &SourceContext<'_>,
        owner: TypeId,
        generic: &HirGenericType,
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        if generic.arguments().len()
            > usize::from(self.input.limits().generic_arguments_per_application())
        {
            let failure = TypeResolutionFailure::Limit {
                kind: NominalResolutionLimitKind::GenericArgumentsPerApplication,
                observed: generic.arguments().len() as u64,
                maximum: u64::from(self.input.limits().generic_arguments_per_application()),
            };
            return Ok(self.failed_node(context, owner, failure, Vec::new()));
        }
        let constructor = BuiltinTypeConstructor::from_hir_path(generic.base());
        let mut resolved_args = Vec::with_capacity(generic.arguments().len());
        for (index, child) in generic.arguments().iter().copied().enumerate() {
            let expectation = u16::try_from(index)
                .ok()
                .and_then(|index| constructor.and_then(|value| value.argument_expectation(index)));
            let resolved = if expectation == Some(TypeArgumentExpectation::EntityFamily) {
                self.resolve_entity_family_node(context, child, depth.saturating_add(1))?
            } else {
                self.resolve_node(context, child, depth.saturating_add(1))?
            };
            resolved_args.push((child, resolved));
        }
        let result = self.resolve_name(context, owner, generic.base(), resolved_args, depth)?;
        Ok(self.finish_node(context, owner, result.value, result.outcome))
    }

    fn resolve_trait_bound(
        &mut self,
        context: &SourceContext<'_>,
        owner: TypeId,
        bound: &HirTraitBoundType,
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let mut causes = Vec::new();
        for child in bound.arguments().iter().copied().chain(
            bound
                .associated()
                .iter()
                .map(arcweft_lang_hir::type_ref::HirAssociatedTypeBinding::value),
        ) {
            let resolved = self.resolve_node(context, child, depth.saturating_add(1))?;
            causes.extend(resolved.causes);
        }
        Ok(self.finish_node(
            context,
            owner,
            NodeValue::typed(TypeKind::Unit, causes),
            TypeNameResolution::TraitHead(bound.base().clone()),
        ))
    }

    fn resolve_projection(
        &mut self,
        context: &SourceContext<'_>,
        owner: TypeId,
        subject: TypeId,
        associated: &str,
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let resolved = self.resolve_node(context, subject, depth.saturating_add(1))?;
        let causes = resolved.causes.clone();
        let subject = self.require_type(context, subject, resolved);
        Ok(self.finish_node(
            context,
            owner,
            NodeValue::typed(
                TypeKind::Projection {
                    subject: Box::new(subject),
                    trait_name: None,
                    assoc: associated.to_owned(),
                },
                causes,
            ),
            TypeNameResolution::Projection,
        ))
    }

    fn resolve_reference(
        &mut self,
        context: &SourceContext<'_>,
        owner: TypeId,
        reference: &HirReferenceType,
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let child = reference.referent();
        let resolved = self.resolve_node(context, child, depth.saturating_add(1))?;
        let causes = resolved.causes.clone();
        let inner = self.require_type(context, child, resolved);
        let kind = match reference.kind() {
            HirBorrowKind::Shared => BorrowKind::Shared,
            HirBorrowKind::Mutable => BorrowKind::Mutable,
        };
        let lifetime = match reference.region() {
            Some(HirTypeRegion::Named(region)) => {
                Some(LifetimeScopeKind::parse(region.name().as_str()))
            }
            Some(HirTypeRegion::Elided(_)) | None => None,
        };
        Ok(self.finish_node(
            context,
            owner,
            NodeValue::typed(
                TypeKind::BorrowRef {
                    kind,
                    lifetime,
                    inner: Box::new(inner),
                },
                causes,
            ),
            TypeNameResolution::Structural(StructuralTypeNodeKind::Reference),
        ))
    }

    fn resolve_slice(
        &mut self,
        context: &SourceContext<'_>,
        owner: TypeId,
        item: TypeId,
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let resolved = self.resolve_node(context, item, depth.saturating_add(1))?;
        let causes = resolved.causes.clone();
        let item = self.require_type(context, item, resolved);
        Ok(self.finish_node(
            context,
            owner,
            NodeValue::typed(TypeKind::Slice(Box::new(item)), causes),
            TypeNameResolution::Structural(StructuralTypeNodeKind::Slice),
        ))
    }

    fn resolve_entity_family_node(
        &mut self,
        context: &SourceContext<'_>,
        owner: TypeId,
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let ty = context
            .module
            .resolve_type(owner)
            .expect("validated final-HIR type identity remains live");
        if let HirTypeKind::Path(path) = ty.kind()
            && let Some(family) = super::direct_name(path).and_then(EntityKind::from_type_name)
        {
            if let Some(halted) = self.begin_node(context, owner, depth) {
                return Ok(halted);
            }
            return Ok(self.finish_node(
                context,
                owner,
                NodeValue::entity_family(family.clone()),
                TypeNameResolution::EntityFamily(family),
            ));
        }
        self.resolve_node(context, owner, depth)
    }

    fn begin_node(
        &mut self,
        context: &SourceContext<'_>,
        owner: TypeId,
        depth: u16,
    ) -> Option<NodeValue> {
        if let Some(poison) = self.global_halt {
            self.nodes.push(ResolvedTypeNode::new(
                owner,
                context.alias_target,
                context.evidence(owner, false),
                context.terminal_evidence(owner),
                context.reference_path(owner),
                Some(TypeKind::Error(poison)),
                TypeNameResolution::Poisoned(poison),
            ));
            return Some(NodeValue::error(poison, []));
        }
        if let Err((attempted, maximum)) = self.charge(if context.alias_target { 2 } else { 1 }) {
            return Some(self.work_overflow_node(context, owner, attempted, maximum));
        }
        if context.alias_target {
            self.alias_nodes = self.alias_nodes.saturating_add(1);
            if self.alias_nodes > self.input.limits().alias_expansion_nodes() {
                let failure = TypeResolutionFailure::Limit {
                    kind: NominalResolutionLimitKind::AliasExpansionNodes,
                    observed: self.alias_nodes,
                    maximum: self.input.limits().alias_expansion_nodes(),
                };
                return Some(self.failed_node(context, owner, failure, Vec::new()));
            }
        } else {
            self.type_nodes = self.type_nodes.saturating_add(1);
            if self.type_nodes > self.input.limits().type_nodes_per_reference() {
                let failure = TypeResolutionFailure::Limit {
                    kind: NominalResolutionLimitKind::TypeNodesPerReference,
                    observed: self.type_nodes,
                    maximum: self.input.limits().type_nodes_per_reference(),
                };
                let failed = self.failed_node(context, owner, failure, Vec::new());
                self.global_halt = failed.causes.first().copied();
                return Some(failed);
            }
        }
        if depth > self.input.limits().recursive_type_depth() {
            let failure = TypeResolutionFailure::Limit {
                kind: NominalResolutionLimitKind::RecursiveTypeDepth,
                observed: u64::from(depth),
                maximum: u64::from(self.input.limits().recursive_type_depth()),
            };
            return Some(self.failed_node(context, owner, failure, Vec::new()));
        }
        None
    }

    fn finish_node(
        &mut self,
        context: &SourceContext<'_>,
        owner: TypeId,
        value: NodeValue,
        outcome: TypeNameResolution,
    ) -> NodeValue {
        let recovered = value.ty.clone();
        self.nodes.push(ResolvedTypeNode::new(
            owner,
            context.alias_target,
            context.evidence(owner, false),
            context.terminal_evidence(owner),
            context.reference_path(owner),
            recovered,
            outcome,
        ));
        value
    }

    pub(super) fn require_type(
        &mut self,
        context: &SourceContext<'_>,
        owner: TypeId,
        value: NodeValue,
    ) -> TypeKind {
        value.ty.unwrap_or_else(|| {
            let poison = self.allocate_poison();
            self.record_poison(
                poison,
                TypePoisonOrigin::UpstreamTypeDiagnostic,
                context.evidence(owner, true),
                false,
            );
            TypeKind::Error(poison)
        })
    }
}
