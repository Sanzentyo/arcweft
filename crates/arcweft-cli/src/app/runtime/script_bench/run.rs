use super::super::bench::run_runtime_bench_steps_with_pure;
use super::super::executor::RuntimeExecutorTemplate;
use super::super::expectations::{
    RuntimeExpectationView, evaluate_runtime_expectation, script_commands,
};
use super::super::invocation::seal_resolved_flow_invocation;
use super::super::options::ScriptBenchOptions;
use super::super::steps::{
    NativeRunHost, NativeRunSource, RuntimeStepRunConfig, run_runtime_flow_steps,
};
use super::BenchRuntimeContext;
use super::samples::{RuntimeBenchSamples, bench_goto_flow, validate_bench_section};
use crate::output::{
    RuntimeExecutorTier, RuntimeStepRunSummary, ScriptBenchMeasurementSummary,
    ScriptBenchRunSummary, ScriptBenchSectionRunSummary, ScriptBenchSectionStatus,
    ScriptBenchStatus,
};
use arcweft_core::engine::{FlowFiberStatus, FlowStatusLabelStyle};
use arcweft_core::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeEntryKind, RuntimeEntryTarget, RuntimePlan,
};
use arcweft_runtime_accelerator::RuntimePureAccelerator;
use arcweft_runtime_host::{NativeFileRoots, host_system_info};
use arcweft_test::{BenchSection, ScriptBench, ScriptCommand, ScriptExpectation};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Copy)]
enum BenchPhase {
    Warmup,
    Measured,
}

impl std::fmt::Display for BenchPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Warmup => "warmup",
            Self::Measured => "measured",
        })
    }
}

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
    let has_error = sections.iter().any(|section| {
        matches!(
            section.status,
            ScriptBenchSectionStatus::Unknown | ScriptBenchSectionStatus::Failed
        )
    }) || !bench
        .sections
        .iter()
        .any(|section| section.name == "measure");
    let status = if has_error {
        ScriptBenchStatus::Failed
    } else if sections
        .iter()
        .any(|section| section.status == ScriptBenchSectionStatus::Skipped)
    {
        ScriptBenchStatus::Skipped
    } else {
        ScriptBenchStatus::Validated
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
    if summary.status != ScriptBenchStatus::Validated {
        return summary;
    }
    if !options.values.is_empty()
        && bench
            .sections
            .iter()
            .any(|section| bench_pure_helper_name(section).is_some())
    {
        summary.status = ScriptBenchStatus::Failed;
        summary.diagnostics.push("--value supplies Flow inputs; pure helper measurements use their checked generated input series".to_owned());
        return summary;
    }
    if !options.values.is_empty()
        && !bench
            .sections
            .iter()
            .any(|section| bench_goto_flow(section).is_some())
    {
        summary.status = ScriptBenchStatus::Failed;
        summary
            .diagnostics
            .push("--value requires a measured goto Flow".to_owned());
        return summary;
    }
    let sections = bench
        .sections
        .iter()
        .map(|section| run_bench_section(section, plan, source_path, options, runtime))
        .collect::<Vec<_>>();
    let section_failures = sections
        .iter()
        .filter(|section| section.status == ScriptBenchSectionStatus::Failed)
        .flat_map(|section| section.diagnostics.iter().cloned())
        .collect::<Vec<_>>();
    let has_measured = sections
        .iter()
        .any(|section| section.status == ScriptBenchSectionStatus::Measured);
    if sections
        .iter()
        .any(|section| section.status == ScriptBenchSectionStatus::Failed)
    {
        summary.status = ScriptBenchStatus::Failed;
        summary.sections = sections;
        summary.diagnostics.extend(section_failures);
    } else if has_measured {
        summary.status = ScriptBenchStatus::Measured;
        summary.sections = sections;
        let diagnostics = bench_expectation_failures(bench, plan, source_path, options, runtime);
        if !diagnostics.is_empty() {
            summary.status = ScriptBenchStatus::Failed;
            summary.diagnostics.extend(diagnostics);
        }
    } else {
        summary.sections = sections;
        if summary
            .sections
            .iter()
            .any(|section| section.status == ScriptBenchSectionStatus::Skipped)
        {
            summary.status = ScriptBenchStatus::Skipped;
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
    match bench_entry_for_flow(plan, &flow) {
        Ok(_) => {}
        Err(error) => return vec![error],
    };
    let invocation =
        match seal_resolved_flow_invocation(plan.clone(), flow.clone(), &options.values) {
            Ok(invocation) => invocation,
            Err(_) => return vec!["bench assertion Flow inputs failed typed admission".to_owned()],
        };
    let frames = run_runtime_flow_steps(
        invocation,
        NativeRunHost {
            source: Some(NativeRunSource::new(source_path, runtime.file_roots)),
            bundle_assets: Some(runtime.bundle_assets),
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
    if !matches!(frames.final_status, FlowFiberStatus::Done(_)) {
        return vec![format!(
            "bench assertion execution did not complete: {}",
            frames
                .final_status
                .status_label(FlowStatusLabelStyle::Compact)
        )];
    }
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
    let commands = script_commands(&section.body)
        .into_iter()
        .filter(|command| !matches!(command, ScriptCommand::Scope { .. }))
        .collect::<Vec<_>>();
    let [command] = commands.as_slice() else {
        return Err(format!(
            "bench assert must contain exactly one typed expectation; found `{}`",
            section.text
        ));
    };
    let ScriptCommand::Expectation { expectation } = *command else {
        return Err(format!(
            "bench assert must contain exactly one typed expectation; found `{}`",
            section.text
        ));
    };
    Ok(expectation)
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
    if section.name != "measure" || validated.status != ScriptBenchSectionStatus::Validated {
        return validated;
    }
    if let Some(helper) = bench_pure_helper_name(section) {
        return super::pure::run(section, &helper, validated, options, runtime);
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
            summary.status = ScriptBenchSectionStatus::Failed;
            summary.diagnostics.push(format!(
                "bench measure `goto` target `{flow}` cannot be resolved: {error}"
            ));
            return summary;
        }
    };
    let mut samples = RuntimeBenchSamples::with_capacity(options.iterations.get());
    match bench_entry_for_flow(plan, &flow) {
        Ok(_) => {}
        Err(error) => {
            let mut summary = validated;
            summary.status = ScriptBenchSectionStatus::Failed;
            summary.diagnostics.push(error);
            return summary;
        }
    };
    let invocation =
        match seal_resolved_flow_invocation(plan.clone(), flow.clone(), &options.values) {
            Ok(invocation) => invocation,
            Err(_) => {
                return ScriptBenchSectionRunSummary::new(
                    &section.name,
                    ScriptBenchSectionStatus::Failed,
                    vec!["bench Flow inputs failed typed admission".to_owned()],
                );
            }
        };
    let pure_plan = Arc::clone(invocation.plan());
    let plain_text_context = match runtime.bundle_assets.plain_text_context_proof() {
        Ok(proof) => proof,
        Err(_) => {
            return ScriptBenchSectionRunSummary::new(
                &section.name,
                ScriptBenchSectionStatus::Failed,
                vec!["source runtime dialogue contract is invalid".to_owned()],
            );
        }
    };
    let executor_template = RuntimeExecutorTemplate::new(
        invocation,
        options.executor,
        Some(runtime.bundle_assets.bundle.product_awbc().program_lease()),
        plain_text_context,
    );
    let mut pure = RuntimePureAccelerator::with_config(runtime.pure_config, &pure_plan);
    for (phase, count) in [
        (BenchPhase::Warmup, options.warmup),
        (BenchPhase::Measured, options.iterations.get()),
    ] {
        for iteration in 0..count {
            pure.reset_runtime_counters();
            let executor = match executor_template.instantiate() {
                Ok(executor) => executor,
                Err(error) => {
                    return ScriptBenchSectionRunSummary::new(
                        &section.name,
                        ScriptBenchSectionStatus::Failed,
                        vec![format!("failed to start bench entry: {error}")],
                    );
                }
            };
            let started = Instant::now();
            let trace = run_runtime_bench_steps_with_pure(
                executor,
                Some(NativeRunSource::new(source_path, runtime.file_roots)),
                runtime.bundle_assets,
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
            let trace = match trace {
                Ok(trace) => trace,
                Err(error) => {
                    let number = iteration + 1;
                    return ScriptBenchSectionRunSummary::new(
                        &section.name,
                        ScriptBenchSectionStatus::Failed,
                        vec![format!("{phase} iteration {number} failed: {error}")],
                    );
                }
            };
            let elapsed_ns = started.elapsed().as_nanos();
            if matches!(phase, BenchPhase::Warmup) {
                continue;
            }
            samples.push(elapsed_ns, &trace);
        }
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
            iterations: options.iterations.get(),
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
