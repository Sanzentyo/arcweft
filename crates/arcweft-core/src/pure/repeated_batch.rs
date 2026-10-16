//! A repeated physical row retains the original function and borrows its inputs.

use super::{
    PureScalarEvaluator, RuntimeI64Args, RuntimePureFunctionRef, VmPureFunctionScratch,
    validate_helper_result,
};
use crate::plan::{RuntimePureInputType, RuntimePureOutputType};
use crate::value::{RuntimeEvalError, RuntimeValue, runtime_value_label};

/// A complete i64 batch header checked before execution, promotion or counters.
/// Copying this capability copies only immutable borrows and the row count.
/// It does not admit a caller's control budget or prove the function total.
#[derive(Clone, Copy, Debug)]
pub struct RuntimePureRepeatedI64Batch<'plan, 'row> {
    function: RuntimePureFunctionRef<'plan>,
    row: &'row [i64],
    rows: usize,
}

impl<'plan> RuntimePureFunctionRef<'plan> {
    /// Borrows one complete row for repeated evaluation of this original body.
    /// Every whole formal, including an unused one, must have the i64 ABI.
    pub fn repeated_i64_batch<'row>(
        self,
        row: &'row [i64],
        rows: usize,
    ) -> Result<RuntimePureRepeatedI64Batch<'plan, 'row>, RuntimeEvalError> {
        if row.len() > RuntimeI64Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: self.name.to_owned(),
                max: RuntimeI64Args::MAX,
                found: row.len(),
            });
        }
        if self.inputs.len() != row.len() {
            return Err(RuntimeEvalError::FunctionArgumentCount {
                expected: self.inputs.len(),
                found: row.len(),
            });
        }
        if self.output_type != RuntimePureOutputType::I64
            || !self
                .inputs
                .iter()
                .all(|input| input.abi() == RuntimePureInputType::I64)
        {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: self.name.to_owned(),
                reason: "repeated i64 batch does not match the complete function signature"
                    .to_owned(),
            });
        }
        i64::try_from(rows).map_err(|_| RuntimeEvalError::UnsupportedPure {
            name: self.name.to_owned(),
            reason: "pure repeated batch row count must fit i64".to_owned(),
        })?;
        Ok(RuntimePureRepeatedI64Batch {
            function: self,
            row,
            rows,
        })
    }
}

impl<'plan, 'row> RuntimePureRepeatedI64Batch<'plan, 'row> {
    pub const fn function(self) -> RuntimePureFunctionRef<'plan> {
        self.function
    }

    pub const fn row(self) -> &'row [i64] {
        self.row
    }

    pub const fn rows(self) -> usize {
        self.rows
    }

    /// Evaluates one attempted row through Core's original expression evaluator.
    /// Scalar bodies inspect the borrowed row directly. General binding keeps
    /// its existing scratch owner; executable bodies require caller control.
    pub fn evaluate_vm_row(
        self,
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<i64, RuntimeEvalError> {
        let function = self.function;
        if function.scalar_eval_supported
            && function.supports_scalar_frame()
            && let Some(expr) = function.body.expression()
        {
            let result =
                PureScalarEvaluator::new(function.inputs, self.row, super::RuntimePureScalar::I64)
                    .evaluate(expr)
                    .map(super::RuntimePureScalar::into_runtime_value);
            if !matches!(result, Err(RuntimeEvalError::UnsupportedPure { .. })) {
                return exact_i64(validate_helper_result(function, result)?);
            }
        }
        exact_i64(scratch.evaluate_i64_slice(function.plan(), function.id(), self.row)?)
    }
}

fn exact_i64(value: RuntimeValue) -> Result<i64, RuntimeEvalError> {
    match value {
        RuntimeValue::Int(value) => value.exact_i64().ok_or_else(|| {
            RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(value)))
        }),
        value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
    }
}
