use std::collections::BTreeMap;

use arcweft_bundle::{ArcweftBundle, BundleKind};
use arcweft_core::{
    bytecode::BytecodeProgram,
    effect::{LineEffectRequest, RuntimeAssertionFailure},
    engine::{EngineStartError, FlowFiberStatus},
    entry::{AgentBudget, RuntimeCallableExecutableCode},
    executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor},
    plan::{EntryRuntimeId, RuntimeEntryKind, RuntimeEntryTarget},
    step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
    task::{LogicalEpoch, TaskEvent, TaskEventKind, TaskSequence},
};
use arcweft_debug_model::{
    event::{DebugEvent, DebugEventKind},
    sink::DebugEventSink,
};

use crate::assertion::{
    agent_assertion_failure_message, agent_assertion_kind_label, agent_assertion_passed,
};
use crate::budget::{
    AgentBudgetContext, AgentBudgetTracker, compatible_entity_mismatch,
    effective_controller_budget, effective_wait_request, project_binding_mismatch,
    record_budget_u32, record_budget_u64,
};
use crate::config::{
    AgentControllerRunConfig, AgentControllerRunReport, AgentHostCallReport, AgentRunnerConfig,
    AgentRunnerResult,
};
use crate::effect_policy::{AgentEffectAuthorization, AgentEffectRegistry};
use crate::error::AgentRunError;
use crate::host_request::{agent_host_request_from_effect, agent_host_request_from_task};
use crate::policy::{RuntimeAgentCapability, RuntimeAgentPolicy};
use crate::predicate::predicate_matches;
use crate::runtime_payload::{project_graph_neighborhood, runtime_payload_from_response};
use crate::session::{AgentSession, RagService};

use arcweft_agent_protocol::{
    artifact::{AgentArtifactManifest, AgentBundleKind, ProjectBinding, ProjectBindingMode},
    ids::{AgentProjectGraphSymbolId, AgentResourceUri, PublicId, StableHash},
    protocol::{
        AgentAction, AgentAssertionRequest, AgentHostRequest, AgentHostResponse, CaptureRequest,
        ObservationEnvelope, ObserveRequest, RagRequest, WaitRequest,
    },
};

/// Runner for Agent controller host calls.
pub struct AgentRunner<S, D, R> {
    session: S,
    debug: D,
    rag: R,
    policy: RuntimeAgentPolicy,
    authorization: Option<AgentEffectAuthorization>,
    config: AgentRunnerConfig,
    sequence: u64,
}

/// Builds the executor used for one Agent controller bytecode run.
///
/// The default factory returns the bytecode VM, while REPL/dev policies can
/// provide a tiered executor without changing host-call dispatch.
pub trait AgentControllerExecutorFactory<S, D, R>
where
    S: AgentSession,
    D: DebugEventSink,
    R: RagService,
{
    type Executor: RuntimeExecutor;

    fn build(&mut self, program: BytecodeProgram) -> AgentRunnerResult<Self::Executor, S, D, R>;

    fn start(
        &mut self,
        executor: &mut Self::Executor,
        entry: &EntryRuntimeId,
    ) -> Result<(), EngineStartError>;
}

/// Default Agent controller executor factory.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BytecodeVmAgentControllerExecutorFactory;

impl<S, D, R> AgentControllerExecutorFactory<S, D, R> for BytecodeVmAgentControllerExecutorFactory
where
    S: AgentSession,
    D: DebugEventSink,
    R: RagService,
{
    type Executor = ArcweftRuntimeExecutor;

    fn build(&mut self, program: BytecodeProgram) -> AgentRunnerResult<Self::Executor, S, D, R> {
        ArcweftRuntimeExecutor::from_bytecode(program, ArcweftExecutionTier::StructuredVm)
            .map_err(AgentRunError::Bytecode)
    }

    fn start(
        &mut self,
        executor: &mut Self::Executor,
        entry: &EntryRuntimeId,
    ) -> Result<(), EngineStartError> {
        executor.start_structured_entry(entry)
    }
}

/// Agent controller executor tier policy used by tooling and REPL/dev adapters.
///
/// Product paths keep using [`BytecodeVmAgentControllerExecutorFactory`]. This
/// policy is intentionally runner-local so selecting a dev tier never makes
/// product players depend on REPL, compiler, codegen, JIT, or AOT crates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentControllerExecutorTierPolicy {
    requested: ArcweftExecutionTier,
    allow_vm_fallback: bool,
}

/// Agent controller executor factory that can request a non-default tier while
/// preserving bytecode-VM fallback semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TieredAgentControllerExecutorFactory {
    policy: AgentControllerExecutorTierPolicy,
    last_requested: Option<ArcweftExecutionTier>,
    last_built: Option<ArcweftExecutionTier>,
    fell_back_to_vm: bool,
}

impl AgentControllerExecutorTierPolicy {
    /// Product and stable runner policy: request the structured bytecode VM.
    #[must_use]
    pub const fn bytecode_vm_first() -> Self {
        Self {
            requested: ArcweftExecutionTier::StructuredVm,
            allow_vm_fallback: true,
        }
    }

    /// REPL/dev policy: request a tier but keep VM fallback enabled.
    #[must_use]
    pub const fn repl_dev(requested: ArcweftExecutionTier) -> Self {
        Self {
            requested,
            allow_vm_fallback: true,
        }
    }

    #[must_use]
    pub const fn requested_tier(self) -> ArcweftExecutionTier {
        self.requested
    }

    #[must_use]
    pub const fn allow_vm_fallback(self) -> bool {
        self.allow_vm_fallback
    }

    #[must_use]
    pub const fn with_vm_fallback(self, allow_vm_fallback: bool) -> Self {
        Self {
            allow_vm_fallback,
            ..self
        }
    }
}

impl TieredAgentControllerExecutorFactory {
    #[must_use]
    pub const fn new(policy: AgentControllerExecutorTierPolicy) -> Self {
        Self {
            policy,
            last_requested: None,
            last_built: None,
            fell_back_to_vm: false,
        }
    }

    #[must_use]
    pub const fn policy(self) -> AgentControllerExecutorTierPolicy {
        self.policy
    }

    #[must_use]
    pub const fn last_requested_tier(self) -> Option<ArcweftExecutionTier> {
        self.last_requested
    }

    #[must_use]
    pub const fn last_built_tier(self) -> Option<ArcweftExecutionTier> {
        self.last_built
    }

    #[must_use]
    pub const fn fell_back_to_vm(self) -> bool {
        self.fell_back_to_vm
    }
}

impl Default for AgentControllerExecutorTierPolicy {
    fn default() -> Self {
        Self::bytecode_vm_first()
    }
}

impl Default for TieredAgentControllerExecutorFactory {
    fn default() -> Self {
        Self::new(AgentControllerExecutorTierPolicy::bytecode_vm_first())
    }
}

impl<S, D, R> AgentControllerExecutorFactory<S, D, R> for TieredAgentControllerExecutorFactory
where
    S: AgentSession,
    D: DebugEventSink,
    R: RagService,
{
    type Executor = ArcweftRuntimeExecutor;

    fn build(&mut self, program: BytecodeProgram) -> AgentRunnerResult<Self::Executor, S, D, R> {
        let requested = self.policy.requested_tier();
        self.last_requested = Some(requested);
        match ArcweftRuntimeExecutor::from_bytecode(program.clone(), requested) {
            Ok(executor) => {
                self.last_built = Some(executor.tier());
                self.fell_back_to_vm = false;
                Ok(executor)
            }
            Err(_error)
                if self.policy.allow_vm_fallback()
                    && requested != ArcweftExecutionTier::StructuredVm =>
            {
                let executor = ArcweftRuntimeExecutor::from_bytecode(
                    program,
                    ArcweftExecutionTier::StructuredVm,
                )
                .map_err(AgentRunError::Bytecode)?;
                self.last_built = Some(executor.tier());
                self.fell_back_to_vm = true;
                Ok(executor)
            }
            Err(error) => Err(AgentRunError::Bytecode(error)),
        }
    }

    fn start(
        &mut self,
        executor: &mut Self::Executor,
        entry: &EntryRuntimeId,
    ) -> Result<(), EngineStartError> {
        executor.start_structured_entry(entry)
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
            authorization: None,
            config,
            sequence: 0,
        }
    }

    pub fn session_mut(&mut self) -> &mut S {
        &mut self.session
    }

    pub fn into_session(self) -> S {
        self.session
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
        if let Some(authorization) = &self.authorization {
            authorization
                .ensure_request(&request)
                .map_err(AgentRunError::EffectPolicy)?;
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
            AgentHostRequest::EntityMetadata { entity } => {
                self.handle_entity_metadata_request(&entity)?
            }
            AgentHostRequest::ProjectGraphNeighborhood { root, depth } => {
                self.handle_project_graph_neighborhood_request(&root, depth)?
            }
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
            | AgentAction::Invoke(_)
            | AgentAction::Scroll(_) => RuntimeAgentCapability::Act,
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
        self.emit(
            DebugEventKind::ResourceRead,
            None,
            serde_json::to_value(&resource).unwrap_or(serde_json::Value::Null),
        )?;
        Ok(AgentHostResponse::Resource(Box::new(
            serde_json::to_value(resource).unwrap_or(serde_json::Value::Null),
        )))
    }

    fn handle_entity_metadata_request(
        &mut self,
        entity: &PublicId,
    ) -> AgentRunnerResult<AgentHostResponse, S, D, R> {
        self.ensure(RuntimeAgentCapability::DebugRead)?;
        let session_info = self.session.info().map_err(AgentRunError::Session)?;
        let metadata = session_info
            .project_entities
            .into_iter()
            .find(|candidate| candidate.public_id == *entity)
            .ok_or_else(|| AgentRunError::ProjectEntityMetadataMissing {
                entity: entity.as_str().to_owned(),
            })?;
        self.emit(
            DebugEventKind::Diagnostic,
            None,
            serde_json::json!({
                "entity_metadata": {
                    "id": metadata.public_id.as_str(),
                    "kind": metadata.kind.as_str(),
                    "semantic_hash": metadata.semantic_hash.as_str(),
                    "source_anchor": metadata.source_anchor.as_ref(),
                }
            }),
        )?;
        Ok(AgentHostResponse::EntityMetadata(Box::new(metadata)))
    }

    fn handle_project_graph_neighborhood_request(
        &mut self,
        root: &AgentProjectGraphSymbolId,
        depth: u32,
    ) -> AgentRunnerResult<AgentHostResponse, S, D, R> {
        self.ensure(RuntimeAgentCapability::DebugRead)?;
        let session_info = self.session.info().map_err(AgentRunError::Session)?;
        let neighborhood = project_graph_neighborhood(&session_info.project_graph, root, depth)
            .ok_or_else(|| AgentRunError::ProjectGraphSymbolMissing {
                entity: root.as_str().to_owned(),
            })?;
        self.emit(
            DebugEventKind::Diagnostic,
            None,
            serde_json::json!({
                "project_graph_neighborhood": {
                    "root": root.as_str(),
                    "depth": depth,
                    "symbol_count": neighborhood.symbols.len(),
                    "edge_count": neighborhood.edges.len(),
                }
            }),
        )?;
        Ok(AgentHostResponse::ProjectGraphNeighborhood(Box::new(
            neighborhood,
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
        entry: &EntryRuntimeId,
        config: AgentControllerRunConfig,
    ) -> AgentRunnerResult<AgentControllerRunReport, S, D, R> {
        let mut factory = BytecodeVmAgentControllerExecutorFactory;
        self.run_controller_bytecode_with_executor_factory_and_budget(
            program,
            entry,
            config,
            effective_controller_budget(AgentBudget::default(), config),
            &mut factory,
        )
    }

    /// Runs one Agent controller bytecode program through a supplied executor
    /// factory.
    pub fn run_controller_bytecode_with_executor_factory<F>(
        &mut self,
        program: BytecodeProgram,
        entry: &EntryRuntimeId,
        config: AgentControllerRunConfig,
        factory: &mut F,
    ) -> AgentRunnerResult<AgentControllerRunReport, S, D, R>
    where
        F: AgentControllerExecutorFactory<S, D, R>,
    {
        self.run_controller_bytecode_with_executor_factory_and_budget(
            program,
            entry,
            config,
            effective_controller_budget(AgentBudget::default(), config),
            factory,
        )
    }

    fn run_controller_bytecode_with_executor_factory_and_budget<F>(
        &mut self,
        program: BytecodeProgram,
        entry: &EntryRuntimeId,
        config: AgentControllerRunConfig,
        budget: AgentBudget,
        factory: &mut F,
    ) -> AgentRunnerResult<AgentControllerRunReport, S, D, R>
    where
        F: AgentControllerExecutorFactory<S, D, R>,
    {
        Self::validate_controller_program_entry(&program, entry)?;
        let mut executor = factory.build(program)?;
        factory
            .start(&mut executor, entry)
            .map_err(AgentRunError::ControllerEntryStart)?;
        self.run_controller_executor_with_budget(&mut executor, config, budget)
    }

    fn validate_controller_program_entry(
        program: &BytecodeProgram,
        selected: &EntryRuntimeId,
    ) -> AgentRunnerResult<(), S, D, R> {
        let invalid = |detail: String| AgentRunError::InvalidControllerEntry { detail };
        let mut entries = program.entries.iter().filter(|entry| &entry.id == selected);
        let Some(entry) = entries.next() else {
            return Err(invalid(format!(
                "selected entry `{}` is missing",
                selected.canonical_label(),
            )));
        };
        if entries.next().is_some() {
            return Err(invalid(format!(
                "selected entry `{}` is duplicated",
                selected.canonical_label(),
            )));
        }
        if entry.kind != RuntimeEntryKind::Agent {
            return Err(invalid(format!(
                "selected entry `{}` has kind `{}` instead of `agent`",
                selected.canonical_label(),
                entry.kind.as_str(),
            )));
        }
        let RuntimeEntryTarget::Controller(controller_flow) = &entry.target else {
            return Err(invalid(format!(
                "selected entry `{}` does not target a controller flow",
                selected.canonical_label(),
            )));
        };
        let Some(roles) = entry.roles.agent() else {
            return Err(invalid(format!(
                "selected entry `{}` has no Agent roles",
                selected.canonical_label(),
            )));
        };
        if entry.binding != roles.binding {
            return Err(invalid(format!(
                "selected entry `{}` has conflicting binding identities",
                selected.canonical_label(),
            )));
        }
        let mut callables = program.callable_executables.iter().filter(|callable| {
            callable.callable == roles.controller.callable
                && callable.contract == roles.controller.contract
                && matches!(
                    &callable.code,
                    RuntimeCallableExecutableCode::ControllerFlow(flow)
                        if flow == controller_flow
                )
        });
        if callables.next().is_none() || callables.next().is_some() {
            return Err(invalid(format!(
                "selected entry `{}` does not own exactly one controller executable",
                selected.canonical_label(),
            )));
        }
        let mut flow_executables = program.flow_executables.iter().filter(|flow| {
            flow.flow == *controller_flow
                && flow.parameters.is_empty()
                && flow.controller.as_ref() == Some(&roles.controller)
                && flow.contract.as_bytes() == roles.controller.contract.as_bytes()
        });
        if flow_executables.next().is_none() || flow_executables.next().is_some() {
            return Err(invalid(format!(
                "selected entry `{}` does not own exactly one controller flow executable",
                selected.canonical_label(),
            )));
        }
        let mut flows = program
            .flows
            .iter()
            .filter(|flow| flow.id == *controller_flow);
        if flows.next().is_none() || flows.next().is_some() {
            return Err(invalid(format!(
                "selected entry `{}` does not own exactly one bytecode flow",
                selected.canonical_label(),
            )));
        }
        Ok(())
    }

    fn run_controller_executor_with_budget<E>(
        &mut self,
        executor: &mut E,
        config: AgentControllerRunConfig,
        budget: AgentBudget,
    ) -> AgentRunnerResult<AgentControllerRunReport, S, D, R>
    where
        E: RuntimeExecutor,
    {
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
            assertion_failures: Vec::new(),
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
                if let LineEffectRequest::Assert(assertion) = effect {
                    report
                        .assertion_failures
                        .push(RuntimeAssertionFailure::new(assertion.clone()));
                    continue;
                }
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
                | FlowFiberStatus::Dialogue(_)
                | FlowFiberStatus::Waiting(_)
                | FlowFiberStatus::NeedWaiting(_)
                | FlowFiberStatus::WaitingMany(_)
                | FlowFiberStatus::HostCall(_)
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
        let entry = Self::validate_controller_artifact(bundle, manifest)?;
        self.validate_project_binding(&manifest.project_binding)?;
        let authorization = AgentEffectRegistry::canonical()
            .authorization_for_artifact(&manifest.verified_effects.inferred, &self.policy)
            .map_err(AgentRunError::EffectPolicy)?;
        let previous_policy = std::mem::replace(&mut self.policy, authorization.policy().clone());
        let previous_authorization = self.authorization.replace(authorization);
        let mut factory = BytecodeVmAgentControllerExecutorFactory;
        let result = self.run_controller_bytecode_with_executor_factory_and_budget(
            bundle.bytecode.program.clone(),
            &entry,
            config,
            effective_controller_budget(manifest.budget, config),
            &mut factory,
        );
        self.policy = previous_policy;
        self.authorization = previous_authorization;
        result
    }

    fn validate_controller_artifact(
        bundle: &ArcweftBundle,
        manifest: &AgentArtifactManifest,
    ) -> AgentRunnerResult<EntryRuntimeId, S, D, R> {
        let mismatch = |detail: String| AgentRunError::AgentArtifactMismatch { detail };
        if manifest.schema_version != 1 || manifest.bundle_kind != AgentBundleKind::AgentController
        {
            return Err(mismatch(
                "manifest is not the final Agent controller schema v1".to_owned(),
            ));
        }
        let entry_id = EntryRuntimeId::from_source_entity_body(manifest.entry_id.as_str())
            .map_err(|error| mismatch(format!("manifest entry ID is invalid: {error}")))?;
        if bundle.manifest.entry.as_deref() != Some(manifest.entry_id.as_str()) {
            return Err(mismatch(
                "bundle launch entry does not match the Agent artifact entry".to_owned(),
            ));
        }
        let [entry] = bundle.bytecode.program.entries.as_slice() else {
            return Err(mismatch(format!(
                "Agent controller artifact must contain exactly one entry, found {}",
                bundle.bytecode.program.entries.len(),
            )));
        };
        if entry.id != entry_id || entry.kind != RuntimeEntryKind::Agent {
            return Err(mismatch(
                "bytecode entry identity or kind does not match the Agent manifest".to_owned(),
            ));
        }
        let RuntimeEntryTarget::Controller(controller_flow) = &entry.target else {
            return Err(mismatch(
                "Agent bytecode entry does not target a controller flow".to_owned(),
            ));
        };
        let Some(roles) = entry.roles.agent() else {
            return Err(mismatch(
                "Agent bytecode entry is missing exact Agent roles".to_owned(),
            ));
        };
        if StableHash::from_blake3_bytes(*entry.binding.as_bytes()) != manifest.entry_binding_hash
            || StableHash::from_blake3_bytes(*roles.binding.as_bytes())
                != manifest.entry_binding_hash
            || roles.controller.callable.as_str() != manifest.controller_id.as_str()
            || StableHash::from_blake3_bytes(*roles.controller.contract.as_bytes())
                != manifest.controller_contract_hash
            || StableHash::from_blake3_bytes(*roles.policy.as_bytes()) != manifest.policy_hash
            || manifest.budget != roles.budget
        {
            return Err(mismatch(
                "Agent manifest identity, contract, policy, or budget does not match bytecode roles"
                    .to_owned(),
            ));
        }
        let [callable] = bundle.bytecode.program.callable_executables.as_slice() else {
            return Err(mismatch(format!(
                "Agent controller artifact must contain exactly one callable executable, found {}",
                bundle.bytecode.program.callable_executables.len(),
            )));
        };
        if callable.callable != roles.controller.callable
            || callable.contract != roles.controller.contract
            || !matches!(
                &callable.code,
                RuntimeCallableExecutableCode::ControllerFlow(flow) if flow == controller_flow
            )
        {
            return Err(mismatch(
                "Agent callable executable does not match the selected controller role".to_owned(),
            ));
        }
        let [flow_executable] = bundle.bytecode.program.flow_executables.as_slice() else {
            return Err(mismatch(format!(
                "Agent controller artifact must contain exactly one flow executable, found {}",
                bundle.bytecode.program.flow_executables.len(),
            )));
        };
        if flow_executable.flow != *controller_flow
            || !flow_executable.parameters.is_empty()
            || flow_executable.controller.as_ref() != Some(&roles.controller)
            || StableHash::from_blake3_bytes(*flow_executable.contract.as_bytes())
                != manifest.controller_contract_hash
        {
            return Err(mismatch(
                "Agent flow executable does not match the selected controller role".to_owned(),
            ));
        }
        let [flow] = bundle.bytecode.program.flows.as_slice() else {
            return Err(mismatch(format!(
                "Agent controller artifact must contain exactly one bytecode flow, found {}",
                bundle.bytecode.program.flows.len(),
            )));
        };
        let controller_label = controller_flow.public_label().into_string();
        if flow.id != *controller_flow
            || bundle.manifest.runtime.entry_flow.as_deref() != Some(controller_label.as_str())
        {
            return Err(mismatch(
                "Agent bytecode flow or runtime summary does not match the selected controller"
                    .to_owned(),
            ));
        }
        Ok(entry_id)
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

#[cfg(test)]
mod tier_policy_tests {
    use super::*;

    #[test]
    fn agent_controller_tier_policy_product_default_is_vm_first() {
        let policy = AgentControllerExecutorTierPolicy::default();

        assert_eq!(policy.requested_tier(), ArcweftExecutionTier::StructuredVm);
        assert!(policy.allow_vm_fallback());
    }

    #[test]
    fn agent_controller_tier_policy_repl_can_request_aot_with_vm_fallback() {
        let policy =
            AgentControllerExecutorTierPolicy::repl_dev(ArcweftExecutionTier::StructuredAot);
        let factory = TieredAgentControllerExecutorFactory::new(policy);

        assert_eq!(
            factory.policy().requested_tier(),
            ArcweftExecutionTier::StructuredAot
        );
        assert!(factory.policy().allow_vm_fallback());
        assert!(!factory.fell_back_to_vm());
    }
}
