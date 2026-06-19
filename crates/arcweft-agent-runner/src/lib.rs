//! Controller runner boundaries for compiled Agent Script programs.
//!
//! This crate does not interpret `.awfagent` source and does not own CLI, MCP,
//! renderer, database, filesystem, or transport I/O. It coordinates typed host
//! requests emitted by a controller VM with an `AgentSession`, debug sink, and
//! RAG service.

use arcweft_agent_protocol::{
    AgentResource,
    ids::{AgentResourceUri, AgentRunId, PublicId, SessionId},
    predicate::{CompareOp, Predicate, Probe},
    protocol::{
        ActionResult, AgentAction, AgentHostRequest, AgentHostResponse, AgentSessionInfo,
        CaptureFormat, CaptureRequest, CaptureResult, CaptureTarget, ObservationEnvelope,
        ObserveRequest, RagRequest, WaitRequest,
    },
    value::AgentValue,
};
use arcweft_bundle::{ArcweftBundle, BundleKind};
use arcweft_core::{
    bytecode::BytecodeProgram,
    effect::{LineEffectRequest, RuntimeCall},
    engine::FlowFiberStatus,
    executor::{BytecodeVmExecutor, RuntimeExecutor},
    plan::RuntimePlanError,
    step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
    task::{HostTaskRequest, LogicalEpoch, TaskEvent, TaskEventKind, TaskSequence},
    value::{RuntimeFieldValue, RuntimePayload, RuntimeValue},
};
use arcweft_debug_model::{
    event::{DebugEvent, DebugEventKind},
    rag::RagContextPack,
    sink::DebugEventSink,
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const AGENT_NAMED_ARGS_VARIANT: &str = "named_args";

/// Target application boundary used by the controller runner.
pub trait AgentSession {
    type Error: std::error::Error + Send + Sync + 'static;

    fn info(&mut self) -> Result<AgentSessionInfo, Self::Error>;

    fn observe(&mut self, request: ObserveRequest) -> Result<ObservationEnvelope, Self::Error>;

    fn act(&mut self, action: AgentAction) -> Result<ActionResult, Self::Error>;

    fn capture(&mut self, request: CaptureRequest) -> Result<CaptureResult, Self::Error>;

    fn read_resource(&mut self, uri: &str) -> Result<AgentResource, Self::Error>;

    fn step_frames(&mut self, count: u32) -> Result<ObservationEnvelope, Self::Error>;
}

/// Deterministic retrieval boundary used by `rag.query`.
pub trait RagService {
    type Error: std::error::Error + Send + Sync + 'static;

    fn query(&mut self, request: RagRequest) -> Result<RagContextPack, Self::Error>;
}

/// RAG service used when retrieval is disabled by policy.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopRagService;

impl RagService for NoopRagService {
    type Error = std::convert::Infallible;

    fn query(&mut self, _request: RagRequest) -> Result<RagContextPack, Self::Error> {
        Ok(RagContextPack {
            schema_version: 1,
            query: arcweft_debug_model::rag::RagQuery {
                query_id: "noop".to_owned(),
                text: String::new(),
                program_hash: arcweft_agent_protocol::ids::StableHash::new("noop")
                    .expect("static noop hash is nonempty"),
                roots: Vec::new(),
                graph_depth: 0,
                limit: 0,
                max_context_bytes: 0,
            },
            items: Vec::new(),
            truncated: false,
        })
    }
}

/// Runtime policy resolved from compiled effects and launch profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAgentPolicy {
    allowed: BTreeSet<RuntimeAgentCapability>,
}

/// Capability that may be granted to an Agent controller at runtime.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RuntimeAgentCapability {
    Observe,
    Act,
    Capture,
    ResourceRead,
    Rag,
}

/// Runner configuration that must remain deterministic under replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunnerConfig {
    pub session_id: SessionId,
    pub run_id: Option<AgentRunId>,
    pub created_unix_ms: i64,
}

/// Deterministic controller-bytecode execution limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentControllerRunConfig {
    pub max_steps: usize,
    pub max_ops_per_step: usize,
}

/// Host-call execution report for the current vertical slice.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentHostCallReport {
    pub response: AgentHostResponse,
    pub events_emitted: u64,
}

/// Summary returned after running one Agent controller bytecode program.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentControllerRunReport {
    pub steps: usize,
    pub host_calls: usize,
    pub responses: Vec<AgentHostResponse>,
    pub events_emitted: u64,
    pub final_status: Option<FlowFiberStatus>,
}

/// Runner for Agent controller host calls.
pub struct AgentRunner<S, D, R> {
    session: S,
    debug: D,
    rag: R,
    policy: RuntimeAgentPolicy,
    config: AgentRunnerConfig,
    sequence: u64,
}

/// Agent runner failure.
#[derive(Debug, Error)]
pub enum AgentRunError<SessionError, DebugError, RagError>
where
    SessionError: std::error::Error + Send + Sync + 'static,
    DebugError: std::error::Error + Send + Sync + 'static,
    RagError: std::error::Error + Send + Sync + 'static,
{
    #[error("Agent host request is denied by runtime policy: {0}")]
    PolicyDenied(&'static str),
    #[error("Agent session failed: {0}")]
    Session(#[source] SessionError),
    #[error("Agent debug sink failed: {0}")]
    Debug(#[source] DebugError),
    #[error("Agent RAG service failed: {0}")]
    Rag(#[source] RagError),
    #[error("Agent controller bytecode is invalid: {0}")]
    Bytecode(#[source] RuntimePlanError),
    #[error("bundle is not an Agent controller bundle")]
    NotAgentControllerBundle,
    #[error("Agent controller bundle is missing its Agent artifact manifest")]
    MissingAgentManifest,
    #[error("Agent controller emitted unsupported effect: {0}")]
    UnsupportedControllerEffect(String),
    #[error("Agent controller failed: {0}")]
    ControllerFailed(String),
    #[error("Agent controller exceeded execution step budget of {max_steps}")]
    ControllerBudgetExceeded { max_steps: usize },
    #[error("Agent wait timed out after {timeout_millis} ms")]
    WaitTimeout { timeout_millis: u64 },
}

impl Default for RuntimeAgentPolicy {
    fn default() -> Self {
        Self::new([RuntimeAgentCapability::Observe])
    }
}

impl RuntimeAgentPolicy {
    pub fn new(capabilities: impl IntoIterator<Item = RuntimeAgentCapability>) -> Self {
        Self {
            allowed: capabilities.into_iter().collect(),
        }
    }

    pub fn allows(&self, capability: RuntimeAgentCapability) -> bool {
        self.allowed.contains(&capability)
    }
}

impl RuntimeAgentCapability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "agent.observe",
            Self::Act => "agent.act",
            Self::Capture => "agent.capture",
            Self::ResourceRead => "agent.resource.read",
            Self::Rag => "agent.rag.query",
        }
    }
}

/// Result type for runner operations.
pub type AgentRunnerResult<T, S, D, R> = Result<
    T,
    AgentRunError<
        <S as AgentSession>::Error,
        <D as DebugEventSink>::Error,
        <R as RagService>::Error,
    >,
>;

impl AgentRunnerConfig {
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            run_id: None,
            created_unix_ms: 0,
        }
    }

    #[must_use]
    pub fn with_run_id(mut self, run_id: AgentRunId) -> Self {
        self.run_id = Some(run_id);
        self
    }

    #[must_use]
    pub const fn with_created_unix_ms(mut self, created_unix_ms: i64) -> Self {
        self.created_unix_ms = created_unix_ms;
        self
    }
}

impl Default for AgentControllerRunConfig {
    fn default() -> Self {
        Self {
            max_steps: 256,
            max_ops_per_step: 1024,
        }
    }
}

impl<S, D, R> AgentRunner<S, D, R>
where
    S: AgentSession,
    D: DebugEventSink,
    R: RagService,
{
    pub fn new(
        session: S,
        debug: D,
        rag: R,
        policy: RuntimeAgentPolicy,
        config: AgentRunnerConfig,
    ) -> Self {
        Self {
            session,
            debug,
            rag,
            policy,
            config,
            sequence: 0,
        }
    }

    pub fn session_mut(&mut self) -> &mut S {
        &mut self.session
    }

    pub fn debug_mut(&mut self) -> &mut D {
        &mut self.debug
    }

    pub fn rag_mut(&mut self) -> &mut R {
        &mut self.rag
    }

    pub fn handle_host_request(
        &mut self,
        request: AgentHostRequest,
    ) -> AgentRunnerResult<AgentHostCallReport, S, D, R> {
        self.emit(DebugEventKind::StepStarted, None, serde_json::json!({}))?;
        let response = match request {
            AgentHostRequest::Observe(request) => {
                self.ensure(RuntimeAgentCapability::Observe)?;
                let observation = self
                    .session
                    .observe(*request)
                    .map_err(AgentRunError::Session)?;
                self.emit(
                    DebugEventKind::Observation,
                    Some(observation.tick),
                    serde_json::to_value(&observation).unwrap_or(serde_json::Value::Null),
                )?;
                AgentHostResponse::Observation(Box::new(observation))
            }
            AgentHostRequest::Act(action) => {
                self.ensure(RuntimeAgentCapability::Act)?;
                let result = self.session.act(*action).map_err(AgentRunError::Session)?;
                self.emit(
                    DebugEventKind::Action,
                    Some(result.after_tick),
                    serde_json::to_value(&result).unwrap_or(serde_json::Value::Null),
                )?;
                AgentHostResponse::Action(Box::new(result))
            }
            AgentHostRequest::Wait(request) => {
                let observation = self.wait(&request)?;
                AgentHostResponse::Observation(Box::new(observation))
            }
            AgentHostRequest::Capture(request) => {
                self.ensure(RuntimeAgentCapability::Capture)?;
                let result = self
                    .session
                    .capture(*request)
                    .map_err(AgentRunError::Session)?;
                self.emit(
                    DebugEventKind::Capture,
                    None,
                    serde_json::to_value(&result).unwrap_or(serde_json::Value::Null),
                )?;
                AgentHostResponse::Capture(Box::new(result))
            }
            AgentHostRequest::ReadResource { uri } => {
                self.ensure(RuntimeAgentCapability::ResourceRead)?;
                let resource = self
                    .session
                    .read_resource(uri.as_str())
                    .map_err(AgentRunError::Session)?;
                AgentHostResponse::Resource(Box::new(
                    serde_json::to_value(resource).unwrap_or(serde_json::Value::Null),
                ))
            }
            AgentHostRequest::RagQuery(request) => {
                self.ensure(RuntimeAgentCapability::Rag)?;
                let context = self.rag.query(*request).map_err(AgentRunError::Rag)?;
                self.emit(
                    DebugEventKind::RagQuery,
                    None,
                    serde_json::to_value(&context).unwrap_or(serde_json::Value::Null),
                )?;
                AgentHostResponse::RagContext(Box::new(
                    serde_json::to_value(context).unwrap_or(serde_json::Value::Null),
                ))
            }
            AgentHostRequest::Checkpoint { name } => {
                self.emit(
                    DebugEventKind::Diagnostic,
                    None,
                    serde_json::json!({ "checkpoint": name }),
                )?;
                AgentHostResponse::Unit
            }
        };
        self.emit(DebugEventKind::StepFinished, None, serde_json::json!({}))?;
        Ok(AgentHostCallReport {
            response,
            events_emitted: self.sequence,
        })
    }

    /// Runs one compiled Agent controller bytecode program and dispatches
    /// Agent host calls in source/runtime order.
    pub fn run_controller_bytecode(
        &mut self,
        program: BytecodeProgram,
        config: AgentControllerRunConfig,
    ) -> AgentRunnerResult<AgentControllerRunReport, S, D, R> {
        let mut executor = BytecodeVmExecutor::new(program).map_err(AgentRunError::Bytecode)?;
        let options = RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget {
                max_ops: config.max_ops_per_step,
            },
        };
        let mut report = AgentControllerRunReport {
            steps: 0,
            host_calls: 0,
            responses: Vec::new(),
            events_emitted: self.sequence,
            final_status: None,
        };
        let mut task_events = Vec::new();

        while report.steps < config.max_steps {
            report.steps += 1;
            let step = executor.step(
                RuntimeStepInput {
                    task_events: std::mem::take(&mut task_events),
                    ..RuntimeStepInput::default()
                },
                options,
            );
            for effect in &step.output.effects.line {
                let request = agent_host_request_from_effect(effect)
                    .map_err(AgentRunError::UnsupportedControllerEffect)?;
                let host_report = self.handle_host_request(request)?;
                report.host_calls += 1;
                report.responses.push(host_report.response);
                report.events_emitted = host_report.events_emitted;
            }
            for task in &step.output.requests.tasks {
                let request = agent_host_request_from_task(&task.request)
                    .map_err(AgentRunError::UnsupportedControllerEffect)?;
                let host_report = self.handle_host_request(request)?;
                task_events.push(TaskEvent {
                    logical_epoch: LogicalEpoch(0),
                    task_id: task.id.clone(),
                    sequence: TaskSequence(report.host_calls as u64),
                    kind: TaskEventKind::Ready(runtime_payload_from_response(
                        &host_report.response,
                    )),
                });
                report.host_calls += 1;
                report.responses.push(host_report.response);
                report.events_emitted = host_report.events_emitted;
            }
            report.final_status = Some(step.fiber_status.clone());

            match step.fiber_status {
                FlowFiberStatus::Done(_) => return Ok(report),
                FlowFiberStatus::Failed(message) => {
                    return Err(AgentRunError::ControllerFailed(message));
                }
                FlowFiberStatus::Running
                | FlowFiberStatus::Waiting(_)
                | FlowFiberStatus::WaitingMany(_)
                | FlowFiberStatus::Choice(_) => {}
            }
        }

        Err(AgentRunError::ControllerBudgetExceeded {
            max_steps: config.max_steps,
        })
    }

    /// Runs a decoded `.awfb` Agent controller bundle through the shared
    /// bytecode VM.
    pub fn run_controller_bundle(
        &mut self,
        bundle: &ArcweftBundle,
        config: AgentControllerRunConfig,
    ) -> AgentRunnerResult<AgentControllerRunReport, S, D, R> {
        if bundle.bundle_kind != BundleKind::AgentController {
            return Err(AgentRunError::NotAgentControllerBundle);
        }
        if bundle.agent.is_none() {
            return Err(AgentRunError::MissingAgentManifest);
        }
        self.run_controller_bytecode(bundle.bytecode.program.clone(), config)
    }

    fn wait(&mut self, request: &WaitRequest) -> AgentRunnerResult<ObservationEnvelope, S, D, R> {
        self.ensure(RuntimeAgentCapability::Observe)?;
        let poll_frames = request.poll_frames.max(1);
        let stable_frames = request.stable_frames.max(1);
        let max_polls = (request.timeout_millis / u64::from(poll_frames)).max(1);
        let mut stable_count = 0;
        let mut last_observation = None;

        for _ in 0..max_polls {
            let observation = self
                .session
                .step_frames(poll_frames)
                .map_err(AgentRunError::Session)?;
            if predicate_matches(&request.predicate, &observation) {
                stable_count += 1;
                if stable_count >= stable_frames {
                    self.emit(
                        DebugEventKind::Observation,
                        Some(observation.tick),
                        serde_json::to_value(&observation).unwrap_or(serde_json::Value::Null),
                    )?;
                    return Ok(observation);
                }
            } else {
                stable_count = 0;
            }
            last_observation = Some(observation);
        }

        if let Some(observation) = last_observation {
            self.emit(
                DebugEventKind::Observation,
                Some(observation.tick),
                serde_json::to_value(&observation).unwrap_or(serde_json::Value::Null),
            )?;
        }
        Err(AgentRunError::WaitTimeout {
            timeout_millis: request.timeout_millis,
        })
    }

    fn ensure(&self, capability: RuntimeAgentCapability) -> AgentRunnerResult<(), S, D, R> {
        self.policy
            .allows(capability)
            .then_some(())
            .ok_or(AgentRunError::PolicyDenied(capability.as_str()))
    }

    fn emit(
        &mut self,
        kind: DebugEventKind,
        tick: Option<u64>,
        payload: serde_json::Value,
    ) -> AgentRunnerResult<(), S, D, R> {
        self.sequence += 1;
        let event = DebugEvent {
            schema_version: 1,
            session_id: self.config.session_id.clone(),
            run_id: self.config.run_id.clone(),
            sequence: self.sequence,
            tick,
            kind,
            payload,
            created_unix_ms: self.config.created_unix_ms,
        };
        self.debug.append(&event).map_err(AgentRunError::Debug)
    }
}

fn agent_host_request_from_effect(effect: &LineEffectRequest) -> Result<AgentHostRequest, String> {
    match effect {
        LineEffectRequest::Call(call) => agent_host_request_from_call(call),
        other => Err(format!("{other:?}")),
    }
}

fn agent_host_request_from_call(call: &RuntimeCall) -> Result<AgentHostRequest, String> {
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
        other => Err(format!("unsupported Agent call `{other}`")),
    }
}

fn agent_host_request_from_task(request: &HostTaskRequest) -> Result<AgentHostRequest, String> {
    let HostTaskRequest::Custom {
        capability,
        operation,
        args,
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
        "observe" => runtime_observe_request(&args)
            .map(|request| AgentHostRequest::Observe(Box::new(request))),
        "capture" => runtime_capture_request(&args)
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
        "invoke" => {
            runtime_invoke_action(&args).map(|action| AgentHostRequest::Act(Box::new(action)))
        }
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
        "rag.query" => {
            runtime_rag_request(&args).map(|request| AgentHostRequest::RagQuery(Box::new(request)))
        }
        "checkpoint" => {
            let name = args
                .positional(0)
                .or_else(|| args.named("name"))
                .map_or_else(|| Ok("checkpoint".to_owned()), runtime_string)?;
            Ok(AgentHostRequest::Checkpoint { name })
        }
        "wait" => {
            runtime_wait_request(&args).map(|request| AgentHostRequest::Wait(Box::new(request)))
        }
        other => Err(format!("unsupported Agent task operation `{other}`")),
    }
}

#[derive(Debug)]
struct RuntimeAgentArgs<'a> {
    positionals: Vec<&'a RuntimeValue>,
    named: BTreeMap<String, &'a RuntimeValue>,
}

impl<'a> RuntimeAgentArgs<'a> {
    fn new(args: &'a [RuntimePayload]) -> Self {
        let mut positionals = Vec::new();
        let mut named = BTreeMap::new();
        for arg in args {
            match arg.value() {
                RuntimeValue::Variant {
                    path,
                    name,
                    payload: Some(payload),
                } if path.as_deref() == Some("agent") && name == AGENT_NAMED_ARGS_VARIANT => {
                    let RuntimeValue::Record(fields) = payload.as_ref() else {
                        positionals.push(arg.value());
                        continue;
                    };
                    named.extend(
                        fields
                            .iter()
                            .map(|field| (field.name.clone(), &field.value)),
                    );
                }
                value => positionals.push(value),
            }
        }
        Self { positionals, named }
    }

    fn positional(&self, index: usize) -> Option<&'a RuntimeValue> {
        self.positionals.get(index).copied()
    }

    fn named(&self, name: &str) -> Option<&'a RuntimeValue> {
        self.named.get(name).copied()
    }

    fn named_any(&self, names: &[&str]) -> Option<&'a RuntimeValue> {
        names.iter().find_map(|name| self.named(name))
    }
}

fn runtime_observe_request(args: &RuntimeAgentArgs<'_>) -> Result<ObserveRequest, String> {
    if !args.positionals.is_empty() {
        return Err("observe does not accept positional arguments".to_owned());
    }
    Ok(ObserveRequest {
        include_images: args
            .named("include_images")
            .map_or(Ok(false), runtime_bool)?,
        include_objects: args
            .named("include_objects")
            .map_or(Ok(false), runtime_bool)?,
        include_logs: args.named("include_logs").map_or(Ok(false), runtime_bool)?,
    })
}

fn runtime_capture_request(args: &RuntimeAgentArgs<'_>) -> Result<CaptureRequest, String> {
    let target = args
        .positional(0)
        .ok_or_else(|| "capture requires a target argument".to_owned())
        .and_then(runtime_capture_target)?;
    Ok(CaptureRequest {
        target,
        format: args
            .named("format")
            .map_or(Ok(CaptureFormat::Png), runtime_capture_format)?,
        capture_kind: args
            .named_any(&["capture_kind", "kind"])
            .map_or_else(|| Ok("color".to_owned()), runtime_string)?,
        name: args
            .named("name")
            .map_or_else(|| Ok("capture".to_owned()), runtime_string)?,
    })
}

fn runtime_rag_request(args: &RuntimeAgentArgs<'_>) -> Result<RagRequest, String> {
    let query = args
        .positional(0)
        .or_else(|| args.named("query"))
        .ok_or_else(|| "rag.query requires a query argument".to_owned())
        .and_then(runtime_string)?;
    Ok(RagRequest {
        query,
        roots: match args.named("roots") {
            Some(value) => runtime_public_ids(value)?,
            None => Vec::new(),
        },
        graph_depth: args.named("graph_depth").map_or(Ok(1), runtime_u32)?,
        limit: args.named("limit").map_or(Ok(8), runtime_usize)?,
    })
}

fn runtime_wait_request(args: &RuntimeAgentArgs<'_>) -> Result<WaitRequest, String> {
    let predicate = args
        .positional(0)
        .or_else(|| args.named("predicate"))
        .ok_or_else(|| "wait requires a predicate argument".to_owned())
        .and_then(runtime_predicate)?;
    let timeout_millis = args
        .positional(1)
        .or_else(|| args.named("timeout"))
        .ok_or_else(|| "wait requires timeout".to_owned())
        .and_then(runtime_duration_millis)?;
    Ok(WaitRequest {
        predicate,
        timeout_millis,
        stable_frames: args.named("stable_frames").map_or(Ok(1), runtime_u32)?,
        poll_frames: args.named("poll_frames").map_or(Ok(1), runtime_u32)?,
    })
}

fn runtime_invoke_action(args: &RuntimeAgentArgs<'_>) -> Result<AgentAction, String> {
    let target = args
        .positional(0)
        .or_else(|| args.named("target"))
        .ok_or_else(|| "invoke requires a target argument".to_owned())
        .and_then(runtime_public_id)?;
    let action = args
        .positional(1)
        .or_else(|| args.named("action"))
        .ok_or_else(|| "invoke requires an action argument".to_owned())
        .and_then(runtime_string)?;
    let call_args = match args.named("args") {
        Some(value) => runtime_agent_value_map(value)?,
        None => BTreeMap::new(),
    };
    Ok(AgentAction::Invoke {
        target,
        action,
        args: call_args,
    })
}

fn runtime_predicate(value: &RuntimeValue) -> Result<Predicate, String> {
    let fields = runtime_record_fields(value, "predicate")?;
    match runtime_record_string(fields, "kind")?.as_str() {
        "compare" => Ok(Predicate::Compare {
            probe: runtime_record_get(fields, "probe").and_then(runtime_probe)?,
            op: runtime_record_get(fields, "op").and_then(runtime_compare_op)?,
            value: runtime_record_get(fields, "value").and_then(runtime_agent_value)?,
        }),
        "exists" => Ok(Predicate::Exists {
            probe: runtime_record_get(fields, "probe").and_then(runtime_probe)?,
        }),
        "all" => runtime_record_get(fields, "predicates")
            .and_then(runtime_predicate_list)
            .map(|predicates| Predicate::All { predicates }),
        "any" => runtime_record_get(fields, "predicates")
            .and_then(runtime_predicate_list)
            .map(|predicates| Predicate::Any { predicates }),
        "not" => runtime_record_get(fields, "predicate")
            .and_then(runtime_predicate)
            .map(|predicate| Predicate::Not {
                predicate: Box::new(predicate),
            }),
        other => Err(format!("unsupported predicate kind `{other}`")),
    }
}

fn runtime_predicate_list(value: &RuntimeValue) -> Result<Vec<Predicate>, String> {
    let RuntimeValue::Tuple(values) = value else {
        return Err(format!(
            "expected predicate tuple, got `{}`",
            value_label(value)
        ));
    };
    values.iter().map(runtime_predicate).collect()
}

fn runtime_probe(value: &RuntimeValue) -> Result<Probe, String> {
    let fields = runtime_record_fields(value, "probe")?;
    match runtime_record_string(fields, "kind")?.as_str() {
        "signal" => Ok(Probe::Signal {
            target: runtime_record_get(fields, "target").and_then(runtime_public_id)?,
        }),
        "metric" => Ok(Probe::Metric {
            target: runtime_record_get(fields, "target").and_then(runtime_public_id)?,
        }),
        "state" | "state_path" => Ok(Probe::StatePath {
            path: runtime_record_string(fields, "path")?,
        }),
        "observation" | "observation_field" => Ok(Probe::ObservationField {
            path: runtime_record_string(fields, "path")?,
        }),
        other => Err(format!("unsupported probe kind `{other}`")),
    }
}

fn runtime_compare_op(value: &RuntimeValue) -> Result<CompareOp, String> {
    match runtime_string(value)?.as_str() {
        "eq" => Ok(CompareOp::Eq),
        "not_eq" | "ne" => Ok(CompareOp::NotEq),
        "greater" | "gt" => Ok(CompareOp::Greater),
        "greater_or_equal" | "ge" => Ok(CompareOp::GreaterOrEqual),
        "less" | "lt" => Ok(CompareOp::Less),
        "less_or_equal" | "le" => Ok(CompareOp::LessOrEqual),
        other => Err(format!("unsupported compare op `{other}`")),
    }
}

fn runtime_record_fields<'a>(
    value: &'a RuntimeValue,
    label: &str,
) -> Result<&'a [RuntimeFieldValue], String> {
    let RuntimeValue::Record(fields) = value else {
        return Err(format!(
            "expected {label} record, got `{}`",
            value_label(value)
        ));
    };
    Ok(fields)
}

fn runtime_record_get<'a>(
    fields: &'a [RuntimeFieldValue],
    name: &str,
) -> Result<&'a RuntimeValue, String> {
    fields
        .iter()
        .find(|field| field.name == name)
        .map(|field| &field.value)
        .ok_or_else(|| format!("record is missing `{name}`"))
}

fn runtime_record_string(fields: &[RuntimeFieldValue], name: &str) -> Result<String, String> {
    runtime_record_get(fields, name).and_then(runtime_string)
}

fn runtime_payload_from_response(response: &AgentHostResponse) -> RuntimePayload {
    RuntimePayload::new(match response {
        AgentHostResponse::Observation(observation) => RuntimeValue::Record(vec![
            runtime_field("tick", RuntimeValue::u64(observation.tick)),
            runtime_field(
                "frame_id",
                RuntimeValue::String(observation.frame_id.clone()),
            ),
            runtime_field(
                "state_hash",
                RuntimeValue::String(observation.state_hash.clone()),
            ),
            runtime_field(
                "render_hash",
                RuntimeValue::String(observation.render_hash.clone()),
            ),
        ]),
        AgentHostResponse::Action(result) => RuntimeValue::Record(vec![
            runtime_field("accepted", RuntimeValue::Bool(result.accepted)),
            runtime_field("before_tick", RuntimeValue::u64(result.before_tick)),
            runtime_field("after_tick", RuntimeValue::u64(result.after_tick)),
            runtime_field(
                "before_state_hash",
                RuntimeValue::String(result.before_state_hash.clone()),
            ),
            runtime_field(
                "after_state_hash",
                RuntimeValue::String(result.after_state_hash.clone()),
            ),
        ]),
        AgentHostResponse::Capture(result) => RuntimeValue::Record(vec![
            runtime_field("uri", RuntimeValue::String(result.uri.as_str().to_owned())),
            runtime_field(
                "content_hash",
                RuntimeValue::String(result.content_hash.clone()),
            ),
            runtime_field(
                "media_type",
                RuntimeValue::String(result.media_type.clone()),
            ),
            runtime_field("byte_len", RuntimeValue::u64(result.byte_len)),
        ]),
        AgentHostResponse::Resource(value) | AgentHostResponse::RagContext(value) => {
            RuntimeValue::String(value.to_string())
        }
        AgentHostResponse::Unit => RuntimeValue::Unit,
    })
}

fn runtime_field(name: &str, value: RuntimeValue) -> RuntimeFieldValue {
    RuntimeFieldValue {
        name: name.to_owned(),
        value,
    }
}

fn runtime_string(value: &RuntimeValue) -> Result<String, String> {
    match value {
        RuntimeValue::String(value) | RuntimeValue::EntityRef(value) => Ok(value.clone()),
        RuntimeValue::Variant { name, .. } => Ok(name.clone()),
        other => Err(format!(
            "expected string-like value, got `{}`",
            value_label(other)
        )),
    }
}

fn runtime_bool(value: &RuntimeValue) -> Result<bool, String> {
    match value {
        RuntimeValue::Bool(value) => Ok(*value),
        RuntimeValue::String(value) => parse_bool_label(value),
        other => Err(format!(
            "expected boolean value, got `{}`",
            value_label(other)
        )),
    }
}

fn runtime_u32(value: &RuntimeValue) -> Result<u32, String> {
    match value {
        RuntimeValue::Int(value) => value
            .try_into_i64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| format!("expected u32-compatible integer, got `{}`", value.label())),
        RuntimeValue::UInt(value) => value
            .try_into_u32()
            .ok_or_else(|| format!("expected u32-compatible integer, got `{}`", value.label())),
        RuntimeValue::String(value) => value
            .parse::<u32>()
            .map_err(|_| format!("expected u32-compatible integer, got `{value}`")),
        other => Err(format!(
            "expected integer value, got `{}`",
            value_label(other)
        )),
    }
}

fn runtime_usize(value: &RuntimeValue) -> Result<usize, String> {
    match value {
        RuntimeValue::Int(value) => value
            .try_into_i64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("expected usize-compatible integer, got `{}`", value.label())),
        RuntimeValue::UInt(value) => value
            .try_into_i64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("expected usize-compatible integer, got `{}`", value.label())),
        RuntimeValue::String(value) => value
            .parse::<usize>()
            .map_err(|_| format!("expected usize-compatible integer, got `{value}`")),
        other => Err(format!(
            "expected integer value, got `{}`",
            value_label(other)
        )),
    }
}

fn runtime_duration_millis(value: &RuntimeValue) -> Result<u64, String> {
    match value {
        RuntimeValue::Duration(duration) => {
            let nanos = duration.as_nanos();
            Ok(if nanos == 0 {
                0
            } else {
                nanos.saturating_add(999_999) / 1_000_000
            })
        }
        RuntimeValue::UInt(value) => value
            .exact_u64()
            .or_else(|| {
                value
                    .try_into_i64()
                    .and_then(|value| u64::try_from(value).ok())
            })
            .ok_or_else(|| format!("expected millisecond duration, got `{}`", value.label())),
        RuntimeValue::Int(value) => value
            .try_into_i64()
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| format!("expected millisecond duration, got `{}`", value.label())),
        RuntimeValue::String(value) => value
            .parse::<u64>()
            .map_err(|_| format!("expected millisecond duration, got `{value}`")),
        other => Err(format!(
            "expected duration value, got `{}`",
            value_label(other)
        )),
    }
}

fn runtime_public_id(value: &RuntimeValue) -> Result<PublicId, String> {
    runtime_string(value).and_then(|value| parse_public_id_arg(&value))
}

fn runtime_public_ids(value: &RuntimeValue) -> Result<Vec<PublicId>, String> {
    match value {
        RuntimeValue::Tuple(values) => values.iter().map(runtime_public_id).collect(),
        RuntimeValue::String(value) => parse_public_id_list(value),
        _ => runtime_public_id(value).map(|id| vec![id]),
    }
}

fn runtime_capture_target(value: &RuntimeValue) -> Result<CaptureTarget, String> {
    runtime_string(value).and_then(|value| parse_capture_target(&value))
}

fn runtime_capture_format(value: &RuntimeValue) -> Result<CaptureFormat, String> {
    runtime_string(value).and_then(|value| parse_capture_format(&value))
}

fn runtime_agent_value_map(value: &RuntimeValue) -> Result<BTreeMap<String, AgentValue>, String> {
    let RuntimeValue::Record(fields) = value else {
        return Err(format!(
            "expected record for invoke args, got `{}`",
            value_label(value)
        ));
    };
    fields
        .iter()
        .map(|field| runtime_agent_value(&field.value).map(|value| (field.name.clone(), value)))
        .collect()
}

fn runtime_agent_value(value: &RuntimeValue) -> Result<AgentValue, String> {
    match value {
        RuntimeValue::Unit => Ok(AgentValue::Null),
        RuntimeValue::Bool(value) => Ok(AgentValue::Bool(*value)),
        RuntimeValue::Int(value) => value
            .try_into_i64()
            .map(AgentValue::I64)
            .ok_or_else(|| format!("integer is out of i64 range: `{}`", value.label())),
        RuntimeValue::UInt(value) => value
            .exact_u64()
            .or_else(|| {
                value
                    .try_into_i64()
                    .and_then(|value| u64::try_from(value).ok())
            })
            .map(AgentValue::U64)
            .ok_or_else(|| format!("integer is out of u64 range: `{}`", value.label())),
        RuntimeValue::F32(value) => Ok(AgentValue::F64(f64::from(*value))),
        RuntimeValue::F64(value) => Ok(AgentValue::F64(*value)),
        RuntimeValue::String(value) => Ok(AgentValue::String(value.clone())),
        RuntimeValue::EntityRef(value) => parse_public_id_arg(value).map(AgentValue::Entity),
        RuntimeValue::Tuple(values) => values
            .iter()
            .map(runtime_agent_value)
            .collect::<Result<Vec<_>, _>>()
            .map(AgentValue::List),
        RuntimeValue::Record(fields) => fields
            .iter()
            .map(|field| runtime_agent_value(&field.value).map(|value| (field.name.clone(), value)))
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(AgentValue::Map),
        other => Err(format!("unsupported Agent value `{}`", value_label(other))),
    }
}

fn value_label(value: &RuntimeValue) -> String {
    RuntimePayload::new(value.clone()).label()
}

fn observe_request(args: &[String]) -> Result<ObserveRequest, String> {
    let mut request = ObserveRequest::default();
    for arg in args {
        match named_arg(arg) {
            Some(("include_images", value)) => request.include_images = parse_bool_label(value)?,
            Some(("include_objects", value)) => request.include_objects = parse_bool_label(value)?,
            Some(("include_logs", value)) => request.include_logs = parse_bool_label(value)?,
            Some((name, _)) => return Err(format!("observe has no parameter named `{name}`")),
            None => {
                return Err(format!(
                    "observe does not accept positional argument `{arg}`"
                ));
            }
        }
    }
    Ok(request)
}

fn capture_request(args: &[String]) -> Result<CaptureRequest, String> {
    let target = args
        .first()
        .ok_or_else(|| "capture requires a target argument".to_owned())
        .and_then(|arg| parse_capture_target(arg))?;
    let mut request = CaptureRequest {
        target,
        format: CaptureFormat::Png,
        capture_kind: "color".to_owned(),
        name: "capture".to_owned(),
    };
    for arg in args.iter().skip(1) {
        match named_arg(arg) {
            Some(("format", value)) => request.format = parse_capture_format(value)?,
            Some(("capture_kind" | "kind", value)) => {
                request.capture_kind =
                    parse_string_label(value).unwrap_or_else(|| value.to_owned());
            }
            Some(("name", value)) => {
                request.name = parse_string_label(value).unwrap_or_else(|| value.to_owned());
            }
            Some((name, _)) => return Err(format!("capture has no parameter named `{name}`")),
            None => {
                return Err(format!(
                    "capture does not accept extra positional argument `{arg}`"
                ));
            }
        }
    }
    Ok(request)
}

fn rag_request(args: &[String]) -> Result<RagRequest, String> {
    let query = args
        .first()
        .and_then(|arg| parse_string_label(arg))
        .ok_or_else(|| "rag.query requires a string query argument".to_owned())?;
    let mut request = RagRequest {
        query,
        roots: Vec::new(),
        graph_depth: 1,
        limit: 8,
    };
    for arg in args.iter().skip(1) {
        match named_arg(arg) {
            Some(("graph_depth", value)) => {
                request.graph_depth = value
                    .parse::<u32>()
                    .map_err(|_| format!("invalid rag.query graph_depth `{value}`"))?;
            }
            Some(("limit", value)) => {
                request.limit = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid rag.query limit `{value}`"))?;
            }
            Some(("roots", value)) => request.roots = parse_public_id_list(value)?,
            Some((name, _)) => return Err(format!("rag.query has no parameter named `{name}`")),
            None => {
                return Err(format!(
                    "rag.query does not accept extra positional argument `{arg}`"
                ));
            }
        }
    }
    Ok(request)
}

fn wait_request(args: &[String]) -> Result<WaitRequest, String> {
    let predicate = args
        .first()
        .ok_or_else(|| "wait requires a predicate argument".to_owned())
        .and_then(|arg| parse_predicate_label(arg))?;
    let mut request = WaitRequest {
        predicate,
        timeout_millis: 0,
        stable_frames: 1,
        poll_frames: 1,
    };
    for arg in args.iter().skip(1) {
        match named_arg(arg) {
            Some(("timeout", value)) => {
                request.timeout_millis = parse_duration_millis_label(value)?;
            }
            Some(("stable_frames", value)) => request.stable_frames = parse_u32_label(value)?,
            Some(("poll_frames", value)) => request.poll_frames = parse_u32_label(value)?,
            Some((name, _)) => return Err(format!("wait has no parameter named `{name}`")),
            None => {
                return Err(format!(
                    "wait does not accept extra positional argument `{arg}`"
                ));
            }
        }
    }
    if request.timeout_millis == 0 {
        return Err("wait requires timeout".to_owned());
    }
    Ok(request)
}

fn parse_predicate_label(value: &str) -> Result<Predicate, String> {
    let (probe, method_call) = value
        .split_once(").")
        .ok_or_else(|| format!("unsupported wait predicate `{value}`"))?;
    let probe = parse_probe_label(&format!("{probe})"))?;
    let (method, expected) = method_call
        .split_once('(')
        .and_then(|(method, rest)| rest.strip_suffix(')').map(|rest| (method, rest)))
        .ok_or_else(|| format!("unsupported wait predicate `{value}`"))?;
    Ok(Predicate::Compare {
        probe,
        op: parse_compare_op_label(method)?,
        value: parse_agent_value_label(expected)?,
    })
}

fn parse_probe_label(value: &str) -> Result<Probe, String> {
    if let Some(target) = value
        .strip_prefix("signal(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return parse_public_id_arg(target).map(|target| Probe::Signal { target });
    }
    if let Some(target) = value
        .strip_prefix("metric(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return parse_public_id_arg(target).map(|target| Probe::Metric { target });
    }
    Err(format!("unsupported probe `{value}`"))
}

fn parse_compare_op_label(value: &str) -> Result<CompareOp, String> {
    match value {
        "eq" => Ok(CompareOp::Eq),
        "not_eq" | "ne" => Ok(CompareOp::NotEq),
        "gt" | "greater" => Ok(CompareOp::Greater),
        "ge" | "greater_or_equal" => Ok(CompareOp::GreaterOrEqual),
        "lt" | "less" => Ok(CompareOp::Less),
        "le" | "less_or_equal" => Ok(CompareOp::LessOrEqual),
        other => Err(format!("unsupported compare op `{other}`")),
    }
}

fn parse_agent_value_label(value: &str) -> Result<AgentValue, String> {
    match value {
        "true" => Ok(AgentValue::Bool(true)),
        "false" => Ok(AgentValue::Bool(false)),
        value if value.starts_with('@') => parse_public_id_arg(value).map(AgentValue::Entity),
        value => value.parse::<i64>().map_or_else(
            |_| {
                Ok(AgentValue::String(
                    parse_string_label(value).unwrap_or_else(|| value.to_owned()),
                ))
            },
            |value| Ok(AgentValue::I64(value)),
        ),
    }
}

fn parse_duration_millis_label(value: &str) -> Result<u64, String> {
    if let Some(amount) = value.strip_suffix("ms") {
        return parse_integer_millis(amount, value);
    }
    if let Some(amount) = value.strip_suffix('s') {
        return parse_seconds_millis(amount, value);
    }
    Err(format!("expected duration literal, got `{value}`"))
}

fn parse_integer_millis(amount: &str, original: &str) -> Result<u64, String> {
    if amount.contains('.') {
        return Err(format!("invalid duration literal `{original}`"));
    }
    amount
        .parse::<u64>()
        .map_err(|_| format!("invalid duration literal `{original}`"))
}

fn parse_seconds_millis(amount: &str, original: &str) -> Result<u64, String> {
    let (seconds, fraction) = amount
        .split_once('.')
        .map_or((amount, None), |(seconds, fraction)| {
            (seconds, Some(fraction))
        });
    let whole = seconds
        .parse::<u64>()
        .map_err(|_| format!("invalid duration literal `{original}`"))?;
    let millis = whole
        .checked_mul(1_000)
        .ok_or_else(|| format!("duration literal `{original}` is too large"))?;
    match fraction {
        Some(fraction)
            if !fraction.is_empty()
                && fraction.len() <= 3
                && fraction.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            let padded_fraction = format!("{fraction:0<3}");
            let fractional_millis = padded_fraction
                .parse::<u64>()
                .map_err(|_| format!("invalid duration literal `{original}`"))?;
            millis
                .checked_add(fractional_millis)
                .ok_or_else(|| format!("duration literal `{original}` is too large"))
        }
        Some(_) => Err(format!("invalid duration literal `{original}`")),
        None => Ok(millis),
    }
}

fn parse_u32_label(value: &str) -> Result<u32, String> {
    value
        .strip_suffix("u32")
        .unwrap_or(value)
        .parse::<u32>()
        .map_err(|_| format!("expected u32 literal, got `{value}`"))
}

fn named_arg(arg: &str) -> Option<(&str, &str)> {
    arg.split_once(" = ")
        .map(|(name, value)| (name.trim(), value.trim()))
}

fn parse_bool_label(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("expected boolean literal, got `{value}`")),
    }
}

fn parse_string_label(value: &str) -> Option<String> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(str::to_owned)
}

fn parse_public_id_arg(value: &str) -> Result<PublicId, String> {
    let id = value.strip_prefix('@').unwrap_or(value);
    PublicId::new(id.to_owned()).map_err(|error| error.to_string())
}

fn parse_public_id_list(value: &str) -> Result<Vec<PublicId>, String> {
    let Some(body) = value
        .trim()
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    else {
        return Err(format!("expected public id list, got `{value}`"));
    };
    body.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(parse_public_id_arg)
        .collect()
}

fn parse_capture_target(value: &str) -> Result<CaptureTarget, String> {
    if value == "viewport()" || value == "viewport" {
        return Ok(CaptureTarget::Viewport);
    }
    if let Some(body) = call_body(value, "layer") {
        return parse_public_id_arg(body).map(|id| CaptureTarget::Layer { id });
    }
    if let Some(body) = call_body(value, "object") {
        let id =
            parse_string_label(body).unwrap_or_else(|| body.trim_start_matches('@').to_owned());
        return Ok(CaptureTarget::Object { id });
    }
    Err(format!("unsupported capture target `{value}`"))
}

fn call_body<'a>(value: &'a str, callee: &str) -> Option<&'a str> {
    value
        .strip_prefix(callee)?
        .strip_prefix('(')?
        .strip_suffix(')')
        .map(str::trim)
}

fn parse_capture_format(value: &str) -> Result<CaptureFormat, String> {
    match value.trim_start_matches('.') {
        "png" => Ok(CaptureFormat::Png),
        "raw_rgba" | "raw" => Ok(CaptureFormat::RawRgba),
        "svg" => Ok(CaptureFormat::Svg),
        _ => Err(format!("unsupported capture format `{value}`")),
    }
}

fn predicate_matches(predicate: &Predicate, observation: &ObservationEnvelope) -> bool {
    match predicate {
        Predicate::Compare { probe, op, value } => observation_value(probe, observation)
            .is_some_and(|actual| compare_values(&actual, *op, value)),
        Predicate::Exists { probe } => observation_value(probe, observation).is_some(),
        Predicate::ActionEnabled { .. } => false,
        Predicate::All { predicates } => predicates
            .iter()
            .all(|predicate| predicate_matches(predicate, observation)),
        Predicate::Any { predicates } => predicates
            .iter()
            .any(|predicate| predicate_matches(predicate, observation)),
        Predicate::Not { predicate } => !predicate_matches(predicate, observation),
    }
}

fn observation_value(probe: &Probe, observation: &ObservationEnvelope) -> Option<AgentValue> {
    match probe {
        Probe::Signal { target } | Probe::Metric { target } => {
            observation.signals.get(target.as_str()).cloned()
        }
        Probe::ObservationField { path } if path == "tick" => {
            Some(AgentValue::I64(i64::try_from(observation.tick).ok()?))
        }
        Probe::StatePath { .. } | Probe::ObservationField { .. } => None,
    }
}

fn compare_values(actual: &AgentValue, op: CompareOp, expected: &AgentValue) -> bool {
    match op {
        CompareOp::Eq => agent_values_equal(actual, expected),
        CompareOp::NotEq => !agent_values_equal(actual, expected),
        CompareOp::Greater => numeric_value(actual)
            .zip(numeric_value(expected))
            .is_some_and(|(left, right)| left > right),
        CompareOp::GreaterOrEqual => numeric_value(actual)
            .zip(numeric_value(expected))
            .is_some_and(|(left, right)| left >= right),
        CompareOp::Less => numeric_value(actual)
            .zip(numeric_value(expected))
            .is_some_and(|(left, right)| left < right),
        CompareOp::LessOrEqual => numeric_value(actual)
            .zip(numeric_value(expected))
            .is_some_and(|(left, right)| left <= right),
    }
}

fn agent_values_equal(left: &AgentValue, right: &AgentValue) -> bool {
    match (left, right) {
        (AgentValue::Entity(left), AgentValue::String(right))
        | (AgentValue::String(right), AgentValue::Entity(left)) => left.as_str() == right,
        _ => left == right,
    }
}

fn numeric_value(value: &AgentValue) -> Option<i64> {
    match value {
        AgentValue::I64(value) => Some(*value),
        AgentValue::U64(value) => i64::try_from(*value).ok(),
        AgentValue::Null
        | AgentValue::Bool(_)
        | AgentValue::F64(_)
        | AgentValue::String(_)
        | AgentValue::Entity(_)
        | AgentValue::List(_)
        | AgentValue::Map(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_agent_protocol::{
        artifact::{
            AgentArtifactManifest, AgentBudget, AgentBundleKind, ProjectBinding, ProjectBindingMode,
        },
        ids::StableHash,
        ids::{AgentResourceUri, PublicId},
        protocol::{CaptureFormat, CaptureTarget},
    };
    use arcweft_bundle::{ArcweftBundle, BundleManifest, BundleRuntimeSummary, BundleSource};
    use arcweft_core::{
        bytecode::BytecodeProgram,
        effect::{LineEffectRequest, RuntimeCall},
        engine::{FlowExit, FlowFiberStatus},
        pattern::RuntimePattern,
        plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan},
        task::{AwaitTarget, HostTaskArgTemplate, HostTaskRequestTemplate, NeedId, TaskId},
        time::LogicalDuration,
        value::{RuntimeExpr, RuntimeFieldExpr, RuntimeValue},
    };
    use arcweft_debug_model::sink::NullDebugEventSink;
    use std::collections::BTreeMap;
    use std::convert::Infallible;

    #[derive(Default)]
    struct TestSession {
        observations: Vec<ObservationEnvelope>,
    }

    impl AgentSession for TestSession {
        type Error = Infallible;

        fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
            Ok(AgentSessionInfo {
                session_id: "session.test".to_owned(),
                program_hash: "hash".to_owned(),
                profile: None,
                capabilities: Vec::new(),
            })
        }

        fn observe(
            &mut self,
            _request: ObserveRequest,
        ) -> Result<ObservationEnvelope, Self::Error> {
            Ok(self.observations.remove(0))
        }

        fn act(&mut self, _action: AgentAction) -> Result<ActionResult, Self::Error> {
            Ok(ActionResult {
                accepted: true,
                before_tick: 1,
                after_tick: 2,
                before_state_hash: "a".to_owned(),
                after_state_hash: "b".to_owned(),
            })
        }

        fn capture(&mut self, _request: CaptureRequest) -> Result<CaptureResult, Self::Error> {
            Ok(CaptureResult {
                uri: AgentResourceUri::new("agent://capture/test").expect("valid uri"),
                content_hash: "hash".to_owned(),
                media_type: "image/png".to_owned(),
                byte_len: 4,
            })
        }

        fn read_resource(&mut self, _uri: &str) -> Result<AgentResource, Self::Error> {
            unreachable!("not used by test")
        }

        fn step_frames(&mut self, _count: u32) -> Result<ObservationEnvelope, Self::Error> {
            Ok(self.observations.remove(0))
        }
    }

    fn observation(tick: u64, ready: bool) -> ObservationEnvelope {
        ObservationEnvelope {
            tick,
            frame_id: format!("frame.{tick}"),
            state_hash: format!("state.{tick}"),
            render_hash: format!("render.{tick}"),
            signals: BTreeMap::from([("signal.ready".to_owned(), AgentValue::Bool(ready))]),
            payload: serde_json::json!({}),
        }
    }

    fn observation_with_signal(
        tick: u64,
        signal: &'static str,
        value: AgentValue,
    ) -> ObservationEnvelope {
        ObservationEnvelope {
            tick,
            frame_id: format!("frame.{tick}"),
            state_hash: format!("state.{tick}"),
            render_hash: format!("render.{tick}"),
            signals: BTreeMap::from([(signal.to_owned(), value)]),
            payload: serde_json::json!({}),
        }
    }

    fn observe_checkpoint_program() -> BytecodeProgram {
        BytecodeProgram::from_runtime_plan(
            RuntimePlan::new(
                Some(FlowRuntimeId("agent.observe_smoke".to_owned())),
                vec![RuntimeFlow {
                    id: FlowRuntimeId("agent.observe_smoke".to_owned()),
                    ops: vec![
                        FlowOp::Effect(LineEffectRequest::Call(RuntimeCall {
                            callee: "observe".to_owned(),
                            args: vec!["include_objects = true".to_owned()],
                        })),
                        FlowOp::Effect(LineEffectRequest::Call(RuntimeCall {
                            callee: "checkpoint".to_owned(),
                            args: vec!["\"after-observe\"".to_owned()],
                        })),
                        FlowOp::Return("done".to_owned()),
                    ],
                }],
                Vec::new(),
            )
            .expect("runtime plan is valid"),
        )
    }

    fn observe_checkpoint_bundle() -> ArcweftBundle {
        let program = observe_checkpoint_program();
        let stats = program.stats();
        let display = arcweft_render_text::LineDisplayCatalog::default();
        ArcweftBundle::new(
            BundleManifest {
                source_label: "agent.observe_smoke.awfagent".to_owned(),
                profile_id: None,
                profile_kind: None,
                entry: Some("entry.agent.observe_smoke".to_owned()),
                adapter: None,
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    entry_flow: program.entry_flow.as_ref().map(|flow| flow.0.clone()),
                    flows: stats.flows,
                    bytecode_instructions: stats.instructions,
                    line_task_groups: stats.line_task_groups,
                    stream_plans: stats.stream_plans,
                    source_plans: stats.source_plans,
                },
            },
            BundleSource {
                label: "agent.observe_smoke.awfagent".to_owned(),
                text: "agent @agent.observe_smoke observe_smoke() { observe() }".to_owned(),
            },
            program,
            display,
        )
        .with_agent_manifest(AgentArtifactManifest {
            schema_version: 1,
            bundle_kind: AgentBundleKind::AgentController,
            agent_id: PublicId::new("agent.observe_smoke").expect("valid agent id"),
            source_hash: StableHash::new("blake3:test").expect("valid source hash"),
            compiler_version: "test".to_owned(),
            project_binding: ProjectBinding {
                program_hash: StableHash::new("program-test").expect("valid program hash"),
                mode: ProjectBindingMode::Compatible,
                required_entities: Vec::new(),
            },
            declared_effects: Vec::new(),
            budget: AgentBudget::default(),
            debug_map_hash: None,
        })
    }

    fn capture_binding_program() -> BytecodeProgram {
        BytecodeProgram::from_runtime_plan(
            RuntimePlan::new(
                Some(FlowRuntimeId("agent.capture_binding".to_owned())),
                vec![RuntimeFlow {
                    id: FlowRuntimeId("agent.capture_binding".to_owned()),
                    ops: vec![
                        FlowOp::Await {
                            binding: Some(RuntimePattern::Ident("shot".to_owned())),
                            target: AwaitTarget::new(
                                NeedId("need.agent.capture".to_owned()),
                                TaskId("task.agent.capture".to_owned()),
                                HostTaskRequestTemplate::new(
                                    "agent",
                                    "capture",
                                    [
                                        HostTaskArgTemplate::positional(RuntimeExpr::Value(
                                            RuntimeValue::String("viewport()".to_owned()),
                                        )),
                                        HostTaskArgTemplate::positional(RuntimeExpr::Record(vec![
                                            RuntimeFieldExpr {
                                                name: "format".to_owned(),
                                                value: RuntimeExpr::Value(RuntimeValue::String(
                                                    ".png".to_owned(),
                                                )),
                                            },
                                            RuntimeFieldExpr {
                                                name: "name".to_owned(),
                                                value: RuntimeExpr::Value(RuntimeValue::String(
                                                    "viewport".to_owned(),
                                                )),
                                            },
                                        ])),
                                    ],
                                ),
                            ),
                            pending: Vec::new(),
                        },
                        FlowOp::ReturnExpr(RuntimeExpr::Field {
                            target: Box::new(RuntimeExpr::Local("shot".to_owned())),
                            field: "uri".to_owned(),
                        }),
                    ],
                }],
                Vec::new(),
            )
            .expect("runtime plan is valid"),
        )
    }

    fn wait_binding_program() -> BytecodeProgram {
        BytecodeProgram::from_runtime_plan(
            RuntimePlan::new(
                Some(FlowRuntimeId("agent.wait_binding".to_owned())),
                vec![RuntimeFlow {
                    id: FlowRuntimeId("agent.wait_binding".to_owned()),
                    ops: vec![
                        FlowOp::Await {
                            binding: Some(RuntimePattern::Ident("obs".to_owned())),
                            target: AwaitTarget::new(
                                NeedId("need.agent.wait".to_owned()),
                                TaskId("task.agent.wait".to_owned()),
                                HostTaskRequestTemplate::new(
                                    "agent",
                                    "wait",
                                    [
                                        HostTaskArgTemplate::positional(RuntimeExpr::Record(vec![
                                            RuntimeFieldExpr {
                                                name: "kind".to_owned(),
                                                value: RuntimeExpr::Value(RuntimeValue::String(
                                                    "compare".to_owned(),
                                                )),
                                            },
                                            RuntimeFieldExpr {
                                                name: "probe".to_owned(),
                                                value: RuntimeExpr::Record(vec![
                                                    RuntimeFieldExpr {
                                                        name: "kind".to_owned(),
                                                        value: RuntimeExpr::Value(
                                                            RuntimeValue::String(
                                                                "signal".to_owned(),
                                                            ),
                                                        ),
                                                    },
                                                    RuntimeFieldExpr {
                                                        name: "target".to_owned(),
                                                        value: RuntimeExpr::Value(
                                                            RuntimeValue::String(
                                                                "signal.ready".to_owned(),
                                                            ),
                                                        ),
                                                    },
                                                ]),
                                            },
                                            RuntimeFieldExpr {
                                                name: "op".to_owned(),
                                                value: RuntimeExpr::Value(RuntimeValue::String(
                                                    "eq".to_owned(),
                                                )),
                                            },
                                            RuntimeFieldExpr {
                                                name: "value".to_owned(),
                                                value: RuntimeExpr::Value(RuntimeValue::Bool(true)),
                                            },
                                        ])),
                                        HostTaskArgTemplate::positional(RuntimeExpr::Variant {
                                            path: Some("agent".to_owned()),
                                            name: "named_args".to_owned(),
                                            payload: Some(Box::new(RuntimeExpr::Record(vec![
                                                RuntimeFieldExpr {
                                                    name: "timeout".to_owned(),
                                                    value: RuntimeExpr::Value(
                                                        RuntimeValue::Duration(
                                                            LogicalDuration::from_nanos(5_000_000),
                                                        ),
                                                    ),
                                                },
                                                RuntimeFieldExpr {
                                                    name: "stable_frames".to_owned(),
                                                    value: RuntimeExpr::Value(RuntimeValue::u32(2)),
                                                },
                                                RuntimeFieldExpr {
                                                    name: "poll_frames".to_owned(),
                                                    value: RuntimeExpr::Value(RuntimeValue::u32(1)),
                                                },
                                            ]))),
                                        }),
                                    ],
                                ),
                            ),
                            pending: Vec::new(),
                        },
                        FlowOp::ReturnExpr(RuntimeExpr::Field {
                            target: Box::new(RuntimeExpr::Local("obs".to_owned())),
                            field: "tick".to_owned(),
                        }),
                    ],
                }],
                Vec::new(),
            )
            .expect("runtime plan is valid"),
        )
    }

    #[test]
    fn wait_requires_stable_predicate_matches() {
        let session = TestSession {
            observations: vec![
                observation(1, false),
                observation(2, true),
                observation(3, true),
            ],
        };
        let mut runner = AgentRunner::new(
            session,
            NullDebugEventSink,
            NoopRagService,
            RuntimeAgentPolicy::default(),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let report = runner
            .handle_host_request(AgentHostRequest::Wait(Box::new(WaitRequest {
                predicate: Predicate::Compare {
                    probe: Probe::Signal {
                        target: PublicId::new("signal.ready").expect("valid public id"),
                    },
                    op: CompareOp::Eq,
                    value: AgentValue::Bool(true),
                },
                timeout_millis: 5,
                stable_frames: 2,
                poll_frames: 1,
            })))
            .expect("wait succeeds");

        assert!(matches!(
            report.response,
            AgentHostResponse::Observation(observation) if observation.tick == 3
        ));
    }

    #[test]
    fn wait_matches_entity_probe_against_string_observation_id() {
        let session = TestSession {
            observations: vec![observation_with_signal(
                1,
                "signal.current_flow",
                AgentValue::String("flow.opening".to_owned()),
            )],
        };
        let mut runner = AgentRunner::new(
            session,
            NullDebugEventSink,
            NoopRagService,
            RuntimeAgentPolicy::default(),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let report = runner
            .handle_host_request(AgentHostRequest::Wait(Box::new(WaitRequest {
                predicate: Predicate::Compare {
                    probe: Probe::Signal {
                        target: PublicId::new("signal.current_flow").expect("valid public id"),
                    },
                    op: CompareOp::Eq,
                    value: AgentValue::Entity(
                        PublicId::new("flow.opening").expect("valid public id"),
                    ),
                },
                timeout_millis: 5,
                stable_frames: 1,
                poll_frames: 1,
            })))
            .expect("wait succeeds");

        assert!(matches!(
            report.response,
            AgentHostResponse::Observation(observation) if observation.tick == 1
        ));
    }

    #[test]
    fn effect_form_wait_call_lowers_to_host_wait_request() {
        let request = agent_host_request_from_call(&RuntimeCall {
            callee: "wait".to_owned(),
            args: vec![
                "signal(@signal.current_flow).eq(@flow.opening)".to_owned(),
                "timeout = 5s".to_owned(),
                "stable_frames = 2u32".to_owned(),
                "poll_frames = 1u32".to_owned(),
            ],
        })
        .expect("effect-form wait lowers");

        let AgentHostRequest::Wait(request) = request else {
            panic!("expected wait host request");
        };
        assert_eq!(request.timeout_millis, 5_000);
        assert_eq!(request.stable_frames, 2);
        assert_eq!(request.poll_frames, 1);
        assert!(matches!(
            request.predicate,
            Predicate::Compare {
                probe: Probe::Signal { ref target },
                op: CompareOp::Eq,
                value: AgentValue::Entity(ref value),
            } if target.as_str() == "signal.current_flow" && value.as_str() == "flow.opening"
        ));
    }

    #[test]
    fn capture_requires_policy_capability() {
        let mut runner = AgentRunner::new(
            TestSession::default(),
            NullDebugEventSink,
            NoopRagService,
            RuntimeAgentPolicy::default(),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let error = runner
            .handle_host_request(AgentHostRequest::Capture(Box::new(CaptureRequest {
                target: CaptureTarget::Viewport,
                format: CaptureFormat::Png,
                capture_kind: "color".to_owned(),
                name: "viewport".to_owned(),
            })))
            .expect_err("capture is denied");

        assert!(matches!(
            error,
            AgentRunError::PolicyDenied("agent.capture")
        ));
    }

    #[test]
    fn controller_bytecode_dispatches_effect_calls_to_runner_host_boundary() {
        let session = TestSession {
            observations: vec![observation(1, true)],
        };
        let mut runner = AgentRunner::new(
            session,
            NullDebugEventSink,
            NoopRagService,
            RuntimeAgentPolicy::default(),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let report = runner
            .run_controller_bytecode(
                observe_checkpoint_program(),
                AgentControllerRunConfig::default(),
            )
            .expect("controller bytecode runs");

        assert_eq!(report.steps, 1);
        assert_eq!(report.host_calls, 2);
        assert_eq!(report.responses.len(), 2);
        assert!(matches!(
            &report.responses[0],
            AgentHostResponse::Observation(observation) if observation.tick == 1
        ));
        assert!(matches!(report.responses[1], AgentHostResponse::Unit));
    }

    #[test]
    fn controller_bundle_runs_through_bytecode_host_boundary() {
        let session = TestSession {
            observations: vec![observation(1, true)],
        };
        let mut runner = AgentRunner::new(
            session,
            NullDebugEventSink,
            NoopRagService,
            RuntimeAgentPolicy::default(),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );
        let bundle = observe_checkpoint_bundle();

        let report = runner
            .run_controller_bundle(&bundle, AgentControllerRunConfig::default())
            .expect("controller bundle runs");

        assert_eq!(report.host_calls, 2);
        assert!(matches!(
            &report.responses[0],
            AgentHostResponse::Observation(observation) if observation.tick == 1
        ));
    }

    #[test]
    fn controller_bytecode_resumes_bound_capture_response() {
        let mut runner = AgentRunner::new(
            TestSession::default(),
            NullDebugEventSink,
            NoopRagService,
            RuntimeAgentPolicy::new([
                RuntimeAgentCapability::Observe,
                RuntimeAgentCapability::Capture,
            ]),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let report = runner
            .run_controller_bytecode(
                capture_binding_program(),
                AgentControllerRunConfig::default(),
            )
            .expect("controller bytecode runs");

        assert_eq!(report.host_calls, 1);
        assert!(matches!(
            &report.responses[0],
            AgentHostResponse::Capture(result) if result.uri.as_str() == "agent://capture/test"
        ));
        assert!(matches!(
            report.final_status,
            Some(FlowFiberStatus::Done(FlowExit::Return(ref value)))
                if value == "agent://capture/test"
        ));
    }

    #[test]
    fn controller_bytecode_resumes_bound_wait_response() {
        let session = TestSession {
            observations: vec![
                observation(1, false),
                observation(2, true),
                observation(3, true),
            ],
        };
        let mut runner = AgentRunner::new(
            session,
            NullDebugEventSink,
            NoopRagService,
            RuntimeAgentPolicy::default(),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let report = runner
            .run_controller_bytecode(wait_binding_program(), AgentControllerRunConfig::default())
            .expect("controller bytecode runs");

        assert_eq!(report.host_calls, 1);
        assert!(matches!(
            &report.responses[0],
            AgentHostResponse::Observation(observation) if observation.tick == 3
        ));
        assert!(matches!(
            report.final_status,
            Some(FlowFiberStatus::Done(FlowExit::Return(ref value))) if value == "3"
        ));
    }
}
