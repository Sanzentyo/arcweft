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
    executor::{BytecodeVmExecutor, RuntimeExecutor},
    plan::RuntimePlanError,
    step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
};
use arcweft_debug_model::{
    event::{DebugEvent, DebugEventKind},
    rag::RagContextPack,
    sink::DebugEventSink,
};
use std::collections::BTreeSet;
use thiserror::Error;

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
                AgentHostResponse::Action(result)
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
                AgentHostResponse::Capture(result)
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
    /// effect-form Agent host calls in source/runtime order.
    ///
    /// This slice intentionally handles calls that do not need their host
    /// response rebound into the VM. Agent expressions such as
    /// `let shot = capture(...)` still require the next suspend/resume slice.
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
        };

        while report.steps < config.max_steps {
            report.steps += 1;
            let step = executor.step(RuntimeStepInput::default(), options);
            for effect in &step.output.effects.line {
                let request = agent_host_request_from_effect(effect)
                    .map_err(AgentRunError::UnsupportedControllerEffect)?;
                let host_report = self.handle_host_request(request)?;
                report.host_calls += 1;
                report.responses.push(host_report.response);
                report.events_emitted = host_report.events_emitted;
            }

            match step.fiber_status {
                arcweft_core::engine::FlowFiberStatus::Done(_) => return Ok(report),
                arcweft_core::engine::FlowFiberStatus::Failed(message) => {
                    return Err(AgentRunError::ControllerFailed(message));
                }
                arcweft_core::engine::FlowFiberStatus::Running
                | arcweft_core::engine::FlowFiberStatus::Waiting(_)
                | arcweft_core::engine::FlowFiberStatus::WaitingMany(_)
                | arcweft_core::engine::FlowFiberStatus::Choice(_) => {}
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
        CompareOp::Eq => actual == expected,
        CompareOp::NotEq => actual != expected,
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
        plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan},
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
}
