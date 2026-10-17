//! Complete physical numeric Map packets, admitted before input materialization.

use super::{
    RuntimeNumericMapInput, RuntimeNumericMapProjection, RuntimePureFunctionRef, RuntimePureScalar,
    runtime_value_as_scalar,
};
use crate::pure::function::RuntimeIntegerRange;
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

#[derive(Clone, Copy)]
enum Source<'a> {
    Bound(&'a [&'a RuntimeValue]),
    Sequence(&'a RuntimeSeq),
}

/// Borrowed complete admission. No physical input/result storage exists yet;
/// the caller can admit its original schedule using this selected completion.
pub(crate) struct RuntimeNumericMapAdmission<'plan, 'source> {
    projection: &'source RuntimeNumericMapProjection<'plan>,
    source: Source<'source>,
    repeated: bool,
    result: ResultMode,
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
    /// Validate the complete immutable source before materialization. A Sum
    /// request retains collection completion when its narrowing is unproven.
    pub(crate) fn admit<'source>(
        &'source self,
        items: &'source [&'source RuntimeValue],
        fused: bool,
    ) -> Option<RuntimeNumericMapAdmission<'plan, 'source>> {
        self.admit_source(Source::Bound(items), fused)
    }

    pub(crate) fn admit_sequence<'source>(
        &'source self,
        items: &'source RuntimeSeq,
        fused: bool,
    ) -> Option<RuntimeNumericMapAdmission<'plan, 'source>> {
        self.admit_source(Source::Sequence(items), fused)
    }

    fn admit_source<'source>(
        &'source self,
        items: Source<'source>,
        mut fused: bool,
    ) -> Option<RuntimeNumericMapAdmission<'plan, 'source>> {
        use crate::plan::{RuntimePureInputType as I, RuntimePureOutputType as O};
        let arity = self.function.inputs.len();
        if arity != self.inputs.len()
            || arity > super::super::RuntimeI64Args::MAX
            || (fused
                && matches!(
                    self.function.output_type,
                    O::F32 | O::F64 | O::Bool | O::Value
                ))
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
                for input in &self.inputs {
                    if let RuntimeNumericMapInput::Constant(value) = input {
                        convert(*value)?;
                    }
                }
                let mut first: Option<RuntimePureScalar> = None;
                let mut item_range = None;
                let mut repeated = true;
                for index in 0..rows {
                    if has_item {
                        let item = items.get(index)?;
                        let scalar = runtime_value_as_scalar(&item)?;
                        convert(scalar)?;
                        let range = RuntimeIntegerRange::from_scalar(scalar);
                        item_range =
                            Some(item_range.map_or(range, |previous: RuntimeIntegerRange| {
                                previous.join(range)
                            }));
                        if let Some(previous) = first {
                            repeated &= previous.same_physical_representation(scalar);
                        } else {
                            first = Some(scalar);
                        }
                    }
                }
                if fused && rows != 0 && !self.output_always_fits_sum() {
                    fused = self
                        .function
                        .integer_result_fits_i64(self.inputs.iter().map(|input| match input {
                            RuntimeNumericMapInput::Item => {
                                item_range.unwrap_or(RuntimeIntegerRange::Unknown)
                            }
                            RuntimeNumericMapInput::Constant(value) => {
                                RuntimeIntegerRange::from_scalar(*value)
                            }
                        }));
                }
                if !fused
                    && rows.checked_mul(
                        std::mem::size_of::<$ty>().max(std::mem::size_of::<RuntimeValue>()),
                    )? > isize::MAX as usize
                {
                    return None;
                }
                // Empty batches need no representative row. All constants and
                // the complete function signature have still been checked.
                repeated && rows != 0
            }};
        }
        let repeated = match (input_ty, self.function.output_type) {
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
        Some(RuntimeNumericMapAdmission {
            projection: self,
            source: items,
            repeated,
            result: if fused {
                ResultMode::Sum
            } else {
                ResultMode::Values
            },
        })
    }
}

impl<'plan> RuntimeNumericMapAdmission<'plan, '_> {
    pub(crate) fn is_sum(&self) -> bool {
        matches!(self.result, ResultMode::Sum)
    }

    /// Source, whole inputs, completion and allocation sizes were checked
    /// together. Immutable borrows preserve those facts across budget admission.
    pub(crate) fn materialize(self) -> RuntimeNumericMapBatch<'plan> {
        use crate::plan::RuntimePureOutputType as O;
        let rows = self.source.len();
        let projection = self.projection;
        let arity = projection.inputs.len();
        let flat_len = rows.checked_mul(arity).expect("admitted row/arity product");
        macro_rules! build {
            ($variant:ident, $ty:ty, $convert:expr) => {{
                let mut values =
                    Vec::<$ty>::with_capacity(if self.repeated { arity } else { flat_len });
                for index in 0..if self.repeated { 1 } else { rows } {
                    for input in &projection.inputs {
                        let scalar = match input {
                            RuntimeNumericMapInput::Item => {
                                let item = self
                                    .source
                                    .get(index)
                                    .expect("admitted immutable source row");
                                runtime_value_as_scalar(&item)
                                    .expect("admitted numeric source item")
                            }
                            RuntimeNumericMapInput::Constant(value) => *value,
                        };
                        let RuntimePureScalar::$variant(value) = scalar else {
                            unreachable!("admitted homogeneous whole input ABI")
                        };
                        values.push(($convert)(value));
                    }
                }
                Inputs::$variant(if self.repeated {
                    Rows::Repeated(values)
                } else {
                    Rows::Dense(values)
                })
            }};
        }
        let inputs = match projection.function.output_type {
            O::I8 => build!(I8, i8, |v| v),
            O::I16 => build!(I16, i16, |v| v),
            O::I32 => build!(I32, i32, |v| v),
            O::I64 => build!(I64, i64, |v| v),
            O::I128 => build!(I128, i128, |v| v),
            O::ISize => build!(
                ISize,
                crate::value::RuntimeISizeValue,
                crate::value::RuntimeISizeValue::new
            ),
            O::U8 => build!(U8, u8, |v| v),
            O::U16 => build!(U16, u16, |v| v),
            O::U32 => build!(U32, u32, |v| v),
            O::U64 => build!(U64, u64, |v| v),
            O::U128 => build!(U128, u128, |v| v),
            O::USize => build!(
                USize,
                crate::value::RuntimeUSizeValue,
                crate::value::RuntimeUSizeValue::new
            ),
            O::F32 => build!(F32, f32, |v| v),
            O::F64 => build!(F64, f64, |v| v),
            _ => unreachable!("admitted homogeneous numeric output ABI"),
        };
        RuntimeNumericMapBatch {
            function: projection.function,
            rows,
            inputs,
            result: self.result,
        }
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
