use super::{
    Engine, RuntimeCallBackend, RuntimeEvalError, RuntimeExpr, RuntimeValue,
    runtime_sequence_values, runtime_value_label,
};
use crate::value::RuntimeIterator;

impl Engine {
    pub(super) fn evaluate_filter_expr(
        &mut self,
        source: &RuntimeExpr,
        param: &str,
        body: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let iterator = match RuntimeIterator::from_value(
            self.evaluate_expr_with_backend(source, pure_backend)?,
        ) {
            Ok(iterator) => iterator,
            Err(value) => {
                return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                    &value,
                )));
            }
        };
        let mut filtered = Vec::new();
        for item in iterator.collect::<Vec<_>>() {
            let keep = self.with_temp_binding_ref(param, &item, |this| {
                this.evaluate_bool_with_backend(body, pure_backend)
            })?;
            if keep {
                filtered.push(item);
            }
        }
        Ok(runtime_sequence_values(filtered))
    }
}
