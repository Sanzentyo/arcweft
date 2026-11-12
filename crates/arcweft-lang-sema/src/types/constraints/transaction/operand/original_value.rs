//! Original values are admitted by their source, never a candidate inventory.

use std::sync::Arc;

use super::{ConstraintOperand, ConstraintOperandOrigin};
use crate::types::constraints::{
    ConstraintDomain, OriginalProducerContext, TypeConstraintError, TypeConstraintRejection,
    context::{TypeConstraintAccounting, TypeConstraintContext},
    lexical::ConstraintLexicalScope,
    references,
    solution::map_term_with_control,
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
        scope: &ConstraintLexicalScope,
        original: &OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError>;
}

impl<D: ConstraintDomain, T> ConstraintOperand<D, T> {
    pub(in crate::types::constraints) fn original<A: TypeConstraintAccounting>(
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
        scope: &ConstraintLexicalScope,
        original: &OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        map_term_with_control(
            self,
            scope.shape(),
            scope.shape(),
            1,
            context,
            &|reference, scope, _, _, _| {
                if let GenericTypeReference::Bound(bound) = reference {
                    scope.bound_type(bound.depth(), bound.slot())?;
                }
                if reference.free_parameter().is_some()
                    && !original.admits_free(&reference.clone().into())
                {
                    return Err(TypeProjectionError::Control(references::type_out_of_scope(
                        reference,
                    )));
                }
                Ok(TypeKind::GenericParam(reference.clone()))
            },
            &|length, scope, _, _, context| {
                length
                    .admit_in_scope(scope, original, context)
                    .map_err(TypeProjectionError::Control)
            },
            &|row, scope, _, depth, context| {
                crate::types::projection_control::visit_effect_row(context, row, depth)?;
                for reference in row.variables().into_iter().flatten() {
                    if let crate::types::GenericEffectReference::Bound(bound) = reference {
                        scope.bound_effect(bound.depth(), bound.slot())?;
                    }
                    if reference.free_parameter().is_some()
                        && !original.admits_free(&reference.clone().into())
                    {
                        return Err(TypeProjectionError::Control(
                            references::effect_out_of_scope(reference),
                        ));
                    }
                }
                Ok(row.clone())
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
    fn admit_in_scope<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        scope: &crate::types::GenericScope,
        original: &OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        context.check_cancelled()?;
        if let Self::Generic(reference) = self {
            if let GenericConstReference::Bound(bound) = reference {
                scope.bound_const(bound.depth(), bound.slot())?;
            }
            if reference.free_parameter().is_some()
                && !original.admits_free(&reference.clone().into())
            {
                return Err(references::const_out_of_scope(reference));
            }
        }
        Ok(self.clone())
    }
}

impl OriginalOperandValue for ArrayLength {
    fn admit<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        scope: &ConstraintLexicalScope,
        original: &OriginalProducerContext,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        context.enter_node()?;
        self.admit_in_scope(scope.shape(), original, context)
    }
}
