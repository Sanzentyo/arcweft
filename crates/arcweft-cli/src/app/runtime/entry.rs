use arcweft_core::plan::{
    FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget, RuntimePlan,
    RuntimeRouteSpec,
};
use std::process::ExitCode;

pub(in crate::app) fn apply_runtime_entry_selection(
    plan: &mut RuntimePlan,
    entry: Option<&str>,
    flow: Option<&str>,
) -> Result<(), ExitCode> {
    if entry.is_some() && flow.is_some() {
        eprintln!("error: --entry and --flow are mutually exclusive");
        return Err(ExitCode::from(2));
    }
    if let Some(flow) = flow {
        let flow = FlowRuntimeId(normalize_flow_id(flow));
        if !plan.flows.iter().any(|candidate| candidate.id == flow) {
            eprintln!("error: unknown flow `{}`", flow.0);
            return Err(ExitCode::FAILURE);
        }
        plan.entry_flow = Some(flow);
        return Ok(());
    }
    if let Some(entry) = entry {
        let entry = normalize_entry_id(entry);
        let Some(spec) = plan
            .entries
            .iter()
            .find(|candidate| candidate.id.0 == entry)
        else {
            eprintln!("error: unknown entry `{entry}`");
            return Err(ExitCode::FAILURE);
        };
        let RuntimeEntryTarget::Flow(flow) = &spec.target else {
            eprintln!("error: entry `{entry}` does not select a single runnable flow");
            return Err(ExitCode::FAILURE);
        };
        plan.entry_flow = Some(flow.clone());
        return Ok(());
    }
    Ok(())
}

pub(in crate::app) fn select_server_entry<'a>(
    plan: &'a RuntimePlan,
    entry: Option<&str>,
) -> Result<&'a RuntimeEntrySpec, ExitCode> {
    if let Some(entry) = entry {
        let entry = normalize_entry_id(entry);
        let Some(spec) = plan
            .entries
            .iter()
            .find(|candidate| candidate.id.0 == entry)
        else {
            eprintln!("error: unknown entry `{entry}`");
            return Err(ExitCode::FAILURE);
        };
        if spec.kind != RuntimeEntryKind::Server {
            eprintln!("error: entry `{entry}` is not a server entry");
            return Err(ExitCode::FAILURE);
        }
        return Ok(spec);
    }
    let Some(spec) = plan
        .entries
        .iter()
        .find(|candidate| candidate.kind == RuntimeEntryKind::Server)
    else {
        eprintln!("error: no server entry found; declare `entry server @entry.name`");
        return Err(ExitCode::FAILURE);
    };
    Ok(spec)
}

pub(in crate::app) fn server_routes(entry: &RuntimeEntrySpec) -> Vec<RuntimeRouteSpec> {
    match &entry.target {
        RuntimeEntryTarget::Routes(routes) => routes.clone(),
        RuntimeEntryTarget::Flow(flow) => vec![RuntimeRouteSpec {
            method: "*".to_owned(),
            path: "*".to_owned(),
            target: flow.clone(),
            bindings: Vec::new(),
        }],
    }
}

pub(in crate::app) fn apply_runtime_cli_entry_selection(
    plan: &mut RuntimePlan,
    entry: Option<&str>,
) -> Result<(), ExitCode> {
    if let Some(entry) = entry {
        return apply_runtime_entry_selection(plan, Some(entry), None);
    }
    let Some(spec) = plan
        .entries
        .iter()
        .find(|candidate| candidate.kind == RuntimeEntryKind::Cli)
    else {
        eprintln!("error: no cli entry found; declare `entry cli @entry.name` or pass --entry");
        return Err(ExitCode::FAILURE);
    };
    let RuntimeEntryTarget::Flow(flow) = &spec.target else {
        eprintln!(
            "error: cli entry `{}` does not select a single runnable flow",
            spec.id.0
        );
        return Err(ExitCode::FAILURE);
    };
    plan.entry_flow = Some(flow.clone());
    Ok(())
}

fn normalize_flow_id(value: &str) -> String {
    normalize_entity_selector(value, "flow")
}

fn normalize_entry_id(value: &str) -> String {
    normalize_entity_selector(value, "entry")
}

fn normalize_entity_selector(value: &str, family: &str) -> String {
    let value = value.trim().trim_start_matches('@');
    if value.contains('.') {
        value.to_owned()
    } else {
        format!("{family}.{value}")
    }
}
