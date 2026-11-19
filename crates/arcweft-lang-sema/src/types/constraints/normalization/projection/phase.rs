//! The calculation phase owns both its graph capability and effect authority.

use super::*;
use crate::effect_row::prepared::PreparedEffectGraph;
use crate::types::constraints::{
    normalization::ConstraintProjectedEffectHeader, transaction::AdmittedConstraintFunctionHeader,
};

pub(in crate::types::constraints) enum ConstraintProjectionPhase<'g, D: ConstraintDomain> {
    Admission(PreparedEffectReadView<'g>),
    Preparation(&'g mut PreparedEffectGraph),
    Completion {
        graph: PreparedEffectReadView<'g>,
        projection: &'g mut ConstraintEffectProjection<D>,
        named: Option<&'g crate::types::effect_domain::PreparedNamedEffectUse<'g, 'g>>,
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
        header: Option<AdmittedConstraintFunctionHeader<'_, D>>,
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
            Self::Preparation(_) => Ok(ConstraintProjectedEffectHeader::Original),
            Self::Completion { projection, .. } => match header {
                Some(header) => projection.project_header(&header, context),
                None => Ok(ConstraintProjectedEffectHeader::Original),
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
            Self::Completion { projection, .. } => {
                // Binding values have already traversed every header through
                // this completion. Transport their canonical algebra without
                // minting query terms after the immutable principal was built.
                let canonical = projection
                    .owner()
                    .canonicalize(value, source.shape(), context)?;
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
