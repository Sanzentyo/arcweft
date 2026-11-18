//! Outputs of the same admission and projection fold.

use crate::types::{CanonicalEffects, PreparedEffects, TypeEffectDomain};
use std::marker::PhantomData;

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
    ) -> Result<ProjectionChildren<'ty, D, Self::Children, Self::ShapeSeed>, TypeConstraintError>;
    fn type_leaf(input: &Self::TypeInput) -> Self::TypeOutput;
    fn const_leaf(input: &Self::ConstInput) -> Self::ConstOutput;
    fn rebuild<A: TypeConstraintAccounting>(
        seed: Self::ShapeSeed,
        children: Vec<Self::TypeOutput>,
        length: Option<Self::ConstOutput>,
        header: Option<AdmittedConstraintFunctionHeader<'ty, D>>,
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

pub(super) struct ProjectionChildren<'ty, D: ConstraintDomain, C, S> {
    pub(super) children: C,
    pub(super) seed: S,
    pub(super) header: Option<ConstraintFunctionHeaderView<'ty, D>>,
}

/// Read-only inspection cannot be persisted as a provenance-bearing operand.
pub(super) struct InspectValues<E = PreparedEffects>(PhantomData<E>);

trait InspectionEffects: TypeEffectDomain {
    fn rebuild<A: TypeConstraintAccounting, D: ConstraintDomain>(
        shape: TypeConstraintShape<'_, PreparedEffects>,
        children: Vec<TypeKind<Self>>,
        length: Option<ArrayLength>,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind<Self>, TypeConstraintError>;

    fn transport<A: TypeConstraintAccounting, D: ConstraintDomain>(
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        value: &TypeKind<Self>,
        parameter: &GenericTypeReference,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind<Self>, TypeConstraintError>;
}

impl<'ty, D: ConstraintDomain, E: InspectionEffects> ProjectionMode<'ty, D> for InspectValues<E> {
    type TypeInput = &'ty TypeKind<PreparedEffects>;
    type ConstInput = &'ty ArrayLength;
    type Children = TypeConstraintChildren<'ty, PreparedEffects>;
    type ShapeSeed = TypeConstraintShape<'ty, PreparedEffects>;
    type TypeOutput = TypeKind<E>;
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
    ) -> Result<ProjectionChildren<'ty, D, Self::Children, Self::ShapeSeed>, TypeConstraintError>
    {
        let shape = input.constraint_shape();
        Ok(ProjectionChildren {
            children: shape.children(),
            seed: shape,
            header: None,
        })
    }
    fn type_leaf(input: &Self::TypeInput) -> Self::TypeOutput {
        let TypeKind::GenericParam(reference) = input else {
            unreachable!("only generic references use the leaf substitution")
        };
        TypeKind::GenericParam(reference.clone())
    }
    fn const_leaf(input: &Self::ConstInput) -> ArrayLength {
        (*input).clone()
    }
    fn rebuild<A: TypeConstraintAccounting>(
        shape: Self::ShapeSeed,
        children: Vec<Self::TypeOutput>,
        length: Option<ArrayLength>,
        _: Option<AdmittedConstraintFunctionHeader<'ty, D>>,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self::TypeOutput, TypeConstraintError> {
        E::rebuild(shape, children, length, phase, context)
    }
    fn substitute_type<A: TypeConstraintAccounting>(
        _: &Self::TypeInput,
        _: &'ty ConstraintOperand<D>,
        value: Self::TypeOutput,
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        parameter: &GenericTypeReference,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self::TypeOutput, TypeConstraintError> {
        if owner == enclosing {
            Ok(value)
        } else {
            E::transport(enclosing, owner, &value, parameter, phase, context)
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

impl InspectionEffects for PreparedEffects {
    fn rebuild<A: TypeConstraintAccounting, D: ConstraintDomain>(
        shape: TypeConstraintShape<'_, PreparedEffects>,
        children: Vec<TypeKind<Self>>,
        length: Option<ArrayLength>,
        _: &mut ConstraintProjectionPhase<'_, D>,
        _: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind<Self>, TypeConstraintError> {
        shape.rebuild_with(
            children,
            &mut (),
            |(), _| Ok(length.expect("array length was projected before its children")),
            |(), row| Ok(row.clone()),
            |(), predicate| Ok(predicate.clone()),
        )
    }

    fn transport<A: TypeConstraintAccounting, D: ConstraintDomain>(
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        value: &TypeKind<Self>,
        parameter: &GenericTypeReference,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind<Self>, TypeConstraintError> {
        phase.transport(enclosing, owner, value, parameter.clone().into(), context)
    }
}

impl InspectionEffects for CanonicalEffects {
    fn rebuild<A: TypeConstraintAccounting, D: ConstraintDomain>(
        shape: TypeConstraintShape<'_, PreparedEffects>,
        children: Vec<TypeKind>,
        length: Option<ArrayLength>,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeConstraintError> {
        let ConstraintProjectionPhase::Completion { owner, .. } = phase else {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        };
        let mut header = match shape {
            TypeConstraintShape::Function {
                effects, predicate, ..
            } => {
                let scope = context.capture_lexical_scope()?;
                let (row, predicate) =
                    owner.project_inspected_header(&scope, effects, predicate, context)?;
                (Some(row), Some(predicate))
            }
            _ => (None, None),
        };
        shape.rebuild_with(
            children,
            &mut header,
            |_, _| Ok(length.expect("array length was projected before its children")),
            |(row, _), _| Ok(row.take().expect("the function row was projected")),
            |(_, predicate), _| {
                Ok(predicate
                    .take()
                    .expect("the function predicate was projected"))
            },
        )
    }

    fn transport<A: TypeConstraintAccounting, D: ConstraintDomain>(
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        value: &TypeKind,
        parameter: &GenericTypeReference,
        _: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<TypeKind, TypeConstraintError> {
        enclosing
            .transport_canonical_type(owner, value, context)
            .map_err(|error| {
                TypeConstraintContext::<A, D>::binding_transport_error(
                    error,
                    parameter.clone().into(),
                )
            })
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
    ) -> Result<ProjectionChildren<'ty, D, Self::Children, Self::ShapeSeed>, TypeConstraintError>
    {
        let children = input.children(context)?;
        let header = children.function_header()?;
        let seed = children.rebuild_seed();
        Ok(ProjectionChildren {
            children,
            seed,
            header,
        })
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
        header: Option<AdmittedConstraintFunctionHeader<'ty, D>>,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self::TypeOutput, TypeConstraintError> {
        seed.rebuild(children, length, header, phase, context)
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
