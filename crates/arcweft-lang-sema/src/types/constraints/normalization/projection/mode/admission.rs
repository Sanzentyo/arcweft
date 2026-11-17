//! Permission-only use of the common type fold; it emits no semantic value.

use super::*;
use crate::types::constraints::transaction::ConstraintFunctionHeaderView;

pub(in crate::types::constraints::normalization::projection) struct AdmitOrigins;

impl<'ty, D: ConstraintDomain + 'ty> ProjectionMode<'ty, D> for AdmitOrigins {
    type TypeInput = ConstraintOperandView<'ty, D>;
    type ConstInput = ConstraintConstOperandView<'ty, D>;
    type Children = ConstraintOperandChildren<'ty, D>;
    type ShapeSeed = Option<ConstraintFunctionHeaderView<'ty, D>>;
    type TypeOutput = ();
    type ConstOutput = ();

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
    ) -> Result<(Self::Children, Self::ShapeSeed), TypeConstraintError> {
        let children = input.children(context)?;
        let header = children.function_header()?;
        Ok((children, header))
    }
    fn type_leaf(_: &Self::TypeInput) {}
    fn const_leaf(_: &Self::ConstInput) {}
    fn rebuild<A: TypeConstraintAccounting>(
        header: Self::ShapeSeed,
        _: Vec<()>,
        _: Option<()>,
        phase: &mut ConstraintProjectionPhase<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        if let Some(header) = header {
            header.admit(phase.read(), context)?;
        }
        Ok(())
    }
    fn substitute_type<A: TypeConstraintAccounting>(
        _: &Self::TypeInput,
        _: &'ty ConstraintOperand<D>,
        (): (),
        _: &ConstraintLexicalScope,
        _: &ConstraintLexicalScope,
        _: &GenericTypeReference,
        _: &mut ConstraintProjectionPhase<'_, D>,
        _: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        // This mode is issued only with Validation, which does not follow
        // bindings. A different policy is an invalid internal phase pairing.
        Err(TypeConstraintInvariant::OriginalProducer.into())
    }
    fn substitute_const<A: TypeConstraintAccounting>(
        _: &Self::ConstInput,
        _: &'ty ConstraintOperand<D, ArrayLength>,
        (): (),
        _: &ConstraintLexicalScope,
        _: &ConstraintLexicalScope,
        _: &GenericConstReference,
        _: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        Err(TypeConstraintInvariant::OriginalProducer.into())
    }
}
