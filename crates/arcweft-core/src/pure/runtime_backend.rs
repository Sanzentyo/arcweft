use super::{
    RuntimeExternalCallBackend, RuntimeFixedArgs, RuntimeFloat32Args, RuntimeFloat64Args,
    RuntimeI32Args, RuntimeI64Args, RuntimeMathCallBackend, RuntimePureCallBackend,
    RuntimePureHelperRef, RuntimePureScalarInteger, VmRuntimePureCallBackend,
};
use crate::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
use crate::plan::{RuntimePureInputType, RuntimePureOutputType};
use crate::step::RuntimePureCallStats;
use crate::value::{
    RuntimeCallTarget, RuntimeEvalError, RuntimeExactInteger, RuntimeIntrinsic, RuntimeValue,
    runtime_value_label,
};

impl RuntimePureCallBackend for VmRuntimePureCallBackend {
    fn record_awbc_pure_program_call(&mut self) {
        self.stats.awbc_pure_program_calls = self.stats.awbc_pure_program_calls.saturating_add(1);
    }

    fn call_i8_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[i8],
    ) -> Result<Option<i8>, RuntimeEvalError> {
        self.call_exact_int_slice(helper, args)
    }

    fn call_i8_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i8],
        arity: usize,
        out: &mut [i8],
    ) -> Result<(), RuntimeEvalError> {
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_i8_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i8],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_i16_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[i16],
    ) -> Result<Option<i16>, RuntimeEvalError> {
        self.call_exact_int_slice(helper, args)
    }

    fn call_i16_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i16],
        arity: usize,
        out: &mut [i16],
    ) -> Result<(), RuntimeEvalError> {
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_i16_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i16],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_i128_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i128],
        arity: usize,
        out: &mut [i128],
    ) -> Result<(), RuntimeEvalError> {
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_i128_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i128],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_i32(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: RuntimeI32Args,
    ) -> Result<Option<i32>, RuntimeEvalError> {
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_stack_packs += 1;
        self.stats.arg_bytes_copied += args.len() * std::mem::size_of::<i32>();
        let value = self
            .scratch
            .evaluate_i32_args(helper.plan(), helper.id(), args)?;
        runtime_value_into_i32_result(helper, value).map(Some)
    }

    fn call_i32_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[i32],
    ) -> Result<Option<i32>, RuntimeEvalError> {
        if args.len() > RuntimeI32Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.declaration().name.clone(),
                max: RuntimeI32Args::MAX,
                found: args.len(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        let value = self
            .scratch
            .evaluate_i32_slice(helper.plan(), helper.id(), args)?;
        runtime_value_into_i32_result(helper, value).map(Some)
    }

    fn call_i32_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i32],
        arity: usize,
        out: &mut [i32],
    ) -> Result<(), RuntimeEvalError> {
        if arity > RuntimeI32Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.declaration().name.clone(),
                max: RuntimeI32Args::MAX,
                found: arity,
            });
        }
        if flat_inputs.len() != out.len().saturating_mul(arity) {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: helper.declaration().name.clone(),
                reason: format!(
                    "pure flat batch expected {} input value(s), got {}",
                    out.len().saturating_mul(arity),
                    flat_inputs.len()
                ),
            });
        }
        self.stats.batch_calls += 1;
        self.stats.batch_items += out.len();
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += out.len();
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += out.len();
        self.stats.vm_calls += out.len();
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.result_bytes_copied += std::mem::size_of_val(out);
        if arity == 0 {
            return out.iter_mut().try_for_each(|slot| {
                let value = self
                    .scratch
                    .evaluate_i32_slice(helper.plan(), helper.id(), &[])?;
                *slot = runtime_value_into_i32_result(helper, value)?;
                Ok(())
            });
        }
        flat_inputs
            .chunks_exact(arity)
            .zip(out.iter_mut())
            .try_for_each(|(row, slot)| {
                let value = self
                    .scratch
                    .evaluate_i32_slice(helper.plan(), helper.id(), row)?;
                *slot = runtime_value_into_i32_result(helper, value)?;
                Ok(())
            })
    }

    fn call_i32_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i32],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        if arity > RuntimeI32Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.declaration().name.clone(),
                max: RuntimeI32Args::MAX,
                found: arity,
            });
        }
        if flat_inputs.len() != rows.saturating_mul(arity) {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: helper.declaration().name.clone(),
                reason: format!(
                    "pure flat batch expected {} input value(s), got {}",
                    rows.saturating_mul(arity),
                    flat_inputs.len()
                ),
            });
        }
        self.stats.batch_calls += 1;
        self.stats.batch_items += rows;
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += rows;
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += rows;
        self.stats.vm_calls += rows;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        let mut sum = 0_i64;
        if arity == 0 {
            for _ in 0..rows {
                let value = self
                    .scratch
                    .evaluate_i32_slice(helper.plan(), helper.id(), &[])?;
                sum += i64::from(runtime_value_into_i32_result(helper, value)?);
            }
            return Ok(sum);
        }
        for row in flat_inputs.chunks_exact(arity) {
            let value = self
                .scratch
                .evaluate_i32_slice(helper.plan(), helper.id(), row)?;
            sum += i64::from(runtime_value_into_i32_result(helper, value)?);
        }
        Ok(sum)
    }

    fn call_u32_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[u32],
    ) -> Result<Option<u32>, RuntimeEvalError> {
        self.call_exact_int_slice(helper, args)
    }

    fn call_u8_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[u8],
    ) -> Result<Option<u8>, RuntimeEvalError> {
        self.call_exact_int_slice(helper, args)
    }

    fn call_u8_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u8],
        arity: usize,
        out: &mut [u8],
    ) -> Result<(), RuntimeEvalError> {
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_u8_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u8],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_u16_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[u16],
    ) -> Result<Option<u16>, RuntimeEvalError> {
        self.call_exact_int_slice(helper, args)
    }

    fn call_u16_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u16],
        arity: usize,
        out: &mut [u16],
    ) -> Result<(), RuntimeEvalError> {
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_u16_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u16],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_u128_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u128],
        arity: usize,
        out: &mut [u128],
    ) -> Result<(), RuntimeEvalError> {
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_u128_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u128],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_u32_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u32],
        arity: usize,
        out: &mut [u32],
    ) -> Result<(), RuntimeEvalError> {
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_u32_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u32],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_u64_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[u64],
    ) -> Result<Option<u64>, RuntimeEvalError> {
        self.call_exact_int_slice(helper, args)
    }

    fn call_u64_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u64],
        arity: usize,
        out: &mut [u64],
    ) -> Result<(), RuntimeEvalError> {
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_u64_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u64],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_exact_int_slice<T: RuntimePureScalarInteger>(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[T],
    ) -> Result<Option<T>, RuntimeEvalError> {
        if args.len() > RuntimeFixedArgs::<T>::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.declaration().name.clone(),
                max: RuntimeFixedArgs::<T>::MAX,
                found: args.len(),
            });
        }
        if helper.declaration().output_type != T::OUTPUT_TYPE
            || helper.declaration().input_types.len() != helper.declaration().input_locals.len()
            || !helper
                .declaration()
                .input_types
                .iter()
                .all(|input| *input == T::INPUT_TYPE)
        {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: helper.declaration().name.clone(),
                reason: "exact integer call type does not match helper signature".to_owned(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        let value = self
            .scratch
            .evaluate_exact_int_slice::<T>(helper.plan(), helper.id(), args)?;
        T::try_from_runtime_value(&helper.declaration().name, value).map(Some)
    }

    fn call_exact_int_flat_batch<T: RuntimePureScalarInteger>(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[T],
        arity: usize,
        out: &mut [T],
    ) -> Result<(), RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<T>(helper, flat_inputs.len(), arity, out.len())?;
        self.stats.batch_calls += 1;
        self.stats.batch_items += out.len();
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += out.len();
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += out.len();
        self.stats.vm_calls += out.len();
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.result_bytes_copied += std::mem::size_of_val(out);
        if arity == 0 {
            return out.iter_mut().try_for_each(|slot| {
                let value =
                    self.scratch
                        .evaluate_exact_int_slice::<T>(helper.plan(), helper.id(), &[])?;
                *slot = T::try_from_runtime_value(&helper.declaration().name, value)?;
                Ok(())
            });
        }
        flat_inputs
            .chunks_exact(arity)
            .zip(out.iter_mut())
            .try_for_each(|(row, slot)| {
                let value =
                    self.scratch
                        .evaluate_exact_int_slice::<T>(helper.plan(), helper.id(), row)?;
                *slot = T::try_from_runtime_value(&helper.declaration().name, value)?;
                Ok(())
            })
    }

    fn call_exact_int_flat_batch_sum<T: RuntimePureScalarInteger>(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[T],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<T>(helper, flat_inputs.len(), arity, rows)?;
        self.stats.batch_calls += 1;
        self.stats.batch_items += rows;
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += rows;
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += rows;
        self.stats.vm_calls += rows;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        let mut sum = 0_i64;
        if arity == 0 {
            for _ in 0..rows {
                let value =
                    self.scratch
                        .evaluate_exact_int_slice::<T>(helper.plan(), helper.id(), &[])?;
                sum += T::try_from_runtime_value(&helper.declaration().name, value)?
                    .try_sum_as_i64(&helper.declaration().name)?;
            }
            return Ok(sum);
        }
        for row in flat_inputs.chunks_exact(arity) {
            let value =
                self.scratch
                    .evaluate_exact_int_slice::<T>(helper.plan(), helper.id(), row)?;
            sum += T::try_from_runtime_value(&helper.declaration().name, value)?
                .try_sum_as_i64(&helper.declaration().name)?;
        }
        Ok(sum)
    }

    fn call_i64(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: RuntimeI64Args,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_stack_packs += 1;
        self.stats.arg_bytes_copied += args.len() * std::mem::size_of::<i64>();
        let value = self
            .scratch
            .evaluate_i64_args(helper.plan(), helper.id(), args)?;
        match value {
            RuntimeValue::Int(value) => value.exact_i64().map(Some).ok_or_else(|| {
                RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(value)))
            }),
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
        }
    }

    fn call_i64_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[i64],
    ) -> Result<Option<i64>, RuntimeEvalError> {
        if args.len() > RuntimeI64Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.declaration().name.clone(),
                max: RuntimeI64Args::MAX,
                found: args.len(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        let value = self
            .scratch
            .evaluate_i64_slice(helper.plan(), helper.id(), args)?;
        match value {
            RuntimeValue::Int(value) => value.exact_i64().map(Some).ok_or_else(|| {
                RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(value)))
            }),
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
        }
    }

    fn call_i64_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        rows: &[RuntimeI64Args],
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError> {
        if rows.len() != out.len() {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: helper.declaration().name.clone(),
                reason: format!(
                    "pure batch expected {} output slot(s), got {}",
                    rows.len(),
                    out.len()
                ),
            });
        }
        self.stats.batch_calls += 1;
        self.stats.batch_items += rows.len();
        self.stats.pure_calls += rows.len();
        self.stats.vm_calls += rows.len();
        self.stats.arg_stack_packs += rows.len();
        self.stats.arg_bytes_copied += rows
            .iter()
            .map(|row| row.len() * std::mem::size_of::<i64>())
            .sum::<usize>();
        self.stats.result_bytes_copied += std::mem::size_of_val(out);
        rows.iter().zip(out.iter_mut()).try_for_each(|(row, slot)| {
            let value = self
                .scratch
                .evaluate_i64_args(helper.plan(), helper.id(), *row)?;
            match value {
                RuntimeValue::Int(value) => {
                    *slot = value.exact_i64().ok_or_else(|| {
                        RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(
                            value,
                        )))
                    })?;
                    Ok(())
                }
                value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
            }
        })
    }

    fn call_i64_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i64],
        arity: usize,
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError> {
        if arity > RuntimeI64Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.declaration().name.clone(),
                max: RuntimeI64Args::MAX,
                found: arity,
            });
        }
        if flat_inputs.len() != out.len().saturating_mul(arity) {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: helper.declaration().name.clone(),
                reason: format!(
                    "pure flat batch expected {} input value(s), got {}",
                    out.len().saturating_mul(arity),
                    flat_inputs.len()
                ),
            });
        }
        self.stats.batch_calls += 1;
        self.stats.batch_items += out.len();
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += out.len();
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += out.len();
        self.stats.vm_calls += out.len();
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.result_bytes_copied += std::mem::size_of_val(out);
        if arity == 0 {
            return out.iter_mut().try_for_each(|slot| {
                let value = self
                    .scratch
                    .evaluate_i64_slice(helper.plan(), helper.id(), &[])?;
                match value {
                    RuntimeValue::Int(value) => {
                        *slot = value.exact_i64().ok_or_else(|| {
                            RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(
                                value,
                            )))
                        })?;
                        Ok(())
                    }
                    value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
                }
            });
        }
        flat_inputs
            .chunks_exact(arity)
            .zip(out.iter_mut())
            .try_for_each(|(row, slot)| {
                let value = self
                    .scratch
                    .evaluate_i64_slice(helper.plan(), helper.id(), row)?;
                match value {
                    RuntimeValue::Int(value) => {
                        *slot = value.exact_i64().ok_or_else(|| {
                            RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(
                                value,
                            )))
                        })?;
                        Ok(())
                    }
                    value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
                }
            })
    }

    fn call_i64_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i64],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        if arity > RuntimeI64Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.declaration().name.clone(),
                max: RuntimeI64Args::MAX,
                found: arity,
            });
        }
        if flat_inputs.len() != rows.saturating_mul(arity) {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: helper.declaration().name.clone(),
                reason: format!(
                    "pure flat batch expected {} input value(s), got {}",
                    rows.saturating_mul(arity),
                    flat_inputs.len()
                ),
            });
        }
        self.stats.batch_calls += 1;
        self.stats.batch_items += rows;
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += rows;
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += rows;
        self.stats.vm_calls += rows;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        let mut sum = 0i64;
        if arity == 0 {
            for _ in 0..rows {
                match self
                    .scratch
                    .evaluate_i64_slice(helper.plan(), helper.id(), &[])?
                {
                    RuntimeValue::Int(value) => {
                        sum += value.exact_i64().ok_or_else(|| {
                            RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(
                                value,
                            )))
                        })?;
                    }
                    value => {
                        return Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value)));
                    }
                }
            }
            return Ok(sum);
        }
        for row in flat_inputs.chunks_exact(arity) {
            match self
                .scratch
                .evaluate_i64_slice(helper.plan(), helper.id(), row)?
            {
                RuntimeValue::Int(value) => {
                    sum += value.exact_i64().ok_or_else(|| {
                        RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(
                            value,
                        )))
                    })?;
                }
                value => return Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
            }
        }
        Ok(sum)
    }

    fn call_i64_repeated_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        row: &[i64],
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        if row.len() > RuntimeI64Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.declaration().name.clone(),
                max: RuntimeI64Args::MAX,
                found: row.len(),
            });
        }
        self.stats.batch_calls += usize::from(rows > 0);
        self.stats.batch_items += rows;
        self.stats.flat_batch_calls += usize::from(rows > 0);
        self.stats.flat_batch_items += rows;
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(row);
        self.stats.pure_calls += rows;
        self.stats.vm_calls += rows;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(row);
        if rows == 0 {
            return Ok(0);
        }
        let value = match self
            .scratch
            .evaluate_i64_slice(helper.plan(), helper.id(), row)?
        {
            RuntimeValue::Int(value) => value.exact_i64().ok_or_else(|| {
                RuntimeEvalError::ExpectedInt(runtime_value_label(&RuntimeValue::Int(value)))
            })?,
            value => return Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
        };
        let rows = i64::try_from(rows).map_err(|_| RuntimeEvalError::UnsupportedPure {
            name: helper.declaration().name.clone(),
            reason: "pure repeated batch row count must fit i64".to_owned(),
        })?;
        value
            .checked_mul(rows)
            .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                name: helper.declaration().name.clone(),
                reason: "pure repeated batch sum overflowed i64".to_owned(),
            })
    }

    fn call_f32_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[f32],
    ) -> Result<Option<f32>, RuntimeEvalError> {
        if args.len() > RuntimeFloat32Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.declaration().name.clone(),
                max: RuntimeFloat32Args::MAX,
                found: args.len(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        let value = self
            .scratch
            .evaluate_f32_slice(helper.plan(), helper.id(), args)?;
        runtime_value_into_f32_result(helper, value).map(Some)
    }

    fn call_f32_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[f32],
        arity: usize,
        out: &mut [f32],
    ) -> Result<(), RuntimeEvalError> {
        validate_float_flat_batch_shape(
            helper,
            RuntimePureInputType::F32,
            RuntimePureOutputType::F32,
            RuntimeFloat32Args::MAX,
            flat_inputs.len(),
            arity,
            out.len(),
        )?;
        self.stats.batch_calls += 1;
        self.stats.batch_items += out.len();
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += out.len();
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += out.len();
        self.stats.vm_calls += out.len();
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.result_bytes_copied += std::mem::size_of_val(out);
        if arity == 0 {
            return out.iter_mut().try_for_each(|slot| {
                let value = self
                    .scratch
                    .evaluate_f32_slice(helper.plan(), helper.id(), &[])?;
                *slot = runtime_value_into_f32_result(helper, value)?;
                Ok(())
            });
        }
        flat_inputs
            .chunks_exact(arity)
            .zip(out.iter_mut())
            .try_for_each(|(row, slot)| {
                let value = self
                    .scratch
                    .evaluate_f32_slice(helper.plan(), helper.id(), row)?;
                *slot = runtime_value_into_f32_result(helper, value)?;
                Ok(())
            })
    }

    fn call_f64_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[f64],
    ) -> Result<Option<f64>, RuntimeEvalError> {
        if args.len() > RuntimeFloat64Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.declaration().name.clone(),
                max: RuntimeFloat64Args::MAX,
                found: args.len(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        let value = self
            .scratch
            .evaluate_f64_slice(helper.plan(), helper.id(), args)?;
        runtime_value_into_f64_result(helper, value).map(Some)
    }

    fn call_f64_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[f64],
        arity: usize,
        out: &mut [f64],
    ) -> Result<(), RuntimeEvalError> {
        validate_float_flat_batch_shape(
            helper,
            RuntimePureInputType::F64,
            RuntimePureOutputType::F64,
            RuntimeFloat64Args::MAX,
            flat_inputs.len(),
            arity,
            out.len(),
        )?;
        self.stats.batch_calls += 1;
        self.stats.batch_items += out.len();
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += out.len();
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += out.len();
        self.stats.vm_calls += out.len();
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.result_bytes_copied += std::mem::size_of_val(out);
        if arity == 0 {
            return out.iter_mut().try_for_each(|slot| {
                let value = self
                    .scratch
                    .evaluate_f64_slice(helper.plan(), helper.id(), &[])?;
                *slot = runtime_value_into_f64_result(helper, value)?;
                Ok(())
            });
        }
        flat_inputs
            .chunks_exact(arity)
            .zip(out.iter_mut())
            .try_for_each(|(row, slot)| {
                let value = self
                    .scratch
                    .evaluate_f64_slice(helper.plan(), helper.id(), row)?;
                *slot = runtime_value_into_f64_result(helper, value)?;
                Ok(())
            })
    }

    fn call_values(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if args.len() != helper.declaration().input_locals.len() {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.declaration().name.clone(),
                max: helper.declaration().input_locals.len(),
                found: args.len(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.fallbacks += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        self.scratch
            .evaluate_values(helper.plan(), helper.id(), args)
    }

    fn stats(&self) -> RuntimePureCallStats {
        self.stats
    }
}

impl RuntimeMathCallBackend for VmRuntimePureCallBackend {
    fn call_math_matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        lhs.matmul_scalar(rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathMatmulF32.as_label().to_owned(),
                reason: error.to_string(),
            })
    }

    fn call_math_matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        lhs.add_scalar(rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathMatrixAddF32.as_label().to_owned(),
                reason: error.to_string(),
            })
    }

    fn call_math_tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        lhs.add_scalar(rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathTensorAddF32.as_label().to_owned(),
                reason: error.to_string(),
            })
    }

    fn call_math_matmul_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeEvalError> {
        self.stats.math_calls += 1;
        lhs.matmul_scalar(rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathMatmulF64.as_label().to_owned(),
                reason: error.to_string(),
            })
    }

    fn call_math_matrix_add_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeEvalError> {
        self.stats.math_calls += 1;
        lhs.add_scalar(rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathMatrixAddF64.as_label().to_owned(),
                reason: error.to_string(),
            })
    }

    fn call_math_tensor_add_f64(
        &mut self,
        lhs: &DenseTensorF64,
        rhs: &DenseTensorF64,
    ) -> Result<DenseTensorF64, RuntimeEvalError> {
        self.stats.math_calls += 1;
        lhs.add_scalar(rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathTensorAddF64.as_label().to_owned(),
                reason: error.to_string(),
            })
    }
}

impl RuntimeExternalCallBackend for VmRuntimePureCallBackend {
    fn call_external(
        &mut self,
        _callee: &RuntimeCallTarget,
        _args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        None
    }
}

fn runtime_value_into_i32_result(
    helper: RuntimePureHelperRef<'_>,
    value: RuntimeValue,
) -> Result<i32, RuntimeEvalError> {
    i32::try_from_runtime_value(&helper.declaration().name, value)
}

fn runtime_value_into_f32_result(
    helper: RuntimePureHelperRef<'_>,
    value: RuntimeValue,
) -> Result<f32, RuntimeEvalError> {
    match value {
        RuntimeValue::F32(value) => Ok(value),
        value => Err(RuntimeEvalError::UnsupportedPure {
            name: helper.declaration().name.clone(),
            reason: format!(
                "pure f32 result expected f32, got {}",
                runtime_value_label(&value)
            ),
        }),
    }
}

fn runtime_value_into_f64_result(
    helper: RuntimePureHelperRef<'_>,
    value: RuntimeValue,
) -> Result<f64, RuntimeEvalError> {
    match value {
        RuntimeValue::F64(value) => Ok(value),
        value => Err(RuntimeEvalError::UnsupportedPure {
            name: helper.declaration().name.clone(),
            reason: format!(
                "pure f64 result expected f64, got {}",
                runtime_value_label(&value)
            ),
        }),
    }
}

fn validate_exact_int_flat_batch_shape<T: RuntimePureScalarInteger>(
    helper: RuntimePureHelperRef<'_>,
    flat_input_len: usize,
    arity: usize,
    rows: usize,
) -> Result<(), RuntimeEvalError> {
    if arity > RuntimeFixedArgs::<T>::MAX {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: helper.declaration().name.clone(),
            max: RuntimeFixedArgs::<T>::MAX,
            found: arity,
        });
    }
    if helper.declaration().output_type != T::OUTPUT_TYPE
        || helper.declaration().input_types.len() != helper.declaration().input_locals.len()
        || !helper
            .declaration()
            .input_types
            .iter()
            .all(|input| *input == T::INPUT_TYPE)
    {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.declaration().name.clone(),
            reason: "exact integer batch type does not match helper signature".to_owned(),
        });
    }
    if flat_input_len != rows.saturating_mul(arity) {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.declaration().name.clone(),
            reason: format!(
                "pure flat batch expected {} input value(s), got {}",
                rows.saturating_mul(arity),
                flat_input_len
            ),
        });
    }
    Ok(())
}

fn validate_float_flat_batch_shape(
    helper: RuntimePureHelperRef<'_>,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    max_arity: usize,
    flat_input_len: usize,
    arity: usize,
    rows: usize,
) -> Result<(), RuntimeEvalError> {
    if arity > max_arity {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: helper.declaration().name.clone(),
            max: max_arity,
            found: arity,
        });
    }
    if helper.declaration().output_type != output_type
        || helper.declaration().input_types.len() != helper.declaration().input_locals.len()
        || !helper
            .declaration()
            .input_types
            .iter()
            .all(|input| *input == input_type)
    {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.declaration().name.clone(),
            reason: "float batch type does not match helper signature".to_owned(),
        });
    }
    if flat_input_len != rows.saturating_mul(arity) {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.declaration().name.clone(),
            reason: format!(
                "pure flat batch expected {} input value(s), got {}",
                rows.saturating_mul(arity),
                flat_input_len
            ),
        });
    }
    Ok(())
}
