use arcweft_agent_protocol::{
    artifact::{ProjectBinding, RequiredEntity},
    protocol::{AgentSessionInfo, WaitRequest},
};
use arcweft_core::entry::AgentBudget;

use crate::config::AgentControllerRunConfig;
use crate::error::AgentRunError;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct AgentBudgetTracker {
    pub(crate) host_calls: u32,
    pub(crate) observations: u32,
    pub(crate) captures: u32,
    pub(crate) capture_bytes: u64,
    pub(crate) rag_queries: u32,
    pub(crate) context_bytes: u64,
}

pub(crate) struct AgentBudgetContext<'a> {
    pub(crate) limits: AgentBudget,
    pub(crate) tracker: &'a mut AgentBudgetTracker,
}
pub(crate) fn project_binding_mismatch<SessionError, DebugError, RagError>(
    binding: &ProjectBinding,
    session_info: &AgentSessionInfo,
    detail: String,
) -> AgentRunError<SessionError, DebugError, RagError>
where
    SessionError: std::error::Error + Send + Sync + 'static,
    DebugError: std::error::Error + Send + Sync + 'static,
    RagError: std::error::Error + Send + Sync + 'static,
{
    AgentRunError::ProjectBindingMismatch {
        expected_program_hash: binding.program_hash.as_str().to_owned(),
        actual_program_hash: session_info.program_hash.clone(),
        mode: binding.mode,
        detail,
    }
}

pub(crate) fn effective_controller_budget(
    manifest: AgentBudget,
    config: AgentControllerRunConfig,
) -> AgentBudget {
    let runtime = AgentBudget::default();
    AgentBudget {
        logical_timeout_millis: manifest
            .logical_timeout_millis
            .min(runtime.logical_timeout_millis),
        max_vm_steps: manifest
            .max_vm_steps
            .min(runtime.max_vm_steps)
            .min(u64::try_from(config.max_steps).unwrap_or(u64::MAX)),
        max_host_calls: manifest.max_host_calls.min(runtime.max_host_calls),
        max_observations: manifest.max_observations.min(runtime.max_observations),
        max_captures: manifest.max_captures.min(runtime.max_captures),
        max_capture_bytes: manifest.max_capture_bytes.min(runtime.max_capture_bytes),
        max_rag_queries: manifest.max_rag_queries.min(runtime.max_rag_queries),
        max_context_bytes: manifest.max_context_bytes.min(runtime.max_context_bytes),
    }
}

pub(crate) fn effective_wait_request(
    mut request: WaitRequest,
    logical_timeout_millis: Option<u64>,
) -> WaitRequest {
    if let Some(limit) = logical_timeout_millis {
        request.timeout_millis = request.timeout_millis.min(limit);
    }
    request
}

pub(crate) fn record_budget_u32<SessionError, DebugError, RagError>(
    kind: &'static str,
    used: &mut u32,
    amount: u32,
    limit: u32,
) -> Result<(), AgentRunError<SessionError, DebugError, RagError>>
where
    SessionError: std::error::Error + Send + Sync + 'static,
    DebugError: std::error::Error + Send + Sync + 'static,
    RagError: std::error::Error + Send + Sync + 'static,
{
    let attempted = used.saturating_add(amount);
    if attempted > limit {
        return Err(AgentRunError::ControllerResourceBudgetExceeded {
            kind,
            limit: u64::from(limit),
            attempted: u64::from(attempted),
        });
    }
    *used = attempted;
    Ok(())
}

pub(crate) fn record_budget_u64<SessionError, DebugError, RagError>(
    kind: &'static str,
    used: &mut u64,
    amount: u64,
    limit: u64,
) -> Result<(), AgentRunError<SessionError, DebugError, RagError>>
where
    SessionError: std::error::Error + Send + Sync + 'static,
    DebugError: std::error::Error + Send + Sync + 'static,
    RagError: std::error::Error + Send + Sync + 'static,
{
    let attempted = used.saturating_add(amount);
    if attempted > limit {
        return Err(AgentRunError::ControllerResourceBudgetExceeded {
            kind,
            limit,
            attempted,
        });
    }
    *used = attempted;
    Ok(())
}

pub(crate) fn compatible_entity_mismatch(
    required: &RequiredEntity,
    actual: Option<&RequiredEntity>,
) -> Option<String> {
    let Some(actual) = actual else {
        return Some(format!(
            "required entity {} is missing",
            required.public_id.as_str()
        ));
    };
    if required.kind != actual.kind {
        return Some(format!(
            "required entity {} kind mismatch: expected {}, actual {}",
            required.public_id.as_str(),
            required.kind,
            actual.kind
        ));
    }
    if required.semantic_hash != actual.semantic_hash {
        return Some(format!(
            "required entity {} semantic hash mismatch: expected {}, actual {}",
            required.public_id.as_str(),
            required.semantic_hash.as_str(),
            actual.semantic_hash.as_str()
        ));
    }
    None
}
