use arcweft_lang_syntax::{
    expr::LifetimeScopeKind,
    types::{TypePath, TypeRef, TypeRefNodePath, TypeRefNodeStep},
};

use crate::{
    effect_row::EffectRow,
    effects::EffectSet,
    types::{EntityKind, TypeKind, TypePoisonId},
};

use super::{
    BuiltinTypeConstructor, NodeValue, NominalResolutionLimitKind, ResolvedTypeNode, Resolver,
    SourceContext, StructuralTypeNodeKind, TypeArgumentExpectation, TypeNameResolution,
    TypePoisonOrigin, TypeResolutionFailure, TypeResolutionInputError,
};

struct SingleArgumentGenericFrame<'a> {
    path: TypeRefNodePath,
    child: TypeRefNodePath,
    base: &'a TypePath,
    depth: u16,
    argument_expectation: Option<TypeArgumentExpectation>,
}

impl Resolver<'_, '_> {
    pub(super) fn resolve_node(
        &mut self,
        context: &SourceContext<'_>,
        value: &TypeRef,
        path: &TypeRefNodePath,
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        if matches!(value, TypeRef::Generic { args, .. } if args.len() == 1) {
            return self.resolve_single_argument_generic_chain(context, value, path, depth);
        }
        if let Some(halted) = self.begin_node(context, path, depth) {
            return Ok(halted);
        }

        match value {
            TypeRef::Never => Ok(self.finish_node(
                context,
                path,
                NodeValue::typed(TypeKind::Never, []),
                TypeNameResolution::Builtin(BuiltinTypeConstructor::Never),
            )),
            TypeRef::ConstInt(value) => Ok(self.finish_node(
                context,
                path,
                NodeValue::constant(*value),
                TypeNameResolution::Structural(StructuralTypeNodeKind::ConstInt),
            )),
            TypeRef::Path(type_path) => {
                let result = self.resolve_name(context, path, type_path, Vec::new(), depth)?;
                Ok(self.finish_node(context, path, result.value, result.outcome))
            }
            TypeRef::Tuple(items) => self.resolve_tuple(context, path, items, depth),
            TypeRef::Function {
                params,
                return_type,
                effects,
            } => self.resolve_function(context, path, params, return_type, effects.as_ref(), depth),
            TypeRef::Choice(alternatives) => {
                self.resolve_choice(context, path, alternatives, depth)
            }
            TypeRef::Generic { base, args } => {
                self.resolve_generic(context, path, base, args, depth)
            }
            TypeRef::TraitBound(bound) => self.resolve_trait_bound(context, path, bound, depth),
            TypeRef::Projection { subject, assoc } => {
                self.resolve_projection(context, path, subject, assoc.as_str(), depth)
            }
            TypeRef::Reference(reference) => {
                self.resolve_reference(context, path, reference, depth)
            }
            TypeRef::Slice(item) => self.resolve_slice(context, path, item, depth),
            TypeRef::Recovery(recovery) => {
                let poison = TypePoisonId::from_index(recovery.index());
                self.record_poison(
                    poison,
                    TypePoisonOrigin::SyntaxTypeDiagnostic,
                    context.evidence(path, true),
                    true,
                );
                Ok(self.finish_node(
                    context,
                    path,
                    NodeValue::error(poison, []),
                    TypeNameResolution::Poisoned(poison),
                ))
            }
        }
    }

    /// Resolves unary constructor chains without consuming one host stack frame
    /// per authored type layer. The ordinary node resolver remains responsible
    /// for the first non-unary child, and completed constructor frames unwind in
    /// the same leaf-to-root order as recursive resolution.
    fn resolve_single_argument_generic_chain(
        &mut self,
        context: &SourceContext<'_>,
        mut value: &TypeRef,
        path: &TypeRefNodePath,
        mut depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let mut path = path.clone();
        let mut frames = Vec::new();

        while let TypeRef::Generic { base, args } = value {
            if args.len() != 1 {
                break;
            }
            if let Some(halted) = self.begin_node(context, &path, depth) {
                return self.finish_single_argument_generic_frames(context, frames, halted);
            }
            if args.len() > usize::from(self.input.limits().generic_arguments_per_application()) {
                let failure = TypeResolutionFailure::Limit {
                    kind: NominalResolutionLimitKind::GenericArgumentsPerApplication,
                    observed: args.len() as u64,
                    maximum: u64::from(self.input.limits().generic_arguments_per_application()),
                };
                let failed = self.failed_node(context, &path, failure, Vec::new());
                return self.finish_single_argument_generic_frames(context, frames, failed);
            }

            let child = context.child_path(&path, TypeRefNodeStep::GenericArgument(0));
            frames.push(SingleArgumentGenericFrame {
                path,
                child: child.clone(),
                base,
                depth,
                argument_expectation: BuiltinTypeConstructor::from_type_path(base)
                    .and_then(|constructor| constructor.argument_expectation(0)),
            });
            path = child;
            value = &args[0];
            depth = depth.saturating_add(1);
        }

        let resolved = if frames.last().is_some_and(|frame| {
            frame.argument_expectation == Some(TypeArgumentExpectation::EntityFamily)
        }) {
            self.resolve_entity_family_node(context, value, &path, depth)?
        } else {
            self.resolve_node(context, value, &path, depth)?
        };
        self.finish_single_argument_generic_frames(context, frames, resolved)
    }

    fn finish_single_argument_generic_frames(
        &mut self,
        context: &SourceContext<'_>,
        frames: Vec<SingleArgumentGenericFrame<'_>>,
        mut value: NodeValue,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        for frame in frames.into_iter().rev() {
            let result = self.resolve_name(
                context,
                &frame.path,
                frame.base,
                vec![(frame.child, value)],
                frame.depth,
            )?;
            value = self.finish_node(context, &frame.path, result.value, result.outcome);
        }
        Ok(value)
    }

    fn resolve_tuple(
        &mut self,
        context: &SourceContext<'_>,
        path: &TypeRefNodePath,
        items: &[TypeRef],
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let mut recovered = Vec::with_capacity(items.len());
        let mut causes = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let child = context.child_path(
                path,
                TypeRefNodeStep::TupleItem(u16::try_from(index).expect("parser cap")),
            );
            let resolved = self.resolve_node(context, item, &child, depth + 1)?;
            causes.extend(&resolved.causes);
            recovered.push(self.require_type(context, &child, resolved));
        }
        Ok(self.finish_node(
            context,
            path,
            NodeValue::typed(TypeKind::Tuple(recovered), causes),
            TypeNameResolution::Structural(StructuralTypeNodeKind::Tuple),
        ))
    }

    fn resolve_function(
        &mut self,
        context: &SourceContext<'_>,
        path: &TypeRefNodePath,
        params: &[TypeRef],
        return_type: &TypeRef,
        effects: Option<&arcweft_lang_syntax::types::TypeEffectRow>,
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let mut recovered = Vec::with_capacity(params.len());
        let mut causes = Vec::new();
        for (index, param) in params.iter().enumerate() {
            let child = context.child_path(
                path,
                TypeRefNodeStep::FunctionParameter(u16::try_from(index).expect("parser cap")),
            );
            let resolved = self.resolve_node(context, param, &child, depth + 1)?;
            causes.extend(&resolved.causes);
            recovered.push(self.require_type(context, &child, resolved));
        }
        let return_path = context.child_path(path, TypeRefNodeStep::FunctionReturn);
        let resolved_return = self.resolve_node(context, return_type, &return_path, depth + 1)?;
        causes.extend(&resolved_return.causes);
        let return_type = self.require_type(context, &return_path, resolved_return);
        let effects = effects.map_or_else(EffectRow::unknown, |effects| {
            EffectSet::from_labels(effects.effects())
                .map_or_else(|_| EffectRow::unknown(), EffectRow::closed)
        });
        Ok(self.finish_node(
            context,
            path,
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
        path: &TypeRefNodePath,
        alternatives: &[TypeRef],
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let mut recovered = Vec::with_capacity(alternatives.len());
        let mut causes = Vec::new();
        for (index, alternative) in alternatives.iter().enumerate() {
            let child = context.child_path(
                path,
                TypeRefNodeStep::ChoiceAlternative(u16::try_from(index).expect("parser cap")),
            );
            let resolved = self.resolve_node(context, alternative, &child, depth + 1)?;
            causes.extend(&resolved.causes);
            recovered.push(self.require_type(context, &child, resolved));
        }
        Ok(self.finish_node(
            context,
            path,
            NodeValue::typed(TypeKind::Choice(recovered), causes),
            TypeNameResolution::Structural(StructuralTypeNodeKind::Choice),
        ))
    }

    fn resolve_generic(
        &mut self,
        context: &SourceContext<'_>,
        path: &TypeRefNodePath,
        base: &arcweft_lang_syntax::types::TypePath,
        args: &[TypeRef],
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        if args.len() > usize::from(self.input.limits().generic_arguments_per_application()) {
            let failure = TypeResolutionFailure::Limit {
                kind: NominalResolutionLimitKind::GenericArgumentsPerApplication,
                observed: args.len() as u64,
                maximum: u64::from(self.input.limits().generic_arguments_per_application()),
            };
            return Ok(self.failed_node(context, path, failure, Vec::new()));
        }
        let mut resolved_args = Vec::with_capacity(args.len());
        let constructor = BuiltinTypeConstructor::from_type_path(base);
        for (index, argument) in args.iter().enumerate() {
            let child = context.child_path(
                path,
                TypeRefNodeStep::GenericArgument(u16::try_from(index).expect("parser cap")),
            );
            let expectation = u16::try_from(index)
                .ok()
                .and_then(|index| constructor.and_then(|value| value.argument_expectation(index)));
            let resolved = if expectation == Some(TypeArgumentExpectation::EntityFamily) {
                self.resolve_entity_family_node(context, argument, &child, depth + 1)?
            } else {
                self.resolve_node(context, argument, &child, depth + 1)?
            };
            resolved_args.push((child, resolved));
        }
        let result = self.resolve_name(context, path, base, resolved_args, depth)?;
        Ok(self.finish_node(context, path, result.value, result.outcome))
    }

    fn resolve_trait_bound(
        &mut self,
        context: &SourceContext<'_>,
        path: &TypeRefNodePath,
        bound: &arcweft_lang_syntax::types::TraitBound,
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let mut causes = Vec::new();
        for (index, argument) in bound.args().iter().enumerate() {
            let child = context.child_path(
                path,
                TypeRefNodeStep::TraitArgument(u16::try_from(index).expect("parser cap")),
            );
            let resolved = self.resolve_node(context, argument, &child, depth + 1)?;
            causes.extend(resolved.causes);
        }
        for (index, binding) in bound.associated().iter().enumerate() {
            let child = context.child_path(
                path,
                TypeRefNodeStep::AssociatedBinding(u16::try_from(index).expect("parser cap")),
            );
            let resolved = self.resolve_node(context, binding.value(), &child, depth + 1)?;
            causes.extend(resolved.causes);
        }
        Ok(self.finish_node(
            context,
            path,
            NodeValue::typed(TypeKind::Unit, causes),
            TypeNameResolution::TraitHead(bound.path().clone()),
        ))
    }

    fn resolve_projection(
        &mut self,
        context: &SourceContext<'_>,
        path: &TypeRefNodePath,
        subject: &TypeRef,
        assoc: &str,
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let child = context.child_path(path, TypeRefNodeStep::ProjectionSubject);
        let resolved = self.resolve_node(context, subject, &child, depth + 1)?;
        let causes = resolved.causes.clone();
        let subject = self.require_type(context, &child, resolved);
        Ok(self.finish_node(
            context,
            path,
            NodeValue::typed(
                TypeKind::Projection {
                    subject: Box::new(subject),
                    trait_name: None,
                    assoc: assoc.to_owned(),
                },
                causes,
            ),
            TypeNameResolution::Projection,
        ))
    }

    fn resolve_reference(
        &mut self,
        context: &SourceContext<'_>,
        path: &TypeRefNodePath,
        reference: &arcweft_lang_syntax::reference::ReferenceType,
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let child = context.child_path(path, TypeRefNodeStep::ReferenceReferent);
        let resolved = self.resolve_node(context, reference.referent(), &child, depth + 1)?;
        let causes = resolved.causes.clone();
        let inner = self.require_type(context, &child, resolved);
        Ok(self.finish_node(
            context,
            path,
            NodeValue::typed(
                TypeKind::BorrowRef {
                    kind: reference.kind(),
                    lifetime: reference
                        .region()
                        .name()
                        .map(|name| LifetimeScopeKind::parse(name.name())),
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
        path: &TypeRefNodePath,
        item: &TypeRef,
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        let child = context.child_path(path, TypeRefNodeStep::SliceItem);
        let resolved = self.resolve_node(context, item, &child, depth + 1)?;
        let causes = resolved.causes.clone();
        let item = self.require_type(context, &child, resolved);
        Ok(self.finish_node(
            context,
            path,
            NodeValue::typed(TypeKind::Slice(Box::new(item)), causes),
            TypeNameResolution::Structural(StructuralTypeNodeKind::Slice),
        ))
    }

    fn resolve_entity_family_node(
        &mut self,
        context: &SourceContext<'_>,
        value: &TypeRef,
        path: &TypeRefNodePath,
        depth: u16,
    ) -> Result<NodeValue, TypeResolutionInputError> {
        if let TypeRef::Path(type_path) = value
            && let Some(family) = super::direct_name(type_path).and_then(EntityKind::from_type_name)
        {
            if let Some(halted) = self.begin_node(context, path, depth) {
                return Ok(halted);
            }
            return Ok(self.finish_node(
                context,
                path,
                NodeValue::entity_family(family.clone()),
                TypeNameResolution::EntityFamily(family),
            ));
        }
        self.resolve_node(context, value, path, depth)
    }

    fn begin_node(
        &mut self,
        context: &SourceContext<'_>,
        path: &TypeRefNodePath,
        depth: u16,
    ) -> Option<NodeValue> {
        if let Some(poison) = self.global_halt {
            self.nodes.push(ResolvedTypeNode::new(
                path.clone(),
                context.evidence(path, false),
                context.terminal_evidence(path),
                context.reference_path(path),
                Some(TypeKind::Error(poison)),
                TypeNameResolution::Poisoned(poison),
            ));
            return Some(NodeValue::error(poison, []));
        }
        if let Err((attempted, maximum)) = self.charge(if context.alias_target { 2 } else { 1 }) {
            return Some(self.work_overflow_node(context, path, attempted, maximum));
        }
        if context.alias_target {
            self.alias_nodes += 1;
            if self.alias_nodes > self.input.limits().alias_expansion_nodes() {
                let failure = TypeResolutionFailure::Limit {
                    kind: NominalResolutionLimitKind::AliasExpansionNodes,
                    observed: self.alias_nodes,
                    maximum: self.input.limits().alias_expansion_nodes(),
                };
                return Some(self.failed_node(context, path, failure, Vec::new()));
            }
        } else {
            self.type_nodes += 1;
            if self.type_nodes > self.input.limits().type_nodes_per_reference() {
                let failure = TypeResolutionFailure::Limit {
                    kind: NominalResolutionLimitKind::TypeNodesPerReference,
                    observed: self.type_nodes,
                    maximum: self.input.limits().type_nodes_per_reference(),
                };
                let failed = self.failed_node(context, path, failure, Vec::new());
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
            return Some(self.failed_node(context, path, failure, Vec::new()));
        }
        None
    }

    fn finish_node(
        &mut self,
        context: &SourceContext<'_>,
        path: &TypeRefNodePath,
        value: NodeValue,
        outcome: TypeNameResolution,
    ) -> NodeValue {
        let recovered = value.ty.clone();
        self.nodes.push(ResolvedTypeNode::new(
            path.clone(),
            context.evidence(path, false),
            context.terminal_evidence(path),
            context.reference_path(path),
            recovered,
            outcome,
        ));
        value
    }

    pub(super) fn require_type(
        &mut self,
        context: &SourceContext<'_>,
        path: &TypeRefNodePath,
        value: NodeValue,
    ) -> TypeKind {
        value.ty.unwrap_or_else(|| {
            let poison = self.allocate_poison();
            self.record_poison(
                poison,
                TypePoisonOrigin::UpstreamTypeDiagnostic,
                context.evidence(path, true),
                false,
            );
            TypeKind::Error(poison)
        })
    }
}
