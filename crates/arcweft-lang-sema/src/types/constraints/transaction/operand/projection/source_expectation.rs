//! Source containers contribute their checked headers, not their value's Free.

use super::*;

impl<D: ConstraintDomain> ClosedPreparedSourceOperand<D> {
    pub(in crate::types::constraints) fn compose_expected<A: TypeConstraintAccounting>(
        &self,
        value_expected: &Self,
        projection: &CheckedConstraintSourceProjection<PreparedEffects>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ConstraintOperand<D, TypeKind>, TypeConstraintError> {
        if !Arc::ptr_eq(&self.completion, &value_expected.completion) {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        context.enter_node()?;
        let operand =
            projection.compose_operand(&self.operand.view(), &value_expected.operand.view())?;
        Self {
            operand,
            completion: Arc::clone(&self.completion),
        }
        .canonicalize(context)
    }
}

impl CheckedConstraintSourceProjection<PreparedEffects> {
    /// The checked constructor selects the actual's header and the template's
    /// expected value. Retain those two roles explicitly: scalar is the
    /// template value, arrays additionally use the actual length, maps the
    /// actual kind/key. The actual's element/value is never an expected child.
    pub(in crate::types::constraints) fn compose_operand<D: ConstraintDomain>(
        &self,
        actual: &ConstraintOperandView<'_, D>,
        value_expected: &ConstraintOperandView<'_, D>,
    ) -> Result<ConstraintOperand<D>, TypeConstraintError> {
        if actual.lexical_scope() != value_expected.lexical_scope()
            || !self.matches_actual(actual.value())
        {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        Ok(ConstraintOperand {
            value: Arc::new(self.compose_expected(value_expected.value())),
            scope: Arc::clone(value_expected.lexical_scope()),
            origin: Arc::new(ConstraintOperandOrigin::SourceExpectation {
                projection: self.clone(),
                actual: actual.retained(),
                value_expected: value_expected.retained(),
            }),
        })
    }
}
