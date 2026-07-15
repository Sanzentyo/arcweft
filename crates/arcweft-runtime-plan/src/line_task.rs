//! Dialogue line-plan lowering data exposed to tooling and tests.

use crate::errors::LinePlanLowerError;
use crate::expr::runtime_call_effect;
use crate::flow::sanitize_task_id_part;
use crate::labels::{duration_expr, expr_label, pattern_label};
use arcweft_core::effect::{
    ConflictPolicy, LineEffectRequest, ResourceAccess, ResourceAccessMode, RuntimeAssignment,
    RuntimeField, RuntimeWaitTarget,
};
use arcweft_core::line_task::{
    ChildCancelPolicy, ChildJoinPolicy, LineAssertionRequest, LineBindingRequest,
    LineCancelRuleRequest, LineChildTask, LineMemoRequest, LineOptionRequest, LineOutRequest,
    LineTaskGroup, LineTaskNode, LineTaskScope, LineTaskTrigger, ParallelPolicy,
};
use arcweft_core::task::{TaskId, TaskKey, TaskPriority};
use arcweft_lang_hir::model::{HirDialogue, HirFlowItem, HirModule};
use arcweft_lang_hir::syntax::{
    ast::{
        flow::{FlowItem, Stmt, WaitTarget},
        ids::EntityRef,
        line_plan::{DeferOutcome, LinePlan, LinePlanItem, TimelineAssertPolicy, TriggerPattern},
        pattern::Pattern,
    },
    expr::{CallArg, Expr, parse_expr},
};

/// Runtime task plan produced from one checked dialogue line plan.
#[derive(Clone, Debug, PartialEq)]
pub struct LoweredLineTaskGroup {
    pub(crate) flow_id: Option<EntityRef>,
    pub(crate) line_id: Option<EntityRef>,
    pub(crate) callee: String,
    pub(crate) group: LineTaskGroup,
}

impl LoweredLineTaskGroup {
    /// Flow that owns this line plan, if it was declared inside a flow.
    pub const fn flow_id(&self) -> Option<&EntityRef> {
        self.flow_id.as_ref()
    }

    /// Dialogue line id, if present or generated during HIR lowering.
    pub const fn line_id(&self) -> Option<&EntityRef> {
        self.line_id.as_ref()
    }

    /// Normalized dialogue callee such as `alice` or `alice.say`.
    pub fn callee(&self) -> &str {
        &self.callee
    }

    /// Sans I/O task group consumed by the future runtime.
    pub const fn group(&self) -> &LineTaskGroup {
        &self.group
    }
}

/// Lowers all dialogue line plans in a HIR module to Sans I/O runtime data.
pub fn lower_line_task_groups(
    module: &HirModule,
) -> Result<Vec<LoweredLineTaskGroup>, Vec<LinePlanLowerError>> {
    let mut lowerer = RuntimePlanLowerer {
        groups: Vec::new(),
        errors: Vec::new(),
    };
    for flow in module.flows() {
        lowerer.lower_flow_items(flow.id(), flow.body());
    }
    lowerer.lower_flow_items(None, module.top_level_items());
    if lowerer.errors.is_empty() {
        Ok(lowerer.groups)
    } else {
        Err(lowerer.errors)
    }
}

struct RuntimePlanLowerer {
    groups: Vec<LoweredLineTaskGroup>,
    errors: Vec<LinePlanLowerError>,
}

impl RuntimePlanLowerer {
    fn lower_flow_items(&mut self, flow_id: Option<&EntityRef>, items: &[HirFlowItem]) {
        for item in items {
            match item {
                HirFlowItem::Dialogue(dialogue) => self.lower_dialogue(flow_id, dialogue),
                HirFlowItem::Stmt(stmt) => self.lower_stmt_dialogue(flow_id, stmt),
                HirFlowItem::Scope(scope) => self.lower_flow_items(flow_id, scope.body()),
                HirFlowItem::If(block) => {
                    self.lower_flow_items(flow_id, block.body());
                    self.lower_flow_items(flow_id, block.else_body());
                }
                HirFlowItem::IfLet(block) => {
                    self.lower_flow_items(flow_id, block.body());
                    self.lower_flow_items(flow_id, block.else_body());
                }
                HirFlowItem::Match(block) => {
                    for arm in block.arms() {
                        self.lower_flow_items(flow_id, arm.body());
                    }
                }
                HirFlowItem::Loop(block) => self.lower_flow_items(flow_id, block.body()),
                HirFlowItem::While(block) => self.lower_flow_items(flow_id, block.body()),
                HirFlowItem::WhileLet(block) => self.lower_flow_items(flow_id, block.body()),
                HirFlowItem::For(block) => self.lower_flow_items(flow_id, block.body()),
                HirFlowItem::Select(block) => {
                    for branch in block.branches() {
                        self.lower_flow_items(flow_id, branch.body());
                    }
                }
                _ => {}
            }
        }
    }

    fn lower_stmt_dialogue(&mut self, flow_id: Option<&EntityRef>, stmt: &Stmt) {
        match stmt {
            Stmt::Let { expr, .. } | Stmt::Expr { expr, .. } => {
                self.lower_dialogue_expr(flow_id, expr);
            }
            _ => {}
        }
    }

    fn lower_dialogue_expr(&mut self, flow_id: Option<&EntityRef>, expr: &Expr) {
        let Some((callee, plan)) = dialogue_expr_plan(expr) else {
            return;
        };
        match lower_line_plan(plan) {
            Ok(group) => self.groups.push(LoweredLineTaskGroup {
                flow_id: flow_id.cloned(),
                line_id: None,
                callee: expr_label(callee),
                group,
            }),
            Err(mut errors) => self.errors.append(&mut errors),
        }
    }

    fn lower_dialogue(&mut self, flow_id: Option<&EntityRef>, dialogue: &HirDialogue) {
        let Some(plan) = dialogue.plan() else {
            return;
        };
        match lower_line_plan(plan) {
            Ok(group) => self.groups.push(LoweredLineTaskGroup {
                flow_id: flow_id.cloned(),
                line_id: dialogue.id().cloned(),
                callee: dialogue.callee().to_owned(),
                group,
            }),
            Err(mut errors) => self.errors.append(&mut errors),
        }
    }
}

pub(crate) fn lower_line_plan(plan: &LinePlan) -> Result<LineTaskGroup, Vec<LinePlanLowerError>> {
    let mut state = LinePlanGraphLowerer::default();
    let mut group = LineTaskGroup::default();
    let nodes = state.lower_line_plan_items(plan.items(), &mut group);
    group.root.node = LineTaskNode::Seq(nodes);
    if state.errors.is_empty() {
        Ok(group)
    } else {
        Err(state.errors)
    }
}

pub(crate) fn lower_line_plan_statements(
    statements: &[Stmt],
) -> (Vec<LineEffectRequest>, Vec<LinePlanLowerError>) {
    let mut state = LinePlanGraphLowerer::default();
    let effects = statements
        .iter()
        .flat_map(|statement| state.lower_stmt(statement))
        .collect();
    (effects, state.errors)
}

#[derive(Default)]
struct LinePlanGraphLowerer {
    next_task_id: usize,
    errors: Vec<LinePlanLowerError>,
}

impl LinePlanGraphLowerer {
    fn lower_line_plan_items(
        &mut self,
        items: &[LinePlanItem],
        group: &mut LineTaskGroup,
    ) -> Vec<LineTaskNode> {
        let mut nodes = Vec::new();
        for item in items {
            nodes.extend(self.lower_line_plan_item(item, group));
        }
        nodes
    }

    fn lower_line_plan_item(
        &mut self,
        item: &LinePlanItem,
        group: &mut LineTaskGroup,
    ) -> Vec<LineTaskNode> {
        match item {
            LinePlanItem::Init(statements) => {
                let scope = self.lower_scoped_stmt_list(statements);
                merge_scope_cleanup(&mut group.root, &scope);
                effect_nodes(scope.body)
            }
            LinePlanItem::Thread(thread) => {
                let body = self.thread_stmt_body(thread.body());
                vec![LineTaskNode::Child(self.lower_child_task(
                    thread.name(),
                    LineTaskTrigger::Immediate,
                    &body,
                ))]
            }
            LinePlanItem::On { trigger, body } => {
                let trigger_name = trigger_child_name(trigger);
                let trigger = Self::line_task_trigger(trigger);
                vec![LineTaskNode::Child(self.lower_child_task(
                    Some(&trigger_name),
                    trigger,
                    body,
                ))]
            }
            LinePlanItem::Stmt(stmt) => self.lower_line_scope_stmt(stmt, &mut group.root),
            LinePlanItem::TimedCue { anchor, body } => {
                vec![LineTaskNode::Child(self.lower_timed_cue(anchor, body))]
            }
            LinePlanItem::StartGroup(items) => {
                let children = self.lower_line_plan_items(items, group);
                vec![LineTaskNode::Start(children)]
            }
            LinePlanItem::TogetherGroup(items) => {
                let children = self.lower_line_plan_items(items, group);
                self.check_parallel_conflicts(&children);
                vec![LineTaskNode::Parallel {
                    policy: ParallelPolicy::JoinAll,
                    children,
                }]
            }
            LinePlanItem::Expr(expr) => {
                if let Some(memo) = line_memo_request(expr) {
                    group.memo.push(memo);
                    Vec::new()
                } else {
                    effect_nodes(self.lower_expr_effect(expr))
                }
            }
            LinePlanItem::Option { name, value } => {
                group.options.push(LineOptionRequest {
                    name: name.clone(),
                    value: expr_label(value),
                });
                Vec::new()
            }
            LinePlanItem::Let { pattern, expr } => {
                group.bindings.push(LineBindingRequest {
                    pattern: pattern_label(pattern),
                    value: expr_label(expr),
                });
                if let Some(anchor) = parse_timed_cue_block_anchor(expr, &mut self.errors) {
                    vec![LineTaskNode::Child(self.lower_timed_cue(&anchor, expr))]
                } else {
                    Vec::new()
                }
            }
            LinePlanItem::Out(expr) => {
                let out = LineOutRequest {
                    label: None,
                    value: expr_label(expr),
                };
                group.out.push(out.clone());
                vec![LineTaskNode::Effect(LineEffectRequest::Out(out))]
            }
            LinePlanItem::CancelRule(rule) => {
                group.cancel_rules.push(LineCancelRuleRequest {
                    trigger: rule.trigger().label(),
                    action: self.lower_scoped_stmt_list(rule.action()).into_effects(),
                });
                Vec::new()
            }
            LinePlanItem::TimelineAssert(assertion) => {
                group.assertions.push(LineAssertionRequest {
                    debug: assertion.policy() == TimelineAssertPolicy::DebugOnly,
                    expr: expr_label(assertion.condition()),
                });
                Vec::new()
            }
            LinePlanItem::Raw(raw) => {
                self.errors.push(LinePlanLowerError::new(format!(
                    "raw line-plan item cannot be lowered: {raw}"
                )));
                Vec::new()
            }
        }
    }

    fn lower_timed_cue(&mut self, anchor: &Expr, body: &Expr) -> LineChildTask {
        let name = Some(format!("at({})", expr_label(anchor)));
        let trigger = if let Some(duration) = duration_expr(anchor) {
            LineTaskTrigger::Delay(duration)
        } else {
            self.errors.push(LinePlanLowerError::new(format!(
                "timed cue anchor must be a literal duration for Phase 1.5 lowering, found {}",
                expr_label(anchor)
            )));
            LineTaskTrigger::Immediate
        };
        let mut scope = LineTaskScope::default();
        let lowered = self.lower_expr_scope(body);
        merge_scope_cleanup(&mut scope, &lowered);
        scope.node = LineTaskNode::Seq(effect_nodes(lowered.body));
        self.child_task(name.as_deref(), trigger, scope)
    }

    fn lower_child_task(
        &mut self,
        name: Option<&str>,
        trigger: LineTaskTrigger,
        statements: &[Stmt],
    ) -> LineChildTask {
        let lowered = self.lower_scoped_stmt_list(statements);
        let mut scope = LineTaskScope {
            node: LineTaskNode::Seq(effect_nodes(lowered.body.clone())),
            ..LineTaskScope::default()
        };
        merge_scope_cleanup(&mut scope, &lowered);
        self.child_task(name, trigger, scope)
    }

    fn thread_stmt_body(&mut self, items: &[FlowItem]) -> Vec<Stmt> {
        items
            .iter()
            .filter_map(|item| match item {
                FlowItem::Stmt(stmt) => Some(stmt.clone()),
                other => {
                    self.errors.push(LinePlanLowerError::new(format!(
                        "line-plan thread body item cannot be lowered as a line task: {other:?}"
                    )));
                    None
                }
            })
            .collect()
    }

    fn child_task(
        &mut self,
        name: Option<&str>,
        trigger: LineTaskTrigger,
        scope: LineTaskScope,
    ) -> LineChildTask {
        let id = self.next_task_id(name);
        LineChildTask {
            id,
            key: name.map(|name| TaskKey(format!("line.task.{name}"))),
            name: name.map(str::to_owned),
            trigger,
            priority: TaskPriority(0),
            join_policy: ChildJoinPolicy::Join,
            cancel_policy: ChildCancelPolicy::CancelAndJoin,
            scope: Box::new(scope),
        }
    }

    fn next_task_id(&mut self, name: Option<&str>) -> TaskId {
        let index = self.next_task_id;
        self.next_task_id += 1;
        let suffix = name
            .map(sanitize_task_id_part)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "anonymous".to_owned());
        TaskId(format!("line.task.{index}.{suffix}"))
    }

    fn lower_line_scope_stmt(
        &mut self,
        stmt: &Stmt,
        scope: &mut LineTaskScope,
    ) -> Vec<LineTaskNode> {
        match stmt {
            Stmt::DeferBlock {
                outcome,
                statements,
            } => {
                push_defer_block(*outcome, self.lower_cleanup_block(statements), scope);
                Vec::new()
            }
            Stmt::Defer { outcome, expr } => {
                push_defer_block(*outcome, self.lower_expr_effect(expr.expr()), scope);
                Vec::new()
            }
            other => effect_nodes(self.lower_stmt(other)),
        }
    }

    fn lower_scoped_stmt_list(&mut self, statements: &[Stmt]) -> LoweredScope {
        let mut scope = LoweredScope::default();
        for statement in statements {
            match statement {
                // `defer` belongs to the nearest runtime scope. Outcome-specific
                // cleanup remains structured so cancellation can choose the right
                // stack instead of executing a synthetic effect in normal flow.
                Stmt::DeferBlock {
                    outcome,
                    statements,
                } => {
                    scope.push_defer(*outcome, self.lower_cleanup_block(statements));
                }
                Stmt::Defer { outcome, expr } => {
                    let effects = self.lower_expr_effect(expr.expr());
                    scope.push_defer(*outcome, effects);
                }
                other => scope.body.extend(self.lower_stmt(other)),
            }
        }
        scope
    }

    fn lower_cleanup_block(&mut self, statements: &[Stmt]) -> Vec<LineEffectRequest> {
        self.lower_scoped_stmt_list(statements).into_effects()
    }

    fn lower_stmt(&mut self, stmt: &Stmt) -> Vec<LineEffectRequest> {
        match stmt {
            Stmt::LifetimeSet { target, expr } => vec![LineEffectRequest::RegisterHandle {
                key: expr_label(target.expr()),
                handle: expr_label(expr.expr()),
            }],
            Stmt::Wait(WaitTarget::Duration(expr)) => {
                if let Some(duration) = duration_expr(expr.expr()) {
                    vec![LineEffectRequest::Wait(RuntimeWaitTarget::Duration(
                        duration,
                    ))]
                } else {
                    self.errors.push(LinePlanLowerError::new(format!(
                        "wait duration must be a literal duration, found {}",
                        expr_label(expr.expr())
                    )));
                    Vec::new()
                }
            }
            Stmt::Wait(WaitTarget::Expr(expr)) => {
                vec![LineEffectRequest::Wait(lower_wait_target_expr(expr.expr()))]
            }
            Stmt::Signal { target, value } => {
                vec![LineEffectRequest::SignalWrite(RuntimeAssignment {
                    target: expr_label(target.expr()),
                    value: expr_label(value.expr()),
                })]
            }
            Stmt::Expr { expr, .. } => self.lower_expr_effect(expr),
            Stmt::Out { label, expr } => vec![LineEffectRequest::Out(LineOutRequest {
                label: label.clone(),
                value: expr_label(expr.expr()),
            })],
            Stmt::Return { expr, .. } => vec![LineEffectRequest::Return(expr_label(expr))],
            Stmt::Goto(expr) => vec![LineEffectRequest::Goto(expr_label(expr.expr()))],
            Stmt::Yield(_) => {
                self.errors.push(LinePlanLowerError::new(
                    "`yield` cannot be lowered from a dialogue line plan; use `out` for line results"
                        .to_owned(),
                ));
                Vec::new()
            }
            Stmt::Close(expr) => vec![LineEffectRequest::Close(expr_label(expr.expr()))],
            Stmt::Select(expr) => vec![LineEffectRequest::Select(expr_label(expr.expr()))],
            Stmt::Break { label, expr } => vec![LineEffectRequest::Break {
                label: label.clone(),
                value: expr.as_ref().map(|expr| expr_label(expr.expr())),
            }],
            Stmt::Continue { label } => vec![LineEffectRequest::Continue {
                label: label.clone(),
            }],
            Stmt::DeferBlock { .. } | Stmt::Defer { .. } => {
                self.errors.push(LinePlanLowerError::new(
                    "`defer` must be lowered through a scoped statement list".to_owned(),
                ));
                Vec::new()
            }
            Stmt::Raw(raw) => {
                self.errors.push(LinePlanLowerError::new(format!(
                    "raw {:?} recovery node cannot be lowered: {}",
                    raw.family(),
                    raw.source()
                )));
                Vec::new()
            }
            other => {
                self.errors.push(LinePlanLowerError::new(format!(
                    "unsupported line-plan statement for runtime lowering: {other:?}"
                )));
                Vec::new()
            }
        }
    }

    fn lower_expr_effect(&mut self, expr: &Expr) -> Vec<LineEffectRequest> {
        if matches!(
            expr,
            Expr::Block { .. } | Expr::NamedBlock { .. } | Expr::ComputationBlock { .. }
        ) {
            return self.lower_expr_scope(expr).into_effects();
        }
        if let Expr::Pipe { lhs, rhs } = expr
            && is_drop_intrinsic(rhs)
        {
            return vec![LineEffectRequest::DropHandle {
                key: expr_label(lhs),
            }];
        }
        if matches!(expr, Expr::Call { .. } | Expr::Path(_)) {
            return vec![runtime_call_effect(expr)];
        }
        self.errors.push(LinePlanLowerError::new(format!(
            "unsupported line-plan expression for runtime lowering: {}",
            expr_label(expr)
        )));
        Vec::new()
    }

    fn lower_expr_scope(&mut self, expr: &Expr) -> LoweredScope {
        match expr {
            Expr::Block { statements, value }
            | Expr::ComputationBlock {
                statements, value, ..
            }
            | Expr::NamedBlock {
                statements, value, ..
            } => {
                let mut scope = self.lower_scoped_stmt_list(statements);
                if let Some(value) = value {
                    scope.body.extend(self.lower_expr_effect(value));
                }
                scope
            }
            other => LoweredScope {
                body: self.lower_expr_effect(other),
                ..LoweredScope::default()
            },
        }
    }

    fn line_task_trigger(trigger: &TriggerPattern) -> LineTaskTrigger {
        if let Some(name) = trigger_mark_name(trigger) {
            return LineTaskTrigger::Mark(name);
        }
        LineTaskTrigger::Immediate
    }

    fn check_parallel_conflicts(&mut self, children: &[LineTaskNode]) {
        let accesses: Vec<Vec<ResourceAccess>> = children.iter().map(node_accesses).collect();
        for (left_index, left) in accesses.iter().enumerate() {
            for (right_offset, right) in accesses[left_index + 1..].iter().enumerate() {
                let right_index = left_index + right_offset + 1;
                for left_access in left {
                    for right_access in right {
                        if accesses_conflict(left_access, right_access) {
                            self.errors.push(LinePlanLowerError::new(format!(
                                "parallel resource conflict between child {left_index} and child {right_index} on `{}`",
                                left_access.key
                            )));
                        }
                    }
                }
            }
        }
    }
}

#[derive(Default)]
struct LoweredScope {
    body: Vec<LineEffectRequest>,
    defer_stack: Vec<Vec<LineEffectRequest>>,
    completed_defer_stack: Vec<Vec<LineEffectRequest>>,
    cancelled_defer_stack: Vec<Vec<LineEffectRequest>>,
    failed_defer_stack: Vec<Vec<LineEffectRequest>>,
}

impl LoweredScope {
    fn push_defer(&mut self, outcome: DeferOutcome, effects: Vec<LineEffectRequest>) {
        match outcome {
            DeferOutcome::Always => self.defer_stack.push(effects),
            DeferOutcome::Completed => self.completed_defer_stack.push(effects),
            DeferOutcome::Cancelled => self.cancelled_defer_stack.push(effects),
            DeferOutcome::Failed => self.failed_defer_stack.push(effects),
        }
    }

    fn into_effects(self) -> Vec<LineEffectRequest> {
        let mut effects = self.body;
        effects.extend(flatten_defer_stack(self.defer_stack));
        effects.extend(flatten_defer_stack(self.completed_defer_stack));
        effects.extend(flatten_defer_stack(self.cancelled_defer_stack));
        effects.extend(flatten_defer_stack(self.failed_defer_stack));
        effects
    }
}

fn merge_scope_cleanup(scope: &mut LineTaskScope, lowered: &LoweredScope) {
    scope.defer_stack.extend(lowered.defer_stack.clone());
    scope
        .completed_defer_stack
        .extend(lowered.completed_defer_stack.clone());
    scope
        .cancelled_defer_stack
        .extend(lowered.cancelled_defer_stack.clone());
    scope
        .failed_defer_stack
        .extend(lowered.failed_defer_stack.clone());
}

fn push_defer_block(
    outcome: DeferOutcome,
    effects: Vec<LineEffectRequest>,
    scope: &mut LineTaskScope,
) {
    match outcome {
        DeferOutcome::Always => scope.defer_stack.push(effects),
        DeferOutcome::Completed => scope.completed_defer_stack.push(effects),
        DeferOutcome::Cancelled => scope.cancelled_defer_stack.push(effects),
        DeferOutcome::Failed => scope.failed_defer_stack.push(effects),
    }
}

fn effect_nodes(effects: Vec<LineEffectRequest>) -> Vec<LineTaskNode> {
    effects.into_iter().map(LineTaskNode::Effect).collect()
}

fn flatten_defer_stack(defer_stack: Vec<Vec<LineEffectRequest>>) -> Vec<LineEffectRequest> {
    defer_stack.into_iter().rev().flatten().collect()
}

fn dialogue_expr_plan(expr: &Expr) -> Option<(&Expr, &LinePlan)> {
    match expr {
        Expr::DialogueCall { callee, plan, .. } => Some((callee.as_ref(), plan.as_ref()?)),
        Expr::Try { expr } => dialogue_expr_plan(expr),
        _ => None,
    }
}

fn parse_timed_cue_block_anchor(expr: &Expr, errors: &mut Vec<LinePlanLowerError>) -> Option<Expr> {
    let Expr::NamedBlock { name, .. } = expr else {
        return None;
    };
    let anchor = name.strip_prefix("at(")?.strip_suffix(')')?.trim();
    if anchor.is_empty() {
        errors.push(LinePlanLowerError::new(
            "timed cue anchor cannot be empty".to_owned(),
        ));
        return None;
    }
    match parse_expr(anchor) {
        Ok(anchor) => Some(anchor),
        Err(error) => {
            errors.push(LinePlanLowerError::new(format!(
                "timed cue anchor is not a valid expression: {error}"
            )));
            None
        }
    }
}

fn trigger_mark_name(trigger: &TriggerPattern) -> Option<String> {
    match trigger {
        TriggerPattern::Mark(Pattern::Variant { name, .. }) => Some(format!(".{name}")),
        TriggerPattern::Mark(Pattern::Ident(name)) => Some(name.clone()),
        _ => None,
    }
}

fn trigger_child_name(trigger: &TriggerPattern) -> String {
    trigger_mark_name(trigger).unwrap_or_else(|| trigger.label())
}

fn node_accesses(node: &LineTaskNode) -> Vec<ResourceAccess> {
    match node {
        LineTaskNode::Seq(nodes) | LineTaskNode::Start(nodes) => {
            nodes.iter().flat_map(node_accesses).collect()
        }
        LineTaskNode::Parallel { children, .. } => {
            children.iter().flat_map(node_accesses).collect()
        }
        LineTaskNode::Child(task) => node_accesses(&task.scope.node),
        LineTaskNode::Effect(effect) => effect_accesses(effect),
    }
}

fn effect_accesses(effect: &LineEffectRequest) -> Vec<ResourceAccess> {
    match effect {
        LineEffectRequest::RegisterHandle { key, .. } => vec![resource_access(
            format!("lifetime:{key}"),
            ResourceAccessMode::Write,
            ConflictPolicy::Error,
        )],
        LineEffectRequest::DropHandle { key } => vec![resource_access(
            format!("lifetime:{key}"),
            ResourceAccessMode::Drop,
            ConflictPolicy::Error,
        )],
        LineEffectRequest::SignalWrite(write) => vec![resource_access(
            format!("signal:{}", write.target),
            ResourceAccessMode::Write,
            ConflictPolicy::Error,
        )],
        LineEffectRequest::MetricWrite(write) => vec![resource_access(
            format!("metric:{}", write.target),
            ResourceAccessMode::Write,
            ConflictPolicy::LastWriterWins { priority: 0 },
        )],
        LineEffectRequest::EmitEvent(event) => vec![resource_access(
            format!("event:{}", event.event),
            ResourceAccessMode::Append,
            ConflictPolicy::Append,
        )],
        LineEffectRequest::Log(log) => vec![resource_access(
            format!("log:{}", log.level),
            ResourceAccessMode::Append,
            ConflictPolicy::Append,
        )],
        LineEffectRequest::Audio(_) => vec![resource_access(
            "audio".to_owned(),
            ResourceAccessMode::Append,
            ConflictPolicy::Append,
        )],
        LineEffectRequest::Out(_) => vec![resource_access(
            "line:out".to_owned(),
            ResourceAccessMode::Write,
            ConflictPolicy::Error,
        )],
        LineEffectRequest::Return(_)
        | LineEffectRequest::Goto(_)
        | LineEffectRequest::Panic(_)
        | LineEffectRequest::Fail(_)
        | LineEffectRequest::Bail(_)
        | LineEffectRequest::Close(_)
        | LineEffectRequest::Select(_)
        | LineEffectRequest::Break { .. }
        | LineEffectRequest::Continue { .. } => vec![resource_access(
            "control".to_owned(),
            ResourceAccessMode::Control,
            ConflictPolicy::Error,
        )],
        LineEffectRequest::Wait(_)
        | LineEffectRequest::Call(_)
        | LineEffectRequest::Ensure { .. }
        | LineEffectRequest::Assert(_) => Vec::new(),
    }
}

fn resource_access(
    key: String,
    mode: ResourceAccessMode,
    policy: ConflictPolicy,
) -> ResourceAccess {
    ResourceAccess { key, mode, policy }
}

fn lower_wait_target_expr(expr: &Expr) -> RuntimeWaitTarget {
    if let Expr::Call { callee, args } = expr
        && matches!(callee.as_ref(), Expr::Path(path) if path == "mark")
        && args.len() == 1
    {
        RuntimeWaitTarget::Mark(expr_label(args[0].value()))
    } else {
        RuntimeWaitTarget::Expr(expr_label(expr))
    }
}

fn accesses_conflict(left: &ResourceAccess, right: &ResourceAccess) -> bool {
    if left.key != right.key {
        return false;
    }
    if matches!(left.mode, ResourceAccessMode::Read)
        && matches!(right.mode, ResourceAccessMode::Read)
    {
        return false;
    }
    if matches!(left.mode, ResourceAccessMode::Append)
        && matches!(right.mode, ResourceAccessMode::Append)
        && matches!(left.policy, ConflictPolicy::Append)
        && matches!(right.policy, ConflictPolicy::Append)
    {
        return false;
    }
    true
}

fn line_memo_request(expr: &Expr) -> Option<LineMemoRequest> {
    let Expr::Call { callee, args } = expr else {
        return None;
    };
    if !matches!(callee.as_ref(), Expr::Path(path) if path == "memo") {
        return None;
    }
    let (first, rest) = args.split_first()?;
    let name = match first {
        CallArg::Positional(Expr::Path(path)) => path.as_label().to_owned(),
        CallArg::Positional(Expr::ShortVariant(name)) => name.as_str().to_owned(),
        _ => return None,
    };
    let options = rest
        .iter()
        .filter_map(|arg| {
            let CallArg::Named { name, value } = arg else {
                return None;
            };
            Some(RuntimeField {
                name: name.clone(),
                value: expr_label(value),
            })
        })
        .collect();
    Some(LineMemoRequest { name, options })
}

fn is_drop_intrinsic(expr: &Expr) -> bool {
    match expr {
        Expr::Path(path) => matches!(path.as_str(), "drop" | "drop_optional"),
        Expr::Call { callee, .. } => {
            matches!(callee.as_ref(), Expr::Path(path) if matches!(path.as_str(), "drop" | "drop_optional"))
        }
        _ => false,
    }
}
