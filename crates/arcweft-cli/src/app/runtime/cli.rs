use super::entry::select_runtime_cli_entry;
use super::options::CliRunOptions;
use super::steps::{NativeRunHost, NativeRunSource, RuntimeStepRunConfig, run_runtime_steps};
use crate::app::project::{
    load_and_check_selection, native_host_policy_for_selection, require_profile_kind,
    resolve_source_selection, runtime_pure_config_for_selection,
};
use crate::app::shared::print_json;
use crate::output::{RuntimeExecutorTier, RuntimeRunReport};
use arcweft_core::engine::FlowStatusLabelStyle;
use arcweft_core::value::{RuntimeBinding, RuntimeValue, runtime_sequence_values};
use arcweft_launch::LaunchKind;
use arcweft_runtime_host::{NativeAdapterRegistrar, host_system_info};
use std::process::ExitCode;

pub(in crate::app) fn runtime_cli_command(
    options: &CliRunOptions,
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
    );
    require_profile_kind(&selection, LaunchKind::Cli, "cli")?;
    let checked = load_and_check_selection(&selection, None)?;
    let host_policy = native_host_policy_for_selection(&selection)?;
    let plan = checked.runtime_plan().plan.clone();
    let entry = selection.command_entry(options.entry.as_deref())?;
    let entry = select_runtime_cli_entry(&plan, entry)?;
    let mut bindings = options.values.clone();
    bindings.push(RuntimeBinding {
        name: "args".to_owned(),
        value: runtime_sequence_values(
            options
                .args
                .iter()
                .cloned()
                .map(RuntimeValue::String)
                .collect(),
        ),
    });
    bindings.push(RuntimeBinding {
        name: "argc".to_owned(),
        value: RuntimeValue::i64(i64::try_from(options.args.len()).unwrap_or(i64::MAX)),
    });

    let file_roots = selection.native_file_roots();
    let trace = run_runtime_steps(
        plan,
        &entry,
        NativeRunHost {
            source: Some(NativeRunSource::new(selection.path(), &file_roots)),
            policy: &host_policy,
            adapter_registrars,
        },
        RuntimeStepRunConfig {
            steps: options.steps,
            mode: options.mode,
            max_ops: options.max_ops,
            executor: options.executor,
            pure_config,
        },
        &bindings,
        &checked.execution_diagnostics,
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
        println!(
            "ok: {} ({} cli arg(s), {} step(s), final_status={})",
            selection.path().display(),
            options.args.len(),
            report.steps.len(),
            report.final_status
        );
        Ok(())
    }
}
