use crate::CheckedModule;
use arcweft_core::{
    FlowEvent, FlowFiber, FlowFiberStatus, LineEffectRequest, LineTaskGroup, LineTaskNode,
    LineTaskScope, LineTaskTrigger, SourceEvent, SourceEventKind, SourcePolicy, StreamEvent,
    StreamOp, TaskSpec,
};
use arcweft_runtime_plan::{LoweredLineTaskGroup, lower_runtime_plan};
use arcweft_test::{ScriptBench, ScriptTest};
use arcweft_verify::{BackendKind, VerificationMode, VerificationPolicy, verify_module};

#[derive(serde::Serialize)]
pub(crate) struct CheckReport {
    pub(crate) status: String,
    pub(crate) flows: usize,
    pub(crate) line_task_groups: usize,
    pub(crate) syntax_warnings: usize,
    pub(crate) verifier_diagnostics: usize,
    pub(crate) verifier_obligations: usize,
    pub(crate) unsafe_audits: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimePlanReport {
    pub(crate) lines: Vec<RuntimeLinePlanSummary>,
    pub(crate) streams: Vec<RuntimeStreamPlanSummary>,
    pub(crate) sources: Vec<RuntimeSourcePlanSummary>,
    pub(crate) verifier_diagnostics: usize,
    pub(crate) verifier_obligations: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeStreamPlanSummary {
    pub(crate) id: String,
    pub(crate) item_ty: String,
    pub(crate) error_ty: String,
    pub(crate) ops: usize,
    pub(crate) yields: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeSourcePlanSummary {
    pub(crate) id: String,
    pub(crate) item_ty: String,
    pub(crate) error_ty: String,
    pub(crate) policy: RuntimeSourcePolicySummary,
    pub(crate) handlers: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeSourcePolicySummary {
    pub(crate) backpressure: String,
    pub(crate) replay: String,
    pub(crate) privacy: String,
    pub(crate) max_queue: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeLinePlanSummary {
    pub(crate) flow_id: Option<String>,
    pub(crate) line_id: Option<String>,
    pub(crate) callee: String,
    pub(crate) child_tasks: usize,
    pub(crate) effects: usize,
    pub(crate) root: RuntimeNodeSummary,
    pub(crate) options: usize,
    pub(crate) bindings: usize,
    pub(crate) out: usize,
    pub(crate) cancel_rules: usize,
    pub(crate) memo: usize,
    pub(crate) assertions: usize,
}

#[derive(serde::Serialize)]
struct RuntimeScopeSummary {
    node: Box<RuntimeNodeSummary>,
    defer_count: usize,
    completed_defer_count: usize,
    cancelled_defer_count: usize,
    failed_defer_count: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeNodeSummary {
    pub(crate) kind: String,
    children: Vec<RuntimeNodeSummary>,
    task: Option<Box<RuntimeTaskSummary>>,
    effect: Option<String>,
}

#[derive(serde::Serialize)]
struct RuntimeTaskSummary {
    id: String,
    key: Option<String>,
    name: Option<String>,
    trigger: String,
    priority: i32,
    join_policy: String,
    cancel_policy: String,
    scope: Box<RuntimeScopeSummary>,
}

impl CheckReport {
    pub(crate) fn from_checked(
        checked: &CheckedModule,
        verification: &arcweft_verify::VerificationReport,
    ) -> Self {
        Self {
            status: if verification.has_errors() {
                "failed"
            } else {
                "ok"
            }
            .to_owned(),
            flows: checked.hir.flows().len(),
            line_task_groups: checked.line_task_groups.len(),
            syntax_warnings: checked.syntax_warnings,
            verifier_diagnostics: verification.diagnostics.len(),
            verifier_obligations: verification.obligations.len(),
            unsafe_audits: verification.unsafe_audit_count(),
        }
    }
}

impl RuntimePlanReport {
    pub(crate) fn from_checked(checked: &CheckedModule) -> Self {
        let verification = verify_module(
            &checked.hir,
            VerificationPolicy {
                mode: VerificationMode::Dev,
                backend: BackendKind::Emit,
            },
        );
        let runtime_plan = lower_runtime_plan(&checked.hir).ok();
        Self {
            lines: checked
                .line_task_groups
                .iter()
                .map(RuntimeLinePlanSummary::from_lowered)
                .collect(),
            streams: runtime_plan
                .as_ref()
                .map(|plan| {
                    plan.stream_plans
                        .iter()
                        .map(|stream| RuntimeStreamPlanSummary {
                            id: stream.id.0.clone(),
                            item_ty: stream.item_ty.clone(),
                            error_ty: stream.error_ty.clone(),
                            ops: stream.ops.len(),
                            yields: stream.ops.iter().map(count_stream_yields).sum(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            sources: runtime_plan
                .as_ref()
                .map(|plan| {
                    plan.source_plans
                        .iter()
                        .map(|source| RuntimeSourcePlanSummary {
                            id: source.id.0.clone(),
                            item_ty: source.item_ty.clone(),
                            error_ty: source.error_ty.clone(),
                            policy: source_policy_summary(&source.policy),
                            handlers: source.handlers.len(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            verifier_diagnostics: verification.diagnostics.len(),
            verifier_obligations: verification.obligations.len(),
        }
    }
}

fn count_stream_yields(op: &StreamOp) -> usize {
    match op {
        StreamOp::Yield { .. } => 1,
        StreamOp::ForNext { body, .. } => body.iter().map(count_stream_yields).sum(),
        StreamOp::If {
            then_ops, else_ops, ..
        } => then_ops
            .iter()
            .chain(else_ops)
            .map(count_stream_yields)
            .sum(),
        StreamOp::Match { arms, .. } => arms
            .iter()
            .flat_map(|arm| &arm.ops)
            .map(count_stream_yields)
            .sum(),
        StreamOp::Let { .. } | StreamOp::Close { .. } | StreamOp::Return | StreamOp::Noop => 0,
    }
}

fn source_policy_summary(policy: &SourcePolicy) -> RuntimeSourcePolicySummary {
    RuntimeSourcePolicySummary {
        backpressure: format!("{:?}", policy.backpressure),
        replay: format!("{:?}", policy.replay),
        privacy: format!("{:?}", policy.privacy),
        max_queue: policy.max_queue,
    }
}

impl RuntimeLinePlanSummary {
    fn from_lowered(line: &LoweredLineTaskGroup) -> Self {
        let group = line.group();
        let root = node_summary(&group.root.node);
        Self {
            flow_id: line.flow_id().map(|id| id.body().to_owned()),
            line_id: line.line_id().map(|id| id.body().to_owned()),
            callee: line.callee().to_owned(),
            child_tasks: count_child_tasks(group),
            effects: count_effects(group),
            root,
            options: group.options.len(),
            bindings: group.bindings.len(),
            out: group.out.len(),
            cancel_rules: group.cancel_rules.len(),
            memo: group.memo.len(),
            assertions: group.assertions.len(),
        }
    }
}

fn scope_summary(scope: &LineTaskScope) -> RuntimeScopeSummary {
    RuntimeScopeSummary {
        node: Box::new(node_summary(&scope.node)),
        defer_count: scope.defer_stack.len(),
        completed_defer_count: scope.completed_defer_stack.len(),
        cancelled_defer_count: scope.cancelled_defer_stack.len(),
        failed_defer_count: scope.failed_defer_stack.len(),
    }
}

fn node_summary(node: &LineTaskNode) -> RuntimeNodeSummary {
    match node {
        LineTaskNode::Seq(children) => node_children_summary("seq", children),
        LineTaskNode::Start(children) => node_children_summary("start", children),
        LineTaskNode::Parallel { children, .. } => node_children_summary("parallel", children),
        LineTaskNode::Child(task) => RuntimeNodeSummary {
            kind: "child".to_owned(),
            children: Vec::new(),
            task: Some(Box::new(RuntimeTaskSummary {
                id: task.id.0.clone(),
                key: task.key.as_ref().map(|key| key.0.clone()),
                name: task.name.clone(),
                trigger: trigger_label(&task.trigger),
                priority: task.priority.0,
                join_policy: format!("{:?}", task.join_policy),
                cancel_policy: format!("{:?}", task.cancel_policy),
                scope: Box::new(scope_summary(&task.scope)),
            })),
            effect: None,
        },
        LineTaskNode::Effect(effect) => RuntimeNodeSummary {
            kind: "effect".to_owned(),
            children: Vec::new(),
            task: None,
            effect: Some(effect_label(effect)),
        },
    }
}

fn node_children_summary(kind: &str, children: &[LineTaskNode]) -> RuntimeNodeSummary {
    RuntimeNodeSummary {
        kind: kind.to_owned(),
        children: children.iter().map(node_summary).collect(),
        task: None,
        effect: None,
    }
}

fn trigger_label(trigger: &LineTaskTrigger) -> String {
    match trigger {
        LineTaskTrigger::Immediate => "immediate".to_owned(),
        LineTaskTrigger::Mark(name) => format!("mark {name}"),
        LineTaskTrigger::Delay(duration) => format!("delay {}ns", duration.as_nanos()),
    }
}

fn effect_label(effect: &LineEffectRequest) -> String {
    match effect {
        LineEffectRequest::RegisterHandle { key, .. } => format!("register {key}"),
        LineEffectRequest::DropHandle { key } => format!("drop {key}"),
        LineEffectRequest::WaitMark(mark) => format!("wait mark {mark}"),
        LineEffectRequest::Wait(duration) => format!("wait {}ns", duration.as_nanos()),
        LineEffectRequest::Call(call) => format!("call {}", call.callee),
        LineEffectRequest::Log(log) => format!("log.{}", log.level),
        LineEffectRequest::SignalWrite(write) => format!("signal.set {}", write.target),
        LineEffectRequest::MetricWrite(write) => format!("metric.set {}", write.target),
        LineEffectRequest::EmitEvent(event) => format!("event.emit {}", event.event),
        LineEffectRequest::Command(command) => format!("command {}", command.name),
        LineEffectRequest::Out(_) => "out".to_owned(),
        LineEffectRequest::Return(_) => "return".to_owned(),
        LineEffectRequest::Goto(_) => "goto".to_owned(),
        LineEffectRequest::Panic(_) => "panic".to_owned(),
        LineEffectRequest::Fail(_) => "fail".to_owned(),
        LineEffectRequest::Bail(_) => "bail".to_owned(),
        LineEffectRequest::Ensure { .. } => "ensure".to_owned(),
        LineEffectRequest::Close(_) => "close".to_owned(),
        LineEffectRequest::Select(_) => "select".to_owned(),
        LineEffectRequest::Break { .. } => "break".to_owned(),
        LineEffectRequest::Continue { .. } => "continue".to_owned(),
    }
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeRunReport {
    pub(crate) frames: Vec<RuntimeFrameRunSummary>,
    pub(crate) final_status: String,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptTestRunReport {
    pub(crate) tests: Vec<ScriptTestRunSummary>,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchRunReport {
    pub(crate) benches: Vec<ScriptBenchRunSummary>,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptTestRunSummary {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) frames_run: usize,
    pub(crate) final_status: Option<String>,
    pub(crate) diagnostics: Vec<String>,
    pub(crate) frames: Vec<RuntimeFrameRunSummary>,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchRunSummary {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) sections: Vec<ScriptBenchSectionRunSummary>,
    pub(crate) diagnostics: Vec<String>,
}

#[derive(serde::Serialize)]
pub(crate) struct ScriptBenchSectionRunSummary {
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) diagnostics: Vec<String>,
}

impl ScriptBenchRunSummary {
    pub(crate) fn new(
        bench: &ScriptBench,
        status: impl Into<String>,
        sections: Vec<ScriptBenchSectionRunSummary>,
        diagnostics: Vec<String>,
    ) -> Self {
        Self {
            id: bench.id.clone(),
            status: status.into(),
            sections,
            diagnostics,
        }
    }
}

impl ScriptBenchSectionRunSummary {
    pub(crate) fn new(
        name: impl Into<String>,
        status: impl Into<String>,
        diagnostics: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: status.into(),
            diagnostics,
        }
    }
}

impl ScriptTestRunSummary {
    pub(crate) fn skipped(test: &ScriptTest, reason: impl Into<String>) -> Self {
        Self {
            id: test.id.clone(),
            kind: test.kind.clone(),
            status: "skipped".to_owned(),
            frames_run: 0,
            final_status: None,
            diagnostics: vec![reason.into()],
            frames: Vec::new(),
        }
    }

    pub(crate) fn completed(
        test: &ScriptTest,
        passed: bool,
        final_status: String,
        diagnostics: Vec<String>,
        frames: Vec<RuntimeFrameRunSummary>,
    ) -> Self {
        Self {
            id: test.id.clone(),
            kind: test.kind.clone(),
            status: if passed { "passed" } else { "failed" }.to_owned(),
            frames_run: frames.len(),
            final_status: Some(final_status),
            diagnostics,
            frames,
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeFrameRunSummary {
    pub(crate) index: usize,
    pub(crate) diagnostics: Vec<String>,
    pub(crate) flow_events: Vec<String>,
    pub(crate) line_effects: Vec<String>,
    pub(crate) task_requests: Vec<String>,
    pub(crate) observations: RuntimeObservationSummary,
    pub(crate) source_events: Vec<String>,
    pub(crate) stream_events: Vec<String>,
    pub(crate) source_close_requests: Vec<String>,
    pub(crate) source_states: Vec<RuntimeQueueStateSummary>,
    pub(crate) stream_states: Vec<RuntimeQueueStateSummary>,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeObservationSummary {
    pub(crate) signals: Vec<RuntimeObservedAssignment>,
    pub(crate) metrics: Vec<RuntimeObservedAssignment>,
    pub(crate) logs: Vec<RuntimeObservedLog>,
    pub(crate) events: Vec<RuntimeObservedEvent>,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeObservedAssignment {
    pub(crate) target: String,
    pub(crate) value: String,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeObservedLog {
    pub(crate) level: String,
    pub(crate) message: String,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeObservedEvent {
    pub(crate) event: String,
}

#[derive(serde::Serialize)]
pub(crate) struct RuntimeQueueStateSummary {
    pub(crate) id: String,
    pub(crate) queue_depth: usize,
    pub(crate) closed: bool,
    pub(crate) overflow_count: u64,
}

impl RuntimeFrameRunSummary {
    pub(crate) fn from_output(
        index: usize,
        output: arcweft_core::FrameOutput,
        fiber: &FlowFiber,
    ) -> Self {
        Self {
            index,
            diagnostics: output
                .diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.message)
                .collect(),
            flow_events: output.flow_events.iter().map(flow_event_label).collect(),
            line_effects: output.line_effects.iter().map(effect_label).collect(),
            task_requests: output
                .task_requests
                .iter()
                .map(task_request_label)
                .collect(),
            observations: RuntimeObservationSummary {
                signals: fiber
                    .observations
                    .signals
                    .iter()
                    .map(|(target, value)| RuntimeObservedAssignment {
                        target: target.clone(),
                        value: value.clone(),
                    })
                    .collect(),
                metrics: fiber
                    .observations
                    .metrics
                    .iter()
                    .map(|(target, value)| RuntimeObservedAssignment {
                        target: target.clone(),
                        value: value.clone(),
                    })
                    .collect(),
                logs: fiber
                    .observations
                    .logs
                    .iter()
                    .map(|log| RuntimeObservedLog {
                        level: log.level.clone(),
                        message: log.message.clone(),
                    })
                    .collect(),
                events: fiber
                    .observations
                    .events
                    .iter()
                    .map(|event| RuntimeObservedEvent {
                        event: event.event.clone(),
                    })
                    .collect(),
            },
            source_events: output
                .source_events
                .iter()
                .map(source_event_label)
                .collect(),
            stream_events: output
                .stream_events
                .iter()
                .map(stream_event_label)
                .collect(),
            source_close_requests: output
                .source_close_requests
                .iter()
                .map(|source| source.0.clone())
                .collect(),
            source_states: fiber
                .source_states
                .values()
                .map(|state| RuntimeQueueStateSummary {
                    id: state.id.0.clone(),
                    queue_depth: state.queue.len(),
                    closed: state.closed,
                    overflow_count: state.overflow_count,
                })
                .collect(),
            stream_states: fiber
                .stream_states
                .values()
                .map(|state| RuntimeQueueStateSummary {
                    id: state.id.0.clone(),
                    queue_depth: state.queue.len(),
                    closed: state.closed,
                    overflow_count: 0,
                })
                .collect(),
        }
    }
}

fn flow_event_label(event: &FlowEvent) -> String {
    match event {
        FlowEvent::DialogueLine { line } => format!("dialogue {}", line.0),
        FlowEvent::LineCancelled { trigger } => format!("line_cancelled {trigger}"),
        FlowEvent::ChoicePresented { id } => {
            format!("choice_presented {}", id.as_deref().unwrap_or("-"))
        }
        FlowEvent::ChoiceSelected { id, option } => {
            format!("choice_selected {} {option}", id.as_deref().unwrap_or("-"))
        }
        FlowEvent::AwaitStarted { need, task } => format!("await_started {} {}", need.0, task.0),
        FlowEvent::AwaitReady { need, value } => format!("await_ready {} {value}", need.0),
        FlowEvent::AwaitProgress { need, progress } => {
            format!("await_progress {} {progress}", need.0)
        }
        FlowEvent::Goto { target } => format!("goto {}", target.0),
        FlowEvent::Return { value } => format!("return {value}"),
        FlowEvent::Done => "done".to_owned(),
    }
}

fn task_request_label(task: &TaskSpec) -> String {
    format!("{} key={} class={:?}", task.id.0, task.key.0, task.class)
}

fn source_event_label(event: &SourceEvent<String, String>) -> String {
    format!("{} {}", event.source.0, event_kind_label(&event.kind))
}

fn stream_event_label(event: &StreamEvent<String, String>) -> String {
    format!("{} {}", event.stream.0, event_kind_label(&event.kind))
}

fn event_kind_label(kind: &SourceEventKind<String, String>) -> String {
    match kind {
        SourceEventKind::Item(item) => format!("item {item}"),
        SourceEventKind::Progress(progress) => format!("progress {progress}"),
        SourceEventKind::Disconnected => "disconnected".to_owned(),
        SourceEventKind::PermissionRevoked => "permission_revoked".to_owned(),
        SourceEventKind::Error(error) => format!("error {error}"),
        SourceEventKind::End => "end".to_owned(),
    }
}

pub(crate) fn flow_status_label(status: &FlowFiberStatus) -> String {
    match status {
        FlowFiberStatus::Running => "running".to_owned(),
        FlowFiberStatus::Waiting(state) => format!("waiting {}", state.target.task.0),
        FlowFiberStatus::Choice(state) => {
            format!("choice {}", state.id.as_deref().unwrap_or("-"))
        }
        FlowFiberStatus::Done(exit) => format!("done {exit:?}"),
        FlowFiberStatus::Failed(message) => format!("failed {message}"),
    }
}

fn count_child_tasks(group: &LineTaskGroup) -> usize {
    count_child_tasks_in_node(&group.root.node)
}

fn count_child_tasks_in_node(node: &LineTaskNode) -> usize {
    match node {
        LineTaskNode::Seq(children)
        | LineTaskNode::Start(children)
        | LineTaskNode::Parallel { children, .. } => {
            children.iter().map(count_child_tasks_in_node).sum()
        }
        LineTaskNode::Child(task) => 1 + count_child_tasks_in_node(&task.scope.node),
        LineTaskNode::Effect(_) => 0,
    }
}

fn count_effects(group: &LineTaskGroup) -> usize {
    count_effects_in_scope(&group.root)
}

fn count_effects_in_scope(scope: &LineTaskScope) -> usize {
    count_effects_in_node(&scope.node)
        + scope.defer_stack.iter().map(Vec::len).sum::<usize>()
        + scope
            .completed_defer_stack
            .iter()
            .map(Vec::len)
            .sum::<usize>()
        + scope
            .cancelled_defer_stack
            .iter()
            .map(Vec::len)
            .sum::<usize>()
        + scope.failed_defer_stack.iter().map(Vec::len).sum::<usize>()
}

fn count_effects_in_node(node: &LineTaskNode) -> usize {
    match node {
        LineTaskNode::Seq(children) | LineTaskNode::Start(children) => {
            children.iter().map(count_effects_in_node).sum()
        }
        LineTaskNode::Parallel { children, .. } => children.iter().map(count_effects_in_node).sum(),
        LineTaskNode::Child(task) => count_effects_in_scope(&task.scope),
        LineTaskNode::Effect(_) => 1,
    }
}
