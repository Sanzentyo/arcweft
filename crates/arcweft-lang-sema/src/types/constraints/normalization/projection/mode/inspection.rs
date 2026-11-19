//! Compare an original operand under a completed relation.

use super::{
    AdmittedConstraintFunctionHeader, ArrayLength, CanonicalEffects, ConstraintConstOperandView,
    ConstraintDomain, ConstraintLexicalScope, ConstraintOperand, ConstraintOperandChildren,
    ConstraintOperandView, ConstraintProjectionPhase, GenericConstReference, GenericTypeReference,
    PreparedEffects, ProjectionChildren, ProjectionMode, TypeConstraintAccounting,
    TypeConstraintContext, TypeConstraintError, TypeConstraintInvariant, TypeConstraintShape,
    TypeKind, rebuild_closed,
};
use crate::effect_row::EffectPredicate;
use crate::types::effect_domain::{RebindableEffects, RetainedEffects};
use std::marker::PhantomData;

pub(in crate::types::constraints::normalization::projection) struct InspectOperand<
    E = CanonicalEffects,
>(PhantomData<E>);

trait InspectHeader: RebindableEffects {
    fn header<A: TypeConstraintAccounting, D: ConstraintDomain>(
        header: &AdmittedConstraintFunctionHeader<'_, D>,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(Self::Row, EffectPredicate), TypeConstraintError>;
}

impl InspectHeader for CanonicalEffects {
    fn header<A: TypeConstraintAccounting, D: ConstraintDomain>(
        header: &AdmittedConstraintFunctionHeader<'_, D>,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(Self::Row, EffectPredicate), TypeConstraintError> {
        let ConstraintProjectionPhase::Completion { projection, .. } = phase else {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        };
        projection.project_header_for_comparison(header, context)
    }
}

impl InspectHeader for RetainedEffects {
    fn header<A: TypeConstraintAccounting, D: ConstraintDomain>(
        header: &AdmittedConstraintFunctionHeader<'_, D>,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(Self::Row, EffectPredicate), TypeConstraintError> {
        let ConstraintProjectionPhase::Completion {
            projection,
            named: Some(named),
            ..
        } = phase
        else {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        };
        projection.project_retained_header(header, named, context)
    }
}

impl<'ty, D: ConstraintDomain + 'ty, E: InspectHeader> ProjectionMode<'ty, D>
    for InspectOperand<E>
{
    type TypeInput = ConstraintOperandView<'ty, D>;
    type ConstInput = ConstraintConstOperandView<'ty, D>;
    type Children = ConstraintOperandChildren<'ty, D>;
    type ShapeSeed = TypeConstraintShape<'ty, PreparedEffects>;
    type TypeOutput = TypeKind<E>;
    type ConstOutput = ArrayLength;

    fn ty(input: &Self::TypeInput) -> &'ty TypeKind<PreparedEffects> {
        input.value()
    }

    fn length(input: &Self::ConstInput) -> &'ty ArrayLength {
        input.value()
    }

    fn admit_type_reference<A: TypeConstraintAccounting>(
        input: &Self::TypeInput,
        reference: &GenericTypeReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        input.admit_type_reference(reference, context)
    }

    fn admit_const_reference<A: TypeConstraintAccounting>(
        input: &Self::ConstInput,
        reference: &GenericConstReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        input.admit_const_reference(reference, context)
    }

    fn type_binding(binding: &'ty ConstraintOperand<D>) -> Self::TypeInput {
        binding.view()
    }

    fn const_binding(binding: &'ty ConstraintOperand<D, ArrayLength>) -> Self::ConstInput {
        binding.view()
    }

    fn array_length(
        input: &Self::TypeInput,
    ) -> Result<Option<Self::ConstInput>, TypeConstraintError> {
        input.array_length()
    }

    fn next_child(
        children: &mut Self::Children,
    ) -> Result<Option<Self::TypeInput>, TypeConstraintError> {
        children.next().transpose()
    }

    fn children<A: TypeConstraintAccounting>(
        input: &Self::TypeInput,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ProjectionChildren<'ty, D, Self::Children, Self::ShapeSeed>, TypeConstraintError>
    {
        let children = input.children(context)?;
        let header = children.function_header()?;
        Ok(ProjectionChildren {
            children,
            seed: input.value().constraint_shape(),
            header,
        })
    }

    fn type_leaf(input: &Self::TypeInput) -> TypeKind<E> {
        let TypeKind::GenericParam(reference) = input.value() else {
            unreachable!("only generic references use the leaf substitution")
        };
        TypeKind::GenericParam(reference.clone())
    }

    fn const_leaf(input: &Self::ConstInput) -> ArrayLength {
        input.value().clone()
    }

    fn rebuild<A: TypeConstraintAccounting>(
        shape: Self::ShapeSeed,
        children: Vec<TypeKind<E>>,
        length: Option<ArrayLength>,
        header: Option<AdmittedConstraintFunctionHeader<'ty, D>>,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind<E>, TypeConstraintError> {
        let header = header
            .as_ref()
            .map(|header| E::header(header, phase, context))
            .transpose()?;
        rebuild_closed(shape, children, length, header)
    }

    fn substitute_type<A: TypeConstraintAccounting>(
        input: &Self::TypeInput,
        binding: &'ty ConstraintOperand<D>,
        value: TypeKind<E>,
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        parameter: &GenericTypeReference,
        _: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind<E>, TypeConstraintError> {
        if input.lexical_scope().as_ref() != enclosing || binding.scope() != owner {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        if owner == enclosing {
            return Ok(value);
        }
        enclosing
            .transport_canonical_type(owner, &value, context)
            .map_err(|error| {
                TypeConstraintContext::<A, D>::binding_transport_error(
                    error,
                    parameter.clone().into(),
                )
            })
    }

    fn substitute_const<A: TypeConstraintAccounting>(
        input: &Self::ConstInput,
        binding: &'ty ConstraintOperand<D, ArrayLength>,
        value: ArrayLength,
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        parameter: &GenericConstReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ArrayLength, TypeConstraintError> {
        if input.lexical_scope().as_ref() != enclosing || binding.scope() != owner {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        context.transport_binding_length(enclosing, owner, &value, parameter.clone().into())
    }
}
