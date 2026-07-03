//! Runtime-plan type validation against type-check evidence.
//!
//! This pass is intentionally independent from HIR lowering. It validates the
//! executable `RuntimePlan` shape that the VM consumes and checks that a
//! corresponding `TypeCheckReport` exists for typed source positions.

use crate::Severity;
use arcweft_core::plan::{
    ChoiceRuntimeOption, FlowOp, FlowRuntimeId, RuntimeEntryTarget, RuntimeFlow, RuntimeMatchArm,
    RuntimePlan,
};
use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeExpr, RuntimeExprMatchArm, RuntimeUnaryOp, RuntimeValue,
};
use arcweft_lang_sema::check::{TypeCheckReport, TypeJudgmentRule, TypeJudgmentSubject};
#[cfg(test)]
use arcweft_lang_sema::effect_analysis::EffectAnalysisReport;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Machine-readable result of runtime-plan type validation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeTypeValidationReport {
    pub diagnostics: Vec<RuntimeTypeDiagnostic>,
    pub stats: RuntimeTypeValidationStats,
}

/// One runtime-plan type validation diagnostic.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeTypeDiagnostic {
    pub severity: Severity,
    pub path: String,
    pub message: String,
}

/// Deterministic counters collected by runtime-plan type validation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeTypeValidationStats {
    pub flows: usize,
    pub ops: usize,
    pub expressions: usize,
    pub conditions: usize,
    pub guards: usize,
    pub let_bindings: usize,
    pub returns: usize,
    pub route_targets: usize,
    pub choice_targets: usize,
    pub type_judgments: usize,
}

/// Validates executable runtime plan typing assumptions.
pub fn validate_runtime_plan_types(
    plan: &RuntimePlan,
    types: &TypeCheckReport,
) -> RuntimeTypeValidationReport {
    let mut validator = RuntimeTypeValidator::new(plan, types);
    validator.validate();
    validator.report
}

impl RuntimeTypeValidationReport {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeShape {
    Unit,
    Bool,
    Int,
    Float,
    Matrix,
    Tensor,
    EntityRef,
    String,
    Tuple,
    BracketSeq,
    Range,
    Record,
    Variant,
    Iterator,
    Unknown,
}

struct RuntimeTypeValidator<'a> {
    plan: &'a RuntimePlan,
    types: &'a TypeCheckReport,
    flow_ids: BTreeSet<&'a str>,
    report: RuntimeTypeValidationReport,
}

impl<'a> RuntimeTypeValidator<'a> {
    fn new(plan: &'a RuntimePlan, types: &'a TypeCheckReport) -> Self {
        Self {
            plan,
            types,
            flow_ids: plan.flows.iter().map(|flow| flow.id.0.as_str()).collect(),
            report: RuntimeTypeValidationReport {
                stats: RuntimeTypeValidationStats {
                    flows: plan.flows.len(),
                    type_judgments: types.judgments.len(),
                    ..RuntimeTypeValidationStats::default()
                },
                ..RuntimeTypeValidationReport::default()
            },
        }
    }

    fn validate(&mut self) {
        self.validate_entry_targets();
        for flow in &self.plan.flows {
            self.validate_flow(flow);
        }
        self.validate_type_evidence();
    }

    fn validate_type_evidence(&mut self) {
        if self.types.stats.judgments != self.types.judgments.len() {
            self.error(
                "typecheck",
                format!(
                    "type judgment count mismatch: stats={}, judgments={}",
                    self.types.stats.judgments,
                    self.types.judgments.len()
                ),
            );
        }
        let evidence = TypeEvidence::from_report(self.types);
        if self.report.stats.expressions > 0 && !evidence.has_expr {
            self.error(
                "typecheck",
                "runtime plan validation requires expression type judgments",
            );
        }
        if self.report.stats.conditions + self.report.stats.guards > 0 && !evidence.has_expected {
            self.error(
                "typecheck",
                "runtime plan validation requires expected-type judgments for conditions and guards",
            );
        }
        if self.report.stats.let_bindings + self.report.stats.returns > 0
            && !evidence.has_binding_or_return
        {
            self.error(
                "typecheck",
                "runtime plan validation requires let-binding or return type judgments",
            );
        }
    }

    fn validate_entry_targets(&mut self) {
        if let Some(entry) = &self.plan.entry_flow {
            self.validate_flow_target("entry_flow", entry);
        }
        for entry in &self.plan.entries {
            match &entry.target {
                RuntimeEntryTarget::Flow(flow) => {
                    self.report.stats.route_targets += 1;
                    self.validate_flow_target(&format!("entry.{}", entry.id.0), flow);
                }
                RuntimeEntryTarget::Routes(routes) => {
                    for route in routes {
                        self.report.stats.route_targets += 1;
                        self.validate_flow_target(
                            &format!("entry.{}.route.{}", entry.id.0, route.path),
                            &route.target,
                        );
                    }
                }
            }
        }
    }

    fn validate_flow(&mut self, flow: &RuntimeFlow) {
        self.validate_ops(&format!("flow.{}", flow.id.0), &flow.ops);
    }

    fn validate_ops(&mut self, path: &str, ops: &[FlowOp]) {
        for (index, op) in ops.iter().enumerate() {
            self.report.stats.ops += 1;
            let path = format!("{path}.op.{index}");
            self.validate_op(&path, op);
        }
    }

    fn validate_op(&mut self, path: &str, op: &FlowOp) {
        match op {
            FlowOp::Bind(bindings) => {
                self.report.stats.let_bindings += bindings.len();
            }
            FlowOp::Let { expr, .. } | FlowOp::ExitScopeBind { expr, .. } => {
                self.report.stats.let_bindings += 1;
                self.validate_expr(path, expr);
            }
            FlowOp::LetElse { expr, else_ops, .. } => {
                self.report.stats.let_bindings += 1;
                self.validate_expr(path, expr);
                self.validate_ops(&format!("{path}.else"), else_ops);
            }
            FlowOp::Dialogue { task_group, .. } => {
                if *task_group >= self.plan.line_task_groups.len() {
                    self.error(
                        path,
                        format!("dialogue references missing line task group {task_group}"),
                    );
                }
            }
            FlowOp::Choice { options, .. } => self.validate_choice_options(path, options),
            FlowOp::If {
                condition,
                then_ops,
                else_ops,
            } => self.validate_branch_ops(path, condition, then_ops, else_ops),
            FlowOp::IfLet {
                expr,
                guard,
                then_ops,
                else_ops,
                ..
            } => self.validate_if_let_ops(path, expr, guard.as_ref(), then_ops, else_ops),
            FlowOp::Match { scrutinee, arms } => self.validate_match_ops(path, scrutinee, arms),
            FlowOp::Loop { body } | FlowOp::LetLoop { body, .. } | FlowOp::Scope(body) => {
                self.validate_ops(path, body);
            }
            FlowOp::LoopNext { body } => self.validate_ops(path, body),
            FlowOp::While { condition, body } => {
                self.validate_condition(path, condition);
                self.validate_ops(&format!("{path}.body"), body);
            }
            FlowOp::WhileNext { condition, body } => {
                self.validate_condition(path, condition);
                self.validate_ops(&format!("{path}.body"), body);
            }
            FlowOp::WhileLet {
                expr, guard, body, ..
            } => self.validate_while_let_ops(path, expr, guard.as_ref(), body),
            FlowOp::WhileLetNext {
                expr, guard, body, ..
            } => self.validate_while_let_ops(path, expr, guard.as_ref(), body),
            FlowOp::For { source, body, .. } => self.validate_for_ops(path, source, body),
            FlowOp::ForNext { body, .. } => {
                self.validate_ops(&format!("{path}.body"), body);
            }
            FlowOp::Thread { body, .. } => {
                self.validate_ops(&format!("{path}.body"), body);
            }
            FlowOp::LetScope { ops, value, .. } => {
                self.report.stats.let_bindings += 1;
                self.validate_ops(&format!("{path}.scope"), ops);
                self.validate_expr(&format!("{path}.value"), value);
            }
            FlowOp::Break(expr) => {
                if let Some(expr) = expr {
                    self.validate_expr(path, expr);
                }
            }
            FlowOp::Goto(target) => self.validate_flow_target(path, target),
            FlowOp::GotoExpr(expr) => self.validate_goto_expr(path, expr),
            FlowOp::ReturnExpr(expr) => {
                self.report.stats.returns += 1;
                self.validate_expr(path, expr);
            }
            FlowOp::Return(_) => {
                self.report.stats.returns += 1;
            }
            FlowOp::HostCall { binding, target } => {
                if binding.is_some() {
                    self.report.stats.let_bindings += 1;
                }
                target.args.iter().for_each(|arg| {
                    self.validate_expr(path, arg);
                });
            }
            FlowOp::Await { .. }
            | FlowOp::AwaitMany { .. }
            | FlowOp::Effect(_)
            | FlowOp::EnterScope
            | FlowOp::ExitScope
            | FlowOp::Continue
            | FlowOp::Noop => {}
        }
    }

    fn validate_branch_ops(
        &mut self,
        path: &str,
        condition: &RuntimeExpr,
        then_ops: &[FlowOp],
        else_ops: &[FlowOp],
    ) {
        self.validate_condition(path, condition);
        self.validate_ops(&format!("{path}.then"), then_ops);
        self.validate_ops(&format!("{path}.else"), else_ops);
    }

    fn validate_if_let_ops(
        &mut self,
        path: &str,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
        then_ops: &[FlowOp],
        else_ops: &[FlowOp],
    ) {
        self.validate_expr(path, expr);
        if let Some(guard) = guard {
            self.validate_guard(path, guard);
        }
        self.validate_ops(&format!("{path}.then"), then_ops);
        self.validate_ops(&format!("{path}.else"), else_ops);
    }

    fn validate_match_ops(
        &mut self,
        path: &str,
        scrutinee: &RuntimeExpr,
        arms: &[RuntimeMatchArm],
    ) {
        self.validate_expr(path, scrutinee);
        for (index, arm) in arms.iter().enumerate() {
            self.validate_match_arm(&format!("{path}.arm.{index}"), arm);
        }
    }

    fn validate_while_let_ops(
        &mut self,
        path: &str,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
        body: &[FlowOp],
    ) {
        self.validate_expr(path, expr);
        if let Some(guard) = guard {
            self.validate_guard(path, guard);
        }
        self.validate_ops(&format!("{path}.body"), body);
    }

    fn validate_for_ops(&mut self, path: &str, source: &RuntimeExpr, body: &[FlowOp]) {
        let shape = self.validate_expr(path, source);
        if shape != RuntimeShape::Unknown
            && shape != RuntimeShape::BracketSeq
            && shape != RuntimeShape::Range
        {
            self.error(
                path,
                format!("for source must be iterable, found {shape:?}"),
            );
        }
        self.validate_ops(&format!("{path}.body"), body);
    }

    fn validate_goto_expr(&mut self, path: &str, expr: &RuntimeExpr) {
        let shape = self.validate_expr(path, expr);
        if shape != RuntimeShape::Unknown && shape != RuntimeShape::EntityRef {
            self.error(
                path,
                format!("goto expression must be a flow reference, found {shape:?}"),
            );
        }
    }

    fn validate_choice_options(&mut self, path: &str, options: &[ChoiceRuntimeOption]) {
        for (index, option) in options.iter().enumerate() {
            if let Some(target) = &option.target {
                self.report.stats.choice_targets += 1;
                self.validate_flow_target(&format!("{path}.option.{index}"), target);
            }
        }
    }

    fn validate_match_arm(&mut self, path: &str, arm: &RuntimeMatchArm) {
        if let Some(guard) = &arm.guard {
            self.validate_guard(path, guard);
        }
        self.validate_ops(path, &arm.ops);
    }

    fn validate_condition(&mut self, path: &str, expr: &RuntimeExpr) {
        self.report.stats.conditions += 1;
        let shape = self.validate_expr(&format!("{path}.condition"), expr);
        self.require_bool(path, shape, "condition");
    }

    fn validate_guard(&mut self, path: &str, expr: &RuntimeExpr) {
        self.report.stats.guards += 1;
        let shape = self.validate_expr(&format!("{path}.guard"), expr);
        self.require_bool(path, shape, "guard");
    }

    fn require_bool(&mut self, path: &str, shape: RuntimeShape, context: &str) {
        if shape != RuntimeShape::Unknown && shape != RuntimeShape::Bool {
            self.error(path, format!("{context} must be Bool, found {shape:?}"));
        }
    }

    fn validate_expr(&mut self, path: &str, expr: &RuntimeExpr) -> RuntimeShape {
        self.report.stats.expressions += 1;
        match expr {
            RuntimeExpr::Value(value) => runtime_value_shape(value),
            RuntimeExpr::Local(_)
            | RuntimeExpr::Field { .. }
            | RuntimeExpr::ProjectTuple { .. }
            | RuntimeExpr::ProjectRecord { .. }
            | RuntimeExpr::Call { .. }
            | RuntimeExpr::PureCall { .. }
            | RuntimeExpr::SpreadArg(_) => RuntimeShape::Unknown,
            RuntimeExpr::EntityRef(_) => RuntimeShape::EntityRef,
            RuntimeExpr::Let { expr, body, .. } => self.validate_let_expr(path, expr, body),
            RuntimeExpr::Tuple(items) => {
                self.validate_expr_items(path, "tuple", items, RuntimeShape::Tuple)
            }
            RuntimeExpr::BracketSeq(items) => {
                self.validate_expr_items(path, "seq", items, RuntimeShape::BracketSeq)
            }
            RuntimeExpr::RepeatSeq { value, .. } => {
                self.validate_expr(&format!("{path}.repeat"), value);
                RuntimeShape::BracketSeq
            }
            RuntimeExpr::Range { start, end, .. } => {
                self.validate_range_expr(path, start.as_deref(), end.as_deref())
            }
            RuntimeExpr::Record(fields) => {
                for field in fields {
                    self.validate_expr(&format!("{path}.field.{}", field.name), &field.value);
                }
                RuntimeShape::Record
            }
            RuntimeExpr::Variant { payload, .. } => {
                if let Some(payload) = payload {
                    self.validate_expr(&format!("{path}.payload"), payload);
                }
                RuntimeShape::Variant
            }
            RuntimeExpr::MethodCall { receiver, args, .. } => {
                self.validate_method_call_expr(path, receiver, args)
            }
            RuntimeExpr::Map { source, body, .. } => {
                self.validate_expr(&format!("{path}.source"), source);
                self.validate_expr(&format!("{path}.body"), body);
                RuntimeShape::BracketSeq
            }
            RuntimeExpr::Sum { source } => {
                self.validate_expr(&format!("{path}.source"), source);
                RuntimeShape::Int
            }
            RuntimeExpr::Unary { op, expr } => self.validate_unary_expr(path, *op, expr),
            RuntimeExpr::Binary { lhs, op, rhs } => self.validate_binary_expr(path, lhs, *op, rhs),
            RuntimeExpr::If {
                condition,
                then_expr,
                else_expr,
            } => self.validate_if_expr(path, condition, then_expr, else_expr),
            RuntimeExpr::IfLet {
                expr,
                guard,
                then_expr,
                else_expr,
                ..
            } => self.validate_if_let_expr(path, expr, guard.as_deref(), then_expr, else_expr),
            RuntimeExpr::Match { scrutinee, arms } => {
                self.validate_match_expr(path, scrutinee, arms)
            }
        }
    }

    fn validate_expr_items(
        &mut self,
        path: &str,
        label: &str,
        items: &[RuntimeExpr],
        shape: RuntimeShape,
    ) -> RuntimeShape {
        for (index, item) in items.iter().enumerate() {
            self.validate_expr(&format!("{path}.{label}.{index}"), item);
        }
        shape
    }

    fn validate_range_expr(
        &mut self,
        path: &str,
        start: Option<&RuntimeExpr>,
        end: Option<&RuntimeExpr>,
    ) -> RuntimeShape {
        if let Some(start) = start {
            self.validate_expr(&format!("{path}.range.start"), start);
        }
        if let Some(end) = end {
            self.validate_expr(&format!("{path}.range.end"), end);
        }
        RuntimeShape::Range
    }

    fn validate_method_call_expr(
        &mut self,
        path: &str,
        receiver: &RuntimeExpr,
        args: &[RuntimeExpr],
    ) -> RuntimeShape {
        self.validate_expr(&format!("{path}.receiver"), receiver);
        for (index, arg) in args.iter().enumerate() {
            self.validate_expr(&format!("{path}.arg.{index}"), arg);
        }
        RuntimeShape::Unknown
    }

    fn validate_if_expr(
        &mut self,
        path: &str,
        condition: &RuntimeExpr,
        then_expr: &RuntimeExpr,
        else_expr: &RuntimeExpr,
    ) -> RuntimeShape {
        let condition = self.validate_expr(&format!("{path}.condition"), condition);
        self.require_bool(path, condition, "if expression condition");
        let then_shape = self.validate_expr(&format!("{path}.then"), then_expr);
        let else_shape = self.validate_expr(&format!("{path}.else"), else_expr);
        merge_shapes(then_shape, else_shape)
    }

    fn validate_if_let_expr(
        &mut self,
        path: &str,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
        then_expr: &RuntimeExpr,
        else_expr: &RuntimeExpr,
    ) -> RuntimeShape {
        self.validate_expr(&format!("{path}.if_let"), expr);
        if let Some(guard) = guard {
            let guard_shape = self.validate_expr(&format!("{path}.guard"), guard);
            self.require_bool(path, guard_shape, "if-let expression guard");
        }
        let then_shape = self.validate_expr(&format!("{path}.then"), then_expr);
        let else_shape = self.validate_expr(&format!("{path}.else"), else_expr);
        merge_shapes(then_shape, else_shape)
    }

    fn validate_match_expr(
        &mut self,
        path: &str,
        scrutinee: &RuntimeExpr,
        arms: &[RuntimeExprMatchArm],
    ) -> RuntimeShape {
        self.validate_expr(&format!("{path}.scrutinee"), scrutinee);
        arms.iter()
            .enumerate()
            .fold(RuntimeShape::Unknown, |shape, (index, arm)| {
                merge_shapes(
                    shape,
                    self.validate_expr_match_arm(&format!("{path}.arm.{index}"), arm),
                )
            })
    }

    fn validate_let_expr(
        &mut self,
        path: &str,
        expr: &RuntimeExpr,
        body: &RuntimeExpr,
    ) -> RuntimeShape {
        self.report.stats.let_bindings += 1;
        self.validate_expr(&format!("{path}.let"), expr);
        self.validate_expr(&format!("{path}.body"), body)
    }

    fn validate_expr_match_arm(&mut self, path: &str, arm: &RuntimeExprMatchArm) -> RuntimeShape {
        if let Some(guard) = &arm.guard {
            let guard_shape = self.validate_expr(&format!("{path}.guard"), guard);
            self.require_bool(path, guard_shape, "match arm guard");
        }
        self.validate_expr(path, &arm.value)
    }

    fn validate_unary_expr(
        &mut self,
        path: &str,
        op: RuntimeUnaryOp,
        expr: &RuntimeExpr,
    ) -> RuntimeShape {
        let shape = self.validate_expr(&format!("{path}.operand"), expr);
        match op {
            RuntimeUnaryOp::Not => {
                self.require_bool(path, shape, "not operand");
                RuntimeShape::Bool
            }
            RuntimeUnaryOp::Neg => {
                if shape != RuntimeShape::Unknown && shape != RuntimeShape::Int {
                    self.error(
                        path,
                        format!("negation operand must be Int, found {shape:?}"),
                    );
                }
                RuntimeShape::Int
            }
        }
    }

    fn validate_binary_expr(
        &mut self,
        path: &str,
        lhs: &RuntimeExpr,
        op: RuntimeBinaryOp,
        rhs: &RuntimeExpr,
    ) -> RuntimeShape {
        let lhs = self.validate_expr(&format!("{path}.lhs"), lhs);
        let rhs = self.validate_expr(&format!("{path}.rhs"), rhs);
        match op {
            RuntimeBinaryOp::Eq
            | RuntimeBinaryOp::Ne
            | RuntimeBinaryOp::Lt
            | RuntimeBinaryOp::Le
            | RuntimeBinaryOp::Gt
            | RuntimeBinaryOp::Ge => {
                self.require_compatible_operands(path, lhs, rhs, "comparison");
                RuntimeShape::Bool
            }
            RuntimeBinaryOp::And | RuntimeBinaryOp::Or => {
                self.require_bool(path, lhs, "logical left operand");
                self.require_bool(path, rhs, "logical right operand");
                RuntimeShape::Bool
            }
            RuntimeBinaryOp::Add
            | RuntimeBinaryOp::Sub
            | RuntimeBinaryOp::Mul
            | RuntimeBinaryOp::Div => {
                self.require_int(path, lhs, "arithmetic left operand");
                self.require_int(path, rhs, "arithmetic right operand");
                RuntimeShape::Int
            }
        }
    }

    fn require_compatible_operands(
        &mut self,
        path: &str,
        lhs: RuntimeShape,
        rhs: RuntimeShape,
        context: &str,
    ) {
        if lhs != RuntimeShape::Unknown && rhs != RuntimeShape::Unknown && lhs != rhs {
            self.error(
                path,
                format!("{context} operands differ: {lhs:?} and {rhs:?}"),
            );
        }
    }

    fn require_int(&mut self, path: &str, shape: RuntimeShape, context: &str) {
        if shape != RuntimeShape::Unknown && shape != RuntimeShape::Int {
            self.error(path, format!("{context} must be Int, found {shape:?}"));
        }
    }

    fn validate_flow_target(&mut self, path: &str, target: &FlowRuntimeId) {
        if !self.flow_ids.contains(target.0.as_str()) {
            self.error(
                path,
                format!("runtime flow target `{}` does not exist", target.0),
            );
        }
    }

    fn error(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.report.diagnostics.push(RuntimeTypeDiagnostic {
            severity: Severity::Error,
            path: path.into(),
            message: message.into(),
        });
    }
}

fn merge_shapes(left: RuntimeShape, right: RuntimeShape) -> RuntimeShape {
    match (left, right) {
        (RuntimeShape::Unknown, shape) | (shape, RuntimeShape::Unknown) => shape,
        (left, right) if left == right => left,
        _ => RuntimeShape::Unknown,
    }
}

fn runtime_value_shape(value: &RuntimeValue) -> RuntimeShape {
    match value {
        RuntimeValue::Unit => RuntimeShape::Unit,
        RuntimeValue::Bool(_) => RuntimeShape::Bool,
        RuntimeValue::Int(_) | RuntimeValue::UInt(_) | RuntimeValue::Duration(_) => {
            RuntimeShape::Int
        }
        RuntimeValue::F32(_) | RuntimeValue::F64(_) => RuntimeShape::Float,
        RuntimeValue::MatrixF32(_) | RuntimeValue::MatrixF64(_) => RuntimeShape::Matrix,
        RuntimeValue::TensorF32(_) | RuntimeValue::TensorF64(_) => RuntimeShape::Tensor,
        RuntimeValue::EntityRef(_) => RuntimeShape::EntityRef,
        RuntimeValue::String(_) | RuntimeValue::Char(_) => RuntimeShape::String,
        RuntimeValue::Tuple(_) => RuntimeShape::Tuple,
        RuntimeValue::Seq(_) => RuntimeShape::BracketSeq,
        RuntimeValue::Range(_) => RuntimeShape::Range,
        RuntimeValue::Record(_) => RuntimeShape::Record,
        RuntimeValue::Variant { .. } => RuntimeShape::Variant,
        RuntimeValue::Iterator(_) => RuntimeShape::Iterator,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TypeEvidence {
    has_expr: bool,
    has_expected: bool,
    has_binding_or_return: bool,
}

impl TypeEvidence {
    fn from_report(types: &TypeCheckReport) -> Self {
        let has_expr = types
            .judgments
            .iter()
            .any(|judgment| judgment.rule == TypeJudgmentRule::Expr);
        let has_expected = types
            .judgments
            .iter()
            .any(|judgment| judgment.rule == TypeJudgmentRule::Expected);
        let has_binding_or_return = types.judgments.iter().any(|judgment| {
            matches!(
                &judgment.subject,
                TypeJudgmentSubject::LetBinding { .. } | TypeJudgmentSubject::Return { .. }
            )
        });
        Self {
            has_expr,
            has_expected,
            has_binding_or_return,
        }
    }
}

#[cfg(test)]
fn has_required_type_evidence(types: &TypeCheckReport) -> bool {
    let evidence = TypeEvidence::from_report(types);
    evidence.has_expr && evidence.has_expected && evidence.has_binding_or_return
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow};
    use arcweft_core::value::{RuntimeBinaryOp, RuntimeExpr, RuntimeValue};
    use arcweft_lang_sema::check::{
        TypeCheckStats, TypeJudgment, TypeJudgmentExpected, TypeJudgmentId,
    };
    use arcweft_lang_sema::types::TypeKind;

    #[test]
    fn runtime_type_validation_accepts_bool_conditions_and_existing_targets() {
        let plan = RuntimePlan {
            entry_flow: Some(FlowRuntimeId("flow.main".to_owned())),
            flows: vec![RuntimeFlow {
                id: FlowRuntimeId("flow.main".to_owned()),
                ops: vec![FlowOp::If {
                    condition: RuntimeExpr::Value(RuntimeValue::Bool(true)),
                    then_ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Value(
                        RuntimeValue::String("ok".to_owned()),
                    ))],
                    else_ops: Vec::new(),
                }],
            }],
            ..RuntimePlan::default()
        };

        let report = validate_runtime_plan_types(&plan, &type_report());

        assert!(
            report.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            report.diagnostics
        );
        assert_eq!(report.stats.flows, 1);
        assert_eq!(report.stats.conditions, 1);
        assert_eq!(report.stats.returns, 1);
    }

    #[test]
    fn runtime_type_validation_rejects_structural_type_conflicts() {
        let plan = RuntimePlan {
            entry_flow: Some(FlowRuntimeId("flow.missing".to_owned())),
            flows: vec![RuntimeFlow {
                id: FlowRuntimeId("flow.main".to_owned()),
                ops: vec![
                    FlowOp::If {
                        condition: RuntimeExpr::Value(RuntimeValue::i64(1)),
                        then_ops: Vec::new(),
                        else_ops: Vec::new(),
                    },
                    FlowOp::ReturnExpr(RuntimeExpr::Binary {
                        lhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(1))),
                        op: RuntimeBinaryOp::Add,
                        rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Bool(false))),
                    }),
                ],
            }],
            ..RuntimePlan::default()
        };

        let report = validate_runtime_plan_types(&plan, &type_report());

        assert!(report.has_errors());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("does not exist"))
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("condition must be Bool"))
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("arithmetic right operand"))
        );
    }

    #[test]
    fn required_type_evidence_checks_report_shape() {
        assert!(has_required_type_evidence(&type_report()));
        assert!(!has_required_type_evidence(&TypeCheckReport {
            diagnostics: Vec::new(),
            warnings: Vec::new(),
            stats: TypeCheckStats::default(),
            judgments: Vec::new(),
            effects: EffectAnalysisReport::default(),
        }));
    }

    fn type_report() -> TypeCheckReport {
        let judgments = vec![
            TypeJudgment {
                id: TypeJudgmentId::from_index(0),
                subject: TypeJudgmentSubject::Expr { kind: "literal" },
                ty: TypeKind::Bool,
                rule: TypeJudgmentRule::Expr,
                expected: None,
            },
            TypeJudgment {
                id: TypeJudgmentId::from_index(1),
                subject: TypeJudgmentSubject::Expr { kind: "literal" },
                ty: TypeKind::Bool,
                rule: TypeJudgmentRule::Expected,
                expected: Some(TypeJudgmentExpected::SameAsJudgment),
            },
            TypeJudgment {
                id: TypeJudgmentId::from_index(2),
                subject: TypeJudgmentSubject::Return {
                    context: "return statement".to_owned(),
                },
                ty: TypeKind::String,
                rule: TypeJudgmentRule::Return,
                expected: Some(TypeJudgmentExpected::SameAsJudgment),
            },
        ];
        TypeCheckReport {
            diagnostics: Vec::new(),
            warnings: Vec::new(),
            stats: TypeCheckStats {
                judgments: judgments.len(),
                ..TypeCheckStats::default()
            },
            judgments,
            effects: EffectAnalysisReport::default(),
        }
    }
}
