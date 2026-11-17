//! Two statically distinct outputs of the same admission and projection fold.

use crate::types::PreparedEffects;

mod admission;
pub(super) use admission::AdmitOrigins;

use super::*;
use crate::types::constraints::transaction::{
    ConstraintConstOperandView, ConstraintOperand, ConstraintOperandChildren,
    ConstraintOperandRebuild, ConstraintOperandView, ConstraintProjectedOperand,
};

pub(super) trait ProjectionMode<'ty, D: ConstraintDomain> {
    type TypeInput: Clone;
    type ConstInput: Clone;
    type Children;
    type ShapeSeed;
    type TypeOutput;
    type ConstOutput;

    fn ty(input: &Self::TypeInput) -> &'ty TypeKind<PreparedEffects>;
    fn length(input: &Self::ConstInput) -> &'ty ArrayLength;
    fn admit_type_reference<A: TypeConstraintAccounting>(
        input: &Self::TypeInput,
        reference: &GenericTypeReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError>;
    fn admit_const_reference<A: TypeConstraintAccounting>(
        input: &Self::ConstInput,
        reference: &GenericConstReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError>;
    fn type_binding(binding: &'ty ConstraintOperand<D>) -> Self::TypeInput;
    fn const_binding(binding: &'ty ConstraintOperand<D, ArrayLength>) -> Self::ConstInput;
    fn array_length(
        input: &Self::TypeInput,
    ) -> Result<Option<Self::ConstInput>, TypeConstraintError>;
    fn next_child(
        children: &mut Self::Children,
    ) -> Result<Option<Self::TypeInput>, TypeConstraintError>;
    fn children<A: TypeConstraintAccounting>(
        input: &Self::TypeInput,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(Self::Children, Self::ShapeSeed), TypeConstraintError>;
    fn type_leaf(input: &Self::TypeInput) -> Self::TypeOutput;
    fn const_leaf(input: &Self::ConstInput) -> Self::ConstOutput;
    fn rebuild<A: TypeConstraintAccounting>(
        seed: Self::ShapeSeed,
        children: Vec<Self::TypeOutput>,
        length: Option<Self::ConstOutput>,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self::TypeOutput, TypeConstraintError>;
    fn substitute_type<A: TypeConstraintAccounting>(
        input: &Self::TypeInput,
        binding: &'ty ConstraintOperand<D>,
        value: Self::TypeOutput,
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        parameter: &GenericTypeReference,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self::TypeOutput, TypeConstraintError>;
    fn substitute_const<A: TypeConstraintAccounting>(
        input: &Self::ConstInput,
        binding: &'ty ConstraintOperand<D, ArrayLength>,
        value: Self::ConstOutput,
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        parameter: &GenericConstReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self::ConstOutput, TypeConstraintError>;
}

/// Read-only inspection cannot be persisted as a provenance-bearing operand.
pub(super) struct InspectValues;

impl<'ty, D: ConstraintDomain> ProjectionMode<'ty, D> for InspectValues {
    type TypeInput = &'ty TypeKind<PreparedEffects>;
    type ConstInput = &'ty ArrayLength;
    type Children = TypeConstraintChildren<'ty, PreparedEffects>;
    type ShapeSeed = TypeConstraintShape<'ty, PreparedEffects>;
    type TypeOutput = TypeKind<PreparedEffects>;
    type ConstOutput = ArrayLength;

    fn admit_type_reference<A: TypeConstraintAccounting>(
        _: &Self::TypeInput,
        _: &GenericTypeReference,
        _: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        Ok(())
    }
    fn admit_const_reference<A: TypeConstraintAccounting>(
        _: &Self::ConstInput,
        _: &GenericConstReference,
        _: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        Ok(())
    }

    fn ty(input: &Self::TypeInput) -> &'ty TypeKind<PreparedEffects> {
        input
    }
    fn length(input: &Self::ConstInput) -> &'ty ArrayLength {
        input
    }
    fn type_binding(binding: &'ty ConstraintOperand<D>) -> Self::TypeInput {
        binding.value()
    }
    fn const_binding(binding: &'ty ConstraintOperand<D, ArrayLength>) -> Self::ConstInput {
        binding.value()
    }
    fn array_length(
        input: &Self::TypeInput,
    ) -> Result<Option<Self::ConstInput>, TypeConstraintError> {
        Ok(match input.constraint_shape() {
            TypeConstraintShape::Array { len, .. } => Some(len),
            _ => None,
        })
    }
    fn next_child(
        children: &mut Self::Children,
    ) -> Result<Option<Self::TypeInput>, TypeConstraintError> {
        Ok(children.next())
    }
    fn children<A: TypeConstraintAccounting>(
        input: &Self::TypeInput,
        _: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(Self::Children, Self::ShapeSeed), TypeConstraintError> {
        let shape = input.constraint_shape();
        Ok((shape.children(), shape))
    }
    fn type_leaf(input: &Self::TypeInput) -> TypeKind<PreparedEffects> {
        (*input).clone()
    }
    fn const_leaf(input: &Self::ConstInput) -> ArrayLength {
        (*input).clone()
    }
    fn rebuild<A: TypeConstraintAccounting>(
        shape: Self::ShapeSeed,
        children: Vec<TypeKind<PreparedEffects>>,
        length: Option<ArrayLength>,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind<PreparedEffects>, TypeConstraintError> {
        let mut header = phase.inspected_header(shape, context)?;
        shape.rebuild_with(
            children,
            &mut header,
            |_, _| Ok(length.expect("array length was projected before its children")),
            |header, row| Ok(header.take_row(row)),
            |header, predicate| Ok(header.take_predicate(predicate)),
        )
    }
    fn substitute_type<A: TypeConstraintAccounting>(
        _: &Self::TypeInput,
        _: &'ty ConstraintOperand<D>,
        value: TypeKind<PreparedEffects>,
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        parameter: &GenericTypeReference,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind<PreparedEffects>, TypeConstraintError> {
        if owner == enclosing {
            Ok(value)
        } else {
            phase.transport(enclosing, owner, &value, parameter.clone().into(), context)
        }
    }
    fn substitute_const<A: TypeConstraintAccounting>(
        _: &Self::ConstInput,
        _: &'ty ConstraintOperand<D, ArrayLength>,
        value: ArrayLength,
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        parameter: &GenericConstReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ArrayLength, TypeConstraintError> {
        context.transport_binding_length(enclosing, owner, &value, parameter.clone().into())
    }
}

pub(super) struct RetainOrigins;

impl<'ty, D: ConstraintDomain + 'ty> ProjectionMode<'ty, D> for RetainOrigins {
    type TypeInput = ConstraintOperandView<'ty, D>;
    type ConstInput = ConstraintConstOperandView<'ty, D>;
    type Children = ConstraintOperandChildren<'ty, D>;
    type ShapeSeed = ConstraintOperandRebuild<'ty, D>;
    type TypeOutput = ConstraintProjectedOperand<D, TypeKind<PreparedEffects>>;
    type ConstOutput = ConstraintProjectedOperand<D, ArrayLength>;

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

    fn ty(input: &Self::TypeInput) -> &'ty TypeKind<PreparedEffects> {
        input.value()
    }
    fn length(input: &Self::ConstInput) -> &'ty ArrayLength {
        input.value()
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
    ) -> Result<(Self::Children, Self::ShapeSeed), TypeConstraintError> {
        let children = input.children(context)?;
        let seed = children.rebuild_seed();
        Ok((children, seed))
    }
    fn type_leaf(input: &Self::TypeInput) -> Self::TypeOutput {
        input.projected_leaf()
    }
    fn const_leaf(input: &Self::ConstInput) -> Self::ConstOutput {
        input.projected_leaf()
    }
    fn rebuild<A: TypeConstraintAccounting>(
        seed: Self::ShapeSeed,
        children: Vec<Self::TypeOutput>,
        length: Option<Self::ConstOutput>,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self::TypeOutput, TypeConstraintError> {
        seed.rebuild(children, length, phase, context)
    }
    fn substitute_type<A: TypeConstraintAccounting>(
        input: &Self::TypeInput,
        binding: &'ty ConstraintOperand<D>,
        value: Self::TypeOutput,
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        parameter: &GenericTypeReference,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self::TypeOutput, TypeConstraintError> {
        value.substitute(input, binding, enclosing, owner, parameter, phase, context)
    }
    fn substitute_const<A: TypeConstraintAccounting>(
        input: &Self::ConstInput,
        binding: &'ty ConstraintOperand<D, ArrayLength>,
        value: Self::ConstOutput,
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        parameter: &GenericConstReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self::ConstOutput, TypeConstraintError> {
        value.substitute(input, binding, enclosing, owner, parameter, context)
    }
}
