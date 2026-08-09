use std::collections::BTreeMap;

use arcweft_agent_protocol::protocol::{
    AgentAction, AgentAssertionKind, AgentAssertionRequest, AgentAttachment, AgentInvokeAction,
    CaptureFormat, CaptureRequest, ObserveRequest, PointerButton, RagRequest, WaitRequest,
};
use arcweft_core::{
    task::NamedHostTaskArg,
    value::{RuntimePayload, RuntimeValue},
};

use crate::label_parse::parse_pointer_button_label;
use crate::runtime_value::{
    runtime_agent_value_map, runtime_bool, runtime_capture_format, runtime_capture_target,
    runtime_duration_millis, runtime_predicate, runtime_public_id, runtime_public_ids,
    runtime_record_get, runtime_string, runtime_u32, runtime_usize, runtime_value_to_json,
    value_label,
};

#[derive(Debug)]
pub(crate) struct RuntimeAgentArgs<'a> {
    positionals: &'a [RuntimePayload],
    named: &'a [NamedHostTaskArg],
}

impl<'a> RuntimeAgentArgs<'a> {
    pub(crate) fn new(args: &'a [RuntimePayload], named_args: &'a [NamedHostTaskArg]) -> Self {
        Self {
            positionals: args,
            named: named_args,
        }
    }

    pub(crate) fn positional(&self, index: usize) -> Option<&'a RuntimeValue> {
        self.positionals.get(index).map(RuntimePayload::value)
    }

    pub(crate) fn named(&self, name: &str) -> Option<&'a RuntimeValue> {
        self.named
            .iter()
            .find(|argument| argument.name == name)
            .map(|argument| argument.value.value())
    }

    pub(crate) fn named_any(&self, names: &[&str]) -> Option<&'a RuntimeValue> {
        names.iter().find_map(|name| self.named(name))
    }

    pub(crate) fn has_named(&self) -> bool {
        !self.named.is_empty()
    }

    pub(crate) fn observe_request(&self) -> Result<ObserveRequest, String> {
        if !self.positionals.is_empty() {
            return Err("observe does not accept positional arguments".to_owned());
        }
        Ok(ObserveRequest {
            include_images: self
                .named("include_images")
                .map_or(Ok(false), runtime_bool)?,
            include_objects: self
                .named("include_objects")
                .map_or(Ok(true), runtime_bool)?,
            include_logs: self.named("include_logs").map_or(Ok(false), runtime_bool)?,
        })
    }

    pub(crate) fn capture_request(&self) -> Result<CaptureRequest, String> {
        let target = self
            .positional(0)
            .ok_or_else(|| "capture requires a target argument".to_owned())
            .and_then(runtime_capture_target)?;
        Ok(CaptureRequest {
            target,
            format: self
                .named("format")
                .map_or(Ok(CaptureFormat::Png), runtime_capture_format)?,
            capture_kind: self
                .named_any(&["capture_kind", "kind"])
                .map_or_else(|| Ok("color".to_owned()), runtime_string)?,
            name: self
                .named("name")
                .map_or_else(|| Ok("capture".to_owned()), runtime_string)?,
        })
    }

    pub(crate) fn rag_request(&self) -> Result<RagRequest, String> {
        let query = self
            .positional(0)
            .or_else(|| self.named("query"))
            .ok_or_else(|| "rag.query requires a query argument".to_owned())
            .and_then(runtime_string)?;
        Ok(RagRequest {
            query,
            roots: match self.named("roots") {
                Some(value) => runtime_public_ids(value)?,
                None => Vec::new(),
            },
            graph_depth: self.named("graph_depth").map_or(Ok(1), runtime_u32)?,
            limit: self.named("limit").map_or(Ok(8), runtime_usize)?,
        })
    }

    pub(crate) fn wait_request(&self) -> Result<WaitRequest, String> {
        let predicate = self
            .positional(0)
            .or_else(|| self.named("predicate"))
            .ok_or_else(|| "wait requires a predicate argument".to_owned())
            .and_then(runtime_predicate)?;
        let timeout_millis = self
            .positional(1)
            .or_else(|| self.named("timeout"))
            .ok_or_else(|| "wait requires timeout".to_owned())
            .and_then(runtime_duration_millis)?;
        Ok(WaitRequest {
            predicate,
            timeout_millis,
            stable_frames: self.named("stable_frames").map_or(Ok(1), runtime_u32)?,
            poll_frames: self.named("poll_frames").map_or(Ok(1), runtime_u32)?,
        })
    }

    pub(crate) fn assertion_request(
        &self,
        kind: AgentAssertionKind,
    ) -> Result<AgentAssertionRequest, String> {
        let condition = self
            .positional(0)
            .or_else(|| self.named("condition"))
            .ok_or_else(|| "Agent assertion requires a condition argument".to_owned())
            .and_then(runtime_bool)?;
        let message = match self.positional(1).or_else(|| self.named("message")) {
            Some(value) => runtime_string(value)?,
            None => String::new(),
        };
        Ok(AgentAssertionRequest {
            kind,
            condition,
            message,
        })
    }

    pub(crate) fn invoke_action(&self) -> Result<AgentAction, String> {
        let target = self
            .positional(0)
            .or_else(|| self.named("target"))
            .ok_or_else(|| "invoke requires a target argument".to_owned())
            .and_then(runtime_public_id)?;
        let action = self
            .positional(1)
            .or_else(|| self.named("action"))
            .ok_or_else(|| "invoke requires an action argument".to_owned())
            .and_then(runtime_string)?;
        let call_args = match self.named("args") {
            Some(value) => runtime_agent_value_map(value)?,
            None => BTreeMap::new(),
        };
        Ok(AgentAction::Invoke(Box::new(AgentInvokeAction {
            target,
            action,
            args: Box::new(call_args),
        })))
    }

    pub(crate) fn pointer_click_action(&self) -> Result<AgentAction, String> {
        let (x, y) = self
            .positional(0)
            .or_else(|| self.named("point"))
            .ok_or_else(|| "pointer.click requires a point argument".to_owned())
            .and_then(runtime_viewport_point)?;
        let button = self
            .named("button")
            .map_or(Ok(PointerButton::Primary), runtime_pointer_button)?;
        Ok(AgentAction::PointerClick { x, y, button })
    }

    pub(crate) fn attach_request(&self) -> Result<AgentAttachment, String> {
        let resource = self
            .positional(0)
            .or_else(|| self.named("resource"))
            .ok_or_else(|| "attach requires a resource argument".to_owned())?;
        if self.positional(1).is_some() {
            return Err("attach received too many positional arguments".to_owned());
        }
        Ok(AgentAttachment {
            resource: Box::new(runtime_value_to_json(resource)),
        })
    }
}

fn runtime_viewport_point(value: &RuntimeValue) -> Result<(u32, u32), String> {
    match value {
        RuntimeValue::Record(fields) => Ok((
            runtime_record_get(fields, "x").and_then(runtime_u32)?,
            runtime_record_get(fields, "y").and_then(runtime_u32)?,
        )),
        RuntimeValue::Tuple(values) if values.len() == 2 => {
            Ok((runtime_u32(&values[0])?, runtime_u32(&values[1])?))
        }
        other => Err(format!(
            "expected viewport point record, got `{}`",
            value_label(other)
        )),
    }
}

fn runtime_pointer_button(value: &RuntimeValue) -> Result<PointerButton, String> {
    parse_pointer_button_label(&runtime_string(value)?)
}
