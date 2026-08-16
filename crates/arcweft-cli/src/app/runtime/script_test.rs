use super::expectations::{test_expectation_failures, test_goto_flow};
use super::options::ScriptTestOptions;
use super::steps::{NativeRunHost, NativeRunSource, RuntimeStepRunConfig, run_runtime_steps};
use crate::app::project::{
    SourceSelection, load_and_check_selection, native_host_policy_for_selection,
    require_profile_kind, resolve_source_selection, runtime_pure_config_for_selection,
};
use crate::app::shared::print_json;
use crate::output::{
    ScriptTestFinalStatus, ScriptTestRunReport, ScriptTestRunSummary, ScriptTestStatus,
};
use arcweft_core::engine::{FlowFiberStatus, FlowStatusLabelStyle};
use arcweft_core::plan::{RuntimeEntryKind, RuntimeEntryTarget, RuntimePlan};
use arcweft_core::value::RuntimeBinding;
use arcweft_launch::LaunchKind;
use arcweft_runtime_host::NativeAdapterRegistrar;
use arcweft_test::{ScriptTest, collect_script_tests};
use std::process::ExitCode;

pub(in crate::app) fn script_test_command(
    options: &ScriptTestOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    require_profile_kind(&selection, LaunchKind::Test, "test")?;
    let pure_config = runtime_pure_config_for_selection(
        &selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.pure_object_artifacts,
        options.math_backend,
        options.math_wgpu_min_elements,
    );
    script_test_selection(
        &selection,
        RuntimeStepRunConfig {
            steps: options.steps,
            mode: options.mode,
            max_ops: options.max_ops,
            executor: options.executor,
            pure_config,
        },
        adapter_registrars,
        &options.values,
        options.json,
    )
}

pub(in crate::app) fn script_test_selection(
    selection: &SourceSelection,
    config: RuntimeStepRunConfig,
    adapter_registrars: &[NativeAdapterRegistrar],
    values: &[RuntimeBinding],
    json: bool,
) -> Result<(), ExitCode> {
    let checked = load_and_check_selection(selection, None)?;
    let host_policy = native_host_policy_for_selection(selection)?;
    let manifest = collect_script_tests(checked.compiled.hir_project());
    let plan = checked.runtime_plan().plan.clone();
    let file_roots = selection.native_file_roots();
    let source = NativeRunSource::new(selection.path(), &file_roots);
    let output = ScriptTestRunReport {
        tests: manifest
            .tests
            .iter()
            .map(|test| {
                run_script_test(
                    test,
                    &plan,
                    NativeRunHost {
                        source: Some(source),
                        policy: &host_policy,
                        adapter_registrars,
                    },
                    config,
                    values,
                    &checked.execution_diagnostics,
                )
            })
            .collect(),
    };
    let failed = output
        .tests
        .iter()
        .any(|test| test.status == ScriptTestStatus::Failed);
    if json {
        print_json(&output)?;
    } else {
        for test in &output.tests {
            println!(
                "{} {} {} ({} step(s))",
                test.id, test.kind, test.status, test.steps_run
            );
            for diagnostic in &test.diagnostics {
                println!("  diagnostic {diagnostic}");
            }
        }
        println!(
            "ok: {} ({} script test(s))",
            selection.path().display(),
            output.tests.len()
        );
    }
    if failed {
        Err(ExitCode::FAILURE)
    } else {
        Ok(())
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one scenario test must retain its setup, execution, and final-status evidence together"
)]
fn run_script_test(
    test: &ScriptTest,
    plan: &RuntimePlan,
    host_config: NativeRunHost<'_>,
    config: RuntimeStepRunConfig,
    values: &[RuntimeBinding],
    execution_diagnostics: &arcweft_compiler::runtime_diagnostics::ExecutionDiagnosticContext,
) -> ScriptTestRunSummary {
    if test.kind != "scenario" {
        return ScriptTestRunSummary::skipped(
            test,
            format!(
                "headless execution for `{}` tests is not implemented",
                test.kind
            ),
        );
    }
    let Some(start) = test_goto_flow(test) else {
        return ScriptTestRunSummary::completed(
            test,
            false,
            ScriptTestFinalStatus::NotStarted,
            vec!["scenario test requires `goto @flow.id`".to_owned()],
            Vec::new(),
        );
    };
    let start = match plan.resolve_flow_target_value(&start) {
        Ok(start) => start,
        Err(error) => {
            return ScriptTestRunSummary::completed(
                test,
                false,
                ScriptTestFinalStatus::NotStarted,
                vec![format!(
                    "scenario test `goto` target `{start}` cannot be resolved: {error}"
                )],
                Vec::new(),
            );
        }
    };
    let matching_entries = plan
        .entries()
        .iter()
        .filter(|entry| {
            entry.kind == RuntimeEntryKind::Test
                && matches!(
                    &entry.target,
                    RuntimeEntryTarget::Flow(flow) | RuntimeEntryTarget::Controller(flow)
                        if flow == &start
                )
        })
        .collect::<Vec<_>>();
    let [entry] = matching_entries.as_slice() else {
        return ScriptTestRunSummary::completed(
            test,
            false,
            ScriptTestFinalStatus::NotStarted,
            vec![format!(
                "scenario target `{}` must be bound by exactly one `entry test` declaration",
                start.public_label()
            )],
            Vec::new(),
        );
    };
    let Ok(trace) = run_runtime_steps(
        plan.clone(),
        &entry.id,
        host_config,
        config,
        values,
        execution_diagnostics,
    ) else {
        return ScriptTestRunSummary::completed(
            test,
            false,
            ScriptTestFinalStatus::AdapterError,
            vec!["native adapter registration failed".to_owned()],
            Vec::new(),
        );
    };
    let final_status = trace.final_status.status_label(FlowStatusLabelStyle::Debug);
    let mut diagnostics = trace
        .steps
        .iter()
        .flat_map(|step| step.diagnostics.iter().cloned())
        .collect::<Vec<_>>();
    diagnostics.extend(trace.steps.iter().flat_map(|step| {
        step.assertion_diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
    }));
    diagnostics.extend(test_expectation_failures(test, &trace.steps));
    match trace.final_status {
        FlowFiberStatus::Done(_) => {}
        FlowFiberStatus::Failed(ref message) => {
            diagnostics.push(format!("runtime failed: {message}"));
        }
        FlowFiberStatus::Running
        | FlowFiberStatus::Dialogue(_)
        | FlowFiberStatus::Waiting(_)
        | FlowFiberStatus::NeedWaiting(_)
        | FlowFiberStatus::WaitingMany(_)
        | FlowFiberStatus::HostCall(_)
        | FlowFiberStatus::Choice(_) => diagnostics.push(format!(
            "scenario did not finish within {} step(s): {final_status}",
            config.steps
        )),
    }
    let passed = diagnostics.is_empty();
    ScriptTestRunSummary::completed(
        test,
        passed,
        ScriptTestFinalStatus::Flow(final_status),
        diagnostics,
        trace.steps,
    )
}
