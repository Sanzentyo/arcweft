use arcweft_agent_protocol::protocol::{AgentAssertionKind, AgentAssertionRequest};

pub(crate) fn agent_assertion_passed(request: &AgentAssertionRequest) -> bool {
    match request.kind {
        AgentAssertionKind::Expect => request.condition,
        AgentAssertionKind::Deny => !request.condition,
    }
}

pub(crate) fn agent_assertion_failure_message(request: &AgentAssertionRequest) -> String {
    if request.message.is_empty() {
        match request.kind {
            AgentAssertionKind::Expect => "expect condition evaluated to false".to_owned(),
            AgentAssertionKind::Deny => "deny condition evaluated to true".to_owned(),
        }
    } else {
        request.message.clone()
    }
}

pub(crate) const fn agent_assertion_kind_label(kind: AgentAssertionKind) -> &'static str {
    match kind {
        AgentAssertionKind::Expect => "expect",
        AgentAssertionKind::Deny => "deny",
    }
}
