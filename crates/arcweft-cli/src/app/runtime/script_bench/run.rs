use super::super::bench::run_runtime_bench_steps_with_pure;
use super::super::executor::RuntimeExecutorTemplate;
use super::super::expectations::{RuntimeExpectationView, evaluate_runtime_expectation};
use super::super::options::ScriptBenchOptions;
use super::super::steps::{
    NativeRunHost, NativeRunSource, RuntimeStepRunConfig, run_runtime_steps,
};
use super::BenchRuntimeContext;
use super::samples::{RuntimeBenchSamples, bench_goto_flow, validate_bench_section};
use crate::output::{
    RuntimeExecutorTier, RuntimeStepRunSummary, ScriptBenchMeasurementSummary,
    ScriptBenchRunSummary, ScriptBenchSectionRunSummary,
};
use arcweft_core::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeEntryKind, RuntimeEntryTarget, RuntimePlan,
};
use arcweft_runtime_accelerator::RuntimePureAccelerator;
use arcweft_runtime_host::{NativeFileRoots, host_system_info};
use arcweft_test::{BenchSection, ScriptBench, ScriptCommand, ScriptExpectation};
use std::path::Path;
use std::sync::Arc;
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
        .map(|section| run_bench_section(section, plan, source_path, options, runtime))
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
    } else {
        summary.sections = sections;
        if summary
            .sections
            .iter()
            .any(|section| section.status == "skipped")
        {
            "skipped".clone_into(&mut summary.status);
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
    let flow = match plan.resolve_flow_target_value(&flow) {
        Ok(flow) => flow,
        Err(error) => {
            return vec![format!(
                "bench assertion `goto` target `{flow}` cannot be resolved: {error}"
            )];
        }
    };
    let entry = match bench_entry_for_flow(plan, &flow) {
        Ok(entry) => entry,
        Err(error) => return vec![error],
    };
    let frames = run_runtime_steps(
        plan.clone(),
        &entry,
        NativeRunHost {
            source: Some(NativeRunSource::new(source_path, runtime.file_roots)),
            policy: runtime.host_policy,
            adapter_registrars: runtime.adapter_registrars,
            cli_args: &[],
        },
        RuntimeStepRunConfig {
            steps: options.steps,
            mode: options.mode,
            max_ops: options.max_ops,
            executor: options.executor,
            pure_config: runtime.pure_config,
        },
        runtime.execution_diagnostics,
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
    match bench_assertion_expectation(section) {
        Ok(expectation) => evaluate_runtime_expectation(
            expectation,
            &RuntimeExpectationView::with_file_roots(frames, file_roots),
        )
        .err()
        .map(|failure| format!("bench assert failed: {failure}"))
        .into_iter()
        .collect(),
        Err(failure) => vec![failure],
    }
}

fn bench_assertion_expectation(section: &BenchSection) -> Result<&ScriptExpectation, String> {
    let [ScriptCommand::Expectation { expectation }] = section.body.as_slice() else {
        return Err(format!(
            "bench assert must contain exactly one typed expectation; found `{}`",
            section.text
        ));
    };
    Ok(expectation)
}

fn run_bench_pure_helper_section(
    section: &BenchSection,
    validated: ScriptBenchSectionRunSummary,
) -> ScriptBenchSectionRunSummary {
    let Some(helper_name) = bench_pure_helper_name(section) else {
        return validated;
    };
    ScriptBenchSectionRunSummary::new(
        &section.name,
        "skipped",
        vec![format!(
            "pure helper `{helper_name}` benchmark is unavailable until it can be driven by an admitted RuntimePlan helper"
        )],
    )
}

fn bench_pure_helper_name(section: &BenchSection) -> Option<String> {
    let [ScriptCommand::Pure { helper }] = section.body.as_slice() else {
        return None;
    };
    Some(helper.clone())
}

fn run_bench_section(
    section: &BenchSection,
    plan: &RuntimePlan,
    source_path: &Path,
    options: &ScriptBenchOptions,
    runtime: BenchRuntimeContext<'_>,
) -> ScriptBenchSectionRunSummary {
    let validated = validate_bench_section(section);
    if section.name != "measure" || validated.status != "validated" {
        return validated;
    }
    if bench_pure_helper_name(section).is_some() {
        return run_bench_pure_helper_section(section, validated);
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
    let flow = match plan.resolve_flow_target_value(flow) {
        Ok(flow) => flow,
        Err(error) => {
            let mut summary = validated;
            "failed".clone_into(&mut summary.status);
            summary.diagnostics.push(format!(
                "bench measure `goto` target `{flow}` cannot be resolved: {error}"
            ));
            return summary;
        }
    };
    let mut samples = RuntimeBenchSamples::with_capacity(options.iterations);
    let entry = match bench_entry_for_flow(plan, &flow) {
        Ok(entry) => entry,
        Err(error) => {
            let mut summary = validated;
            "failed".clone_into(&mut summary.status);
            summary.diagnostics.push(error);
            return summary;
        }
    };
    let executor_template = RuntimeExecutorTemplate::new(plan, entry, options.executor);
    let pure_plan = Arc::new(plan.clone());
    let mut pure = RuntimePureAccelerator::with_config(runtime.pure_config, &pure_plan);
    for iteration in 0..options.warmup + options.iterations {
        pure.reset_runtime_counters();
        let executor = match executor_template.instantiate() {
            Ok(executor) => executor,
            Err(error) => {
                return ScriptBenchSectionRunSummary::new(
                    &section.name,
                    "failed",
                    vec![format!("failed to start bench entry: {error}")],
                );
            }
        };
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

fn bench_entry_for_flow(
    plan: &RuntimePlan,
    flow: &FlowRuntimeId,
) -> Result<EntryRuntimeId, String> {
    let matching_entries = plan
        .entries()
        .iter()
        .filter(|entry| {
            entry.kind == RuntimeEntryKind::Bench
                && matches!(
                    &entry.target,
                    RuntimeEntryTarget::Flow(target) | RuntimeEntryTarget::Controller(target)
                        if target == flow
                )
        })
        .collect::<Vec<_>>();
    let [entry] = matching_entries.as_slice() else {
        return Err(format!(
            "bench target `{}` must be bound by exactly one `entry bench` declaration",
            flow.public_label()
        ));
    };
    Ok(entry.id.clone())
}
