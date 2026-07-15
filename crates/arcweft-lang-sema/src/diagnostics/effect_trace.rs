use crate::{
    effect_diagnostics::{EffectDiagnostic, EffectTraceStep},
    effect_model::EffectSite,
};
use arcweft_source::Diagnostic;

pub(super) fn with_effect_trace_notes(
    mut diagnostic: Diagnostic,
    effect_diagnostic: &EffectDiagnostic,
) -> Diagnostic {
    let Some(trace) = effect_diagnostic.trace() else {
        return diagnostic;
    };
    diagnostic = diagnostic.with_note(format!("effect trace for `{}`:", trace.effect()));
    for (index, step) in trace.steps().iter().enumerate() {
        diagnostic = diagnostic.with_note(format!(
            "{}. {}",
            index + 1,
            effect_trace_step_message(step)
        ));
    }
    diagnostic
}

fn effect_trace_step_message(step: &EffectTraceStep) -> String {
    match step {
        EffectTraceStep::Call {
            caller,
            callee,
            site,
        } => format!("`{caller}` calls `{callee}`{}", effect_site_suffix(site)),
        EffectTraceStep::ExternalCall {
            caller,
            callee,
            site,
        } => format!(
            "`{caller}` calls external `{callee}`{}",
            effect_site_suffix(site)
        ),
        EffectTraceStep::DynamicCall {
            caller,
            target,
            site,
        } => format!(
            "`{caller}` invokes function value `{target}`{}",
            effect_site_suffix(site)
        ),
        EffectTraceStep::Perform {
            callable,
            effect,
            site,
        } => format!(
            "`{callable}` performs `{effect}`{}",
            effect_site_suffix(site)
        ),
    }
}

fn effect_site_suffix(site: &EffectSite) -> String {
    let mut parts = Vec::new();
    if !site.label().is_empty() {
        parts.push(site.label().to_owned());
    }
    if let Some(path) = site.path() {
        parts.push(format!("path {path}"));
    }
    match (site.line(), site.column()) {
        (Some(line), Some(column)) => parts.push(format!("line {line}, column {column}")),
        (Some(line), None) => parts.push(format!("line {line}")),
        (None, Some(column)) => parts.push(format!("column {column}")),
        (None, None) => {}
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" via {}", parts.join("; "))
    }
}
