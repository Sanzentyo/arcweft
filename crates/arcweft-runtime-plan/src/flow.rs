//! Flow-runtime lowering.

use crate::errors::{LinePlanLowerError, RuntimePlanLowerError};
use crate::expr::{lower_runtime_expr_strict, runtime_call_effect};
use crate::labels::expr_label;
use crate::pattern::lower_runtime_pattern;
use crate::{lower_line_plan, lower_line_plan_statements};
use arcweft_core::effect::{LineEffectRequest, RuntimeCommand};
use arcweft_core::line_task::{LineOutRequest, LineTaskGroup};
use arcweft_core::plan::{
    ChoiceRuntimeOption, FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeLineId, RuntimeMatchArm,
};
use arcweft_core::task::{AwaitTarget, NeedId, TaskId};
use arcweft_core::value::{RuntimeExpr, RuntimeValue};
use arcweft_lang_hir::model::{
    HirAwait, HirChoice, HirChoiceOption, HirDialogue, HirFlow, HirFlowItem, HirLoop, HirMatch,
    HirModule, HirScopeExpr,
};
use arcweft_lang_syntax::{
    AwaitBranchKind, ChoiceAction, EntityRef, EntityRefSyntax, FlowItem, Pattern, Stmt,
};

pub(crate) struct LoweredRuntimeFlows {
    pub(crate) flows: Vec<RuntimeFlow>,
    pub(crate) line_task_groups: Vec<LineTaskGroup>,
}

/// Lowers HIR flow bodies into executable Sans I/O flow operations.
pub(crate) fn lower_runtime_flows(
    module: &HirModule,
) -> Result<LoweredRuntimeFlows, Vec<RuntimePlanLowerError>> {
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
    if lowerer.errors.is_empty() {
        Ok(LoweredRuntimeFlows {
            flows,
            line_task_groups: lowerer.line_task_groups,
        })
    } else {
        Err(lowerer.errors)
    }
}

struct FlowRuntimeLowerer {
    line_task_groups: Vec<LineTaskGroup>,
    errors: Vec<RuntimePlanLowerError>,
}

impl FlowRuntimeLowerer {
    fn lower_runtime_expr(&mut self, expr: &arcweft_lang_syntax::Expr) -> RuntimeExpr {
        match lower_runtime_expr_strict(expr) {
            Ok(expr) => expr,
            Err(message) => {
                self.errors.push(RuntimePlanLowerError::new(message));
                RuntimeExpr::Value(RuntimeValue::Unit)
            }
        }
    }

    fn lower_optional_runtime_expr(
        &mut self,
        expr: Option<&arcweft_lang_syntax::Expr>,
    ) -> Option<RuntimeExpr> {
        expr.map(|expr| self.lower_runtime_expr(expr))
    }

    fn lower_flow(&mut self, index: usize, flow: &HirFlow) -> RuntimeFlow {
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
                    self.push_line_errors(errors);
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
        let (effects, errors) = lower_line_plan_statements(statements);
        self.push_line_errors(errors);
        effects
    }

    fn push_line_errors(&mut self, errors: Vec<LinePlanLowerError>) {
        self.errors.extend(
            errors
                .into_iter()
                .map(|error| RuntimePlanLowerError::new(error.message().to_owned())),
        );
    }
}

pub(crate) fn sanitize_task_id_part(name: &str) -> String {
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
