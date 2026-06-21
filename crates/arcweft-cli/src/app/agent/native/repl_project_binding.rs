use super::repl::{AgentReplBinding, AgentReplState};

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(super) struct AgentReplBindingProjectDecision {
    pub(super) name: String,
    pub(super) binding_kind: String,
    pub(super) status: String,
    pub(super) snapshot_kind: Option<String>,
    pub(super) decision: &'static str,
    pub(super) reason: &'static str,
    pub(super) old_program_hash: String,
    pub(super) new_program_hash: String,
}

pub(super) fn agent_repl_reconcile_project_bound_bindings(
    state: &mut AgentReplState,
    old_program_hash: Option<&str>,
    new_program_hash: Option<&str>,
) -> Vec<AgentReplBindingProjectDecision> {
    let (Some(old_program_hash), Some(new_program_hash)) = (old_program_hash, new_program_hash)
    else {
        return Vec::new();
    };
    if old_program_hash == new_program_hash {
        return Vec::new();
    }

    let names = state.bindings.keys().cloned().collect::<Vec<_>>();
    let mut decisions = Vec::with_capacity(names.len());
    for name in names {
        let Some(binding) = state.bindings.get(&name) else {
            continue;
        };
        let (decision, reason) = agent_repl_project_binding_decision(binding);
        decisions.push(AgentReplBindingProjectDecision {
            name: binding.name.clone(),
            binding_kind: binding.binding_kind.clone(),
            status: binding.status.clone(),
            snapshot_kind: binding.snapshot_kind.clone(),
            decision,
            reason,
            old_program_hash: old_program_hash.to_owned(),
            new_program_hash: new_program_hash.to_owned(),
        });
        if decision == "dropped" {
            state.bindings.remove(&name);
        }
    }
    decisions
}

fn agent_repl_project_binding_decision(binding: &AgentReplBinding) -> (&'static str, &'static str) {
    if binding.status != "ok" {
        return ("dropped", "binding did not complete successfully");
    }
    if binding.binding_kind == "local"
        && binding.serializable
        && binding.snapshot_kind.as_deref() == Some("literal")
    {
        return (
            "preserved",
            "literal snapshot is independent of the previous remote session",
        );
    }
    match binding.binding_kind.as_str() {
        "local" => (
            "dropped",
            "local binding snapshot is project-bound or session-derived",
        ),
        "cell" => (
            "dropped",
            "cell artifact belongs to the previous program hash",
        ),
        "loaded_agent" => (
            "dropped",
            "loaded Agent source belongs to the previous program hash",
        ),
        _ => ("dropped", "binding kind is not project-independent"),
    }
}
