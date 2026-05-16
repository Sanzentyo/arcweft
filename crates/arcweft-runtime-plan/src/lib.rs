use arcweft_core::{
    AwaitTarget, ChildCancelPolicy, ChildJoinPolicy, ChoiceRuntimeOption, ConflictPolicy, FlowOp,
    FlowRuntimeId, LineAssertionRequest, LineBindingRequest, LineCancelRuleRequest, LineChildTask,
    LineEffectRequest, LineMemoRequest, LineOptionRequest, LineOutRequest, LineTaskGroup,
    LineTaskNode, LineTaskScope, LineTaskTrigger, LogicalDuration, NeedId, ParallelPolicy,
    ResourceAccess, ResourceAccessMode, RuntimeAssignment, RuntimeBinaryOp, RuntimeCall,
    RuntimeCommand, RuntimeEvent, RuntimeExpr, RuntimeExprMatchArm, RuntimeField, RuntimeFieldExpr,
    RuntimeFlow, RuntimeLineId, RuntimeLog, RuntimeMatchArm, RuntimePattern, RuntimePlan,
    RuntimeRecordPatternField, RuntimeUnaryOp, RuntimeValue, TaskId, TaskKey, TaskPriority,
};
use arcweft_lang_hir::{HirAwait, HirChoice, HirChoiceOption, HirDialogue, HirFlowItem, HirModule};
use arcweft_lang_syntax::{
    AwaitBranchKind, ChoiceAction, DeferOutcome, EntityRef, EntityRefSyntax, LinePlan,
    LinePlanItem, Pattern, Stmt, TriggerPattern, WaitTarget,
};
use arcweft_lang_syntax::{BinaryOp, DurationUnit, Expr, Literal, UnaryOp};
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

/// Error produced while converting HIR flows to the executable runtime plan.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct RuntimePlanLowerError {
    message: String,
}

/// Lowers checked HIR flows to the Sans I/O core runtime program.
///
/// This pass is intentionally stricter than `lower_line_task_groups`: it must
/// not silently skip flow syntax because the engine would otherwise execute a
/// different story than the source describes.
pub fn lower_runtime_plan(module: &HirModule) -> Result<RuntimePlan, Vec<RuntimePlanLowerError>> {
    let mut lowerer = FlowRuntimeLowerer {
        line_task_groups: Vec::new(),
        errors: Vec::new(),
    };
    let flows = module
        .flows()
        .iter()
        .enumerate()
        .map(|(index, flow)| lowerer.lower_flow(index, flow))
        .collect::<Vec<_>>();
    if !module.top_level_items().is_empty() {
        lowerer.errors.push(RuntimePlanLowerError::new(
            "top-level flow items are not executable by the flow runtime yet",
        ));
    }
    if !lowerer.errors.is_empty() {
        return Err(lowerer.errors);
    }
    let entry = flows.first().map(|flow| flow.id.clone());
    RuntimePlan::new(entry, flows, lowerer.line_task_groups)
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])
}

struct FlowRuntimeLowerer {
    line_task_groups: Vec<LineTaskGroup>,
    errors: Vec<RuntimePlanLowerError>,
}

impl FlowRuntimeLowerer {
    fn lower_flow(&mut self, index: usize, flow: &arcweft_lang_hir::HirFlow) -> RuntimeFlow {
        let id = flow.id().map_or_else(
            || FlowRuntimeId(format!("flow.{}", flow.name().unwrap_or("anonymous"))),
            flow_runtime_id,
        );
        let ops = self.lower_flow_items(&id, flow.body(), index);
        RuntimeFlow { id, ops }
    }

    fn lower_flow_items(
        &mut self,
        flow_id: &FlowRuntimeId,
        items: &[HirFlowItem],
        flow_index: usize,
    ) -> Vec<FlowOp> {
        let mut ops = Vec::new();
        for item in items {
            match item {
                HirFlowItem::Dialogue(dialogue) => {
                    ops.push(self.lower_runtime_dialogue(flow_id, flow_index, dialogue));
                }
                HirFlowItem::Choice(choice) | HirFlowItem::LetChoice { choice, .. } => {
                    ops.push(self.lower_choice(choice));
                }
                HirFlowItem::Await(await_with) | HirFlowItem::LetAwait { await_with, .. } => {
                    ops.push(self.lower_await(await_with));
                }
                HirFlowItem::Stmt(stmt) => {
                    ops.extend(self.lower_flow_stmt(stmt));
                }
                HirFlowItem::Scope(scope) => {
                    ops.push(FlowOp::Scope(self.lower_flow_items(
                        flow_id,
                        scope.body(),
                        flow_index,
                    )));
                }
                HirFlowItem::If(block) => {
                    ops.push(FlowOp::If {
                        condition: lower_runtime_expr(block.condition()),
                        then_ops: self.lower_flow_items(flow_id, block.body(), flow_index),
                        else_ops: Vec::new(),
                    });
                }
                HirFlowItem::IfLet(block) => {
                    ops.push(FlowOp::IfLet {
                        pattern: lower_runtime_pattern(block.pattern()),
                        expr: lower_runtime_expr(block.expr()),
                        guard: block.guard().map(lower_runtime_expr),
                        then_ops: self.lower_flow_items(flow_id, block.body(), flow_index),
                        else_ops: Vec::new(),
                    });
                }
                HirFlowItem::Match(block) => {
                    let arms = block
                        .arms()
                        .iter()
                        .map(|arm| RuntimeMatchArm {
                            pattern: lower_runtime_pattern(arm.pattern()),
                            guard: arm.guard().map(lower_runtime_expr),
                            ops: self.lower_flow_items(flow_id, arm.body(), flow_index),
                        })
                        .collect();
                    ops.push(FlowOp::Match {
                        scrutinee: lower_runtime_expr(block.expr()),
                        arms,
                    });
                }
                HirFlowItem::Loop(block) => {
                    ops.push(FlowOp::Loop {
                        body: self.lower_flow_items(flow_id, block.body(), flow_index),
                    });
                }
                HirFlowItem::While(block) => {
                    ops.push(FlowOp::While {
                        condition: lower_runtime_expr(block.condition()),
                        body: self.lower_flow_items(flow_id, block.body(), flow_index),
                    });
                }
                HirFlowItem::WhileLet(block) => {
                    ops.push(FlowOp::WhileLet {
                        pattern: lower_runtime_pattern(block.pattern()),
                        expr: lower_runtime_expr(block.expr()),
                        guard: block.guard().map(lower_runtime_expr),
                        body: self.lower_flow_items(flow_id, block.body(), flow_index),
                    });
                }
                HirFlowItem::For(block) => {
                    ops.push(FlowOp::For {
                        pattern: lower_runtime_pattern(block.pattern()),
                        source: lower_runtime_expr(block.source()),
                        body: self.lower_flow_items(flow_id, block.body(), flow_index),
                    });
                }
                HirFlowItem::Scenario { name, args } => {
                    ops.push(FlowOp::Effect(LineEffectRequest::Command(RuntimeCommand {
                        name: name.clone(),
                        args: args.iter().map(expr_label).collect(),
                    })));
                }
                other => {
                    self.errors.push(RuntimePlanLowerError::new(format!(
                        "unsupported flow item for runtime lowering: {other:?}"
                    )));
                }
            }
        }
        ops
    }

    fn lower_runtime_dialogue(
        &mut self,
        flow_id: &FlowRuntimeId,
        flow_index: usize,
        dialogue: &HirDialogue,
    ) -> FlowOp {
        let group = if let Some(plan) = dialogue.plan() {
            match lower_line_plan(plan) {
                Ok(group) => group,
                Err(errors) => {
                    self.errors.extend(
                        errors
                            .into_iter()
                            .map(|error| RuntimePlanLowerError::new(error.message().to_owned())),
                    );
                    LineTaskGroup::default()
                }
            }
        } else {
            LineTaskGroup::default()
        };
        let task_group = self.line_task_groups.len();
        self.line_task_groups.push(group);
        let line = dialogue.id().map_or_else(
            || RuntimeLineId(format!("{}.line.{task_group}", flow_id.0)),
            |id| RuntimeLineId(id.body().to_owned()),
        );
        let _ = flow_index;
        FlowOp::Dialogue { line, task_group }
    }

    fn lower_choice(&mut self, choice: &HirChoice) -> FlowOp {
        FlowOp::Choice {
            id: choice.id().map(|id| id.body().to_owned()),
            options: choice
                .options()
                .iter()
                .map(|option| self.lower_choice_option(option))
                .collect(),
        }
    }

    fn lower_choice_option(&mut self, option: &HirChoiceOption) -> ChoiceRuntimeOption {
        let mut effects = Vec::new();
        let mut out = None;
        let mut target = option.target().map(flow_runtime_id);
        match option.action() {
            ChoiceAction::Goto(target_ref) => {
                if let EntityRefSyntax::Absolute(target_ref) = target_ref {
                    target = Some(flow_runtime_id(target_ref));
                }
            }
            ChoiceAction::Out(expr) => {
                out = Some(LineOutRequest {
                    label: None,
                    value: expr_label(expr),
                });
            }
            ChoiceAction::SelectBlock(statements) => {
                effects.extend(self.lower_flow_statements(statements));
            }
            ChoiceAction::None => {}
        }
        ChoiceRuntimeOption {
            id: option.id().map(|id| id.body().to_owned()),
            label: option.label().to_owned(),
            target,
            out,
            effects,
        }
    }

    fn lower_await(&mut self, await_with: &HirAwait) -> FlowOp {
        let label = expr_label(await_with.expr());
        let task_name = sanitize_task_id_part(&label);
        let pending = await_with
            .branches()
            .iter()
            .filter(|branch| branch.kind() == AwaitBranchKind::Pending)
            .flat_map(|branch| self.lower_pending_flow_items(branch.body()))
            .collect();
        FlowOp::Await {
            target: AwaitTarget {
                need: NeedId(format!("need.await.{task_name}")),
                task: TaskId(format!("task.await.{task_name}")),
            },
            pending,
        }
    }

    fn lower_pending_flow_items(&mut self, items: &[HirFlowItem]) -> Vec<LineEffectRequest> {
        items
            .iter()
            .flat_map(|item| match item {
                HirFlowItem::Stmt(stmt) => self.lower_flow_statements(std::slice::from_ref(stmt)),
                HirFlowItem::Scenario { name, args } => {
                    vec![LineEffectRequest::Command(RuntimeCommand {
                        name: name.clone(),
                        args: args.iter().map(expr_label).collect(),
                    })]
                }
                other => {
                    self.errors.push(RuntimePlanLowerError::new(format!(
                        "unsupported await pending item for runtime lowering: {other:?}"
                    )));
                    Vec::new()
                }
            })
            .collect()
    }

    fn lower_flow_stmt(&mut self, stmt: &Stmt) -> Vec<FlowOp> {
        match stmt {
            Stmt::Let { pattern, expr, .. } => vec![FlowOp::Let {
                pattern: lower_runtime_pattern(pattern),
                expr: lower_runtime_expr(expr),
            }],
            Stmt::LetElse {
                pattern,
                expr,
                else_body,
                ..
            } => vec![FlowOp::LetElse {
                pattern: lower_runtime_pattern(pattern),
                expr: lower_runtime_expr(expr),
                else_ops: self.lower_flow_stmt_list(else_body),
            }],
            Stmt::Goto(expr) => vec![FlowOp::GotoExpr(lower_runtime_expr(expr))],
            Stmt::Return(expr) => vec![FlowOp::ReturnExpr(lower_runtime_expr(expr))],
            Stmt::Expr(expr) => vec![FlowOp::Effect(runtime_call_effect(expr))],
            Stmt::Out { label, expr } => {
                vec![FlowOp::Effect(LineEffectRequest::Out(LineOutRequest {
                    label: label.clone(),
                    value: expr_label(expr),
                }))]
            }
            Stmt::Command(command) => {
                vec![FlowOp::Effect(LineEffectRequest::Command(RuntimeCommand {
                    name: command.name().to_owned(),
                    args: command.args().iter().map(expr_label).collect(),
                }))]
            }
            Stmt::If { condition, body } => vec![FlowOp::If {
                condition: lower_runtime_expr(condition),
                then_ops: self.lower_flow_stmt_list(body),
                else_ops: Vec::new(),
            }],
            Stmt::Loop { body } => vec![FlowOp::Loop {
                body: self.lower_flow_stmt_list(body),
            }],
            Stmt::While { condition, body } => vec![FlowOp::While {
                condition: lower_runtime_expr(condition),
                body: self.lower_flow_stmt_list(body),
            }],
            Stmt::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => vec![FlowOp::WhileLet {
                pattern: lower_runtime_pattern(pattern),
                expr: lower_runtime_expr(expr),
                guard: guard.as_ref().map(lower_runtime_expr),
                body: self.lower_flow_stmt_list(body),
            }],
            Stmt::For {
                pattern,
                source,
                body,
            } => vec![FlowOp::For {
                pattern: lower_runtime_pattern(pattern),
                source: lower_runtime_expr(source),
                body: self.lower_flow_stmt_list(body),
            }],
            Stmt::Match { expr, arms } => vec![FlowOp::Match {
                scrutinee: lower_runtime_expr(expr),
                arms: arms
                    .iter()
                    .map(|arm| RuntimeMatchArm {
                        pattern: lower_runtime_pattern(arm.pattern()),
                        guard: arm.guard().map(lower_runtime_expr),
                        ops: self.lower_flow_stmt_list(arm.body()),
                    })
                    .collect(),
            }],
            Stmt::Break { expr, .. } => {
                vec![FlowOp::Break(expr.as_ref().map(lower_runtime_expr))]
            }
            Stmt::Continue { .. } => vec![FlowOp::Continue],
            other => {
                self.errors.push(RuntimePlanLowerError::new(format!(
                    "unsupported flow statement for runtime lowering: {other:?}"
                )));
                Vec::new()
            }
        }
    }

    fn lower_flow_stmt_list(&mut self, statements: &[Stmt]) -> Vec<FlowOp> {
        statements
            .iter()
            .flat_map(|statement| self.lower_flow_stmt(statement))
            .collect()
    }

    fn lower_flow_statements(&mut self, statements: &[Stmt]) -> Vec<LineEffectRequest> {
        let mut line_lowerer = LinePlanGraphLowerer::default();
        let effects = statements
            .iter()
            .flat_map(|statement| line_lowerer.lower_stmt(statement))
            .collect::<Vec<_>>();
        self.errors.extend(
            line_lowerer
                .errors
                .into_iter()
                .map(|error| RuntimePlanLowerError::new(error.message().to_owned())),
        );
        effects
    }
}

impl RuntimePlanLowerError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Human-readable runtime lowering diagnostic.
    pub fn message(&self) -> &str {
        &self.message
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
                vec![LineTaskNode::Child(self.lower_child_task(
                    thread.name(),
                    LineTaskTrigger::Immediate,
                    thread.body(),
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
            LinePlanItem::Expr(expr) => effect_nodes(self.lower_expr_effect(expr)),
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
                Vec::new()
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
            LinePlanItem::Memo { name, options } => {
                group.memo.push(LineMemoRequest {
                    name: name.clone(),
                    options: runtime_fields(options),
                });
                Vec::new()
            }
            LinePlanItem::Assert { debug, expr } => {
                group.assertions.push(LineAssertionRequest {
                    debug: *debug,
                    expr: expr_label(expr),
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
                push_defer_block(*outcome, self.lower_expr_effect(expr), scope);
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
                    let effects = self.lower_expr_effect(expr);
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
                key: expr_label(target),
                handle: expr_label(expr),
            }],
            Stmt::Wait(WaitTarget::Mark(mark)) => vec![LineEffectRequest::WaitMark(mark.clone())],
            Stmt::Wait(WaitTarget::Duration(expr)) => {
                if let Some(duration) = duration_expr(expr) {
                    vec![LineEffectRequest::Wait(duration)]
                } else {
                    self.errors.push(LinePlanLowerError::new(format!(
                        "wait duration must be a literal duration, found {}",
                        expr_label(expr)
                    )));
                    Vec::new()
                }
            }
            Stmt::Signal { target, value } => {
                vec![LineEffectRequest::SignalWrite(RuntimeAssignment {
                    target: expr_label(target),
                    value: expr_label(value),
                })]
            }
            Stmt::Expr(expr) => self.lower_expr_effect(expr),
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
                self.errors.push(LinePlanLowerError::new(
                    "`defer` must be lowered through a scoped statement list".to_owned(),
                ));
                Vec::new()
            }
            Stmt::Raw(raw) => {
                self.errors.push(LinePlanLowerError::new(format!(
                    "raw statement cannot be lowered: {raw}"
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
        if matches!(
            expr,
            Expr::Call { .. } | Expr::MethodCall { .. } | Expr::Path(_)
        ) {
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

fn sanitize_task_id_part(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn flow_runtime_id(id: &EntityRef) -> FlowRuntimeId {
    FlowRuntimeId(id.body().to_owned())
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
        LineEffectRequest::Out(_) => vec![resource_access(
            "line:out".to_owned(),
            ResourceAccessMode::Write,
            ConflictPolicy::Error,
        )],
        LineEffectRequest::Return(_)
        | LineEffectRequest::Goto(_)
        | LineEffectRequest::Yield(_)
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
        LineEffectRequest::WaitMark(_)
        | LineEffectRequest::Wait(_)
        | LineEffectRequest::Call(_)
        | LineEffectRequest::Command(_)
        | LineEffectRequest::Ensure { .. } => Vec::new(),
    }
}

fn resource_access(
    key: String,
    mode: ResourceAccessMode,
    policy: ConflictPolicy,
) -> ResourceAccess {
    ResourceAccess { key, mode, policy }
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

fn lower_runtime_expr(expr: &Expr) -> RuntimeExpr {
    match expr {
        Expr::Literal(literal) => RuntimeExpr::Value(lower_runtime_literal(literal)),
        Expr::EntityRef(entity) => RuntimeExpr::EntityRef(entity.body().to_owned()),
        Expr::Path(path) => RuntimeExpr::Local(path.clone()),
        Expr::Tuple(items) => RuntimeExpr::Tuple(items.iter().map(lower_runtime_expr).collect()),
        Expr::List(items) => RuntimeExpr::List(items.iter().map(lower_runtime_expr).collect()),
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => RuntimeExpr::Record(
            fields
                .iter()
                .map(|(name, value)| RuntimeFieldExpr {
                    name: name.clone(),
                    value: lower_runtime_expr(value),
                })
                .collect(),
        ),
        Expr::Field { target, field } => RuntimeExpr::Field {
            target: Box::new(lower_runtime_expr(target)),
            field: field.clone(),
        },
        Expr::Unary { op, expr } => RuntimeExpr::Unary {
            op: lower_runtime_unary_op(*op),
            expr: Box::new(lower_runtime_expr(expr)),
        },
        Expr::Binary { lhs, op, rhs } => {
            if let Some(op) = lower_runtime_binary_op(*op) {
                RuntimeExpr::Binary {
                    lhs: Box::new(lower_runtime_expr(lhs)),
                    op,
                    rhs: Box::new(lower_runtime_expr(rhs)),
                }
            } else {
                RuntimeExpr::Value(RuntimeValue::String(expr_label(expr)))
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => RuntimeExpr::If {
            condition: Box::new(lower_runtime_expr(condition)),
            then_expr: Box::new(lower_runtime_expr(then_branch)),
            else_expr: Box::new(
                else_branch
                    .as_deref()
                    .map_or(RuntimeExpr::Value(RuntimeValue::Unit), lower_runtime_expr),
            ),
        },
        Expr::Match { scrutinee, arms } => RuntimeExpr::Match {
            scrutinee: Box::new(lower_runtime_expr(scrutinee)),
            arms: arms
                .iter()
                .map(|arm| RuntimeExprMatchArm {
                    pattern: lower_runtime_pattern(arm.pattern()),
                    guard: arm.guard().map(lower_runtime_expr),
                    value: lower_runtime_expr(arm.value()),
                })
                .collect(),
        },
        Expr::Call { .. } | Expr::MethodCall { .. } => {
            RuntimeExpr::Value(RuntimeValue::String(expr_label(expr)))
        }
        Expr::NamedArg { value, .. } => lower_runtime_expr(value),
        Expr::Try { expr }
        | Expr::Await { expr, .. }
        | Expr::Index { target: expr, .. }
        | Expr::Pipe { lhs: expr, .. } => lower_runtime_expr(expr),
        _ => RuntimeExpr::Value(RuntimeValue::String(expr_label(expr))),
    }
}

fn lower_runtime_literal(literal: &Literal) -> RuntimeValue {
    match literal {
        Literal::String(value) => RuntimeValue::String(value.clone()),
        Literal::Char { value, .. } => RuntimeValue::Char(*value),
        Literal::Int(value) => RuntimeValue::Int(*value),
        Literal::Float(value) => RuntimeValue::Float(value.clone()),
        Literal::Bool(value) => RuntimeValue::Bool(*value),
        Literal::Duration { .. } => duration_expr(&Expr::Literal(literal.clone())).map_or_else(
            || RuntimeValue::String(literal_label(literal)),
            RuntimeValue::Duration,
        ),
    }
}

fn lower_runtime_pattern(pattern: &Pattern) -> RuntimePattern {
    match pattern {
        Pattern::Ident(name) => RuntimePattern::Ident(name.clone()),
        Pattern::MutIdent(name) => RuntimePattern::MutIdent(name.clone()),
        Pattern::Discard => RuntimePattern::Discard,
        Pattern::Literal(expr) => match lower_runtime_expr(expr) {
            RuntimeExpr::Value(value) => RuntimePattern::Literal(value),
            RuntimeExpr::EntityRef(entity) => RuntimePattern::Entity(entity),
            _ => RuntimePattern::Literal(RuntimeValue::String(expr_label(expr))),
        },
        Pattern::Entity(entity) => RuntimePattern::Entity(entity.body().to_owned()),
        Pattern::Tuple(items) => {
            RuntimePattern::Tuple(items.iter().map(lower_runtime_pattern).collect())
        }
        Pattern::Record { path, fields, rest } => RuntimePattern::Record {
            path: path.clone(),
            fields: fields
                .iter()
                .map(|field| RuntimeRecordPatternField {
                    name: field.name().to_owned(),
                    pattern: lower_runtime_pattern(field.pattern()),
                })
                .collect(),
            rest: *rest,
        },
        Pattern::List { items, rest } => RuntimePattern::List {
            items: items.iter().map(lower_runtime_pattern).collect(),
            rest: rest.clone(),
        },
        Pattern::Variant {
            path,
            name,
            payload,
        } => RuntimePattern::Variant {
            path: path.clone(),
            name: name.clone(),
            payload: payload
                .as_ref()
                .map(|payload| Box::new(lower_runtime_variant_payload(payload))),
        },
        Pattern::Whole { name, pattern } => RuntimePattern::Whole {
            name: name.clone(),
            pattern: Box::new(lower_runtime_pattern(pattern)),
        },
        Pattern::Typed { name, ty } => RuntimePattern::Typed {
            name: name.clone(),
            ty: format!("{ty:?}"),
        },
        Pattern::Raw(raw) => RuntimePattern::Literal(RuntimeValue::String(raw.clone())),
    }
}

fn lower_runtime_variant_payload(
    payload: &arcweft_lang_syntax::VariantPatternPayload,
) -> RuntimePattern {
    match payload {
        arcweft_lang_syntax::VariantPatternPayload::Tuple(items) => {
            RuntimePattern::Tuple(items.iter().map(lower_runtime_pattern).collect())
        }
        arcweft_lang_syntax::VariantPatternPayload::Record { fields, rest } => {
            RuntimePattern::Record {
                path: None,
                fields: fields
                    .iter()
                    .map(|field| RuntimeRecordPatternField {
                        name: field.name().to_owned(),
                        pattern: lower_runtime_pattern(field.pattern()),
                    })
                    .collect(),
                rest: *rest,
            }
        }
    }
}

fn lower_runtime_unary_op(op: UnaryOp) -> RuntimeUnaryOp {
    match op {
        UnaryOp::Not => RuntimeUnaryOp::Not,
        UnaryOp::Neg => RuntimeUnaryOp::Neg,
    }
}

fn lower_runtime_binary_op(op: BinaryOp) -> Option<RuntimeBinaryOp> {
    Some(match op {
        BinaryOp::Eq => RuntimeBinaryOp::Eq,
        BinaryOp::NotEq => RuntimeBinaryOp::Ne,
        BinaryOp::Lt => RuntimeBinaryOp::Lt,
        BinaryOp::Lte => RuntimeBinaryOp::Le,
        BinaryOp::Gt => RuntimeBinaryOp::Gt,
        BinaryOp::Gte => RuntimeBinaryOp::Ge,
        BinaryOp::Add => RuntimeBinaryOp::Add,
        BinaryOp::Sub => RuntimeBinaryOp::Sub,
        BinaryOp::Mul => RuntimeBinaryOp::Mul,
        BinaryOp::Div => RuntimeBinaryOp::Div,
        BinaryOp::And => RuntimeBinaryOp::And,
        BinaryOp::Or => RuntimeBinaryOp::Or,
        BinaryOp::Implies | BinaryOp::In | BinaryOp::Merge | BinaryOp::Rem => return None,
    })
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

fn pattern_label(pattern: &arcweft_lang_syntax::Pattern) -> String {
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
        Literal::Char { raw, .. } => raw.clone(),
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
