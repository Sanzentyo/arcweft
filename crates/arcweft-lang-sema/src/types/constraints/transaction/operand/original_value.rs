//! Original values are admitted by their source, never a candidate inventory.

use std::sync::Arc;

use super::{ConstraintOperand, ConstraintOperandOrigin};
use crate::types::constraints::{
    ConstraintDomain, OriginalProducerContext, TypeConstraintError, TypeConstraintRejection,
    context::{TypeConstraintAccounting, TypeConstraintContext},
    lexical::ConstraintLexicalScope,
    references,
    solution::{map_effects_with_control, map_term_with_control},
};
use crate::types::{
    ArrayLength, GenericConstReference, GenericTypeReference, TypeKind, TypeProjectionError,
};

#[cfg(test)]
mod tests;

pub(in crate::types::constraints) trait OriginalOperandValue:
    Sized
{
    fn admit<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        target: &ConstraintLexicalScope,
        original: &OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError>;
}

impl<D: ConstraintDomain, T> ConstraintOperand<D, T> {
    /// Admit in the owning source Γ and transport into the requested actual
    /// frames in one controlled fold. The target never reinterprets the input.
    pub(in crate::types::constraints) fn from_original_in<A: TypeConstraintAccounting>(
        value: &T,
        scope: Arc<ConstraintLexicalScope>,
        original: OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError>
    where
        T: OriginalOperandValue,
    {
        let value = value.admit(&scope, &original, context)?;
        Ok(Self {
            value: Arc::new(value),
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Original(original)),
        })
    }
}

#[cfg(test)]
impl<A: TypeConstraintAccounting, D: ConstraintDomain> TypeConstraintContext<'_, A, D> {
    pub(in crate::types::constraints) fn fixture_operand_in<T: OriginalOperandValue>(
        &mut self,
        original: &OriginalProducerContext,
        value: &T,
    ) -> Result<ConstraintOperand<D, T>, TypeConstraintError> {
        self.check_cancelled()?;
        for _ in original.scope().binders() {
            self.enter_node()?;
        }
        ConstraintOperand::from_original_in(
            value,
            Arc::new(original.gamma().clone()),
            original.clone(),
            self,
        )
    }
}

#[cfg(test)]
impl<D: ConstraintDomain, T> ConstraintOperand<D, T> {
    /// Deliberately raw terms for lower solver fixtures. Source admission tests
    /// use the production constructor with an explicit source authority.
    pub(in crate::types::constraints) fn fixture_original(
        value: T,
        scope: Arc<ConstraintLexicalScope>,
    ) -> Self {
        let original = OriginalProducerContext::ground_fixture(scope.as_ref().clone());
        Self {
            value: Arc::new(value),
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Original(original)),
        }
    }
}

impl OriginalOperandValue for TypeKind {
    fn admit<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        target_scope: &ConstraintLexicalScope,
        original: &OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        map_term_with_control(
            self,
            original.scope(),
            target_scope.shape(),
            1,
            context,
            &|reference, source, target, _, context| {
                if let GenericTypeReference::Bound(bound) = reference {
                    source.bound_type(bound.depth(), bound.slot())?;
                }
                if reference.free_parameter().is_some()
                    && !original.admits_parameter(&reference.clone().into())
                {
                    return Err(TypeProjectionError::Control(references::type_out_of_scope(
                        reference,
                    )));
                }
                let mapped = target_scope
                    .type_reference(original.gamma(), source, target, reference, context)
                    .map_err(|error| {
                        TypeProjectionError::Control(
                            TypeConstraintContext::<A, D>::binding_transport_error(
                                error,
                                reference.clone().into(),
                            ),
                        )
                    })?;
                Ok(TypeKind::GenericParam(mapped))
            },
            &|length, source, target, _, context| {
                length.admit_in_scopes(source, target, target_scope, original, context)
            },
            &|row, source, target, depth, context| {
                map_effects_with_control(row, depth, context, &|reference, context| {
                    if let crate::types::GenericEffectReference::Bound(bound) = reference {
                        source.bound_effect(bound.depth(), bound.slot())?;
                    }
                    if reference.free_parameter().is_some()
                        && !original.admits_parameter(&reference.clone().into())
                    {
                        return Err(TypeProjectionError::Control(
                            references::effect_out_of_scope(reference),
                        ));
                    }
                    let mapped = target_scope
                        .effect_reference(original.gamma(), source, target, reference, context)
                        .map_err(|error| {
                            TypeProjectionError::Control(
                                TypeConstraintContext::<A, D>::binding_transport_error(
                                    error,
                                    reference.clone().into(),
                                ),
                            )
                        })?;
                    Ok(crate::effect_row::EffectRow::open(
                        crate::effects::EffectSet::new(),
                        mapped,
                    ))
                })
            },
        )
        .map_err(|error| match error {
            TypeProjectionError::Control(error) => error,
            TypeProjectionError::Instantiation(crate::types::TypeInstantiationError::Scope(
                error,
            )) => error.into(),
            TypeProjectionError::Instantiation(_) => TypeConstraintRejection::UnresolvedType.into(),
        })
    }
}

impl ArrayLength {
    fn admit_in_scopes<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        source: &crate::types::GenericScope,
        target: &crate::types::GenericScope,
        target_scope: &ConstraintLexicalScope,
        original: &OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeProjectionError<TypeConstraintError>> {
        context
            .check_cancelled()
            .map_err(TypeProjectionError::Control)?;
        if let Self::Generic(reference) = self {
            if let GenericConstReference::Bound(bound) = reference {
                source.bound_const(bound.depth(), bound.slot())?;
            }
            if reference.free_parameter().is_some()
                && !original.admits_parameter(&reference.clone().into())
            {
                return Err(TypeProjectionError::Control(
                    references::const_out_of_scope(reference),
                ));
            }
            let mapped = target_scope
                .const_reference(original.gamma(), source, target, reference, context)
                .map_err(|error| {
                    TypeProjectionError::Control(
                        TypeConstraintContext::<A, D>::binding_transport_error(
                            error,
                            reference.clone().into(),
                        ),
                    )
                })?;
            return Ok(Self::Generic(mapped));
        }
        Ok(self.clone())
    }
}

impl OriginalOperandValue for ArrayLength {
    fn admit<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        target: &ConstraintLexicalScope,
        original: &OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        context.enter_node()?;
        self.admit_in_scopes(original.scope(), target.shape(), target, original, context)
            .map_err(|error| match error {
                TypeProjectionError::Control(error) => error,
                TypeProjectionError::Instantiation(
                    crate::types::TypeInstantiationError::Scope(error),
                ) => error.into(),
                TypeProjectionError::Instantiation(_) => {
                    TypeConstraintRejection::UnresolvedType.into()
                }
            })
    }
}
