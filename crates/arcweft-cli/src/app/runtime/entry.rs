use arcweft_core::plan::{
    EntryRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget, RuntimePlan,
    RuntimeRouteSpec,
};
use std::process::ExitCode;

pub(in crate::app) fn select_runtime_entry(
    plan: &RuntimePlan,
    entry: &str,
) -> Result<EntryRuntimeId, ExitCode> {
    let entry = parse_runtime_entry_id(entry)?;
    let Some(spec) = plan
        .entries()
        .iter()
        .find(|candidate| candidate.id == entry)
    else {
        eprintln!("error: unknown entry `{}`", entry.public_label());
        return Err(ExitCode::FAILURE);
    };
    if matches!(&spec.target, RuntimeEntryTarget::Routes(_)) {
        eprintln!(
            "error: entry `{}` does not select a runnable flow or controller",
            entry.public_label()
        );
        return Err(ExitCode::FAILURE);
    }
    Ok(entry)
}

pub(in crate::app) fn select_server_entry<'a>(
    plan: &'a RuntimePlan,
    entry: &str,
) -> Result<&'a RuntimeEntrySpec, ExitCode> {
    let entry = parse_runtime_entry_id(entry)?;
    let Some(spec) = plan
        .entries()
        .iter()
        .find(|candidate| candidate.id == entry)
    else {
        eprintln!("error: unknown entry `{}`", entry.public_label());
        return Err(ExitCode::FAILURE);
    };
    if spec.kind != RuntimeEntryKind::Server {
        eprintln!(
            "error: entry `{}` is not a server entry",
            entry.public_label()
        );
        return Err(ExitCode::FAILURE);
    }
    Ok(spec)
}

pub(in crate::app) fn server_routes(entry: &RuntimeEntrySpec) -> Option<&[RuntimeRouteSpec]> {
    match &entry.target {
        RuntimeEntryTarget::Routes(routes) => Some(routes),
        RuntimeEntryTarget::Flow(_) | RuntimeEntryTarget::Controller(_) => None,
    }
}

pub(in crate::app) fn select_runtime_cli_entry(
    plan: &RuntimePlan,
    entry: Option<&str>,
) -> Result<EntryRuntimeId, ExitCode> {
    let entry = match entry {
        Some(entry) => parse_runtime_entry_id(entry)?,
        None => {
            let mut candidates = plan
                .entries()
                .iter()
                .filter(|candidate| candidate.kind == RuntimeEntryKind::Cli);
            let Some(candidate) = candidates.next() else {
                eprintln!("error: source launch has no `entry cli` declaration");
                return Err(ExitCode::FAILURE);
            };
            if candidates.next().is_some() {
                eprintln!(
                    "error: source launch has multiple `entry cli` declarations; pass --entry entry.*"
                );
                return Err(ExitCode::from(2));
            }
            candidate.id.clone()
        }
    };
    let Some(spec) = plan
        .entries()
        .iter()
        .find(|candidate| candidate.id == entry)
    else {
        eprintln!("error: unknown entry `{}`", entry.public_label());
        return Err(ExitCode::FAILURE);
    };
    if spec.kind != RuntimeEntryKind::Cli {
        eprintln!("error: entry `{}` is not a cli entry", entry.public_label());
        return Err(ExitCode::FAILURE);
    }
    if matches!(&spec.target, RuntimeEntryTarget::Routes(_)) {
        eprintln!(
            "error: cli entry `{}` does not select a runnable flow or controller",
            entry.public_label()
        );
        return Err(ExitCode::FAILURE);
    }
    Ok(entry)
}

pub(in crate::app) fn parse_runtime_entry_id(value: &str) -> Result<EntryRuntimeId, ExitCode> {
    EntryRuntimeId::from_source_entity_body(value).map_err(|error| {
        eprintln!("error: entry selector must be an exact canonical entry.* ID: {error}");
        ExitCode::from(2)
    })
}
