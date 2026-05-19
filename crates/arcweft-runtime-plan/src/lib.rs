pub mod errors;
pub mod expr;
pub mod labels;
pub mod line_task;
pub mod pattern;
pub mod source;
pub mod stream;

use crate::errors::{LinePlanLowerError, RuntimePlanLowerError};
use crate::expr::{lower_runtime_expr, lower_runtime_expr_strict, runtime_call_effect};
use crate::labels::{duration_expr, expr_label, pattern_label};
use crate::line_task::LoweredLineTaskGroup;
use crate::pattern::lower_runtime_pattern;
use crate::source::lower_source_plan;
use crate::stream::lower_stream_function;
use arcweft_core::effect::{
    ConflictPolicy, LineEffectRequest, ResourceAccess, ResourceAccessMode, RuntimeAssignment,
    RuntimeCommand, RuntimeField,
};
use arcweft_core::line_task::{
    ChildCancelPolicy, ChildJoinPolicy, LineAssertionRequest, LineBindingRequest,
    LineCancelRuleRequest, LineChildTask, LineMemoRequest, LineOptionRequest, LineOutRequest,
    LineTaskGroup, LineTaskNode, LineTaskScope, LineTaskTrigger, ParallelPolicy,
};
use arcweft_core::plan::{
    ChoiceRuntimeOption, FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeLineId, RuntimeMatchArm,
    RuntimePlan,
};
use arcweft_core::task::{AwaitTarget, NeedId, TaskId, TaskKey, TaskPriority};
use arcweft_core::value::{RuntimeExpr, RuntimeValue};
use arcweft_lang_hir::{
    HirAwait, HirChoice, HirChoiceOption, HirDialogue, HirFlowItem, HirLoop, HirMatch, HirModule,
    HirScopeExpr,
};
use arcweft_lang_syntax::Expr;
use arcweft_lang_syntax::{
    AwaitBranchKind, ChoiceAction, DeferOutcome, EntityRef, EntityRefSyntax, FlowItem,
    FunctionKind, LinePlan, LinePlanItem, Pattern, Stmt, TriggerPattern, WaitTarget,
};

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
    let stream_plans = module
        .functions()
        .iter()
        .filter(|function| function.kind() == FunctionKind::Stream)
        .map(lower_stream_function)
        .collect::<Vec<_>>();
    let source_plans = module
        .declarations()
        .iter()
        .filter_map(|decl| match decl {
            arcweft_lang_hir::HirTopLevelDecl::Source(source) => Some(lower_source_plan(source)),
            _ => None,
        })
        .collect::<Result<Vec<_>, _>>()?;
    RuntimePlan::new(entry, flows, lowerer.line_task_groups)
        .map(|plan| plan.with_generation_plans(stream_plans, source_plans))
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])
}

struct FlowRuntimeLowerer {
    line_task_groups: Vec<LineTaskGroup>,
    errors: Vec<RuntimePlanLowerError>,
}

impl FlowRuntimeLowerer {
    fn lower_runtime_expr(&mut self, expr: &Expr) -> RuntimeExpr {
        match lower_runtime_expr_strict(expr) {
            Ok(expr) => expr,
            Err(message) => {
                self.errors.push(RuntimePlanLowerError::new(message));
                RuntimeExpr::Value(RuntimeValue::Unit)
            }
        }
    }

    fn lower_optional_runtime_expr(&mut self, expr: Option<&Expr>) -> Option<RuntimeExpr> {
        expr.map(|expr| self.lower_runtime_expr(expr))
    }

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
                HirFlowItem::LetScope { pattern, scope } => {
                    ops.push(self.lower_scope_expr(pattern, scope));
                }
                HirFlowItem::If(block) => {
                    ops.push(FlowOp::If {
                        condition: self.lower_runtime_expr(block.condition()),
                        then_ops: self.lower_flow_items(flow_id, block.body(), flow_index),
                        else_ops: Vec::new(),
                    });
                }
                HirFlowItem::IfLet(block) => {
                    ops.push(FlowOp::IfLet {
                        pattern: lower_runtime_pattern(block.pattern()),
                        expr: self.lower_runtime_expr(block.expr()),
                        guard: self.lower_optional_runtime_expr(block.guard()),
                        then_ops: self.lower_flow_items(flow_id, block.body(), flow_index),
                        else_ops: Vec::new(),
                    });
                }
                HirFlowItem::Match(block) => {
                    ops.push(self.lower_match_block(flow_id, block, flow_index));
                }
                HirFlowItem::Loop(block) => {
                    ops.push(FlowOp::Loop {
                        body: self.lower_flow_items(flow_id, block.body(), flow_index),
                    });
                }
                HirFlowItem::LetLoop { pattern, block } => {
                    ops.push(self.lower_loop_expr(flow_id, pattern, block, flow_index));
                }
                HirFlowItem::While(block) => {
                    ops.push(FlowOp::While {
                        condition: self.lower_runtime_expr(block.condition()),
                        body: self.lower_flow_items(flow_id, block.body(), flow_index),
                    });
                }
                HirFlowItem::WhileLet(block) => {
                    ops.push(FlowOp::WhileLet {
                        pattern: lower_runtime_pattern(block.pattern()),
                        expr: self.lower_runtime_expr(block.expr()),
                        guard: self.lower_optional_runtime_expr(block.guard()),
                        body: self.lower_flow_items(flow_id, block.body(), flow_index),
                    });
                }
                HirFlowItem::For(block) => {
                    ops.push(FlowOp::For {
                        pattern: lower_runtime_pattern(block.pattern()),
                        source: self.lower_runtime_expr(block.source()),
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

    fn lower_scope_expr(&mut self, pattern: &Pattern, scope: &HirScopeExpr) -> FlowOp {
        FlowOp::LetScope {
            pattern: lower_runtime_pattern(pattern),
            ops: self.lower_flow_stmt_list(scope.statements()),
            value: scope
                .value()
                .map_or(RuntimeExpr::Value(RuntimeValue::Unit), |value| {
                    self.lower_runtime_expr(value)
                }),
        }
    }

    fn lower_match_block(
        &mut self,
        flow_id: &FlowRuntimeId,
        block: &HirMatch,
        flow_index: usize,
    ) -> FlowOp {
        FlowOp::Match {
            scrutinee: self.lower_runtime_expr(block.expr()),
            arms: block
                .arms()
                .iter()
                .map(|arm| RuntimeMatchArm {
                    pattern: lower_runtime_pattern(arm.pattern()),
                    guard: self.lower_optional_runtime_expr(arm.guard()),
                    ops: self.lower_flow_items(flow_id, arm.body(), flow_index),
                })
                .collect(),
        }
    }

    fn lower_loop_expr(
        &mut self,
        flow_id: &FlowRuntimeId,
        pattern: &Pattern,
        block: &HirLoop,
        flow_index: usize,
    ) -> FlowOp {
        FlowOp::LetLoop {
            pattern: lower_runtime_pattern(pattern),
            body: self.lower_flow_items(flow_id, block.body(), flow_index),
        }
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
                expr: self.lower_runtime_expr(expr),
            }],
            Stmt::LetScope { pattern, scope } => vec![FlowOp::LetScope {
                pattern: lower_runtime_pattern(pattern),
                ops: self.lower_flow_stmt_list(scope.statements()),
                value: scope
                    .value()
                    .map_or(RuntimeExpr::Value(RuntimeValue::Unit), |value| {
                        self.lower_runtime_expr(value)
                    }),
            }],
            Stmt::LetLoop { pattern, block } => vec![FlowOp::LetLoop {
                pattern: lower_runtime_pattern(pattern),
                body: self.lower_syntax_flow_items(block.body()),
            }],
            Stmt::LetElse {
                pattern,
                expr,
                else_body,
                ..
            } => vec![FlowOp::LetElse {
                pattern: lower_runtime_pattern(pattern),
                expr: self.lower_runtime_expr(expr),
                else_ops: self.lower_flow_stmt_list(else_body),
            }],
            Stmt::Goto(expr) => vec![FlowOp::GotoExpr(self.lower_runtime_expr(expr))],
            Stmt::Return(expr) => vec![FlowOp::ReturnExpr(self.lower_runtime_expr(expr))],
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
                condition: self.lower_runtime_expr(condition),
                then_ops: self.lower_flow_stmt_list(body),
                else_ops: Vec::new(),
            }],
            Stmt::Loop { body } => vec![FlowOp::Loop {
                body: self.lower_flow_stmt_list(body),
            }],
            Stmt::While { condition, body } => vec![FlowOp::While {
                condition: self.lower_runtime_expr(condition),
                body: self.lower_flow_stmt_list(body),
            }],
            Stmt::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => vec![FlowOp::WhileLet {
                pattern: lower_runtime_pattern(pattern),
                expr: self.lower_runtime_expr(expr),
                guard: self.lower_optional_runtime_expr(guard.as_ref()),
                body: self.lower_flow_stmt_list(body),
            }],
            Stmt::For {
                pattern,
                source,
                body,
            } => vec![FlowOp::For {
                pattern: lower_runtime_pattern(pattern),
                source: self.lower_runtime_expr(source),
                body: self.lower_flow_stmt_list(body),
            }],
            Stmt::Match { expr, arms } => vec![FlowOp::Match {
                scrutinee: self.lower_runtime_expr(expr),
                arms: arms
                    .iter()
                    .map(|arm| RuntimeMatchArm {
                        pattern: lower_runtime_pattern(arm.pattern()),
                        guard: self.lower_optional_runtime_expr(arm.guard()),
                        ops: self.lower_flow_stmt_list(arm.body()),
                    })
                    .collect(),
            }],
            Stmt::Break { expr, .. } => {
                vec![FlowOp::Break(
                    self.lower_optional_runtime_expr(expr.as_ref()),
                )]
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

    fn lower_syntax_flow_items(&mut self, items: &[FlowItem]) -> Vec<FlowOp> {
        items
            .iter()
            .flat_map(|item| match item {
                FlowItem::Stmt(statement) => self.lower_flow_stmt(statement),
                other => {
                    self.errors.push(RuntimePlanLowerError::new(format!(
                        "unsupported nested flow item for runtime lowering: {other:?}"
                    )));
                    Vec::new()
                }
            })
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
            Stmt::Yield(_) => {
                self.errors.push(LinePlanLowerError::new(
                    "`yield` cannot be lowered from a dialogue line plan; use `out` for line results"
                        .to_owned(),
                ));
                Vec::new()
            }
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

fn runtime_fields(fields: &[(String, Expr)]) -> Vec<RuntimeField> {
    fields
        .iter()
        .map(|(name, value)| RuntimeField {
            name: name.clone(),
            value: expr_label(value),
        })
        .collect()
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
