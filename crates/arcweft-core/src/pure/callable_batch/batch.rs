//! Complete physical numeric Map packets, admitted before input materialization.

use super::{
    RuntimeNumericMapInput, RuntimeNumericMapProjection, RuntimePureFunctionRef, RuntimePureScalar,
    runtime_value_as_scalar,
};
use crate::value::{RuntimeEvalError, RuntimeExactInteger, RuntimeSeq, RuntimeValue};
use std::borrow::Cow;

#[cfg(test)]
mod tests;

#[derive(Debug)]
enum Rows<T> {
    Repeated(Vec<T>),
    Dense(Vec<T>),
}

impl<T: Copy> Rows<T> {
    fn into_flat(self, rows: usize) -> Vec<T> {
        match self {
            Self::Repeated(row) => row.repeat(rows),
            Self::Dense(values) => values,
        }
    }
}

#[derive(Debug)]
enum Inputs {
    I8(Rows<i8>),
    I16(Rows<i16>),
    I32(Rows<i32>),
    I64(Rows<i64>),
    I128(Rows<i128>),
    ISize(Rows<crate::value::RuntimeISizeValue>),
    U8(Rows<u8>),
    U16(Rows<u16>),
    U32(Rows<u32>),
    U64(Rows<u64>),
    U128(Rows<u128>),
    USize(Rows<crate::value::RuntimeUSizeValue>),
    F32(Rows<f32>),
    F64(Rows<f64>),
}

#[derive(Clone, Copy, Debug)]
enum ResultMode {
    Values,
    Sum,
}

/// The original function, complete row shape and reduction mode cannot be
/// supplied independently after admission. Repeated storage is one whole row.
#[derive(Debug)]
pub(crate) struct RuntimeNumericMapBatch<'plan> {
    function: RuntimePureFunctionRef<'plan>,
    rows: usize,
    inputs: Inputs,
    result: ResultMode,
}

pub(crate) enum RuntimeNumericMapBatchResult {
    Values(Vec<RuntimeValue>),
    Sum(i64),
}

enum Source<'a> {
    Bound(&'a [&'a RuntimeValue]),
    Sequence(&'a RuntimeSeq),
}

impl Source<'_> {
    fn permits_copy(&self) -> bool {
        match self {
            Self::Bound(items) => items.iter().all(|item| item.ownership().permits_copy()),
            Self::Sequence(items) => items.ownership().permits_copy(),
        }
    }
    fn len(&self) -> usize {
        match self {
            Self::Bound(items) => items.len(),
            Self::Sequence(items) => items.len(),
        }
    }
    fn get(&self, index: usize) -> Option<Cow<'_, RuntimeValue>> {
        match self {
            Self::Bound(items) => items.get(index).map(|item| Cow::Borrowed(*item)),
            Self::Sequence(RuntimeSeq::Values(items)) => items.get(index).map(Cow::Borrowed),
            Self::Sequence(items @ RuntimeSeq::Dense(dense)) if index < items.len() => {
                use crate::value::DenseSeq;
                match dense {
                    DenseSeq::Units(_)
                    | DenseSeq::I8(_)
                    | DenseSeq::I16(_)
                    | DenseSeq::I32(_)
                    | DenseSeq::I64(_)
                    | DenseSeq::I128(_)
                    | DenseSeq::ISize(_)
                    | DenseSeq::U8(_)
                    | DenseSeq::U16(_)
                    | DenseSeq::U32(_)
                    | DenseSeq::U64(_)
                    | DenseSeq::U128(_)
                    | DenseSeq::USize(_)
                    | DenseSeq::F32(_)
                    | DenseSeq::F64(_)
                    | DenseSeq::Bool(_)
                    | DenseSeq::Bytes(_)
                    | DenseSeq::Chars(_)
                    | DenseSeq::Durations(_) => {
                        // These rows are scalar copies, never capture/object graphs.
                        Some(Cow::Owned(items.value_at(index)))
                    }
                    DenseSeq::Strings(_) | DenseSeq::EntityRefs(_) => None,
                }
            }
            Self::Sequence(_) => None,
        }
    }
}

impl<'plan> RuntimeNumericMapProjection<'plan> {
    /// Validates every original input and item before allocating or copying
    /// numeric arguments. Equal physical rows are retained only once.
    pub(crate) fn batch(
        &self,
        items: &[&RuntimeValue],
        fused: bool,
    ) -> Option<RuntimeNumericMapBatch<'plan>> {
        self.batch_source(Source::Bound(items), fused)
    }

    pub(crate) fn batch_sequence(
        &self,
        items: &RuntimeSeq,
        fused: bool,
    ) -> Option<RuntimeNumericMapBatch<'plan>> {
        self.batch_source(Source::Sequence(items), fused)
    }

    fn batch_source(
        &self,
        items: Source<'_>,
        fused: bool,
    ) -> Option<RuntimeNumericMapBatch<'plan>> {
        use crate::plan::{RuntimePureInputType as I, RuntimePureOutputType as O};
        let arity = self.function.inputs.len();
        if arity != self.inputs.len()
            || arity > super::super::RuntimeI64Args::MAX
            || (fused && !self.supports_total_sum())
            || !items.permits_copy()
        {
            return None;
        }
        let input_ty = self
            .function
            .inputs
            .first()
            .map(super::super::RuntimePureFunctionInputRef::abi);
        if self
            .function
            .inputs
            .iter()
            .any(|input| Some(input.abi()) != input_ty)
        {
            return None;
        }
        let rows = items.len();
        i64::try_from(rows).ok()?;
        let has_item = self
            .inputs
            .iter()
            .any(|input| matches!(input, RuntimeNumericMapInput::Item));
        macro_rules! build {
            ($variant:ident, $ty:ty, $convert:expr) => {{
                let convert = |scalar| {
                    if let RuntimePureScalar::$variant(value) = scalar {
                        Some(($convert)(value))
                    } else {
                        None
                    }
                };
                let flat_len = rows.checked_mul(arity)?;
                if flat_len.checked_mul(std::mem::size_of::<$ty>())? > isize::MAX as usize {
                    return None;
                }
                if !fused
                    && rows.checked_mul(
                        std::mem::size_of::<$ty>().max(std::mem::size_of::<RuntimeValue>()),
                    )? > isize::MAX as usize
                {
                    return None;
                }
                for input in &self.inputs {
                    if let RuntimeNumericMapInput::Constant(value) = input {
                        convert(*value)?;
                    }
                }
                let mut first: Option<RuntimePureScalar> = None;
                let mut repeated = true;
                for index in 0..rows {
                    if has_item {
                        let item = items.get(index)?;
                        let scalar = runtime_value_as_scalar(&item)?;
                        convert(scalar)?;
                        if let Some(previous) = first {
                            repeated &= previous.same_physical_representation(scalar);
                        } else {
                            first = Some(scalar);
                        }
                    }
                }
                // Empty batches need no representative row. All constants and
                // the complete function signature have still been checked.
                let repeated = repeated && rows != 0;
                let mut values = Vec::<$ty>::with_capacity(if repeated { arity } else { flat_len });
                for index in 0..if repeated { 1 } else { rows } {
                    for input in &self.inputs {
                        let scalar = match input {
                            RuntimeNumericMapInput::Item => {
                                runtime_value_as_scalar(items.get(index)?.as_ref())?
                            }
                            RuntimeNumericMapInput::Constant(value) => *value,
                        };
                        values.push(convert(scalar)?);
                    }
                }
                Inputs::$variant(if repeated {
                    Rows::Repeated(values)
                } else {
                    Rows::Dense(values)
                })
            }};
        }
        let inputs = match (input_ty, self.function.output_type) {
            (None | Some(I::I8), O::I8) => build!(I8, i8, |v| v),
            (None | Some(I::I16), O::I16) => build!(I16, i16, |v| v),
            (None | Some(I::I32), O::I32) => build!(I32, i32, |v| v),
            (None | Some(I::I64), O::I64) => build!(I64, i64, |v| v),
            (None | Some(I::I128), O::I128) => build!(I128, i128, |v| v),
            (None | Some(I::ISize), O::ISize) => build!(
                ISize,
                crate::value::RuntimeISizeValue,
                crate::value::RuntimeISizeValue::new
            ),
            (None | Some(I::U8), O::U8) => build!(U8, u8, |v| v),
            (None | Some(I::U16), O::U16) => build!(U16, u16, |v| v),
            (None | Some(I::U32), O::U32) => build!(U32, u32, |v| v),
            (None | Some(I::U64), O::U64) => build!(U64, u64, |v| v),
            (None | Some(I::U128), O::U128) => build!(U128, u128, |v| v),
            (None | Some(I::USize), O::USize) => build!(
                USize,
                crate::value::RuntimeUSizeValue,
                crate::value::RuntimeUSizeValue::new
            ),
            (None | Some(I::F32), O::F32) => build!(F32, f32, |v| v),
            (None | Some(I::F64), O::F64) => build!(F64, f64, |v| v),
            _ => return None,
        };
        Some(RuntimeNumericMapBatch {
            function: self.function,
            rows,
            inputs,
            result: if fused {
                ResultMode::Sum
            } else {
                ResultMode::Values
            },
        })
    }
}

impl RuntimePureScalar {
    fn same_physical_representation(self, other: Self) -> bool {
        match (self, other) {
            (Self::F32(lhs), Self::F32(rhs)) => lhs.to_bits() == rhs.to_bits(),
            (Self::F64(lhs), Self::F64(rhs)) => lhs.to_bits() == rhs.to_bits(),
            (lhs, rhs) => lhs == rhs,
        }
    }
}

impl<'plan> RuntimeNumericMapBatch<'plan> {
    pub(crate) const fn function(&self) -> RuntimePureFunctionRef<'plan> {
        self.function
    }

    pub(crate) const fn rows(&self) -> usize {
        self.rows
    }

    pub(crate) fn execute(
        self,
        backend: &mut impl super::super::RuntimePureCallBackend,
    ) -> Result<RuntimeNumericMapBatchResult, RuntimeEvalError> {
        let function = self.function;
        let rows = self.rows;
        let arity = function.inputs.len();
        let fused = matches!(self.result, ResultMode::Sum);
        if rows == 0 {
            return Ok(if fused {
                RuntimeNumericMapBatchResult::Sum(0)
            } else {
                RuntimeNumericMapBatchResult::Values(Vec::new())
            });
        }
        macro_rules! integers {
            ($inputs:expr, $ty:ty, $batch:ident, $sum:ident, $convert:expr) => {{
                let inputs = $inputs.into_flat(rows);
                if fused {
                    backend
                        .$sum(function, &inputs, arity, rows)
                        .map(RuntimeNumericMapBatchResult::Sum)
                } else {
                    let mut output = vec![<$ty>::default(); rows];
                    backend.$batch(function, &inputs, arity, &mut output)?;
                    Ok(RuntimeNumericMapBatchResult::Values(
                        output.into_iter().map($convert).collect(),
                    ))
                }
            }};
        }
        macro_rules! float {
            ($inputs:expr, $ty:ty, $batch:ident, $variant:ident) => {{
                let inputs = $inputs.into_flat(rows);
                let mut output = vec![<$ty>::default(); rows];
                backend.$batch(function, &inputs, arity, &mut output)?;
                Ok(RuntimeNumericMapBatchResult::Values(
                    output.into_iter().map(RuntimeValue::$variant).collect(),
                ))
            }};
        }
        match self.inputs {
            Inputs::I8(inputs) => integers!(
                inputs,
                i8,
                call_i8_flat_batch,
                call_i8_flat_batch_sum,
                RuntimeValue::i8
            ),
            Inputs::I16(inputs) => integers!(
                inputs,
                i16,
                call_i16_flat_batch,
                call_i16_flat_batch_sum,
                RuntimeValue::i16
            ),
            Inputs::I32(inputs) => integers!(
                inputs,
                i32,
                call_i32_flat_batch,
                call_i32_flat_batch_sum,
                RuntimeValue::i32
            ),
            Inputs::I64(Rows::Repeated(inputs)) if fused => backend
                .call_i64_repeated_flat_batch_sum(function.repeated_i64_batch(&inputs, rows)?)
                .map(RuntimeNumericMapBatchResult::Sum),
            Inputs::I64(inputs) => integers!(
                inputs,
                i64,
                call_i64_flat_batch,
                call_i64_flat_batch_sum,
                RuntimeValue::i64
            ),
            Inputs::I128(inputs) => integers!(
                inputs,
                i128,
                call_i128_flat_batch,
                call_i128_flat_batch_sum,
                RuntimeValue::i128
            ),
            Inputs::ISize(inputs) => integers!(
                inputs,
                crate::value::RuntimeISizeValue,
                call_exact_int_flat_batch,
                call_exact_int_flat_batch_sum,
                RuntimeExactInteger::into_runtime_value
            ),
            Inputs::U8(inputs) => integers!(
                inputs,
                u8,
                call_u8_flat_batch,
                call_u8_flat_batch_sum,
                RuntimeValue::u8
            ),
            Inputs::U16(inputs) => integers!(
                inputs,
                u16,
                call_u16_flat_batch,
                call_u16_flat_batch_sum,
                RuntimeValue::u16
            ),
            Inputs::U32(inputs) => integers!(
                inputs,
                u32,
                call_u32_flat_batch,
                call_u32_flat_batch_sum,
                RuntimeValue::u32
            ),
            Inputs::U64(inputs) => integers!(
                inputs,
                u64,
                call_u64_flat_batch,
                call_u64_flat_batch_sum,
                RuntimeValue::u64
            ),
            Inputs::U128(inputs) => integers!(
                inputs,
                u128,
                call_u128_flat_batch,
                call_u128_flat_batch_sum,
                RuntimeValue::u128
            ),
            Inputs::USize(inputs) => integers!(
                inputs,
                crate::value::RuntimeUSizeValue,
                call_exact_int_flat_batch,
                call_exact_int_flat_batch_sum,
                RuntimeExactInteger::into_runtime_value
            ),
            Inputs::F32(inputs) => float!(inputs, f32, call_f32_flat_batch, F32),
            Inputs::F64(inputs) => float!(inputs, f64, call_f64_flat_batch, F64),
        }
    }
}
