use super::{
    AwaitState, AwaitTarget, CancelScopeId, ChoiceRuntimeOption, ChoiceState, Engine, FlowEvent,
    FlowFiberStatus, LineEffectRequest, RuntimeDiagnostic, RuntimeFieldValue, RuntimePayload,
    RuntimeStepInput, RuntimeStepOutput, RuntimeValue, TaskEvent, TaskEventKind, TaskKey,
    TaskPolicy, TaskPriority, TaskSpec,
};
use crate::task::{
    AssetRequest, AudioDecodeRequest, FileReadBytesRequest, FileReadTextRequest,
    FileWriteBytesRequest, FileWriteTextRequest, HostTaskArgTemplate, HostTaskRequest,
    HttpFetchRequest, HttpRespondRequest, ProcessRunRequest, ShaderRequest, TtsRequest,
    WasmCallRequest,
};

impl Engine {
    pub(super) fn resume_suspended(
        &mut self,
        input: &RuntimeStepInput,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
    ) -> bool {
        match self.fiber.status.clone() {
            FlowFiberStatus::Waiting(state) => {
                self.resume_await_state(state, events, output);
                true
            }
            FlowFiberStatus::Choice(state) => {
                self.resume_choice_state(state, input, output);
                true
            }
            FlowFiberStatus::Running | FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_) => {
                false
            }
        }
    }

    pub(super) fn resume_await_state(
        &mut self,
        state: AwaitState,
        events: &[TaskEvent],
        output: &mut RuntimeStepOutput,
    ) {
        let Some(event) = events
            .iter()
            .find(|event| event.task_id == state.target.task)
            .cloned()
        else {
            self.fiber.status = FlowFiberStatus::Waiting(state);
            return;
        };
        match event.kind {
            TaskEventKind::Ready(value) => {
                if let Some(binding) = &state.binding {
                    let ready_value = RuntimeValue::String(value.clone());
                    match self.try_bind_pattern(binding, &ready_value) {
                        Ok(true) => {}
                        Ok(false) => {
                            self.fiber.status = FlowFiberStatus::Failed(
                                "await result did not match binding pattern".to_owned(),
                            );
                            output.diagnostics.push(RuntimeDiagnostic {
                                message: "await result did not match binding pattern".to_owned(),
                            });
                            return;
                        }
                        Err(error) => {
                            self.fail_eval(error, output);
                            return;
                        }
                    }
                }
                output.flow_events.push(FlowEvent::AwaitReady {
                    need: state.target.need,
                    value,
                });
                self.fiber.cursor = Some(state.resume);
                self.fiber.status = FlowFiberStatus::Running;
            }
            TaskEventKind::Progress(progress) => {
                output.flow_events.push(FlowEvent::AwaitProgress {
                    need: state.target.need.clone(),
                    progress,
                });
                self.fiber.status = FlowFiberStatus::Waiting(state);
            }
            TaskEventKind::Err(error) => {
                self.fiber.status = FlowFiberStatus::Failed(error.clone());
                output.diagnostics.push(RuntimeDiagnostic {
                    message: format!("await task {} failed: {error}", state.target.task.0),
                });
            }
            TaskEventKind::Cancelled => {
                let message = format!("await task {} was cancelled", state.target.task.0);
                self.fiber.status = FlowFiberStatus::Failed(message.clone());
                output.diagnostics.push(RuntimeDiagnostic { message });
            }
        }
    }

    pub(super) fn resume_choice_state(
        &mut self,
        state: ChoiceState,
        input: &RuntimeStepInput,
        output: &mut RuntimeStepOutput,
    ) {
        let Some(option) = state
            .options
            .iter()
            .find(|option| input_selects_choice(input, option))
            .cloned()
        else {
            self.fiber.status = FlowFiberStatus::Choice(state);
            return;
        };
        let selected = option.id.clone().unwrap_or_else(|| option.label.clone());
        output.flow_events.push(FlowEvent::ChoiceSelected {
            id: state.id.clone(),
            option: selected,
        });
        output.effects.line.extend(option.effects.clone());
        if let Some(out) = option.out {
            output.effects.line.push(LineEffectRequest::Out(out));
        }
        if let Some(target) = option.target {
            self.goto(target, output);
        } else {
            self.fiber.cursor = Some(state.resume);
            self.fiber.status = FlowFiberStatus::Running;
        }
    }

    pub(super) fn await_task_spec(
        &mut self,
        target: &AwaitTarget,
        output: &mut RuntimeStepOutput,
    ) -> Option<TaskSpec> {
        let request = match self.evaluate_host_task_request(target) {
            Ok(request) => request,
            Err(error) => {
                self.fail_eval(error, output);
                return None;
            }
        };
        Some(TaskSpec::new(
            target.task.clone(),
            TaskKey(target.task.0.clone()),
            request.task_class(),
            TaskPriority(0),
            CancelScopeId("flow".to_owned()),
            TaskPolicy::JoinSameKey,
            request,
        ))
    }

    fn evaluate_host_task_request(
        &mut self,
        target: &AwaitTarget,
    ) -> Result<HostTaskRequest, String> {
        let args = self.evaluate_host_task_args(&target.request.args)?;
        let call = EvaluatedHostCall {
            capability: target.request.capability.0.as_str(),
            operation: target.request.operation.as_str(),
            args: &args,
        };
        lower_evaluated_host_request(&call)
    }

    fn evaluate_host_task_args(
        &mut self,
        args: &[HostTaskArgTemplate],
    ) -> Result<Vec<EvaluatedHostArg>, String> {
        let mut evaluated = Vec::new();
        for arg in args {
            let value = self
                .evaluate_expr(arg.value())
                .map_err(|error| error.to_string())?;
            if arg.is_spread() {
                for value in spread_host_arg_values(value)? {
                    evaluated.push(EvaluatedHostArg { name: None, value });
                }
            } else {
                evaluated.push(EvaluatedHostArg {
                    name: arg.name().map(str::to_owned),
                    value,
                });
            }
        }
        Ok(evaluated)
    }
}

struct EvaluatedHostArg {
    name: Option<String>,
    value: RuntimeValue,
}

struct EvaluatedHostCall<'a> {
    capability: &'a str,
    operation: &'a str,
    args: &'a [EvaluatedHostArg],
}

fn spread_host_arg_values(value: RuntimeValue) -> Result<Vec<RuntimeValue>, String> {
    match value {
        RuntimeValue::Tuple(items) | RuntimeValue::BracketSeq(items) => Ok(items),
        value => Err(format!(
            "spread host argument requires a tuple or bracket sequence, found {}",
            super::runtime_value_label(&value)
        )),
    }
}

fn lower_evaluated_host_request(call: &EvaluatedHostCall<'_>) -> Result<HostTaskRequest, String> {
    match (call.capability, call.operation) {
        ("file" | "fs", "read_text") => Ok(HostTaskRequest::FileReadText(FileReadTextRequest {
            path: positional_string(call.args, 0)?,
        })),
        ("file" | "fs", "read_bytes") => Ok(HostTaskRequest::FileReadBytes(FileReadBytesRequest {
            path: positional_string(call.args, 0)?,
        })),
        ("file" | "fs", "write_text") => Ok(HostTaskRequest::FileWriteText(FileWriteTextRequest {
            path: positional_string(call.args, 0)?,
            text: positional_string(call.args, 1)?,
        })),
        ("file" | "fs", "write_bytes") => {
            Ok(HostTaskRequest::FileWriteBytes(FileWriteBytesRequest {
                path: positional_string(call.args, 0)?,
                bytes: positional_bytes(call.args, 1)?,
            }))
        }
        ("file" | "fs", "write") => lower_file_write_request(call.args),
        ("http", "fetch") => Ok(HostTaskRequest::HttpFetch(HttpFetchRequest {
            url: positional_string(call.args, 0)?,
            method: named_string(call.args, "method").unwrap_or_else(|| "GET".to_owned()),
            headers: named_headers(call.args, "headers").unwrap_or_default(),
            body: named_payload(call.args, "body"),
        })),
        ("http", "respond") => Ok(HostTaskRequest::HttpRespond(HttpRespondRequest {
            request_id: positional_string(call.args, 0)?,
            status: named_u16(call.args, "status").unwrap_or(200),
            headers: named_headers(call.args, "headers").unwrap_or_default(),
            body: named_payload(call.args, "body"),
        })),
        ("process", "run") => Ok(HostTaskRequest::ProcessRun(ProcessRunRequest {
            program: positional_string(call.args, 0)?,
            args: named_string_seq(call.args, "args").unwrap_or_default(),
            env: named_headers(call.args, "env").unwrap_or_default(),
        })),
        ("asset", "load") => Ok(HostTaskRequest::AssetLoad(AssetRequest {
            id: positional_string(call.args, 0)?,
            kind: named_string(call.args, "kind").unwrap_or_else(|| "asset".to_owned()),
        })),
        ("asset", kind) => Ok(HostTaskRequest::AssetLoad(AssetRequest {
            id: positional_string(call.args, 0)?,
            kind: kind.to_owned(),
        })),
        ("voice", "load") => Ok(HostTaskRequest::AssetLoad(AssetRequest {
            id: positional_string(call.args, 0)?,
            kind: "voice".to_owned(),
        })),
        ("shader", "compile") => Ok(HostTaskRequest::ShaderCompile(ShaderRequest {
            id: positional_string(call.args, 0)?,
            entry: named_string(call.args, "entry"),
        })),
        ("audio", "decode") => Ok(HostTaskRequest::AudioDecode(AudioDecodeRequest {
            id: positional_string(call.args, 0)?,
        })),
        ("tts", "synthesize" | "synthesis") => Ok(HostTaskRequest::TtsSynthesis(TtsRequest {
            voice: named_string(call.args, "voice"),
            text: named_string(call.args, "text")
                .map_or_else(|| positional_string(call.args, 0), Ok)?,
        })),
        ("wasm", "call") => Ok(HostTaskRequest::WasmCall(WasmCallRequest {
            module: positional_string(call.args, 0)?,
            function: positional_string(call.args, 1)?,
            args: positional_after(call.args, 2)
                .into_iter()
                .map(|value| RuntimePayload::new(value.clone()))
                .collect(),
        })),
        _ => Ok(HostTaskRequest::custom(
            call.capability,
            call.operation,
            call.args
                .iter()
                .map(|arg| RuntimePayload::new(arg.value.clone())),
        )),
    }
}

fn lower_file_write_request(args: &[EvaluatedHostArg]) -> Result<HostTaskRequest, String> {
    let path = positional_string(args, 0)?;
    let body =
        positional_arg(args, 1).ok_or_else(|| "missing positional argument #1".to_owned())?;
    match body {
        RuntimeValue::String(text) => Ok(HostTaskRequest::FileWriteText(FileWriteTextRequest {
            path,
            text: text.clone(),
        })),
        RuntimeValue::BracketSeq(_) => Ok(HostTaskRequest::FileWriteBytes(FileWriteBytesRequest {
            path,
            bytes: runtime_value_to_bytes(body)?,
        })),
        value => Err(format!(
            "fs.write body must be String or byte sequence, found {}",
            super::runtime_value_label(value)
        )),
    }
}

fn positional_after(args: &[EvaluatedHostArg], count: usize) -> Vec<&RuntimeValue> {
    args.iter()
        .filter(|arg| arg.name.is_none())
        .skip(count)
        .map(|arg| &arg.value)
        .collect()
}

fn positional_string(args: &[EvaluatedHostArg], index: usize) -> Result<String, String> {
    positional_arg(args, index)
        .map(runtime_value_to_string)
        .ok_or_else(|| format!("missing positional task argument {index}"))
}

fn positional_bytes(args: &[EvaluatedHostArg], index: usize) -> Result<Vec<u8>, String> {
    let Some(value) = positional_arg(args, index) else {
        return Err(format!("missing positional task argument {index}"));
    };
    runtime_value_to_bytes(value)
}

fn positional_arg(args: &[EvaluatedHostArg], index: usize) -> Option<&RuntimeValue> {
    args.iter()
        .filter(|arg| arg.name.is_none())
        .nth(index)
        .map(|arg| &arg.value)
}

fn named_arg<'a>(args: &'a [EvaluatedHostArg], name: &str) -> Option<&'a RuntimeValue> {
    args.iter()
        .find(|arg| arg.name.as_deref() == Some(name))
        .map(|arg| &arg.value)
}

fn named_string(args: &[EvaluatedHostArg], name: &str) -> Option<String> {
    named_arg(args, name).map(runtime_value_to_string)
}

fn named_payload(args: &[EvaluatedHostArg], name: &str) -> Option<RuntimePayload> {
    named_arg(args, name).cloned().map(RuntimePayload::new)
}

fn named_u16(args: &[EvaluatedHostArg], name: &str) -> Option<u16> {
    named_arg(args, name).and_then(|value| match value {
        RuntimeValue::Int(value) => u16::try_from(*value).ok(),
        value => runtime_value_to_string(value).parse().ok(),
    })
}

fn named_string_seq(args: &[EvaluatedHostArg], name: &str) -> Option<Vec<String>> {
    named_arg(args, name).map(|value| match value {
        RuntimeValue::BracketSeq(items) | RuntimeValue::Tuple(items) => {
            items.iter().map(runtime_value_to_string).collect()
        }
        value => vec![runtime_value_to_string(value)],
    })
}

fn named_headers(args: &[EvaluatedHostArg], name: &str) -> Option<Vec<(String, String)>> {
    named_arg(args, name).and_then(runtime_value_to_headers)
}

fn runtime_value_to_headers(value: &RuntimeValue) -> Option<Vec<(String, String)>> {
    match value {
        RuntimeValue::Record(fields) => fields
            .iter()
            .map(|field| Some((field.name.clone(), runtime_value_to_string(&field.value))))
            .collect(),
        RuntimeValue::BracketSeq(items) | RuntimeValue::Tuple(items) => {
            items.iter().map(runtime_value_to_header_pair).collect()
        }
        _ => None,
    }
}

fn runtime_value_to_header_pair(value: &RuntimeValue) -> Option<(String, String)> {
    match value {
        RuntimeValue::Tuple(items) if items.len() == 2 => Some((
            runtime_value_to_string(&items[0]),
            runtime_value_to_string(&items[1]),
        )),
        RuntimeValue::Record(fields) => {
            let key = record_field(fields, "key")
                .or_else(|| record_field(fields, "name"))
                .map(runtime_value_to_string)?;
            let value = record_field(fields, "value").map(runtime_value_to_string)?;
            Some((key, value))
        }
        _ => None,
    }
}

fn record_field<'a>(fields: &'a [RuntimeFieldValue], name: &str) -> Option<&'a RuntimeValue> {
    fields
        .iter()
        .find(|field| field.name == name)
        .map(|field| &field.value)
}

fn runtime_value_to_bytes(value: &RuntimeValue) -> Result<Vec<u8>, String> {
    let RuntimeValue::BracketSeq(items) = value else {
        return Err("byte payload must be a bracket sequence".to_owned());
    };
    items
        .iter()
        .map(|item| match item {
            RuntimeValue::Int(value) => u8::try_from(*value)
                .map_err(|_| format!("byte value `{value}` is outside u8 range")),
            value => Err(format!(
                "byte payload item must be Int, found {}",
                super::runtime_value_label(value)
            )),
        })
        .collect()
}

fn runtime_value_to_string(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::String(value)
        | RuntimeValue::EntityRef(value)
        | RuntimeValue::Float(value) => value.clone(),
        RuntimeValue::Char(value) => value.to_string(),
        RuntimeValue::Int(value) => value.to_string(),
        RuntimeValue::Bool(value) => value.to_string(),
        RuntimeValue::Duration(value) => format!("{}ns", value.as_nanos()),
        RuntimeValue::Unit
        | RuntimeValue::Tuple(_)
        | RuntimeValue::BracketSeq(_)
        | RuntimeValue::Record(_)
        | RuntimeValue::Variant { .. } => super::runtime_value_label(value),
    }
}

fn input_selects_choice(input: &RuntimeStepInput, option: &ChoiceRuntimeOption) -> bool {
    input.input_events.iter().any(|event| {
        let Some(payload) = event.payload.as_deref() else {
            return false;
        };
        matches!(event.kind.as_str(), "choice" | "select")
            && (option.id.as_deref() == Some(payload) || option.label == payload)
    })
}
