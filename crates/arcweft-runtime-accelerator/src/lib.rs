//! Runtime pure helper acceleration adapters.
//!
//! This crate owns native acceleration state so `arcweft-core` can stay Sans I/O
//! and dependency-light.

use arcweft_core::{
    plan::{RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin},
    pure::{
        AotPureFunctionBackend, AotPureI64Plan, PureFunctionBackend, PureFunctionRequest,
        RuntimeI64Args, RuntimePureCallBackend, VmPureFunctionBackend,
    },
    step::RuntimePureCallStats,
    value::{RuntimeBinding, RuntimeEvalError, RuntimeValue},
};
use arcweft_lang_jit_cranelift::{CompiledPureI64Inputs, CraneliftPureFunctionBackend};
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
    pub batch_min_len: usize,
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
    pool: Option<ThreadPool>,
    resolved_workers: usize,
}

enum RuntimePureCacheEntry {
    Jit(Box<CompiledPureI64Inputs>),
    Aot(AotPureI64Plan),
    Vm,
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
        let resolved_workers = resolve_worker_count(config.workers);
        let mut cache = helper_cache_slots(helpers);
        for helper in helpers {
            cache[helper.id.0] = Some(compile_helper(config.backend, helper, &mut compile_stats));
        }
        compile_stats.compile_elapsed_ns = started.elapsed().as_nanos();
        Self {
            config,
            cache,
            stats: RuntimePureCallStats::default(),
            compile_stats,
            helper_summary,
            pool: build_thread_pool(resolved_workers),
            resolved_workers,
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
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::Jit(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += rows.len();
                call_jit_batch(compiled, rows, out, helper)
            }
            Some(RuntimePureCacheEntry::Aot(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.aot_calls += rows.len();
                if self.should_parallelize(rows.len()) {
                    self.stats.thread_pool_jobs += self.parallel_jobs(rows.len());
                    call_aot_batch_parallel(self.pool.as_ref(), compiled, rows, out)
                } else {
                    call_aot_batch(compiled, rows, out)
                }
            }
            Some(RuntimePureCacheEntry::Vm) => {
                self.compile_stats.cache_hits += 1;
                self.stats.vm_calls += rows.len();
                self.stats.fallbacks += rows.len();
                if self.should_parallelize(rows.len()) {
                    self.stats.thread_pool_jobs += self.parallel_jobs(rows.len());
                    call_vm_batch_parallel(self.pool.as_ref(), helper, rows, out)
                } else {
                    call_vm_batch(helper, rows, out)
                }
            }
            None => {
                self.compile_stats.cache_misses += 1;
                self.stats.vm_calls += rows.len();
                self.stats.fallbacks += rows.len();
                call_vm_batch(helper, rows, out)
            }
        }
    }

    fn should_parallelize(&self, len: usize) -> bool {
        self.pool.is_some() && len >= self.config.batch_min_len && self.resolved_workers > 1
    }

    fn parallel_jobs(&self, len: usize) -> usize {
        self.resolved_workers.min(len)
    }
}

impl RuntimePureCallBackend for RuntimePureAccelerator {
    fn call_i64(
        &mut self,
        helper: &RuntimePureHelper,
        args: RuntimeI64Args,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        self.stats.pure_calls += 1;
        self.stats.arg_stack_packs += 1;
        match cache_entry(&self.cache, helper.id) {
            Some(RuntimePureCacheEntry::Jit(compiled)) => {
                self.compile_stats.cache_hits += 1;
                self.stats.jit_calls += 1;
                compiled.call(args.as_slice()).map(Some).map_err(|error| {
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
                    .call_with_inputs(args.as_slice())
                    .map(|(value, _)| Some(value))
            }
            Some(RuntimePureCacheEntry::Vm) | None => {
                if cache_entry(&self.cache, helper.id).is_some() {
                    self.compile_stats.cache_hits += 1;
                } else {
                    self.compile_stats.cache_misses += 1;
                }
                self.stats.vm_calls += 1;
                self.stats.fallbacks += 1;
                Self::call_vm_i64(helper, args).map(Some)
            }
        }
    }

    fn call_values(
        &mut self,
        helper: &RuntimePureHelper,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.stats.pure_calls += 1;
        self.stats.vm_calls += 1;
        self.stats.fallbacks += 1;
        self.stats.arg_vec_allocations += 1;
        evaluate_vm(helper, args)
    }

    fn stats(&self) -> RuntimePureCallStats {
        self.stats
    }
}

impl RuntimePureAccelerator {
    fn cache_entries(&self) -> usize {
        self.cache.iter().filter(|entry| entry.is_some()).count()
    }

    fn call_vm_i64(
        helper: &RuntimePureHelper,
        args: RuntimeI64Args,
    ) -> Result<i64, RuntimeEvalError> {
        match VmPureFunctionBackend.evaluate_i64_args(helper, args)? {
            RuntimeValue::Int(value) => Ok(value),
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
        }
    }
}

fn runtime_value_kind(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Unit => "()",
        RuntimeValue::Bool(_) => "bool",
        RuntimeValue::Int(_) => "int",
        RuntimeValue::Float(_) => "float",
        RuntimeValue::String(_) => "string",
        RuntimeValue::Char(_) => "char",
        RuntimeValue::Duration(_) => "duration",
        RuntimeValue::EntityRef(_) => "entity_ref",
        RuntimeValue::Tuple(_) => "tuple",
        RuntimeValue::BracketSeq(_) => "bracket_seq",
        RuntimeValue::Record(_) => "record",
        RuntimeValue::Variant { .. } => "variant",
    }
    .to_owned()
}

fn compile_helper(
    mode: RuntimePureBackendMode,
    helper: &RuntimePureHelper,
    stats: &mut RuntimePureCompileStats,
) -> RuntimePureCacheEntry {
    match mode {
        RuntimePureBackendMode::Vm => RuntimePureCacheEntry::Vm,
        RuntimePureBackendMode::Aot => {
            compile_aot(helper, stats).unwrap_or(RuntimePureCacheEntry::Vm)
        }
        RuntimePureBackendMode::Jit => {
            compile_jit(helper, stats).unwrap_or(RuntimePureCacheEntry::Vm)
        }
        RuntimePureBackendMode::Auto => compile_jit(helper, stats)
            .or_else(|| compile_aot(helper, stats))
            .unwrap_or(RuntimePureCacheEntry::Vm),
    }
}

fn compile_jit(
    helper: &RuntimePureHelper,
    stats: &mut RuntimePureCompileStats,
) -> Option<RuntimePureCacheEntry> {
    stats.jit_attempts += 1;
    let compiled = CraneliftPureFunctionBackend
        .compile_i64_with_inputs(
            &compile_request(helper),
            helper.input_names.iter().map(String::as_str),
        )
        .ok();
    if compiled.is_some() {
        stats.jit_successes += 1;
    } else {
        stats.jit_failures += 1;
    }
    compiled.map(Box::new).map(RuntimePureCacheEntry::Jit)
}

fn compile_aot(
    helper: &RuntimePureHelper,
    stats: &mut RuntimePureCompileStats,
) -> Option<RuntimePureCacheEntry> {
    stats.aot_attempts += 1;
    let compiled = AotPureFunctionBackend::new()
        .compile_i64_with_inputs(
            &compile_request(helper),
            helper.input_names.iter().map(String::as_str),
        )
        .ok();
    if compiled.is_some() {
        stats.aot_successes += 1;
    } else {
        stats.aot_failures += 1;
    }
    compiled.map(RuntimePureCacheEntry::Aot)
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
) -> Result<(), RuntimeEvalError> {
    let arity = compiled.param_names().len();
    let mut flat_inputs = Vec::with_capacity(rows.len().saturating_mul(arity));
    for row in rows {
        flat_inputs.extend_from_slice(row.as_slice());
    }
    compiled
        .call_flat_batch(&flat_inputs, out)
        .map_err(|error| RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: error.to_string(),
        })
}

fn call_aot_batch(
    compiled: &AotPureI64Plan,
    rows: &[RuntimeI64Args],
    out: &mut [i64],
) -> Result<(), RuntimeEvalError> {
    let mut slots = Vec::new();
    rows.iter().zip(out.iter_mut()).try_for_each(|(row, slot)| {
        compiled
            .call_with_inputs_scratch(row.as_slice(), &mut slots)
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

fn call_vm_batch(
    helper: &RuntimePureHelper,
    rows: &[RuntimeI64Args],
    out: &mut [i64],
) -> Result<(), RuntimeEvalError> {
    rows.iter().zip(out.iter_mut()).try_for_each(|(row, slot)| {
        match VmPureFunctionBackend.evaluate_i64_args(helper, *row)? {
            RuntimeValue::Int(value) => {
                *slot = value;
                Ok(())
            }
            value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
        }
    })
}

fn call_vm_batch_parallel(
    pool: Option<&ThreadPool>,
    helper: &RuntimePureHelper,
    rows: &[RuntimeI64Args],
    out: &mut [i64],
) -> Result<(), RuntimeEvalError> {
    let mut run = || {
        rows.par_iter()
            .zip(out.par_iter_mut())
            .try_for_each(|(row, slot)| {
                match VmPureFunctionBackend.evaluate_i64_args(helper, *row)? {
                    RuntimeValue::Int(value) => {
                        *slot = value;
                        Ok(())
                    }
                    value => Err(RuntimeEvalError::ExpectedInt(runtime_value_kind(&value))),
                }
            })
    };
    match pool {
        Some(pool) => pool.install(run),
        None => run(),
    }
}

fn compile_request(helper: &RuntimePureHelper) -> PureFunctionRequest {
    PureFunctionRequest::new(
        helper.name.clone(),
        helper.expr.clone(),
        helper
            .input_names
            .iter()
            .cloned()
            .map(|name| RuntimeBinding {
                name,
                value: RuntimeValue::Int(0),
            }),
    )
}

fn evaluate_vm(
    helper: &RuntimePureHelper,
    args: &[RuntimeValue],
) -> Result<RuntimeValue, RuntimeEvalError> {
    if args.len() != helper.input_names.len() {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: helper.name.clone(),
            max: helper.input_names.len(),
            found: args.len(),
        });
    }
    let request = PureFunctionRequest::new(
        helper.name.clone(),
        helper.expr.clone(),
        helper
            .input_names
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .map(|(name, value)| RuntimeBinding { name, value }),
    );
    VmPureFunctionBackend
        .evaluate(&request)
        .map(|result| result.value)
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
                RuntimePureCacheEntry::Jit(_) => jit += 1,
                RuntimePureCacheEntry::Aot(_) => aot += 1,
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::{
        plan::RuntimePureHelperId,
        value::{RuntimeBinaryOp, RuntimeExpr},
    };

    #[test]
    fn auto_accelerator_calls_jit_without_value_vec_allocation() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "score".to_owned(),
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(2))),
                }),
            },
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
        assert!(accelerator.resolved_worker_count() >= 1);
        assert_eq!(accelerator.summary().jit, 1);
    }

    #[test]
    fn aot_batch_matches_scalar_results_and_records_parallel_stats() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "score".to_owned(),
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(2))),
                }),
            },
            origin: RuntimePureHelperOrigin::Inferred,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Aot,
                workers: RuntimePureWorkerCount::Fixed(2),
                batch_min_len: 2,
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
        assert!(accelerator.stats().thread_pool_jobs > 0);
    }

    #[test]
    fn vm_batch_uses_i64_args_without_value_vec_allocation() {
        let helper = RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "score".to_owned(),
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            expr: RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(2))),
                }),
            },
            origin: RuntimePureHelperOrigin::Annotated,
        };
        let mut accelerator = RuntimePureAccelerator::with_config(
            RuntimePureAcceleratorConfig {
                backend: RuntimePureBackendMode::Vm,
                workers: RuntimePureWorkerCount::Fixed(2),
                batch_min_len: 2,
            },
            std::slice::from_ref(&helper),
        );
        let rows = [
            RuntimeI64Args::new([3, 4, 0, 0], 2),
            RuntimeI64Args::new([5, 1, 0, 0], 2),
        ];
        let mut out = [0; 2];

        accelerator
            .call_i64_batch(&helper, &rows, &mut out)
            .expect("VM batch succeeds");

        assert_eq!(out, [18, 15]);
        assert_eq!(accelerator.stats().batch_calls, 1);
        assert_eq!(accelerator.stats().batch_items, 2);
        assert_eq!(accelerator.stats().vm_calls, 2);
        assert_eq!(accelerator.stats().fallbacks, 2);
        assert_eq!(accelerator.stats().arg_stack_packs, 2);
        assert_eq!(accelerator.stats().arg_vec_allocations, 0);
        assert_eq!(accelerator.stats().thread_pool_jobs, 2);
    }
}
