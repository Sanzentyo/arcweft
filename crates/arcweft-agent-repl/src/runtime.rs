use std::collections::{BTreeMap, BTreeSet};

use arcweft_agent_protocol::protocol::{
    ActionResult, AgentAction, AgentSessionInfo, CaptureRequest, CaptureResult,
    ObservationEnvelope, ObserveRequest, RagRequest,
};
use arcweft_agent_protocol::resource::AgentResource;
use arcweft_agent_runner::config::{AgentControllerRunConfig, AgentRunnerConfig};
use arcweft_agent_runner::policy::{RuntimeAgentCapability, RuntimeAgentPolicy};
use arcweft_agent_runner::runner::AgentRunner;
use arcweft_agent_runner::session::{AgentSession, RagService};
use arcweft_debug_model::event::{DebugEvent, DebugEventKind};
use arcweft_debug_model::rag::RagContextPack;
use arcweft_debug_model::sink::DebugEventSink;

use crate::cell::CommittedReplCell;
use crate::evidence::{ReplDebugEventCount, ReplExecutionRecord, ReplHostEffectEvidence};

/// Runtime capabilities granted to committed-cell execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplCapabilitySet {
    allowed: BTreeSet<RuntimeAgentCapability>,
}

/// Public capability projection for commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplCapabilityReport {
    pub allowed: Vec<RuntimeAgentCapability>,
}

/// Borrowed runtime handles required for immediate VM execution.
pub struct ReplEvaluationRuntime<'a, S, D, R>
where
    S: AgentSession,
    D: DebugEventSink,
    R: RagService,
{
    pub session: &'a mut S,
    pub debug: &'a mut D,
    pub rag: &'a mut R,
    pub runner_config: AgentRunnerConfig,
    pub run_config: AgentControllerRunConfig,
}

struct BorrowedAgentSession<'a, S: AgentSession> {
    inner: &'a mut S,
}

struct BorrowedRagService<'a, R: RagService> {
    inner: &'a mut R,
}

struct ReplDebugTeeSink<'a, D: DebugEventSink> {
    inner: &'a mut D,
    events: Vec<DebugEvent>,
}

impl ReplCapabilitySet {
    #[must_use]
    pub fn new(capabilities: impl IntoIterator<Item = RuntimeAgentCapability>) -> Self {
        Self {
            allowed: capabilities.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn observe_only() -> Self {
        Self::new([RuntimeAgentCapability::Observe])
    }

    #[must_use]
    pub fn all_current() -> Self {
        Self::new([
            RuntimeAgentCapability::Observe,
            RuntimeAgentCapability::Act,
            RuntimeAgentCapability::ActPhysical,
            RuntimeAgentCapability::Capture,
            RuntimeAgentCapability::ResourceRead,
            RuntimeAgentCapability::DebugRead,
            RuntimeAgentCapability::DebugRecord,
            RuntimeAgentCapability::Rag,
        ])
    }

    #[must_use]
    pub fn runtime_policy(&self) -> RuntimeAgentPolicy {
        RuntimeAgentPolicy::new(self.allowed.iter().copied())
    }

    #[must_use]
    pub fn report(&self) -> ReplCapabilityReport {
        let allowed = self.allowed.iter().copied().collect::<Vec<_>>();
        ReplCapabilityReport { allowed }
    }
}

impl ReplCapabilityReport {
    #[must_use]
    pub fn allows(&self, capability: RuntimeAgentCapability) -> bool {
        self.allowed.contains(&capability)
    }
}

impl Default for ReplCapabilitySet {
    fn default() -> Self {
        Self::all_current()
    }
}

impl<'a, S, D, R> ReplEvaluationRuntime<'a, S, D, R>
where
    S: AgentSession,
    D: DebugEventSink,
    R: RagService,
{
    #[must_use]
    pub fn new(
        session: &'a mut S,
        debug: &'a mut D,
        rag: &'a mut R,
        runner_config: AgentRunnerConfig,
    ) -> Self {
        Self {
            session,
            debug,
            rag,
            runner_config,
            run_config: AgentControllerRunConfig::default(),
        }
    }

    #[must_use]
    pub const fn with_run_config(mut self, run_config: AgentControllerRunConfig) -> Self {
        self.run_config = run_config;
        self
    }
}

pub(crate) fn execute_committed_cell<S, D, R>(
    cell: &CommittedReplCell,
    runtime: ReplEvaluationRuntime<'_, S, D, R>,
    launch_policy: RuntimeAgentPolicy,
) -> ReplExecutionRecord
where
    S: AgentSession,
    D: DebugEventSink,
    R: RagService,
{
    let mut runner = AgentRunner::new(
        BorrowedAgentSession {
            inner: runtime.session,
        },
        ReplDebugTeeSink::new(runtime.debug),
        BorrowedRagService { inner: runtime.rag },
        launch_policy,
        runtime.runner_config,
    );
    match runner.run_controller_bundle(&cell.bundle, runtime.run_config) {
        Ok(report) => {
            ReplExecutionRecord::from_report(&report, runner.debug_mut().effect_evidence())
        }
        Err(error) => {
            ReplExecutionRecord::from_error(error.to_string(), runner.debug_mut().effect_evidence())
        }
    }
}

impl<S> AgentSession for BorrowedAgentSession<'_, S>
where
    S: AgentSession,
{
    type Error = S::Error;

    fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
        self.inner.info()
    }

    fn observe(&mut self, request: ObserveRequest) -> Result<ObservationEnvelope, Self::Error> {
        self.inner.observe(request)
    }

    fn act(&mut self, action: AgentAction) -> Result<ActionResult, Self::Error> {
        self.inner.act(action)
    }

    fn capture(&mut self, request: CaptureRequest) -> Result<CaptureResult, Self::Error> {
        self.inner.capture(request)
    }

    fn read_resource(&mut self, uri: &str) -> Result<AgentResource, Self::Error> {
        self.inner.read_resource(uri)
    }

    fn step_frames(&mut self, count: u32) -> Result<ObservationEnvelope, Self::Error> {
        self.inner.step_frames(count)
    }
}

impl<R> RagService for BorrowedRagService<'_, R>
where
    R: RagService,
{
    type Error = R::Error;

    fn query(&mut self, request: RagRequest) -> Result<RagContextPack, Self::Error> {
        self.inner.query(request)
    }
}

impl<D> ReplDebugTeeSink<'_, D>
where
    D: DebugEventSink,
{
    fn new(inner: &mut D) -> ReplDebugTeeSink<'_, D> {
        ReplDebugTeeSink {
            inner,
            events: Vec::new(),
        }
    }

    fn effect_evidence(&self) -> ReplHostEffectEvidence {
        let mut counts = BTreeMap::new();
        for event in &self.events {
            if repl_effectful_debug_event(event.kind) {
                *counts.entry(event.kind).or_insert(0) += 1;
            }
        }
        let event_kinds = counts
            .into_iter()
            .map(|(kind, count)| ReplDebugEventCount { kind, count })
            .collect::<Vec<_>>();
        let host_calls = event_kinds.iter().map(|item| item.count).sum();
        ReplHostEffectEvidence {
            host_calls,
            events_emitted: self.events.last().map_or(0, |event| event.sequence),
            partially_effectful: host_calls > 0,
            event_kinds,
        }
    }
}

impl<D> DebugEventSink for ReplDebugTeeSink<'_, D>
where
    D: DebugEventSink,
{
    type Error = D::Error;

    fn append(&mut self, event: &DebugEvent) -> Result<(), Self::Error> {
        self.events.push(event.clone());
        self.inner.append(event)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner.flush()
    }
}

fn repl_effectful_debug_event(kind: DebugEventKind) -> bool {
    matches!(
        kind,
        DebugEventKind::Observation
            | DebugEventKind::Action
            | DebugEventKind::Capture
            | DebugEventKind::Assertion
            | DebugEventKind::RagQuery
    )
}
