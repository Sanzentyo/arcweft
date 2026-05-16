use crate::ast::{EntityRef, LinePlan, LinePlanItem, Stmt, WaitTarget};
use crate::expr::{DurationUnit, Expr, Literal};
use crate::lower::{HirDialogue, HirFlowItem, HirModule};
use arcweft_core::{LineChildTask, LineEffectRequest, LineTaskGroup, LogicalDuration};
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
        match item {
            LinePlanItem::Init(statements) => {
                let scope = lower_scoped_stmt_list(statements, &mut errors);
                group.init.extend(scope.body);
                group.init_defer_stack.extend(scope.defer_stack);
            }
            LinePlanItem::Thread(thread) => {
                let task = lower_child_task(thread.name(), thread.body(), &mut errors);
                group.children.push(task);
            }
            LinePlanItem::On { trigger, body } => {
                let mut task = lower_child_task(Some(&expr_label(trigger)), body, &mut errors);
                if let Expr::Path(name) = trigger {
                    task.body
                        .insert(0, LineEffectRequest::WaitMark(name.clone()));
                }
                group.children.push(task);
            }
            LinePlanItem::Finally(statements) => {
                let scope = lower_scoped_stmt_list(statements, &mut errors);
                group.finally.extend(scope.body);
                group.finally.extend(flatten_defer_stack(scope.defer_stack));
            }
            LinePlanItem::Stmt(stmt) => {
                lower_line_scope_stmt(stmt, &mut group, &mut errors);
            }
            LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
                lower_group_items(items, &mut group, &mut errors);
            }
            LinePlanItem::Raw(raw) => errors.push(LinePlanLowerError::new(format!(
                "raw line-plan item cannot be lowered: {raw}"
            ))),
            _ => {}
        }
    }
    if errors.is_empty() {
        Ok(group)
    } else {
        Err(errors)
    }
}

fn lower_group_items(
    items: &[LinePlanItem],
    group: &mut LineTaskGroup,
    errors: &mut Vec<LinePlanLowerError>,
) {
    for item in items {
        match item {
            LinePlanItem::Thread(thread) => {
                group
                    .children
                    .push(lower_child_task(thread.name(), thread.body(), errors));
            }
            LinePlanItem::Finally(statements) => {
                let scope = lower_scoped_stmt_list(statements, errors);
                group.finally.extend(scope.body);
                group.finally.extend(flatten_defer_stack(scope.defer_stack));
            }
            LinePlanItem::Stmt(stmt) => lower_line_scope_stmt(stmt, group, errors),
            LinePlanItem::Raw(raw) => errors.push(LinePlanLowerError::new(format!(
                "raw grouped line-plan item cannot be lowered: {raw}"
            ))),
            _ => {}
        }
    }
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

fn lower_line_scope_stmt(
    stmt: &Stmt,
    group: &mut LineTaskGroup,
    errors: &mut Vec<LinePlanLowerError>,
) {
    match stmt {
        Stmt::DeferBlock(statements) => {
            group
                .defer_stack
                .push(lower_cleanup_block(statements, errors));
        }
        Stmt::Defer(expr) => {
            group.defer_stack.push(lower_expr_effect(expr, errors));
        }
        other => group.init.extend(lower_stmt(other, errors)),
    }
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
            Stmt::DeferBlock(statements) => {
                scope
                    .defer_stack
                    .push(lower_cleanup_block(statements, errors));
            }
            Stmt::Defer(expr) => {
                scope.defer_stack.push(lower_expr_effect(expr, errors));
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
        Stmt::Expr(expr) => lower_expr_effect(expr, errors),
        Stmt::Command(command) => vec![LineEffectRequest::EmitSignal(command.name().to_owned())],
        Stmt::DeferBlock(_) | Stmt::Defer(_) => {
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
        return vec![LineEffectRequest::EmitSignal(expr_label(expr))];
    }
    errors.push(LinePlanLowerError::new(format!(
        "unsupported line-plan expression for runtime lowering: {}",
        expr_label(expr)
    )));
    Vec::new()
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
