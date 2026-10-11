//! Measured adapters for one compiler-owned checked function root.
//! All execution stays on Core/AOT/JIT/accelerator boundaries.

use super::super::options::ScriptBenchOptions;
use super::BenchRuntimeContext;
use crate::app::jit::{
    JitBuiltinCase, JitCheckOptions, JitCheckTarget, PureMeasurementError, jit_check_input_array,
    per_iteration_ns, run_jit_check, speedup_x, timing_samples,
};
use crate::output::{
    RuntimePureCallStatsSummary, ScriptBenchPureHelperMeasurementSummary,
    ScriptBenchPureHelperRuntimeBatchSummary, ScriptBenchPureHelperTimingSamples,
    ScriptBenchSectionRunSummary,
};
use arcweft_core::pure::RuntimePureCallBackend;
use arcweft_core::step::RuntimeStepBudget;
use arcweft_runtime_accelerator::RuntimePureAccelerator;
use arcweft_test::BenchSection;
use std::time::Instant;

#[derive(Debug, thiserror::Error)]
enum PureBenchMeasurementError {
    #[error(transparent)]
    Selection(#[from] arcweft_compiler::project::DeterministicFunctionCompileError),
    #[error(transparent)]
    Measurement(#[from] PureMeasurementError),
}

pub(super) fn run(
    section: &BenchSection,
    helper: &str,
    validated: ScriptBenchSectionRunSummary,
    options: &ScriptBenchOptions,
    runtime: BenchRuntimeContext<'_>,
) -> ScriptBenchSectionRunSummary {
    match measure(helper, options, runtime) {
        Ok(measurement) => ScriptBenchSectionRunSummary::measured_pure(
            &section.name,
            validated.diagnostics,
            measurement,
        ),
        Err(error) => {
            let mut result = validated;
            "failed".clone_into(&mut result.status);
            result
                .diagnostics
                .push(format!("pure helper {helper} measurement failed: {error}"));
            result
        }
    }
}

fn measure(
    helper: &str,
    options: &ScriptBenchOptions,
    runtime: BenchRuntimeContext<'_>,
) -> Result<ScriptBenchPureHelperMeasurementSummary, PureBenchMeasurementError> {
    let selected = runtime.analysis.compile_deterministic_function(
        Some(helper),
        &arcweft_compiler::lower::ProjectInstantiationControl::default(),
    )?;
    let target = JitCheckTarget::from_program(
        selected.name().to_owned(),
        selected.program(),
        selected.input_labels().to_vec(),
        None,
    )?;
    let measurement_options = JitCheckOptions {
        path: None,
        helper: Some(helper.to_owned()),
        case: JitBuiltinCase::Score,
        julia: false,
        iterations: options.iterations,
        warmup: options.warmup,
        samples: options.samples,
        input_seed: options.input_seed,
        vm_max_ops: options.max_ops,
        json: false,
    };
    let report = run_jit_check(&measurement_options, &target)?;
    if !report.matches_vm {
        return Err(PureMeasurementError::ConformanceMismatch.into());
    }
    let runtime_batch = measure_runtime_batch(
        &target,
        options,
        runtime,
        report.timings.vm,
        report.deterministic.vm,
    )?;
    if !runtime_batch.matches_vm {
        return Err(PureMeasurementError::ConformanceMismatch.into());
    }
    let mut result = ScriptBenchPureHelperMeasurementSummary::from(&report);
    result.runtime_batch = Some(runtime_batch);
    Ok(result)
}

fn measure_runtime_batch(
    target: &JitCheckTarget,
    options: &ScriptBenchOptions,
    runtime: BenchRuntimeContext<'_>,
    vm_elapsed_ns: u128,
    vm_accumulator: i64,
) -> Result<ScriptBenchPureHelperRuntimeBatchSummary, PureMeasurementError> {
    let arity = target.inputs().len();
    let input_count =
        options
            .iterations
            .checked_mul(arity)
            .ok_or(PureMeasurementError::InvalidWorkload(
                "flat input series size overflow",
            ))?;
    let mut accelerator = RuntimePureAccelerator::with_config(runtime.pure_config, target.plan());
    let function = target.helper();
    let budget = RuntimeStepBudget {
        max_ops: options.max_ops,
    };
    for iteration in 0..options.warmup {
        let input = jit_check_input_array(options.input_seed, 0, iteration, arity);
        let mut out = [0_i64; 1];
        accelerator.call_i64_flat_batch_invocations(
            function,
            &input[..arity],
            arity,
            &mut out,
            budget,
        )?;
    }
    accelerator.reset_runtime_counters();
    let mut inputs = Vec::with_capacity(input_count);
    let mut out = vec![0_i64; options.iterations];
    let mut elapsed = Vec::with_capacity(options.samples);
    let mut accumulator = 0_i64;
    for sample in 0..options.samples {
        inputs.clear();
        for iteration in 0..options.iterations {
            let row = jit_check_input_array(options.input_seed, sample, iteration, arity);
            inputs.extend_from_slice(&row[..arity]);
        }
        let started = Instant::now();
        accelerator.call_i64_flat_batch_invocations(function, &inputs, arity, &mut out, budget)?;
        accumulator = out
            .iter()
            .fold(accumulator, |sum, value| sum.saturating_add(*value));
        elapsed.push(started.elapsed().as_nanos());
    }
    let samples = timing_samples(elapsed);
    let executor = arcweft_runtime_host::runtime_executor_stats(0, &accelerator);
    Ok(ScriptBenchPureHelperRuntimeBatchSummary {
        matches_vm: accumulator == vm_accumulator,
        accumulator,
        elapsed_ns: samples.median,
        per_iteration_ns: per_iteration_ns(samples.median, options.iterations),
        speedup_x: speedup_x(vm_elapsed_ns, samples.median),
        samples: ScriptBenchPureHelperTimingSamples::from(samples),
        config: executor.pure_config,
        compile: executor.pure_compile,
        stats: RuntimePureCallStatsSummary::from(accelerator.stats()),
    })
}
