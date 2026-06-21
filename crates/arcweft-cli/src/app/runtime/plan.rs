use super::options::PlanOptions;
use crate::app::project::{
    load_and_check_selection, resolve_source_selection, runtime_plan_options_for_selection,
};
use crate::app::shared::print_json;
use crate::output::RuntimePlanReport;
use arcweft_compiler::lower::lower_source_runtime_plan_with_stats_and_options;
use std::process::ExitCode;

pub(in crate::app) fn runtime_plan_command(options: &PlanOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let checked = load_and_check_selection(&selection, None)?;
    let runtime_options = runtime_plan_options_for_selection(&selection);
    let lowered = lower_source_runtime_plan_with_stats_and_options(&checked.hir, &runtime_options)
        .map_err(|errors| {
            for error in errors {
                eprintln!("error: {error}");
            }
            ExitCode::from(2)
        })?;
    let report = RuntimePlanReport::from_lowered(&checked, &lowered);
    if options.json {
        print_json(&report)
    } else {
        for line in &report.lines {
            println!(
                "{} {} {} task_node={} child_task(s)={} effect(s)={}",
                line.flow_id.as_deref().unwrap_or("-"),
                line.line_id.as_deref().unwrap_or("-"),
                line.callee,
                line.root.kind,
                line.child_tasks,
                line.effects
            );
        }
        println!(
            "ok: {} ({} line task group(s), {} verifier obligation(s))",
            selection.path().display(),
            report.lines.len(),
            report.verifier_obligations
        );
        Ok(())
    }
}
