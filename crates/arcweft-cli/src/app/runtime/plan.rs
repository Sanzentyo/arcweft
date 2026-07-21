use super::options::PlanOptions;
use crate::app::project::{load_and_check_selection, resolve_source_selection};
use crate::app::shared::print_json;
use crate::output::RuntimePlanReport;
use std::process::ExitCode;

pub(in crate::app) fn runtime_plan_command(options: &PlanOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let checked = load_and_check_selection(&selection, None)?;
    let lowered = checked.runtime_plan();
    let report = RuntimePlanReport::from_lowered(&checked, lowered);
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
