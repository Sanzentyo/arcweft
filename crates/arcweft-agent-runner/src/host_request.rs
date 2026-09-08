use arcweft_agent_protocol::{
    ids::{AgentProjectGraphSymbolId, AgentResourceUri},
    protocol::{AgentAction, AgentAssertionKind, AgentAttachment, AgentHostRequest},
};
use arcweft_core::{
    effect::{LineEffectRequest, RuntimeCall},
    step::RuntimeHostCallRequest,
    task::HostTaskRequest,
};

use crate::error::AgentHostRequestAdmissionError;
use crate::label_parse::{
    capture_request, effect_form_attachment_resource, invoke_action, observe_request,
    parse_public_id_arg, parse_string_label, pointer_click_action, rag_request, wait_request,
};
use crate::runtime_args::RuntimeAgentArgs;
use crate::runtime_value::{runtime_public_id, runtime_string, runtime_u32};

pub(crate) fn agent_host_request_from_effect(
    effect: &LineEffectRequest,
) -> Result<AgentHostRequest, AgentHostRequestAdmissionError> {
    match effect {
        LineEffectRequest::Call(call) => agent_host_request_from_call(call),
        other => Err(AgentHostRequestAdmissionError::UnsupportedEffect {
            effect: format!("{other:?}"),
        }),
    }
}

pub(crate) fn agent_host_request_from_call(
    call: &RuntimeCall,
) -> Result<AgentHostRequest, AgentHostRequestAdmissionError> {
    match call.callee.as_str() {
        "observe" => observe_request(&call.args)
            .map(|request| AgentHostRequest::Observe(Box::new(request)))
            .map_err(|detail| AgentHostRequestAdmissionError::invalid_arguments("observe", detail)),
        "checkpoint" => Ok(AgentHostRequest::Checkpoint {
            name: call
                .args
                .first()
                .ok_or(AgentHostRequestAdmissionError::MissingArgument {
                    operation: "checkpoint",
                    argument: "name",
                })
                .and_then(|arg| {
                    parse_string_label(arg).ok_or_else(|| {
                        AgentHostRequestAdmissionError::invalid_arguments(
                            "checkpoint",
                            "name must be a string label",
                        )
                    })
                })?,
        }),
        "attach" => effect_form_attachment_resource(&call.args)
            .map(|resource| {
                AgentHostRequest::Attach(Box::new(AgentAttachment {
                    resource: Box::new(resource),
                }))
            })
            .map_err(|detail| AgentHostRequestAdmissionError::invalid_arguments("attach", detail)),
        "advance_text" => {
            if !call.args.is_empty() {
                return Err(AgentHostRequestAdmissionError::invalid_arguments(
                    "advance_text",
                    "does not accept arguments",
                ));
            }
            Ok(AgentHostRequest::Act(Box::new(AgentAction::AdvanceText)))
        }
        "choose" => {
            let choice = call
                .args
                .first()
                .ok_or(AgentHostRequestAdmissionError::MissingArgument {
                    operation: "choose",
                    argument: "choice",
                })
                .and_then(|arg| {
                    parse_public_id_arg(arg).map_err(|detail| {
                        AgentHostRequestAdmissionError::invalid_arguments("choose", detail)
                    })
                })?;
            Ok(AgentHostRequest::Act(Box::new(AgentAction::SelectChoice {
                choice,
            })))
        }
        "pointer.click" => pointer_click_action(&call.args)
            .map(|action| AgentHostRequest::Act(Box::new(action)))
            .map_err(|detail| {
                AgentHostRequestAdmissionError::invalid_arguments("pointer.click", detail)
            }),
        "invoke" => invoke_action(&call.args)
            .map(|action| AgentHostRequest::Act(Box::new(action)))
            .map_err(|detail| AgentHostRequestAdmissionError::invalid_arguments("invoke", detail)),
        "capture" => capture_request(&call.args)
            .map(|request| AgentHostRequest::Capture(Box::new(request)))
            .map_err(|detail| AgentHostRequestAdmissionError::invalid_arguments("capture", detail)),
        "wait" => wait_request(&call.args)
            .map(|request| AgentHostRequest::Wait(Box::new(request)))
            .map_err(|detail| AgentHostRequestAdmissionError::invalid_arguments("wait", detail)),
        "rag.query" => rag_request(&call.args)
            .map(|request| AgentHostRequest::RagQuery(Box::new(request)))
            .map_err(|detail| {
                AgentHostRequestAdmissionError::invalid_arguments("rag.query", detail)
            }),
        "read_resource" => {
            let uri = call
                .args
                .first()
                .and_then(|arg| parse_string_label(arg).or_else(|| Some(arg.clone())))
                .ok_or(AgentHostRequestAdmissionError::MissingArgument {
                    operation: "read_resource",
                    argument: "uri",
                })?;
            Ok(AgentHostRequest::ReadResource {
                uri: AgentResourceUri::new(uri).map_err(|error| {
                    AgentHostRequestAdmissionError::invalid_arguments(
                        "read_resource",
                        error.to_string(),
                    )
                })?,
            })
        }
        "entity_meta" => {
            let entity = call
                .args
                .first()
                .ok_or(AgentHostRequestAdmissionError::MissingArgument {
                    operation: "entity_meta",
                    argument: "entity",
                })
                .and_then(|arg| {
                    parse_public_id_arg(arg).map_err(|detail| {
                        AgentHostRequestAdmissionError::invalid_arguments("entity_meta", detail)
                    })
                })?;
            Ok(AgentHostRequest::EntityMetadata { entity })
        }
        "project_neighbors" => {
            let root = call
                .args
                .first()
                .and_then(|arg| parse_string_label(arg))
                .ok_or(AgentHostRequestAdmissionError::MissingArgument {
                    operation: "project_neighbors",
                    argument: "root",
                })?;
            let root = AgentProjectGraphSymbolId::new(root).map_err(|error| {
                AgentHostRequestAdmissionError::invalid_arguments(
                    "project_neighbors",
                    error.to_string(),
                )
            })?;
            Ok(AgentHostRequest::ProjectGraphNeighborhood { root, depth: 1 })
        }
        other => Err(AgentHostRequestAdmissionError::UnsupportedOperation {
            operation: other.to_owned(),
        }),
    }
}

pub(crate) fn agent_host_request_from_task(
    request: &HostTaskRequest,
) -> Result<AgentHostRequest, AgentHostRequestAdmissionError> {
    let HostTaskRequest::Custom {
        capability,
        operation,
        args,
        named_args,
    } = request
    else {
        return Err(AgentHostRequestAdmissionError::UnsupportedEffect {
            effect: format!("{request:?}"),
        });
    };
    if capability.0 != "agent" {
        return Err(AgentHostRequestAdmissionError::UnsupportedCapability {
            capability: capability.0.clone(),
        });
    }
    let args = RuntimeAgentArgs::new(args, named_args);
    agent_host_request_from_runtime_args(operation, &args)
}

pub(crate) fn agent_host_request_from_host_call(
    request: &RuntimeHostCallRequest,
) -> Result<AgentHostRequest, AgentHostRequestAdmissionError> {
    if request.capability != "agent" {
        return Err(AgentHostRequestAdmissionError::UnsupportedCapability {
            capability: request.capability.clone(),
        });
    }
    let args = RuntimeAgentArgs::new(&request.args, &request.named_args);
    agent_host_request_from_runtime_args(&request.operation, &args)
}

fn agent_host_request_from_runtime_args(
    operation: &str,
    args: &RuntimeAgentArgs<'_>,
) -> Result<AgentHostRequest, AgentHostRequestAdmissionError> {
    match operation {
        "observe" => args
            .observe_request()
            .map(|request| AgentHostRequest::Observe(Box::new(request)))
            .map_err(|detail| AgentHostRequestAdmissionError::invalid_arguments("observe", detail)),
        "capture" => args
            .capture_request()
            .map(|request| AgentHostRequest::Capture(Box::new(request)))
            .map_err(|detail| AgentHostRequestAdmissionError::invalid_arguments("capture", detail)),
        "choose" => {
            let choice = args
                .positional(0)
                .ok_or(AgentHostRequestAdmissionError::MissingArgument {
                    operation: "choose",
                    argument: "choice",
                })
                .and_then(|value| {
                    runtime_public_id(value).map_err(|detail| {
                        AgentHostRequestAdmissionError::invalid_arguments("choose", detail)
                    })
                })?;
            Ok(AgentHostRequest::Act(Box::new(AgentAction::SelectChoice {
                choice,
            })))
        }
        "advance_text" => {
            if args.positional(0).is_some() || args.has_named() {
                return Err(AgentHostRequestAdmissionError::invalid_arguments(
                    "advance_text",
                    "does not accept arguments",
                ));
            }
            Ok(AgentHostRequest::Act(Box::new(AgentAction::AdvanceText)))
        }
        "pointer.click" => args
            .pointer_click_action()
            .map(|action| AgentHostRequest::Act(Box::new(action)))
            .map_err(|detail| {
                AgentHostRequestAdmissionError::invalid_arguments("pointer.click", detail)
            }),
        "invoke" => args
            .invoke_action()
            .map(|action| AgentHostRequest::Act(Box::new(action)))
            .map_err(|detail| AgentHostRequestAdmissionError::invalid_arguments("invoke", detail)),
        "read_resource" => {
            let uri = args
                .positional(0)
                .or_else(|| args.named("uri"))
                .ok_or(AgentHostRequestAdmissionError::MissingArgument {
                    operation: "read_resource",
                    argument: "uri",
                })
                .and_then(|value| {
                    runtime_string(value).map_err(|detail| {
                        AgentHostRequestAdmissionError::invalid_arguments("read_resource", detail)
                    })
                })?;
            Ok(AgentHostRequest::ReadResource {
                uri: AgentResourceUri::new(uri).map_err(|error| {
                    AgentHostRequestAdmissionError::invalid_arguments(
                        "read_resource",
                        error.to_string(),
                    )
                })?,
            })
        }
        "entity_meta" => {
            let entity = args
                .positional(0)
                .or_else(|| args.named("entity"))
                .ok_or(AgentHostRequestAdmissionError::MissingArgument {
                    operation: "entity_meta",
                    argument: "entity",
                })
                .and_then(|value| {
                    runtime_public_id(value).map_err(|detail| {
                        AgentHostRequestAdmissionError::invalid_arguments("entity_meta", detail)
                    })
                })?;
            Ok(AgentHostRequest::EntityMetadata { entity })
        }
        "project_neighbors" => {
            let root = args
                .positional(0)
                .or_else(|| args.named("root"))
                .ok_or(AgentHostRequestAdmissionError::MissingArgument {
                    operation: "project_neighbors",
                    argument: "root",
                })
                .and_then(|value| {
                    runtime_string(value).map_err(|detail| {
                        AgentHostRequestAdmissionError::invalid_arguments(
                            "project_neighbors",
                            detail,
                        )
                    })
                })?;
            let root = AgentProjectGraphSymbolId::new(root).map_err(|error| {
                AgentHostRequestAdmissionError::invalid_arguments(
                    "project_neighbors",
                    error.to_string(),
                )
            })?;
            let depth = args
                .named("depth")
                .map_or(Ok(1), runtime_u32)
                .map_err(|detail| {
                    AgentHostRequestAdmissionError::invalid_arguments("project_neighbors", detail)
                })?;
            Ok(AgentHostRequest::ProjectGraphNeighborhood { root, depth })
        }
        "rag.query" => args
            .rag_request()
            .map(|request| AgentHostRequest::RagQuery(Box::new(request)))
            .map_err(|detail| {
                AgentHostRequestAdmissionError::invalid_arguments("rag.query", detail)
            }),
        "expect" => args
            .assertion_request(AgentAssertionKind::Expect)
            .map(|request| AgentHostRequest::Assert(Box::new(request)))
            .map_err(|detail| AgentHostRequestAdmissionError::invalid_arguments("expect", detail)),
        "deny" => args
            .assertion_request(AgentAssertionKind::Deny)
            .map(|request| AgentHostRequest::Assert(Box::new(request)))
            .map_err(|detail| AgentHostRequestAdmissionError::invalid_arguments("deny", detail)),
        "checkpoint" => {
            let name = args
                .positional(0)
                .or_else(|| args.named("name"))
                .ok_or(AgentHostRequestAdmissionError::MissingArgument {
                    operation: "checkpoint",
                    argument: "name",
                })
                .and_then(|value| {
                    runtime_string(value).map_err(|detail| {
                        AgentHostRequestAdmissionError::invalid_arguments("checkpoint", detail)
                    })
                })?;
            Ok(AgentHostRequest::Checkpoint { name })
        }
        "attach" => args
            .attach_request()
            .map(|request| AgentHostRequest::Attach(Box::new(request))),
        "wait" => args
            .wait_request()
            .map(|request| AgentHostRequest::Wait(Box::new(request)))
            .map_err(|detail| AgentHostRequestAdmissionError::invalid_arguments("wait", detail)),
        other => Err(AgentHostRequestAdmissionError::UnsupportedOperation {
            operation: other.to_owned(),
        }),
    }
}
