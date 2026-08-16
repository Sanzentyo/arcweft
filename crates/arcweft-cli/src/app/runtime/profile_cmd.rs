use super::entry::select_runtime_entry;
use super::executor::RuntimeExecutorInstance;
use super::options::RuntimeProfileOptions;
use super::profile::{compile_profile_runtime_plan, report_path, run_profile_phase};
use super::steps::{NativeRunHost, NativeRunSource, run_runtime_steps_with_executor};
use crate::app::project::{
    native_host_policy_for_selection_with_adapter, resolve_source_selection,
    runtime_pure_config_for_selection, semantic_context_for_selection,
};
use crate::app::shared::{is_arcw_path, print_json};
use crate::output::{
    AotProfileStats, AwbcProfileStats, FinalSemanticProfileStats, RuntimeExecutorTier,
    RuntimePlanProfileStats, RuntimeProfileCompiler, RuntimeProfileReport, RuntimeProfileRuntime,
};
use arcweft_core::engine::FlowStatusLabelStyle;
use arcweft_runtime_host::{NativeAdapterRegistrar, host_system_info};
use std::process::ExitCode;

pub(in crate::app) fn runtime_profile_command(
    options: &RuntimeProfileOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let mut phases = Vec::new();
    let pure_config = runtime_pure_config_for_selection(
        &selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.pure_object_artifacts,
        options.math_backend,
        options.math_wgpu_min_elements,
    );
    let semantic = semantic_context_for_selection(&selection, options.adapter.as_deref())?;
    let host_policy =
        native_host_policy_for_selection_with_adapter(&selection, options.adapter.as_deref())?;
    if !is_arcw_path(selection.path()) {
        eprintln!(
            "error: {} is not an .arcw source file",
            selection.path().display()
        );
        return Err(ExitCode::from(2));
    }

    let compiled = compile_profile_runtime_plan(&selection, &semantic, &mut phases)?;
    let execution_diagnostics = compiled.execution_diagnostics.clone();
    let plan = compiled.plan;
    let entry = selection.command_entry(options.entry.as_deref())?;
    let entry = select_runtime_entry(&plan, entry)?;
    let mut executor = run_profile_phase(&mut phases, "executor_prepare", || {
        RuntimeExecutorInstance::new(plan, &entry, options.executor, pure_config).map_err(|error| {
            eprintln!(
                "error: failed to start entry `{}`: {error}",
                entry.public_label()
            );
            ExitCode::FAILURE
        })
    })?;
    let file_roots = selection.native_file_roots();
    let trace = run_profile_phase(&mut phases, "run", || {
        run_runtime_steps_with_executor(
            &mut executor,
            NativeRunHost {
                source: Some(NativeRunSource::new(selection.path(), &file_roots)),
                policy: &host_policy,
                adapter_registrars,
            },
            options.steps,
            options.mode,
            options.max_ops,
            &options.values,
            &execution_diagnostics,
        )
    })?;
    let final_status = trace.final_status.status_label(FlowStatusLabelStyle::Debug);
    let report = RuntimeProfileReport {
        source: report_path(selection.path()),
        syntax_warnings: compiled.syntax_warnings,
        line_task_groups: compiled.line_task_groups,
        compiler: RuntimeProfileCompiler {
            syntax: compiled.syntax_stats.into(),
            semantic: FinalSemanticProfileStats::from(compiled.compiled.final_analysis().as_ref()),
            runtime_plan: RuntimePlanProfileStats::from(compiled.runtime_plan_stats),
            awbc: AwbcProfileStats::from(&compiled.product_awbc),
            aot: AotProfileStats::from(&compiled.aot_stats),
        },
        phases,
        runtime: RuntimeProfileRuntime {
            host_system: host_system_info(),
            executor: RuntimeExecutorTier::from(options.executor),
            executor_stats: trace.executor_stats,
            native_io: trace.native_io,
            steps: trace.steps,
            final_status,
        },
    };
    if options.json {
        print_json(&report)
    } else {
        println!(
            "ok: {} ({} phase(s), {} step(s), final_status={})",
            report.source,
            report.phases.len(),
            report.runtime.steps.len(),
            report.runtime.final_status
        );
        for phase in &report.phases {
            println!("  phase {} {}ns", phase.name, phase.elapsed_ns);
        }
        Ok(())
    }
}
