use crate::plan::FlowOp;
use crate::runtime_id::{
    DialogueActivationId, RuntimeDialogueEffectSiteId, RuntimeDialogueMarkId,
    RuntimeLineHandleSiteId, RuntimeLineHandleToken, RuntimeLineTaskNodeId,
    RuntimeLocalDeclarationId, RuntimePlanTypeId,
};
use crate::step::RuntimeDialogueContentEventKind;
use crate::time::LogicalDuration;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

mod activation;
mod handle;

pub(crate) use activation::{
    RuntimeDialogueActivationRegistry, RuntimeDialogueActivationTransaction,
    RuntimeDialogueRegistryCommitReceipt, RuntimeDialogueRegistrySaveSnapshot,
    RuntimeDialogueRegistrySnapshotError,
};

pub(crate) use handle::{
    AwbcRuntimeDialogueActivationSnapshot, AwbcRuntimePublishedDialogueHandlesSnapshot,
    RuntimeDialogueCommitReceipt, RuntimeDialogueTerminalKind, RuntimeHandleDropReceipt,
    RuntimePublishedDialogueHandles,
};

pub use handle::{
    LineRuntimeError, MAX_LINE_HANDLE_SITES, MAX_LINE_LIVE_HANDLES, MAX_LINE_SCHEDULED_CALLBACKS,
    RuntimeCueLease, RuntimeCueOrigin, RuntimeDialogueActivationState, RuntimeDialogueResultState,
    RuntimeHandleLease, RuntimeHandleLeaseState, RuntimeHandleOwnerSlot, RuntimeHandleResource,
    RuntimeLineHandleLedger, RuntimeLineHandleScope, RuntimeLineHandleSite,
    RuntimeLineHandleSiteKind, RuntimeScheduledLineTask, RuntimeScheduledState,
    RuntimeStageActorLease, RuntimeVoiceLease,
};

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub enum ScopeExit {
    #[default]
    Completed,
    Cancelled,
    Failed,
}

/// Sealed, plan-owned task graph activated with one dialogue content plan.
///
/// The graph never carries host request spellings or raw effects. Its only
/// executable leaves are typed flow operations, evaluated by the native flow
/// executor in a child fiber with the exact declared capture set.
#[derive(Clone, Debug, PartialEq)]
pub struct LineTaskGroup {
    captures: Box<[RuntimeLocalDeclarationId]>,
    activation_ops: Box<[FlowOp]>,
    result_type: RuntimePlanTypeId,
    handle_sites: Box<[RuntimeLineHandleSite]>,
    root: RuntimeLineTaskNodeId,
    nodes: Box<[LineTaskNode]>,
    cancel_rules: Box<[LineCancelRule]>,
    cleanup: LineTaskCleanup,
}

impl LineTaskGroup {
    pub(crate) fn new(
        captures: Box<[RuntimeLocalDeclarationId]>,
        activation_ops: Box<[FlowOp]>,
        result_type: RuntimePlanTypeId,
        handle_sites: Box<[RuntimeLineHandleSite]>,
        root: RuntimeLineTaskNodeId,
        nodes: Box<[LineTaskNode]>,
        cancel_rules: Box<[LineCancelRule]>,
        cleanup: LineTaskCleanup,
    ) -> Self {
        Self {
            captures,
            activation_ops,
            result_type,
            handle_sites,
            root,
            nodes,
            cancel_rules,
            cleanup,
        }
    }

    #[must_use]
    pub const fn captures(&self) -> &[RuntimeLocalDeclarationId] {
        &self.captures
    }

    #[must_use]
    pub const fn activation_ops(&self) -> &[FlowOp] {
        &self.activation_ops
    }

    #[must_use]
    pub const fn result_type(&self) -> RuntimePlanTypeId {
        self.result_type
    }

    #[must_use]
    pub const fn handle_sites(&self) -> &[RuntimeLineHandleSite] {
        &self.handle_sites
    }

    #[must_use]
    pub fn handle_site(&self, id: RuntimeLineHandleSiteId) -> Option<&RuntimeLineHandleSite> {
        self.handle_sites.get(id.index())
    }

    #[must_use]
    pub const fn root(&self) -> RuntimeLineTaskNodeId {
        self.root
    }

    #[must_use]
    pub const fn nodes(&self) -> &[LineTaskNode] {
        &self.nodes
    }

    #[must_use]
    pub const fn cancel_rules(&self) -> &[LineCancelRule] {
        &self.cancel_rules
    }

    #[must_use]
    pub const fn cleanup(&self) -> &LineTaskCleanup {
        &self.cleanup
    }

    #[must_use]
    pub fn node(&self, id: RuntimeLineTaskNodeId) -> Option<&LineTaskNode> {
        self.nodes.get(id.index())
    }

    /// Resolves a reducer command only at the native executor boundary. The
    /// shared reducer intentionally never sees `FlowOp` payloads.
    #[must_use]
    pub(crate) fn command_ops(&self, tag: &LineTaskWorkTag) -> &[FlowOp] {
        match tag.work {
            LineTaskWork::Node(node) => match self.node(node) {
                Some(LineTaskNode::Action(ops)) => ops,
                _ => &[],
            },
            LineTaskWork::Cancellation(mark) => self
                .cancel_rules
                .iter()
                .find(|rule| rule.trigger == mark)
                .map_or(&[], LineCancelRule::action),
            LineTaskWork::Cleanup(exit) => self.cleanup.actions(exit),
        }
    }
}

/// Read-only graph surface consumed by the common native/AWBC reducer.
/// Payload ownership is deliberately absent: executors map a command to their
/// own native ops or AWBC function body after reduction.
pub(crate) trait LineTaskPlanView {
    fn node_count(&self) -> usize;
    fn root_node(&self) -> RuntimeLineTaskNodeId;
    fn node_view(&self, id: RuntimeLineTaskNodeId) -> Option<LineTaskNodeView<'_>>;
    fn has_action(&self, node: RuntimeLineTaskNodeId) -> bool;
    fn cancellation_mark(
        &self,
        marks: &BTreeSet<RuntimeDialogueMarkId>,
    ) -> Option<RuntimeDialogueMarkId>;
    fn has_cancellation_work(&self, mark: RuntimeDialogueMarkId) -> bool;
    fn has_cleanup(&self, exit: ScopeExit) -> bool;
    fn scheduled_child(&self, _site: RuntimeLineHandleSiteId) -> Option<RuntimeLineTaskNodeId> {
        None
    }
}

/// Typed, activation-scoped events that may arm line-task children during one
/// reducer step. Rich-text effect sites deliberately do not share the authored
/// mark namespace: their identities are sealed by the dialogue content plan.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LineTaskReadyEvents<'a> {
    marks: &'a BTreeSet<RuntimeDialogueMarkId>,
    content_effects: &'a BTreeSet<RuntimeDialogueEffectSiteId>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct AcceptedLineTaskContentEvents {
    marks: BTreeSet<RuntimeDialogueMarkId>,
    effects: BTreeSet<RuntimeDialogueEffectSiteId>,
}

impl AcceptedLineTaskContentEvents {
    pub(crate) const fn ready(&self) -> LineTaskReadyEvents<'_> {
        LineTaskReadyEvents::new(&self.marks, &self.effects)
    }

    pub(crate) const fn marks(&self) -> &BTreeSet<RuntimeDialogueMarkId> {
        &self.marks
    }
}

impl<'a> LineTaskReadyEvents<'a> {
    pub(crate) const fn new(
        marks: &'a BTreeSet<RuntimeDialogueMarkId>,
        content_effects: &'a BTreeSet<RuntimeDialogueEffectSiteId>,
    ) -> Self {
        Self {
            marks,
            content_effects,
        }
    }

    const fn marks(self) -> &'a BTreeSet<RuntimeDialogueMarkId> {
        self.marks
    }

    const fn content_effects(self) -> &'a BTreeSet<RuntimeDialogueEffectSiteId> {
        self.content_effects
    }
}

#[derive(Clone, Debug)]
pub(crate) enum LineTaskNodeView<'a> {
    Sequence(&'a [RuntimeLineTaskNodeId]),
    Start(&'a [RuntimeLineTaskNodeId]),
    Parallel(&'a [RuntimeLineTaskNodeId]),
    Child {
        trigger: LineTaskTrigger,
        policy: LineTaskExitPolicy,
        scope: RuntimeLineTaskNodeId,
    },
    Action,
}

impl LineTaskPlanView for LineTaskGroup {
    fn node_count(&self) -> usize {
        self.nodes.len()
    }

    fn root_node(&self) -> RuntimeLineTaskNodeId {
        self.root
    }

    fn node_view(&self, id: RuntimeLineTaskNodeId) -> Option<LineTaskNodeView<'_>> {
        match self.node(id)? {
            LineTaskNode::Sequence(children) => Some(LineTaskNodeView::Sequence(children)),
            LineTaskNode::Start(children) => Some(LineTaskNodeView::Start(children)),
            LineTaskNode::Parallel { children, .. } => Some(LineTaskNodeView::Parallel(children)),
            LineTaskNode::Child {
                trigger,
                join_policy,
                cancel_policy,
                scope,
                ..
            } => Some(LineTaskNodeView::Child {
                trigger: trigger.clone(),
                policy: LineTaskExitPolicy::new(*join_policy, *cancel_policy),
                scope: *scope,
            }),
            LineTaskNode::Action(_) => Some(LineTaskNodeView::Action),
        }
    }

    fn cancellation_mark(
        &self,
        marks: &BTreeSet<RuntimeDialogueMarkId>,
    ) -> Option<RuntimeDialogueMarkId> {
        self.cancel_rules
            .iter()
            .find(|rule| marks.contains(&rule.trigger))
            .map(LineCancelRule::trigger)
    }

    fn has_action(&self, node: RuntimeLineTaskNodeId) -> bool {
        matches!(self.node(node), Some(LineTaskNode::Action(ops)) if !ops.is_empty())
    }

    fn has_cleanup(&self, exit: ScopeExit) -> bool {
        !self.cleanup.actions(exit).is_empty()
    }

    fn scheduled_child(&self, site: RuntimeLineHandleSiteId) -> Option<RuntimeLineTaskNodeId> {
        self.handle_site(site)?.scheduled_child()
    }

    fn has_cancellation_work(&self, mark: RuntimeDialogueMarkId) -> bool {
        self.cancel_rules
            .iter()
            .find(|rule| rule.trigger == mark)
            .is_some_and(|rule| !rule.action.is_empty())
    }
}

/// Dense, preorder-indexed node of a sealed line task graph.
#[derive(Clone, Debug, PartialEq)]
pub enum LineTaskNode {
    Sequence(Box<[RuntimeLineTaskNodeId]>),
    Start(Box<[RuntimeLineTaskNodeId]>),
    Parallel {
        policy: ParallelPolicy,
        children: Box<[RuntimeLineTaskNodeId]>,
    },
    Child {
        trigger: LineTaskTrigger,
        join_policy: ChildJoinPolicy,
        cancel_policy: ChildCancelPolicy,
        scope: RuntimeLineTaskNodeId,
    },
    Action(Box<[FlowOp]>),
}

/// Parallel group execution policy.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum ParallelPolicy {
    #[default]
    JoinAll,
}

/// Condition that starts a line-scoped child task.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum LineTaskTrigger {
    #[default]
    Immediate,
    Mark(RuntimeDialogueMarkId),
    ContentEffect(RuntimeDialogueEffectSiteId),
    Scheduled(RuntimeLineHandleSiteId),
}

/// Whether the containing graph waits for a child scope to finish.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChildJoinPolicy {
    #[default]
    Join,
    Detached,
}

/// How a running child fiber is handled when its dialogue line is cancelled.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChildCancelPolicy {
    #[default]
    CancelAndJoin,
    Finish,
    Detach,
}

/// Nearest lexical child exit policy. Nested child scopes replace this policy;
/// it is carried to the executor with every action command.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct LineTaskExitPolicy {
    pub join: ChildJoinPolicy,
    pub cancel: ChildCancelPolicy,
}

impl LineTaskExitPolicy {
    const fn new(join: ChildJoinPolicy, cancel: ChildCancelPolicy) -> Self {
        Self { join, cancel }
    }
}

/// One mark-selected cancellation branch.
#[derive(Clone, Debug, PartialEq)]
pub struct LineCancelRule {
    trigger: RuntimeDialogueMarkId,
    action: Box<[FlowOp]>,
}

impl LineCancelRule {
    pub(crate) const fn new(trigger: RuntimeDialogueMarkId, action: Box<[FlowOp]>) -> Self {
        Self { trigger, action }
    }

    #[must_use]
    pub const fn trigger(&self) -> RuntimeDialogueMarkId {
        self.trigger
    }

    #[must_use]
    pub const fn action(&self) -> &[FlowOp] {
        &self.action
    }
}

/// Typed value exported by a flow choice or line action.
///
/// This remains an effect payload because its consumer is the host/output
/// boundary; it is not part of the line-task graph topology.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineOutRequest {
    pub label: Option<String>,
    pub value: String,
}

/// Typed cleanup actions and host cleanup policy for a line task group.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineCleanupPolicy {
    pub child_tasks: ChildTaskCleanup,
    pub presentation: PresentationCleanup,
    pub audio: AudioCleanup,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineTaskCleanup {
    completed: Box<[FlowOp]>,
    cancelled: Box<[FlowOp]>,
    failed: Box<[FlowOp]>,
    policy: LineCleanupPolicy,
}

impl LineTaskCleanup {
    pub(crate) const fn new(
        completed: Box<[FlowOp]>,
        cancelled: Box<[FlowOp]>,
        failed: Box<[FlowOp]>,
        policy: LineCleanupPolicy,
    ) -> Self {
        Self {
            completed,
            cancelled,
            failed,
            policy,
        }
    }

    #[must_use]
    pub const fn actions(&self, exit: ScopeExit) -> &[FlowOp] {
        match exit {
            ScopeExit::Completed => &self.completed,
            ScopeExit::Cancelled => &self.cancelled,
            ScopeExit::Failed => &self.failed,
        }
    }

    #[must_use]
    pub const fn policy(&self) -> &LineCleanupPolicy {
        &self.policy
    }
}

/// How line-scoped child tasks are treated on cleanup.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum ChildTaskCleanup {
    #[default]
    CancelAndJoin,
    Detach,
    Finish,
}

/// How presentation handles registered in the line lifetime are cleaned up.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum PresentationCleanup {
    #[default]
    DropRegistered,
    KeepRegistered,
}

/// How line-scoped audio handles are cleaned up.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum AudioCleanup {
    #[default]
    StopRegistered,
    FadeRegistered,
    KeepRegistered,
}

/// Precise owner tag for an action fiber.  This is deliberately not merely a
/// node id: cleanup and cancellation branches are distinct work of the same
/// activation and must not be mistaken for a graph node completing.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum LineTaskWorkInstance {
    Activation(DialogueActivationId),
    Scheduled(RuntimeLineHandleToken),
}

impl LineTaskWorkInstance {
    #[must_use]
    pub const fn activation(&self) -> &DialogueActivationId {
        match self {
            Self::Activation(activation) => activation,
            Self::Scheduled(token) => token.activation(),
        }
    }

    #[must_use]
    pub const fn scheduled_token(&self) -> Option<&RuntimeLineHandleToken> {
        match self {
            Self::Activation(_) => None,
            Self::Scheduled(token) => Some(token),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct LineTaskWorkTag {
    instance: LineTaskWorkInstance,
    work: LineTaskWork,
}

impl LineTaskWorkTag {
    #[must_use]
    pub(crate) const fn activation(activation: DialogueActivationId, work: LineTaskWork) -> Self {
        Self {
            instance: LineTaskWorkInstance::Activation(activation),
            work,
        }
    }

    #[must_use]
    pub(crate) const fn scheduled(
        token: RuntimeLineHandleToken,
        node: RuntimeLineTaskNodeId,
    ) -> Self {
        Self {
            instance: LineTaskWorkInstance::Scheduled(token),
            work: LineTaskWork::Node(node),
        }
    }

    #[must_use]
    pub const fn instance(&self) -> &LineTaskWorkInstance {
        &self.instance
    }

    #[must_use]
    pub const fn activation_id(&self) -> &DialogueActivationId {
        self.instance.activation()
    }

    #[must_use]
    pub const fn scheduled_token(&self) -> Option<&RuntimeLineHandleToken> {
        self.instance.scheduled_token()
    }

    #[must_use]
    pub const fn work(&self) -> LineTaskWork {
        self.work
    }

    #[must_use]
    pub(crate) const fn is_well_formed(&self) -> bool {
        matches!(
            (&self.instance, self.work),
            (LineTaskWorkInstance::Activation(_), _)
                | (LineTaskWorkInstance::Scheduled(_), LineTaskWork::Node(_))
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum LineTaskWork {
    Node(RuntimeLineTaskNodeId),
    Cancellation(RuntimeDialogueMarkId),
    Cleanup(ScopeExit),
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LineTaskCompletionError {
    #[error("line-task completion belongs to a different dialogue activation")]
    StaleActivation {
        expected: DialogueActivationId,
        actual: DialogueActivationId,
    },
    #[error("line-task completion does not name one outstanding work item")]
    UnknownOrDuplicateWork { tag: LineTaskWorkTag },
    #[error("line-task completion names an invalid scheduled runtime instance")]
    InvalidScheduledInstance { token: RuntimeLineHandleToken },
}

/// Actions made runnable by one executor-neutral reducer transition.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct LineTaskActivation {
    pub commands: Vec<LineTaskCommand>,
    pub scheduled_completions: Vec<LineTaskScheduledCompletion>,
}

impl LineTaskActivation {
    pub(crate) fn append(&mut self, mut other: Self) {
        self.commands.append(&mut other.commands);
        self.scheduled_completions
            .append(&mut other.scheduled_completions);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LineTaskScheduledCompletion {
    token: RuntimeLineHandleToken,
    exit: ScopeExit,
}

impl LineTaskScheduledCompletion {
    pub(crate) const fn new(token: RuntimeLineHandleToken, exit: ScopeExit) -> Self {
        Self { token, exit }
    }

    pub(crate) const fn token(&self) -> &RuntimeLineHandleToken {
        &self.token
    }

    pub(crate) const fn exit(&self) -> ScopeExit {
        self.exit
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LineTaskCommand {
    Run {
        tag: LineTaskWorkTag,
        policy: LineTaskExitPolicy,
    },
    Cancel {
        tag: LineTaskWorkTag,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LineTaskActiveRoot {
    node: RuntimeLineTaskNodeId,
}

impl LineTaskActiveRoot {
    pub(crate) const fn new(node: RuntimeLineTaskNodeId) -> Self {
        Self { node }
    }

    pub(crate) const fn node(self) -> RuntimeLineTaskNodeId {
        self.node
    }
}

/// Snapshot-persisted state for each dense node in one active dialogue line.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LineTaskNodeState {
    #[default]
    Armed,
    Running,
    Cancelling,
    Detached,
    Completed,
    Cancelled,
    Failed,
}

impl LineTaskNodeState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Detached | Self::Completed | Self::Cancelled | Self::Failed
        )
    }
}

/// Lifecycle of an active group.  Closing is an observable protocol state,
/// not a queue deletion: executor-owned work is allowed to unwind and joined
/// work is accounted for before the parent dialogue may resume.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum LineTaskPhase {
    #[default]
    Active,
    Closing {
        exit: ScopeExit,
    },
    Closed {
        exit: ScopeExit,
    },
}

/// Canonical persistence carrier for one reducer activation. Executors map it
/// to their own DTO/wire schema; no executor may reconstruct live reducer
/// state from selected fields.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LineTaskLiveSnapshot {
    activation: DialogueActivationId,
    phase: LineTaskPhase,
    activation_lane: LineTaskExecutionLaneSnapshot,
    scheduled_lanes: Box<[LineTaskScheduledLaneSnapshot]>,
    scheduled_ready: Box<[RuntimeLineHandleToken]>,
    consumed_content_events: Box<[RuntimeDialogueContentEventKind]>,
    cleanup_started: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LineTaskExecutionLaneSnapshot {
    node_states: Box<[LineTaskNodeState]>,
    outstanding: Box<[LineTaskWork]>,
    active_roots: Box<[LineTaskActiveRoot]>,
    cancelling_nodes: Box<[RuntimeLineTaskNodeId]>,
}

impl LineTaskExecutionLaneSnapshot {
    pub(crate) fn new(
        node_states: Box<[LineTaskNodeState]>,
        outstanding: Box<[LineTaskWork]>,
        active_roots: Box<[LineTaskActiveRoot]>,
        cancelling_nodes: Box<[RuntimeLineTaskNodeId]>,
    ) -> Self {
        Self {
            node_states,
            outstanding,
            active_roots,
            cancelling_nodes,
        }
    }

    pub(crate) const fn node_states(&self) -> &[LineTaskNodeState] {
        &self.node_states
    }

    pub(crate) const fn outstanding(&self) -> &[LineTaskWork] {
        &self.outstanding
    }

    pub(crate) const fn active_roots(&self) -> &[LineTaskActiveRoot] {
        &self.active_roots
    }

    pub(crate) const fn cancelling_nodes(&self) -> &[RuntimeLineTaskNodeId] {
        &self.cancelling_nodes
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LineTaskScheduledLaneSnapshot {
    token: RuntimeLineHandleToken,
    lane: LineTaskExecutionLaneSnapshot,
}

impl LineTaskScheduledLaneSnapshot {
    pub(crate) const fn new(
        token: RuntimeLineHandleToken,
        lane: LineTaskExecutionLaneSnapshot,
    ) -> Self {
        Self { token, lane }
    }

    pub(crate) const fn token(&self) -> &RuntimeLineHandleToken {
        &self.token
    }

    pub(crate) const fn lane(&self) -> &LineTaskExecutionLaneSnapshot {
        &self.lane
    }
}

impl LineTaskLiveSnapshot {
    #[must_use]
    pub(crate) fn new(
        activation: DialogueActivationId,
        phase: LineTaskPhase,
        activation_lane: LineTaskExecutionLaneSnapshot,
        scheduled_lanes: Box<[LineTaskScheduledLaneSnapshot]>,
        scheduled_ready: Box<[RuntimeLineHandleToken]>,
        consumed_content_events: Box<[RuntimeDialogueContentEventKind]>,
        cleanup_started: bool,
    ) -> Self {
        Self {
            activation,
            phase,
            activation_lane,
            scheduled_lanes,
            scheduled_ready,
            consumed_content_events,
            cleanup_started,
        }
    }

    #[must_use]
    pub(crate) const fn activation(&self) -> &DialogueActivationId {
        &self.activation
    }

    #[must_use]
    pub(crate) const fn phase(&self) -> LineTaskPhase {
        self.phase
    }

    #[must_use]
    pub(crate) const fn node_states(&self) -> &[LineTaskNodeState] {
        self.activation_lane.node_states()
    }

    #[must_use]
    pub(crate) const fn outstanding(&self) -> &[LineTaskWork] {
        self.activation_lane.outstanding()
    }

    #[must_use]
    pub(crate) const fn active_roots(&self) -> &[LineTaskActiveRoot] {
        self.activation_lane.active_roots()
    }

    #[must_use]
    pub(crate) const fn cancelling_nodes(&self) -> &[RuntimeLineTaskNodeId] {
        self.activation_lane.cancelling_nodes()
    }

    #[must_use]
    pub(crate) const fn scheduled_lanes(&self) -> &[LineTaskScheduledLaneSnapshot] {
        &self.scheduled_lanes
    }

    #[must_use]
    pub(crate) const fn scheduled_ready(&self) -> &[RuntimeLineHandleToken] {
        &self.scheduled_ready
    }

    #[must_use]
    pub(crate) const fn consumed_content_events(&self) -> &[RuntimeDialogueContentEventKind] {
        &self.consumed_content_events
    }

    #[must_use]
    pub(crate) const fn cleanup_started(&self) -> bool {
        self.cleanup_started
    }

    /// Returns the exact runtime work identities that must each have one
    /// joined executor child. Static node coordinates alone are insufficient
    /// because scheduled lanes distinguish concurrent issuances by token.
    pub(crate) fn outstanding_tags(&self) -> BTreeSet<LineTaskWorkTag> {
        self.activation_lane
            .outstanding
            .iter()
            .copied()
            .map(|work| LineTaskWorkTag::activation(self.activation.clone(), work))
            .chain(self.scheduled_lanes.iter().flat_map(|scheduled| {
                scheduled
                    .lane()
                    .outstanding()
                    .iter()
                    .filter_map(|work| match work {
                        LineTaskWork::Node(node) => {
                            Some(LineTaskWorkTag::scheduled(scheduled.token().clone(), *node))
                        }
                        LineTaskWork::Cancellation(_) | LineTaskWork::Cleanup(_) => None,
                    })
            }))
            .collect()
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(crate) enum LineTaskSnapshotError {
    #[error("line-task snapshot has {actual} node states; plan has {expected} nodes")]
    NodeStateCount { expected: usize, actual: usize },
    #[error("line-task snapshot references unknown node {node}")]
    UnknownNode { node: RuntimeLineTaskNodeId },
    #[error("line-task snapshot repeats active root {node}")]
    DuplicateActiveRoot { node: RuntimeLineTaskNodeId },
    #[error("line-task snapshot repeats cancelling node {node}")]
    DuplicateCancellingNode { node: RuntimeLineTaskNodeId },
    #[error("line-task snapshot repeats outstanding work {work:?}")]
    DuplicateOutstanding { work: LineTaskWork },
    #[error("line-task snapshot repeats scheduled runtime instance {token:?}")]
    DuplicateScheduledInstance { token: RuntimeLineHandleToken },
    #[error("line-task snapshot references an unknown scheduled runtime instance {token:?}")]
    UnknownScheduledInstance { token: RuntimeLineHandleToken },
    #[error("line-task snapshot scheduled lane does not match its token {token:?}")]
    InvalidScheduledLane { token: RuntimeLineHandleToken },
    #[error("line-task snapshot repeats consumed dialogue content event {event:?}")]
    DuplicateContentEvent {
        event: RuntimeDialogueContentEventKind,
    },
    #[error("line-task snapshot marks terminal node {node} as an active root")]
    TerminalActiveRoot { node: RuntimeLineTaskNodeId },
    #[error("line-task snapshot marks node {node} cancelling without Cancelling state")]
    CancellingNodeState { node: RuntimeLineTaskNodeId },
    #[error("line-task snapshot work {work:?} is incompatible with its phase")]
    WorkPhase { work: LineTaskWork },
    #[error("line-task snapshot cleanup flag is incompatible with its phase")]
    CleanupPhase,
}

#[derive(Clone, Debug, PartialEq)]
struct LineTaskExecutionLane {
    node_states: Box<[LineTaskNodeState]>,
    outstanding: BTreeSet<LineTaskWork>,
    active_roots: BTreeSet<RuntimeLineTaskNodeId>,
    cancelling_nodes: BTreeSet<RuntimeLineTaskNodeId>,
}

impl LineTaskExecutionLane {
    fn new(node_count: usize) -> Self {
        Self {
            node_states: vec![LineTaskNodeState::Armed; node_count].into_boxed_slice(),
            outstanding: BTreeSet::new(),
            active_roots: BTreeSet::new(),
            cancelling_nodes: BTreeSet::new(),
        }
    }

    fn snapshot(&self) -> LineTaskExecutionLaneSnapshot {
        LineTaskExecutionLaneSnapshot::new(
            self.node_states.clone(),
            self.outstanding.iter().copied().collect(),
            self.active_roots
                .iter()
                .copied()
                .map(LineTaskActiveRoot::new)
                .collect(),
            self.cancelling_nodes.iter().copied().collect(),
        )
    }

    fn from_snapshot(snapshot: LineTaskExecutionLaneSnapshot) -> Self {
        Self {
            node_states: snapshot.node_states,
            outstanding: snapshot.outstanding.into_vec().into_iter().collect(),
            active_roots: snapshot
                .active_roots
                .into_vec()
                .into_iter()
                .map(LineTaskActiveRoot::node)
                .collect(),
            cancelling_nodes: snapshot.cancelling_nodes.into_vec().into_iter().collect(),
        }
    }

    fn node_state(&self, node: RuntimeLineTaskNodeId) -> Option<LineTaskNodeState> {
        self.node_states.get(node.index()).copied()
    }

    fn set_node_state(&mut self, node: RuntimeLineTaskNodeId, state: LineTaskNodeState) {
        if let Some(slot) = self.node_states.get_mut(node.index()) {
            *slot = state;
        }
    }

    fn cancel_pending_nodes(&mut self) {
        for (index, state) in self.node_states.iter_mut().enumerate() {
            if !state.is_terminal() {
                *state = LineTaskNodeState::Cancelling;
                if let Some(node) = RuntimeLineTaskNodeId::from_zero_based(index) {
                    self.cancelling_nodes.insert(node);
                }
            }
        }
    }
}

/// Native dialogue state for one group. Capture values themselves are held by
/// `DialogueActivationFrame`; this owner records only deterministic graph progress.
#[derive(Clone, Debug, PartialEq)]
pub struct LineTaskLiveState {
    activation: DialogueActivationId,
    phase: LineTaskPhase,
    activation_lane: LineTaskExecutionLane,
    scheduled_lanes: BTreeMap<RuntimeLineHandleToken, LineTaskExecutionLane>,
    scheduled_ready: BTreeSet<RuntimeLineHandleToken>,
    consumed_content_events: BTreeSet<RuntimeDialogueContentEventKind>,
    cleanup_started: bool,
}

impl LineTaskLiveState {
    #[must_use]
    pub(crate) fn new<P: LineTaskPlanView>(group: &P, activation: DialogueActivationId) -> Self {
        Self {
            activation,
            phase: LineTaskPhase::Active,
            activation_lane: LineTaskExecutionLane::new(group.node_count()),
            scheduled_lanes: BTreeMap::new(),
            scheduled_ready: BTreeSet::new(),
            consumed_content_events: BTreeSet::new(),
            cleanup_started: false,
        }
    }

    /// Returns the sole complete persistence representation of this reducer.
    #[must_use]
    pub(crate) fn snapshot(&self) -> LineTaskLiveSnapshot {
        LineTaskLiveSnapshot::new(
            self.activation.clone(),
            self.phase,
            self.activation_lane.snapshot(),
            self.scheduled_lanes
                .iter()
                .map(|(token, lane)| {
                    LineTaskScheduledLaneSnapshot::new(token.clone(), lane.snapshot())
                })
                .collect(),
            self.scheduled_ready.iter().cloned().collect(),
            self.consumed_content_events.iter().copied().collect(),
            self.cleanup_started,
        )
    }

    /// Restores only an owner-validated complete snapshot. `P` is the same
    /// plan view used by the reducer, so native and AWBC share validation.
    pub(crate) fn restore<P: LineTaskPlanView>(
        plan: &P,
        snapshot: LineTaskLiveSnapshot,
    ) -> Result<Self, LineTaskSnapshotError> {
        validate_snapshot(plan, &snapshot)?;
        Ok(Self {
            activation: snapshot.activation,
            phase: snapshot.phase,
            activation_lane: LineTaskExecutionLane::from_snapshot(snapshot.activation_lane),
            scheduled_lanes: snapshot
                .scheduled_lanes
                .into_vec()
                .into_iter()
                .map(|scheduled| {
                    (
                        scheduled.token,
                        LineTaskExecutionLane::from_snapshot(scheduled.lane),
                    )
                })
                .collect(),
            scheduled_ready: snapshot.scheduled_ready.into_vec().into_iter().collect(),
            consumed_content_events: snapshot
                .consumed_content_events
                .into_vec()
                .into_iter()
                .collect(),
            cleanup_started: snapshot.cleanup_started,
        })
    }

    #[must_use]
    pub(crate) fn is_closing(&self) -> bool {
        matches!(self.phase, LineTaskPhase::Closing { .. })
    }

    #[must_use]
    pub(crate) fn is_closed(&self) -> bool {
        matches!(self.phase, LineTaskPhase::Closed { .. })
    }

    #[must_use]
    pub fn node_state(&self, node: RuntimeLineTaskNodeId) -> Option<LineTaskNodeState> {
        self.activation_lane.node_state(node)
    }

    pub(crate) fn mark_scheduled_ready(
        &mut self,
        token: RuntimeLineHandleToken,
    ) -> Result<bool, LineRuntimeError> {
        if token.activation() != &self.activation {
            return Err(LineRuntimeError::WrongActivation);
        }
        if self.scheduled_lanes.contains_key(&token) || !self.scheduled_ready.insert(token) {
            return Err(LineRuntimeError::DuplicateScheduledWorkInstance);
        }
        Ok(true)
    }

    /// Validates and consumes one step's exact content-reveal events as one
    /// transaction. The content-plan owner supplies `contains`; no event is
    /// recorded if any activation, coordinate, duplicate, or replay check
    /// fails.
    pub(crate) fn accept_content_event_kinds(
        &mut self,
        events: &[RuntimeDialogueContentEventKind],
        mut contains: impl FnMut(RuntimeDialogueContentEventKind) -> bool,
    ) -> Result<AcceptedLineTaskContentEvents, LineRuntimeError> {
        let mut batch = BTreeSet::new();
        for &kind in events {
            if !contains(kind) {
                return Err(LineRuntimeError::UnknownContentEvent { event: kind });
            }
            if !batch.insert(kind) {
                return Err(LineRuntimeError::DuplicateContentEvent { event: kind });
            }
            if self.consumed_content_events.contains(&kind) {
                return Err(LineRuntimeError::ConsumedContentEvent { event: kind });
            }
        }
        self.consumed_content_events.extend(batch.iter().copied());
        let mut accepted = AcceptedLineTaskContentEvents::default();
        for event in batch {
            match event {
                RuntimeDialogueContentEventKind::Mark(mark) => {
                    accepted.marks.insert(mark);
                }
                RuntimeDialogueContentEventKind::Effect(effect) => {
                    accepted.effects.insert(effect);
                }
            }
        }
        Ok(accepted)
    }

    fn complete_work(
        &mut self,
        tag: LineTaskWorkTag,
        failed: bool,
    ) -> Result<(), LineTaskCompletionError> {
        if tag.activation_id() != &self.activation {
            return Err(LineTaskCompletionError::StaleActivation {
                expected: self.activation.clone(),
                actual: tag.activation_id().clone(),
            });
        }
        let lane = match tag.instance() {
            LineTaskWorkInstance::Activation(_) => &mut self.activation_lane,
            LineTaskWorkInstance::Scheduled(token) => {
                self.scheduled_lanes.get_mut(token).ok_or_else(|| {
                    LineTaskCompletionError::UnknownOrDuplicateWork { tag: tag.clone() }
                })?
            }
        };
        let work = tag.work();
        if !lane.outstanding.remove(&work) {
            return Err(LineTaskCompletionError::UnknownOrDuplicateWork { tag });
        }
        if let LineTaskWork::Node(node) = work {
            lane.set_node_state(
                node,
                if matches!(self.phase, LineTaskPhase::Closing { .. }) {
                    LineTaskNodeState::Cancelled
                } else if failed {
                    LineTaskNodeState::Failed
                } else {
                    LineTaskNodeState::Completed
                },
            );
        }
        Ok(())
    }

    fn begin_close(&mut self, exit: ScopeExit) -> bool {
        if !matches!(self.phase, LineTaskPhase::Active) {
            return false;
        }
        self.phase = LineTaskPhase::Closing { exit };
        self.activation_lane.cancel_pending_nodes();
        for lane in self.scheduled_lanes.values_mut() {
            lane.cancel_pending_nodes();
        }
        true
    }

    fn has_outstanding(&self) -> bool {
        !self.activation_lane.outstanding.is_empty()
            || self
                .scheduled_lanes
                .values()
                .any(|lane| !lane.outstanding.is_empty())
    }
}

fn validate_snapshot<P: LineTaskPlanView>(
    plan: &P,
    snapshot: &LineTaskLiveSnapshot,
) -> Result<(), LineTaskSnapshotError> {
    validate_lane_snapshot(plan, &snapshot.activation_lane, snapshot.phase, false)?;
    let mut scheduled_instances = BTreeSet::new();
    for scheduled in &snapshot.scheduled_lanes {
        if scheduled.token.activation() != &snapshot.activation
            || plan.scheduled_child(scheduled.token.site()).is_none()
        {
            return Err(LineTaskSnapshotError::UnknownScheduledInstance {
                token: scheduled.token.clone(),
            });
        }
        if !scheduled_instances.insert(scheduled.token.clone()) {
            return Err(LineTaskSnapshotError::DuplicateScheduledInstance {
                token: scheduled.token.clone(),
            });
        }
        validate_lane_snapshot(plan, &scheduled.lane, snapshot.phase, true).map_err(|_| {
            LineTaskSnapshotError::InvalidScheduledLane {
                token: scheduled.token.clone(),
            }
        })?;
        validate_scheduled_lane_scope(plan, scheduled).map_err(|_| {
            LineTaskSnapshotError::InvalidScheduledLane {
                token: scheduled.token.clone(),
            }
        })?;
    }
    for token in &snapshot.scheduled_ready {
        if token.activation() != &snapshot.activation
            || plan.scheduled_child(token.site()).is_none()
        {
            return Err(LineTaskSnapshotError::UnknownScheduledInstance {
                token: token.clone(),
            });
        }
        if !scheduled_instances.insert(token.clone()) {
            return Err(LineTaskSnapshotError::DuplicateScheduledInstance {
                token: token.clone(),
            });
        }
    }
    let mut content_events = BTreeSet::new();
    for &event in &snapshot.consumed_content_events {
        if !content_events.insert(event) {
            return Err(LineTaskSnapshotError::DuplicateContentEvent { event });
        }
    }
    if snapshot.cleanup_started && matches!(snapshot.phase, LineTaskPhase::Active) {
        return Err(LineTaskSnapshotError::CleanupPhase);
    }
    Ok(())
}

fn validate_lane_snapshot<P: LineTaskPlanView>(
    plan: &P,
    lane: &LineTaskExecutionLaneSnapshot,
    phase: LineTaskPhase,
    scheduled: bool,
) -> Result<(), LineTaskSnapshotError> {
    if lane.node_states.len() != plan.node_count() {
        return Err(LineTaskSnapshotError::NodeStateCount {
            expected: plan.node_count(),
            actual: lane.node_states.len(),
        });
    }
    let validate_node = |node| {
        plan.node_view(node)
            .is_some()
            .then_some(())
            .ok_or(LineTaskSnapshotError::UnknownNode { node })
    };
    let mut roots = BTreeSet::new();
    for root in &lane.active_roots {
        let node = root.node;
        validate_node(node)?;
        if !roots.insert(node) {
            return Err(LineTaskSnapshotError::DuplicateActiveRoot { node });
        }
        if lane.node_states[node.index()].is_terminal() {
            return Err(LineTaskSnapshotError::TerminalActiveRoot { node });
        }
    }
    let mut cancelling = BTreeSet::new();
    for &node in &lane.cancelling_nodes {
        validate_node(node)?;
        if !cancelling.insert(node) {
            return Err(LineTaskSnapshotError::DuplicateCancellingNode { node });
        }
        if lane.node_states[node.index()] != LineTaskNodeState::Cancelling {
            return Err(LineTaskSnapshotError::CancellingNodeState { node });
        }
    }
    let mut outstanding = BTreeSet::new();
    for &work in &lane.outstanding {
        if !outstanding.insert(work) {
            return Err(LineTaskSnapshotError::DuplicateOutstanding { work });
        }
        match work {
            LineTaskWork::Node(node) => validate_node(node)?,
            LineTaskWork::Cancellation(_) | LineTaskWork::Cleanup(_) => {}
        }
        let valid_phase = match work {
            LineTaskWork::Node(_) => !matches!(phase, LineTaskPhase::Closed { .. }),
            LineTaskWork::Cancellation(_) | LineTaskWork::Cleanup(_) => {
                !scheduled && matches!(phase, LineTaskPhase::Closing { .. })
            }
        };
        if !valid_phase {
            return Err(LineTaskSnapshotError::WorkPhase { work });
        }
    }
    Ok(())
}

fn validate_scheduled_lane_scope<P: LineTaskPlanView>(
    plan: &P,
    scheduled: &LineTaskScheduledLaneSnapshot,
) -> Result<(), ()> {
    let child = plan.scheduled_child(scheduled.token.site()).ok_or(())?;
    let Some(LineTaskNodeView::Child {
        trigger: LineTaskTrigger::Scheduled(site),
        policy,
        scope,
    }) = plan.node_view(child)
    else {
        return Err(());
    };
    if site != scheduled.token.site()
        || policy.join != ChildJoinPolicy::Join
        || policy.cancel != ChildCancelPolicy::CancelAndJoin
        || !matches!(plan.node_view(scope), Some(LineTaskNodeView::Action))
    {
        return Err(());
    }
    let mut allowed = BTreeSet::new();
    collect_line_task_subtree(plan, child, &mut allowed)?;
    for (index, state) in scheduled.lane.node_states.iter().copied().enumerate() {
        if state != LineTaskNodeState::Armed {
            let node = RuntimeLineTaskNodeId::from_zero_based(index).ok_or(())?;
            if !allowed.contains(&node) {
                return Err(());
            }
        }
    }
    if scheduled.lane.outstanding.is_empty()
        || scheduled
            .lane
            .outstanding
            .iter()
            .any(|work| !matches!(work, LineTaskWork::Node(node) if allowed.contains(node)))
        || scheduled
            .lane
            .active_roots
            .iter()
            .any(|root| !allowed.contains(&root.node()))
        || scheduled
            .lane
            .cancelling_nodes
            .iter()
            .any(|node| !allowed.contains(node))
        || !matches!(
            scheduled.lane.node_states[child.index()],
            LineTaskNodeState::Running | LineTaskNodeState::Cancelling
        )
    {
        return Err(());
    }
    Ok(())
}

fn collect_line_task_subtree<P: LineTaskPlanView>(
    plan: &P,
    node: RuntimeLineTaskNodeId,
    nodes: &mut BTreeSet<RuntimeLineTaskNodeId>,
) -> Result<(), ()> {
    if !nodes.insert(node) {
        return Ok(());
    }
    match plan.node_view(node).ok_or(())? {
        LineTaskNodeView::Sequence(children)
        | LineTaskNodeView::Start(children)
        | LineTaskNodeView::Parallel(children) => {
            for child in children {
                collect_line_task_subtree(plan, *child, nodes)?;
            }
        }
        LineTaskNodeView::Child { scope, .. } => {
            collect_line_task_subtree(plan, scope, nodes)?;
        }
        LineTaskNodeView::Action => {}
    }
    Ok(())
}

fn drain_cancellations(state: &mut LineTaskLiveState, activation: &mut LineTaskActivation) {
    activation.commands.extend(
        std::mem::take(&mut state.activation_lane.cancelling_nodes)
            .into_iter()
            .map(|node| LineTaskCommand::Cancel {
                tag: LineTaskWorkTag::activation(
                    state.activation.clone(),
                    LineTaskWork::Node(node),
                ),
            }),
    );
    for (token, lane) in &mut state.scheduled_lanes {
        activation
            .commands
            .extend(
                std::mem::take(&mut lane.cancelling_nodes)
                    .into_iter()
                    .map(|node| LineTaskCommand::Cancel {
                        tag: LineTaskWorkTag::scheduled(token.clone(), node),
                    }),
            );
    }
}

/// Starts or progresses a sealed line-task graph. Node identity, rather than
/// a traversal ordinal, makes repeated host steps idempotent.
pub(crate) fn progress_live_line_task_group<P: LineTaskPlanView>(
    group: &P,
    elapsed: LogicalDuration,
    events: LineTaskReadyEvents<'_>,
    state: &mut LineTaskLiveState,
) -> Result<LineTaskActivation, LineRuntimeError> {
    if !matches!(state.phase, LineTaskPhase::Active) {
        return Ok(LineTaskActivation::default());
    }
    let mut candidate = state.clone();
    let mut activation = LineTaskActivation::default();
    let activation_instance = LineTaskWorkInstance::Activation(candidate.activation.clone());
    activate_node(
        group,
        group.root_node(),
        elapsed,
        events,
        &mut candidate.activation_lane,
        &activation_instance,
        LineTaskExitPolicy::default(),
        &mut activation,
    );
    progress_active_roots(
        group,
        elapsed,
        events,
        &mut candidate.activation_lane,
        &activation_instance,
        &mut activation,
    );
    progress_ready_scheduled(group, elapsed, events, &mut candidate, &mut activation)?;
    drain_cancellations(&mut candidate, &mut activation);
    *state = candidate;
    Ok(activation)
}

/// Begins the cancellation closing protocol. The returned activation contains
/// only cancellation work; cleanup is emitted after joined work has drained.
/// This gives every executor the same ordering without exposing its queue.
pub(crate) fn cancel_live_line_task_group<P: LineTaskPlanView>(
    group: &P,
    marks: &BTreeSet<RuntimeDialogueMarkId>,
    state: &mut LineTaskLiveState,
) -> Option<LineTaskActivation> {
    let mark = group.cancellation_mark(marks)?;
    if !state.begin_close(ScopeExit::Cancelled) {
        return None;
    }
    let mut activation = LineTaskActivation::default();
    if group.has_cancellation_work(mark) {
        activation.commands.push(LineTaskCommand::Run {
            tag: LineTaskWorkTag::activation(
                state.activation.clone(),
                LineTaskWork::Cancellation(mark),
            ),
            policy: LineTaskExitPolicy::new(ChildJoinPolicy::Join, ChildCancelPolicy::Finish),
        });
        state
            .activation_lane
            .outstanding
            .insert(LineTaskWork::Cancellation(mark));
    }
    drain_cancellations(state, &mut activation);
    Some(activation)
}

/// Completes a live line scope after an explicit host advance.
pub(crate) fn finish_live_line_task_group<P: LineTaskPlanView>(
    group: &P,
    state: &mut LineTaskLiveState,
) -> LineTaskActivation {
    state.begin_close(ScopeExit::Completed);
    let mut activation = finalize_live_line_task_close(group, state);
    drain_cancellations(state, &mut activation);
    activation
}

/// Begins failure close for one accepted reducer graph. This is the sole
/// reducer-owned transition used by both structured and Product executors;
/// executor adapters only realize the returned typed child commands.
pub(crate) fn fail_live_line_task_group<P: LineTaskPlanView>(
    group: &P,
    state: &mut LineTaskLiveState,
) -> LineTaskActivation {
    state.begin_close(ScopeExit::Failed);
    let mut activation = finalize_live_line_task_close(group, state);
    drain_cancellations(state, &mut activation);
    activation
}

/// Completes one tagged work item, then emits cleanup exactly once when the
/// closing protocol has no joined graph work left.
pub(crate) fn complete_live_line_task_work<P: LineTaskPlanView>(
    group: &P,
    state: &mut LineTaskLiveState,
    tag: LineTaskWorkTag,
    failed: bool,
) -> Result<LineTaskActivation, LineTaskCompletionError> {
    let mut candidate = state.clone();
    let instance = tag.instance().clone();
    candidate.complete_work(tag, failed)?;
    let mut activation = LineTaskActivation::default();
    if matches!(candidate.phase, LineTaskPhase::Active) {
        let empty_marks = BTreeSet::new();
        let empty_effects = BTreeSet::new();
        let ready = LineTaskReadyEvents::new(&empty_marks, &empty_effects);
        match &instance {
            LineTaskWorkInstance::Activation(_) => {
                activate_node(
                    group,
                    group.root_node(),
                    LogicalDuration::default(),
                    ready,
                    &mut candidate.activation_lane,
                    &instance,
                    LineTaskExitPolicy::default(),
                    &mut activation,
                );
                progress_active_roots(
                    group,
                    LogicalDuration::default(),
                    ready,
                    &mut candidate.activation_lane,
                    &instance,
                    &mut activation,
                );
            }
            LineTaskWorkInstance::Scheduled(token) => {
                progress_scheduled_lane(
                    group,
                    token,
                    LogicalDuration::default(),
                    ready,
                    &mut candidate,
                    &mut activation,
                )
                .map_err(|_| {
                    LineTaskCompletionError::InvalidScheduledInstance {
                        token: token.clone(),
                    }
                })?;
            }
        }
    }
    if failed && matches!(candidate.phase, LineTaskPhase::Active) {
        candidate.begin_close(ScopeExit::Failed);
    }
    activation
        .commands
        .extend(finalize_live_line_task_close(group, &mut candidate).commands);
    drain_cancellations(&mut candidate, &mut activation);
    *state = candidate;
    Ok(activation)
}

/// Advances Closing after executor-owned joined work reports drained.
pub(crate) fn finalize_live_line_task_close<P: LineTaskPlanView>(
    group: &P,
    state: &mut LineTaskLiveState,
) -> LineTaskActivation {
    let LineTaskPhase::Closing { exit } = state.phase else {
        return LineTaskActivation::default();
    };
    if state.has_outstanding() {
        return LineTaskActivation::default();
    }
    if state.cleanup_started {
        state.phase = LineTaskPhase::Closed { exit };
        return LineTaskActivation::default();
    }
    state.cleanup_started = true;
    if !group.has_cleanup(exit) {
        state.phase = LineTaskPhase::Closed { exit };
        return LineTaskActivation::default();
    }
    state
        .activation_lane
        .outstanding
        .insert(LineTaskWork::Cleanup(exit));
    LineTaskActivation {
        commands: vec![LineTaskCommand::Run {
            tag: LineTaskWorkTag::activation(state.activation.clone(), LineTaskWork::Cleanup(exit)),
            policy: LineTaskExitPolicy::new(ChildJoinPolicy::Join, ChildCancelPolicy::Finish),
        }],
        scheduled_completions: Vec::new(),
    }
}

fn activate_node<P: LineTaskPlanView>(
    group: &P,
    id: RuntimeLineTaskNodeId,
    elapsed: LogicalDuration,
    events: LineTaskReadyEvents<'_>,
    lane: &mut LineTaskExecutionLane,
    instance: &LineTaskWorkInstance,
    policy: LineTaskExitPolicy,
    activation: &mut LineTaskActivation,
) {
    let Some(node) = group.node_view(id) else {
        return;
    };
    match node {
        LineTaskNodeView::Sequence(nodes) => {
            if lane.node_state(id) == Some(LineTaskNodeState::Armed) {
                lane.set_node_state(id, LineTaskNodeState::Running);
            }
            if let Some(exit) = sequence_exit(nodes, lane) {
                lane.set_node_state(id, exit);
                cancel_remaining(nodes, lane);
                return;
            }
            if let Some(child) = nodes.iter().find(|child| {
                !lane
                    .node_state(**child)
                    .is_some_and(LineTaskNodeState::is_terminal)
            }) {
                activate_node(
                    group, *child, elapsed, events, lane, instance, policy, activation,
                );
            }
            if let Some(exit) = sequence_exit(nodes, lane) {
                lane.set_node_state(id, exit);
                cancel_remaining(nodes, lane);
            }
        }
        LineTaskNodeView::Start(nodes) => {
            if lane.node_state(id) == Some(LineTaskNodeState::Armed) {
                lane.set_node_state(id, LineTaskNodeState::Running);
                for child in nodes {
                    activate_node(
                        group, *child, elapsed, events, lane, instance, policy, activation,
                    );
                }
                lane.active_roots.extend(nodes.iter().copied());
                lane.set_node_state(id, LineTaskNodeState::Completed);
            }
        }
        LineTaskNodeView::Parallel(children) => {
            if lane.node_state(id) == Some(LineTaskNodeState::Armed) {
                lane.set_node_state(id, LineTaskNodeState::Running);
            }
            for child in children {
                activate_node(
                    group, *child, elapsed, events, lane, instance, policy, activation,
                );
            }
            if let Some(exit) = parallel_exit(children, lane) {
                lane.set_node_state(id, exit);
                if exit != LineTaskNodeState::Completed {
                    cancel_remaining(children, lane);
                }
            }
        }
        child @ LineTaskNodeView::Child { .. } => {
            activate_child(
                group, id, elapsed, events, lane, instance, activation, child,
            );
        }
        LineTaskNodeView::Action => {
            if lane.node_state(id) == Some(LineTaskNodeState::Armed) {
                lane.set_node_state(id, LineTaskNodeState::Running);
                if !group.has_action(id) {
                    lane.set_node_state(id, LineTaskNodeState::Completed);
                    return;
                }
                let work = LineTaskWork::Node(id);
                if policy.join == ChildJoinPolicy::Detached {
                    lane.set_node_state(id, LineTaskNodeState::Detached);
                } else {
                    lane.outstanding.insert(work);
                }
                activation.commands.push(LineTaskCommand::Run {
                    tag: LineTaskWorkTag {
                        instance: instance.clone(),
                        work,
                    },
                    policy,
                });
            }
        }
    }
}

fn activate_child<P: LineTaskPlanView>(
    group: &P,
    id: RuntimeLineTaskNodeId,
    elapsed: LogicalDuration,
    events: LineTaskReadyEvents<'_>,
    lane: &mut LineTaskExecutionLane,
    instance: &LineTaskWorkInstance,
    activation: &mut LineTaskActivation,
    node: LineTaskNodeView<'_>,
) {
    let LineTaskNodeView::Child {
        trigger,
        policy: child_policy,
        scope,
    } = node
    else {
        return;
    };
    if matches!(trigger, LineTaskTrigger::Scheduled(_)) {
        return;
    }
    if lane.node_state(id) == Some(LineTaskNodeState::Armed) && trigger_is_ready(&trigger, events) {
        lane.set_node_state(id, LineTaskNodeState::Running);
        activate_node(
            group,
            scope,
            elapsed,
            events,
            lane,
            instance,
            child_policy,
            activation,
        );
        if child_policy.join == ChildJoinPolicy::Detached {
            lane.active_roots.insert(scope);
            lane.set_node_state(id, LineTaskNodeState::Detached);
        }
    } else if lane.node_state(id) == Some(LineTaskNodeState::Running) {
        activate_node(
            group,
            scope,
            elapsed,
            events,
            lane,
            instance,
            child_policy,
            activation,
        );
        if let Some(exit) = lane.node_state(scope).filter(|state| state.is_terminal()) {
            lane.set_node_state(id, exit);
        }
    }
}

fn progress_ready_scheduled<P: LineTaskPlanView>(
    group: &P,
    elapsed: LogicalDuration,
    events: LineTaskReadyEvents<'_>,
    state: &mut LineTaskLiveState,
    activation: &mut LineTaskActivation,
) -> Result<(), LineRuntimeError> {
    let ready = std::mem::take(&mut state.scheduled_ready);
    for token in ready {
        if token.activation() != &state.activation || state.scheduled_lanes.contains_key(&token) {
            return Err(LineRuntimeError::DuplicateScheduledWorkInstance);
        }
        let (child, scope, policy) = scheduled_lane_schema(group, &token)?;
        let instance = LineTaskWorkInstance::Scheduled(token.clone());
        let mut lane = LineTaskExecutionLane::new(group.node_count());
        lane.set_node_state(child, LineTaskNodeState::Running);
        activate_node(
            group, scope, elapsed, events, &mut lane, &instance, policy, activation,
        );
        if let Some(exit) = lane.node_state(scope).filter(|state| state.is_terminal()) {
            lane.set_node_state(child, exit);
        }
        if lane.outstanding.is_empty() {
            activation
                .scheduled_completions
                .push(LineTaskScheduledCompletion::new(
                    token,
                    ScopeExit::Completed,
                ));
            continue;
        }
        state.scheduled_lanes.insert(token, lane);
    }
    Ok(())
}

fn progress_scheduled_lane<P: LineTaskPlanView>(
    group: &P,
    token: &RuntimeLineHandleToken,
    elapsed: LogicalDuration,
    events: LineTaskReadyEvents<'_>,
    state: &mut LineTaskLiveState,
    activation: &mut LineTaskActivation,
) -> Result<(), LineRuntimeError> {
    let (child, scope, policy) = scheduled_lane_schema(group, token)?;
    let mut lane = state
        .scheduled_lanes
        .remove(token)
        .ok_or(LineRuntimeError::MissingScheduledWork)?;
    let instance = LineTaskWorkInstance::Scheduled(token.clone());
    activate_node(
        group, scope, elapsed, events, &mut lane, &instance, policy, activation,
    );
    progress_active_roots(group, elapsed, events, &mut lane, &instance, activation);
    if let Some(exit) = lane.node_state(scope).filter(|state| state.is_terminal()) {
        lane.set_node_state(child, exit);
    }
    let terminal = lane
        .node_state(child)
        .is_some_and(LineTaskNodeState::is_terminal)
        && lane.outstanding.is_empty()
        && lane.active_roots.is_empty();
    if !terminal {
        state.scheduled_lanes.insert(token.clone(), lane);
    }
    Ok(())
}

fn scheduled_lane_schema<P: LineTaskPlanView>(
    group: &P,
    token: &RuntimeLineHandleToken,
) -> Result<
    (
        RuntimeLineTaskNodeId,
        RuntimeLineTaskNodeId,
        LineTaskExitPolicy,
    ),
    LineRuntimeError,
> {
    let child = group
        .scheduled_child(token.site())
        .ok_or(LineRuntimeError::InvalidScheduledWorkState)?;
    let Some(LineTaskNodeView::Child {
        trigger: LineTaskTrigger::Scheduled(site),
        policy,
        scope,
    }) = group.node_view(child)
    else {
        return Err(LineRuntimeError::InvalidScheduledWorkState);
    };
    if site != token.site()
        || policy.join != ChildJoinPolicy::Join
        || policy.cancel != ChildCancelPolicy::CancelAndJoin
        || !matches!(group.node_view(scope), Some(LineTaskNodeView::Action))
    {
        return Err(LineRuntimeError::InvalidScheduledWorkState);
    }
    Ok((child, scope, policy))
}

fn sequence_exit(
    children: &[RuntimeLineTaskNodeId],
    lane: &LineTaskExecutionLane,
) -> Option<LineTaskNodeState> {
    for child in children {
        match lane.node_state(*child)? {
            LineTaskNodeState::Failed => return Some(LineTaskNodeState::Failed),
            LineTaskNodeState::Cancelled | LineTaskNodeState::Cancelling => {
                return Some(LineTaskNodeState::Cancelled);
            }
            LineTaskNodeState::Armed | LineTaskNodeState::Running => return None,
            LineTaskNodeState::Detached | LineTaskNodeState::Completed => {}
        }
    }
    Some(LineTaskNodeState::Completed)
}

fn parallel_exit(
    children: &[RuntimeLineTaskNodeId],
    lane: &LineTaskExecutionLane,
) -> Option<LineTaskNodeState> {
    let states = children
        .iter()
        .map(|child| lane.node_state(*child))
        .collect::<Option<Vec<_>>>()?;
    if states.contains(&LineTaskNodeState::Failed) {
        Some(LineTaskNodeState::Failed)
    } else if states.contains(&LineTaskNodeState::Cancelled)
        || states.contains(&LineTaskNodeState::Cancelling)
    {
        Some(LineTaskNodeState::Cancelled)
    } else if states.iter().all(|state| state.is_terminal()) {
        Some(LineTaskNodeState::Completed)
    } else {
        None
    }
}

fn cancel_remaining(children: &[RuntimeLineTaskNodeId], lane: &mut LineTaskExecutionLane) {
    for child in children {
        if !lane
            .node_state(*child)
            .is_some_and(LineTaskNodeState::is_terminal)
        {
            lane.set_node_state(*child, LineTaskNodeState::Cancelling);
            lane.cancelling_nodes.insert(*child);
        }
    }
}

fn progress_active_roots<P: LineTaskPlanView>(
    group: &P,
    elapsed: LogicalDuration,
    events: LineTaskReadyEvents<'_>,
    lane: &mut LineTaskExecutionLane,
    instance: &LineTaskWorkInstance,
    activation: &mut LineTaskActivation,
) {
    let roots = lane.active_roots.iter().copied().collect::<Vec<_>>();
    for root in roots {
        if lane
            .node_state(root)
            .is_some_and(LineTaskNodeState::is_terminal)
        {
            lane.active_roots.remove(&root);
            continue;
        }
        activate_node(
            group,
            root,
            elapsed,
            events,
            lane,
            instance,
            LineTaskExitPolicy::default(),
            activation,
        );
        if lane
            .node_state(root)
            .is_some_and(LineTaskNodeState::is_terminal)
        {
            lane.active_roots.remove(&root);
        }
    }
}

fn trigger_is_ready(trigger: &LineTaskTrigger, events: LineTaskReadyEvents<'_>) -> bool {
    match trigger {
        LineTaskTrigger::Immediate => true,
        LineTaskTrigger::Mark(mark) => events.marks().contains(mark),
        LineTaskTrigger::ContentEffect(site) => events.content_effects().contains(site),
        LineTaskTrigger::Scheduled(_) => false,
    }
}
