use super::compile::{
    auto_jit_flat_batch_threshold, build_thread_pool, cache_entry, call_aot_batch,
    call_aot_batch_parallel, call_aot_flat_batch, call_aot_flat_batch_parallel,
    call_aot_flat_batch_sum_with_policy, call_jit_batch, call_jit_flat_batch_sum, call_vm_batch,
    call_vm_batch_parallel, call_vm_flat_batch, call_vm_flat_batch_parallel,
    call_vm_flat_batch_sum, call_vm_flat_batch_sum_with_policy, compile_helper, compile_native_jit,
    compile_request, exact_i64_result, helper_cache_slots, helper_work_unit_slots,
    record_aot_object_artifact_bundle, resolve_worker_count, runtime_expr_work_units,
    validate_exact_int_slice_shape, validate_flat_batch_shape,
};
use super::{
    AUTO_JIT_SCALAR_WORK_UNITS, CompiledPureI64Inputs, FlatBatchSumPolicy, FlatBatchSumShape,
    RuntimeBatchBackendKind, RuntimeEvalError, RuntimeI64Args, RuntimeMathPrepareCache,
    RuntimePlan, RuntimePureAccelerator, RuntimePureAcceleratorConfig, RuntimePureBackendMode,
    RuntimePureCacheEntry, RuntimePureCallStats, RuntimePureCompileStats, RuntimePureHelperId,
    RuntimePureHelperRef, RuntimePureNativeKind, VmPureFunctionScratch, helper_native_kind,
    helper_summary_from_helpers, math, native_jit_enabled,
};

impl RuntimePureAccelerator {
    /// Creates an accelerator for the selected pure-function backend.
    pub fn new(mode: RuntimePureBackendMode, plan: &std::sync::Arc<RuntimePlan>) -> Self {
        Self::with_config(
            RuntimePureAcceleratorConfig {
                backend: mode,
                ..RuntimePureAcceleratorConfig::default()
            },
            plan,
        )
    }

    /// Creates an accelerator and eagerly compiles the pure helpers in `plan`.
    ///
    /// # Panics
    ///
    /// Panics when a helper index admitted by `plan` cannot be resolved back
    /// through that same plan. A sealed `RuntimePlan` guarantees this lookup.
    pub fn with_config(
        config: RuntimePureAcceleratorConfig,
        plan: &std::sync::Arc<RuntimePlan>,
    ) -> Self {
        let started = std::time::Instant::now();
        let mut compile_stats = RuntimePureCompileStats::default();
        let helpers = plan.pure_helpers();
        let helper_summary = helper_summary_from_helpers(helpers);
        let helper_work_units = helper_work_unit_slots(helpers);
        let resolved_workers = resolve_worker_count(config.workers);
        let mut cache = helper_cache_slots(helpers);
        for helper in helpers {
            let work_units = helper_work_units
                .get(helper.id.0)
                .copied()
                .unwrap_or_else(|| runtime_expr_work_units(&helper.expr));
            let helper_ref = RuntimePureHelperRef::resolve(plan, helper.id)
                .expect("runtime plan admitted an unresolved pure helper");
            cache[helper.id.0] = Some(compile_helper(
                config.backend,
                helper_ref,
                work_units,
                &mut compile_stats,
            ));
        }
        if config.emit_object_artifacts {
            record_aot_object_artifact_bundle(plan, &cache, &mut compile_stats);
        }
        compile_stats.compile_elapsed_ns = started.elapsed().as_nanos();
        Self {
            config,
            cache,
            stats: RuntimePureCallStats::default(),
            compile_stats,
            helper_summary,
            helper_work_units,
            auto_scalar_work_units: vec![0; helpers.len()],
            pool: None,
            resolved_workers,
            flat_i64_inputs: Vec::new(),
            aot_i64_slots: Vec::new(),
            aot_scalar_slots: Vec::new(),
            vm_scratch: VmPureFunctionScratch::default(),
            math: math::RuntimeMathAccelerator::new(config.math),
            math_prepare_cache: RuntimeMathPrepareCache::default(),
        }
    }

    pub const fn mode(&self) -> RuntimePureBackendMode {
        self.config.backend
    }

    pub const fn config(&self) -> RuntimePureAcceleratorConfig {
        self.config
    }

    pub const fn compile_stats(&self) -> RuntimePureCompileStats {
        self.compile_stats
    }

    pub const fn resolved_worker_count(&self) -> usize {
        self.resolved_workers
    }

    pub const fn has_worker_pool(&self) -> bool {
        self.pool.is_some()
    }

    pub const fn math_stats(&self) -> math::RuntimeMathStats {
        self.math.stats()
    }

    pub fn reset_runtime_counters(&mut self) {
        self.stats = RuntimePureCallStats::default();
        self.compile_stats.cache_hits = 0;
        self.compile_stats.cache_misses = 0;
        self.math.reset_stats();
    }

    pub fn call_i64_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        rows: &[RuntimeI64Args],
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError> {
        if rows.len() != out.len() {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
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
        self.stats.arg_stack_packs += rows.len();
        self.stats.arg_bytes_copied += rows
            .iter()
            .map(|row| row.len() * std::mem::size_of::<i64>())
            .sum::<usize>();
        self.stats.result_bytes_copied += std::mem::size_of_val(out);
        if rows.is_empty() {
            return Ok(());
        }
        let backend = self.batch_backend_kind(helper.id);
        let wants_parallel = self.should_parallelize_batch(helper, rows.len(), backend);
        if wants_parallel {
            self.ensure_thread_pool();
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::Jit(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += rows.len();
                self.stats.flatten_materializations += usize::from(!rows.is_empty());
                self.stats.flatten_bytes_copied += rows
                    .iter()
                    .map(|row| row.len() * std::mem::size_of::<i64>())
                    .sum::<usize>();
                call_jit_batch(compiled, rows, out, helper, &mut self.flat_i64_inputs)
            }
            Some(RuntimePureCacheEntry::Aot(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += rows.len();
                let compiled = compiled.require_i64(helper)?;
                if wants_parallel {
                    self.stats.thread_pool_jobs += self.parallel_jobs(rows.len());
                    call_aot_batch_parallel(self.pool.as_ref(), compiled, rows, out)
                } else {
                    call_aot_batch(compiled, rows, out, &mut self.aot_i64_slots)
                }
            }
            Some(RuntimePureCacheEntry::AutoAot { aot, jit, .. }) => {
                self.compile_stats.cache_hits += 1;
                if let Some(compiled) = jit {
                    self.stats.jit_calls += rows.len();
                    self.stats.flatten_materializations += usize::from(!rows.is_empty());
                    self.stats.flatten_bytes_copied += rows
                        .iter()
                        .map(|row| row.len() * std::mem::size_of::<i64>())
                        .sum::<usize>();
                    call_jit_batch(compiled, rows, out, helper, &mut self.flat_i64_inputs)
                } else {
                    self.stats.aot_calls += rows.len();
                    let aot = aot.require_i64(helper)?;
                    if wants_parallel {
                        self.stats.thread_pool_jobs += self.parallel_jobs(rows.len());
                        call_aot_batch_parallel(self.pool.as_ref(), aot, rows, out)
                    } else {
                        call_aot_batch(aot, rows, out, &mut self.aot_i64_slots)
                    }
                }
            }
            Some(entry) => {
                debug_assert!(entry.is_non_i64_runtime_fallback());
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += rows.len();
                self.stats.fallbacks += rows.len();
                if wants_parallel {
                    self.stats.thread_pool_jobs += self.parallel_jobs(rows.len());
                    call_vm_batch_parallel(self.pool.as_ref(), helper, rows, out)
                } else {
                    call_vm_batch(helper, rows, out, &mut self.vm_scratch)
                }
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += rows.len();
                self.stats.fallbacks += rows.len();
                call_vm_batch(helper, rows, out, &mut self.vm_scratch)
            }
        }
    }

    pub fn call_i64_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i64],
        arity: usize,
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError> {
        validate_flat_batch_shape(helper, flat_inputs.len(), arity, out.len())?;
        self.record_flat_batch_stats(flat_inputs, out.len(), true);
        if out.is_empty() {
            return Ok(());
        }
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, out.len());
        }
        let backend = self.batch_backend_kind(helper.id);
        let wants_parallel = self.should_parallelize_batch(helper, out.len(), backend);
        if wants_parallel {
            self.ensure_thread_pool();
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::Jit(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += out.len();
                compiled.call_flat_batch(flat_inputs, out).map_err(|error| {
                    RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    }
                })
            }
            Some(RuntimePureCacheEntry::Aot(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += out.len();
                let compiled = compiled.require_i64(helper)?;
                if wants_parallel {
                    self.stats.thread_pool_jobs += self.parallel_jobs(out.len());
                    call_aot_flat_batch_parallel(
                        self.pool.as_ref(),
                        compiled,
                        flat_inputs,
                        arity,
                        out,
                    )
                } else {
                    call_aot_flat_batch(compiled, flat_inputs, arity, out, &mut self.aot_i64_slots)
                }
            }
            Some(RuntimePureCacheEntry::AutoAot { aot, jit, .. }) => {
                self.compile_stats.cache_hits += 1;
                if let Some(compiled) = jit {
                    self.stats.jit_calls += out.len();
                    compiled.call_flat_batch(flat_inputs, out).map_err(|error| {
                        RuntimeEvalError::UnsupportedPure {
                            name: helper.name.clone(),
                            reason: error.to_string(),
                        }
                    })
                } else {
                    self.stats.aot_calls += out.len();
                    let aot = aot.require_i64(helper)?;
                    if wants_parallel {
                        self.stats.thread_pool_jobs += self.parallel_jobs(out.len());
                        call_aot_flat_batch_parallel(
                            self.pool.as_ref(),
                            aot,
                            flat_inputs,
                            arity,
                            out,
                        )
                    } else {
                        call_aot_flat_batch(aot, flat_inputs, arity, out, &mut self.aot_i64_slots)
                    }
                }
            }
            Some(entry) => {
                debug_assert!(entry.is_non_i64_runtime_fallback());
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                if wants_parallel {
                    self.stats.thread_pool_jobs += self.parallel_jobs(out.len());
                    call_vm_flat_batch_parallel(self.pool.as_ref(), helper, flat_inputs, arity, out)
                } else {
                    call_vm_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
                }
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
        }
    }

    pub fn call_i64_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i64],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        validate_flat_batch_shape(helper, flat_inputs.len(), arity, rows)?;
        self.record_flat_batch_stats(flat_inputs, rows, false);
        if rows == 0 {
            return Ok(0);
        }
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, rows);
        }
        let backend = self.batch_backend_kind(helper.id);
        let wants_parallel = self.should_parallelize_batch(helper, rows, backend);
        if wants_parallel {
            self.ensure_thread_pool();
        }
        let shape = FlatBatchSumShape {
            flat_inputs,
            arity,
            rows,
        };
        let policy = FlatBatchSumPolicy {
            pool: self.pool.as_ref(),
            wants_parallel,
            parallel_jobs: self.parallel_jobs(rows),
        };
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::Jit(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += rows;
                call_jit_flat_batch_sum(compiled, helper, flat_inputs, rows)
            }
            Some(RuntimePureCacheEntry::Aot(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += rows;
                let compiled = compiled.require_i64(helper)?;
                call_aot_flat_batch_sum_with_policy(
                    policy,
                    &mut self.stats,
                    compiled,
                    shape,
                    &mut self.aot_i64_slots,
                )
            }
            Some(RuntimePureCacheEntry::AutoAot { aot, jit, .. }) => {
                self.compile_stats.cache_hits += 1;
                if let Some(compiled) = jit {
                    self.stats.jit_calls += rows;
                    call_jit_flat_batch_sum(compiled, helper, flat_inputs, rows)
                } else {
                    self.stats.aot_calls += rows;
                    let aot = aot.require_i64(helper)?;
                    call_aot_flat_batch_sum_with_policy(
                        policy,
                        &mut self.stats,
                        aot,
                        shape,
                        &mut self.aot_i64_slots,
                    )
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
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                call_vm_flat_batch_sum_with_policy(
                    policy,
                    &mut self.stats,
                    helper,
                    shape,
                    &mut self.vm_scratch,
                )
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                call_vm_flat_batch_sum(helper, flat_inputs, arity, rows, &mut self.vm_scratch)
            }
        }
    }

    pub fn call_i64_repeated_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        row: &[i64],
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        if row.len() > RuntimeI64Args::MAX {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: helper.name.clone(),
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
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(row);
        if rows == 0 {
            return Ok(0);
        }
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_flat_batch(helper, rows);
        }
        let rows_i64 = i64::try_from(rows).map_err(|_| RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: "pure repeated batch row count must fit i64".to_owned(),
        })?;
        let value = self.repeated_flat_batch_value(helper, row, rows)?;
        value
            .checked_mul(rows_i64)
            .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: "pure repeated batch sum overflowed i64".to_owned(),
            })
    }

    fn repeated_flat_batch_value(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        row: &[i64],
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        let value = match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::Jit(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += rows;
                compiled
                    .call(row)
                    .map_err(|error| RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: error.to_string(),
                    })?
            }
            Some(RuntimePureCacheEntry::Aot(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += rows;
                let (value, _) =
                    compiled.call_i64_with_inputs_scratch(row, &mut self.aot_i64_slots)?;
                value
            }
            Some(RuntimePureCacheEntry::AutoAot { aot, jit, .. }) => {
                self.compile_stats.cache_hits += 1;
                if let Some(compiled) = jit {
                    self.stats.jit_calls += rows;
                    compiled
                        .call(row)
                        .map_err(|error| RuntimeEvalError::UnsupportedPure {
                            name: helper.name.clone(),
                            reason: error.to_string(),
                        })?
                } else {
                    self.stats.aot_calls += rows;
                    let (value, _) =
                        aot.call_i64_with_inputs_scratch(row, &mut self.aot_i64_slots)?;
                    value
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
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                exact_i64_result(self.vm_scratch.evaluate_i64_slice(
                    helper.plan(),
                    helper.id(),
                    row,
                )?)?
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                exact_i64_result(self.vm_scratch.evaluate_i64_slice(
                    helper.plan(),
                    helper.id(),
                    row,
                )?)?
            }
        };
        Ok(value)
    }

    pub(super) fn batch_backend_kind(&self, id: RuntimePureHelperId) -> RuntimeBatchBackendKind {
        match cache_entry(&self.cache, id) {
            Some(
                RuntimePureCacheEntry::Jit(_)
                | RuntimePureCacheEntry::JitI8(_)
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
                | RuntimePureCacheEntry::JitF64(_),
            ) => RuntimeBatchBackendKind::Jit,
            Some(RuntimePureCacheEntry::Aot(_)) => RuntimeBatchBackendKind::Aot,
            Some(RuntimePureCacheEntry::AutoAot { jit, .. }) => {
                if jit.is_some() {
                    RuntimeBatchBackendKind::Jit
                } else {
                    RuntimeBatchBackendKind::Aot
                }
            }
            Some(RuntimePureCacheEntry::Vm) => RuntimeBatchBackendKind::Vm,
            None => RuntimeBatchBackendKind::Missing,
        }
    }

    pub(super) fn promote_auto_jit_for_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        rows: usize,
    ) {
        if !native_jit_enabled() {
            return;
        }
        let work_units = rows.saturating_mul(self.helper_work_units(helper));
        if work_units <= auto_jit_flat_batch_threshold(helper, rows) {
            self.compile_stats.auto_jit_skipped_small += 1;
            return;
        }
        if !self.has_promotable_auto_slot(helper.id) {
            return;
        }

        let Some(kind) = helper_native_kind(helper) else {
            self.mark_auto_jit_failed(helper.id);
            return;
        };
        self.promote_auto_native_jit(helper, kind);
    }

    pub(super) fn promote_auto_jit_for_scalar_call(&mut self, helper: RuntimePureHelperRef<'_>) {
        if !native_jit_enabled() || !self.has_promotable_auto_slot(helper.id) {
            return;
        }
        let work_units = self.helper_work_units(helper);
        let Some(accumulated) = self.auto_scalar_work_units.get_mut(helper.id.0) else {
            return;
        };
        *accumulated = accumulated.saturating_add(work_units);
        if *accumulated < AUTO_JIT_SCALAR_WORK_UNITS {
            return;
        }
        let Some(kind) = helper_native_kind(helper) else {
            self.mark_auto_jit_failed(helper.id);
            return;
        };
        self.promote_auto_native_jit(helper, kind);
    }

    pub(super) fn has_promotable_auto_slot(&self, id: RuntimePureHelperId) -> bool {
        matches!(
            self.cache.get(id.0).and_then(Option::as_ref),
            Some(RuntimePureCacheEntry::AutoAot {
                jit: None,
                jit_failed: false,
                ..
            })
        )
    }

    pub(super) fn promote_auto_native_jit(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        kind: RuntimePureNativeKind,
    ) {
        let request = compile_request(helper, || kind.zero_value());
        match compile_native_jit(kind, &request, helper, &mut self.compile_stats) {
            Some(RuntimePureCacheEntry::Jit(compiled)) => {
                self.install_promoted_i64_jit(helper.id, compiled);
            }
            Some(entry) => {
                self.install_promoted_jit_entry(helper.id, entry);
            }
            None => self.mark_auto_jit_failed(helper.id),
        }
    }

    pub(super) fn install_promoted_i64_jit(
        &mut self,
        id: RuntimePureHelperId,
        compiled: Box<CompiledPureI64Inputs>,
    ) {
        let Some(RuntimePureCacheEntry::AutoAot {
            jit, jit_failed, ..
        }) = self.cache.get_mut(id.0).and_then(Option::as_mut)
        else {
            return;
        };
        *jit = Some(compiled);
        *jit_failed = false;
        self.compile_stats.auto_jit_promotions += 1;
    }

    pub(super) fn install_promoted_jit_entry(
        &mut self,
        id: RuntimePureHelperId,
        entry: RuntimePureCacheEntry,
    ) {
        if let Some(slot) = self.cache.get_mut(id.0) {
            *slot = Some(entry);
        }
        self.compile_stats.auto_jit_promotions += 1;
    }

    pub(super) fn mark_auto_jit_failed(&mut self, id: RuntimePureHelperId) {
        if let Some(Some(RuntimePureCacheEntry::AutoAot { jit_failed, .. })) =
            self.cache.get_mut(id.0)
        {
            *jit_failed = true;
        }
    }

    pub(super) fn should_parallelize_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        rows: usize,
        backend: RuntimeBatchBackendKind,
    ) -> bool {
        self.stats.parallel_policy_checks += 1;
        let work_units = rows.saturating_mul(self.helper_work_units(helper));
        self.stats.parallel_work_units = self.stats.parallel_work_units.saturating_add(work_units);
        if !matches!(
            backend,
            RuntimeBatchBackendKind::Aot | RuntimeBatchBackendKind::Vm
        ) {
            self.stats.parallel_skipped_backend += 1;
            return false;
        }
        if self.resolved_workers <= 1 {
            self.stats.parallel_skipped_small += 1;
            return false;
        }
        let per_worker_min = self.config.batch_min_len.max(1);
        let base_threshold = per_worker_min.saturating_mul(self.resolved_workers);
        let threshold = match backend {
            RuntimeBatchBackendKind::Aot => base_threshold.saturating_mul(4),
            RuntimeBatchBackendKind::Vm => base_threshold,
            RuntimeBatchBackendKind::Jit | RuntimeBatchBackendKind::Missing => unreachable!(),
        };
        if work_units <= threshold {
            self.stats.parallel_skipped_small += 1;
            return false;
        }
        self.stats.parallel_batches += 1;
        true
    }

    pub(super) fn helper_work_units(&self, helper: RuntimePureHelperRef<'_>) -> usize {
        self.helper_work_units
            .get(helper.id.0)
            .copied()
            .filter(|weight| *weight > 0)
            .unwrap_or_else(|| runtime_expr_work_units(&helper.expr))
    }

    pub(super) fn ensure_thread_pool(&mut self) {
        if self.pool.is_none() {
            let started = std::time::Instant::now();
            self.pool = build_thread_pool(self.resolved_workers);
            self.stats.thread_pool_build_elapsed_ns += started.elapsed().as_nanos();
        }
    }

    pub(super) fn parallel_jobs(&self, len: usize) -> usize {
        self.resolved_workers.min(len)
    }

    pub(super) fn record_flat_batch_stats<T>(
        &mut self,
        flat_inputs: &[T],
        rows: usize,
        copies_result: bool,
    ) {
        self.stats.batch_calls += 1;
        self.stats.batch_items += rows;
        self.stats.flat_batch_calls += 1;
        self.stats.flat_batch_items += rows;
        self.stats.flat_batch_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        self.stats.pure_calls += rows;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(flat_inputs);
        if copies_result {
            self.stats.result_bytes_copied += rows * std::mem::size_of::<T>();
        }
    }

    pub(super) fn call_i32_slice_with_accounting(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[i32],
        count_borrowed_args: bool,
    ) -> Result<Option<i32>, RuntimeEvalError> {
        validate_exact_int_slice_shape::<i32>(helper, args.len())?;
        self.stats.pure_calls += 1;
        if count_borrowed_args {
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        }
        if self.config.backend == RuntimePureBackendMode::Auto {
            self.promote_auto_jit_for_scalar_call(helper);
        }
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::JitI32(compiled)) => {
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
                Self::call_vm_i32_slice(helper, args, &mut self.vm_scratch).map(Some)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                Self::call_vm_i32_slice(helper, args, &mut self.vm_scratch).map(Some)
            }
        }
    }
}
