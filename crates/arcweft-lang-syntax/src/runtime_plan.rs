use crate::ast::{
    DeferOutcome, EntityRef, LinePlan, LinePlanItem, Pattern, Stmt, TriggerPattern, WaitTarget,
};
use crate::expr::{DurationUnit, Expr, Literal};
use crate::lower::{HirDialogue, HirFlowItem, HirModule};
use arcweft_core::{
    LineAssertionRequest, LineBindingRequest, LineCancelRuleRequest, LineChildTask,
    LineEffectRequest, LineMemoRequest, LineOptionRequest, LineOutRequest, LineTaskGroup,
    LogicalDuration, RuntimeAssignment, RuntimeCall, RuntimeCommand, RuntimeEvent, RuntimeField,
    RuntimeLog,
};
use thiserror::Error;

/// Runtime task plan produced from one checked dialogue line plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredLineTaskGroup {
    flow_id: Option<EntityRef>,
    line_id: Option<EntityRef>,
    callee: String,
    group: LineTaskGroup,
}

/// Error produced while converting syntax/HIR line plans to core data.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct LinePlanLowerError {
    message: String,
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
                HirFlowItem::Scope(scope) => self.lower_flow_items(flow_id, scope.body()),
                HirFlowItem::If(block) => self.lower_flow_items(flow_id, block.body()),
                HirFlowItem::IfLet(block) => self.lower_flow_items(flow_id, block.body()),
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

fn lower_line_plan(plan: &LinePlan) -> Result<LineTaskGroup, Vec<LinePlanLowerError>> {
    let mut group = LineTaskGroup::default();
    let mut errors = Vec::new();
    for item in plan.items() {
        lower_line_plan_item(item, &mut group, &mut errors);
    }
    if errors.is_empty() {
        Ok(group)
    } else {
        Err(errors)
    }
}

fn lower_line_plan_item(
    item: &LinePlanItem,
    group: &mut LineTaskGroup,
    errors: &mut Vec<LinePlanLowerError>,
) {
    match item {
        LinePlanItem::Init(statements) => {
            let scope = lower_scoped_stmt_list(statements, errors);
            group.init.extend(scope.body);
            group.init_defer_stack.extend(scope.defer_stack);
        }
        LinePlanItem::Thread(thread) => {
            group
                .children
                .push(lower_child_task(thread.name(), thread.body(), errors));
        }
        LinePlanItem::On { trigger, body } => {
            let trigger_name = trigger_child_name(trigger);
            let mut task = lower_child_task(Some(&trigger_name), body, errors);
            if let Some(name) = trigger_mark_name(trigger) {
                task.body.insert(0, LineEffectRequest::WaitMark(name));
            }
            group.children.push(task);
        }
        LinePlanItem::Stmt(stmt) => {
            lower_line_scope_stmt(stmt, group, errors);
        }
        LinePlanItem::TimedCue { anchor, body } => {
            group.children.push(lower_timed_cue(anchor, body, errors));
        }
        LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
            for item in items {
                lower_line_plan_item(item, group, errors);
            }
        }
        LinePlanItem::Expr(expr) => {
            group.init.extend(lower_expr_effect(expr, errors));
        }
        LinePlanItem::Option { name, value } => group.options.push(LineOptionRequest {
            name: name.clone(),
            value: expr_label(value),
        }),
        LinePlanItem::Let { pattern, expr } => group.bindings.push(LineBindingRequest {
            pattern: pattern_label(pattern),
            value: expr_label(expr),
        }),
        LinePlanItem::Out(expr) => {
            let out = LineOutRequest {
                label: None,
                value: expr_label(expr),
            };
            group.init.push(LineEffectRequest::Out(out.clone()));
            group.out.push(out);
        }
        LinePlanItem::CancelRule(rule) => {
            group.cancel_rules.push(LineCancelRuleRequest {
                trigger: rule.trigger().label(),
                action: lower_scoped_stmt_list(rule.action(), errors).into_effects(),
            });
        }
        LinePlanItem::Memo { name, options } => group.memo.push(LineMemoRequest {
            name: name.clone(),
            options: runtime_fields(options),
        }),
        LinePlanItem::Assert { debug, expr } => {
            group.assertions.push(LineAssertionRequest {
                debug: *debug,
                expr: expr_label(expr),
            });
        }
        LinePlanItem::Raw(raw) => errors.push(LinePlanLowerError::new(format!(
            "raw line-plan item cannot be lowered: {raw}"
        ))),
    }
}

fn lower_timed_cue(
    anchor: &Expr,
    body: &Expr,
    errors: &mut Vec<LinePlanLowerError>,
) -> LineChildTask {
    let mut task = LineChildTask {
        name: Some(format!("at({})", expr_label(anchor))),
        body: Vec::new(),
        defer_stack: Vec::new(),
    };
    if let Some(duration) = duration_expr(anchor) {
        task.body.push(LineEffectRequest::Wait(duration));
    } else {
        errors.push(LinePlanLowerError::new(format!(
            "timed cue anchor must be a literal duration for Phase 1.5 lowering, found {}",
            expr_label(anchor)
        )));
    }
    append_expr_scope(body, &mut task.body, &mut task.defer_stack, errors);
    task
}

fn lower_child_task(
    name: Option<&str>,
    statements: &[Stmt],
    errors: &mut Vec<LinePlanLowerError>,
) -> LineChildTask {
    let scope = lower_scoped_stmt_list(statements, errors);
    LineChildTask {
        name: name.map(str::to_owned),
        body: scope.body,
        defer_stack: scope.defer_stack,
    }
}

#[derive(Default)]
struct LoweredScope {
    body: Vec<LineEffectRequest>,
    defer_stack: Vec<Vec<LineEffectRequest>>,
}

impl LoweredScope {
    fn into_effects(self) -> Vec<LineEffectRequest> {
        let mut effects = self.body;
        effects.extend(flatten_defer_stack(self.defer_stack));
        effects
    }
}

fn lower_line_scope_stmt(
    stmt: &Stmt,
    group: &mut LineTaskGroup,
    errors: &mut Vec<LinePlanLowerError>,
) {
    match stmt {
        Stmt::DeferBlock {
            outcome,
            statements,
        } => {
            push_defer_block(*outcome, lower_cleanup_block(statements, errors), group);
        }
        Stmt::Defer { outcome, expr } => {
            push_defer_block(*outcome, lower_expr_effect(expr, errors), group);
        }
        other => group.init.extend(lower_stmt(other, errors)),
    }
}

fn push_defer_block(
    outcome: DeferOutcome,
    effects: Vec<LineEffectRequest>,
    group: &mut LineTaskGroup,
) {
    match outcome {
        DeferOutcome::Always => group.defer_stack.push(effects),
        DeferOutcome::Completed => group.completed_defer_stack.push(effects),
        DeferOutcome::Cancelled => group.cancelled_defer_stack.push(effects),
        DeferOutcome::Failed => group.failed_defer_stack.push(effects),
    }
}

fn outcome_effects(
    outcome: DeferOutcome,
    effects: Vec<LineEffectRequest>,
) -> Vec<LineEffectRequest> {
    if matches!(outcome, DeferOutcome::Always) {
        effects
    } else {
        vec![LineEffectRequest::DeferOn {
            outcome: match outcome {
                DeferOutcome::Always => "always",
                DeferOutcome::Completed => "completed",
                DeferOutcome::Cancelled => "cancelled",
                DeferOutcome::Failed => "failed",
            }
            .to_owned(),
            effects,
        }]
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

fn lower_scoped_stmt_list(
    statements: &[Stmt],
    errors: &mut Vec<LinePlanLowerError>,
) -> LoweredScope {
    let mut scope = LoweredScope::default();
    for statement in statements {
        match statement {
            // `defer` is scoped cleanup, not a thread-only construct. The
            // caller decides which runtime scope owns the resulting stack.
            Stmt::DeferBlock {
                outcome,
                statements,
            } => {
                scope.defer_stack.push(outcome_effects(
                    *outcome,
                    lower_cleanup_block(statements, errors),
                ));
            }
            Stmt::Defer { outcome, expr } => {
                scope
                    .defer_stack
                    .push(outcome_effects(*outcome, lower_expr_effect(expr, errors)));
            }
            other => scope.body.extend(lower_stmt(other, errors)),
        }
    }
    scope
}

fn lower_cleanup_block(
    statements: &[Stmt],
    errors: &mut Vec<LinePlanLowerError>,
) -> Vec<LineEffectRequest> {
    let scope = lower_scoped_stmt_list(statements, errors);
    let mut effects = scope.body;
    effects.extend(flatten_defer_stack(scope.defer_stack));
    effects
}

fn flatten_defer_stack(defer_stack: Vec<Vec<LineEffectRequest>>) -> Vec<LineEffectRequest> {
    defer_stack.into_iter().rev().flatten().collect()
}

fn lower_stmt(stmt: &Stmt, errors: &mut Vec<LinePlanLowerError>) -> Vec<LineEffectRequest> {
    match stmt {
        Stmt::LifetimeSet { target, expr } => vec![LineEffectRequest::RegisterHandle {
            key: expr_label(target),
            handle: expr_label(expr),
        }],
        Stmt::Wait(WaitTarget::Mark(mark)) => vec![LineEffectRequest::WaitMark(mark.clone())],
        Stmt::Wait(WaitTarget::Duration(expr)) => {
            if let Some(duration) = duration_expr(expr) {
                vec![LineEffectRequest::Wait(duration)]
            } else {
                errors.push(LinePlanLowerError::new(format!(
                    "wait duration must be a literal duration, found {}",
                    expr_label(expr)
                )));
                Vec::new()
            }
        }
        Stmt::Signal { target, value } => vec![LineEffectRequest::SignalWrite(RuntimeAssignment {
            target: expr_label(target),
            value: expr_label(value),
        })],
        Stmt::Expr(expr) => lower_expr_effect(expr, errors),
        Stmt::Command(command) => vec![LineEffectRequest::Command(RuntimeCommand {
            name: command.name().to_owned(),
            args: command.args().iter().map(expr_label).collect(),
        })],
        Stmt::Out { label, expr } => vec![LineEffectRequest::Out(LineOutRequest {
            label: label.clone(),
            value: expr_label(expr),
        })],
        Stmt::Return(expr) => vec![LineEffectRequest::Return(expr_label(expr))],
        Stmt::Goto(expr) => vec![LineEffectRequest::Goto(expr_label(expr))],
        Stmt::Yield(expr) => vec![LineEffectRequest::Yield(expr_label(expr))],
        Stmt::Panic(expr) => vec![LineEffectRequest::Panic(expr_label(expr))],
        Stmt::Fail(expr) => vec![LineEffectRequest::Fail(expr_label(expr))],
        Stmt::Bail(expr) => vec![LineEffectRequest::Bail(expr_label(expr))],
        Stmt::Ensure { condition, message } => vec![LineEffectRequest::Ensure {
            condition: expr_label(condition),
            message: expr_label(message),
        }],
        Stmt::Close(expr) => vec![LineEffectRequest::Close(expr_label(expr))],
        Stmt::Select(expr) => vec![LineEffectRequest::Select(expr_label(expr))],
        Stmt::Break { label, expr } => vec![LineEffectRequest::Break {
            label: label.clone(),
            value: expr.as_ref().map(expr_label),
        }],
        Stmt::Continue { label } => vec![LineEffectRequest::Continue {
            label: label.clone(),
        }],
        Stmt::DeferBlock { .. } | Stmt::Defer { .. } => {
            errors.push(LinePlanLowerError::new(
                "`defer` must be lowered through a scoped statement list".to_owned(),
            ));
            Vec::new()
        }
        Stmt::Raw(raw) => {
            errors.push(LinePlanLowerError::new(format!(
                "raw statement cannot be lowered: {raw}"
            )));
            Vec::new()
        }
        other => {
            errors.push(LinePlanLowerError::new(format!(
                "unsupported line-plan statement for runtime lowering: {other:?}"
            )));
            Vec::new()
        }
    }
}

fn lower_expr_effect(expr: &Expr, errors: &mut Vec<LinePlanLowerError>) -> Vec<LineEffectRequest> {
    if matches!(
        expr,
        Expr::Block { .. } | Expr::NamedBlock { .. } | Expr::ComputationBlock { .. }
    ) {
        let mut body = Vec::new();
        let mut defer_stack = Vec::new();
        append_expr_scope(expr, &mut body, &mut defer_stack, errors);
        body.extend(flatten_defer_stack(defer_stack));
        return body;
    }
    if let Expr::Pipe { lhs, rhs } = expr
        && is_drop_intrinsic(rhs)
    {
        return vec![LineEffectRequest::DropHandle {
            key: expr_label(lhs),
        }];
    }
    if matches!(
        expr,
        Expr::Call { .. } | Expr::MethodCall { .. } | Expr::Path(_)
    ) {
        return vec![runtime_call_effect(expr)];
    }
    errors.push(LinePlanLowerError::new(format!(
        "unsupported line-plan expression for runtime lowering: {}",
        expr_label(expr)
    )));
    Vec::new()
}

fn append_expr_scope(
    expr: &Expr,
    body: &mut Vec<LineEffectRequest>,
    defer_stack: &mut Vec<Vec<LineEffectRequest>>,
    errors: &mut Vec<LinePlanLowerError>,
) {
    match expr {
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => {
            let scope = lower_scoped_stmt_list(statements, errors);
            body.extend(scope.body);
            defer_stack.extend(scope.defer_stack);
            if let Some(value) = value {
                body.extend(lower_expr_effect(value, errors));
            }
        }
        other => body.extend(lower_expr_effect(other, errors)),
    }
}

fn runtime_fields(fields: &[(String, Expr)]) -> Vec<RuntimeField> {
    fields
        .iter()
        .map(|(name, value)| RuntimeField {
            name: name.clone(),
            value: expr_label(value),
        })
        .collect()
}

fn runtime_call(expr: &Expr) -> RuntimeCall {
    match expr {
        Expr::Call { callee, args } => RuntimeCall {
            callee: expr_label(callee),
            args: args.iter().map(expr_label).collect(),
        },
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => RuntimeCall {
            callee: format!("{}.{}", expr_label(receiver), method),
            args: args.iter().map(expr_label).collect(),
        },
        Expr::Path(path) => RuntimeCall {
            callee: path.clone(),
            args: Vec::new(),
        },
        other => RuntimeCall {
            callee: expr_label(other),
            args: Vec::new(),
        },
    }
}

fn runtime_call_effect(expr: &Expr) -> LineEffectRequest {
    let call = runtime_call(expr);
    if let Some(log) = runtime_log_call(&call) {
        return LineEffectRequest::Log(log);
    }
    if let Some(write) = runtime_assignment_call(&call, "signal.set") {
        return LineEffectRequest::SignalWrite(write);
    }
    if let Some(write) = runtime_assignment_call(&call, "metric.set") {
        return LineEffectRequest::MetricWrite(write);
    }
    if let Some(event) = runtime_event_call(&call) {
        return LineEffectRequest::EmitEvent(event);
    }
    LineEffectRequest::Call(call)
}

fn runtime_log_call(call: &RuntimeCall) -> Option<RuntimeLog> {
    let level = call.callee.strip_prefix("log.")?;
    let (message, rest) = call.args.split_first()?;
    Some(RuntimeLog {
        level: level.to_owned(),
        message: message.trim_matches('"').to_owned(),
        fields: rest
            .iter()
            .enumerate()
            .map(|(idx, value)| RuntimeField {
                name: named_arg_label(value).unwrap_or_else(|| format!("arg{idx}")),
                value: named_arg_value(value).unwrap_or_else(|| value.clone()),
            })
            .collect(),
    })
}

fn runtime_assignment_call(call: &RuntimeCall, callee: &str) -> Option<RuntimeAssignment> {
    if call.callee != callee || call.args.len() < 2 {
        return None;
    }
    Some(RuntimeAssignment {
        target: call.args[0].clone(),
        value: call.args[1].clone(),
    })
}

fn runtime_event_call(call: &RuntimeCall) -> Option<RuntimeEvent> {
    if call.callee != "event.emit" {
        return None;
    }
    let (event, rest) = call.args.split_first()?;
    Some(RuntimeEvent {
        event: event.clone(),
        fields: rest
            .iter()
            .enumerate()
            .map(|(idx, value)| RuntimeField {
                name: named_arg_label(value).unwrap_or_else(|| format!("arg{idx}")),
                value: named_arg_value(value).unwrap_or_else(|| value.clone()),
            })
            .collect(),
    })
}

fn named_arg_label(value: &str) -> Option<String> {
    value.split_once(" = ").map(|(name, _)| name.to_owned())
}

fn named_arg_value(value: &str) -> Option<String> {
    value.split_once(" = ").map(|(_, value)| value.to_owned())
}

fn pattern_label(pattern: &crate::ast::Pattern) -> String {
    format!("{pattern:?}")
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

fn duration_expr(expr: &Expr) -> Option<LogicalDuration> {
    let Expr::Literal(Literal::Duration { amount, unit }) = expr else {
        return None;
    };
    decimal_to_nanos(
        amount,
        match unit {
            DurationUnit::Millis => 1_000_000,
            DurationUnit::Seconds => 1_000_000_000,
        },
    )
    .map(LogicalDuration::from_nanos)
}

fn decimal_to_nanos(amount: &str, unit_nanos: u64) -> Option<u64> {
    let (whole, frac) = amount.split_once('.').unwrap_or((amount, ""));
    let whole_nanos = whole.parse::<u64>().ok()?.checked_mul(unit_nanos)?;
    if frac.is_empty() {
        return Some(whole_nanos);
    }
    let scale = 10_u64.checked_pow(u32::try_from(frac.len()).ok()?)?;
    let frac_nanos = frac.parse::<u64>().ok()?.checked_mul(unit_nanos)? / scale;
    whole_nanos.checked_add(frac_nanos)
}

fn expr_label(expr: &Expr) -> String {
    match expr {
        Expr::LifetimePath { key, optional } => {
            format!("'{}{}", key.as_dotted(), if *optional { "?" } else { "" })
        }
        Expr::Path(path) => path.clone(),
        Expr::EntityRef(entity) => format!("@{}", entity.body()),
        Expr::Literal(literal) => literal_label(literal),
        Expr::NamedArg { name, value } => format!("{name} = {}", expr_label(value)),
        Expr::Call { callee, args } => format!(
            "{}({})",
            expr_label(callee),
            args.iter().map(expr_label).collect::<Vec<_>>().join(", ")
        ),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => format!(
            "{}.{}({})",
            expr_label(receiver),
            method,
            args.iter().map(expr_label).collect::<Vec<_>>().join(", ")
        ),
        Expr::Field { target, field } => format!("{}.{}", expr_label(target), field),
        Expr::Pipe { lhs, rhs } => format!("{} |> {}", expr_label(lhs), expr_label(rhs)),
        other => format!("{other:?}"),
    }
}

fn literal_label(literal: &Literal) -> String {
    match literal {
        Literal::String(value) => format!("\"{value}\""),
        Literal::Int(value) => value.to_string(),
        Literal::Float(value) => value.clone(),
        Literal::Bool(value) => value.to_string(),
        Literal::Duration { amount, unit } => format!(
            "{amount}{}",
            match unit {
                DurationUnit::Millis => "ms",
                DurationUnit::Seconds => "s",
            }
        ),
    }
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

impl LinePlanLowerError {
    fn new(message: String) -> Self {
        Self { message }
    }

    /// Human-readable lowering diagnostic.
    pub fn message(&self) -> &str {
        &self.message
    }
}
