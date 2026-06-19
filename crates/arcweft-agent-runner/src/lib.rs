//! Controller runner boundaries for compiled Agent Script programs.
//!
//! This crate does not interpret `.awfagent` source and does not own CLI, MCP,
//! renderer, database, filesystem, or transport I/O. It coordinates typed host
//! requests emitted by a controller VM with an `AgentSession`, debug sink, and
//! RAG service.

use arcweft_agent_protocol::{
    AgentActionTarget, AgentResource,
    artifact::{AgentBudget, ProjectBinding, ProjectBindingMode, RequiredEntity},
    ids::{AgentResourceUri, AgentRunId, PublicId, SessionId},
    predicate::{CompareOp, Predicate, Probe},
    protocol::{
        ActionResult, AgentAction, AgentAssertionKind, AgentAssertionRequest, AgentAttachment,
        AgentHostRequest, AgentHostResponse, AgentSessionInfo, CaptureFormat, CaptureRequest,
        CaptureResult, CaptureTarget, ObservationEnvelope, ObserveRequest, PointerButton,
        RagRequest, WaitRequest,
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
    ActPhysical,
    Capture,
    ResourceRead,
    DebugRecord,
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
    #[error(
        "Agent controller project binding mismatch: expected program hash {expected_program_hash}, actual {actual_program_hash}, mode {mode:?}: {detail}"
    )]
    ProjectBindingMismatch {
        expected_program_hash: String,
        actual_program_hash: String,
        mode: ProjectBindingMode,
        detail: String,
    },
    #[error("Agent controller emitted unsupported effect: {0}")]
    UnsupportedControllerEffect(String),
    #[error("Agent assertion failed ({kind:?}): {message}")]
    AssertionFailed {
        kind: AgentAssertionKind,
        message: String,
    },
    #[error("Agent controller failed: {0}")]
    ControllerFailed(String),
    #[error("Agent controller exceeded execution step budget of {max_steps}")]
    ControllerBudgetExceeded { max_steps: usize },
    #[error("Agent controller exceeded {kind} budget: attempted {attempted}, limit {limit}")]
    ControllerResourceBudgetExceeded {
        kind: &'static str,
        limit: u64,
        attempted: u64,
    },
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
            Self::Act => "agent.act.semantic",
            Self::ActPhysical => "agent.act.physical",
            Self::Capture => "agent.capture",
            Self::ResourceRead => "agent.resource.read",
            Self::DebugRecord => "debug.record",
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct AgentBudgetTracker {
    host_calls: u32,
    observations: u32,
    captures: u32,
    capture_bytes: u64,
    rag_queries: u32,
    context_bytes: u64,
}

struct AgentBudgetContext<'a> {
    limits: AgentBudget,
    tracker: &'a mut AgentBudgetTracker,
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
        self.handle_host_request_inner(request, None)
    }

    fn handle_controller_host_request(
        &mut self,
        request: AgentHostRequest,
        limits: AgentBudget,
        tracker: &mut AgentBudgetTracker,
    ) -> AgentRunnerResult<AgentHostCallReport, S, D, R> {
        self.handle_host_request_inner(request, Some(AgentBudgetContext { limits, tracker }))
    }

    fn handle_host_request_inner(
        &mut self,
        request: AgentHostRequest,
        mut budget: Option<AgentBudgetContext<'_>>,
    ) -> AgentRunnerResult<AgentHostCallReport, S, D, R> {
        if let Some(budget) = budget.as_mut() {
            record_budget_u32(
                "host call",
                &mut budget.tracker.host_calls,
                1,
                budget.limits.max_host_calls,
            )?;
        }
        self.emit(DebugEventKind::StepStarted, None, serde_json::json!({}))?;
        let response = match request {
            AgentHostRequest::Observe(request) => {
                self.handle_observe_request(*request, budget.as_mut())?
            }
            AgentHostRequest::Act(action) => self.handle_action_request(*action)?,
            AgentHostRequest::Wait(request) => {
                self.handle_wait_request(*request, budget.as_mut())?
            }
            AgentHostRequest::Capture(request) => {
                self.handle_capture_request(*request, budget.as_mut())?
            }
            AgentHostRequest::ReadResource { uri } => self.handle_read_resource_request(&uri)?,
            AgentHostRequest::RagQuery(request) => {
                self.handle_rag_query_request(*request, budget.as_mut())?
            }
            AgentHostRequest::Assert(request) => self.handle_assertion_request(request.as_ref())?,
            AgentHostRequest::Attach(attachment) => {
                self.ensure(RuntimeAgentCapability::DebugRecord)?;
                self.emit(
                    DebugEventKind::Diagnostic,
                    None,
                    serde_json::json!({ "attachment": attachment.resource }),
                )?;
                AgentHostResponse::Unit
            }
            AgentHostRequest::Checkpoint { name } => {
                self.ensure(RuntimeAgentCapability::DebugRecord)?;
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

    fn handle_observe_request(
        &mut self,
        request: ObserveRequest,
        budget: Option<&mut AgentBudgetContext<'_>>,
    ) -> AgentRunnerResult<AgentHostResponse, S, D, R> {
        self.ensure(RuntimeAgentCapability::Observe)?;
        if let Some(budget) = budget {
            record_budget_u32(
                "observation",
                &mut budget.tracker.observations,
                1,
                budget.limits.max_observations,
            )?;
        }
        let observation = self
            .session
            .observe(request)
            .map_err(AgentRunError::Session)?;
        self.emit(
            DebugEventKind::Observation,
            Some(observation.tick),
            serde_json::to_value(&observation).unwrap_or(serde_json::Value::Null),
        )?;
        Ok(AgentHostResponse::Observation(Box::new(observation)))
    }

    fn handle_action_request(
        &mut self,
        action: AgentAction,
    ) -> AgentRunnerResult<AgentHostResponse, S, D, R> {
        self.ensure(match &action {
            AgentAction::PointerClick { .. } => RuntimeAgentCapability::ActPhysical,
            AgentAction::AdvanceText
            | AgentAction::SelectChoice { .. }
            | AgentAction::Invoke { .. } => RuntimeAgentCapability::Act,
        })?;
        let result = self.session.act(action).map_err(AgentRunError::Session)?;
        self.emit(
            DebugEventKind::Action,
            Some(result.after_tick),
            serde_json::to_value(&result).unwrap_or(serde_json::Value::Null),
        )?;
        Ok(AgentHostResponse::Action(Box::new(result)))
    }

    fn handle_wait_request(
        &mut self,
        request: WaitRequest,
        budget: Option<&mut AgentBudgetContext<'_>>,
    ) -> AgentRunnerResult<AgentHostResponse, S, D, R> {
        let request = effective_wait_request(
            request,
            budget
                .as_ref()
                .map(|budget| budget.limits.logical_timeout_millis),
        );
        let observation = self.wait(&request, budget)?;
        Ok(AgentHostResponse::Observation(Box::new(observation)))
    }

    fn handle_capture_request(
        &mut self,
        request: CaptureRequest,
        budget: Option<&mut AgentBudgetContext<'_>>,
    ) -> AgentRunnerResult<AgentHostResponse, S, D, R> {
        self.ensure(RuntimeAgentCapability::Capture)?;
        let mut budget = budget;
        if let Some(budget) = budget.as_mut() {
            record_budget_u32(
                "capture",
                &mut budget.tracker.captures,
                1,
                budget.limits.max_captures,
            )?;
        }
        let result = self
            .session
            .capture(request)
            .map_err(AgentRunError::Session)?;
        if let Some(budget) = budget.as_mut() {
            record_budget_u64(
                "capture byte",
                &mut budget.tracker.capture_bytes,
                result.byte_len,
                budget.limits.max_capture_bytes,
            )?;
        }
        self.emit(
            DebugEventKind::Capture,
            None,
            serde_json::to_value(&result).unwrap_or(serde_json::Value::Null),
        )?;
        Ok(AgentHostResponse::Capture(Box::new(result)))
    }

    fn handle_read_resource_request(
        &mut self,
        uri: &AgentResourceUri,
    ) -> AgentRunnerResult<AgentHostResponse, S, D, R> {
        self.ensure(RuntimeAgentCapability::ResourceRead)?;
        let resource = self
            .session
            .read_resource(uri.as_str())
            .map_err(AgentRunError::Session)?;
        Ok(AgentHostResponse::Resource(Box::new(
            serde_json::to_value(resource).unwrap_or(serde_json::Value::Null),
        )))
    }

    fn handle_rag_query_request(
        &mut self,
        request: RagRequest,
        budget: Option<&mut AgentBudgetContext<'_>>,
    ) -> AgentRunnerResult<AgentHostResponse, S, D, R> {
        self.ensure(RuntimeAgentCapability::Rag)?;
        let mut budget = budget;
        if let Some(budget) = budget.as_mut() {
            record_budget_u32(
                "RAG query",
                &mut budget.tracker.rag_queries,
                1,
                budget.limits.max_rag_queries,
            )?;
        }
        let context = self.rag.query(request).map_err(AgentRunError::Rag)?;
        let context_value = serde_json::to_value(&context).unwrap_or(serde_json::Value::Null);
        if let Some(budget) = budget.as_mut() {
            let context_bytes = serde_json::to_vec(&context_value).map_or(u64::MAX, |bytes| {
                u64::try_from(bytes.len()).unwrap_or(u64::MAX)
            });
            record_budget_u64(
                "RAG context byte",
                &mut budget.tracker.context_bytes,
                context_bytes,
                budget.limits.max_context_bytes,
            )?;
        }
        self.emit(DebugEventKind::RagQuery, None, context_value.clone())?;
        Ok(AgentHostResponse::RagContext(Box::new(context_value)))
    }

    fn handle_assertion_request(
        &mut self,
        request: &AgentAssertionRequest,
    ) -> AgentRunnerResult<AgentHostResponse, S, D, R> {
        let passed = agent_assertion_passed(request);
        self.emit(
            DebugEventKind::Assertion,
            None,
            serde_json::json!({
                "kind": agent_assertion_kind_label(request.kind),
                "condition": request.condition,
                "passed": passed,
                "message": request.message.clone(),
            }),
        )?;
        if passed {
            Ok(AgentHostResponse::Unit)
        } else {
            Err(AgentRunError::AssertionFailed {
                kind: request.kind,
                message: agent_assertion_failure_message(request),
            })
        }
    }

    /// Runs one compiled Agent controller bytecode program and dispatches
    /// Agent host calls in source/runtime order.
    pub fn run_controller_bytecode(
        &mut self,
        program: BytecodeProgram,
        config: AgentControllerRunConfig,
    ) -> AgentRunnerResult<AgentControllerRunReport, S, D, R> {
        self.run_controller_bytecode_with_budget(
            program,
            config,
            effective_controller_budget(AgentBudget::default(), config),
        )
    }

    fn run_controller_bytecode_with_budget(
        &mut self,
        program: BytecodeProgram,
        config: AgentControllerRunConfig,
        budget: AgentBudget,
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
        let mut budget_tracker = AgentBudgetTracker::default();

        let max_steps = usize::try_from(budget.max_vm_steps)
            .unwrap_or(usize::MAX)
            .min(config.max_steps);
        while report.steps < max_steps {
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
                let host_report =
                    self.handle_controller_host_request(request, budget, &mut budget_tracker)?;
                report.host_calls += 1;
                report.responses.push(host_report.response);
                report.events_emitted = host_report.events_emitted;
            }
            for task in &step.output.requests.tasks {
                let request = agent_host_request_from_task(&task.request)
                    .map_err(AgentRunError::UnsupportedControllerEffect)?;
                let host_report =
                    self.handle_controller_host_request(request, budget, &mut budget_tracker)?;
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

        Err(AgentRunError::ControllerBudgetExceeded { max_steps })
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
        let manifest = bundle
            .agent
            .as_ref()
            .ok_or(AgentRunError::MissingAgentManifest)?;
        self.validate_project_binding(&manifest.project_binding)?;
        self.run_controller_bytecode_with_budget(
            bundle.bytecode.program.clone(),
            config,
            effective_controller_budget(manifest.budget, config),
        )
    }

    fn validate_project_binding(
        &mut self,
        binding: &ProjectBinding,
    ) -> AgentRunnerResult<(), S, D, R> {
        let session_info = self.session.info().map_err(AgentRunError::Session)?;
        match binding.mode {
            ProjectBindingMode::Strict => {
                if binding.program_hash.as_str() == session_info.program_hash {
                    Ok(())
                } else {
                    Err(project_binding_mismatch(
                        binding,
                        &session_info,
                        "strict program hash mismatch".to_owned(),
                    ))
                }
            }
            ProjectBindingMode::Compatible => {
                let runtime_entities = session_info
                    .project_entities
                    .iter()
                    .map(|entity| (entity.public_id.as_str(), entity))
                    .collect::<BTreeMap<_, _>>();
                if let Some(detail) = binding.required_entities.iter().find_map(|required| {
                    compatible_entity_mismatch(
                        required,
                        runtime_entities.get(required.public_id.as_str()).copied(),
                    )
                }) {
                    Err(project_binding_mismatch(binding, &session_info, detail))
                } else {
                    Ok(())
                }
            }
        }
    }

    fn wait(
        &mut self,
        request: &WaitRequest,
        mut budget: Option<&mut AgentBudgetContext<'_>>,
    ) -> AgentRunnerResult<ObservationEnvelope, S, D, R> {
        self.ensure(RuntimeAgentCapability::Observe)?;
        let poll_frames = request.poll_frames.max(1);
        let stable_frames = request.stable_frames.max(1);
        let max_polls = (request.timeout_millis / u64::from(poll_frames)).max(1);
        let mut stable_count = 0;
        let mut last_observation = None;

        for _ in 0..max_polls {
            if let Some(budget) = budget.as_mut() {
                record_budget_u32(
                    "observation",
                    &mut budget.tracker.observations,
                    1,
                    budget.limits.max_observations,
                )?;
            }
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

fn project_binding_mismatch<SessionError, DebugError, RagError>(
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

fn effective_controller_budget(
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

fn effective_wait_request(
    mut request: WaitRequest,
    logical_timeout_millis: Option<u64>,
) -> WaitRequest {
    if let Some(limit) = logical_timeout_millis {
        request.timeout_millis = request.timeout_millis.min(limit);
    }
    request
}

fn record_budget_u32<SessionError, DebugError, RagError>(
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

fn record_budget_u64<SessionError, DebugError, RagError>(
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

fn compatible_entity_mismatch(
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
    if required.type_fingerprint != actual.type_fingerprint {
        return Some(format!(
            "required entity {} type fingerprint mismatch: expected {}, actual {}",
            required.public_id.as_str(),
            required.type_fingerprint.as_str(),
            actual.type_fingerprint.as_str()
        ));
    }
    None
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
        "advance_text" => {
            if args.positional(0).is_some() || !args.named.is_empty() {
                return Err("advance_text does not accept arguments".to_owned());
            }
            Ok(AgentHostRequest::Act(Box::new(AgentAction::AdvanceText)))
        }
        "pointer.click" => runtime_pointer_click_action(&args)
            .map(|action| AgentHostRequest::Act(Box::new(action))),
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
        "expect" => runtime_assertion_request(&args, AgentAssertionKind::Expect)
            .map(|request| AgentHostRequest::Assert(Box::new(request))),
        "deny" => runtime_assertion_request(&args, AgentAssertionKind::Deny)
            .map(|request| AgentHostRequest::Assert(Box::new(request))),
        "checkpoint" => {
            let name = args
                .positional(0)
                .or_else(|| args.named("name"))
                .map_or_else(|| Ok("checkpoint".to_owned()), runtime_string)?;
            Ok(AgentHostRequest::Checkpoint { name })
        }
        "attach" => {
            runtime_attach_request(&args).map(|request| AgentHostRequest::Attach(Box::new(request)))
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
            .map_or(Ok(true), runtime_bool)?,
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

fn runtime_assertion_request(
    args: &RuntimeAgentArgs<'_>,
    kind: AgentAssertionKind,
) -> Result<AgentAssertionRequest, String> {
    let condition = args
        .positional(0)
        .or_else(|| args.named("condition"))
        .ok_or_else(|| "Agent assertion requires a condition argument".to_owned())
        .and_then(runtime_bool)?;
    let message = match args.positional(1).or_else(|| args.named("message")) {
        Some(value) => runtime_string(value)?,
        None => String::new(),
    };
    Ok(AgentAssertionRequest {
        kind,
        condition,
        message,
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
        args: Box::new(call_args),
    })
}

fn runtime_pointer_click_action(args: &RuntimeAgentArgs<'_>) -> Result<AgentAction, String> {
    let (x, y) = args
        .positional(0)
        .or_else(|| args.named("point"))
        .ok_or_else(|| "pointer.click requires a point argument".to_owned())
        .and_then(runtime_viewport_point)?;
    let button = args
        .named("button")
        .map_or(Ok(PointerButton::Primary), runtime_pointer_button)?;
    Ok(AgentAction::PointerClick { x, y, button })
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

fn runtime_attach_request(args: &RuntimeAgentArgs<'_>) -> Result<AgentAttachment, String> {
    let resource = args
        .positional(0)
        .or_else(|| args.named("resource"))
        .ok_or_else(|| "attach requires a resource argument".to_owned())?;
    if args.positional(1).is_some() {
        return Err("attach received too many positional arguments".to_owned());
    }
    Ok(AgentAttachment {
        resource: Box::new(runtime_value_to_json(resource)),
    })
}

fn runtime_value_to_json(value: &RuntimeValue) -> serde_json::Value {
    match value {
        RuntimeValue::Unit => serde_json::Value::Null,
        RuntimeValue::Bool(value) => serde_json::Value::Bool(*value),
        RuntimeValue::Int(value) => runtime_int_to_json(*value),
        RuntimeValue::UInt(value) => runtime_uint_to_json(*value),
        RuntimeValue::F32(value) => serde_json::json!(*value),
        RuntimeValue::F64(value) => serde_json::json!(*value),
        RuntimeValue::String(value) | RuntimeValue::EntityRef(value) => {
            serde_json::Value::String(value.clone())
        }
        RuntimeValue::Char(value) => serde_json::Value::String(value.to_string()),
        RuntimeValue::Tuple(values) => {
            serde_json::Value::Array(values.iter().map(runtime_value_to_json).collect())
        }
        RuntimeValue::Seq(values) => {
            serde_json::to_value(values).unwrap_or(serde_json::Value::Null)
        }
        RuntimeValue::Record(fields) => serde_json::Value::Object(
            fields
                .iter()
                .map(|field| (field.name.clone(), runtime_value_to_json(&field.value)))
                .collect(),
        ),
        RuntimeValue::Variant {
            path,
            name,
            payload,
        } => serde_json::json!({
            "path": path,
            "name": name,
            "payload": payload.as_deref().map(runtime_value_to_json),
        }),
        RuntimeValue::Duration(_)
        | RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_) => {
            serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
        }
    }
}

fn runtime_int_to_json(value: arcweft_core::value::RuntimeInt) -> serde_json::Value {
    match value {
        arcweft_core::value::RuntimeInt::I8(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeInt::I16(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeInt::I32(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeInt::I64(value)
        | arcweft_core::value::RuntimeInt::ISize(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeInt::I128(value) => i64::try_from(value).map_or_else(
            |_| serde_json::json!(value.to_string()),
            |value| serde_json::json!(value),
        ),
    }
}

fn runtime_uint_to_json(value: arcweft_core::value::RuntimeUInt) -> serde_json::Value {
    match value {
        arcweft_core::value::RuntimeUInt::U8(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeUInt::U16(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeUInt::U32(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeUInt::U64(value)
        | arcweft_core::value::RuntimeUInt::USize(value) => serde_json::json!(value),
        arcweft_core::value::RuntimeUInt::U128(value) => u64::try_from(value).map_or_else(
            |_| serde_json::json!(value.to_string()),
            |value| serde_json::json!(value),
        ),
    }
}

fn runtime_predicate(value: &RuntimeValue) -> Result<Predicate, String> {
    let fields = runtime_record_fields(value, "predicate")?;
    match runtime_record_string(fields, "kind")?.as_str() {
        "compare" => Ok(Predicate::Compare {
            probe: runtime_record_get(fields, "probe").and_then(runtime_probe)?,
            op: runtime_record_get(fields, "op").and_then(runtime_compare_op)?,
            value: Box::new(runtime_record_get(fields, "value").and_then(runtime_agent_value)?),
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
        "diagnostics_has_error" => Ok(Predicate::DiagnosticsHasError),
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
            runtime_field("actions", runtime_action_targets(&observation.actions)),
            runtime_field("objects", runtime_observed_objects(&observation.payload)),
            runtime_field("signals", runtime_agent_value_fields(&observation.signals)),
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
        AgentHostResponse::Resource(value) => runtime_resource_payload(value),
        AgentHostResponse::RagContext(value) => runtime_rag_context_payload(value),
        AgentHostResponse::Unit => RuntimeValue::Unit,
    })
}

fn agent_assertion_passed(request: &AgentAssertionRequest) -> bool {
    match request.kind {
        AgentAssertionKind::Expect => request.condition,
        AgentAssertionKind::Deny => !request.condition,
    }
}

fn agent_assertion_failure_message(request: &AgentAssertionRequest) -> String {
    if request.message.is_empty() {
        match request.kind {
            AgentAssertionKind::Expect => "expect condition evaluated to false".to_owned(),
            AgentAssertionKind::Deny => "deny condition evaluated to true".to_owned(),
        }
    } else {
        request.message.clone()
    }
}

const fn agent_assertion_kind_label(kind: AgentAssertionKind) -> &'static str {
    match kind {
        AgentAssertionKind::Expect => "expect",
        AgentAssertionKind::Deny => "deny",
    }
}

fn runtime_rag_context_payload(value: &serde_json::Value) -> RuntimeValue {
    let item_count = value
        .get("items")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    RuntimeValue::Record(vec![
        runtime_field("summary", RuntimeValue::String(rag_context_summary(value))),
        runtime_field(
            "item_count",
            RuntimeValue::usize(u64::try_from(item_count).unwrap_or(u64::MAX)),
        ),
        runtime_field(
            "truncated",
            RuntimeValue::Bool(
                value
                    .get("truncated")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            ),
        ),
        runtime_field("json", RuntimeValue::String(value.to_string())),
    ])
}

fn rag_context_summary(value: &serde_json::Value) -> String {
    let query = value
        .get("query")
        .and_then(|query| query.get("text"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let item_count = value
        .get("items")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if query.is_empty() {
        format!("{item_count} RAG context item(s)")
    } else {
        format!("{item_count} RAG context item(s) for `{query}`")
    }
}

fn runtime_resource_payload(value: &serde_json::Value) -> RuntimeValue {
    RuntimeValue::Record(vec![
        runtime_field("uri", runtime_json_string_field(value, "uri")),
        runtime_field("kind", runtime_json_string_field(value, "kind")),
        runtime_field("mime_type", runtime_json_string_field(value, "mime_type")),
        runtime_field("hash", runtime_json_string_field(value, "hash")),
        runtime_field("body", runtime_resource_body_payload(value.get("body"))),
    ])
}

fn runtime_action_targets(actions: &[AgentActionTarget]) -> RuntimeValue {
    RuntimeValue::Seq(arcweft_core::value::RuntimeSeq::values(
        actions
            .iter()
            .map(|action| {
                RuntimeValue::Record(vec![
                    runtime_field("id", RuntimeValue::String(action.id.clone())),
                    runtime_field("target", RuntimeValue::String(action.target.clone())),
                    runtime_field(
                        "action",
                        RuntimeValue::String(agent_action_kind_label(action.action).to_owned()),
                    ),
                    runtime_field(
                        "kind",
                        RuntimeValue::String(agent_action_dispatch_label(action.kind).to_owned()),
                    ),
                    runtime_field("enabled", RuntimeValue::Bool(action.enabled)),
                ])
            })
            .collect(),
    ))
}

fn runtime_observed_objects(payload: &serde_json::Value) -> RuntimeValue {
    RuntimeValue::Seq(arcweft_core::value::RuntimeSeq::values(
        payload
            .get("objects")
            .and_then(serde_json::Value::as_array)
            .map_or_else(Vec::new, |objects| {
                objects.iter().map(runtime_observed_object).collect()
            }),
    ))
}

fn runtime_observed_object(object: &serde_json::Value) -> RuntimeValue {
    RuntimeValue::Record(vec![
        runtime_field("id", runtime_json_string_field(object, "id")),
        runtime_field("parent_id", runtime_json_string_field(object, "parent_id")),
        runtime_field("entity", runtime_json_string_field(object, "entity")),
        runtime_field("layer", runtime_json_string_field(object, "layer")),
        runtime_field("role", runtime_json_string_field(object, "role")),
        runtime_field(
            "visible",
            RuntimeValue::Bool(
                object
                    .get("visible")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            ),
        ),
        runtime_field(
            "enabled",
            RuntimeValue::Bool(
                object
                    .get("enabled")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true),
            ),
        ),
        runtime_field("bbox", runtime_bbox(object.get("bbox"))),
        runtime_field("text", runtime_json_string_field(object, "text")),
    ])
}

fn runtime_bbox(value: Option<&serde_json::Value>) -> RuntimeValue {
    let Some(value) = value else {
        return RuntimeValue::Record(vec![
            runtime_field("space", RuntimeValue::String(String::new())),
            runtime_field("x", RuntimeValue::u32(0)),
            runtime_field("y", RuntimeValue::u32(0)),
            runtime_field("width", RuntimeValue::u32(0)),
            runtime_field("height", RuntimeValue::u32(0)),
        ]);
    };
    RuntimeValue::Record(vec![
        runtime_field("space", runtime_json_string_field(value, "space")),
        runtime_field("x", runtime_json_u32_field(value, "x")),
        runtime_field("y", runtime_json_u32_field(value, "y")),
        runtime_field("width", runtime_json_u32_field(value, "width")),
        runtime_field("height", runtime_json_u32_field(value, "height")),
    ])
}

fn runtime_agent_value_fields(values: &BTreeMap<String, AgentValue>) -> RuntimeValue {
    RuntimeValue::Record(
        values
            .iter()
            .map(|(name, value)| runtime_field(name, runtime_agent_value_payload(value)))
            .collect(),
    )
}

fn runtime_agent_value_payload(value: &AgentValue) -> RuntimeValue {
    match value {
        AgentValue::Null => RuntimeValue::Unit,
        AgentValue::Bool(value) => RuntimeValue::Bool(*value),
        AgentValue::I64(value) => RuntimeValue::i64(*value),
        AgentValue::U64(value) => RuntimeValue::u64(*value),
        AgentValue::F64(value) => RuntimeValue::F64(*value),
        AgentValue::String(value) => RuntimeValue::String(value.clone()),
        AgentValue::Entity(value) => RuntimeValue::EntityRef(value.as_str().to_owned()),
        AgentValue::List(values) => RuntimeValue::Seq(arcweft_core::value::RuntimeSeq::values(
            values.iter().map(runtime_agent_value_payload).collect(),
        )),
        AgentValue::Map(values) => RuntimeValue::Record(
            values
                .iter()
                .map(|(name, value)| runtime_field(name, runtime_agent_value_payload(value)))
                .collect(),
        ),
    }
}

fn agent_action_kind_label(kind: arcweft_agent_protocol::AgentActionKind) -> &'static str {
    match kind {
        arcweft_agent_protocol::AgentActionKind::AdvanceText => "advance_text",
        arcweft_agent_protocol::AgentActionKind::SelectChoice => "select_choice",
        arcweft_agent_protocol::AgentActionKind::Invoke => "invoke",
        arcweft_agent_protocol::AgentActionKind::PointerClick => "pointer_click",
    }
}

fn agent_action_dispatch_label(kind: arcweft_agent_protocol::AgentActionDispatch) -> &'static str {
    match kind {
        arcweft_agent_protocol::AgentActionDispatch::Semantic => "semantic",
        arcweft_agent_protocol::AgentActionDispatch::Physical => "physical",
    }
}

fn runtime_json_string_field(value: &serde_json::Value, field: &str) -> RuntimeValue {
    RuntimeValue::String(
        value
            .get(field)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    )
}

fn runtime_json_u32_field(value: &serde_json::Value, field: &str) -> RuntimeValue {
    RuntimeValue::u32(
        value
            .get(field)
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0),
    )
}

fn runtime_resource_body_payload(value: Option<&serde_json::Value>) -> RuntimeValue {
    let Some(value) = value else {
        return runtime_empty_resource_body();
    };
    let kind = value
        .get("body_kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let body = value.get("body");
    match kind {
        "json" => RuntimeValue::Record(vec![
            runtime_field("kind", RuntimeValue::String(kind.to_owned())),
            runtime_field(
                "json",
                RuntimeValue::String(body.map_or_else(String::new, serde_json::Value::to_string)),
            ),
            runtime_field(
                "value",
                body.map_or(RuntimeValue::Unit, runtime_value_from_json),
            ),
            runtime_field("text", RuntimeValue::String(String::new())),
            runtime_field("base64", RuntimeValue::String(String::new())),
            runtime_field("encoding", RuntimeValue::String(String::new())),
        ]),
        "text" => RuntimeValue::Record(vec![
            runtime_field("kind", RuntimeValue::String(kind.to_owned())),
            runtime_field("json", RuntimeValue::String(String::new())),
            runtime_field(
                "value",
                RuntimeValue::String(
                    body.and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                ),
            ),
            runtime_field(
                "text",
                RuntimeValue::String(
                    body.and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                ),
            ),
            runtime_field("base64", RuntimeValue::String(String::new())),
            runtime_field("encoding", RuntimeValue::String(String::new())),
        ]),
        "bytes_base64" => runtime_bytes_base64_body_payload(kind, body),
        _ => runtime_empty_resource_body(),
    }
}

fn runtime_bytes_base64_body_payload(kind: &str, body: Option<&serde_json::Value>) -> RuntimeValue {
    let data = body
        .and_then(|body| body.get("data"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let encoding = body
        .and_then(|body| body.get("encoding"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    RuntimeValue::Record(vec![
        runtime_field("kind", RuntimeValue::String(kind.to_owned())),
        runtime_field("json", RuntimeValue::String(String::new())),
        runtime_field("value", runtime_bytes_base64_value(body)),
        runtime_field("text", RuntimeValue::String(String::new())),
        runtime_field("base64", RuntimeValue::String(data)),
        runtime_field("encoding", RuntimeValue::String(encoding)),
    ])
}

fn runtime_empty_resource_body() -> RuntimeValue {
    RuntimeValue::Record(vec![
        runtime_field("kind", RuntimeValue::String(String::new())),
        runtime_field("json", RuntimeValue::String(String::new())),
        runtime_field("value", RuntimeValue::Unit),
        runtime_field("text", RuntimeValue::String(String::new())),
        runtime_field("base64", RuntimeValue::String(String::new())),
        runtime_field("encoding", RuntimeValue::String(String::new())),
    ])
}

fn runtime_bytes_base64_value(body: Option<&serde_json::Value>) -> RuntimeValue {
    RuntimeValue::Record(vec![
        runtime_field(
            "encoding",
            RuntimeValue::String(
                body.and_then(|body| body.get("encoding"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
        ),
        runtime_field(
            "data",
            RuntimeValue::String(
                body.and_then(|body| body.get("data"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
        ),
    ])
}

fn runtime_value_from_json(value: &serde_json::Value) -> RuntimeValue {
    match value {
        serde_json::Value::Null => RuntimeValue::Unit,
        serde_json::Value::Bool(value) => RuntimeValue::Bool(*value),
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(RuntimeValue::i64)
            .or_else(|| value.as_u64().map(RuntimeValue::u64))
            .or_else(|| value.as_f64().map(RuntimeValue::F64))
            .unwrap_or(RuntimeValue::Unit),
        serde_json::Value::String(value) => RuntimeValue::String(value.clone()),
        serde_json::Value::Array(values) => RuntimeValue::Tuple(
            values
                .iter()
                .map(runtime_value_from_json)
                .collect::<Vec<_>>(),
        ),
        serde_json::Value::Object(values) => RuntimeValue::Record(
            values
                .iter()
                .map(|(key, value)| runtime_field(key, runtime_value_from_json(value)))
                .collect(),
        ),
    }
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

fn invoke_action(args: &[String]) -> Result<AgentAction, String> {
    let mut target = None;
    let mut action = None;
    let mut call_args = None;
    let mut positional = Vec::new();
    for arg in args {
        if arg.trim_start().starts_with('{') {
            positional.push(arg.as_str());
            continue;
        }
        match named_arg(arg) {
            Some(("target", value)) => target = Some(value),
            Some(("action", value)) => action = Some(value),
            Some(("args", value)) => call_args = Some(value),
            Some((name, _)) => return Err(format!("invoke has no parameter named `{name}`")),
            None => positional.push(arg.as_str()),
        }
    }
    let target = target
        .or_else(|| positional.first().copied())
        .ok_or_else(|| "invoke requires a target argument".to_owned())
        .and_then(parse_public_id_arg)?;
    let action = action
        .or_else(|| positional.get(1).copied())
        .ok_or_else(|| "invoke requires an action argument".to_owned())
        .map(parse_action_label)?;
    let call_args = call_args
        .or_else(|| positional.get(2).copied())
        .map(parse_agent_value_map_label)
        .transpose()?
        .unwrap_or_default();
    Ok(AgentAction::Invoke {
        target,
        action,
        args: Box::new(call_args),
    })
}

fn pointer_click_action(args: &[String]) -> Result<AgentAction, String> {
    let point = args
        .first()
        .ok_or_else(|| "pointer.click requires a point argument".to_owned())
        .and_then(|arg| parse_viewport_point_label(arg))?;
    let mut button = PointerButton::Primary;
    for arg in args.iter().skip(1) {
        match named_arg(arg) {
            Some(("button", value)) => button = parse_pointer_button_label(value)?,
            Some((name, _)) => {
                return Err(format!("pointer.click has no parameter named `{name}`"));
            }
            None => {
                return Err(format!(
                    "pointer.click does not accept extra positional argument `{arg}`"
                ));
            }
        }
    }
    Ok(AgentAction::PointerClick {
        x: point.0,
        y: point.1,
        button,
    })
}

fn effect_form_attachment_resource(args: &[String]) -> Result<serde_json::Value, String> {
    let value = args
        .first()
        .ok_or_else(|| "attach requires a resource argument".to_owned())?;
    if args.len() > 1 {
        return Err("attach received too many positional arguments".to_owned());
    }
    Ok(parse_string_label(value).map_or_else(
        || serde_json::json!({ "label": value }),
        |value| serde_json::json!({ "label": value }),
    ))
}

fn parse_viewport_point_label(value: &str) -> Result<(u32, u32), String> {
    let value = value.trim();
    let body = value
        .strip_prefix("viewport_point(")
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(value);
    let parts = split_top_level_args(body);
    let [x, y] = parts.as_slice() else {
        return Err("viewport point requires x and y".to_owned());
    };
    Ok((parse_u32_label(x)?, parse_u32_label(y)?))
}

fn parse_pointer_button_label(value: &str) -> Result<PointerButton, String> {
    match value.trim().trim_start_matches('.') {
        "primary" => Ok(PointerButton::Primary),
        "secondary" => Ok(PointerButton::Secondary),
        "middle" => Ok(PointerButton::Middle),
        other => Err(format!("unsupported pointer button `{other}`")),
    }
}

fn parse_predicate_label(value: &str) -> Result<Predicate, String> {
    let value = value.trim();
    if let Some(body) = value
        .strip_prefix("exists(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return Ok(Predicate::Exists {
            probe: parse_probe_label(body)?,
        });
    }
    if let Some(body) = value
        .strip_prefix("all(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return split_top_level_args(body)
            .into_iter()
            .map(parse_predicate_label)
            .collect::<Result<Vec<_>, _>>()
            .map(|predicates| Predicate::All { predicates });
    }
    if let Some(body) = value
        .strip_prefix("any(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return split_top_level_args(body)
            .into_iter()
            .map(parse_predicate_label)
            .collect::<Result<Vec<_>, _>>()
            .map(|predicates| Predicate::Any { predicates });
    }
    if let Some(body) = value
        .strip_prefix("not(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return parse_predicate_label(body).map(|predicate| Predicate::Not {
            predicate: Box::new(predicate),
        });
    }
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
        value: Box::new(parse_agent_value_label(expected)?),
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
    if let Some(path) = value
        .strip_prefix("state(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return Ok(Probe::StatePath {
            path: parse_string_label(path).unwrap_or_else(|| path.to_owned()),
        });
    }
    if let Some(path) = value
        .strip_prefix("observation(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return Ok(Probe::ObservationField {
            path: parse_string_label(path).unwrap_or_else(|| path.to_owned()),
        });
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
        value if value.ends_with("f32") || value.ends_with("f64") => value
            .trim_end_matches("f32")
            .trim_end_matches("f64")
            .parse::<f64>()
            .map(AgentValue::F64)
            .map_err(|_| format!("invalid float literal `{value}`")),
        value if value.ends_with("u32") || value.ends_with("u64") || value.ends_with("usize") => {
            value
                .trim_end_matches("usize")
                .trim_end_matches("u32")
                .trim_end_matches("u64")
                .parse::<u64>()
                .map(AgentValue::U64)
                .map_err(|_| format!("invalid unsigned integer literal `{value}`"))
        }
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

fn parse_agent_value_map_label(value: &str) -> Result<BTreeMap<String, AgentValue>, String> {
    let Some(body) = value
        .trim()
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    else {
        return Err(format!("expected invoke args record, got `{value}`"));
    };
    split_top_level_args(body)
        .into_iter()
        .map(|field| {
            record_field_arg(field)
                .ok_or_else(|| format!("expected invoke arg field, got `{field}`"))
                .and_then(|(name, value)| {
                    parse_agent_value_label(value).map(|value| (name.to_owned(), value))
                })
        })
        .collect()
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

fn split_top_level_args(value: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut start = 0usize;
    let mut depth = 0u32;
    let mut in_string = false;
    let mut escape = false;
    for (index, ch) in value.char_indices() {
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' | '[' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let arg = value[start..index].trim();
                if !arg.is_empty() {
                    args.push(arg);
                }
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    let arg = value[start..].trim();
    if !arg.is_empty() {
        args.push(arg);
    }
    args
}

fn named_arg(arg: &str) -> Option<(&str, &str)> {
    arg.split_once(" = ")
        .map(|(name, value)| (name.trim(), value.trim()))
}

fn record_field_arg(arg: &str) -> Option<(&str, &str)> {
    arg.split_once('=')
        .map(|(name, value)| (name.trim(), value.trim()))
        .filter(|(name, _)| !name.is_empty())
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

fn parse_action_label(value: &str) -> String {
    parse_string_label(value).unwrap_or_else(|| value.strip_prefix('.').unwrap_or(value).to_owned())
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
        Predicate::DiagnosticsHasError => diagnostics_has_error(observation),
        Predicate::All { predicates } => predicates
            .iter()
            .all(|predicate| predicate_matches(predicate, observation)),
        Predicate::Any { predicates } => predicates
            .iter()
            .any(|predicate| predicate_matches(predicate, observation)),
        Predicate::Not { predicate } => !predicate_matches(predicate, observation),
    }
}

fn diagnostics_has_error(observation: &ObservationEnvelope) -> bool {
    observation
        .payload
        .get("diagnostics")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|diagnostics| {
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .get("severity")
                    .and_then(serde_json::Value::as_str)
                    == Some("error")
            })
        })
}

fn observation_value(probe: &Probe, observation: &ObservationEnvelope) -> Option<AgentValue> {
    match probe {
        Probe::Signal { target } | Probe::Metric { target } => {
            observation.signals.get(target.as_str()).cloned()
        }
        Probe::StatePath { path } => observation
            .payload
            .get("state")
            .and_then(|state| json_path_value(state, path))
            .and_then(agent_value_from_json),
        Probe::ObservationField { path } if path == "tick" => {
            Some(AgentValue::I64(i64::try_from(observation.tick).ok()?))
        }
        Probe::ObservationField { path } if path == "frame_id" => {
            Some(AgentValue::String(observation.frame_id.clone()))
        }
        Probe::ObservationField { path } if path == "state_hash" => {
            Some(AgentValue::String(observation.state_hash.clone()))
        }
        Probe::ObservationField { path } if path == "render_hash" => {
            Some(AgentValue::String(observation.render_hash.clone()))
        }
        Probe::ObservationField { path } => path
            .strip_prefix("signals.")
            .and_then(|signal| observation.signals.get(signal).cloned())
            .or_else(|| {
                json_path_value(&observation.payload, path).and_then(agent_value_from_json)
            }),
    }
}

fn json_path_value<'a>(root: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    if let Some(value) = root.get(path) {
        return Some(value);
    }
    path.split('.')
        .try_fold(root, |value, segment| value.get(segment))
}

fn agent_value_from_json(value: &serde_json::Value) -> Option<AgentValue> {
    Some(match value {
        serde_json::Value::Null => AgentValue::Null,
        serde_json::Value::Bool(value) => AgentValue::Bool(*value),
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(AgentValue::I64)
            .or_else(|| value.as_u64().map(AgentValue::U64))
            .or_else(|| value.as_f64().map(AgentValue::F64))?,
        serde_json::Value::String(value) => AgentValue::String(value.clone()),
        serde_json::Value::Array(values) => AgentValue::List(
            values
                .iter()
                .map(agent_value_from_json)
                .collect::<Option<Vec<_>>>()?,
        ),
        serde_json::Value::Object(values) => AgentValue::Map(
            values
                .iter()
                .map(|(key, value)| Some((key.clone(), agent_value_from_json(value)?)))
                .collect::<Option<BTreeMap<_, _>>>()?,
        ),
    })
}

fn compare_values(actual: &AgentValue, op: CompareOp, expected: &AgentValue) -> bool {
    match op {
        CompareOp::Eq => agent_values_equal(actual, expected),
        CompareOp::NotEq => !agent_values_equal(actual, expected),
        CompareOp::Greater => {
            compare_numeric_values(actual, expected).is_some_and(i32::is_positive)
        }
        CompareOp::GreaterOrEqual => {
            compare_numeric_values(actual, expected).is_some_and(|order| order >= 0)
        }
        CompareOp::Less => compare_numeric_values(actual, expected).is_some_and(i32::is_negative),
        CompareOp::LessOrEqual => {
            compare_numeric_values(actual, expected).is_some_and(|order| order <= 0)
        }
    }
}

fn agent_values_equal(left: &AgentValue, right: &AgentValue) -> bool {
    match (left, right) {
        (AgentValue::Entity(left), AgentValue::String(right))
        | (AgentValue::String(right), AgentValue::Entity(left)) => left.as_str() == right,
        _ => left == right,
    }
}

fn compare_numeric_values(left: &AgentValue, right: &AgentValue) -> Option<i32> {
    Some(match (left, right) {
        (AgentValue::I64(left), AgentValue::I64(right)) => compare_order(left.cmp(right)),
        (AgentValue::U64(left), AgentValue::U64(right)) => compare_order(left.cmp(right)),
        (AgentValue::I64(left), AgentValue::U64(right)) => {
            if *left < 0 {
                -1
            } else {
                compare_order(u64::try_from(*left).ok()?.cmp(right))
            }
        }
        (AgentValue::U64(left), AgentValue::I64(right)) => {
            if *right < 0 {
                1
            } else {
                compare_order(left.cmp(&u64::try_from(*right).ok()?))
            }
        }
        (AgentValue::F64(left), AgentValue::F64(right)) => compare_order(left.partial_cmp(right)?),
        _ => return None,
    })
}

fn compare_order(order: std::cmp::Ordering) -> i32 {
    match order {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_agent_protocol::{
        AgentActionDispatch, AgentActionKind, AgentResourceBody, AgentResourceKind,
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
    use arcweft_debug_model::{
        event::{DebugEvent, DebugEventKind},
        sink::NullDebugEventSink,
    };
    use std::collections::BTreeMap;
    use std::convert::Infallible;

    #[derive(Default)]
    struct TestSession {
        observations: Vec<ObservationEnvelope>,
    }

    #[derive(Default)]
    struct RecordingDebugSink {
        events: Vec<DebugEvent>,
    }

    impl DebugEventSink for RecordingDebugSink {
        type Error = Infallible;

        fn append(&mut self, event: &DebugEvent) -> Result<(), Self::Error> {
            self.events.push(event.clone());
            Ok(())
        }

        fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl AgentSession for TestSession {
        type Error = Infallible;

        fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
            Ok(AgentSessionInfo {
                session_id: "session.test".to_owned(),
                program_hash: "hash".to_owned(),
                project_entities: Vec::new(),
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

        fn read_resource(&mut self, uri: &str) -> Result<AgentResource, Self::Error> {
            Ok(AgentResource {
                uri: uri.to_owned(),
                kind: AgentResourceKind::ObservationLatest,
                mime_type: "application/json".to_owned(),
                hash: "resource.hash".to_owned(),
                image: None,
                body: AgentResourceBody::Json(serde_json::json!({ "uri": uri })),
            })
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
            actions: Vec::new(),
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
            actions: Vec::new(),
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
        agent_controller_test_bundle(
            program,
            "agent.observe_smoke",
            "agent.observe_smoke.awfagent",
            "agent @agent.observe_smoke observe_smoke() { observe() }",
            AgentBudget::default(),
        )
    }

    fn capture_binding_bundle_with_budget(budget: AgentBudget) -> ArcweftBundle {
        agent_controller_test_bundle(
            capture_binding_program(),
            "agent.capture_binding",
            "agent.capture_binding.awfagent",
            "agent @agent.capture_binding capture_binding() { let shot = try capture(viewport()) }",
            budget,
        )
    }

    fn agent_controller_test_bundle(
        program: BytecodeProgram,
        agent_id: &str,
        source_label: &str,
        source_text: &str,
        budget: AgentBudget,
    ) -> ArcweftBundle {
        let stats = program.stats();
        let display = arcweft_render_text::LineDisplayCatalog::default();
        ArcweftBundle::new(
            BundleManifest {
                source_label: source_label.to_owned(),
                profile_id: None,
                profile_kind: None,
                entry: Some(format!("entry.{agent_id}")),
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
                label: source_label.to_owned(),
                text: source_text.to_owned(),
            },
            program,
            display,
        )
        .with_agent_manifest(AgentArtifactManifest {
            schema_version: 1,
            bundle_kind: AgentBundleKind::AgentController,
            agent_id: PublicId::new(agent_id).expect("valid agent id"),
            source_hash: StableHash::new("blake3:test").expect("valid source hash"),
            compiler_version: "test".to_owned(),
            project_binding: ProjectBinding {
                program_hash: StableHash::new("program-test").expect("valid program hash"),
                mode: ProjectBindingMode::Compatible,
                required_entities: Vec::new(),
            },
            declared_effects: Vec::new(),
            budget,
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

    fn read_resource_binding_program() -> BytecodeProgram {
        BytecodeProgram::from_runtime_plan(
            RuntimePlan::new(
                Some(FlowRuntimeId("agent.read_resource_binding".to_owned())),
                vec![RuntimeFlow {
                    id: FlowRuntimeId("agent.read_resource_binding".to_owned()),
                    ops: vec![
                        FlowOp::Await {
                            binding: Some(RuntimePattern::Ident("resource".to_owned())),
                            target: AwaitTarget::new(
                                NeedId("need.agent.read_resource".to_owned()),
                                TaskId("task.agent.read_resource".to_owned()),
                                HostTaskRequestTemplate::new(
                                    "agent",
                                    "read_resource",
                                    [HostTaskArgTemplate::positional(RuntimeExpr::Value(
                                        RuntimeValue::String("agent://resource/test".to_owned()),
                                    ))],
                                ),
                            ),
                            pending: Vec::new(),
                        },
                        FlowOp::ReturnExpr(RuntimeExpr::Field {
                            target: Box::new(RuntimeExpr::Field {
                                target: Box::new(RuntimeExpr::Local("resource".to_owned())),
                                field: "body".to_owned(),
                            }),
                            field: "json".to_owned(),
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
            RuntimeAgentPolicy::new([RuntimeAgentCapability::Observe]),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let report = runner
            .handle_host_request(AgentHostRequest::Wait(Box::new(WaitRequest {
                predicate: Predicate::Compare {
                    probe: Probe::Signal {
                        target: PublicId::new("signal.ready").expect("valid public id"),
                    },
                    op: CompareOp::Eq,
                    value: Box::new(AgentValue::Bool(true)),
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
            RuntimeAgentPolicy::new([RuntimeAgentCapability::Observe]),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let report = runner
            .handle_host_request(AgentHostRequest::Wait(Box::new(WaitRequest {
                predicate: Predicate::Compare {
                    probe: Probe::Signal {
                        target: PublicId::new("signal.current_flow").expect("valid public id"),
                    },
                    op: CompareOp::Eq,
                    value: Box::new(AgentValue::Entity(
                        PublicId::new("flow.opening").expect("valid public id"),
                    )),
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
                ref value,
            } if target.as_str() == "signal.current_flow"
                && matches!(
                    value.as_ref(),
                    AgentValue::Entity(value) if value.as_str() == "flow.opening"
                )
        ));
    }

    #[test]
    fn effect_form_observe_defaults_to_object_payloads() {
        let request = agent_host_request_from_call(&RuntimeCall {
            callee: "observe".to_owned(),
            args: Vec::new(),
        })
        .expect("effect-form observe lowers");

        let AgentHostRequest::Observe(request) = request else {
            panic!("expected observe host request");
        };
        assert!(request.include_objects);
        assert!(!request.include_images);
        assert!(!request.include_logs);
    }

    #[test]
    fn effect_form_advance_text_call_lowers_to_host_action() {
        let request = agent_host_request_from_call(&RuntimeCall {
            callee: "advance_text".to_owned(),
            args: Vec::new(),
        })
        .expect("effect-form advance_text lowers");

        assert!(matches!(
            request,
            AgentHostRequest::Act(action) if matches!(*action, AgentAction::AdvanceText)
        ));
    }

    #[test]
    fn effect_form_invoke_call_lowers_to_host_action() {
        let request = agent_host_request_from_call(&RuntimeCall {
            callee: "invoke".to_owned(),
            args: vec![
                "@activity.inventory".to_owned(),
                ".open".to_owned(),
                r#"{ label = "main", index = 7u32, focused = true }"#.to_owned(),
            ],
        })
        .expect("effect-form invoke lowers");

        let AgentHostRequest::Act(action) = request else {
            panic!("expected action host request");
        };
        let AgentAction::Invoke {
            target,
            action,
            args,
        } = *action
        else {
            panic!("expected invoke action");
        };
        assert_eq!(target.as_str(), "activity.inventory");
        assert_eq!(action, "open");
        assert_eq!(
            args.get("label"),
            Some(&AgentValue::String("main".to_owned()))
        );
        assert_eq!(args.get("index"), Some(&AgentValue::U64(7)));
        assert_eq!(args.get("focused"), Some(&AgentValue::Bool(true)));
    }

    #[test]
    fn effect_form_pointer_click_lowers_to_physical_action() {
        let request = agent_host_request_from_call(&RuntimeCall {
            callee: "pointer.click".to_owned(),
            args: vec![
                "viewport_point(12u32, 34u32)".to_owned(),
                "button = .secondary".to_owned(),
            ],
        })
        .expect("effect-form pointer.click lowers");

        let AgentHostRequest::Act(action) = request else {
            panic!("expected action host request");
        };
        assert!(matches!(
            *action,
            AgentAction::PointerClick {
                x: 12,
                y: 34,
                button: PointerButton::Secondary
            }
        ));
    }

    #[test]
    fn physical_pointer_click_requires_runtime_policy_grant() {
        let request = AgentHostRequest::Act(Box::new(AgentAction::PointerClick {
            x: 12,
            y: 34,
            button: PointerButton::Primary,
        }));
        let mut denied = AgentRunner::new(
            TestSession::default(),
            NullDebugEventSink,
            NoopRagService,
            RuntimeAgentPolicy::new([RuntimeAgentCapability::Act]),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );
        let error = denied
            .handle_host_request(request.clone())
            .expect_err("physical action is denied without physical policy");
        assert!(matches!(
            error,
            AgentRunError::PolicyDenied("agent.act.physical")
        ));

        let mut granted = AgentRunner::new(
            TestSession::default(),
            NullDebugEventSink,
            NoopRagService,
            RuntimeAgentPolicy::new([RuntimeAgentCapability::ActPhysical]),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );
        let report = granted
            .handle_host_request(request)
            .expect("physical policy allows pointer.click host action");
        assert!(matches!(report.response, AgentHostResponse::Action(_)));
    }

    #[test]
    fn custom_task_attach_records_runtime_resource_payload() {
        let request = HostTaskRequest::Custom {
            capability: arcweft_core::task::HostCapabilityId("agent".to_owned()),
            operation: "attach".to_owned(),
            args: vec![RuntimePayload::new(RuntimeValue::Record(vec![
                runtime_field(
                    "uri",
                    RuntimeValue::String(
                        "arcweft://session/cli/observation/latest.json".to_owned(),
                    ),
                ),
                runtime_field(
                    "kind",
                    RuntimeValue::String("observation_latest".to_owned()),
                ),
            ]))],
        };
        let request = agent_host_request_from_task(&request).expect("attach task lowers");
        let mut runner = AgentRunner::new(
            TestSession::default(),
            RecordingDebugSink::default(),
            NoopRagService,
            RuntimeAgentPolicy::new([RuntimeAgentCapability::DebugRecord]),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let report = runner
            .handle_host_request(request)
            .expect("debug record policy allows attach");

        assert!(matches!(report.response, AgentHostResponse::Unit));
        assert!(
            runner
                .debug_mut()
                .events
                .iter()
                .any(|event| event.payload["attachment"]["uri"]
                    == "arcweft://session/cli/observation/latest.json")
        );
    }

    #[test]
    fn observation_payload_exposes_action_targets_for_contains_checks() {
        let response = AgentHostResponse::Observation(Box::new(ObservationEnvelope {
            tick: 7,
            frame_id: "frame.7".to_owned(),
            state_hash: "state.7".to_owned(),
            render_hash: "render.7".to_owned(),
            actions: vec![AgentActionTarget {
                id: "action.select_choice.choice.opening.listen".to_owned(),
                target: "choice.opening.listen".to_owned(),
                action: AgentActionKind::SelectChoice,
                kind: AgentActionDispatch::Semantic,
                enabled: true,
            }],
            signals: BTreeMap::new(),
            payload: serde_json::json!({}),
        }));

        let RuntimeValue::Record(fields) = runtime_payload_from_response(&response).0 else {
            panic!("observation payload is a record");
        };
        let RuntimeValue::Seq(actions) =
            &runtime_record_get(&fields, "actions").expect("actions field exists")
        else {
            panic!("actions field is a sequence");
        };

        assert_eq!(actions.len(), 1);
        assert!(matches!(
            actions.value_at(0),
            RuntimeValue::Record(ref fields)
                if runtime_record_get(fields, "target")
                    == Ok(&RuntimeValue::String("choice.opening.listen".to_owned()))
        ));
    }

    #[test]
    fn observation_payload_exposes_observed_objects_for_visual_regression_scripts() {
        let response = AgentHostResponse::Observation(Box::new(ObservationEnvelope {
            tick: 8,
            frame_id: "frame.8".to_owned(),
            state_hash: "state.8".to_owned(),
            render_hash: "render.8".to_owned(),
            actions: Vec::new(),
            signals: BTreeMap::new(),
            payload: serde_json::json!({
                "objects": [
                    {
                        "id": "object.dialogue.0.0",
                        "parent_id": "object.dialogue.0",
                        "entity": "dialogue.main",
                        "layer": "dialogue.rich_text",
                        "role": "dialogue_textbox",
                        "visible": true,
                        "enabled": true,
                        "bbox": {
                            "space": "viewport",
                            "x": 24,
                            "y": 384,
                            "width": 752,
                            "height": 168
                        },
                        "text": "Hello"
                    }
                ]
            }),
        }));

        let RuntimeValue::Record(fields) = runtime_payload_from_response(&response).0 else {
            panic!("observation payload is a record");
        };
        let RuntimeValue::Seq(objects) =
            &runtime_record_get(&fields, "objects").expect("objects field exists")
        else {
            panic!("objects field is a sequence");
        };
        let RuntimeValue::Record(object_fields) = objects.value_at(0) else {
            panic!("object is a record");
        };
        let RuntimeValue::Record(bbox_fields) =
            runtime_record_get(&object_fields, "bbox").expect("bbox field exists")
        else {
            panic!("bbox field is a record");
        };

        assert_eq!(
            runtime_record_get(&object_fields, "role"),
            Ok(&RuntimeValue::String("dialogue_textbox".to_owned()))
        );
        assert_eq!(
            runtime_record_get(bbox_fields, "width"),
            Ok(&RuntimeValue::u32(752))
        );
        assert_eq!(
            runtime_record_get(bbox_fields, "height"),
            Ok(&RuntimeValue::u32(168))
        );
    }

    #[test]
    fn effect_form_wait_call_lowers_composite_predicate() {
        let request = agent_host_request_from_call(&RuntimeCall {
            callee: "wait".to_owned(),
            args: vec![
                "all(exists(signal(@signal.ready)), not(metric(@metric.fps).lt(30.0f32)))"
                    .to_owned(),
                "timeout = 5s".to_owned(),
            ],
        })
        .expect("effect-form composite wait lowers");

        let AgentHostRequest::Wait(request) = request else {
            panic!("expected wait host request");
        };
        assert!(
            matches!(request.predicate, Predicate::All { ref predicates } if predicates.len() == 2)
        );
    }

    #[test]
    fn wait_matches_composite_float_predicate() {
        let session = TestSession {
            observations: vec![ObservationEnvelope {
                tick: 1,
                frame_id: "frame.1".to_owned(),
                state_hash: "state.1".to_owned(),
                render_hash: "render.1".to_owned(),
                actions: Vec::new(),
                signals: BTreeMap::from([
                    ("signal.ready".to_owned(), AgentValue::Bool(true)),
                    ("metric.fps".to_owned(), AgentValue::F64(60.0)),
                ]),
                payload: serde_json::json!({}),
            }],
        };
        let mut runner = AgentRunner::new(
            session,
            NullDebugEventSink,
            NoopRagService,
            RuntimeAgentPolicy::new([RuntimeAgentCapability::Observe]),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let report = runner
            .handle_host_request(AgentHostRequest::Wait(Box::new(WaitRequest {
                predicate: Predicate::All {
                    predicates: vec![
                        Predicate::Exists {
                            probe: Probe::Signal {
                                target: PublicId::new("signal.ready").expect("valid public id"),
                            },
                        },
                        Predicate::Not {
                            predicate: Box::new(Predicate::Compare {
                                probe: Probe::Metric {
                                    target: PublicId::new("metric.fps").expect("valid public id"),
                                },
                                op: CompareOp::Less,
                                value: Box::new(AgentValue::F64(30.0)),
                            }),
                        },
                    ],
                },
                timeout_millis: 5,
                stable_frames: 1,
                poll_frames: 1,
            })))
            .expect("composite wait succeeds");

        assert!(matches!(
            report.response,
            AgentHostResponse::Observation(observation) if observation.tick == 1
        ));
    }

    #[test]
    fn wait_matches_state_and_observation_field_predicates() {
        let session = TestSession {
            observations: vec![ObservationEnvelope {
                tick: 2,
                frame_id: "frame.2".to_owned(),
                state_hash: "state.2".to_owned(),
                render_hash: "render.2".to_owned(),
                actions: Vec::new(),
                signals: BTreeMap::new(),
                payload: serde_json::json!({
                    "state": {
                        "route.phase": "opening"
                    }
                }),
            }],
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
                predicate: Predicate::All {
                    predicates: vec![
                        Predicate::Compare {
                            probe: Probe::StatePath {
                                path: "route.phase".to_owned(),
                            },
                            op: CompareOp::Eq,
                            value: Box::new(AgentValue::String("opening".to_owned())),
                        },
                        Predicate::Compare {
                            probe: Probe::ObservationField {
                                path: "tick".to_owned(),
                            },
                            op: CompareOp::GreaterOrEqual,
                            value: Box::new(AgentValue::I64(2)),
                        },
                    ],
                },
                timeout_millis: 5,
                stable_frames: 1,
                poll_frames: 1,
            })))
            .expect("state and observation wait succeeds");

        assert!(matches!(
            report.response,
            AgentHostResponse::Observation(observation) if observation.tick == 2
        ));
    }

    #[test]
    fn wait_matches_diagnostics_has_error_predicate() {
        let session = TestSession {
            observations: vec![
                ObservationEnvelope {
                    tick: 1,
                    frame_id: "frame.1".to_owned(),
                    state_hash: "state.1".to_owned(),
                    render_hash: "render.1".to_owned(),
                    actions: Vec::new(),
                    signals: BTreeMap::new(),
                    payload: serde_json::json!({
                        "diagnostics": [
                            { "severity": "warning", "message": "not fatal" }
                        ]
                    }),
                },
                ObservationEnvelope {
                    tick: 2,
                    frame_id: "frame.2".to_owned(),
                    state_hash: "state.2".to_owned(),
                    render_hash: "render.2".to_owned(),
                    actions: Vec::new(),
                    signals: BTreeMap::new(),
                    payload: serde_json::json!({
                        "diagnostics": [
                            { "severity": "error", "message": "render mismatch" }
                        ]
                    }),
                },
            ],
        };
        let mut runner = AgentRunner::new(
            session,
            NullDebugEventSink,
            NoopRagService,
            RuntimeAgentPolicy::new([RuntimeAgentCapability::Observe]),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let report = runner
            .handle_host_request(AgentHostRequest::Wait(Box::new(WaitRequest {
                predicate: Predicate::DiagnosticsHasError,
                timeout_millis: 5,
                stable_frames: 1,
                poll_frames: 1,
            })))
            .expect("diagnostic wait succeeds");

        assert!(matches!(
            report.response,
            AgentHostResponse::Observation(observation) if observation.tick == 2
        ));
    }

    #[test]
    fn assertion_host_request_records_passed_expect() {
        let mut runner = AgentRunner::new(
            TestSession::default(),
            RecordingDebugSink::default(),
            NoopRagService,
            RuntimeAgentPolicy::default(),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let report = runner
            .handle_host_request(AgentHostRequest::Assert(Box::new(AgentAssertionRequest {
                kind: AgentAssertionKind::Expect,
                condition: true,
                message: "accepted should be true".to_owned(),
            })))
            .expect("passing assertion succeeds");

        assert!(matches!(report.response, AgentHostResponse::Unit));
        assert!(runner.debug_mut().events.iter().any(|event| {
            event.kind == DebugEventKind::Assertion
                && event.payload["kind"] == "expect"
                && event.payload["passed"] == serde_json::json!(true)
        }));
    }

    #[test]
    fn assertion_host_request_fails_deny_with_structured_event() {
        let mut runner = AgentRunner::new(
            TestSession::default(),
            RecordingDebugSink::default(),
            NoopRagService,
            RuntimeAgentPolicy::default(),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let error = runner
            .handle_host_request(AgentHostRequest::Assert(Box::new(AgentAssertionRequest {
                kind: AgentAssertionKind::Deny,
                condition: true,
                message: "route should not be open".to_owned(),
            })))
            .expect_err("failing deny stops the controller");

        assert!(matches!(
            error,
            AgentRunError::AssertionFailed {
                kind: AgentAssertionKind::Deny,
                ref message,
            } if message == "route should not be open"
        ));
        assert!(runner.debug_mut().events.iter().any(|event| {
            event.kind == DebugEventKind::Assertion
                && event.payload["kind"] == "deny"
                && event.payload["passed"] == serde_json::json!(false)
        }));
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
            RuntimeAgentPolicy::new([
                RuntimeAgentCapability::Observe,
                RuntimeAgentCapability::DebugRecord,
            ]),
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
            RuntimeAgentPolicy::new([
                RuntimeAgentCapability::Observe,
                RuntimeAgentCapability::DebugRecord,
            ]),
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
    fn controller_bundle_rejects_strict_project_binding_mismatch_before_execution() {
        let session = TestSession {
            observations: vec![observation(1, true)],
        };
        let mut runner = AgentRunner::new(
            session,
            RecordingDebugSink::default(),
            NoopRagService,
            RuntimeAgentPolicy::new([
                RuntimeAgentCapability::Observe,
                RuntimeAgentCapability::DebugRecord,
            ]),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );
        let mut bundle = observe_checkpoint_bundle();
        let manifest = bundle.agent.as_mut().expect("agent manifest exists");
        manifest.project_binding.mode = ProjectBindingMode::Strict;
        manifest.project_binding.program_hash =
            StableHash::new("different-program").expect("valid program hash");

        let error = runner
            .run_controller_bundle(&bundle, AgentControllerRunConfig::default())
            .expect_err("strict binding mismatch is rejected");

        assert!(matches!(
            error,
            AgentRunError::ProjectBindingMismatch {
                expected_program_hash,
                actual_program_hash,
                mode: ProjectBindingMode::Strict,
                detail,
            } if expected_program_hash == "different-program"
                && actual_program_hash == "hash"
                && detail == "strict program hash mismatch"
        ));
        assert_eq!(runner.session_mut().observations.len(), 1);
        assert!(runner.debug_mut().events.is_empty());
    }

    #[test]
    fn controller_bundle_rejects_compatible_project_entity_mismatch_before_execution() {
        let session = TestSession {
            observations: vec![observation(1, true)],
        };
        let mut runner = AgentRunner::new(
            session,
            RecordingDebugSink::default(),
            NoopRagService,
            RuntimeAgentPolicy::new([
                RuntimeAgentCapability::Observe,
                RuntimeAgentCapability::DebugRecord,
            ]),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );
        let mut bundle = observe_checkpoint_bundle();
        let manifest = bundle.agent.as_mut().expect("agent manifest exists");
        manifest.project_binding.required_entities = vec![RequiredEntity {
            public_id: PublicId::new("signal.ready").expect("valid public id"),
            kind: "signal".to_owned(),
            type_fingerprint: StableHash::new("shape.signal.ready.v1")
                .expect("valid type fingerprint"),
        }];

        let error = runner
            .run_controller_bundle(&bundle, AgentControllerRunConfig::default())
            .expect_err("compatible entity mismatch is rejected");

        assert!(matches!(
            error,
            AgentRunError::ProjectBindingMismatch {
                expected_program_hash,
                actual_program_hash,
                mode: ProjectBindingMode::Compatible,
                detail,
            } if expected_program_hash == "program-test"
                && actual_program_hash == "hash"
                && detail == "required entity signal.ready is missing"
        ));
        assert_eq!(runner.session_mut().observations.len(), 1);
        assert!(runner.debug_mut().events.is_empty());
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
    fn controller_bundle_enforces_agent_manifest_capture_budget() {
        let budget = AgentBudget {
            max_captures: 0,
            ..AgentBudget::default()
        };
        let bundle = capture_binding_bundle_with_budget(budget);
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

        let error = runner
            .run_controller_bundle(&bundle, AgentControllerRunConfig::default())
            .expect_err("capture budget stops controller bundle");

        assert!(matches!(
            error,
            AgentRunError::ControllerResourceBudgetExceeded {
                kind: "capture",
                limit: 0,
                attempted: 1,
            }
        ));
    }

    #[test]
    fn controller_bytecode_resumes_bound_resource_response_fields() {
        let mut runner = AgentRunner::new(
            TestSession::default(),
            NullDebugEventSink,
            NoopRagService,
            RuntimeAgentPolicy::new([RuntimeAgentCapability::ResourceRead]),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let report = runner
            .run_controller_bytecode(
                read_resource_binding_program(),
                AgentControllerRunConfig::default(),
            )
            .expect("controller bytecode runs");

        assert_eq!(report.host_calls, 1);
        assert!(matches!(
            &report.responses[0],
            AgentHostResponse::Resource(resource) if resource["uri"] == "agent://resource/test"
        ));
        assert!(matches!(
            report.final_status,
            Some(FlowFiberStatus::Done(FlowExit::Return(ref value)))
                if value == "{\"uri\":\"agent://resource/test\"}"
        ));
    }

    #[test]
    fn resource_runtime_payload_preserves_json_body_value() {
        let json_payload = runtime_resource_payload(&serde_json::json!({
            "uri": "agent://resource/json",
            "kind": "observation_latest",
            "mime_type": "application/json",
            "hash": "json.hash",
            "body": {
                "body_kind": "json",
                "body": {
                    "uri": "agent://resource/json",
                    "tick": 3,
                    "matched": true
                }
            }
        }));
        let RuntimeValue::Record(resource_fields) = json_payload else {
            panic!("resource payload is a record");
        };
        let RuntimeValue::Record(body_fields) =
            runtime_record_get(&resource_fields, "body").expect("body field exists")
        else {
            panic!("body payload is a record");
        };
        assert_eq!(
            runtime_record_string(body_fields, "kind").expect("body kind is a string"),
            "json"
        );
        assert_eq!(
            runtime_record_string(body_fields, "json").expect("body json is a string"),
            "{\"matched\":true,\"tick\":3,\"uri\":\"agent://resource/json\"}"
        );
        let RuntimeValue::Record(value_fields) =
            runtime_record_get(body_fields, "value").expect("body value exists")
        else {
            panic!("json body value is a record");
        };
        assert_eq!(
            runtime_record_string(value_fields, "uri").expect("json uri is a string"),
            "agent://resource/json"
        );
        assert!(matches!(
            runtime_record_get(value_fields, "matched").expect("matched field exists"),
            RuntimeValue::Bool(true)
        ));
    }

    #[test]
    fn resource_runtime_payload_preserves_text_body_value() {
        let text_payload = runtime_resource_payload(&serde_json::json!({
            "uri": "agent://resource/text",
            "kind": "logs",
            "mime_type": "text/plain",
            "hash": "text.hash",
            "body": {
                "body_kind": "text",
                "body": "hello"
            }
        }));
        let RuntimeValue::Record(resource_fields) = text_payload else {
            panic!("resource payload is a record");
        };
        let RuntimeValue::Record(body_fields) =
            runtime_record_get(&resource_fields, "body").expect("body field exists")
        else {
            panic!("body payload is a record");
        };
        assert_eq!(
            runtime_record_string(body_fields, "kind").expect("body kind is a string"),
            "text"
        );
        assert_eq!(
            runtime_record_string(body_fields, "text").expect("body text is a string"),
            "hello"
        );
        assert_eq!(
            runtime_record_string(body_fields, "value").expect("body value is a string"),
            "hello"
        );
    }

    #[test]
    fn resource_runtime_payload_preserves_bytes_body_value() {
        let bytes_payload = runtime_resource_payload(&serde_json::json!({
            "uri": "agent://resource/image",
            "kind": "image",
            "mime_type": "image/png",
            "hash": "image.hash",
            "body": {
                "body_kind": "bytes_base64",
                "body": {
                    "encoding": "base64",
                    "data": "aGVsbG8="
                }
            }
        }));
        let RuntimeValue::Record(resource_fields) = bytes_payload else {
            panic!("resource payload is a record");
        };
        let RuntimeValue::Record(body_fields) =
            runtime_record_get(&resource_fields, "body").expect("body field exists")
        else {
            panic!("body payload is a record");
        };
        assert_eq!(
            runtime_record_string(body_fields, "kind").expect("body kind is a string"),
            "bytes_base64"
        );
        assert_eq!(
            runtime_record_string(body_fields, "base64").expect("body data is a string"),
            "aGVsbG8="
        );
        assert_eq!(
            runtime_record_string(body_fields, "encoding").expect("body encoding is a string"),
            "base64"
        );
        let RuntimeValue::Record(value_fields) =
            runtime_record_get(body_fields, "value").expect("body value exists")
        else {
            panic!("bytes body value is a record");
        };
        assert_eq!(
            runtime_record_string(value_fields, "data").expect("body value data is a string"),
            "aGVsbG8="
        );
    }

    #[test]
    fn rag_context_runtime_payload_exposes_summary_fields() {
        let rag_payload = runtime_rag_context_payload(&serde_json::json!({
            "query": {
                "text": "why did opening flow stall?"
            },
            "items": [
                { "id": "item.1" },
                { "id": "item.2" }
            ],
            "truncated": true
        }));
        let RuntimeValue::Record(fields) = rag_payload else {
            panic!("RAG context payload is a record");
        };

        assert_eq!(
            runtime_record_string(&fields, "summary").expect("summary is a string"),
            "2 RAG context item(s) for `why did opening flow stall?`"
        );
        assert_eq!(
            runtime_record_get(&fields, "item_count").expect("item_count exists"),
            &RuntimeValue::usize(2)
        );
        assert!(matches!(
            runtime_record_get(&fields, "truncated").expect("truncated exists"),
            RuntimeValue::Bool(true)
        ));
        assert_eq!(
            runtime_record_string(&fields, "json").expect("json is a string"),
            "{\"items\":[{\"id\":\"item.1\"},{\"id\":\"item.2\"}],\"query\":{\"text\":\"why did opening flow stall?\"},\"truncated\":true}"
        );
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
