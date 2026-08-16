use super::compile::{
    cache_entry, call_aot_exact_int_flat_batch, call_aot_exact_int_flat_batch_sum,
    call_aot_f32_flat_batch, call_aot_f64_flat_batch, call_vm_exact_int_flat_batch,
    call_vm_exact_int_flat_batch_sum, call_vm_f32_flat_batch, call_vm_f64_flat_batch,
    call_vm_i32_flat_batch, call_vm_i32_flat_batch_sum, validate_exact_int_flat_batch_shape,
    validate_exact_int_slice_shape, validate_float_flat_batch_shape,
};
use super::external::infer_runtime_error;
use super::{
    DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64, RuntimeEvalError,
    RuntimeExactInteger, RuntimeFloat32Args, RuntimeFloat64Args, RuntimeI32Args, RuntimeI64Args,
    RuntimeIntrinsic, RuntimeMathCallBackend, RuntimePureAccelerator, RuntimePureBackendMode,
    RuntimePureCacheEntry, RuntimePureCallBackend, RuntimePureCallStats, RuntimePureHelperRef,
    RuntimePureInputType, RuntimePureOutputType, RuntimePureScalarInteger, RuntimeValue,
    call_jit_exact_int_flat_batch, call_jit_exact_int_flat_batch_sum, call_jit_exact_int_slice,
    inference,
};

impl RuntimePureCallBackend for RuntimePureAccelerator {
    fn call_i8_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[i8],
    ) -> Result<Option<i8>, RuntimeEvalError> {
        if let Some(RuntimePureCacheEntry::JitI8(compiled)) = cache_entry(&self.cache, helper.id) {
            validate_exact_int_slice_shape::<i8>(helper, args.len())?;
            self.stats.pure_calls += 1;
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += 1;
            return compiled.call(args).map(Some).map_err(|error| {
                RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: error.to_string(),
                }
            });
        }
        self.call_exact_int_slice(helper, args)
    }

    fn call_i8_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i8],
        arity: usize,
        out: &mut [i8],
    ) -> Result<(), RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<i8>(helper, flat_inputs.len(), arity, out.len())?;
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, out.len());
        }
        if let Some(RuntimePureCacheEntry::JitI8(compiled)) = cache_entry(&self.cache, helper.id) {
            self.stats.batch_calls += 1;
            self.stats.batch_items += out.len();
            self.stats.flat_batch_calls += 1;
            self.stats.flat_batch_items += out.len();
            self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.stats.pure_calls += out.len();
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.stats.result_bytes_copied += std::mem::size_of_val(out);
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += out.len();
            return compiled.call_flat_batch(flat_inputs, out).map_err(|error| {
                RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: error.to_string(),
                }
            });
        }
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_i8_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i8],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<i8>(helper, flat_inputs.len(), arity, rows)?;
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, rows);
        }
        if let Some(RuntimePureCacheEntry::JitI8(compiled)) = cache_entry(&self.cache, helper.id) {
            self.stats.batch_calls += 1;
            self.stats.batch_items += rows;
            self.stats.flat_batch_calls += 1;
            self.stats.flat_batch_items += rows;
            self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.stats.pure_calls += rows;
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += rows;
            return compiled
                .call_flat_batch_sum(flat_inputs, rows)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: error.to_string(),
                });
        }
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_i16_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[i16],
    ) -> Result<Option<i16>, RuntimeEvalError> {
        if let Some(RuntimePureCacheEntry::JitI16(compiled)) = cache_entry(&self.cache, helper.id) {
            validate_exact_int_slice_shape::<i16>(helper, args.len())?;
            self.stats.pure_calls += 1;
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += 1;
            return compiled.call(args).map(Some).map_err(|error| {
                RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: error.to_string(),
                }
            });
        }
        self.call_exact_int_slice(helper, args)
    }

    fn call_i16_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i16],
        arity: usize,
        out: &mut [i16],
    ) -> Result<(), RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<i16>(helper, flat_inputs.len(), arity, out.len())?;
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, out.len());
        }
        if let Some(RuntimePureCacheEntry::JitI16(compiled)) = cache_entry(&self.cache, helper.id) {
            self.stats.batch_calls += 1;
            self.stats.batch_items += out.len();
            self.stats.flat_batch_calls += 1;
            self.stats.flat_batch_items += out.len();
            self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.stats.pure_calls += out.len();
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.stats.result_bytes_copied += std::mem::size_of_val(out);
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += out.len();
            return compiled.call_flat_batch(flat_inputs, out).map_err(|error| {
                RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: error.to_string(),
                }
            });
        }
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_i16_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i16],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<i16>(helper, flat_inputs.len(), arity, rows)?;
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, rows);
        }
        if let Some(RuntimePureCacheEntry::JitI16(compiled)) = cache_entry(&self.cache, helper.id) {
            self.stats.batch_calls += 1;
            self.stats.batch_items += rows;
            self.stats.flat_batch_calls += 1;
            self.stats.flat_batch_items += rows;
            self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.stats.pure_calls += rows;
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += rows;
            return compiled
                .call_flat_batch_sum(flat_inputs, rows)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: error.to_string(),
                });
        }
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_i128_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i128],
        arity: usize,
        out: &mut [i128],
    ) -> Result<(), RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<i128>(helper, flat_inputs.len(), arity, out.len())?;
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, out.len());
        }
        if let Some(RuntimePureCacheEntry::JitI128Batch(compiled)) =
            cache_entry(&self.cache, helper.id)
        {
            self.stats.batch_calls += 1;
            self.stats.batch_items += out.len();
            self.stats.flat_batch_calls += 1;
            self.stats.flat_batch_items += out.len();
            self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.stats.pure_calls += out.len();
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.stats.result_bytes_copied += std::mem::size_of_val(out);
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += out.len();
            return compiled.call_flat_batch(flat_inputs, out).map_err(|error| {
                RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: error.to_string(),
                }
            });
        }
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_i128_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i128],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<i128>(helper, flat_inputs.len(), arity, rows)?;
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, rows);
        }
        if let Some(RuntimePureCacheEntry::JitI128Batch(compiled)) =
            cache_entry(&self.cache, helper.id)
        {
            self.stats.batch_calls += 1;
            self.stats.batch_items += rows;
            self.stats.flat_batch_calls += 1;
            self.stats.flat_batch_items += rows;
            self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.stats.pure_calls += rows;
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += rows;
            return compiled
                .call_flat_batch_sum(flat_inputs, rows)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: error.to_string(),
                });
        }
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_i32(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: RuntimeI32Args,
    ) -> Result<Option<i32>, RuntimeEvalError> {
        self.stats.arg_stack_packs += 1;
        self.stats.arg_bytes_copied += args.len() * std::mem::size_of::<i32>();
        self.call_i32_slice_with_accounting(helper, args.as_slice(), false)
    }

    fn call_i32_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[i32],
    ) -> Result<Option<i32>, RuntimeEvalError> {
        self.call_i32_slice_with_accounting(helper, args, true)
    }

    fn call_i32_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i32],
        arity: usize,
        out: &mut [i32],
    ) -> Result<(), RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<i32>(helper, flat_inputs.len(), arity, out.len())?;
        self.record_flat_batch_stats(flat_inputs, out.len(), true);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, out.len());
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::JitI32(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += out.len();
                compiled.call_flat_batch(flat_inputs, out).map_err(|error| {
                    RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    }
                })
            }
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += out.len();
                call_aot_exact_int_flat_batch(
                    compiled,
                    flat_inputs,
                    arity,
                    out,
                    &mut self.aot_scalar_slots,
                )
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_i32_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_i32_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
        }
    }

    fn call_i32_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i32],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<i32>(helper, flat_inputs.len(), arity, rows)?;
        self.record_flat_batch_stats(flat_inputs, rows, false);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, rows);
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::JitI32(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += rows;
                compiled
                    .call_flat_batch_sum(flat_inputs, rows)
                    .map_err(|error| RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    })
            }
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += rows;
                call_aot_exact_int_flat_batch_sum(
                    compiled,
                    flat_inputs,
                    arity,
                    rows,
                    &mut self.aot_scalar_slots,
                )
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                call_vm_i32_flat_batch_sum(helper, flat_inputs, arity, rows, &mut self.vm_scratch)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                call_vm_i32_flat_batch_sum(helper, flat_inputs, arity, rows, &mut self.vm_scratch)
            }
        }
    }

    fn call_u32_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[u32],
    ) -> Result<Option<u32>, RuntimeEvalError> {
        validate_exact_int_slice_shape::<u32>(helper, args.len())?;
        self.stats.pure_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_scalar_call(helper);
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::JitU32(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += 1;
                compiled
                    .call(args)
                    .map(Some)
                    .map_err(|error| RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    })
            }
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += 1;
                compiled
                    .call_exact_int_with_inputs_scratch(args, &mut self.aot_scalar_slots)
                    .map(|(value, _)| Some(value))
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                let value = self.vm_scratch.evaluate_exact_int_slice::<u32>(
                    helper.plan(),
                    helper.id(),
                    args,
                )?;
                u32::try_from_runtime_value(&helper.name, value).map(Some)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                let value = self.vm_scratch.evaluate_exact_int_slice::<u32>(
                    helper.plan(),
                    helper.id(),
                    args,
                )?;
                u32::try_from_runtime_value(&helper.name, value).map(Some)
            }
        }
    }

    fn call_u8_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[u8],
    ) -> Result<Option<u8>, RuntimeEvalError> {
        if let Some(RuntimePureCacheEntry::JitU8(compiled)) = cache_entry(&self.cache, helper.id) {
            validate_exact_int_slice_shape::<u8>(helper, args.len())?;
            self.stats.pure_calls += 1;
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += 1;
            return compiled.call(args).map(Some).map_err(|error| {
                RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: error.to_string(),
                }
            });
        }
        self.call_exact_int_slice(helper, args)
    }

    fn call_u8_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u8],
        arity: usize,
        out: &mut [u8],
    ) -> Result<(), RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<u8>(helper, flat_inputs.len(), arity, out.len())?;
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, out.len());
        }
        if let Some(RuntimePureCacheEntry::JitU8(compiled)) = cache_entry(&self.cache, helper.id) {
            self.stats.batch_calls += 1;
            self.stats.batch_items += out.len();
            self.stats.flat_batch_calls += 1;
            self.stats.flat_batch_items += out.len();
            self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.stats.pure_calls += out.len();
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.stats.result_bytes_copied += std::mem::size_of_val(out);
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += out.len();
            return compiled.call_flat_batch(flat_inputs, out).map_err(|error| {
                RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: error.to_string(),
                }
            });
        }
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_u8_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u8],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<u8>(helper, flat_inputs.len(), arity, rows)?;
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, rows);
        }
        if let Some(RuntimePureCacheEntry::JitU8(compiled)) = cache_entry(&self.cache, helper.id) {
            self.stats.batch_calls += 1;
            self.stats.batch_items += rows;
            self.stats.flat_batch_calls += 1;
            self.stats.flat_batch_items += rows;
            self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.stats.pure_calls += rows;
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += rows;
            return compiled
                .call_flat_batch_sum(flat_inputs, rows)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: error.to_string(),
                });
        }
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_u16_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[u16],
    ) -> Result<Option<u16>, RuntimeEvalError> {
        if let Some(RuntimePureCacheEntry::JitU16(compiled)) = cache_entry(&self.cache, helper.id) {
            validate_exact_int_slice_shape::<u16>(helper, args.len())?;
            self.stats.pure_calls += 1;
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += 1;
            return compiled.call(args).map(Some).map_err(|error| {
                RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: error.to_string(),
                }
            });
        }
        self.call_exact_int_slice(helper, args)
    }

    fn call_u16_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u16],
        arity: usize,
        out: &mut [u16],
    ) -> Result<(), RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<u16>(helper, flat_inputs.len(), arity, out.len())?;
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, out.len());
        }
        if let Some(RuntimePureCacheEntry::JitU16(compiled)) = cache_entry(&self.cache, helper.id) {
            self.stats.batch_calls += 1;
            self.stats.batch_items += out.len();
            self.stats.flat_batch_calls += 1;
            self.stats.flat_batch_items += out.len();
            self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.stats.pure_calls += out.len();
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.stats.result_bytes_copied += std::mem::size_of_val(out);
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += out.len();
            return compiled.call_flat_batch(flat_inputs, out).map_err(|error| {
                RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: error.to_string(),
                }
            });
        }
        self.call_exact_int_flat_batch(helper, flat_inputs, arity, out)
    }

    fn call_u16_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u16],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<u16>(helper, flat_inputs.len(), arity, rows)?;
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, rows);
        }
        if let Some(RuntimePureCacheEntry::JitU16(compiled)) = cache_entry(&self.cache, helper.id) {
            self.stats.batch_calls += 1;
            self.stats.batch_items += rows;
            self.stats.flat_batch_calls += 1;
            self.stats.flat_batch_items += rows;
            self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.stats.pure_calls += rows;
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += rows;
            return compiled
                .call_flat_batch_sum(flat_inputs, rows)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: error.to_string(),
                });
        }
        self.call_exact_int_flat_batch_sum(helper, flat_inputs, arity, rows)
    }

    fn call_u32_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u32],
        arity: usize,
        out: &mut [u32],
    ) -> Result<(), RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<u32>(helper, flat_inputs.len(), arity, out.len())?;
        self.record_flat_batch_stats(flat_inputs, out.len(), true);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, out.len());
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::JitU32(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += out.len();
                compiled.call_flat_batch(flat_inputs, out).map_err(|error| {
                    RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    }
                })
            }
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += out.len();
                call_aot_exact_int_flat_batch(
                    compiled,
                    flat_inputs,
                    arity,
                    out,
                    &mut self.aot_scalar_slots,
                )
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_exact_int_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_exact_int_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
        }
    }

    fn call_u32_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u32],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<u32>(helper, flat_inputs.len(), arity, rows)?;
        self.record_flat_batch_stats(flat_inputs, rows, false);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, rows);
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::JitU32(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += rows;
                compiled
                    .call_flat_batch_sum(flat_inputs, rows)
                    .map_err(|error| RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    })
            }
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += rows;
                call_aot_exact_int_flat_batch_sum(
                    compiled,
                    flat_inputs,
                    arity,
                    rows,
                    &mut self.aot_scalar_slots,
                )
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                call_vm_exact_int_flat_batch_sum(
                    helper,
                    flat_inputs,
                    arity,
                    rows,
                    &mut self.vm_scratch,
                )
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                call_vm_exact_int_flat_batch_sum(
                    helper,
                    flat_inputs,
                    arity,
                    rows,
                    &mut self.vm_scratch,
                )
            }
        }
    }

    fn call_u64_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[u64],
    ) -> Result<Option<u64>, RuntimeEvalError> {
        validate_exact_int_slice_shape::<u64>(helper, args.len())?;
        self.stats.pure_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_scalar_call(helper);
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::JitU64(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += 1;
                compiled
                    .call(args)
                    .map(Some)
                    .map_err(|error| RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    })
            }
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += 1;
                compiled
                    .call_exact_int_with_inputs_scratch(args, &mut self.aot_scalar_slots)
                    .map(|(value, _)| Some(value))
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                let value = self.vm_scratch.evaluate_exact_int_slice::<u64>(
                    helper.plan(),
                    helper.id(),
                    args,
                )?;
                u64::try_from_runtime_value(&helper.name, value).map(Some)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                let value = self.vm_scratch.evaluate_exact_int_slice::<u64>(
                    helper.plan(),
                    helper.id(),
                    args,
                )?;
                u64::try_from_runtime_value(&helper.name, value).map(Some)
            }
        }
    }

    fn call_u64_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u64],
        arity: usize,
        out: &mut [u64],
    ) -> Result<(), RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<u64>(helper, flat_inputs.len(), arity, out.len())?;
        self.record_flat_batch_stats(flat_inputs, out.len(), true);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, out.len());
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::JitU64(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += out.len();
                compiled.call_flat_batch(flat_inputs, out).map_err(|error| {
                    RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    }
                })
            }
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += out.len();
                call_aot_exact_int_flat_batch(
                    compiled,
                    flat_inputs,
                    arity,
                    out,
                    &mut self.aot_scalar_slots,
                )
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_exact_int_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_exact_int_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
        }
    }

    fn call_u64_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u64],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<u64>(helper, flat_inputs.len(), arity, rows)?;
        self.record_flat_batch_stats(flat_inputs, rows, false);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, rows);
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::JitU64(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += rows;
                compiled
                    .call_flat_batch_sum(flat_inputs, rows)
                    .map_err(|error| RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    })
            }
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += rows;
                call_aot_exact_int_flat_batch_sum(
                    compiled,
                    flat_inputs,
                    arity,
                    rows,
                    &mut self.aot_scalar_slots,
                )
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                call_vm_exact_int_flat_batch_sum(
                    helper,
                    flat_inputs,
                    arity,
                    rows,
                    &mut self.vm_scratch,
                )
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                call_vm_exact_int_flat_batch_sum(
                    helper,
                    flat_inputs,
                    arity,
                    rows,
                    &mut self.vm_scratch,
                )
            }
        }
    }

    fn call_u128_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u128],
        arity: usize,
        out: &mut [u128],
    ) -> Result<(), RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<u128>(helper, flat_inputs.len(), arity, out.len())?;
        self.record_flat_batch_stats(flat_inputs, out.len(), true);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, out.len());
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::JitU128Batch(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += out.len();
                compiled.call_flat_batch(flat_inputs, out).map_err(|error| {
                    RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    }
                })
            }
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += out.len();
                call_aot_exact_int_flat_batch(
                    compiled,
                    flat_inputs,
                    arity,
                    out,
                    &mut self.aot_scalar_slots,
                )
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_exact_int_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_exact_int_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
        }
    }

    fn call_u128_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u128],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<u128>(helper, flat_inputs.len(), arity, rows)?;
        self.record_flat_batch_stats(flat_inputs, rows, false);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, rows);
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::JitU128Batch(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += rows;
                compiled
                    .call_flat_batch_sum(flat_inputs, rows)
                    .map_err(|error| RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    })
            }
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += rows;
                call_aot_exact_int_flat_batch_sum(
                    compiled,
                    flat_inputs,
                    arity,
                    rows,
                    &mut self.aot_scalar_slots,
                )
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                call_vm_exact_int_flat_batch_sum(
                    helper,
                    flat_inputs,
                    arity,
                    rows,
                    &mut self.vm_scratch,
                )
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                call_vm_exact_int_flat_batch_sum(
                    helper,
                    flat_inputs,
                    arity,
                    rows,
                    &mut self.vm_scratch,
                )
            }
        }
    }

    fn call_exact_int_flat_batch_sum<T: RuntimePureScalarInteger>(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[T],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<T>(helper, flat_inputs.len(), arity, rows)?;
        self.record_flat_batch_stats(flat_inputs, rows, false);
        if rows == 0 {
            return Ok(0);
        }
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, rows);
        }
        let entry = cache_entry(&self.cache, helper.id);
        if let Some(entry) = entry
            && let Some(result) =
                call_jit_exact_int_flat_batch_sum(entry, helper, flat_inputs, rows)
        {
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += rows;
            return result;
        }
        match entry {
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += rows;
                call_aot_exact_int_flat_batch_sum(
                    compiled,
                    flat_inputs,
                    arity,
                    rows,
                    &mut self.aot_scalar_slots,
                )
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                call_vm_exact_int_flat_batch_sum(
                    helper,
                    flat_inputs,
                    arity,
                    rows,
                    &mut self.vm_scratch,
                )
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                call_vm_exact_int_flat_batch_sum(
                    helper,
                    flat_inputs,
                    arity,
                    rows,
                    &mut self.vm_scratch,
                )
            }
        }
    }

    fn call_exact_int_slice<T: RuntimePureScalarInteger>(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[T],
    ) -> Result<Option<T>, RuntimeEvalError> {
        validate_exact_int_slice_shape::<T>(helper, args.len())?;
        self.stats.pure_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_scalar_call(helper);
        }
        let entry = cache_entry(&self.cache, helper.id);
        if let Some(entry) = entry
            && let Some(result) = call_jit_exact_int_slice(entry, helper, args)
        {
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += 1;
            return result;
        }
        match entry {
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += 1;
                compiled
                    .call_exact_int_with_inputs_scratch(args, &mut self.aot_scalar_slots)
                    .map(|(value, _)| Some(value))
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                let value = self.vm_scratch.evaluate_exact_int_slice::<T>(
                    helper.plan(),
                    helper.id(),
                    args,
                )?;
                T::try_from_runtime_value(&helper.name, value).map(Some)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                let value = self.vm_scratch.evaluate_exact_int_slice::<T>(
                    helper.plan(),
                    helper.id(),
                    args,
                )?;
                T::try_from_runtime_value(&helper.name, value).map(Some)
            }
        }
    }

    fn call_exact_int_flat_batch<T: RuntimePureScalarInteger>(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[T],
        arity: usize,
        out: &mut [T],
    ) -> Result<(), RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<T>(helper, flat_inputs.len(), arity, out.len())?;
        self.record_flat_batch_stats(flat_inputs, out.len(), true);
        if out.is_empty() {
            return Ok(());
        }
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, out.len());
        }
        let entry = cache_entry(&self.cache, helper.id);
        if let Some(entry) = entry
            && let Some(result) = call_jit_exact_int_flat_batch(entry, helper, flat_inputs, out)
        {
            self.compile_stats.cache_hits += 1;
            self.stats.jit_calls += out.len();
            return result;
        }
        match entry {
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += out.len();
                call_aot_exact_int_flat_batch(
                    compiled,
                    flat_inputs,
                    arity,
                    out,
                    &mut self.aot_scalar_slots,
                )
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_exact_int_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_exact_int_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
        }
    }

    fn call_i64(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: RuntimeI64Args,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        self.stats.pure_calls += 1;
        self.stats.arg_stack_packs += 1;
        self.stats.arg_bytes_copied += args.len() * std::mem::size_of::<i64>();
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_scalar_call(helper);
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::Jit(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += 1;
                compiled.call_i64_args(args).map(Some).map_err(|error| {
                    RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    }
                })
            }
            Some(RuntimePureCacheEntry::Aot(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += 1;
                compiled
                    .call_i64_with_inputs_scratch(args.as_slice(), &mut self.aot_i64_slots)
                    .map(|(value, _)| Some(value))
            }
            Some(RuntimePureCacheEntry::AutoAot { aot, jit, .. }) => {
                self.compile_stats.cache_hits += 1;
                if let Some(compiled) = jit {
                    self.stats.jit_calls += 1;
                    compiled.call_i64_args(args).map(Some).map_err(|error| {
                        RuntimeEvalError::UnsupportedPure {
                            name: helper.name.clone(),
                            reason: error.to_string(),
                        }
                    })
                } else {
                    self.stats.aot_calls += 1;
                    aot.call_i64_with_inputs_scratch(args.as_slice(), &mut self.aot_i64_slots)
                        .map(|(value, _)| Some(value))
                }
            }
            Some(
                RuntimePureCacheEntry::JitI8(_)
                | RuntimePureCacheEntry::JitI16(_)
                | RuntimePureCacheEntry::JitI128Batch(_)
                | RuntimePureCacheEntry::JitI32(_)
                | RuntimePureCacheEntry::JitISize(_)
                | RuntimePureCacheEntry::JitU8(_)
                | RuntimePureCacheEntry::JitU16(_)
                | RuntimePureCacheEntry::JitU32(_)
                | RuntimePureCacheEntry::JitU64(_)
                | RuntimePureCacheEntry::JitU128Batch(_)
                | RuntimePureCacheEntry::JitUSize(_)
                | RuntimePureCacheEntry::JitF32(_)
                | RuntimePureCacheEntry::JitF64(_)
                | RuntimePureCacheEntry::Vm,
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                Self::call_vm_i64(helper, args, &mut self.vm_scratch).map(Some)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                Self::call_vm_i64(helper, args, &mut self.vm_scratch).map(Some)
            }
        }
    }

    fn call_i64_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[i64],
    ) -> Result<Option<i64>, RuntimeEvalError> {
        if args.len() > RuntimeI64Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: RuntimeI64Args::MAX,
                found: args.len(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_scalar_call(helper);
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::Jit(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += 1;
                compiled
                    .call(args)
                    .map(Some)
                    .map_err(|error| RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    })
            }
            Some(RuntimePureCacheEntry::Aot(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += 1;
                compiled
                    .call_i64_with_inputs_scratch(args, &mut self.aot_i64_slots)
                    .map(|(value, _)| Some(value))
            }
            Some(RuntimePureCacheEntry::AutoAot { aot, jit, .. }) => {
                self.compile_stats.cache_hits += 1;
                if let Some(compiled) = jit {
                    self.stats.jit_calls += 1;
                    compiled.call(args).map(Some).map_err(|error| {
                        RuntimeEvalError::UnsupportedPure {
                            name: helper.name.clone(),
                            reason: error.to_string(),
                        }
                    })
                } else {
                    self.stats.aot_calls += 1;
                    aot.call_i64_with_inputs_scratch(args, &mut self.aot_i64_slots)
                        .map(|(value, _)| Some(value))
                }
            }
            Some(
                RuntimePureCacheEntry::JitI8(_)
                | RuntimePureCacheEntry::JitI16(_)
                | RuntimePureCacheEntry::JitI128Batch(_)
                | RuntimePureCacheEntry::JitI32(_)
                | RuntimePureCacheEntry::JitISize(_)
                | RuntimePureCacheEntry::JitU8(_)
                | RuntimePureCacheEntry::JitU16(_)
                | RuntimePureCacheEntry::JitU32(_)
                | RuntimePureCacheEntry::JitU64(_)
                | RuntimePureCacheEntry::JitU128Batch(_)
                | RuntimePureCacheEntry::JitUSize(_)
                | RuntimePureCacheEntry::JitF32(_)
                | RuntimePureCacheEntry::JitF64(_)
                | RuntimePureCacheEntry::Vm,
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                Self::call_vm_i64_slice(helper, args, &mut self.vm_scratch).map(Some)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                Self::call_vm_i64_slice(helper, args, &mut self.vm_scratch).map(Some)
            }
        }
    }

    fn call_i64_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        rows: &[RuntimeI64Args],
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError> {
        Self::call_i64_batch(self, helper, rows, out)
    }

    fn call_i64_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i64],
        arity: usize,
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError> {
        Self::call_i64_flat_batch(self, helper, flat_inputs, arity, out)
    }

    fn call_i64_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i64],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        Self::call_i64_flat_batch_sum(self, helper, flat_inputs, arity, rows)
    }

    fn call_i64_repeated_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        row: &[i64],
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        Self::call_i64_repeated_flat_batch_sum(self, helper, row, rows)
    }

    fn call_f32_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[f32],
    ) -> Result<Option<f32>, RuntimeEvalError> {
        if args.len() > RuntimeFloat32Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: RuntimeFloat32Args::MAX,
                found: args.len(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_scalar_call(helper);
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::JitF32(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += 1;
                compiled
                    .call(args)
                    .map(Some)
                    .map_err(|error| RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    })
            }
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += 1;
                compiled
                    .call_f32_with_inputs_scratch(args, &mut self.aot_scalar_slots)
                    .map(|(value, _)| Some(value))
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                Self::call_vm_f32_slice(helper, args, &mut self.vm_scratch).map(Some)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                Self::call_vm_f32_slice(helper, args, &mut self.vm_scratch).map(Some)
            }
        }
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
        self.record_flat_batch_stats(flat_inputs, out.len(), true);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, out.len());
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::JitF32(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += out.len();
                compiled.call_flat_batch(flat_inputs, out).map_err(|error| {
                    RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    }
                })
            }
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += out.len();
                call_aot_f32_flat_batch(
                    compiled,
                    flat_inputs,
                    arity,
                    out,
                    &mut self.aot_scalar_slots,
                )
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_f32_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_f32_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
        }
    }

    fn call_f64_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[f64],
    ) -> Result<Option<f64>, RuntimeEvalError> {
        if args.len() > RuntimeFloat64Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
                max: RuntimeFloat64Args::MAX,
                found: args.len(),
            });
        }
        self.stats.pure_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_scalar_call(helper);
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::JitF64(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += 1;
                compiled
                    .call(args)
                    .map(Some)
                    .map_err(|error| RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    })
            }
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += 1;
                compiled
                    .call_f64_with_inputs_scratch(args, &mut self.aot_scalar_slots)
                    .map(|(value, _)| Some(value))
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                Self::call_vm_f64_slice(helper, args, &mut self.vm_scratch).map(Some)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                Self::call_vm_f64_slice(helper, args, &mut self.vm_scratch).map(Some)
            }
        }
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
        self.record_flat_batch_stats(flat_inputs, out.len(), true);
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, out.len());
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::JitF64(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += out.len();
                compiled.call_flat_batch(flat_inputs, out).map_err(|error| {
                    RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    }
                })
            }
            Some(
                RuntimePureCacheEntry::Aot(compiled)
                | RuntimePureCacheEntry::AutoAot {
                    aot: compiled,
                    jit: None,
                    ..
                },
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += out.len();
                call_aot_f64_flat_batch(
                    compiled,
                    flat_inputs,
                    arity,
                    out,
                    &mut self.aot_scalar_slots,
                )
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_f64_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_f64_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
        }
    }

    fn call_values(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.fallbacks += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        self.vm_scratch
            .evaluate_values(helper.plan(), helper.id(), args)
    }

    fn stats(&self) -> RuntimePureCallStats {
        self.stats
    }
}

impl RuntimeMathCallBackend for RuntimePureAccelerator {
    fn call_math_matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f32>(lhs.values().len(), rhs.values().len());
        let result = self
            .call_runtime_math_matmul_f32(lhs, rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathMatmulF32.as_label().to_owned(),
                reason: error.to_string(),
            })?;
        self.record_math_result::<f32>(result.values().len());
        Ok(result)
    }

    fn call_math_matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f32>(lhs.values().len(), rhs.values().len());
        let result = self
            .call_runtime_math_matrix_add_f32(lhs, rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathMatrixAddF32.as_label().to_owned(),
                reason: error.to_string(),
            })?;
        self.record_math_result::<f32>(result.values().len());
        Ok(result)
    }

    fn call_math_tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f32>(lhs.values().len(), rhs.values().len());
        let result = self
            .call_runtime_math_tensor_add_f32(lhs, rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathTensorAddF32.as_label().to_owned(),
                reason: error.to_string(),
            })?;
        self.record_math_result::<f32>(result.values().len());
        Ok(result)
    }

    fn call_math_matmul_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f64>(lhs.values().len(), rhs.values().len());
        let result =
            self.math
                .matmul_f64(lhs, rhs)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: RuntimeIntrinsic::MathMatmulF64.as_label().to_owned(),
                    reason: error.to_string(),
                })?;
        self.record_math_result::<f64>(result.values().len());
        Ok(result)
    }

    fn call_math_matrix_add_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f64>(lhs.values().len(), rhs.values().len());
        let result = self.math.matrix_add_f64(lhs, rhs).map_err(|error| {
            RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathMatrixAddF64.as_label().to_owned(),
                reason: error.to_string(),
            }
        })?;
        self.record_math_result::<f64>(result.values().len());
        Ok(result)
    }

    fn call_math_tensor_add_f64(
        &mut self,
        lhs: &DenseTensorF64,
        rhs: &DenseTensorF64,
    ) -> Result<DenseTensorF64, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f64>(lhs.values().len(), rhs.values().len());
        let result = self.math.tensor_add_f64(lhs, rhs).map_err(|error| {
            RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathTensorAddF64.as_label().to_owned(),
                reason: error.to_string(),
            }
        })?;
        self.record_math_result::<f64>(result.values().len());
        Ok(result)
    }
}

impl RuntimePureAccelerator {
    pub(super) fn call_infer_matmul_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f32>(lhs.values().len(), rhs.values().len());
        let lhs = lhs
            .as_matrix()
            .ok_or_else(|| infer_runtime_error("infer.matmul_f32", "expected rank-2 lhs tensor"))?;
        let rhs = rhs
            .as_matrix()
            .ok_or_else(|| infer_runtime_error("infer.matmul_f32", "expected rank-2 rhs tensor"))?;
        let result = self
            .call_runtime_math_matmul_f32(&lhs, &rhs)
            .map(DenseTensorF32::from_matrix)
            .map_err(|error| infer_runtime_error("infer.matmul_f32", error))?;
        self.record_math_result::<f32>(result.values().len());
        Ok(result)
    }

    pub(super) fn call_infer_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f32>(lhs.values().len(), rhs.values().len());
        let result = lhs
            .add_scalar(rhs)
            .map_err(|error| infer_runtime_error("infer.add_f32", error))?;
        self.record_math_result::<f32>(result.values().len());
        Ok(result)
    }

    pub(super) fn call_infer_bias_add_f32(
        &mut self,
        tensor: &DenseTensorF32,
        bias: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f32>(tensor.values().len(), bias.values().len());
        let result = inference::bias_add(tensor, bias)
            .map_err(|error| infer_runtime_error("infer.bias_add_f32", error))?;
        self.record_math_result::<f32>(result.values().len());
        Ok(result)
    }

    pub(super) fn call_infer_matmul_bias_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
        bias: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f32>(
            lhs.values().len().saturating_add(rhs.values().len()),
            bias.values().len(),
        );
        let lhs = lhs.as_matrix().ok_or_else(|| {
            infer_runtime_error("infer.matmul_bias_add_f32", "expected rank-2 lhs tensor")
        })?;
        let rhs = rhs.as_matrix().ok_or_else(|| {
            infer_runtime_error("infer.matmul_bias_add_f32", "expected rank-2 rhs tensor")
        })?;
        let result = self
            .call_runtime_math_matmul_bias_add_f32(&lhs, &rhs, bias)
            .map(DenseTensorF32::from_matrix)
            .map_err(|error| infer_runtime_error("infer.matmul_bias_add_f32", error))?;
        self.record_math_result::<f32>(result.values().len());
        Ok(result)
    }

    pub(super) fn call_conv2d_valid_f32(
        &mut self,
        input: &DenseTensorF32,
        kernel: &DenseTensorF32,
        stride_y: usize,
        stride_x: usize,
    ) -> Result<DenseTensorF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f32>(input.values().len(), kernel.values().len());
        let result = inference::conv2d_valid_nchw(input, kernel, stride_y, stride_x)
            .map_err(|error| infer_runtime_error("conv2d.valid_f32", error))?;
        self.record_math_result::<f32>(result.values().len());
        Ok(result)
    }

    pub(super) fn call_infer_relu_f32(
        &mut self,
        input: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f32>(input.values().len(), 0);
        let result = inference::map_tensor(input, |value| value.max(0.0))
            .map_err(|error| infer_runtime_error("infer.relu_f32", error))?;
        self.record_math_result::<f32>(result.values().len());
        Ok(result)
    }

    pub(super) fn call_infer_max_pool2d_f32(
        &mut self,
        input: &DenseTensorF32,
        kernel_y: usize,
        kernel_x: usize,
        stride_y: usize,
        stride_x: usize,
    ) -> Result<DenseTensorF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f32>(input.values().len(), 0);
        let result = inference::max_pool2d_nchw(input, kernel_y, kernel_x, stride_y, stride_x)
            .map_err(|error| infer_runtime_error("infer.max_pool2d_f32", error))?;
        self.record_math_result::<f32>(result.values().len());
        Ok(result)
    }

    pub(super) fn call_infer_softmax_last_dim_f32(
        &mut self,
        input: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f32>(input.values().len(), 0);
        let result = inference::softmax_last_dim(input)
            .map_err(|error| infer_runtime_error("infer.softmax_last_dim_f32", error))?;
        self.record_math_result::<f32>(result.values().len());
        Ok(result)
    }

    pub(super) fn call_infer_argmax_last_dim_f32(&mut self, input: &DenseTensorF32) -> Vec<usize> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f32>(input.values().len(), 0);
        inference::argmax_last_dim(input)
    }

    pub(super) fn call_infer_flatten_outer_f32(
        &mut self,
        input: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs::<f32>(input.values().len(), 0);
        let result = inference::flatten_outer(input)
            .map_err(|error| infer_runtime_error("infer.flatten_outer_f32", error))?;
        self.record_math_result::<f32>(result.values().len());
        Ok(result)
    }
}
