use crate::plan::FlowOp;
use crate::runtime_id::{RuntimeDialogueMarkId, RuntimeLineTaskNodeId, RuntimeLocalDeclarationId};
use crate::task::{TaskId, TaskKey, TaskPriority};
use crate::time::LogicalDuration;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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
    root: RuntimeLineTaskNodeId,
    nodes: Box<[LineTaskNode]>,
    cancel_rules: Box<[LineCancelRule]>,
    cleanup: LineTaskCleanup,
}

impl LineTaskGroup {
    pub(crate) fn new(
        captures: Box<[RuntimeLocalDeclarationId]>,
        root: RuntimeLineTaskNodeId,
        nodes: Box<[LineTaskNode]>,
        cancel_rules: Box<[LineCancelRule]>,
        cleanup: LineTaskCleanup,
    ) -> Self {
        Self {
            captures,
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
    pub(crate) fn command_ops(&self, tag: LineTaskWorkTag) -> &[FlowOp] {
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
        id: TaskId,
        key: Option<TaskKey>,
        name: Option<String>,
        trigger: LineTaskTrigger,
        priority: TaskPriority,
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
    Delay(LogicalDuration),
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

/// Identity of one concrete activation of a dialogue-owned task graph.
///
/// Node ids are plan identities and can recur every time a dialogue is
/// revisited. Runtime work therefore carries this activation as well as its
/// node identity; a completion from an old line can never advance a new one.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct LineTaskActivationId(u64);

impl LineTaskActivationId {
    #[must_use]
    pub(crate) const fn value(self) -> u64 {
        self.0
    }

    #[must_use]
    pub(crate) const fn from_value(value: u64) -> Self {
        Self(value)
    }
}

/// Precise owner tag for an action fiber.  This is deliberately not merely a
/// node id: cleanup and cancellation branches are distinct work of the same
/// activation and must not be mistaken for a graph node completing.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct LineTaskWorkTag {
    pub activation: LineTaskActivationId,
    pub work: LineTaskWork,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum LineTaskWork {
    Node(RuntimeLineTaskNodeId),
    Cancellation(RuntimeDialogueMarkId),
    Cleanup(ScopeExit),
}

/// Actions made runnable by one executor-neutral reducer transition.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct LineTaskActivation {
    pub commands: Vec<LineTaskCommand>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LineTaskCommand {
    Run {
        tag: LineTaskWorkTag,
        policy: LineTaskExitPolicy,
    },
    Cancel {
        activation: LineTaskActivationId,
        node: RuntimeLineTaskNodeId,
    },
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
    activation: u64,
    phase: LineTaskPhase,
    node_states: Box<[LineTaskNodeState]>,
    outstanding: Box<[LineTaskWork]>,
    active_roots: Box<[RuntimeLineTaskNodeId]>,
    cancelling_nodes: Box<[RuntimeLineTaskNodeId]>,
    cleanup_started: bool,
}

impl LineTaskLiveSnapshot {
    #[must_use]
    pub(crate) fn new(
        activation: u64,
        phase: LineTaskPhase,
        node_states: Box<[LineTaskNodeState]>,
        outstanding: Box<[LineTaskWork]>,
        active_roots: Box<[RuntimeLineTaskNodeId]>,
        cancelling_nodes: Box<[RuntimeLineTaskNodeId]>,
        cleanup_started: bool,
    ) -> Self {
        Self {
            activation,
            phase,
            node_states,
            outstanding,
            active_roots,
            cancelling_nodes,
            cleanup_started,
        }
    }

    #[must_use]
    pub(crate) const fn activation(&self) -> u64 {
        self.activation
    }

    #[must_use]
    pub(crate) const fn phase(&self) -> LineTaskPhase {
        self.phase
    }

    #[must_use]
    pub(crate) const fn node_states(&self) -> &[LineTaskNodeState] {
        &self.node_states
    }

    #[must_use]
    pub(crate) const fn outstanding(&self) -> &[LineTaskWork] {
        &self.outstanding
    }

    #[must_use]
    pub(crate) const fn active_roots(&self) -> &[RuntimeLineTaskNodeId] {
        &self.active_roots
    }

    #[must_use]
    pub(crate) const fn cancelling_nodes(&self) -> &[RuntimeLineTaskNodeId] {
        &self.cancelling_nodes
    }

    #[must_use]
    pub(crate) const fn cleanup_started(&self) -> bool {
        self.cleanup_started
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
    #[error("line-task snapshot marks terminal node {node} as an active root")]
    TerminalActiveRoot { node: RuntimeLineTaskNodeId },
    #[error("line-task snapshot marks node {node} cancelling without Cancelling state")]
    CancellingNodeState { node: RuntimeLineTaskNodeId },
    #[error("line-task snapshot work {work:?} is incompatible with its phase")]
    WorkPhase { work: LineTaskWork },
    #[error("line-task snapshot cleanup flag is incompatible with its phase")]
    CleanupPhase,
}

/// Native dialogue state for one group. Capture values themselves are held by
/// `DialogueState`; this owner records only deterministic graph progress.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineTaskLiveState {
    node_states: Box<[LineTaskNodeState]>,
    activation: LineTaskActivationId,
    phase: LineTaskPhase,
    outstanding: BTreeSet<LineTaskWork>,
    active_roots: BTreeSet<RuntimeLineTaskNodeId>,
    cancelling_nodes: BTreeSet<RuntimeLineTaskNodeId>,
    cleanup_started: bool,
}

impl LineTaskLiveState {
    #[must_use]
    pub(crate) fn new<P: LineTaskPlanView>(group: &P, activation: u64) -> Self {
        Self {
            node_states: vec![LineTaskNodeState::Armed; group.node_count()].into_boxed_slice(),
            activation: LineTaskActivationId(activation),
            phase: LineTaskPhase::Active,
            outstanding: BTreeSet::new(),
            active_roots: BTreeSet::new(),
            cancelling_nodes: BTreeSet::new(),
            cleanup_started: false,
        }
    }

    /// Returns the sole complete persistence representation of this reducer.
    #[must_use]
    pub(crate) fn snapshot(&self) -> LineTaskLiveSnapshot {
        LineTaskLiveSnapshot::new(
            self.activation.0,
            self.phase,
            self.node_states.clone(),
            self.outstanding.iter().copied().collect(),
            self.active_roots.iter().copied().collect(),
            self.cancelling_nodes.iter().copied().collect(),
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
            node_states: snapshot.node_states,
            activation: LineTaskActivationId(snapshot.activation),
            phase: snapshot.phase,
            outstanding: snapshot.outstanding.into_vec().into_iter().collect(),
            active_roots: snapshot.active_roots.into_vec().into_iter().collect(),
            cancelling_nodes: snapshot.cancelling_nodes.into_vec().into_iter().collect(),
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
        self.node_states.get(node.index()).copied()
    }

    pub(crate) fn set_node_state(&mut self, node: RuntimeLineTaskNodeId, state: LineTaskNodeState) {
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

    fn complete_work(&mut self, tag: LineTaskWorkTag, failed: bool) -> bool {
        if tag.activation != self.activation || !self.outstanding.remove(&tag.work) {
            return false;
        }
        if let LineTaskWork::Node(node) = tag.work {
            self.set_node_state(
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
        true
    }

    fn begin_close(&mut self, exit: ScopeExit) -> bool {
        if !matches!(self.phase, LineTaskPhase::Active) {
            return false;
        }
        self.phase = LineTaskPhase::Closing { exit };
        self.cancel_pending_nodes();
        true
    }
}

fn validate_snapshot<P: LineTaskPlanView>(
    plan: &P,
    snapshot: &LineTaskLiveSnapshot,
) -> Result<(), LineTaskSnapshotError> {
    if snapshot.node_states.len() != plan.node_count() {
        return Err(LineTaskSnapshotError::NodeStateCount {
            expected: plan.node_count(),
            actual: snapshot.node_states.len(),
        });
    }
    let validate_node = |node| {
        plan.node_view(node)
            .is_some()
            .then_some(())
            .ok_or(LineTaskSnapshotError::UnknownNode { node })
    };
    let mut roots = BTreeSet::new();
    for &node in &snapshot.active_roots {
        validate_node(node)?;
        if !roots.insert(node) {
            return Err(LineTaskSnapshotError::DuplicateActiveRoot { node });
        }
        if snapshot.node_states[node.index()].is_terminal() {
            return Err(LineTaskSnapshotError::TerminalActiveRoot { node });
        }
    }
    let mut cancelling = BTreeSet::new();
    for &node in &snapshot.cancelling_nodes {
        validate_node(node)?;
        if !cancelling.insert(node) {
            return Err(LineTaskSnapshotError::DuplicateCancellingNode { node });
        }
        if snapshot.node_states[node.index()] != LineTaskNodeState::Cancelling {
            return Err(LineTaskSnapshotError::CancellingNodeState { node });
        }
    }
    let mut outstanding = BTreeSet::new();
    for &work in &snapshot.outstanding {
        if !outstanding.insert(work) {
            return Err(LineTaskSnapshotError::DuplicateOutstanding { work });
        }
        match work {
            LineTaskWork::Node(node) => validate_node(node)?,
            LineTaskWork::Cancellation(_) | LineTaskWork::Cleanup(_) => {}
        }
        let valid_phase = match work {
            LineTaskWork::Node(_) => !matches!(snapshot.phase, LineTaskPhase::Closed { .. }),
            LineTaskWork::Cancellation(_) | LineTaskWork::Cleanup(_) => {
                matches!(snapshot.phase, LineTaskPhase::Closing { .. })
            }
        };
        if !valid_phase {
            return Err(LineTaskSnapshotError::WorkPhase { work });
        }
    }
    if snapshot.cleanup_started && matches!(snapshot.phase, LineTaskPhase::Active) {
        return Err(LineTaskSnapshotError::CleanupPhase);
    }
    Ok(())
}

fn drain_cancellations(state: &mut LineTaskLiveState, activation: &mut LineTaskActivation) {
    activation
        .commands
        .extend(
            std::mem::take(&mut state.cancelling_nodes)
                .into_iter()
                .map(|node| LineTaskCommand::Cancel {
                    activation: state.activation,
                    node,
                }),
        );
}

/// Starts or progresses a sealed line-task graph. Node identity, rather than
/// a traversal ordinal, makes repeated host steps idempotent.
pub(crate) fn progress_live_line_task_group<P: LineTaskPlanView>(
    group: &P,
    elapsed: LogicalDuration,
    marks: &BTreeSet<RuntimeDialogueMarkId>,
    state: &mut LineTaskLiveState,
) -> LineTaskActivation {
    if !matches!(state.phase, LineTaskPhase::Active) {
        return LineTaskActivation::default();
    }
    let mut activation = LineTaskActivation::default();
    activate_node(
        group,
        group.root_node(),
        elapsed,
        marks,
        state,
        LineTaskExitPolicy::default(),
        &mut activation,
    );
    progress_active_roots(group, elapsed, marks, state, &mut activation);
    drain_cancellations(state, &mut activation);
    activation
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
            tag: LineTaskWorkTag {
                activation: state.activation,
                work: LineTaskWork::Cancellation(mark),
            },
            policy: LineTaskExitPolicy::new(ChildJoinPolicy::Join, ChildCancelPolicy::Finish),
        });
        state.outstanding.insert(LineTaskWork::Cancellation(mark));
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

/// Completes one tagged work item, then emits cleanup exactly once when the
/// closing protocol has no joined graph work left.
pub(crate) fn complete_live_line_task_work<P: LineTaskPlanView>(
    group: &P,
    state: &mut LineTaskLiveState,
    tag: LineTaskWorkTag,
    failed: bool,
) -> LineTaskActivation {
    if !state.complete_work(tag, failed) {
        return LineTaskActivation::default();
    }
    let mut activation = LineTaskActivation::default();
    if matches!(state.phase, LineTaskPhase::Active) {
        activate_node(
            group,
            group.root_node(),
            LogicalDuration::default(),
            &BTreeSet::new(),
            state,
            LineTaskExitPolicy::default(),
            &mut activation,
        );
        progress_active_roots(
            group,
            LogicalDuration::default(),
            &BTreeSet::new(),
            state,
            &mut activation,
        );
    }
    if failed && matches!(state.phase, LineTaskPhase::Active) {
        state.begin_close(ScopeExit::Failed);
    }
    activation
        .commands
        .extend(finalize_live_line_task_close(group, state).commands);
    drain_cancellations(state, &mut activation);
    activation
}

/// Advances Closing after executor-owned joined work reports drained.
pub(crate) fn finalize_live_line_task_close<P: LineTaskPlanView>(
    group: &P,
    state: &mut LineTaskLiveState,
) -> LineTaskActivation {
    let LineTaskPhase::Closing { exit } = state.phase else {
        return LineTaskActivation::default();
    };
    if !state.outstanding.is_empty() {
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
    state.outstanding.insert(LineTaskWork::Cleanup(exit));
    LineTaskActivation {
        commands: vec![LineTaskCommand::Run {
            tag: LineTaskWorkTag {
                activation: state.activation,
                work: LineTaskWork::Cleanup(exit),
            },
            policy: LineTaskExitPolicy::new(ChildJoinPolicy::Join, ChildCancelPolicy::Finish),
        }],
    }
}

fn activate_node<P: LineTaskPlanView>(
    group: &P,
    id: RuntimeLineTaskNodeId,
    elapsed: LogicalDuration,
    marks: &BTreeSet<RuntimeDialogueMarkId>,
    state: &mut LineTaskLiveState,
    policy: LineTaskExitPolicy,
    activation: &mut LineTaskActivation,
) {
    let Some(node) = group.node_view(id) else {
        return;
    };
    match node {
        LineTaskNodeView::Sequence(nodes) => {
            if state.node_state(id) == Some(LineTaskNodeState::Armed) {
                state.set_node_state(id, LineTaskNodeState::Running);
            }
            if let Some(exit) = sequence_exit(nodes, state) {
                state.set_node_state(id, exit);
                cancel_remaining(nodes, state);
                return;
            }
            if let Some(child) = nodes.iter().find(|child| {
                !state
                    .node_state(**child)
                    .is_some_and(LineTaskNodeState::is_terminal)
            }) {
                activate_node(group, *child, elapsed, marks, state, policy, activation);
            }
            if let Some(exit) = sequence_exit(nodes, state) {
                state.set_node_state(id, exit);
                cancel_remaining(nodes, state);
            }
        }
        LineTaskNodeView::Start(nodes) => {
            if state.node_state(id) == Some(LineTaskNodeState::Armed) {
                state.set_node_state(id, LineTaskNodeState::Running);
                for child in nodes {
                    activate_node(group, *child, elapsed, marks, state, policy, activation);
                }
                state.active_roots.extend(nodes.iter().copied());
                state.set_node_state(id, LineTaskNodeState::Completed);
            }
        }
        LineTaskNodeView::Parallel(children) => {
            if state.node_state(id) == Some(LineTaskNodeState::Armed) {
                state.set_node_state(id, LineTaskNodeState::Running);
            }
            for child in children {
                activate_node(group, *child, elapsed, marks, state, policy, activation);
            }
            if let Some(exit) = parallel_exit(children, state) {
                state.set_node_state(id, exit);
                if exit != LineTaskNodeState::Completed {
                    cancel_remaining(children, state);
                }
            }
        }
        child @ LineTaskNodeView::Child { .. } => {
            activate_child(group, id, elapsed, marks, state, activation, child);
        }
        LineTaskNodeView::Action => {
            if state.node_state(id) == Some(LineTaskNodeState::Armed) {
                state.set_node_state(id, LineTaskNodeState::Running);
                if !group.has_action(id) {
                    state.set_node_state(id, LineTaskNodeState::Completed);
                    return;
                }
                let work = LineTaskWork::Node(id);
                if policy.join == ChildJoinPolicy::Detached {
                    state.set_node_state(id, LineTaskNodeState::Detached);
                } else {
                    state.outstanding.insert(work);
                }
                activation.commands.push(LineTaskCommand::Run {
                    tag: LineTaskWorkTag {
                        activation: state.activation,
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
    marks: &BTreeSet<RuntimeDialogueMarkId>,
    state: &mut LineTaskLiveState,
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
    if state.node_state(id) == Some(LineTaskNodeState::Armed)
        && trigger_is_ready(&trigger, marks, elapsed)
    {
        state.set_node_state(id, LineTaskNodeState::Running);
        activate_node(
            group,
            scope,
            elapsed,
            marks,
            state,
            child_policy,
            activation,
        );
        if child_policy.join == ChildJoinPolicy::Detached {
            state.active_roots.insert(scope);
            state.set_node_state(id, LineTaskNodeState::Detached);
        }
    } else if state.node_state(id) == Some(LineTaskNodeState::Running) {
        activate_node(
            group,
            scope,
            elapsed,
            marks,
            state,
            child_policy,
            activation,
        );
        if let Some(exit) = state.node_state(scope).filter(|state| state.is_terminal()) {
            state.set_node_state(id, exit);
        }
    }
}

fn sequence_exit(
    children: &[RuntimeLineTaskNodeId],
    state: &LineTaskLiveState,
) -> Option<LineTaskNodeState> {
    for child in children {
        match state.node_state(*child)? {
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
    state: &LineTaskLiveState,
) -> Option<LineTaskNodeState> {
    let states = children
        .iter()
        .map(|child| state.node_state(*child))
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

fn cancel_remaining(children: &[RuntimeLineTaskNodeId], state: &mut LineTaskLiveState) {
    for child in children {
        if !state
            .node_state(*child)
            .is_some_and(LineTaskNodeState::is_terminal)
        {
            state.set_node_state(*child, LineTaskNodeState::Cancelling);
            state.cancelling_nodes.insert(*child);
        }
    }
}

fn progress_active_roots<P: LineTaskPlanView>(
    group: &P,
    elapsed: LogicalDuration,
    marks: &BTreeSet<RuntimeDialogueMarkId>,
    state: &mut LineTaskLiveState,
    activation: &mut LineTaskActivation,
) {
    let roots = state.active_roots.iter().copied().collect::<Vec<_>>();
    for root in roots {
        if state
            .node_state(root)
            .is_some_and(LineTaskNodeState::is_terminal)
        {
            state.active_roots.remove(&root);
            continue;
        }
        activate_node(
            group,
            root,
            elapsed,
            marks,
            state,
            LineTaskExitPolicy::default(),
            activation,
        );
        if state
            .node_state(root)
            .is_some_and(LineTaskNodeState::is_terminal)
        {
            state.active_roots.remove(&root);
        }
    }
}

fn trigger_is_ready(
    trigger: &LineTaskTrigger,
    marks: &BTreeSet<RuntimeDialogueMarkId>,
    elapsed: LogicalDuration,
) -> bool {
    match trigger {
        LineTaskTrigger::Immediate => true,
        LineTaskTrigger::Mark(mark) => marks.contains(mark),
        LineTaskTrigger::Delay(duration) => elapsed.as_nanos() >= duration.as_nanos(),
    }
}
