//! The calculation phase owns both its graph capability and effect authority.

use super::*;
use crate::effect_row::prepared::PreparedEffectGraph;
use crate::types::constraints::{
    normalization::ConstraintProjectedEffectHeader, transaction::ConstraintFunctionHeaderView,
};
use std::sync::Arc;

pub(in crate::types::constraints) enum ConstraintProjectionPhase<'g, D: ConstraintDomain> {
    Admission(PreparedEffectReadView<'g>),
    Preparation(&'g mut PreparedEffectGraph),
    Completion {
        graph: PreparedEffectReadView<'g>,
        owner: Arc<super::super::ConstraintClosedEffects<D>>,
    },
}

impl<D: ConstraintDomain> ConstraintProjectionPhase<'_, D> {
    pub(in crate::types::constraints) fn read(&self) -> PreparedEffectReadView<'_> {
        match self {
            Self::Admission(graph) | Self::Completion { graph, .. } => *graph,
            Self::Preparation(graph) => graph.read(),
        }
    }

    pub(in crate::types::constraints) fn project_header<A: TypeConstraintAccounting>(
        &mut self,
        header: Option<ConstraintFunctionHeaderView<'_, D>>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<
        ConstraintProjectedEffectHeader<
            Option<crate::types::constraints::normalization::ConstraintClosedEffectHeader<D>>,
            PreparedEffects,
        >,
        TypeConstraintError,
    > {
        match self {
            Self::Admission(_) => Err(TypeConstraintInvariant::OriginalProducer.into()),
            Self::Preparation(graph) => {
                if let Some(header) = header {
                    header.admit(graph.read(), context)?;
                }
                Ok(ConstraintProjectedEffectHeader::Original)
            }
            Self::Completion { owner, graph } => match header {
                Some(header) => owner.project_header(&header, *graph, context),
                None => Ok(ConstraintProjectedEffectHeader::Original),
            },
        }
    }

    pub(super) fn inspected_header<A: TypeConstraintAccounting>(
        &mut self,
        shape: TypeConstraintShape<'_, PreparedEffects>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ConstraintProjectedEffectHeader<(), PreparedEffects>, TypeConstraintError> {
        match self {
            Self::Admission(_) => Ok(ConstraintProjectedEffectHeader::Original),
            Self::Preparation(_) => Ok(ConstraintProjectedEffectHeader::Original),
            Self::Completion { owner, .. } => match shape {
                TypeConstraintShape::Function {
                    effects, predicate, ..
                } => {
                    let scope = Arc::new(context.capture_lexical_scope()?);
                    owner.project_inspected_header(&scope, effects, predicate, context)
                }
                _ => Ok(ConstraintProjectedEffectHeader::Original),
            },
        }
    }

    pub(in crate::types::constraints) fn transport<A: TypeConstraintAccounting>(
        &mut self,
        target: &ConstraintLexicalScope,
        source: &ConstraintLexicalScope,
        value: &TypeKind<PreparedEffects>,
        parameter: crate::types::constraints::ConstraintGenericParameterId,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind<PreparedEffects>, TypeConstraintError> {
        match self {
            Self::Admission(_) => Err(TypeConstraintInvariant::OriginalProducer.into()),
            Self::Preparation(graph) => target
                .transport_type(source, value, graph, context)
                .map_err(|error| {
                    TypeConstraintContext::<A, D>::binding_transport_error(error, parameter)
                }),
            Self::Completion { owner, .. } => {
                // Binding values have already traversed every header through
                // this completion. Transport their canonical algebra without
                // minting query terms after the immutable principal was built.
                let canonical = owner.canonicalize(value, source.shape(), context)?;
                let value = target
                    .transport_canonical_type(source, &canonical, context)
                    .map_err(|error| {
                        TypeConstraintContext::<A, D>::binding_transport_error(
                            error,
                            parameter.clone(),
                        )
                    })?;
                value
                    .prepare_effects(target.shape(), context)
                    .map_err(|error| {
                        TypeConstraintContext::<A, D>::binding_transport_error(error, parameter)
                    })
            }
        }
    }
}
