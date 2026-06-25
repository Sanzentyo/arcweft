use arcweft_agent_protocol::{
    ids::AgentResourceUri,
    protocol::{AgentAction, AgentAssertionKind, AgentAttachment, AgentHostRequest},
};
use arcweft_core::{
    effect::{LineEffectRequest, RuntimeCall},
    task::HostTaskRequest,
};

use crate::label_parse::{
    capture_request, effect_form_attachment_resource, invoke_action, observe_request,
    parse_public_id_arg, parse_string_label, pointer_click_action, rag_request, wait_request,
};
use crate::runtime_args::RuntimeAgentArgs;
use crate::runtime_value::{runtime_public_id, runtime_string, runtime_u32};

pub(crate) fn agent_host_request_from_effect(
    effect: &LineEffectRequest,
) -> Result<AgentHostRequest, String> {
    match effect {
        LineEffectRequest::Call(call) => agent_host_request_from_call(call),
        other => Err(format!("{other:?}")),
    }
}

pub(crate) fn agent_host_request_from_call(call: &RuntimeCall) -> Result<AgentHostRequest, String> {
    match call.callee.as_str() {
        "observe" => Ok(AgentHostRequest::Observe(Box::new(observe_request(
            &call.args,
        )?))),
        "checkpoint" => Ok(AgentHostRequest::Checkpoint {
            name: call
                .args
                .first()
                .and_then(|arg| parse_string_label(arg))
                .unwrap_or_else(|| call.args.first().cloned().unwrap_or_default()),
        }),
        "attach" => Ok(AgentHostRequest::Attach(Box::new(AgentAttachment {
            resource: Box::new(effect_form_attachment_resource(&call.args)?),
        }))),
        "advance_text" => {
            if !call.args.is_empty() {
                return Err("advance_text does not accept arguments".to_owned());
            }
            Ok(AgentHostRequest::Act(Box::new(AgentAction::AdvanceText)))
        }
        "choose" => {
            let choice = call
                .args
                .first()
                .ok_or_else(|| "choose requires a choice argument".to_owned())
                .and_then(|arg| parse_public_id_arg(arg))?;
            Ok(AgentHostRequest::Act(Box::new(AgentAction::SelectChoice {
                choice,
            })))
        }
        "pointer.click" => {
            pointer_click_action(&call.args).map(|action| AgentHostRequest::Act(Box::new(action)))
        }
        "invoke" => invoke_action(&call.args).map(|action| AgentHostRequest::Act(Box::new(action))),
        "capture" => Ok(AgentHostRequest::Capture(Box::new(capture_request(
            &call.args,
        )?))),
        "wait" => Ok(AgentHostRequest::Wait(Box::new(wait_request(&call.args)?))),
        "rag.query" => Ok(AgentHostRequest::RagQuery(Box::new(rag_request(
            &call.args,
        )?))),
        "read_resource" => {
            let uri = call
                .args
                .first()
                .and_then(|arg| parse_string_label(arg).or_else(|| Some(arg.clone())))
                .ok_or_else(|| "read_resource requires a uri argument".to_owned())?;
            Ok(AgentHostRequest::ReadResource {
                uri: AgentResourceUri::new(uri).map_err(|error| error.to_string())?,
            })
        }
        "entity_meta" => {
            let entity = call
                .args
                .first()
                .ok_or_else(|| "entity_meta requires an entity argument".to_owned())
                .and_then(|arg| parse_public_id_arg(arg))?;
            Ok(AgentHostRequest::EntityMetadata { entity })
        }
        "project_neighbors" => {
            let root = call
                .args
                .first()
                .ok_or_else(|| "project_neighbors requires a root argument".to_owned())
                .and_then(|arg| parse_public_id_arg(arg))?;
            Ok(AgentHostRequest::ProjectGraphNeighborhood { root, depth: 1 })
        }
        other => Err(format!("unsupported Agent call `{other}`")),
    }
}

pub(crate) fn agent_host_request_from_task(
    request: &HostTaskRequest,
) -> Result<AgentHostRequest, String> {
    let HostTaskRequest::Custom {
        capability,
        operation,
        args,
        ..
    } = request
    else {
        return Err(format!("unsupported Agent task request `{request:?}`"));
    };
    if capability.0 != "agent" {
        return Err(format!(
            "unsupported Agent task capability `{}`",
            capability.0
        ));
    }
    let args = RuntimeAgentArgs::new(args);
    match operation.as_str() {
        "observe" => args
            .observe_request()
            .map(|request| AgentHostRequest::Observe(Box::new(request))),
        "capture" => args
            .capture_request()
            .map(|request| AgentHostRequest::Capture(Box::new(request))),
        "choose" => {
            let choice = args
                .positional(0)
                .ok_or_else(|| "choose requires a choice argument".to_owned())
                .and_then(runtime_public_id)?;
            Ok(AgentHostRequest::Act(Box::new(AgentAction::SelectChoice {
                choice,
            })))
        }
        "advance_text" => {
            if args.positional(0).is_some() || args.has_named() {
                return Err("advance_text does not accept arguments".to_owned());
            }
            Ok(AgentHostRequest::Act(Box::new(AgentAction::AdvanceText)))
        }
        "pointer.click" => args
            .pointer_click_action()
            .map(|action| AgentHostRequest::Act(Box::new(action))),
        "invoke" => args
            .invoke_action()
            .map(|action| AgentHostRequest::Act(Box::new(action))),
        "read_resource" => {
            let uri = args
                .positional(0)
                .or_else(|| args.named("uri"))
                .ok_or_else(|| "read_resource requires a uri argument".to_owned())
                .and_then(runtime_string)?;
            Ok(AgentHostRequest::ReadResource {
                uri: AgentResourceUri::new(uri).map_err(|error| error.to_string())?,
            })
        }
        "entity_meta" => {
            let entity = args
                .positional(0)
                .or_else(|| args.named("entity"))
                .ok_or_else(|| "entity_meta requires an entity argument".to_owned())
                .and_then(runtime_public_id)?;
            Ok(AgentHostRequest::EntityMetadata { entity })
        }
        "project_neighbors" => {
            let root = args
                .positional(0)
                .or_else(|| args.named("root"))
                .ok_or_else(|| "project_neighbors requires a root argument".to_owned())
                .and_then(runtime_public_id)?;
            let depth = args.named("depth").map_or(Ok(1), runtime_u32)?;
            Ok(AgentHostRequest::ProjectGraphNeighborhood { root, depth })
        }
        "rag.query" => args
            .rag_request()
            .map(|request| AgentHostRequest::RagQuery(Box::new(request))),
        "expect" => args
            .assertion_request(AgentAssertionKind::Expect)
            .map(|request| AgentHostRequest::Assert(Box::new(request))),
        "deny" => args
            .assertion_request(AgentAssertionKind::Deny)
            .map(|request| AgentHostRequest::Assert(Box::new(request))),
        "checkpoint" => {
            let name = args
                .positional(0)
                .or_else(|| args.named("name"))
                .map_or_else(|| Ok("checkpoint".to_owned()), runtime_string)?;
            Ok(AgentHostRequest::Checkpoint { name })
        }
        "attach" => args
            .attach_request()
            .map(|request| AgentHostRequest::Attach(Box::new(request))),
        "wait" => args
            .wait_request()
            .map(|request| AgentHostRequest::Wait(Box::new(request))),
        other => Err(format!("unsupported Agent task operation `{other}`")),
    }
}
