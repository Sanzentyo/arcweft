//! Runtime pure helper acceleration adapters.
//!
//! This crate owns native acceleration state so `arcweft-core` can stay Sans I/O
//! and dependency-light.

pub mod math;

use arcweft_core::{
    math::{DenseMatrixF32, DenseTensorF32},
    plan::{
        RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin, RuntimePureInputType,
        RuntimePureOutputType,
    },
    pure::{
        AotPureFunctionBackend, AotPureI64Plan, AotPureScalarPlan, PureFunctionRequest,
        PureFunctionStats, RuntimeFixedArgs, RuntimeFloat32Args, RuntimeFloat64Args,
        RuntimeI32Args, RuntimeI64Args, RuntimePureCallBackend, RuntimePureScalar,
        RuntimePureScalarInteger, VmPureFunctionScratch,
    },
    step::RuntimePureCallStats,
    value::{
        DenseSeq, RuntimeBinding, RuntimeEvalError, RuntimeExpr, RuntimeIntrinsic, RuntimeSeq,
        RuntimeValue,
    },
};
use arcweft_lang_jit_cranelift::{
    CompiledPureF32Inputs, CompiledPureF64Inputs, CompiledPureI32Inputs, CompiledPureI64Inputs,
    CraneliftPureFunctionBackend,
};
use rayon::{ThreadPool, ThreadPoolBuilder, prelude::*};
use std::fmt;

/// Runtime pure backend selection used by CLI/player adapters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimePureBackendMode {
    Vm,
    Aot,
    Jit,
    #[default]
    Auto,
}

/// Runtime pure batch worker-count policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimePureWorkerCount {
    #[default]
    Auto,
    Fixed(usize),
}

/// Adapter-owned pure helper acceleration settings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePureAcceleratorConfig {
    pub backend: RuntimePureBackendMode,
    pub workers: RuntimePureWorkerCount,
    /// Minimum rows per resolved worker before an AOT/VM batch uses the pool.
    pub batch_min_len: usize,
    pub math: math::RuntimeMathAcceleratorConfig,
}

/// Compile-cache and runtime cache counters for pure acceleration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimePureCompileStats {
    pub jit_attempts: usize,
    pub jit_successes: usize,
    pub jit_failures: usize,
    pub aot_attempts: usize,
    pub aot_successes: usize,
    pub aot_failures: usize,
    pub auto_jit_selected: usize,
    pub auto_aot_selected: usize,
    pub auto_vm_selected: usize,
    pub auto_jit_deferred: usize,
    pub auto_jit_promotions: usize,
    pub auto_jit_skipped_small: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub compile_elapsed_ns: u128,
}

/// Compile-cache backed runtime pure helper accelerator.
pub struct RuntimePureAccelerator {
    config: RuntimePureAcceleratorConfig,
    cache: Vec<Option<RuntimePureCacheEntry>>,
    stats: RuntimePureCallStats,
    compile_stats: RuntimePureCompileStats,
    helper_summary: RuntimePureAccelerationSummary,
    helper_work_units: Vec<usize>,
    pool: Option<ThreadPool>,
    resolved_workers: usize,
    flat_i64_inputs: Vec<i64>,
    aot_i64_slots: Vec<i64>,
    aot_scalar_slots: Vec<RuntimePureScalar>,
    vm_scratch: VmPureFunctionScratch,
    math: math::RuntimeMathAccelerator,
    math_prepare_cache: RuntimeMathPrepareCache,
}

enum RuntimePureCacheEntry {
    Jit(Box<CompiledPureI64Inputs>),
    JitI32(Box<CompiledPureI32Inputs>),
    JitF32(Box<CompiledPureF32Inputs>),
    JitF64(Box<CompiledPureF64Inputs>),
    Aot(RuntimePureAotPlan),
    AutoAot {
        aot: RuntimePureAotPlan,
        jit: Option<Box<CompiledPureI64Inputs>>,
        jit_failed: bool,
    },
    Vm,
}

enum RuntimePureAotPlan {
    I64(AotPureI64Plan),
    Scalar(AotPureScalarPlan),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeBatchBackendKind {
    Jit,
    Aot,
    Vm,
    Missing,
}

#[derive(Clone, Copy)]
struct FlatBatchSumShape<'a> {
    flat_inputs: &'a [i64],
    arity: usize,
    rows: usize,
}

#[derive(Clone, Copy)]
struct FlatBatchSumPolicy<'a> {
    pool: Option<&'a ThreadPool>,
    wants_parallel: bool,
    parallel_jobs: usize,
}

#[derive(Default)]
struct RuntimeMathPrepareCache {
    matmul: Option<PreparedMatrixMatmulCache>,
    matrix_add: Option<PreparedMatrixAddCache>,
    tensor_add: Option<PreparedTensorAddCache>,
}

struct PreparedMatrixMatmulCache {
    signature: MatrixBinarySignature,
    prepared: math::RuntimePreparedMatrixMatmulF32,
}

struct PreparedMatrixAddCache {
    signature: MatrixBinarySignature,
    prepared: math::RuntimePreparedMatrixAddF32,
}

struct PreparedTensorAddCache {
    signature: TensorBinarySignature,
    prepared: math::RuntimePreparedTensorAddF32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MatrixBinarySignature {
    lhs: MatrixSignature,
    rhs: MatrixSignature,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MatrixSignature {
    rows: usize,
    cols: usize,
    value_bits: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TensorBinarySignature {
    lhs: TensorSignature,
    rhs: TensorSignature,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TensorSignature {
    dims: Vec<usize>,
    value_bits: Vec<u32>,
}

impl MatrixBinarySignature {
    fn new(lhs: &DenseMatrixF32, rhs: &DenseMatrixF32) -> Self {
        Self {
            lhs: MatrixSignature::new(lhs),
            rhs: MatrixSignature::new(rhs),
        }
    }
}

impl MatrixSignature {
    fn new(matrix: &DenseMatrixF32) -> Self {
        Self {
            rows: matrix.rows(),
            cols: matrix.cols(),
            value_bits: f32_value_bits(matrix.values()),
        }
    }
}

impl TensorBinarySignature {
    fn new(lhs: &DenseTensorF32, rhs: &DenseTensorF32) -> Self {
        Self {
            lhs: TensorSignature::new(lhs),
            rhs: TensorSignature::new(rhs),
        }
    }
}

impl TensorSignature {
    fn new(tensor: &DenseTensorF32) -> Self {
        Self {
            dims: tensor.shape().dims().to_vec(),
            value_bits: f32_value_bits(tensor.values()),
        }
    }
}

fn f32_value_bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|value| value.to_bits()).collect()
}

const AUTO_EAGER_JIT_WORK_UNITS: usize = 64;
const AUTO_JIT_FLAT_BATCH_WORK_UNITS: usize = 512;

impl RuntimePureAotPlan {
    fn i64_plan(&self) -> Option<&AotPureI64Plan> {
        match self {
            Self::I64(plan) => Some(plan),
            Self::Scalar(_) => None,
        }
    }

    fn call_i64_with_inputs_scratch(
        &self,
        inputs: &[i64],
        slots: &mut Vec<i64>,
    ) -> Result<(i64, PureFunctionStats), RuntimeEvalError> {
        match self {
            Self::I64(plan) => plan.call_with_inputs_scratch(inputs, slots),
            Self::Scalar(plan) => Err(RuntimeEvalError::UnsupportedPure {
                name: plan.name().to_owned(),
                reason: "AOT scalar plan is not an i64 plan".to_owned(),
            }),
        }
    }

    fn call_exact_int_with_inputs_scratch<T: RuntimePureScalarInteger>(
        &self,
        inputs: &[T],
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(T, PureFunctionStats), RuntimeEvalError> {
        match self {
            Self::Scalar(plan) => plan.call_exact_int_with_inputs_scratch(inputs, slots),
            Self::I64(plan) => Err(RuntimeEvalError::UnsupportedPure {
                name: plan.name().to_owned(),
                reason: "AOT i64 plan is not an exact scalar plan".to_owned(),
            }),
        }
    }

    fn call_f32_with_inputs_scratch(
        &self,
        inputs: &[f32],
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(f32, PureFunctionStats), RuntimeEvalError> {
        match self {
            Self::Scalar(plan) => plan.call_f32_with_inputs_scratch(inputs, slots),
            Self::I64(plan) => Err(RuntimeEvalError::UnsupportedPure {
                name: plan.name().to_owned(),
                reason: "AOT i64 plan is not an f32 scalar plan".to_owned(),
            }),
        }
    }

    fn call_usize_with_inputs_scratch(
        &self,
        inputs: &[u64],
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(u64, PureFunctionStats), RuntimeEvalError> {
        match self {
            Self::Scalar(plan) => plan.call_usize_with_inputs_scratch(inputs, slots),
            Self::I64(plan) => Err(RuntimeEvalError::UnsupportedPure {
                name: plan.name().to_owned(),
                reason: "AOT i64 plan is not a usize scalar plan".to_owned(),
            }),
        }
    }

    fn call_f64_with_inputs_scratch(
        &self,
        inputs: &[f64],
        slots: &mut Vec<RuntimePureScalar>,
    ) -> Result<(f64, PureFunctionStats), RuntimeEvalError> {
        match self {
            Self::Scalar(plan) => plan.call_f64_with_inputs_scratch(inputs, slots),
            Self::I64(plan) => Err(RuntimeEvalError::UnsupportedPure {
                name: plan.name().to_owned(),
                reason: "AOT i64 plan is not an f64 scalar plan".to_owned(),
            }),
        }
    }

    fn require_i64(&self, helper: &RuntimePureHelper) -> Result<&AotPureI64Plan, RuntimeEvalError> {
        self.i64_plan()
            .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: "AOT scalar plan cannot serve an i64 batch call".to_owned(),
            })
    }
}

impl fmt::Debug for RuntimePureAccelerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimePureAccelerator")
            .field("config", &self.config)
            .field("cache_entries", &self.cache_entries())
            .field("stats", &self.stats)
            .field("compile_stats", &self.compile_stats)
            .field("helper_summary", &self.helper_summary)
            .field("has_pool", &self.pool.is_some())
            .field("resolved_workers", &self.resolved_workers)
            .field("math_stats", &self.math.stats())
            .finish_non_exhaustive()
    }
}

impl RuntimePureAccelerator {
    pub fn new(mode: RuntimePureBackendMode, helpers: &[RuntimePureHelper]) -> Self {
        Self::with_config(
            RuntimePureAcceleratorConfig {
                backend: mode,
                ..RuntimePureAcceleratorConfig::default()
            },
            helpers,
        )
    }

    pub fn with_config(
        config: RuntimePureAcceleratorConfig,
        helpers: &[RuntimePureHelper],
    ) -> Self {
        let started = std::time::Instant::now();
        let mut compile_stats = RuntimePureCompileStats::default();
        let helper_summary = helper_summary_from_helpers(helpers);
        let helper_work_units = helper_work_unit_slots(helpers);
        let resolved_workers = resolve_worker_count(config.workers);
        let mut cache = helper_cache_slots(helpers);
        for helper in helpers {
            let work_units = helper_work_units
                .get(helper.id.0)
                .copied()
                .unwrap_or_else(|| runtime_expr_work_units(&helper.expr));
            cache[helper.id.0] = Some(compile_helper(
                config.backend,
                helper,
                work_units,
                &mut compile_stats,
            ));
        }
        compile_stats.compile_elapsed_ns = started.elapsed().as_nanos();
        Self {
            config,
            cache,
            stats: RuntimePureCallStats::default(),
            compile_stats,
            helper_summary,
            helper_work_units,
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
        helper: &RuntimePureHelper,
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
            Some(
                RuntimePureCacheEntry::JitI32(_)
                | RuntimePureCacheEntry::JitF32(_)
                | RuntimePureCacheEntry::JitF64(_)
                | RuntimePureCacheEntry::Vm,
            ) => {
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
        helper: &RuntimePureHelper,
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
            Some(
                RuntimePureCacheEntry::JitI32(_)
                | RuntimePureCacheEntry::JitF32(_)
                | RuntimePureCacheEntry::JitF64(_)
                | RuntimePureCacheEntry::Vm,
            ) => {
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
        helper: &RuntimePureHelper,
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
                RuntimePureCacheEntry::JitI32(_)
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
        helper: &RuntimePureHelper,
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
        let rows_i64 = i64::try_from(rows).map_err(|_| RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: "pure repeated batch row count must fit i64".to_owned(),
        })?;
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
                RuntimePureCacheEntry::JitI32(_)
                | RuntimePureCacheEntry::JitF32(_)
                | RuntimePureCacheEntry::JitF64(_)
                | RuntimePureCacheEntry::Vm,
            ) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                exact_i64_result(self.vm_scratch.evaluate_i64_slice(helper, row)?)?
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                exact_i64_result(self.vm_scratch.evaluate_i64_slice(helper, row)?)?
            }
        };
        value
            .checked_mul(rows_i64)
            .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: "pure repeated batch sum overflowed i64".to_owned(),
            })
    }

    fn batch_backend_kind(&self, id: RuntimePureHelperId) -> RuntimeBatchBackendKind {
        match cache_entry(&self.cache, id) {
            Some(
                RuntimePureCacheEntry::Jit(_)
                | RuntimePureCacheEntry::JitI32(_)
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

    fn promote_auto_jit_for_flat_batch(&mut self, helper: &RuntimePureHelper, rows: usize) {
        let work_units = rows.saturating_mul(self.helper_work_units(helper));
        if work_units <= auto_jit_flat_batch_threshold(helper, rows) {
            self.compile_stats.auto_jit_skipped_small += 1;
            return;
        }
        if !self.has_promotable_auto_slot(helper.id) {
            return;
        }

        if helper_has_only_i32_inputs(helper) {
            self.promote_auto_i32_jit(helper);
            return;
        }

        if helper_has_only_f32_inputs(helper) {
            self.promote_auto_f32_jit(helper);
            return;
        }

        if helper_has_only_f64_inputs(helper) {
            self.promote_auto_f64_jit(helper);
            return;
        }

        self.promote_auto_i64_jit(helper);
    }

    fn has_promotable_auto_slot(&self, id: RuntimePureHelperId) -> bool {
        matches!(
            self.cache.get(id.0).and_then(Option::as_ref),
            Some(RuntimePureCacheEntry::AutoAot {
                jit: None,
                jit_failed: false,
                ..
            })
        )
    }

    fn promote_auto_i32_jit(&mut self, helper: &RuntimePureHelper) {
        let request = compile_request(helper, || RuntimeValue::i32(0));
        match compile_jit_i32(&request, helper, &mut self.compile_stats) {
            Some(entry @ RuntimePureCacheEntry::JitI32(_)) => {
                self.install_promoted_jit_entry(helper.id, entry);
            }
            Some(_) => unreachable!("compile_jit_i32 only returns i32 JIT entries"),
            None => self.mark_auto_jit_failed(helper.id),
        }
    }

    fn promote_auto_f32_jit(&mut self, helper: &RuntimePureHelper) {
        let request = compile_request(helper, || RuntimeValue::F32(0.0));
        match compile_jit_f32(&request, helper, &mut self.compile_stats) {
            Some(entry @ RuntimePureCacheEntry::JitF32(_)) => {
                self.install_promoted_jit_entry(helper.id, entry);
            }
            Some(_) => unreachable!("compile_jit_f32 only returns f32 JIT entries"),
            None => self.mark_auto_jit_failed(helper.id),
        }
    }

    fn promote_auto_f64_jit(&mut self, helper: &RuntimePureHelper) {
        let request = compile_request(helper, || RuntimeValue::F64(0.0));
        match compile_jit_f64(&request, helper, &mut self.compile_stats) {
            Some(entry @ RuntimePureCacheEntry::JitF64(_)) => {
                self.install_promoted_jit_entry(helper.id, entry);
            }
            Some(_) => unreachable!("compile_jit_f64 only returns f64 JIT entries"),
            None => self.mark_auto_jit_failed(helper.id),
        }
    }

    fn promote_auto_i64_jit(&mut self, helper: &RuntimePureHelper) {
        let Some(RuntimePureCacheEntry::AutoAot {
            jit, jit_failed, ..
        }) = self.cache.get_mut(helper.id.0).and_then(Option::as_mut)
        else {
            return;
        };
        if !helper_has_only_i64_inputs(helper) {
            *jit_failed = true;
            return;
        }
        let request = compile_request(helper, || RuntimeValue::i64(0));
        match compile_jit(&request, helper, &mut self.compile_stats) {
            Some(RuntimePureCacheEntry::Jit(compiled)) => {
                *jit = Some(compiled);
                self.compile_stats.auto_jit_promotions += 1;
            }
            Some(_) => unreachable!("compile_jit only returns JIT entries"),
            None => *jit_failed = true,
        }
    }

    fn install_promoted_jit_entry(
        &mut self,
        id: RuntimePureHelperId,
        entry: RuntimePureCacheEntry,
    ) {
        if let Some(slot) = self.cache.get_mut(id.0) {
            *slot = Some(entry);
        }
        self.compile_stats.auto_jit_promotions += 1;
    }

    fn mark_auto_jit_failed(&mut self, id: RuntimePureHelperId) {
        if let Some(Some(RuntimePureCacheEntry::AutoAot { jit_failed, .. })) =
            self.cache.get_mut(id.0)
        {
            *jit_failed = true;
        }
    }

    fn should_parallelize_batch(
        &mut self,
        helper: &RuntimePureHelper,
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

    fn helper_work_units(&self, helper: &RuntimePureHelper) -> usize {
        self.helper_work_units
            .get(helper.id.0)
            .copied()
            .filter(|weight| *weight > 0)
            .unwrap_or_else(|| runtime_expr_work_units(&helper.expr))
    }

    fn ensure_thread_pool(&mut self) {
        if self.pool.is_none() {
            let started = std::time::Instant::now();
            self.pool = build_thread_pool(self.resolved_workers);
            self.stats.thread_pool_build_elapsed_ns += started.elapsed().as_nanos();
        }
    }

    fn parallel_jobs(&self, len: usize) -> usize {
        self.resolved_workers.min(len)
    }

    fn record_flat_batch_stats<T>(&mut self, flat_inputs: &[T], rows: usize, copies_result: bool) {
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

    fn call_i32_slice_with_accounting(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[i32],
        count_borrowed_args: bool,
    ) -> Result<Option<i32>, RuntimeEvalError> {
        validate_exact_int_slice_shape::<i32>(helper, args.len())?;
        self.stats.pure_calls += 1;
        if count_borrowed_args {
            self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
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

impl RuntimePureCallBackend for RuntimePureAccelerator {
    fn call_i32(
        &mut self,
        helper: &RuntimePureHelper,
        args: RuntimeI32Args,
    ) -> Result<Option<i32>, RuntimeEvalError> {
        self.stats.arg_stack_packs += 1;
        self.stats.arg_bytes_copied += args.len() * std::mem::size_of::<i32>();
        self.call_i32_slice_with_accounting(helper, args.as_slice(), false)
    }

    fn call_i32_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[i32],
    ) -> Result<Option<i32>, RuntimeEvalError> {
        self.call_i32_slice_with_accounting(helper, args, true)
    }

    fn call_i32_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
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
        helper: &RuntimePureHelper,
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

    fn call_exact_int_flat_batch_sum<T: RuntimePureScalarInteger>(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[T],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<T>(helper, flat_inputs.len(), arity, rows)?;
        self.record_flat_batch_stats(flat_inputs, rows, false);
        match cache_entry(&self.cache, helper.id) {
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
        helper: &RuntimePureHelper,
        args: &[T],
    ) -> Result<Option<T>, RuntimeEvalError> {
        validate_exact_int_slice_shape::<T>(helper, args.len())?;
        self.stats.pure_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        match cache_entry(&self.cache, helper.id) {
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
                let value = self
                    .vm_scratch
                    .evaluate_exact_int_slice::<T>(helper, args)?;
                T::try_from_runtime_value(&helper.name, value).map(Some)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                let value = self
                    .vm_scratch
                    .evaluate_exact_int_slice::<T>(helper, args)?;
                T::try_from_runtime_value(&helper.name, value).map(Some)
            }
        }
    }

    fn call_exact_int_flat_batch<T: RuntimePureScalarInteger>(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[T],
        arity: usize,
        out: &mut [T],
    ) -> Result<(), RuntimeEvalError> {
        validate_exact_int_flat_batch_shape::<T>(helper, flat_inputs.len(), arity, out.len())?;
        self.record_flat_batch_stats(flat_inputs, out.len(), true);
        match cache_entry(&self.cache, helper.id) {
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

    fn call_usize_slice(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[u64],
    ) -> Result<Option<u64>, RuntimeEvalError> {
        validate_usize_slice_shape(helper, args.len())?;
        self.stats.pure_calls += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        match cache_entry(&self.cache, helper.id) {
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
                    .call_usize_with_inputs_scratch(args, &mut self.aot_scalar_slots)
                    .map(|(value, _)| Some(value))
            }
            Some(_) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                vm_usize_result(helper, self.vm_scratch.evaluate_usize_slice(helper, args)?)
                    .map(Some)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                vm_usize_result(helper, self.vm_scratch.evaluate_usize_slice(helper, args)?)
                    .map(Some)
            }
        }
    }

    fn call_usize_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u64],
        arity: usize,
        out: &mut [u64],
    ) -> Result<(), RuntimeEvalError> {
        validate_usize_flat_batch_shape(helper, flat_inputs.len(), arity, out.len())?;
        self.record_flat_batch_stats(flat_inputs, out.len(), true);
        match cache_entry(&self.cache, helper.id) {
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
                call_aot_usize_flat_batch(
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
                call_vm_usize_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += out.len();
                self.stats.fallbacks += out.len();
                call_vm_usize_flat_batch(helper, flat_inputs, arity, out, &mut self.vm_scratch)
            }
        }
    }

    fn call_usize_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[u64],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        validate_usize_flat_batch_shape(helper, flat_inputs.len(), arity, rows)?;
        self.record_flat_batch_stats(flat_inputs, rows, false);
        match cache_entry(&self.cache, helper.id) {
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
                call_aot_usize_flat_batch_sum(
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
                call_vm_usize_flat_batch_sum(helper, flat_inputs, arity, rows, &mut self.vm_scratch)
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += rows;
                self.stats.fallbacks += rows;
                call_vm_usize_flat_batch_sum(helper, flat_inputs, arity, rows, &mut self.vm_scratch)
            }
        }
    }

    fn call_i64(
        &mut self,
        helper: &RuntimePureHelper,
        args: RuntimeI64Args,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        self.stats.pure_calls += 1;
        self.stats.arg_stack_packs += 1;
        self.stats.arg_bytes_copied += args.len() * std::mem::size_of::<i64>();
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
                RuntimePureCacheEntry::JitI32(_)
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
        helper: &RuntimePureHelper,
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
                RuntimePureCacheEntry::JitI32(_)
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
        helper: &RuntimePureHelper,
        rows: &[RuntimeI64Args],
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError> {
        Self::call_i64_batch(self, helper, rows, out)
    }

    fn call_i64_flat_batch(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i64],
        arity: usize,
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError> {
        Self::call_i64_flat_batch(self, helper, flat_inputs, arity, out)
    }

    fn call_i64_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        flat_inputs: &[i64],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        Self::call_i64_flat_batch_sum(self, helper, flat_inputs, arity, rows)
    }

    fn call_i64_repeated_flat_batch_sum(
        &mut self,
        helper: &RuntimePureHelper,
        row: &[i64],
        rows: usize,
    ) -> Result<i64, RuntimeEvalError> {
        Self::call_i64_repeated_flat_batch_sum(self, helper, row, rows)
    }

    fn call_f32_slice(
        &mut self,
        helper: &RuntimePureHelper,
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
        helper: &RuntimePureHelper,
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
        helper: &RuntimePureHelper,
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
        helper: &RuntimePureHelper,
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

    fn call_math_matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs(lhs.values().len(), rhs.values().len());
        let result = self
            .call_runtime_math_matmul_f32(lhs, rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathMatmulF32.as_label().to_owned(),
                reason: error.to_string(),
            })?;
        self.record_math_result(result.values().len());
        Ok(result)
    }

    fn call_math_matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs(lhs.values().len(), rhs.values().len());
        let result = self
            .call_runtime_math_matrix_add_f32(lhs, rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathMatrixAddF32.as_label().to_owned(),
                reason: error.to_string(),
            })?;
        self.record_math_result(result.values().len());
        Ok(result)
    }

    fn call_math_tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeEvalError> {
        self.stats.math_calls += 1;
        self.record_math_inputs(lhs.values().len(), rhs.values().len());
        let result = self
            .call_runtime_math_tensor_add_f32(lhs, rhs)
            .map_err(|error| RuntimeEvalError::UnsupportedPure {
                name: RuntimeIntrinsic::MathTensorAddF32.as_label().to_owned(),
                reason: error.to_string(),
            })?;
        self.record_math_result(result.values().len());
        Ok(result)
    }

    fn call_values(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.fallbacks += 1;
        self.stats.arg_bytes_borrowed += std::mem::size_of_val(args);
        self.vm_scratch.evaluate_values(helper, args)
    }

    fn stats(&self) -> RuntimePureCallStats {
        self.stats
    }
}

impl RuntimePureAccelerator {
    fn cache_entries(&self) -> usize {
        self.cache.iter().filter(|entry| entry.is_some()).count()
    }

    fn call_runtime_math_matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, math::RuntimeMathAcceleratorError> {
        let selection = self.math.matmul_backend_selection(lhs, rhs);
        if selection.backend() != math::RuntimeMathBackend::Wgpu {
            return self.math.matmul_f32(lhs, rhs);
        }
        self.math.record_backend_selection(selection);
        let signature = MatrixBinarySignature::new(lhs, rhs);
        if let Some(cache) = self.math_prepare_cache.matmul.take()
            && cache.signature == signature
        {
            let result = self.math.run_prepared_matrix_matmul_f32(&cache.prepared);
            self.math_prepare_cache.matmul = Some(cache);
            return result;
        }
        let prepared = self.math.prepare_matrix_matmul_f32(lhs, rhs)?;
        let result = self.math.run_prepared_matrix_matmul_f32(&prepared);
        self.math_prepare_cache.matmul = Some(PreparedMatrixMatmulCache {
            signature,
            prepared,
        });
        result
    }

    fn call_runtime_math_matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, math::RuntimeMathAcceleratorError> {
        if self.math.config().backend != math::RuntimeMathBackend::Wgpu {
            return self.math.matrix_add_f32(lhs, rhs);
        }
        let signature = MatrixBinarySignature::new(lhs, rhs);
        if let Some(cache) = self.math_prepare_cache.matrix_add.take()
            && cache.signature == signature
        {
            let result = self.math.run_prepared_matrix_add_f32(&cache.prepared);
            self.math_prepare_cache.matrix_add = Some(cache);
            return result;
        }
        let prepared = self.math.prepare_matrix_add_f32(lhs, rhs)?;
        let result = self.math.run_prepared_matrix_add_f32(&prepared);
        self.math_prepare_cache.matrix_add = Some(PreparedMatrixAddCache {
            signature,
            prepared,
        });
        result
    }

    fn call_runtime_math_tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, math::RuntimeMathAcceleratorError> {
        if self.math.config().backend != math::RuntimeMathBackend::Wgpu {
            return self.math.tensor_add_f32(lhs, rhs);
        }
        let signature = TensorBinarySignature::new(lhs, rhs);
        if let Some(cache) = self.math_prepare_cache.tensor_add.take()
            && cache.signature == signature
        {
            let result = self.math.run_prepared_tensor_add_f32(&cache.prepared);
            self.math_prepare_cache.tensor_add = Some(cache);
            return result;
        }
        let prepared = self.math.prepare_tensor_add_f32(lhs, rhs)?;
        let result = self.math.run_prepared_tensor_add_f32(&prepared);
        self.math_prepare_cache.tensor_add = Some(PreparedTensorAddCache {
            signature,
            prepared,
        });
        result
    }

    fn record_math_inputs(&mut self, lhs_elements: usize, rhs_elements: usize) {
        self.stats.arg_bytes_borrowed +=
            lhs_elements.saturating_add(rhs_elements) * std::mem::size_of::<f32>();
    }

    fn record_math_result(&mut self, result_elements: usize) {
        self.stats.result_bytes_copied += result_elements * std::mem::size_of::<f32>();
        if !matches!(
            self.math.stats().last_backend,
            Some(math::RuntimeMathBackend::Scalar) | None
        ) {
            self.stats.math_accelerated_calls += 1;
        }
    }

    fn call_vm_i64(
        helper: &RuntimePureHelper,
        args: RuntimeI64Args,
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<i64, RuntimeEvalError> {
        match scratch.evaluate_i64_args(helper, args)? {
            value @ RuntimeValue::Int(_) => exact_i64_result(value),
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
        }
    }

    fn call_vm_i64_slice(
        helper: &RuntimePureHelper,
        args: &[i64],
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<i64, RuntimeEvalError> {
        match scratch.evaluate_i64_slice(helper, args)? {
            value @ RuntimeValue::Int(_) => exact_i64_result(value),
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
        }
    }

    fn call_vm_i32_slice(
        helper: &RuntimePureHelper,
        args: &[i32],
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<i32, RuntimeEvalError> {
        match scratch.evaluate_i32_slice(helper, args)? {
            RuntimeValue::Int(value) => {
                value
                    .exact_i32()
                    .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                        name: helper.name.clone(),
                        reason: format!("pure i32 result `{value}` is outside i32 range"),
                    })
            }
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
        }
    }

    fn call_vm_f32_slice(
        helper: &RuntimePureHelper,
        args: &[f32],
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<f32, RuntimeEvalError> {
        match scratch.evaluate_f32_slice(helper, args)? {
            RuntimeValue::F32(value) => Ok(value),
            value => Err(RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: format!(
                    "pure f32 result expected f32, got {}",
                    runtime_value_kind(&value)
                ),
            }),
        }
    }

    fn call_vm_f64_slice(
        helper: &RuntimePureHelper,
        args: &[f64],
        scratch: &mut VmPureFunctionScratch,
    ) -> Result<f64, RuntimeEvalError> {
        match scratch.evaluate_f64_slice(helper, args)? {
            RuntimeValue::F64(value) => Ok(value),
            value => Err(RuntimeEvalError::UnsupportedPure {
                name: helper.name.clone(),
                reason: format!(
                    "pure f64 result expected f64, got {}",
                    runtime_value_kind(&value)
                ),
            }),
        }
    }
}

fn runtime_value_kind(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Unit => "()",
        RuntimeValue::Bool(_) => "bool",
        RuntimeValue::Int(_) => "int",
        RuntimeValue::UInt(_) => "uint",
        RuntimeValue::F32(_) => "f32",
        RuntimeValue::F64(_) => "f64",
        RuntimeValue::MatrixF32(_) => "matrix_f32",
        RuntimeValue::TensorF32(_) => "tensor_f32",
        RuntimeValue::String(_) => "string",
        RuntimeValue::Char(_) => "char",
        RuntimeValue::Duration(_) => "duration",
        RuntimeValue::EntityRef(_) => "entity_ref",
        RuntimeValue::Tuple(_) => "tuple",
        RuntimeValue::Seq(RuntimeSeq::Values(_)) => "seq_values",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Units(_))) => "seq_units",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I8(_))) => "seq_i8",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I16(_))) => "seq_i16",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I32(_))) => "seq_i32",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I64(_))) => "seq_i64",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I128(_))) => "seq_i128",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::ISize(_))) => "seq_isize",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::U8(_))) => "seq_u8",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::U16(_))) => "seq_u16",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::U32(_))) => "seq_u32",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::U64(_))) => "seq_u64",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::U128(_))) => "seq_u128",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::USize(_))) => "seq_usize",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::F32(_))) => "seq_f32",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::F64(_))) => "seq_f64",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Bool(_))) => "seq_bool",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Bytes(_))) => "seq_bytes",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Chars(_))) => "seq_chars",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Durations(_))) => "seq_durations",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Strings(_))) => "seq_strings",
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::EntityRefs(_))) => "seq_entity_refs",
        RuntimeValue::Seq(RuntimeSeq::TupleColumns(_)) => "seq_tuple_columns",
        RuntimeValue::Seq(RuntimeSeq::RecordColumns(_)) => "seq_record_columns",
        RuntimeValue::Record(_) => "record",
        RuntimeValue::Variant { .. } => "variant",
    }
    .to_owned()
}

fn compile_helper(
    mode: RuntimePureBackendMode,
    helper: &RuntimePureHelper,
    work_units: usize,
    stats: &mut RuntimePureCompileStats,
) -> RuntimePureCacheEntry {
    if mode == RuntimePureBackendMode::Vm {
        return RuntimePureCacheEntry::Vm;
    }
    if helper_has_only_i64_inputs(helper) {
        let request = compile_request(helper, || RuntimeValue::i64(0));
        return match mode {
            RuntimePureBackendMode::Vm => RuntimePureCacheEntry::Vm,
            RuntimePureBackendMode::Aot => {
                compile_aot_i64(&request, helper, stats).unwrap_or(RuntimePureCacheEntry::Vm)
            }
            RuntimePureBackendMode::Jit => {
                compile_jit(&request, helper, stats).unwrap_or(RuntimePureCacheEntry::Vm)
            }
            RuntimePureBackendMode::Auto => compile_auto(&request, helper, work_units, stats),
        };
    }
    let Some(input_type) = helper_scalar_aot_input_type(helper) else {
        if mode == RuntimePureBackendMode::Auto {
            stats.auto_vm_selected += 1;
        }
        return RuntimePureCacheEntry::Vm;
    };
    let output_type = helper.output_type;
    let zero = scalar_zero_for_input(input_type);
    let request = compile_request(helper, zero);
    match mode {
        RuntimePureBackendMode::Vm => RuntimePureCacheEntry::Vm,
        RuntimePureBackendMode::Aot => {
            compile_aot_scalar(&request, helper, input_type, output_type, stats)
                .unwrap_or(RuntimePureCacheEntry::Vm)
        }
        RuntimePureBackendMode::Jit => {
            if let Some(jit) = compile_jit_i32(&request, helper, stats)
                .or_else(|| compile_jit_f32(&request, helper, stats))
                .or_else(|| compile_jit_f64(&request, helper, stats))
            {
                jit
            } else {
                stats.jit_attempts += 1;
                stats.jit_failures += 1;
                compile_aot_scalar(&request, helper, input_type, output_type, stats)
                    .unwrap_or(RuntimePureCacheEntry::Vm)
            }
        }
        RuntimePureBackendMode::Auto => {
            compile_auto_scalar(&request, helper, input_type, output_type, stats)
        }
    }
}

fn exact_i64_result(value: RuntimeValue) -> Result<i64, RuntimeEvalError> {
    match value {
        RuntimeValue::Int(value) => value
            .exact_i64()
            .ok_or_else(|| RuntimeEvalError::ExpectedInt(value.to_string())),
        value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
    }
}

fn validate_flat_batch_shape(
    helper: &RuntimePureHelper,
    flat_input_len: usize,
    arity: usize,
    rows: usize,
) -> Result<(), RuntimeEvalError> {
    if arity > RuntimeI64Args::MAX {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: helper.name.clone(),
            max: RuntimeI64Args::MAX,
            found: arity,
        });
    }
    let expected = rows.saturating_mul(arity);
    if flat_input_len != expected {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure flat batch expected {expected} input value(s), got {flat_input_len}"
            ),
        });
    }
    Ok(())
}

fn validate_exact_int_flat_batch_shape<T: RuntimePureScalarInteger>(
    helper: &RuntimePureHelper,
    flat_input_len: usize,
    arity: usize,
    rows: usize,
) -> Result<(), RuntimeEvalError> {
    if arity > RuntimeFixedArgs::<T>::MAX {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: helper.name.clone(),
            max: RuntimeFixedArgs::<T>::MAX,
            found: arity,
        });
    }
    if helper.output_type != T::OUTPUT_TYPE
        || helper.input_types.len() != helper.input_names.len()
        || !helper
            .input_types
            .iter()
            .all(|input| *input == T::INPUT_TYPE)
    {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: "exact integer batch type does not match helper signature".to_owned(),
        });
    }
    if arity != helper.input_names.len() {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure batch arity expected {} input value(s), got {arity}",
                helper.input_names.len()
            ),
        });
    }
    if flat_input_len != rows.saturating_mul(arity) {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure flat batch expected {} input value(s), got {}",
                rows.saturating_mul(arity),
                flat_input_len
            ),
        });
    }
    Ok(())
}

fn validate_exact_int_slice_shape<T: RuntimePureScalarInteger>(
    helper: &RuntimePureHelper,
    arg_len: usize,
) -> Result<(), RuntimeEvalError> {
    if arg_len > RuntimeFixedArgs::<T>::MAX {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: helper.name.clone(),
            max: RuntimeFixedArgs::<T>::MAX,
            found: arg_len,
        });
    }
    if helper.output_type != T::OUTPUT_TYPE
        || helper.input_types.len() != helper.input_names.len()
        || !helper
            .input_types
            .iter()
            .all(|input| *input == T::INPUT_TYPE)
    {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: "exact integer slice type does not match helper signature".to_owned(),
        });
    }
    if arg_len != helper.input_names.len() {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure slice expected {} input value(s), got {arg_len}",
                helper.input_names.len()
            ),
        });
    }
    Ok(())
}

fn validate_usize_slice_shape(
    helper: &RuntimePureHelper,
    arg_len: usize,
) -> Result<(), RuntimeEvalError> {
    if arg_len > RuntimeFixedArgs::<u64>::MAX {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: helper.name.clone(),
            max: RuntimeFixedArgs::<u64>::MAX,
            found: arg_len,
        });
    }
    if helper.output_type != RuntimePureOutputType::USize
        || helper.input_types.len() != helper.input_names.len()
        || !helper
            .input_types
            .iter()
            .all(|input| *input == RuntimePureInputType::USize)
    {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: "usize slice type does not match helper signature".to_owned(),
        });
    }
    if arg_len != helper.input_names.len() {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure slice expected {} input value(s), got {arg_len}",
                helper.input_names.len()
            ),
        });
    }
    Ok(())
}

fn validate_usize_flat_batch_shape(
    helper: &RuntimePureHelper,
    flat_input_len: usize,
    arity: usize,
    rows: usize,
) -> Result<(), RuntimeEvalError> {
    validate_usize_slice_shape(helper, arity)?;
    if flat_input_len != rows.saturating_mul(arity) {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
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
    helper: &RuntimePureHelper,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    max_arity: usize,
    flat_input_len: usize,
    arity: usize,
    rows: usize,
) -> Result<(), RuntimeEvalError> {
    if arity > max_arity {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: helper.name.clone(),
            max: max_arity,
            found: arity,
        });
    }
    if helper.output_type != output_type
        || helper.input_types.len() != helper.input_names.len()
        || !helper.input_types.iter().all(|input| *input == input_type)
    {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: "float batch type does not match helper signature".to_owned(),
        });
    }
    if flat_input_len != rows.saturating_mul(arity) {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure flat batch expected {} input value(s), got {}",
                rows.saturating_mul(arity),
                flat_input_len
            ),
        });
    }
    Ok(())
}

fn compile_auto(
    request: &PureFunctionRequest,
    helper: &RuntimePureHelper,
    work_units: usize,
    stats: &mut RuntimePureCompileStats,
) -> RuntimePureCacheEntry {
    if work_units >= AUTO_EAGER_JIT_WORK_UNITS {
        stats.auto_jit_selected += 1;
        return compile_jit(request, helper, stats)
            .or_else(|| compile_aot_i64(request, helper, stats))
            .unwrap_or_else(|| {
                stats.auto_vm_selected += 1;
                RuntimePureCacheEntry::Vm
            });
    }
    match compile_aot_i64(request, helper, stats) {
        Some(RuntimePureCacheEntry::Aot(aot)) => {
            stats.auto_aot_selected += 1;
            stats.auto_jit_deferred += 1;
            RuntimePureCacheEntry::AutoAot {
                aot,
                jit: None,
                jit_failed: false,
            }
        }
        Some(_) => unreachable!("compile_aot only returns AOT entries"),
        None => {
            stats.auto_vm_selected += 1;
            RuntimePureCacheEntry::Vm
        }
    }
}

fn helper_has_only_i64_inputs(helper: &RuntimePureHelper) -> bool {
    helper.output_type == RuntimePureOutputType::I64
        && helper.input_names.len() == helper.input_types.len()
        && helper
            .input_types
            .iter()
            .all(|ty| matches!(ty, RuntimePureInputType::I64))
}

fn helper_has_only_i32_inputs(helper: &RuntimePureHelper) -> bool {
    helper.output_type == RuntimePureOutputType::I32
        && helper.input_names.len() == helper.input_types.len()
        && helper
            .input_types
            .iter()
            .all(|ty| matches!(ty, RuntimePureInputType::I32))
}

fn helper_has_only_f32_inputs(helper: &RuntimePureHelper) -> bool {
    helper.output_type == RuntimePureOutputType::F32
        && helper.input_names.len() == helper.input_types.len()
        && helper
            .input_types
            .iter()
            .all(|ty| matches!(ty, RuntimePureInputType::F32))
}

fn helper_has_only_f64_inputs(helper: &RuntimePureHelper) -> bool {
    helper.output_type == RuntimePureOutputType::F64
        && helper.input_names.len() == helper.input_types.len()
        && helper
            .input_types
            .iter()
            .all(|ty| matches!(ty, RuntimePureInputType::F64))
}

fn auto_jit_flat_batch_threshold(helper: &RuntimePureHelper, rows: usize) -> usize {
    if (helper_has_only_i32_inputs(helper)
        || helper_has_only_f32_inputs(helper)
        || helper_has_only_f64_inputs(helper))
        && rows >= 64
    {
        0
    } else {
        AUTO_JIT_FLAT_BATCH_WORK_UNITS.max(1)
    }
}

fn compile_jit(
    request: &PureFunctionRequest,
    helper: &RuntimePureHelper,
    stats: &mut RuntimePureCompileStats,
) -> Option<RuntimePureCacheEntry> {
    if !helper_has_only_i64_inputs(helper) {
        return None;
    }
    stats.jit_attempts += 1;
    let compiled = CraneliftPureFunctionBackend
        .compile_i64_with_inputs(request, helper.input_names.iter().map(String::as_str))
        .ok();
    if compiled.is_some() {
        stats.jit_successes += 1;
    } else {
        stats.jit_failures += 1;
    }
    compiled.map(Box::new).map(RuntimePureCacheEntry::Jit)
}

fn compile_jit_i32(
    request: &PureFunctionRequest,
    helper: &RuntimePureHelper,
    stats: &mut RuntimePureCompileStats,
) -> Option<RuntimePureCacheEntry> {
    if !helper_has_only_i32_inputs(helper) {
        return None;
    }
    stats.jit_attempts += 1;
    let compiled = CraneliftPureFunctionBackend
        .compile_i32_with_inputs(request, helper.input_names.iter().map(String::as_str))
        .ok();
    if compiled.is_some() {
        stats.jit_successes += 1;
    } else {
        stats.jit_failures += 1;
    }
    compiled.map(Box::new).map(RuntimePureCacheEntry::JitI32)
}

fn compile_jit_f32(
    request: &PureFunctionRequest,
    helper: &RuntimePureHelper,
    stats: &mut RuntimePureCompileStats,
) -> Option<RuntimePureCacheEntry> {
    if !helper_has_only_f32_inputs(helper) {
        return None;
    }
    stats.jit_attempts += 1;
    let compiled = CraneliftPureFunctionBackend
        .compile_f32_with_inputs(request, helper.input_names.iter().map(String::as_str))
        .ok();
    if compiled.is_some() {
        stats.jit_successes += 1;
    } else {
        stats.jit_failures += 1;
    }
    compiled.map(Box::new).map(RuntimePureCacheEntry::JitF32)
}

fn compile_jit_f64(
    request: &PureFunctionRequest,
    helper: &RuntimePureHelper,
    stats: &mut RuntimePureCompileStats,
) -> Option<RuntimePureCacheEntry> {
    if !helper_has_only_f64_inputs(helper) {
        return None;
    }
    stats.jit_attempts += 1;
    let compiled = CraneliftPureFunctionBackend
        .compile_f64_with_inputs(request, helper.input_names.iter().map(String::as_str))
        .ok();
    if compiled.is_some() {
        stats.jit_successes += 1;
    } else {
        stats.jit_failures += 1;
    }
    compiled.map(Box::new).map(RuntimePureCacheEntry::JitF64)
}

fn compile_auto_scalar(
    request: &PureFunctionRequest,
    helper: &RuntimePureHelper,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    stats: &mut RuntimePureCompileStats,
) -> RuntimePureCacheEntry {
    match compile_aot_scalar(request, helper, input_type, output_type, stats) {
        Some(RuntimePureCacheEntry::Aot(aot)) => {
            stats.auto_aot_selected += 1;
            if helper_has_only_i32_inputs(helper)
                || helper_has_only_f32_inputs(helper)
                || helper_has_only_f64_inputs(helper)
            {
                stats.auto_jit_deferred += 1;
            }
            RuntimePureCacheEntry::AutoAot {
                aot,
                jit: None,
                jit_failed: !(helper_has_only_i32_inputs(helper)
                    || helper_has_only_f32_inputs(helper)
                    || helper_has_only_f64_inputs(helper)),
            }
        }
        Some(_) => unreachable!("compile_aot_scalar only returns AOT entries"),
        None => {
            stats.auto_vm_selected += 1;
            RuntimePureCacheEntry::Vm
        }
    }
}

fn compile_aot_i64(
    request: &PureFunctionRequest,
    helper: &RuntimePureHelper,
    stats: &mut RuntimePureCompileStats,
) -> Option<RuntimePureCacheEntry> {
    stats.aot_attempts += 1;
    let compiled = helper_has_only_i64_inputs(helper)
        .then(|| {
            AotPureFunctionBackend::new()
                .compile_i64_with_inputs(request, helper.input_names.iter().map(String::as_str))
                .map(RuntimePureAotPlan::I64)
                .ok()
        })
        .flatten();
    if compiled.is_some() {
        stats.aot_successes += 1;
    } else {
        stats.aot_failures += 1;
    }
    compiled.map(RuntimePureCacheEntry::Aot)
}

fn compile_aot_scalar(
    request: &PureFunctionRequest,
    helper: &RuntimePureHelper,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    stats: &mut RuntimePureCompileStats,
) -> Option<RuntimePureCacheEntry> {
    stats.aot_attempts += 1;
    let compiled = AotPureFunctionBackend::new()
        .compile_scalar_with_inputs(
            request,
            helper.input_names.iter().map(String::as_str),
            input_type,
            output_type,
        )
        .map(RuntimePureAotPlan::Scalar)
        .ok();
    if compiled.is_some() {
        stats.aot_successes += 1;
    } else {
        stats.aot_failures += 1;
    }
    compiled.map(RuntimePureCacheEntry::Aot)
}

fn helper_scalar_aot_input_type(helper: &RuntimePureHelper) -> Option<RuntimePureInputType> {
    if helper.input_names.len() != helper.input_types.len()
        || matches!(helper.output_type, RuntimePureOutputType::Value)
    {
        return None;
    }
    let expected = scalar_input_type_for_output(helper.output_type)?;
    helper
        .input_types
        .iter()
        .all(|input| *input == expected)
        .then_some(expected)
}

const fn scalar_input_type_for_output(
    output: RuntimePureOutputType,
) -> Option<RuntimePureInputType> {
    match output {
        RuntimePureOutputType::I8 => Some(RuntimePureInputType::I8),
        RuntimePureOutputType::I16 => Some(RuntimePureInputType::I16),
        RuntimePureOutputType::I32 => Some(RuntimePureInputType::I32),
        RuntimePureOutputType::I64 => Some(RuntimePureInputType::I64),
        RuntimePureOutputType::I128 => Some(RuntimePureInputType::I128),
        RuntimePureOutputType::ISize => Some(RuntimePureInputType::ISize),
        RuntimePureOutputType::U8 => Some(RuntimePureInputType::U8),
        RuntimePureOutputType::U16 => Some(RuntimePureInputType::U16),
        RuntimePureOutputType::U32 => Some(RuntimePureInputType::U32),
        RuntimePureOutputType::U64 => Some(RuntimePureInputType::U64),
        RuntimePureOutputType::U128 => Some(RuntimePureInputType::U128),
        RuntimePureOutputType::USize => Some(RuntimePureInputType::USize),
        RuntimePureOutputType::F32 => Some(RuntimePureInputType::F32),
        RuntimePureOutputType::F64 => Some(RuntimePureInputType::F64),
        RuntimePureOutputType::Bool | RuntimePureOutputType::Value => None,
    }
}

const fn scalar_zero_for_input(input: RuntimePureInputType) -> fn() -> RuntimeValue {
    match input {
        RuntimePureInputType::I8 => zero_i8,
        RuntimePureInputType::I16 => zero_i16,
        RuntimePureInputType::I32 => zero_i32,
        RuntimePureInputType::I64 => zero_i64,
        RuntimePureInputType::I128 => zero_i128,
        RuntimePureInputType::ISize => zero_isize,
        RuntimePureInputType::U8 => zero_u8,
        RuntimePureInputType::U16 => zero_u16,
        RuntimePureInputType::U32 => zero_u32,
        RuntimePureInputType::U64 => zero_u64,
        RuntimePureInputType::U128 => zero_u128,
        RuntimePureInputType::USize => zero_usize,
        RuntimePureInputType::F32 => zero_f32,
        RuntimePureInputType::F64 => zero_f64,
        RuntimePureInputType::Value => zero_unit,
    }
}

fn zero_unit() -> RuntimeValue {
    RuntimeValue::Unit
}

fn zero_i8() -> RuntimeValue {
    RuntimeValue::i8(0)
}

fn zero_i16() -> RuntimeValue {
    RuntimeValue::i16(0)
}

fn zero_i32() -> RuntimeValue {
    RuntimeValue::i32(0)
}

fn zero_i64() -> RuntimeValue {
    RuntimeValue::i64(0)
}

fn zero_i128() -> RuntimeValue {
    RuntimeValue::i128(0)
}

fn zero_isize() -> RuntimeValue {
    RuntimeValue::isize(0)
}

fn zero_u8() -> RuntimeValue {
    RuntimeValue::u8(0)
}

fn zero_u16() -> RuntimeValue {
    RuntimeValue::u16(0)
}

fn zero_u32() -> RuntimeValue {
    RuntimeValue::u32(0)
}

fn zero_u64() -> RuntimeValue {
    RuntimeValue::u64(0)
}

fn zero_u128() -> RuntimeValue {
    RuntimeValue::u128(0)
}

fn zero_usize() -> RuntimeValue {
    RuntimeValue::usize(0)
}

fn zero_f32() -> RuntimeValue {
    RuntimeValue::F32(0.0)
}

fn zero_f64() -> RuntimeValue {
    RuntimeValue::F64(0.0)
}

fn resolve_worker_count(workers: RuntimePureWorkerCount) -> usize {
    match workers {
        RuntimePureWorkerCount::Auto => std::thread::available_parallelism()
            .ok()
            .map_or(1, std::num::NonZeroUsize::get),
        RuntimePureWorkerCount::Fixed(value) => value.max(1),
    }
}

fn build_thread_pool(worker_count: usize) -> Option<ThreadPool> {
    (worker_count > 1)
        .then(|| {
            ThreadPoolBuilder::new()
                .num_threads(worker_count)
                .build()
                .ok()
        })
        .flatten()
}

fn helper_cache_slots(helpers: &[RuntimePureHelper]) -> Vec<Option<RuntimePureCacheEntry>> {
    let slots = helpers
        .iter()
        .map(|helper| helper.id.0)
        .max()
        .map_or(0, |max_id| max_id + 1);
    let mut cache = Vec::with_capacity(slots);
    cache.resize_with(slots, || None);
    cache
}

fn helper_work_unit_slots(helpers: &[RuntimePureHelper]) -> Vec<usize> {
    let slots = helpers
        .iter()
        .map(|helper| helper.id.0)
        .max()
        .map_or(0, |max_id| max_id + 1);
    let mut weights = vec![0; slots];
    for helper in helpers {
        weights[helper.id.0] = runtime_expr_work_units(&helper.expr);
    }
    weights
}

fn runtime_expr_work_units(expr: &RuntimeExpr) -> usize {
    match expr {
        RuntimeExpr::Value(_) | RuntimeExpr::Local(_) | RuntimeExpr::EntityRef(_) => 1,
        RuntimeExpr::Unary { expr, .. } => 1 + runtime_expr_work_units(expr),
        RuntimeExpr::Binary { lhs, rhs, .. } => {
            2 + runtime_expr_work_units(lhs) + runtime_expr_work_units(rhs)
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            3 + runtime_expr_work_units(condition)
                + runtime_expr_work_units(then_expr)
                + runtime_expr_work_units(else_expr)
        }
        RuntimeExpr::Let { expr, body, .. } => {
            2 + runtime_expr_work_units(expr) + runtime_expr_work_units(body)
        }
        RuntimeExpr::Tuple(items) | RuntimeExpr::BracketSeq(items) => {
            1 + items.iter().map(runtime_expr_work_units).sum::<usize>()
        }
        RuntimeExpr::RepeatSeq { value, .. } => 2 + runtime_expr_work_units(value),
        RuntimeExpr::Record(fields) => {
            1 + fields
                .iter()
                .map(|field| runtime_expr_work_units(&field.value))
                .sum::<usize>()
        }
        RuntimeExpr::Variant { payload, .. } => {
            1 + payload.as_deref().map_or(0, runtime_expr_work_units)
        }
        RuntimeExpr::SpreadArg(payload) => 1 + runtime_expr_work_units(payload),
        RuntimeExpr::Field { target, .. }
        | RuntimeExpr::ProjectTuple { target, .. }
        | RuntimeExpr::ProjectRecord { target, .. } => 1 + runtime_expr_work_units(target),
        RuntimeExpr::Call { args, .. } | RuntimeExpr::PureCall { args, .. } => {
            8 + args.iter().map(runtime_expr_work_units).sum::<usize>()
        }
        RuntimeExpr::MethodCall { receiver, args, .. } => {
            8 + runtime_expr_work_units(receiver)
                + args.iter().map(runtime_expr_work_units).sum::<usize>()
        }
        RuntimeExpr::Map { source, body, .. } => {
            8 + runtime_expr_work_units(source) + runtime_expr_work_units(body)
        }
        RuntimeExpr::Sum { source } => 4 + runtime_expr_work_units(source),
        RuntimeExpr::IfLet {
            expr,
            guard,
            then_expr,
            else_expr,
            ..
        } => {
            4 + runtime_expr_work_units(expr)
                + guard.as_deref().map_or(0, runtime_expr_work_units)
                + runtime_expr_work_units(then_expr)
                + runtime_expr_work_units(else_expr)
        }
        RuntimeExpr::Match { scrutinee, arms } => {
            6 + runtime_expr_work_units(scrutinee)
                + arms
                    .iter()
                    .map(|arm| {
                        arm.guard.as_ref().map_or(0, runtime_expr_work_units)
                            + runtime_expr_work_units(&arm.value)
                    })
                    .sum::<usize>()
        }
    }
}

fn cache_entry(
    cache: &[Option<RuntimePureCacheEntry>],
    id: RuntimePureHelperId,
) -> Option<&RuntimePureCacheEntry> {
    cache.get(id.0).and_then(Option::as_ref)
}

fn call_jit_batch(
    compiled: &CompiledPureI64Inputs,
    rows: &[RuntimeI64Args],
    out: &mut [i64],
    helper: &RuntimePureHelper,
    flat_inputs: &mut Vec<i64>,
) -> Result<(), RuntimeEvalError> {
    let arity = compiled.param_names().len();
    flat_inputs.clear();
    flat_inputs.reserve(rows.len().saturating_mul(arity));
    for row in rows {
        flat_inputs.extend_from_slice(row.as_slice());
    }
    compiled
        .call_flat_batch(flat_inputs, out)
        .map_err(|error| RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: error.to_string(),
        })
}

fn call_jit_flat_batch_sum(
    compiled: &CompiledPureI64Inputs,
    helper: &RuntimePureHelper,
    flat_inputs: &[i64],
    rows: usize,
) -> Result<i64, RuntimeEvalError> {
    compiled
        .call_flat_batch_sum(flat_inputs, rows)
        .map_err(|error| RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: error.to_string(),
        })
}

fn call_aot_batch(
    compiled: &AotPureI64Plan,
    rows: &[RuntimeI64Args],
    out: &mut [i64],
    slots: &mut Vec<i64>,
) -> Result<(), RuntimeEvalError> {
    rows.iter().zip(out.iter_mut()).try_for_each(|(row, slot)| {
        compiled
            .call_with_inputs_scratch(row.as_slice(), slots)
            .map(|(value, _)| *slot = value)
    })
}

fn call_aot_batch_parallel(
    pool: Option<&ThreadPool>,
    compiled: &AotPureI64Plan,
    rows: &[RuntimeI64Args],
    out: &mut [i64],
) -> Result<(), RuntimeEvalError> {
    let mut run = || {
        rows.par_iter()
            .zip(out.par_iter_mut())
            .try_for_each_init(Vec::new, |slots, (row, slot)| {
                compiled
                    .call_with_inputs_scratch(row.as_slice(), slots)
                    .map(|(value, _)| *slot = value)
            })
    };
    match pool {
        Some(pool) => pool.install(run),
        None => run(),
    }
}

fn call_aot_flat_batch(
    compiled: &AotPureI64Plan,
    flat_inputs: &[i64],
    arity: usize,
    out: &mut [i64],
    slots: &mut Vec<i64>,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            compiled
                .call_with_inputs_scratch(&[], slots)
                .map(|(value, _)| *slot = value)
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            compiled
                .call_with_inputs_scratch(row, slots)
                .map(|(value, _)| *slot = value)
        })
}

fn call_aot_flat_batch_parallel(
    pool: Option<&ThreadPool>,
    compiled: &AotPureI64Plan,
    flat_inputs: &[i64],
    arity: usize,
    out: &mut [i64],
) -> Result<(), RuntimeEvalError> {
    let mut run = || {
        if arity == 0 {
            return out
                .par_iter_mut()
                .try_for_each_init(Vec::new, |slots, slot| {
                    compiled
                        .call_with_inputs_scratch(&[], slots)
                        .map(|(value, _)| *slot = value)
                });
        }
        flat_inputs
            .par_chunks_exact(arity)
            .zip(out.par_iter_mut())
            .try_for_each_init(Vec::new, |slots, (row, slot)| {
                compiled
                    .call_with_inputs_scratch(row, slots)
                    .map(|(value, _)| *slot = value)
            })
    };
    match pool {
        Some(pool) => pool.install(run),
        None => run(),
    }
}

fn call_aot_flat_batch_sum(
    compiled: &AotPureI64Plan,
    flat_inputs: &[i64],
    arity: usize,
    rows: usize,
    slots: &mut Vec<i64>,
) -> Result<i64, RuntimeEvalError> {
    let mut sum = 0i64;
    if arity == 0 {
        for _ in 0..rows {
            let (value, _) = compiled.call_with_inputs_scratch(&[], slots)?;
            sum += value;
        }
        return Ok(sum);
    }
    for row in flat_inputs.chunks_exact(arity) {
        let (value, _) = compiled.call_with_inputs_scratch(row, slots)?;
        sum += value;
    }
    Ok(sum)
}

fn call_aot_flat_batch_sum_parallel(
    pool: Option<&ThreadPool>,
    compiled: &AotPureI64Plan,
    flat_inputs: &[i64],
    arity: usize,
    rows: usize,
) -> Result<i64, RuntimeEvalError> {
    let run = || {
        if arity == 0 {
            return (0..rows)
                .into_par_iter()
                .try_fold(
                    || (Vec::new(), 0i64),
                    |(mut slots, sum), _| {
                        let (value, _) = compiled.call_with_inputs_scratch(&[], &mut slots)?;
                        Ok::<(Vec<i64>, i64), RuntimeEvalError>((slots, sum + value))
                    },
                )
                .map(|result| result.map(|(_, sum)| sum))
                .try_reduce(|| 0, |lhs, rhs| Ok(lhs + rhs));
        }
        flat_inputs
            .par_chunks_exact(arity)
            .try_fold(
                || (Vec::new(), 0i64),
                |(mut slots, sum), row| {
                    let (value, _) = compiled.call_with_inputs_scratch(row, &mut slots)?;
                    Ok::<(Vec<i64>, i64), RuntimeEvalError>((slots, sum + value))
                },
            )
            .map(|result| result.map(|(_, sum)| sum))
            .try_reduce(|| 0, |lhs, rhs| Ok(lhs + rhs))
    };
    match pool {
        Some(pool) => pool.install(run),
        None => run(),
    }
}

fn call_aot_flat_batch_sum_with_policy(
    policy: FlatBatchSumPolicy<'_>,
    stats: &mut RuntimePureCallStats,
    compiled: &AotPureI64Plan,
    shape: FlatBatchSumShape<'_>,
    slots: &mut Vec<i64>,
) -> Result<i64, RuntimeEvalError> {
    if policy.wants_parallel {
        stats.thread_pool_jobs += policy.parallel_jobs;
        call_aot_flat_batch_sum_parallel(
            policy.pool,
            compiled,
            shape.flat_inputs,
            shape.arity,
            shape.rows,
        )
    } else {
        call_aot_flat_batch_sum(compiled, shape.flat_inputs, shape.arity, shape.rows, slots)
    }
}

fn call_vm_batch(
    helper: &RuntimePureHelper,
    rows: &[RuntimeI64Args],
    out: &mut [i64],
    scratch: &mut VmPureFunctionScratch,
) -> Result<(), RuntimeEvalError> {
    rows.iter().zip(out.iter_mut()).try_for_each(|(row, slot)| {
        *slot = exact_i64_result(scratch.evaluate_i64_args(helper, *row)?)?;
        Ok(())
    })
}

fn call_vm_batch_parallel(
    pool: Option<&ThreadPool>,
    helper: &RuntimePureHelper,
    rows: &[RuntimeI64Args],
    out: &mut [i64],
) -> Result<(), RuntimeEvalError> {
    let mut run = || {
        rows.par_iter().zip(out.par_iter_mut()).try_for_each_init(
            VmPureFunctionScratch::default,
            |scratch, (row, slot)| {
                *slot = exact_i64_result(scratch.evaluate_i64_args(helper, *row)?)?;
                Ok(())
            },
        )
    };
    match pool {
        Some(pool) => pool.install(run),
        None => run(),
    }
}

fn call_vm_flat_batch(
    helper: &RuntimePureHelper,
    flat_inputs: &[i64],
    arity: usize,
    out: &mut [i64],
    scratch: &mut VmPureFunctionScratch,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            *slot = exact_i64_result(scratch.evaluate_i64_slice(helper, &[])?)?;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            *slot = exact_i64_result(scratch.evaluate_i64_slice(helper, row)?)?;
            Ok(())
        })
}

fn call_vm_i32_flat_batch(
    helper: &RuntimePureHelper,
    flat_inputs: &[i32],
    arity: usize,
    out: &mut [i32],
    scratch: &mut VmPureFunctionScratch,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            *slot = vm_i32_result(helper, scratch.evaluate_i32_slice(helper, &[])?)?;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            *slot = vm_i32_result(helper, scratch.evaluate_i32_slice(helper, row)?)?;
            Ok(())
        })
}

fn call_vm_i32_flat_batch_sum(
    helper: &RuntimePureHelper,
    flat_inputs: &[i32],
    arity: usize,
    rows: usize,
    scratch: &mut VmPureFunctionScratch,
) -> Result<i64, RuntimeEvalError> {
    let mut sum = 0_i64;
    if arity == 0 {
        for _ in 0..rows {
            sum += i64::from(vm_i32_result(
                helper,
                scratch.evaluate_i32_slice(helper, &[])?,
            )?);
        }
        return Ok(sum);
    }
    for row in flat_inputs.chunks_exact(arity) {
        sum += i64::from(vm_i32_result(
            helper,
            scratch.evaluate_i32_slice(helper, row)?,
        )?);
    }
    Ok(sum)
}

fn call_vm_exact_int_flat_batch_sum<T: RuntimePureScalarInteger>(
    helper: &RuntimePureHelper,
    flat_inputs: &[T],
    arity: usize,
    rows: usize,
    scratch: &mut VmPureFunctionScratch,
) -> Result<i64, RuntimeEvalError> {
    let mut sum = 0_i64;
    if arity == 0 {
        for _ in 0..rows {
            let value = scratch.evaluate_exact_int_slice::<T>(helper, &[])?;
            sum += T::try_from_runtime_value(&helper.name, value)?.try_sum_as_i64(&helper.name)?;
        }
        return Ok(sum);
    }
    for row in flat_inputs.chunks_exact(arity) {
        let value = scratch.evaluate_exact_int_slice::<T>(helper, row)?;
        sum += T::try_from_runtime_value(&helper.name, value)?.try_sum_as_i64(&helper.name)?;
    }
    Ok(sum)
}

fn call_vm_exact_int_flat_batch<T: RuntimePureScalarInteger>(
    helper: &RuntimePureHelper,
    flat_inputs: &[T],
    arity: usize,
    out: &mut [T],
    scratch: &mut VmPureFunctionScratch,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            let value = scratch.evaluate_exact_int_slice::<T>(helper, &[])?;
            *slot = T::try_from_runtime_value(&helper.name, value)?;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            let value = scratch.evaluate_exact_int_slice::<T>(helper, row)?;
            *slot = T::try_from_runtime_value(&helper.name, value)?;
            Ok(())
        })
}

fn call_vm_usize_flat_batch_sum(
    helper: &RuntimePureHelper,
    flat_inputs: &[u64],
    arity: usize,
    rows: usize,
    scratch: &mut VmPureFunctionScratch,
) -> Result<i64, RuntimeEvalError> {
    let mut sum = 0_i64;
    if arity == 0 {
        for _ in 0..rows {
            let value = vm_usize_result(helper, scratch.evaluate_usize_slice(helper, &[])?)?;
            sum += usize_result_as_i64(helper, value)?;
        }
        return Ok(sum);
    }
    for row in flat_inputs.chunks_exact(arity) {
        let value = vm_usize_result(helper, scratch.evaluate_usize_slice(helper, row)?)?;
        sum += usize_result_as_i64(helper, value)?;
    }
    Ok(sum)
}

fn call_vm_usize_flat_batch(
    helper: &RuntimePureHelper,
    flat_inputs: &[u64],
    arity: usize,
    out: &mut [u64],
    scratch: &mut VmPureFunctionScratch,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            *slot = vm_usize_result(helper, scratch.evaluate_usize_slice(helper, &[])?)?;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            *slot = vm_usize_result(helper, scratch.evaluate_usize_slice(helper, row)?)?;
            Ok(())
        })
}

fn call_aot_exact_int_flat_batch<T: RuntimePureScalarInteger>(
    compiled: &RuntimePureAotPlan,
    flat_inputs: &[T],
    arity: usize,
    out: &mut [T],
    slots: &mut Vec<RuntimePureScalar>,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            let (value, _) = compiled.call_exact_int_with_inputs_scratch(&[], slots)?;
            *slot = value;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            let (value, _) = compiled.call_exact_int_with_inputs_scratch(row, slots)?;
            *slot = value;
            Ok(())
        })
}

fn call_aot_usize_flat_batch(
    compiled: &RuntimePureAotPlan,
    flat_inputs: &[u64],
    arity: usize,
    out: &mut [u64],
    slots: &mut Vec<RuntimePureScalar>,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            let (value, _) = compiled.call_usize_with_inputs_scratch(&[], slots)?;
            *slot = value;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            let (value, _) = compiled.call_usize_with_inputs_scratch(row, slots)?;
            *slot = value;
            Ok(())
        })
}

fn call_aot_exact_int_flat_batch_sum<T: RuntimePureScalarInteger>(
    compiled: &RuntimePureAotPlan,
    flat_inputs: &[T],
    arity: usize,
    rows: usize,
    slots: &mut Vec<RuntimePureScalar>,
) -> Result<i64, RuntimeEvalError> {
    let mut sum = 0_i64;
    if arity == 0 {
        for _ in 0..rows {
            let (value, _) = compiled.call_exact_int_with_inputs_scratch::<T>(&[], slots)?;
            sum += value.try_sum_as_i64("aot_exact_int_batch_sum")?;
        }
        return Ok(sum);
    }
    for row in flat_inputs.chunks_exact(arity) {
        let (value, _) = compiled.call_exact_int_with_inputs_scratch(row, slots)?;
        sum += value.try_sum_as_i64("aot_exact_int_batch_sum")?;
    }
    Ok(sum)
}

fn call_aot_usize_flat_batch_sum(
    compiled: &RuntimePureAotPlan,
    flat_inputs: &[u64],
    arity: usize,
    rows: usize,
    slots: &mut Vec<RuntimePureScalar>,
) -> Result<i64, RuntimeEvalError> {
    let mut sum = 0_i64;
    if arity == 0 {
        for _ in 0..rows {
            let (value, _) = compiled.call_usize_with_inputs_scratch(&[], slots)?;
            sum += usize_result_as_i64_name("aot_usize_batch_sum", value)?;
        }
        return Ok(sum);
    }
    for row in flat_inputs.chunks_exact(arity) {
        let (value, _) = compiled.call_usize_with_inputs_scratch(row, slots)?;
        sum += usize_result_as_i64_name("aot_usize_batch_sum", value)?;
    }
    Ok(sum)
}

fn call_vm_f32_flat_batch(
    helper: &RuntimePureHelper,
    flat_inputs: &[f32],
    arity: usize,
    out: &mut [f32],
    scratch: &mut VmPureFunctionScratch,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            *slot = vm_f32_result(helper, scratch.evaluate_f32_slice(helper, &[])?)?;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            *slot = vm_f32_result(helper, scratch.evaluate_f32_slice(helper, row)?)?;
            Ok(())
        })
}

fn call_aot_f32_flat_batch(
    compiled: &RuntimePureAotPlan,
    flat_inputs: &[f32],
    arity: usize,
    out: &mut [f32],
    slots: &mut Vec<RuntimePureScalar>,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            let (value, _) = compiled.call_f32_with_inputs_scratch(&[], slots)?;
            *slot = value;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            let (value, _) = compiled.call_f32_with_inputs_scratch(row, slots)?;
            *slot = value;
            Ok(())
        })
}

fn call_vm_f64_flat_batch(
    helper: &RuntimePureHelper,
    flat_inputs: &[f64],
    arity: usize,
    out: &mut [f64],
    scratch: &mut VmPureFunctionScratch,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            *slot = vm_f64_result(helper, scratch.evaluate_f64_slice(helper, &[])?)?;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            *slot = vm_f64_result(helper, scratch.evaluate_f64_slice(helper, row)?)?;
            Ok(())
        })
}

fn call_aot_f64_flat_batch(
    compiled: &RuntimePureAotPlan,
    flat_inputs: &[f64],
    arity: usize,
    out: &mut [f64],
    slots: &mut Vec<RuntimePureScalar>,
) -> Result<(), RuntimeEvalError> {
    if arity == 0 {
        return out.iter_mut().try_for_each(|slot| {
            let (value, _) = compiled.call_f64_with_inputs_scratch(&[], slots)?;
            *slot = value;
            Ok(())
        });
    }
    flat_inputs
        .chunks_exact(arity)
        .zip(out.iter_mut())
        .try_for_each(|(row, slot)| {
            let (value, _) = compiled.call_f64_with_inputs_scratch(row, slots)?;
            *slot = value;
            Ok(())
        })
}

fn vm_i32_result(helper: &RuntimePureHelper, value: RuntimeValue) -> Result<i32, RuntimeEvalError> {
    match value {
        RuntimeValue::Int(value) => {
            value
                .exact_i32()
                .ok_or_else(|| RuntimeEvalError::UnsupportedPure {
                    name: helper.name.clone(),
                    reason: format!("pure i32 result `{value}` is outside i32 range"),
                })
        }
        value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
    }
}

fn vm_f32_result(helper: &RuntimePureHelper, value: RuntimeValue) -> Result<f32, RuntimeEvalError> {
    match value {
        RuntimeValue::F32(value) => Ok(value),
        value => Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure f32 result expected f32, got {}",
                runtime_value_kind(&value)
            ),
        }),
    }
}

fn vm_f64_result(helper: &RuntimePureHelper, value: RuntimeValue) -> Result<f64, RuntimeEvalError> {
    match value {
        RuntimeValue::F64(value) => Ok(value),
        value => Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure f64 result expected f64, got {}",
                runtime_value_kind(&value)
            ),
        }),
    }
}

fn vm_usize_result(
    helper: &RuntimePureHelper,
    value: RuntimeValue,
) -> Result<u64, RuntimeEvalError> {
    match value {
        RuntimeValue::UInt(arcweft_core::value::RuntimeUInt::USize(value)) => Ok(value),
        value => Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: format!(
                "pure usize result expected usize, got {}",
                runtime_value_kind(&value)
            ),
        }),
    }
}

fn usize_result_as_i64(helper: &RuntimePureHelper, value: u64) -> Result<i64, RuntimeEvalError> {
    usize_result_as_i64_name(&helper.name, value)
}

fn usize_result_as_i64_name(name: &str, value: u64) -> Result<i64, RuntimeEvalError> {
    i64::try_from(value).map_err(|_| RuntimeEvalError::UnsupportedPure {
        name: name.to_owned(),
        reason: format!("pure usize result `{value}` cannot be represented as an i64 sum"),
    })
}

fn call_vm_flat_batch_parallel(
    pool: Option<&ThreadPool>,
    helper: &RuntimePureHelper,
    flat_inputs: &[i64],
    arity: usize,
    out: &mut [i64],
) -> Result<(), RuntimeEvalError> {
    let mut run = || {
        if arity == 0 {
            return out.par_iter_mut().try_for_each_init(
                VmPureFunctionScratch::default,
                |scratch, slot| {
                    *slot = exact_i64_result(scratch.evaluate_i64_slice(helper, &[])?)?;
                    Ok(())
                },
            );
        }
        flat_inputs
            .par_chunks_exact(arity)
            .zip(out.par_iter_mut())
            .try_for_each_init(VmPureFunctionScratch::default, |scratch, (row, slot)| {
                *slot = exact_i64_result(scratch.evaluate_i64_slice(helper, row)?)?;
                Ok(())
            })
    };
    match pool {
        Some(pool) => pool.install(run),
        None => run(),
    }
}

fn call_vm_flat_batch_sum(
    helper: &RuntimePureHelper,
    flat_inputs: &[i64],
    arity: usize,
    rows: usize,
    scratch: &mut VmPureFunctionScratch,
) -> Result<i64, RuntimeEvalError> {
    let mut sum = 0i64;
    if arity == 0 {
        for _ in 0..rows {
            sum += exact_i64_result(scratch.evaluate_i64_slice(helper, &[])?)?;
        }
        return Ok(sum);
    }
    for row in flat_inputs.chunks_exact(arity) {
        sum += exact_i64_result(scratch.evaluate_i64_slice(helper, row)?)?;
    }
    Ok(sum)
}

fn call_vm_flat_batch_sum_parallel(
    pool: Option<&ThreadPool>,
    helper: &RuntimePureHelper,
    flat_inputs: &[i64],
    arity: usize,
    rows: usize,
) -> Result<i64, RuntimeEvalError> {
    let run = || {
        if arity == 0 {
            return (0..rows)
                .into_par_iter()
                .try_fold(
                    || (VmPureFunctionScratch::default(), 0i64),
                    |(mut scratch, sum), _| {
                        let value = exact_i64_result(scratch.evaluate_i64_slice(helper, &[])?)?;
                        Ok::<(VmPureFunctionScratch, i64), RuntimeEvalError>((scratch, sum + value))
                    },
                )
                .map(|result| result.map(|(_, sum)| sum))
                .try_reduce(|| 0, |lhs, rhs| Ok(lhs + rhs));
        }
        flat_inputs
            .par_chunks_exact(arity)
            .try_fold(
                || (VmPureFunctionScratch::default(), 0i64),
                |(mut scratch, sum), row| {
                    let value = exact_i64_result(scratch.evaluate_i64_slice(helper, row)?)?;
                    Ok::<(VmPureFunctionScratch, i64), RuntimeEvalError>((scratch, sum + value))
                },
            )
            .map(|result| result.map(|(_, sum)| sum))
            .try_reduce(|| 0, |lhs, rhs| Ok(lhs + rhs))
    };
    match pool {
        Some(pool) => pool.install(run),
        None => run(),
    }
}

fn call_vm_flat_batch_sum_with_policy(
    policy: FlatBatchSumPolicy<'_>,
    stats: &mut RuntimePureCallStats,
    helper: &RuntimePureHelper,
    shape: FlatBatchSumShape<'_>,
    scratch: &mut VmPureFunctionScratch,
) -> Result<i64, RuntimeEvalError> {
    if policy.wants_parallel {
        stats.thread_pool_jobs += policy.parallel_jobs;
        call_vm_flat_batch_sum_parallel(
            policy.pool,
            helper,
            shape.flat_inputs,
            shape.arity,
            shape.rows,
        )
    } else {
        call_vm_flat_batch_sum(helper, shape.flat_inputs, shape.arity, shape.rows, scratch)
    }
}

fn compile_request(
    helper: &RuntimePureHelper,
    zero: impl Fn() -> RuntimeValue + Copy,
) -> PureFunctionRequest {
    PureFunctionRequest::new(
        helper.name.clone(),
        helper.expr.clone(),
        helper
            .input_names
            .iter()
            .cloned()
            .map(|name| RuntimeBinding {
                name,
                value: zero(),
            }),
    )
}

/// Summary of helpers selected for acceleration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimePureAccelerationSummary {
    pub annotated: usize,
    pub inferred: usize,
    pub jit: usize,
    pub aot: usize,
    pub vm: usize,
}

impl RuntimePureAccelerator {
    pub fn summary(&self) -> RuntimePureAccelerationSummary {
        let mut jit = 0;
        let mut aot = 0;
        let mut vm = 0;
        for entry in self.cache.iter().filter_map(Option::as_ref) {
            match entry {
                RuntimePureCacheEntry::Jit(_)
                | RuntimePureCacheEntry::JitI32(_)
                | RuntimePureCacheEntry::JitF32(_)
                | RuntimePureCacheEntry::JitF64(_)
                | RuntimePureCacheEntry::AutoAot { jit: Some(_), .. } => jit += 1,
                RuntimePureCacheEntry::Aot(_)
                | RuntimePureCacheEntry::AutoAot { jit: None, .. } => {
                    aot += 1;
                }
                RuntimePureCacheEntry::Vm => vm += 1,
            }
        }
        RuntimePureAccelerationSummary {
            annotated: self.helper_summary.annotated,
            inferred: self.helper_summary.inferred,
            jit,
            aot,
            vm,
        }
    }
}

fn helper_summary_from_helpers(helpers: &[RuntimePureHelper]) -> RuntimePureAccelerationSummary {
    let annotated = helpers
        .iter()
        .filter(|helper| helper.origin == RuntimePureHelperOrigin::Annotated)
        .count();
    RuntimePureAccelerationSummary {
        annotated,
        inferred: helpers.len().saturating_sub(annotated),
        jit: 0,
        aot: 0,
        vm: 0,
    }
}

impl Default for RuntimePureAcceleratorConfig {
    fn default() -> Self {
        Self {
            backend: RuntimePureBackendMode::Auto,
            workers: RuntimePureWorkerCount::Auto,
            batch_min_len: 1024,
            math: math::RuntimeMathAcceleratorConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::{
        engine::{Engine, FlowExit, FlowFiberStatus},
        plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan, RuntimePureHelperId},
        step::{RuntimeStepInput, RuntimeStepOptions},
        value::{RuntimeBinaryOp, RuntimeCallTarget, RuntimeExpr},
    };

    #[test]
    fn runtime_flow_math_intrinsic_uses_adapter_math_accelerator() {
        let lhs = DenseMatrixF32::new(
            4,
            4,
            vec![
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        )
        .expect("matrix shape is valid");
        let rhs = DenseMatrixF32::new(
            4,
            4,
            vec![
                1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0,
                16.0,
            ],
        )
        .expect("matrix shape is valid");
        let plan = RuntimePlan::new(
            Some(FlowRuntimeId("flow.math".to_owned())),
            vec![RuntimeFlow {
                id: FlowRuntimeId("flow.math".to_owned()),
                ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Call {
                    callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::MathMatmulF32),
                    args: vec![
                        RuntimeExpr::Value(RuntimeValue::matrix_f32(lhs)),
                        RuntimeExpr::Value(RuntimeValue::matrix_f32(rhs)),
                    ],
                })],
            }],
            Vec::new(),
        )
        .expect("runtime plan is valid");
        let mut engine = Engine::new(plan);
        let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, &[]);

        let result = engine.step_with_pure_backend(
            RuntimeStepInput::default(),
            RuntimeStepOptions::default(),
            &mut accelerator,
        );

        assert_eq!(result.stats.pure.math_calls, 1);
        assert_eq!(result.stats.pure.math_accelerated_calls, 1);
        assert_eq!(
            accelerator.math_stats().last_backend,
            Some(math::RuntimeMathBackend::Glam)
        );
        assert!(matches!(
            result.fiber_status,
            FlowFiberStatus::Done(FlowExit::Return(_))
        ));
    }

    #[cfg(feature = "math-wgpu")]
    #[test]
    fn runtime_wgpu_math_cache_reuses_prepared_matmul_buffers_across_counter_reset() {
        let lhs = DenseMatrixF32::new(16, 16, vec![1.0; 256]).expect("matrix shape is valid");
        let rhs = DenseMatrixF32::new(16, 16, vec![2.0; 256]).expect("matrix shape is valid");
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                math: math::RuntimeMathAcceleratorConfig {
                    backend: math::RuntimeMathBackend::Wgpu,
                    ..math::RuntimeMathAcceleratorConfig::default()
                },
                ..RuntimePureAcceleratorConfig::default()
            },
            &[],
        );

        let Ok(first) = RuntimePureCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs)
        else {
            return;
        };
        assert_eq!(first.rows(), 16);
        assert_eq!(first.cols(), 16);
        assert_eq!(accelerator.math_stats().gpu_buffer_creations, 4);

        accelerator.reset_runtime_counters();
        let second = RuntimePureCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs)
            .expect("prepared runtime math matmul cache is reusable");

        assert_eq!(second.values(), first.values());
        assert_eq!(accelerator.math_stats().wgpu_calls, 1);
        assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
        assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 4);
        assert_eq!(accelerator.math_stats().gpu_reused_dispatches, 1);
        assert_eq!(accelerator.math_stats().bytes_uploaded, 0);
        assert_eq!(
            accelerator.math_stats().bytes_downloaded,
            std::mem::size_of_val(first.values())
        );
        assert_eq!(
            accelerator.stats().arg_bytes_borrowed,
            (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>()
        );
        assert_eq!(
            accelerator.stats().result_bytes_copied,
            std::mem::size_of_val(second.values())
        );
    }

    #[cfg(feature = "math-wgpu")]
    #[test]
    fn runtime_auto_wgpu_matmul_uses_prepared_cache_when_threshold_selects_gpu() {
        let lhs = DenseMatrixF32::new(8, 8, vec![1.0; 64]).expect("matrix shape is valid");
        let rhs = DenseMatrixF32::new(8, 8, vec![2.0; 64]).expect("matrix shape is valid");
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                math: math::RuntimeMathAcceleratorConfig {
                    backend: math::RuntimeMathBackend::Auto,
                    wgpu_min_elements: 1,
                },
                ..RuntimePureAcceleratorConfig::default()
            },
            &[],
        );

        let Ok(first) = RuntimePureCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs)
        else {
            return;
        };
        assert_eq!(
            accelerator.math_stats().last_auto_reason,
            Some(math::RuntimeMathAutoSelectionReason::MatmulWgpuWorkThreshold)
        );

        accelerator.reset_runtime_counters();
        let second = RuntimePureCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs)
            .expect("auto-selected wgpu matmul reuses prepared runtime cache");

        assert_eq!(second.values(), first.values());
        assert_eq!(
            accelerator.math_stats().last_auto_reason,
            Some(math::RuntimeMathAutoSelectionReason::MatmulWgpuWorkThreshold)
        );
        assert_eq!(accelerator.math_stats().wgpu_calls, 1);
        assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
        assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 4);
        assert_eq!(accelerator.math_stats().bytes_uploaded, 0);
    }

    #[cfg(feature = "math-wgpu")]
    #[test]
    fn runtime_wgpu_math_cache_reuses_prepared_tensor_add_buffers() {
        let lhs = DenseTensorF32::new(vec![32], vec![1.0; 32]).expect("tensor shape is valid");
        let rhs = DenseTensorF32::new(vec![32], vec![2.0; 32]).expect("tensor shape is valid");
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                math: math::RuntimeMathAcceleratorConfig {
                    backend: math::RuntimeMathBackend::Wgpu,
                    ..math::RuntimeMathAcceleratorConfig::default()
                },
                ..RuntimePureAcceleratorConfig::default()
            },
            &[],
        );

        let Ok(first) =
            RuntimePureCallBackend::call_math_tensor_add_f32(&mut accelerator, &lhs, &rhs)
        else {
            return;
        };
        assert_eq!(first.values(), vec![3.0; 32].as_slice());

        accelerator.reset_runtime_counters();
        let second = RuntimePureCallBackend::call_math_tensor_add_f32(&mut accelerator, &lhs, &rhs)
            .expect("prepared runtime tensor add cache is reusable");

        assert_eq!(second.values(), vec![3.0; 32].as_slice());
        assert_eq!(accelerator.math_stats().wgpu_calls, 1);
        assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
        assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 4);
        assert_eq!(accelerator.math_stats().bytes_uploaded, 0);
        assert_eq!(
            accelerator.math_stats().bytes_downloaded,
            std::mem::size_of_val(second.values())
        );
    }

    #[test]
    fn auto_accelerator_uses_aot_for_cold_scalar_calls_without_value_vec_allocation() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "score".to_owned(),
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
            output_type: RuntimePureOutputType::I64,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
                }),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::new(
            RuntimePureBackendMode::Auto,
            std::slice::from_ref(&helper),
        );

        let value = accelerator
            .call_i64(&helper, RuntimeI64Args::new([3, 4, 0, 0], 2))
            .expect("accelerated call succeeds");

        assert_eq!(value, Some(18));
        assert_eq!(accelerator.stats().pure_calls, 1);
        assert_eq!(accelerator.stats().arg_stack_packs, 1);
        assert_eq!(accelerator.stats().arg_vec_allocations, 0);
        assert_eq!(
            accelerator.stats().arg_bytes_copied,
            2 * std::mem::size_of::<i64>()
        );
        assert_eq!(accelerator.stats().result_bytes_copied, 0);
        assert!(accelerator.resolved_worker_count() >= 1);
        assert!(!accelerator.has_worker_pool());
        assert_eq!(accelerator.summary().aot, 1);
        assert_eq!(accelerator.summary().jit, 0);
        assert_eq!(accelerator.compile_stats().auto_aot_selected, 1);
        assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    }

    #[test]
    fn aot_scalar_preserves_i32_and_f32_without_vm_fallback() {
        let i32_helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "i32_score".to_owned(),
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            input_types: vec![RuntimePureInputType::I32, RuntimePureInputType::I32],
            output_type: RuntimePureOutputType::I32,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let f32_helper = RuntimePureHelper {
            id: RuntimePureHelperId(1),
            name: "f32_score".to_owned(),
            input_names: vec!["base".to_owned(), "scale".to_owned()],
            input_types: vec![RuntimePureInputType::F32, RuntimePureInputType::F32],
            output_type: RuntimePureOutputType::F32,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Aot,
                workers: RuntimePureWorkerCount::Fixed(1),
                batch_min_len: 1024,
                ..RuntimePureAcceleratorConfig::default()
            },
            &[i32_helper.clone(), f32_helper.clone()],
        );

        let i32_value = accelerator
            .call_i32_slice(&i32_helper, &[7, 9])
            .expect("i32 AOT scalar succeeds");
        let f32_value = accelerator
            .call_f32_slice(&f32_helper, &[3.5, 2.0])
            .expect("f32 AOT scalar succeeds");
        let mut i32_out = [0; 3];
        accelerator
            .call_i32_flat_batch(&i32_helper, &[1, 2, 3, 4, 5, 6], 2, &mut i32_out)
            .expect("i32 AOT flat batch succeeds");
        let i32_sum = accelerator
            .call_i32_flat_batch_sum(&i32_helper, &[1, 2, 3, 4, 5, 6], 2, 3)
            .expect("i32 AOT flat batch sum succeeds");

        assert_eq!(i32_value, Some(16));
        assert_eq!(f32_value, Some(7.0));
        assert_eq!(i32_out, [3, 7, 11]);
        assert_eq!(i32_sum, 21);
        assert_eq!(accelerator.stats().aot_calls, 8);
        assert_eq!(accelerator.stats().vm_calls, 0);
        assert_eq!(accelerator.stats().fallbacks, 0);
        assert_eq!(accelerator.summary().aot, 2);
    }

    #[test]
    fn explicit_jit_uses_aot_for_typed_scalar_helpers_without_vm_fallback() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "i16_score".to_owned(),
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            input_types: vec![RuntimePureInputType::I16, RuntimePureInputType::I16],
            output_type: RuntimePureOutputType::I16,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Jit,
                workers: RuntimePureWorkerCount::Fixed(1),
                batch_min_len: 1024,
                ..RuntimePureAcceleratorConfig::default()
            },
            std::slice::from_ref(&helper),
        );
        let value = accelerator
            .call_exact_int_slice(&helper, &[20_i16, 22])
            .expect("typed scalar helper runs through AOT when native JIT is unavailable");

        assert_eq!(value, Some(42));
        assert_eq!(accelerator.stats().aot_calls, 1);
        assert_eq!(accelerator.stats().vm_calls, 0);
        assert_eq!(accelerator.stats().fallbacks, 0);
        assert_eq!(accelerator.compile_stats().jit_attempts, 1);
        assert_eq!(accelerator.compile_stats().jit_failures, 1);
    }

    #[test]
    fn explicit_jit_uses_native_i32_for_slice_and_flat_batch() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "i32_score_jit".to_owned(),
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            input_types: vec![RuntimePureInputType::I32, RuntimePureInputType::I32],
            output_type: RuntimePureOutputType::I32,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i32(2))),
                }),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Jit,
                workers: RuntimePureWorkerCount::Fixed(1),
                batch_min_len: 1024,
                ..RuntimePureAcceleratorConfig::default()
            },
            std::slice::from_ref(&helper),
        );

        let value = accelerator
            .call_i32_slice(&helper, &[3, 4])
            .expect("native i32 JIT slice call succeeds");
        let mut out = [0; 3];
        accelerator
            .call_i32_flat_batch(&helper, &[3, 4, 2, 99, 7, 1], 2, &mut out)
            .expect("native i32 JIT flat batch succeeds");
        let sum = accelerator
            .call_i32_flat_batch_sum(&helper, &[3, 4, 2, 99, 7, 1], 2, 3)
            .expect("native i32 JIT flat batch sum succeeds");

        assert_eq!(value, Some(18));
        assert_eq!(out, [18, 202, 21]);
        assert_eq!(sum, 241);
        assert_eq!(accelerator.stats().jit_calls, 7);
        assert_eq!(accelerator.stats().aot_calls, 0);
        assert_eq!(accelerator.stats().vm_calls, 0);
        assert_eq!(accelerator.summary().jit, 1);
    }

    #[test]
    fn explicit_jit_uses_native_f32_for_slice_and_flat_batch() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "f32_score_jit".to_owned(),
            input_names: vec!["base".to_owned(), "scale".to_owned()],
            input_types: vec![RuntimePureInputType::F32, RuntimePureInputType::F32],
            output_type: RuntimePureOutputType::F32,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F32(2.0))),
                }),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Jit,
                workers: RuntimePureWorkerCount::Fixed(1),
                batch_min_len: 1024,
                ..RuntimePureAcceleratorConfig::default()
            },
            std::slice::from_ref(&helper),
        );

        let value = accelerator
            .call_f32_slice(&helper, &[3.0, 4.0])
            .expect("native f32 JIT slice call succeeds");
        let mut out = [0.0; 3];
        accelerator
            .call_f32_flat_batch(&helper, &[3.0, 4.0, 2.0, 99.0, 7.0, 1.0], 2, &mut out)
            .expect("native f32 JIT flat batch succeeds");

        assert_eq!(value.map(f32::to_bits), Some(18.0f32.to_bits()));
        assert_eq!(
            out.map(f32::to_bits),
            [18.0f32, 202.0, 21.0].map(f32::to_bits)
        );
        assert_eq!(accelerator.stats().jit_calls, 4);
        assert_eq!(accelerator.stats().aot_calls, 0);
        assert_eq!(accelerator.stats().vm_calls, 0);
        assert_eq!(
            accelerator.stats().flat_batch_bytes_borrowed,
            6 * std::mem::size_of::<f32>()
        );
        assert_eq!(
            accelerator.stats().result_bytes_copied,
            3 * std::mem::size_of::<f32>()
        );
        assert_eq!(accelerator.summary().jit, 1);
    }

    #[test]
    fn explicit_jit_uses_native_f64_for_slice_and_flat_batch() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "f64_score_jit".to_owned(),
            input_names: vec!["base".to_owned(), "scale".to_owned()],
            input_types: vec![RuntimePureInputType::F64, RuntimePureInputType::F64],
            output_type: RuntimePureOutputType::F64,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F64(2.0))),
                }),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Jit,
                workers: RuntimePureWorkerCount::Fixed(1),
                batch_min_len: 1024,
                ..RuntimePureAcceleratorConfig::default()
            },
            std::slice::from_ref(&helper),
        );

        let value = accelerator
            .call_f64_slice(&helper, &[3.0, 4.0])
            .expect("native f64 JIT slice call succeeds");
        let mut out = [0.0; 3];
        accelerator
            .call_f64_flat_batch(&helper, &[3.0, 4.0, 2.0, 99.0, 7.0, 1.0], 2, &mut out)
            .expect("native f64 JIT flat batch succeeds");

        assert_eq!(value.map(f64::to_bits), Some(18.0f64.to_bits()));
        assert_eq!(
            out.map(f64::to_bits),
            [18.0f64, 202.0, 21.0].map(f64::to_bits)
        );
        assert_eq!(accelerator.stats().jit_calls, 4);
        assert_eq!(accelerator.stats().aot_calls, 0);
        assert_eq!(accelerator.stats().vm_calls, 0);
        assert_eq!(
            accelerator.stats().flat_batch_bytes_borrowed,
            6 * std::mem::size_of::<f64>()
        );
        assert_eq!(
            accelerator.stats().result_bytes_copied,
            3 * std::mem::size_of::<f64>()
        );
        assert_eq!(accelerator.summary().jit, 1);
    }

    #[test]
    fn auto_promotes_large_i32_flat_batch_to_native_jit() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "i32_score_auto_jit".to_owned(),
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            input_types: vec![RuntimePureInputType::I32, RuntimePureInputType::I32],
            output_type: RuntimePureOutputType::I32,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Auto,
                workers: RuntimePureWorkerCount::Fixed(1),
                batch_min_len: 1024,
                ..RuntimePureAcceleratorConfig::default()
            },
            std::slice::from_ref(&helper),
        );
        let flat_inputs = (0..128).flat_map(|value| [value, 1]).collect::<Vec<i32>>();
        let mut out = [0; 128];

        accelerator
            .call_i32_flat_batch(&helper, &flat_inputs, 2, &mut out)
            .expect("auto promotes large i32 flat batch");

        assert_eq!(out[0], 1);
        assert_eq!(out[127], 128);
        assert_eq!(accelerator.stats().jit_calls, 128);
        assert_eq!(accelerator.stats().aot_calls, 0);
        assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
        assert_eq!(accelerator.summary().jit, 1);
    }

    #[test]
    fn auto_promotes_large_f32_flat_batch_to_native_jit() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "f32_score_auto_jit".to_owned(),
            input_names: vec!["base".to_owned(), "scale".to_owned()],
            input_types: vec![RuntimePureInputType::F32, RuntimePureInputType::F32],
            output_type: RuntimePureOutputType::F32,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F32(2.0))),
                }),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Auto,
                workers: RuntimePureWorkerCount::Fixed(1),
                batch_min_len: 1024,
                ..RuntimePureAcceleratorConfig::default()
            },
            std::slice::from_ref(&helper),
        );
        let flat_inputs = (1..=128)
            .flat_map(|value: u16| [f32::from(value), 2.0])
            .collect::<Vec<f32>>();
        let mut out = [0.0; 128];

        accelerator
            .call_f32_flat_batch(&helper, &flat_inputs, 2, &mut out)
            .expect("auto promotes large f32 flat batch");

        assert_eq!(out[0].to_bits(), 4.0f32.to_bits());
        assert_eq!(out[127].to_bits(), 512.0f32.to_bits());
        assert_eq!(accelerator.stats().jit_calls, 128);
        assert_eq!(accelerator.stats().aot_calls, 0);
        assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
        assert_eq!(accelerator.summary().jit, 1);
    }

    #[test]
    fn auto_promotes_large_f64_flat_batch_to_native_jit() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "f64_score_auto_jit".to_owned(),
            input_names: vec!["base".to_owned(), "scale".to_owned()],
            input_types: vec![RuntimePureInputType::F64, RuntimePureInputType::F64],
            output_type: RuntimePureOutputType::F64,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("scale".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::F64(2.0))),
                }),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Auto,
                workers: RuntimePureWorkerCount::Fixed(1),
                batch_min_len: 1024,
                ..RuntimePureAcceleratorConfig::default()
            },
            std::slice::from_ref(&helper),
        );
        let flat_inputs = (1..=128)
            .flat_map(|value: u16| [f64::from(value), 2.0])
            .collect::<Vec<f64>>();
        let mut out = [0.0; 128];

        accelerator
            .call_f64_flat_batch(&helper, &flat_inputs, 2, &mut out)
            .expect("auto promotes large f64 flat batch");

        assert_eq!(out[0].to_bits(), 4.0f64.to_bits());
        assert_eq!(out[127].to_bits(), 512.0f64.to_bits());
        assert_eq!(accelerator.stats().jit_calls, 128);
        assert_eq!(accelerator.stats().aot_calls, 0);
        assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
        assert_eq!(accelerator.summary().jit, 1);
    }

    #[test]
    fn auto_accelerator_promotes_large_flat_batches_to_jit() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "score".to_owned(),
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
            output_type: RuntimePureOutputType::I64,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
                }),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Auto,
                workers: RuntimePureWorkerCount::Fixed(1),
                batch_min_len: 1024,
                ..RuntimePureAcceleratorConfig::default()
            },
            std::slice::from_ref(&helper),
        );
        let mut flat_inputs = Vec::new();
        for value in 1..=128 {
            flat_inputs.extend([value, 2]);
        }
        let mut out = [0; 128];

        accelerator
            .call_i64_flat_batch(&helper, &flat_inputs, 2, &mut out)
            .expect("large auto flat batch succeeds");

        assert_eq!(out[0], 4);
        assert_eq!(out[127], 512);
        assert_eq!(accelerator.stats().jit_calls, 128);
        assert_eq!(accelerator.stats().aot_calls, 0);
        assert_eq!(accelerator.stats().arg_vec_allocations, 0);
        assert_eq!(accelerator.stats().flatten_materializations, 0);
        assert_eq!(accelerator.stats().flatten_bytes_copied, 0);
        assert_eq!(
            accelerator.stats().flat_batch_bytes_borrowed,
            flat_inputs.len() * std::mem::size_of::<i64>()
        );
        assert_eq!(accelerator.summary().jit, 1);
        assert_eq!(accelerator.compile_stats().auto_aot_selected, 1);
        assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
    }

    #[test]
    fn aot_accelerates_exact_width_scalar_calls_without_i64_widening() {
        fn add_helper(
            id: usize,
            name: &str,
            input_type: RuntimePureInputType,
            output_type: RuntimePureOutputType,
        ) -> RuntimePureHelper {
            RuntimePureHelper {
                id: RuntimePureHelperId(id),
                name: name.to_owned(),
                input_names: vec!["lhs".to_owned(), "rhs".to_owned()],
                input_types: vec![input_type, input_type],
                output_type,
                expr: RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("lhs".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Local("rhs".to_owned())),
                },
                scalar_eval_supported: true,
                origin: RuntimePureHelperOrigin::Annotated,
            }
        }

        let helpers = [
            add_helper(
                0,
                "i32_add",
                RuntimePureInputType::I32,
                RuntimePureOutputType::I32,
            ),
            add_helper(
                1,
                "u32_add",
                RuntimePureInputType::U32,
                RuntimePureOutputType::U32,
            ),
            add_helper(
                2,
                "f32_add",
                RuntimePureInputType::F32,
                RuntimePureOutputType::F32,
            ),
            add_helper(
                3,
                "f64_add",
                RuntimePureInputType::F64,
                RuntimePureOutputType::F64,
            ),
        ];
        let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Aot, &helpers);

        let i32_value = accelerator
            .call_i32_slice(&helpers[0], &[7, 11])
            .expect("i32 AOT call succeeds");
        let u32_value = accelerator
            .call_exact_int_slice::<u32>(&helpers[1], &[13, 17])
            .expect("u32 AOT call succeeds");
        let f32_value = accelerator
            .call_f32_slice(&helpers[2], &[1.25, 2.5])
            .expect("f32 AOT call succeeds");
        let f64_value = accelerator
            .call_f64_slice(&helpers[3], &[3.0, 4.5])
            .expect("f64 AOT call succeeds");

        assert_eq!(i32_value, Some(18));
        assert_eq!(u32_value, Some(30));
        assert_eq!(f32_value, Some(3.75));
        assert_eq!(f64_value, Some(7.5));
        assert_eq!(accelerator.stats().aot_calls, 4);
        assert_eq!(accelerator.stats().vm_calls, 0);
        assert_eq!(accelerator.stats().fallbacks, 0);
        assert_eq!(accelerator.summary().aot, 4);
    }

    #[test]
    fn value_fallback_reuses_vm_scratch_without_value_vec_allocation() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "echo".to_owned(),
            input_names: vec!["label".to_owned()],
            input_types: vec![RuntimePureInputType::Value],
            output_type: RuntimePureOutputType::Value,
            expr: RuntimeExpr::Local("label".to_owned()),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Vm,
                workers: RuntimePureWorkerCount::Fixed(1),
                batch_min_len: 2,
                ..RuntimePureAcceleratorConfig::default()
            },
            std::slice::from_ref(&helper),
        );

        let value = accelerator
            .call_values(&helper, &[RuntimeValue::String("ready".to_owned())])
            .expect("VM value fallback succeeds");

        assert_eq!(value, RuntimeValue::String("ready".to_owned()));
        assert_eq!(accelerator.stats().pure_calls, 1);
        assert_eq!(accelerator.stats().vm_calls, 1);
        assert_eq!(accelerator.stats().fallbacks, 1);
        assert_eq!(accelerator.stats().arg_vec_allocations, 0);
        assert_eq!(
            accelerator.stats().arg_bytes_borrowed,
            std::mem::size_of_val(&[RuntimeValue::String("ready".to_owned())])
        );
    }

    #[test]
    fn aot_batch_matches_scalar_results_and_records_parallel_stats() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "score".to_owned(),
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
            output_type: RuntimePureOutputType::I64,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
                }),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Inferred,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Aot,
                workers: RuntimePureWorkerCount::Fixed(2),
                batch_min_len: 1,
                ..RuntimePureAcceleratorConfig::default()
            },
            std::slice::from_ref(&helper),
        );
        let rows = [
            RuntimeI64Args::new([3, 4, 0, 0], 2),
            RuntimeI64Args::new([5, 1, 0, 0], 2),
            RuntimeI64Args::new([2, 8, 0, 0], 2),
            RuntimeI64Args::new([7, 0, 0, 0], 2),
        ];
        let mut out = [0; 4];

        accelerator
            .call_i64_batch(&helper, &rows, &mut out)
            .expect("batch succeeds");

        assert_eq!(out, [18, 15, 20, 14]);
        assert_eq!(accelerator.stats().batch_calls, 1);
        assert_eq!(accelerator.stats().batch_items, 4);
        assert_eq!(accelerator.stats().aot_calls, 4);
        assert_eq!(accelerator.stats().arg_vec_allocations, 0);
        assert_eq!(accelerator.resolved_worker_count(), 2);
        assert!(accelerator.has_worker_pool());
        assert_eq!(accelerator.stats().parallel_policy_checks, 1);
        assert_eq!(accelerator.stats().parallel_batches, 1);
        assert_eq!(accelerator.stats().parallel_skipped_small, 0);
        assert_eq!(accelerator.stats().parallel_skipped_backend, 0);
        assert!(accelerator.stats().parallel_work_units > rows.len());
        assert!(accelerator.stats().thread_pool_jobs > 0);
    }

    #[test]
    fn aot_worker_pool_is_created_only_for_parallel_batches() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "score".to_owned(),
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
            output_type: RuntimePureOutputType::I64,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
                }),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Aot,
                workers: RuntimePureWorkerCount::Fixed(2),
                batch_min_len: 2,
                ..RuntimePureAcceleratorConfig::default()
            },
            std::slice::from_ref(&helper),
        );
        let small_rows = [
            RuntimeI64Args::new([3, 4, 0, 0], 2),
            RuntimeI64Args::new([5, 1, 0, 0], 2),
        ];
        let mut small_out = [0; 2];

        accelerator
            .call_i64_batch(&helper, &small_rows, &mut small_out)
            .expect("small AOT batch succeeds without pool");

        assert_eq!(small_out, [18, 15]);
        assert!(!accelerator.has_worker_pool());
        assert_eq!(accelerator.stats().parallel_policy_checks, 1);
        assert_eq!(accelerator.stats().parallel_skipped_small, 1);
        assert_eq!(accelerator.stats().thread_pool_jobs, 0);

        let mut small_flat_out = [0; 2];
        accelerator
            .call_i64_flat_batch(&helper, &[3, 4, 5, 1], 2, &mut small_flat_out)
            .expect("small flat AOT batch reuses sequential scratch without pool");

        assert_eq!(small_flat_out, [18, 15]);
        assert!(!accelerator.has_worker_pool());
        assert_eq!(accelerator.stats().parallel_policy_checks, 2);
        assert_eq!(accelerator.stats().parallel_skipped_small, 2);
        assert_eq!(accelerator.stats().thread_pool_jobs, 0);

        let large_rows = [
            RuntimeI64Args::new([3, 4, 0, 0], 2),
            RuntimeI64Args::new([5, 1, 0, 0], 2),
            RuntimeI64Args::new([2, 8, 0, 0], 2),
            RuntimeI64Args::new([7, 0, 0, 0], 2),
            RuntimeI64Args::new([9, 1, 0, 0], 2),
        ];
        let mut large_out = [0; 5];

        accelerator
            .call_i64_batch(&helper, &large_rows, &mut large_out)
            .expect("large AOT batch creates pool");

        assert_eq!(large_out, [18, 15, 20, 14, 27]);
        assert!(accelerator.has_worker_pool());
        assert_eq!(accelerator.stats().parallel_policy_checks, 3);
        assert_eq!(accelerator.stats().parallel_batches, 1);
        assert_eq!(accelerator.stats().parallel_skipped_small, 2);
        assert_eq!(accelerator.stats().thread_pool_jobs, 2);
    }

    #[test]
    fn jit_batch_matches_scalar_results_without_value_vec_allocation() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "score".to_owned(),
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
            output_type: RuntimePureOutputType::I64,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
                }),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Jit,
                workers: RuntimePureWorkerCount::Fixed(1),
                batch_min_len: 2,
                ..RuntimePureAcceleratorConfig::default()
            },
            std::slice::from_ref(&helper),
        );
        let rows = [
            RuntimeI64Args::new([3, 4, 0, 0], 2),
            RuntimeI64Args::new([5, 1, 0, 0], 2),
            RuntimeI64Args::new([2, 8, 0, 0], 2),
        ];
        let mut out = [0; 3];

        RuntimePureCallBackend::call_i64_batch(&mut accelerator, &helper, &rows, &mut out)
            .expect("JIT batch succeeds");

        assert_eq!(out, [18, 15, 20]);
        assert_eq!(accelerator.stats().batch_calls, 1);
        assert_eq!(accelerator.stats().batch_items, 3);
        assert_eq!(accelerator.stats().jit_calls, 3);
        assert_eq!(accelerator.stats().arg_stack_packs, 3);
        assert_eq!(accelerator.stats().arg_vec_allocations, 0);
        assert_eq!(accelerator.stats().flat_batch_calls, 0);
        assert_eq!(accelerator.stats().flat_batch_items, 0);
        assert_eq!(accelerator.stats().flatten_materializations, 1);
        assert_eq!(accelerator.stats().parallel_policy_checks, 1);
        assert_eq!(accelerator.stats().parallel_skipped_backend, 1);
        assert_eq!(accelerator.stats().parallel_batches, 0);
        assert_eq!(
            accelerator.stats().flatten_bytes_copied,
            6 * std::mem::size_of::<i64>()
        );
        assert_eq!(accelerator.summary().jit, 1);
    }

    #[test]
    fn jit_flat_batch_sum_avoids_output_copy() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "score".to_owned(),
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
            output_type: RuntimePureOutputType::I64,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
                }),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Jit,
                workers: RuntimePureWorkerCount::Fixed(1),
                batch_min_len: 2,
                ..RuntimePureAcceleratorConfig::default()
            },
            std::slice::from_ref(&helper),
        );

        let sum = accelerator
            .call_i64_flat_batch_sum(&helper, &[3, 4, 5, 1, 2, 8], 2, 3)
            .expect("JIT flat batch sum succeeds");

        assert_eq!(sum, 53);
        assert_eq!(accelerator.stats().batch_calls, 1);
        assert_eq!(accelerator.stats().batch_items, 3);
        assert_eq!(accelerator.stats().flat_batch_calls, 1);
        assert_eq!(accelerator.stats().flat_batch_items, 3);
        assert_eq!(
            accelerator.stats().flat_batch_bytes_borrowed,
            6 * std::mem::size_of::<i64>()
        );
        assert_eq!(accelerator.stats().jit_calls, 3);
        assert_eq!(accelerator.stats().arg_stack_packs, 0);
        assert_eq!(accelerator.stats().arg_vec_allocations, 0);
        assert_eq!(accelerator.stats().flatten_materializations, 0);
        assert_eq!(accelerator.stats().flatten_bytes_copied, 0);
        assert_eq!(accelerator.stats().result_bytes_copied, 0);
        assert_eq!(accelerator.stats().parallel_policy_checks, 1);
        assert_eq!(accelerator.stats().parallel_skipped_backend, 1);
        assert_eq!(accelerator.stats().parallel_batches, 0);
    }

    #[test]
    fn vm_batch_uses_i64_args_without_value_vec_allocation() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "score".to_owned(),
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
            output_type: RuntimePureOutputType::I64,
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
                }),
            },
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Vm,
                workers: RuntimePureWorkerCount::Fixed(2),
                batch_min_len: 1,
                ..RuntimePureAcceleratorConfig::default()
            },
            std::slice::from_ref(&helper),
        );
        let rows = [
            RuntimeI64Args::new([3, 4, 0, 0], 2),
            RuntimeI64Args::new([5, 1, 0, 0], 2),
            RuntimeI64Args::new([2, 8, 0, 0], 2),
        ];
        let mut out = [0; 3];

        accelerator
            .call_i64_batch(&helper, &rows, &mut out)
            .expect("VM batch succeeds");

        assert_eq!(out, [18, 15, 20]);
        assert_eq!(accelerator.stats().batch_calls, 1);
        assert_eq!(accelerator.stats().batch_items, 3);
        assert_eq!(accelerator.stats().vm_calls, 3);
        assert_eq!(accelerator.stats().fallbacks, 3);
        assert_eq!(accelerator.stats().arg_stack_packs, 3);
        assert_eq!(accelerator.stats().arg_vec_allocations, 0);
        assert_eq!(accelerator.stats().parallel_policy_checks, 1);
        assert_eq!(accelerator.stats().parallel_batches, 1);
        assert_eq!(accelerator.stats().thread_pool_jobs, 2);
    }
}
