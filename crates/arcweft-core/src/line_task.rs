use crate::effect::{LineEffectRequest, RuntimeField};
use crate::plan::FlowEvent;
use crate::step::{RuntimeStepInput, RuntimeStepOutput};
use crate::task::{
    CancelScopeId, HostTaskRequest, TaskClass, TaskId, TaskKey, TaskPolicy, TaskPriority, TaskSpec,
};
use crate::time::LogicalDuration;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum ScopeExit {
    #[default]
    Completed,
    Cancelled,
    Failed,
}

/// Sans I/O runtime model for a dialogue line's scoped task group.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct LineTaskGroup {
    /// Root runtime scope for init work, child tasks, and grouped timeline work.
    pub root: LineTaskScope,
    /// Line option assignments such as `voice = auto`.
    pub options: Vec<LineOptionRequest>,
    /// Bindings introduced by `let PAT = EXPR` in a line plan.
    pub bindings: Vec<LineBindingRequest>,
    /// Values exported from the line plan with `out`.
    pub out: Vec<LineOutRequest>,
    /// Cancellation branches attached to this line.
    pub cancel_rules: Vec<LineCancelRuleRequest>,
    /// Memoization directives local to this line plan.
    pub memo: Vec<LineMemoRequest>,
    /// Runtime-checkable assertions attached to this line plan.
    pub assertions: Vec<LineAssertionRequest>,
    /// Automatic cleanup policy for line-owned handles and child tasks.
    pub cleanup: LineCleanupPolicy,
}

/// Runtime scope with a task graph and deterministic cleanup stacks.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct LineTaskScope {
    pub node: LineTaskNode,
    pub defer_stack: Vec<Vec<LineEffectRequest>>,
    pub completed_defer_stack: Vec<Vec<LineEffectRequest>>,
    pub cancelled_defer_stack: Vec<Vec<LineEffectRequest>>,
    pub failed_defer_stack: Vec<Vec<LineEffectRequest>>,
}

/// Structured line-plan runtime graph.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum LineTaskNode {
    Seq(Vec<LineTaskNode>),
    Start(Vec<LineTaskNode>),
    Parallel {
        policy: ParallelPolicy,
        children: Vec<LineTaskNode>,
    },
    Child(LineChildTask),
    Effect(LineEffectRequest),
}

impl Default for LineTaskNode {
    fn default() -> Self {
        Self::Seq(Vec::new())
    }
}

/// Parallel group execution policy.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum ParallelPolicy {
    #[default]
    JoinAll,
}

/// A child task declared by `thread name { ... }` inside a line plan.
///
/// Thread-local cleanup is modeled as a scoped defer stack, not as line-level
/// `finally`. That keeps cancellation semantics identical for flow, handler,
/// and line-plan threads.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineChildTask {
    pub id: TaskId,
    pub key: Option<TaskKey>,
    pub name: Option<String>,
    pub trigger: LineTaskTrigger,
    pub priority: TaskPriority,
    pub join_policy: ChildJoinPolicy,
    pub cancel_policy: ChildCancelPolicy,
    pub scope: Box<LineTaskScope>,
}

/// Condition that starts a line-scoped child task.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum LineTaskTrigger {
    #[default]
    Immediate,
    Mark(String),
    Delay(LogicalDuration),
}

/// Whether the parent waits for a child task result.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum ChildJoinPolicy {
    #[default]
    Join,
    Detached,
}

/// How a child task exits when its owning scope is cancelled.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum ChildCancelPolicy {
    #[default]
    CancelAndJoin,
    Finish,
    Detach,
}

/// Option assignment preserved from a line plan.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineOptionRequest {
    pub name: String,
    pub value: String,
}

/// Binding preserved from a line plan before full HIR execution exists.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineBindingRequest {
    pub pattern: String,
    pub value: String,
}

/// `out` value exported from a line plan or cancel branch.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineOutRequest {
    pub label: Option<String>,
    pub value: String,
}

/// Runtime representation of `cancel on ... { ... }`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineCancelRuleRequest {
    pub trigger: String,
    pub action: Vec<LineEffectRequest>,
}

/// Line-local memo directive.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineMemoRequest {
    pub name: String,
    pub options: Vec<RuntimeField>,
}

/// Runtime-checkable line assertion.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineAssertionRequest {
    pub debug: bool,
    pub expr: String,
}

/// Declarative cleanup policy applied when the line scope exits.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct LineCleanupPolicy {
    pub child_tasks: ChildTaskCleanup,
    pub presentation: PresentationCleanup,
    pub audio: AudioCleanup,
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

/// Runs a line task group into deterministic effect requests.
pub fn run_line_task_group(
    group: &LineTaskGroup,
    input: &RuntimeStepInput,
    exit: ScopeExit,
) -> RuntimeStepOutput {
    let mut output = RuntimeStepOutput::default();
    run_scope(&group.root, input, exit, &mut output);
    output
}

pub(crate) fn run_line_task_group_for_input(
    group: &LineTaskGroup,
    input: &RuntimeStepInput,
) -> RuntimeStepOutput {
    if let Some(rule) = group
        .cancel_rules
        .iter()
        .find(|rule| input_matches_trigger(input, &rule.trigger))
    {
        let mut output = RuntimeStepOutput::default();
        output.flow_events.push(FlowEvent::LineCancelled {
            trigger: rule.trigger.clone(),
        });
        output.effects.line.extend(rule.action.clone());
        run_scope_cleanup(&group.root, ScopeExit::Cancelled, &mut output);
        output
    } else {
        run_line_task_group(group, input, ScopeExit::Completed)
    }
}

fn input_matches_trigger(input: &RuntimeStepInput, trigger: &str) -> bool {
    input.input_events.iter().any(|event| {
        if event.kind == trigger {
            return true;
        }
        let Some(payload) = event.payload.as_deref() else {
            return false;
        };
        trigger == format!("{} {payload}", event.kind)
            || trigger == format!("{}:{payload}", event.kind)
    })
}

fn run_scope(
    scope: &LineTaskScope,
    input: &RuntimeStepInput,
    exit: ScopeExit,
    output: &mut RuntimeStepOutput,
) {
    run_node(&scope.node, input, output);
    run_scope_cleanup(scope, exit, output);
}

fn run_scope_cleanup(scope: &LineTaskScope, exit: ScopeExit, output: &mut RuntimeStepOutput) {
    output
        .effects
        .line
        .extend(flatten_defer_stack(&scope.defer_stack));
    output
        .effects
        .line
        .extend(flatten_defer_stack(outcome_defer_stack(scope, exit)));
}

fn run_node(node: &LineTaskNode, input: &RuntimeStepInput, output: &mut RuntimeStepOutput) {
    match node {
        LineTaskNode::Seq(nodes) | LineTaskNode::Start(nodes) => {
            for node in nodes {
                run_node(node, input, output);
            }
        }
        LineTaskNode::Parallel { children, .. } => {
            for child in children {
                run_node(child, input, output);
            }
        }
        LineTaskNode::Child(task) => run_child_task(task, input, output),
        LineTaskNode::Effect(effect) => output.effects.line.push(effect.clone()),
    }
}

fn run_child_task(task: &LineChildTask, input: &RuntimeStepInput, output: &mut RuntimeStepOutput) {
    if !trigger_is_ready(&task.trigger, input) {
        return;
    }
    output.requests.tasks.push(task_spec(task));
    run_scope(&task.scope, input, ScopeExit::Completed, output);
}

fn trigger_is_ready(trigger: &LineTaskTrigger, input: &RuntimeStepInput) -> bool {
    match trigger {
        LineTaskTrigger::Immediate => true,
        LineTaskTrigger::Mark(name) => input.input_events.iter().any(|event| {
            (event.kind == "mark" && event.payload.as_deref() == Some(name.as_str()))
                || event.kind == format!("mark:{name}")
        }),
        LineTaskTrigger::Delay(duration) => input.dt.as_nanos() >= duration.as_nanos(),
    }
}

fn task_spec(task: &LineChildTask) -> TaskSpec {
    let key = task
        .key
        .clone()
        .unwrap_or_else(|| TaskKey(task.id.0.clone()));
    let name = task
        .name
        .clone()
        .unwrap_or_else(|| "anonymous line task".to_owned());
    TaskSpec::new(
        task.id.clone(),
        key,
        TaskClass::LocalUi,
        task.priority,
        CancelScopeId("line".to_owned()),
        TaskPolicy::JoinSameKey,
        HostTaskRequest::custom("line_task", "run_child", [name.into()]),
    )
}

fn outcome_defer_stack(scope: &LineTaskScope, exit: ScopeExit) -> &[Vec<LineEffectRequest>] {
    match exit {
        ScopeExit::Completed => &scope.completed_defer_stack,
        ScopeExit::Cancelled => &scope.cancelled_defer_stack,
        ScopeExit::Failed => &scope.failed_defer_stack,
    }
}

fn flatten_defer_stack(stack: &[Vec<LineEffectRequest>]) -> Vec<LineEffectRequest> {
    stack.iter().rev().flatten().cloned().collect()
}
