//! Source containers contribute their checked headers, not their value's Free.

use super::*;

impl CheckedConstraintSourceProjection {
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
