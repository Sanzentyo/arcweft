use super::entry::apply_runtime_entry_selection;
use super::options::{RuntimeRunOptions, ScriptBenchOptions};
use super::script_bench::script_bench_selection;
use super::script_test::script_test_selection;
use super::serve::{RuntimeServeSelectionConfig, runtime_serve_selection};
use super::steps::{run_runtime_steps, runtime_step_run_config_from_run_options};
use crate::app::project::ProfileOptions;
use crate::app::project::{
    SourceSelection, load_and_check_selection, native_host_policy_for_selection,
    resolve_source_selection, runtime_plan_options_for_selection,
    runtime_pure_config_for_selection,
};
use crate::app::shared::print_json;
use crate::output::{RuntimeExecutorTier, RuntimeRunReport};
use arcweft_compiler::lower::lower_source_runtime_plan_with_options;
use arcweft_core::engine::FlowStatusLabelStyle;
use arcweft_launch::LaunchKind;
use arcweft_runtime_host::{NativeAdapterRegistrar, host_system_info};
use std::process::ExitCode;

pub(in crate::app) fn runtime_run_command(
    options: &RuntimeRunOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let pure_config = runtime_pure_config_for_selection(
        &selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.pure_object_artifacts,
        options.math_backend,
        options.math_wgpu_min_elements,
    )?;
    if let Some(profile) = selection.profile() {
        match profile.kind() {
            LaunchKind::Server => {
                return runtime_serve_selection(
                    &selection,
                    options.entry.as_deref(),
                    None,
                    RuntimeServeSelectionConfig {
                        listen: None,
                        once: false,
                        max_ops: options.max_ops,
                        pure_config,
                        json: options.json,
                    },
                    adapter_registrars,
                );
            }
            LaunchKind::Test => {
                return script_test_selection(
                    &selection,
                    runtime_step_run_config_from_run_options(options, pure_config),
                    adapter_registrars,
                    &options.values,
                    options.json,
                );
            }
            LaunchKind::Bench => {
                return runtime_run_bench_selection(&selection, options, adapter_registrars);
            }
            LaunchKind::Game | LaunchKind::Cli => {}
        }
    }

    let checked = load_and_check_selection(&selection, None)?;
    let host_policy = native_host_policy_for_selection(&selection)?;
    let runtime_options = runtime_plan_options_for_selection(&selection);
    let mut plan = lower_source_runtime_plan_with_options(&checked.hir, &runtime_options).map_err(
        |errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        },
    )?;
    let entry = options.entry.as_deref().or(selection.entry());
    apply_runtime_entry_selection(&mut plan, entry, options.flow.as_deref())?;
    let trace = run_runtime_steps(
        plan,
        Some(selection.path()),
        runtime_step_run_config_from_run_options(options, pure_config),
        &host_policy,
        adapter_registrars,
        &options.values,
    )?;
    let report = RuntimeRunReport {
        host_system: host_system_info(),
        executor: RuntimeExecutorTier::from(options.executor),
        executor_stats: trace.executor_stats,
        native_io: trace.native_io,
        steps: trace.steps,
        final_status: trace.final_status.status_label(FlowStatusLabelStyle::Debug),
    };
    if options.json {
        print_json(&report)
    } else {
        for step in &report.steps {
            println!(
                "step {}: {} flow event(s), {} effect(s), {} task request(s), {} diagnostic(s)",
                step.index,
                step.flow_events.len(),
                step.line_effects.len(),
                step.task_requests.len(),
                step.diagnostics.len()
            );
            for event in &step.flow_events {
                println!("  event {event}");
            }
            for effect in &step.line_effects {
                println!("  effect {effect}");
            }
        }
        println!(
            "ok: {} ({} step(s), final_status={})",
            selection.path().display(),
            report.steps.len(),
            report.final_status
        );
        Ok(())
    }
}

fn runtime_run_bench_selection(
    selection: &SourceSelection,
    options: &RuntimeRunOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let bench_options = ScriptBenchOptions {
        path: None,
        profile: ProfileOptions::default(),
        steps: options.steps,
        mode: options.mode,
        max_ops: options.max_ops,
        iterations: 1,
        warmup: 0,
        samples: 5,
        input_seed: 0,
        executor: options.executor,
        pure_backend: options.pure_backend,
        pure_workers: options.pure_workers,
        pure_batch_min_len: options.pure_batch_min_len,
        pure_object_artifacts: options.pure_object_artifacts,
        math_backend: options.math_backend,
        math_wgpu_min_elements: options.math_wgpu_min_elements,
        values: options.values.clone(),
        json: options.json,
    };
    script_bench_selection(selection, &bench_options, adapter_registrars)
}
