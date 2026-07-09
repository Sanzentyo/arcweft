use super::super::bench::run_runtime_bench_steps_with_pure;
use super::super::executor::RuntimeExecutorTemplate;
use super::super::expectations::{RuntimeExpectationView, evaluate_runtime_expectation};
use super::super::options::ScriptBenchOptions;
use super::super::steps::{NativeRunSource, RuntimeStepRunConfig, run_runtime_steps};
use super::BenchRuntimeContext;
use super::samples::{RuntimeBenchSamples, bench_goto_flow, validate_bench_section};
use crate::app::jit::{
    JitBuiltinCase, JitCheckOptions, JitCheckReport, JitCheckTarget, jit_check_input_array,
    per_iteration_ns, run_jit_check, speedup_x, timing_samples,
};
use crate::output::{
    RuntimeExecutorTier, RuntimePureCallStatsSummary, RuntimeStepRunSummary,
    ScriptBenchMeasurementSummary, ScriptBenchPureHelperMeasurementSummary,
    ScriptBenchPureHelperRuntimeBatchSummary, ScriptBenchPureHelperTimingSamples,
    ScriptBenchRunSummary, ScriptBenchSectionRunSummary,
};
use arcweft_core::plan::{
    FlowRuntimeId, RuntimePlan, RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin,
    RuntimePureInputType, RuntimePureOutputType,
};
use arcweft_core::pure::RuntimePureCallBackend;
use arcweft_lang_syntax::expr::{Expr, parse_expr};
use arcweft_runtime_accelerator::{RuntimePureAccelerator, RuntimePureAcceleratorConfig};
use arcweft_runtime_host::{NativeFileRoots, host_system_info, runtime_executor_stats};
use arcweft_runtime_plan::pure::{PureHelperCandidate, PureHelperLowerError};
use arcweft_test::{BenchSection, ScriptBench};
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

fn validate_script_bench(bench: &ScriptBench) -> ScriptBenchRunSummary {
    let sections = bench
        .sections
        .iter()
        .map(validate_bench_section)
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    if !bench
        .sections
        .iter()
        .any(|section| section.name == "measure")
    {
        diagnostics.push("bench requires a `measure` section".to_owned());
    }
    diagnostics.extend(
        sections
            .iter()
            .flat_map(|section| section.diagnostics.iter().cloned()),
    );
    let has_error = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("unknown bench section"))
        || !bench
            .sections
            .iter()
            .any(|section| section.name == "measure");
    let has_unsupported = sections
        .iter()
        .any(|section| section.status == "unsupported");
    let status = if has_error {
        "failed"
    } else if has_unsupported {
        "skipped"
    } else {
        "validated"
    };
    ScriptBenchRunSummary::new(bench, status, sections, diagnostics)
}

pub(in crate::app) fn run_script_bench(
    bench: &ScriptBench,
    plan: &RuntimePlan,
    pure_helpers: &Result<Vec<PureHelperCandidate>, Vec<PureHelperLowerError>>,
    source_path: &Path,
    options: &ScriptBenchOptions,
    runtime: BenchRuntimeContext<'_>,
) -> ScriptBenchRunSummary {
    let mut summary = validate_script_bench(bench);
    if summary.status != "validated" {
        return summary;
    }
    let sections = bench
        .sections
        .iter()
        .map(|section| {
            run_bench_section(section, plan, pure_helpers, source_path, options, runtime)
        })
        .collect::<Vec<_>>();
    let section_failures = sections
        .iter()
        .filter(|section| section.status == "failed")
        .flat_map(|section| section.diagnostics.iter().cloned())
        .collect::<Vec<_>>();
    let has_measured = sections.iter().any(|section| section.status == "measured");
    if !section_failures.is_empty() {
        "failed".clone_into(&mut summary.status);
        summary.sections = sections;
        summary.diagnostics.extend(section_failures);
    } else if has_measured {
        "measured".clone_into(&mut summary.status);
        summary.sections = sections;
        let diagnostics = bench_expectation_failures(bench, plan, source_path, options, runtime);
        if !diagnostics.is_empty() {
            "failed".clone_into(&mut summary.status);
            summary.diagnostics.extend(diagnostics);
        }
    }
    summary
}

fn bench_expectation_failures(
    bench: &ScriptBench,
    plan: &RuntimePlan,
    source_path: &Path,
    options: &ScriptBenchOptions,
    runtime: BenchRuntimeContext<'_>,
) -> Vec<String> {
    let assertions = bench
        .sections
        .iter()
        .filter(|section| section.name == "assert")
        .collect::<Vec<_>>();
    if assertions.is_empty() {
        return Vec::new();
    }
    let Some(flow) = bench.sections.iter().find_map(bench_goto_flow) else {
        return vec![
            "bench assertions require a runnable `measure { goto @flow.id }` section".to_owned(),
        ];
    };
    let Ok(flow) = FlowRuntimeId::from_runtime_target_value(&flow) else {
        return vec![format!(
            "bench assertion `goto` target `{flow}` is not a valid flow runtime ID"
        )];
    };
    let mut assertion_plan = plan.clone();
    assertion_plan.entry_flow = Some(flow);
    let frames = run_runtime_steps(
        assertion_plan,
        Some(NativeRunSource::new(source_path, runtime.file_roots)),
        RuntimeStepRunConfig {
            steps: options.steps,
            mode: options.mode,
            max_ops: options.max_ops,
            executor: options.executor,
            pure_config: runtime.pure_config,
        },
        runtime.host_policy,
        runtime.adapter_registrars,
        &options.values,
    );
    let Ok(frames) = frames else {
        return vec!["native adapter registration failed".to_owned()];
    };
    assertions
        .into_iter()
        .flat_map(|section| bench_assertion_failures(section, &frames.steps, runtime.file_roots))
        .collect()
}

fn bench_assertion_failures(
    section: &BenchSection,
    frames: &[RuntimeStepRunSummary],
    file_roots: &NativeFileRoots,
) -> Vec<String> {
    match bench_assertion_text(section) {
        Ok(text) => evaluate_runtime_expectation(
            text,
            &RuntimeExpectationView::with_file_roots(frames, file_roots),
        )
        .err()
        .map(|failure| format!("bench assert failed: {failure}"))
        .into_iter()
        .collect(),
        Err(failure) => vec![failure],
    }
}

fn bench_assertion_text(section: &BenchSection) -> Result<&str, String> {
    let rest = section
        .text
        .trim()
        .strip_prefix("assert")
        .map(str::trim_start)
        .ok_or_else(|| format!("invalid assert section `{}`", section.text))?;
    let Some(body) = rest
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    else {
        return Err("bench assert must use `assert { expect.*(...) }`".to_owned());
    };
    let body = body.trim();
    if body.is_empty() {
        return Err("bench assert body must contain an expectation call".to_owned());
    }
    Ok(body)
}

fn run_bench_pure_helper_section(
    section: &BenchSection,
    pure_helpers: &Result<Vec<PureHelperCandidate>, Vec<PureHelperLowerError>>,
    options: &ScriptBenchOptions,
    pure_config: RuntimePureAcceleratorConfig,
    validated: ScriptBenchSectionRunSummary,
) -> ScriptBenchSectionRunSummary {
    let Some(helper_name) = bench_pure_helper_name(section) else {
        return validated;
    };
    let candidates = match pure_helpers {
        Ok(candidates) => candidates,
        Err(errors) => {
            return ScriptBenchSectionRunSummary::new(
                &section.name,
                "failed",
                errors
                    .iter()
                    .map(|error| format!("pure helper lowering failed: {error}"))
                    .collect(),
            );
        }
    };
    let Some(candidate) = candidates
        .iter()
        .find(|candidate| candidate.name() == helper_name)
    else {
        return ScriptBenchSectionRunSummary::new(
            &section.name,
            "failed",
            vec![format!("pure helper `{helper_name}` was not found")],
        );
    };
    let target = match JitCheckTarget::from_candidate(candidate, None) {
        Ok(target) => target,
        Err(code) => {
            return ScriptBenchSectionRunSummary::new(
                &section.name,
                "failed",
                vec![format!(
                    "pure helper `{helper_name}` cannot be measured by the current JIT tier: exit code {code:?}"
                )],
            );
        }
    };
    match script_bench_pure_helper_summary(helper_name, &target, options, pure_config) {
        Ok(summary) => ScriptBenchSectionRunSummary::measured_pure_helper(
            &section.name,
            validated.diagnostics,
            summary,
        ),
        Err(message) => ScriptBenchSectionRunSummary::new(&section.name, "failed", vec![message]),
    }
}

fn script_bench_pure_helper_summary(
    helper_name: String,
    target: &JitCheckTarget,
    options: &ScriptBenchOptions,
    pure_config: RuntimePureAcceleratorConfig,
) -> Result<ScriptBenchPureHelperMeasurementSummary, String> {
    let jit_options = JitCheckOptions {
        path: None,
        helper: Some(helper_name),
        case: JitBuiltinCase::Score,
        julia: false,
        iterations: options.iterations,
        warmup: options.warmup,
        samples: options.samples,
        input_seed: options.input_seed,
        json: false,
    };
    let report = run_jit_check(&jit_options, target).map_err(|code| {
        format!(
            "pure helper `{}` failed during VM/AOT/JIT measurement: exit code {code:?}",
            target.name
        )
    })?;
    if !report.matches_vm {
        return Err(format!(
            "pure helper `{}` did not match the VM reference",
            report.helper
        ));
    }
    let runtime_batch = measure_script_bench_runtime_pure_batch(target, &report, options, pure_config)
        .map_err(|code| {
            format!(
                "pure helper `{}` failed during runtime accelerator batch measurement: exit code {code:?}",
                target.name
            )
        })?;
    if !runtime_batch.matches_vm {
        return Err(format!(
            "pure helper `{}` runtime accelerator batch did not match the VM reference",
            target.name
        ));
    }
    let mut summary = ScriptBenchPureHelperMeasurementSummary::from(&report);
    summary.runtime_batch = Some(runtime_batch);
    Ok(summary)
}

fn measure_script_bench_runtime_pure_batch(
    target: &JitCheckTarget,
    report: &JitCheckReport,
    options: &ScriptBenchOptions,
    pure_config: RuntimePureAcceleratorConfig,
) -> Result<ScriptBenchPureHelperRuntimeBatchSummary, ExitCode> {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: target.name.clone(),
        input_names: target.input_names.clone(),
        input_types: vec![RuntimePureInputType::I64; target.input_names.len()],
        output_type: RuntimePureOutputType::I64,
        expr: target.expr.clone(),
        scalar_eval_supported: target.expr.supports_scalar_pure_eval(),
        origin: RuntimePureHelperOrigin::Annotated,
    };
    if options.warmup > 0 {
        let mut warmup_accelerator =
            RuntimePureAccelerator::with_config(pure_config, std::slice::from_ref(&helper));
        let mut rows = Vec::with_capacity(options.warmup.saturating_mul(target.input_names.len()));
        fill_runtime_flat_batch_inputs(&mut rows, target, options.input_seed, 0, options.warmup);
        let mut out = vec![0_i64; options.warmup];
        warmup_accelerator
            .call_i64_flat_batch(&helper, &rows, target.input_names.len(), &mut out)
            .map_err(|error| {
                eprintln!("error: runtime pure batch warmup failed: {error}");
                ExitCode::FAILURE
            })?;
    }

    let mut accelerator =
        RuntimePureAccelerator::with_config(pure_config, std::slice::from_ref(&helper));
    let mut elapsed = Vec::with_capacity(options.samples);
    let mut rows = Vec::with_capacity(options.iterations.saturating_mul(target.input_names.len()));
    let mut out = vec![0_i64; options.iterations];
    let mut accumulator = 0_i64;
    for sample in 0..options.samples {
        fill_runtime_flat_batch_inputs(
            &mut rows,
            target,
            options.input_seed,
            sample,
            options.iterations,
        );
        out.fill(0);
        let started = Instant::now();
        accelerator
            .call_i64_flat_batch(&helper, &rows, target.input_names.len(), &mut out)
            .map_err(|error| {
                eprintln!("error: runtime pure batch measurement failed: {error}");
                ExitCode::FAILURE
            })?;
        elapsed.push(started.elapsed().as_nanos());
        accumulator = out.iter().copied().fold(accumulator, i64::saturating_add);
    }
    let samples = timing_samples(elapsed);
    let executor_stats = runtime_executor_stats(0, &accelerator);
    Ok(ScriptBenchPureHelperRuntimeBatchSummary {
        matches_vm: accumulator == report.deterministic.vm,
        accumulator,
        elapsed_ns: samples.median,
        per_iteration_ns: per_iteration_ns(samples.median, options.iterations),
        speedup_x: speedup_x(report.timings.vm, samples.median),
        samples: ScriptBenchPureHelperTimingSamples::from(samples),
        config: executor_stats.pure_config,
        compile: executor_stats.pure_compile,
        stats: RuntimePureCallStatsSummary::from(accelerator.stats()),
    })
}

fn fill_runtime_flat_batch_inputs(
    inputs: &mut Vec<i64>,
    target: &JitCheckTarget,
    input_seed: u64,
    sample: usize,
    iterations: usize,
) {
    let arity = target.input_names.len();
    inputs.clear();
    inputs.reserve(iterations.saturating_mul(arity));
    for iteration in 0..iterations {
        inputs.extend_from_slice(
            &jit_check_input_array(input_seed, sample, iteration, arity)[..arity],
        );
    }
}

fn bench_pure_helper_name(section: &BenchSection) -> Option<String> {
    let Expr::Call { callee, args } = parse_expr(bench_measure_body(section)?).ok()? else {
        return None;
    };
    let Expr::Path(callee) = callee.as_ref() else {
        return None;
    };
    if callee != "pure" {
        return None;
    }
    let [helper] = args.as_slice() else {
        return None;
    };
    match helper.value() {
        Expr::Path(name) => Some(name.as_label().to_owned()),
        _ => None,
    }
}

fn bench_measure_body(section: &BenchSection) -> Option<&str> {
    let rest = section.text.trim().strip_prefix("measure")?;
    let open = rest.find('{')?;
    rest[open + 1..].strip_suffix('}').map(str::trim)
}

fn run_bench_section(
    section: &BenchSection,
    plan: &RuntimePlan,
    pure_helpers: &Result<Vec<PureHelperCandidate>, Vec<PureHelperLowerError>>,
    source_path: &Path,
    options: &ScriptBenchOptions,
    runtime: BenchRuntimeContext<'_>,
) -> ScriptBenchSectionRunSummary {
    let validated = validate_bench_section(section);
    if section.name != "measure" || validated.status != "validated" {
        return validated;
    }
    if bench_pure_helper_name(section).is_some() {
        return run_bench_pure_helper_section(
            section,
            pure_helpers,
            options,
            runtime.pure_config,
            validated,
        );
    }
    let Some(flow) = bench_goto_flow(section) else {
        return validated;
    };
    run_bench_flow_section(
        section,
        plan,
        &flow,
        source_path,
        options,
        runtime,
        validated,
    )
}

fn run_bench_flow_section(
    section: &BenchSection,
    plan: &RuntimePlan,
    flow: &str,
    source_path: &Path,
    options: &ScriptBenchOptions,
    runtime: BenchRuntimeContext<'_>,
    validated: ScriptBenchSectionRunSummary,
) -> ScriptBenchSectionRunSummary {
    let Ok(flow) = FlowRuntimeId::from_runtime_target_value(flow) else {
        let mut summary = validated;
        "failed".clone_into(&mut summary.status);
        summary.diagnostics.push(format!(
            "bench measure `goto` target `{flow}` is not a valid flow runtime ID"
        ));
        return summary;
    };
    let mut samples = RuntimeBenchSamples::with_capacity(options.iterations);
    let mut selected_plan = plan.clone();
    selected_plan.entry_flow = Some(flow);
    let executor_template = RuntimeExecutorTemplate::new(&selected_plan, options.executor);
    let mut pure =
        RuntimePureAccelerator::with_config(runtime.pure_config, &selected_plan.pure_helpers);
    for iteration in 0..options.warmup + options.iterations {
        pure.reset_runtime_counters();
        let executor = executor_template.instantiate();
        let started = Instant::now();
        let trace = run_runtime_bench_steps_with_pure(
            executor,
            Some(NativeRunSource::new(source_path, runtime.file_roots)),
            RuntimeStepRunConfig {
                steps: options.steps,
                mode: options.mode,
                max_ops: options.max_ops,
                executor: options.executor,
                pure_config: runtime.pure_config,
            },
            runtime.host_policy,
            runtime.adapter_registrars,
            &options.values,
            &mut pure,
        );
        let Ok(trace) = trace else {
            return ScriptBenchSectionRunSummary::new(
                &section.name,
                "failed",
                vec!["native adapter registration failed".to_owned()],
            );
        };
        let elapsed_ns = started.elapsed().as_nanos();
        if iteration < options.warmup {
            continue;
        }
        samples.push(elapsed_ns, &trace);
    }
    ScriptBenchSectionRunSummary::measured(
        &section.name,
        validated.diagnostics,
        ScriptBenchMeasurementSummary {
            host_system: host_system_info(),
            executor: RuntimeExecutorTier::from(options.executor),
            executor_stats: samples.executor_stats(),
            native_io: samples.native_io_median(),
            warmup: options.warmup,
            iterations: options.iterations,
            steps: options.steps,
            per_executed_op_ns: samples.per_executed_op_ns(),
            elapsed_ns: samples.elapsed_summary(),
            deterministic: samples.deterministic_summary(),
        },
    )
}
