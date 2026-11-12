//! Two statically distinct outputs of the same admission and projection fold.

use super::*;
use crate::types::constraints::transaction::{
    ConstraintConstOperandView, ConstraintOperand, ConstraintOperandChildren,
    ConstraintOperandEffects, ConstraintOperandRebuild, ConstraintOperandView,
    ConstraintProjectedOperand,
};

pub(super) trait ProjectionMode<'ty, D: ConstraintDomain> {
    type TypeInput: Clone;
    type ConstInput: Clone;
    type Children: Iterator<Item = Self::TypeInput>;
    type ShapeSeed;
    type TypeOutput;
    type ConstOutput;
    type Effects;

    fn ty(input: &Self::TypeInput) -> &'ty TypeKind;
    fn length(input: &Self::ConstInput) -> &'ty ArrayLength;
    fn type_binding(binding: &'ty ConstraintOperand<D>) -> Self::TypeInput;
    fn const_binding(binding: &'ty ConstraintOperand<D, ArrayLength>) -> Self::ConstInput;
    fn array_length(input: &Self::TypeInput) -> Option<Self::ConstInput>;
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
        effects: &Self::Effects,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self::TypeOutput, TypeConstraintError>;
    fn substitute_type<A: TypeConstraintAccounting>(
        input: &Self::TypeInput,
        binding: &'ty ConstraintOperand<D>,
        value: Self::TypeOutput,
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        parameter: &GenericTypeReference,
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
    type TypeInput = &'ty TypeKind;
    type ConstInput = &'ty ArrayLength;
    type Children = TypeConstraintChildren<'ty>;
    type ShapeSeed = TypeConstraintShape<'ty>;
    type TypeOutput = TypeKind;
    type ConstOutput = ArrayLength;
    type Effects = ConstraintOperandEffects<D>;

    fn ty(input: &Self::TypeInput) -> &'ty TypeKind {
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
    fn array_length(input: &Self::TypeInput) -> Option<Self::ConstInput> {
        match input.constraint_shape() {
            TypeConstraintShape::Array { len, .. } => Some(len),
            _ => None,
        }
    }
    fn children<A: TypeConstraintAccounting>(
        input: &Self::TypeInput,
        _: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(Self::Children, Self::ShapeSeed), TypeConstraintError> {
        let shape = input.constraint_shape();
        Ok((shape.children(), shape))
    }
    fn type_leaf(input: &Self::TypeInput) -> TypeKind {
        (*input).clone()
    }
    fn const_leaf(input: &Self::ConstInput) -> ArrayLength {
        (*input).clone()
    }
    fn rebuild<A: TypeConstraintAccounting>(
        shape: Self::ShapeSeed,
        children: Vec<TypeKind>,
        length: Option<ArrayLength>,
        effects: &Self::Effects,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeConstraintError> {
        let mut header = match (effects, shape) {
            (
                ConstraintOperandEffects::Closed(owner),
                TypeConstraintShape::Function {
                    effects, predicate, ..
                },
            ) => {
                let scope = std::sync::Arc::new(context.capture_lexical_scope()?);
                owner.project_header(&scope, effects, predicate, context)?.0
            }
            _ => super::super::ConstraintProjectedEffectHeader::Original,
        };
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
        value: TypeKind,
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        parameter: &GenericTypeReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeConstraintError> {
        if owner == enclosing {
            Ok(value)
        } else {
            context.transport_binding_type(enclosing, owner, &value, parameter.clone().into())
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
    type TypeOutput = ConstraintProjectedOperand<D, TypeKind>;
    type ConstOutput = ConstraintProjectedOperand<D, ArrayLength>;
    type Effects = ConstraintOperandEffects<D>;

    fn ty(input: &Self::TypeInput) -> &'ty TypeKind {
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
    fn array_length(input: &Self::TypeInput) -> Option<Self::ConstInput> {
        input.array_length()
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
        effects: &Self::Effects,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self::TypeOutput, TypeConstraintError> {
        seed.rebuild(children, length, effects, context)
    }
    fn substitute_type<A: TypeConstraintAccounting>(
        input: &Self::TypeInput,
        binding: &'ty ConstraintOperand<D>,
        value: Self::TypeOutput,
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        parameter: &GenericTypeReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self::TypeOutput, TypeConstraintError> {
        value.substitute(input, binding, enclosing, owner, parameter, context)
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
