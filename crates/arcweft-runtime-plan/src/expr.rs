//! Runtime expression and effect-call lowering.

use crate::function_values::RuntimeFunctionValueCandidate;
use crate::labels::{call_arg_label, duration_expr, entity_ref_label, expr_label, literal_label};
use crate::pattern::lower_runtime_pattern;
use crate::typed_evidence::{
    RuntimeDataLastMethodFallbackArg, RuntimeNumericType, RuntimeTypedExpressionId,
    RuntimeTypedLoweringEvidence, RuntimeTypedLoweringEvidenceLookup,
};
use arcweft_core::plan::{RuntimePureHelper, RuntimePureHelperId};
use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeCallTarget, RuntimeExpr, RuntimeExprMatchArm, RuntimeFieldExpr,
    RuntimeUnaryOp, RuntimeValue, runtime_sequence_dense_i8, runtime_sequence_dense_i16,
    runtime_sequence_dense_i32, runtime_sequence_dense_i64, runtime_sequence_dense_i128,
    runtime_sequence_dense_isize, runtime_sequence_dense_u8, runtime_sequence_dense_u16,
    runtime_sequence_dense_u32, runtime_sequence_dense_u64, runtime_sequence_dense_u128,
    runtime_sequence_dense_usize, runtime_sequence_from_literal_values,
};
use arcweft_lang_syntax::{
    ast::{flow::Stmt, line_plan::LinePlanItem, pattern::Pattern},
    expr::{
        BinaryOp, CallArg, ClosureParam, DecodedStringLiteral, Expr, FloatSuffix, IntLiteral,
        Literal, MatchExprArm, NumericBracketSeq, Placeholder, UnaryOp,
    },
    types::TypeRef,
};
use std::{cell::Cell, collections::BTreeMap, sync::LazyLock};

pub(crate) mod desugar;
mod effect;
mod enum_constructor;
mod named_callable;
use desugar::expr_contains_partial_placeholder;
pub(crate) use effect::{
    LoweredRuntimeEffect, lower_runtime_effect_strict_with_pure, runtime_call_effect,
};
use enum_constructor::{lower_constructor_call, lower_expected_enum_record_constructor};
use named_callable::{
    PureHelperNamedCallLowering, lower_strict_function_value_named_call,
    lower_strict_named_callable_args, lower_strict_pure_helper_named_call,
};

#[derive(Clone, Copy)]
pub(crate) struct RuntimePureHelperLookup<'helpers, 'functions, 'locals> {
    ids: &'helpers BTreeMap<String, RuntimePureHelperId>,
    helpers: &'helpers [RuntimePureHelper],
    function_values: Option<&'functions BTreeMap<String, RuntimeFunctionValueCandidate>>,
    function_locals: Option<&'locals BTreeMap<String, usize>>,
    typed_lowering_evidence: Option<RuntimeTypedLoweringEvidenceLookup<'helpers>>,
    expression_cursor: Option<&'helpers Cell<usize>>,
    pipe_binding_depth: Option<u32>,
    partial_binding_name: Option<&'static str>,
}

static EMPTY_PURE_HELPER_IDS: LazyLock<BTreeMap<String, RuntimePureHelperId>> =
    LazyLock::new(BTreeMap::new);

impl<'helpers> RuntimePureHelperLookup<'helpers, 'static, 'static> {
    pub(crate) fn new(
        ids: &'helpers BTreeMap<String, RuntimePureHelperId>,
        helpers: &'helpers [RuntimePureHelper],
    ) -> Self {
        Self {
            ids,
            helpers,
            function_values: None,
            function_locals: None,
            typed_lowering_evidence: None,
            expression_cursor: None,
            pipe_binding_depth: None,
            partial_binding_name: None,
        }
    }
}

fn empty_runtime_lookup<'helpers, 'functions, 'locals>()
-> RuntimePureHelperLookup<'helpers, 'functions, 'locals> {
    RuntimePureHelperLookup {
        ids: &EMPTY_PURE_HELPER_IDS,
        helpers: &[],
        function_values: None,
        function_locals: None,
        typed_lowering_evidence: None,
        expression_cursor: None,
        pipe_binding_depth: None,
        partial_binding_name: None,
    }
}

impl<'helpers, 'functions, 'locals> RuntimePureHelperLookup<'helpers, 'functions, 'locals> {
    pub(crate) fn with_function_locals(
        self,
        function_locals: &'locals BTreeMap<String, usize>,
    ) -> RuntimePureHelperLookup<'helpers, 'functions, 'locals> {
        RuntimePureHelperLookup {
            ids: self.ids,
            helpers: self.helpers,
            function_values: self.function_values,
            function_locals: Some(function_locals),
            typed_lowering_evidence: self.typed_lowering_evidence,
            expression_cursor: self.expression_cursor,
            pipe_binding_depth: self.pipe_binding_depth,
            partial_binding_name: self.partial_binding_name,
        }
    }

    pub(crate) fn with_runtime_function_values<'new_functions>(
        self,
        function_values: &'new_functions BTreeMap<String, RuntimeFunctionValueCandidate>,
    ) -> RuntimePureHelperLookup<'helpers, 'new_functions, 'locals> {
        RuntimePureHelperLookup {
            ids: self.ids,
            helpers: self.helpers,
            function_values: Some(function_values),
            function_locals: self.function_locals,
            typed_lowering_evidence: self.typed_lowering_evidence,
            expression_cursor: self.expression_cursor,
            pipe_binding_depth: self.pipe_binding_depth,
            partial_binding_name: self.partial_binding_name,
        }
    }

    pub(crate) fn with_typed_lowering_evidence(
        self,
        typed_lowering_evidence: &'helpers [RuntimeTypedLoweringEvidence],
        expression_cursor: &'helpers Cell<usize>,
    ) -> RuntimePureHelperLookup<'helpers, 'functions, 'locals> {
        RuntimePureHelperLookup {
            ids: self.ids,
            helpers: self.helpers,
            function_values: self.function_values,
            function_locals: self.function_locals,
            typed_lowering_evidence: Some(RuntimeTypedLoweringEvidenceLookup::new(
                typed_lowering_evidence,
            )),
            expression_cursor: Some(expression_cursor),
            pipe_binding_depth: self.pipe_binding_depth,
            partial_binding_name: self.partial_binding_name,
        }
    }

    pub(crate) fn with_project_function_typed_lowering_evidence<'new_helpers>(
        self,
        typed_lowering_evidence: &'new_helpers [RuntimeTypedLoweringEvidence],
        expression_cursor: &'new_helpers Cell<usize>,
        declaration: &'new_helpers arcweft_lang_hir::symbol::CallableDeclarationId,
    ) -> RuntimePureHelperLookup<'new_helpers, 'functions, 'locals>
    where
        'helpers: 'new_helpers,
    {
        RuntimePureHelperLookup {
            ids: self.ids,
            helpers: self.helpers,
            function_values: self.function_values,
            function_locals: self.function_locals,
            typed_lowering_evidence: Some(
                RuntimeTypedLoweringEvidenceLookup::for_project_function(
                    typed_lowering_evidence,
                    declaration,
                ),
            ),
            expression_cursor: Some(expression_cursor),
            pipe_binding_depth: self.pipe_binding_depth,
            partial_binding_name: self.partial_binding_name,
        }
    }

    fn enter_pipe_binding(mut self) -> Self {
        self.pipe_binding_depth = Some(
            self.pipe_binding_depth
                .map_or(0, |depth| depth.saturating_add(1)),
        );
        self
    }

    fn pipe_binding_name(self) -> Option<String> {
        self.pipe_binding_depth
            .map(|depth| format!("\0arcweft.pipe.{depth}"))
    }

    fn enter_partial_binding(mut self, name: &'static str) -> Self {
        self.partial_binding_name = Some(name);
        self
    }

    const fn partial_binding_name(self) -> Option<&'static str> {
        self.partial_binding_name
    }

    fn id(self, name: &str) -> Option<RuntimePureHelperId> {
        self.ids.get(name).copied()
    }

    fn helper(self, name: &str) -> Option<&'helpers RuntimePureHelper> {
        let id = self.id(name)?;
        self.helpers
            .get(id.0)
            .filter(|helper| helper.id == id && helper.name == name)
    }

    fn arity(self, name: &str) -> Option<usize> {
        self.helper(name).map(|helper| helper.input_names.len())
    }

    pub(crate) fn pure_helper_input_names(self, name: &str) -> Option<&'helpers [String]> {
        self.helper(name)
            .map(|helper| helper.input_names.as_slice())
    }

    fn function_value(self, name: &str) -> Option<RuntimeExpr> {
        self.helper(name)
            .map(|helper| RuntimeExpr::Function {
                params: helper.input_names.clone(),
                body: Box::new(helper.expr.clone()),
            })
            .or_else(|| {
                self.function_value_candidate(name)
                    .map(RuntimeFunctionValueCandidate::value)
            })
    }

    pub(crate) fn function_value_candidate(
        self,
        name: &str,
    ) -> Option<&'functions RuntimeFunctionValueCandidate> {
        self.function_values
            .and_then(|function_values| function_values.get(name))
    }

    fn local_function_arity(self, name: &str) -> Option<usize> {
        self.function_locals
            .and_then(|function_locals| function_locals.get(name).copied())
    }

    fn value_expr(self, name: &str) -> Option<RuntimeExpr> {
        if self.local_function_arity(name).is_some() {
            Some(RuntimeExpr::Local(name.to_owned()))
        } else {
            self.function_value(name)
        }
    }

    fn next_expression_id(self) -> Option<RuntimeTypedExpressionId> {
        let cursor = self.expression_cursor?;
        let index = cursor.get();
        cursor.set(index + 1);
        Some(RuntimeTypedExpressionId::from_index(index))
    }

    fn current_expression_id(self) -> Option<RuntimeTypedExpressionId> {
        self.expression_cursor
            .map(Cell::get)
            .map(RuntimeTypedExpressionId::from_index)
    }

    fn has_function_value_call_evidence(
        self,
        expression_id: Option<RuntimeTypedExpressionId>,
        callee: Option<&str>,
        arg_count: usize,
    ) -> bool {
        expression_id.is_some_and(|expression_id| {
            self.typed_lowering_evidence.is_some_and(|evidence| {
                evidence.has_function_value_call(expression_id, callee, arg_count)
            })
        })
    }

    fn has_partial_function_value_call_evidence(
        self,
        expression_id: Option<RuntimeTypedExpressionId>,
        callee: Option<&str>,
        arg_count: usize,
    ) -> bool {
        expression_id.is_some_and(|expression_id| {
            self.typed_lowering_evidence.is_some_and(|evidence| {
                evidence.has_partial_function_value_call(expression_id, callee, arg_count)
            })
        })
    }

    fn resolved_numeric_type(
        self,
        expression_id: Option<RuntimeTypedExpressionId>,
    ) -> Option<RuntimeNumericType> {
        expression_id.and_then(|expression_id| {
            self.typed_lowering_evidence
                .and_then(|evidence| evidence.resolved_numeric_type(expression_id))
        })
    }

    fn has_expected_function_value_evidence(self, expression_id: RuntimeTypedExpressionId) -> bool {
        self.typed_lowering_evidence
            .is_some_and(|evidence| evidence.has_expected_function_value(expression_id))
    }

    fn should_lower_partial_function_value(
        self,
        expr: &Expr,
        expression_id: Option<RuntimeTypedExpressionId>,
        expected_by_type: bool,
    ) -> bool {
        (expected_by_type
            || expression_id.is_some_and(|id| self.has_expected_function_value_evidence(id)))
            && self.partial_binding_name().is_none()
            && expr_contains_partial_placeholder(expr)
    }

    fn has_function_value_reference_evidence(
        self,
        expression_id: Option<RuntimeTypedExpressionId>,
        callee: &str,
    ) -> bool {
        expression_id.is_some_and(|expression_id| {
            self.typed_lowering_evidence.is_some_and(|evidence| {
                evidence.has_function_value_reference(expression_id, callee)
            })
        })
    }

    fn has_signature_partial_call_evidence(
        self,
        expression_id: Option<RuntimeTypedExpressionId>,
        callee: &str,
        arg_count: usize,
    ) -> bool {
        expression_id.is_some_and(|expression_id| {
            self.typed_lowering_evidence.is_some_and(|evidence| {
                evidence.has_signature_partial_call(expression_id, callee, arg_count)
            })
        })
    }

    fn data_last_method_fallback_arg_order(
        self,
        expression_id: Option<RuntimeTypedExpressionId>,
        method: &str,
        arg_count: usize,
    ) -> Option<&'helpers [RuntimeDataLastMethodFallbackArg]> {
        expression_id.and_then(|expression_id| {
            self.typed_lowering_evidence.and_then(|evidence| {
                evidence.data_last_method_fallback_arg_order(expression_id, method, arg_count)
            })
        })
    }
}

/// Lowers an expression into a runtime value expression, preserving a lossy
/// string label for adapter-facing values that are not executable by the core.
pub(crate) fn lower_runtime_expr(expr: &Expr) -> RuntimeExpr {
    match expr {
        Expr::Literal(literal) => RuntimeExpr::Value(
            lower_runtime_literal(literal, None)
                .unwrap_or_else(|_| RuntimeValue::String(literal_label(literal))),
        ),
        Expr::EntityRef(entity) => RuntimeExpr::EntityRef(entity_ref_label(entity)),
        Expr::Path(path) => RuntimeExpr::Local(path.as_label().to_owned()),
        Expr::ShortVariant(name) => RuntimeExpr::Value(RuntimeValue::String(format!(".{name}"))),
        Expr::Tuple(items) if items.is_empty() => RuntimeExpr::Value(RuntimeValue::Unit),
        Expr::Tuple(items) => RuntimeExpr::Tuple(items.iter().map(lower_runtime_expr).collect()),
        Expr::BracketSeq(items) => lower_runtime_bracket_seq(items),
        Expr::NumericBracketSeq(seq) => lower_runtime_numeric_bracket_seq_lossy(seq),
        Expr::ArrayRepeat { value, len } => lower_runtime_array_repeat(value, len),
        Expr::Range {
            start,
            end,
            inclusive,
        } => lower_runtime_range_expr_lossy(start.as_deref(), end.as_deref(), *inclusive),
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => RuntimeExpr::Record(
            fields
                .iter()
                .map(|(name, value)| RuntimeFieldExpr {
                    name: name.clone(),
                    value: lower_runtime_expr(value),
                })
                .collect(),
        ),
        Expr::Select(select) => {
            let target = select.target();
            let field = select.member().as_str();
            lower_enum_variant_field(target, field).unwrap_or_else(|| {
                lower_std_float_constant(expr).map_or_else(
                    || lower_runtime_field_expr(target, field),
                    RuntimeExpr::Value,
                )
            })
        }
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } if negated_signed_min_value(expr, None).is_some() => RuntimeExpr::Value(
            negated_signed_min_value(expr, None).expect("guard checked signed minimum"),
        ),
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
        Expr::Call(call) => lower_runtime_selected_call(call.callee(), call.args())
            .or_else(|| lower_choice_action_call(call.callee(), call.args()))
            .unwrap_or_else(|| RuntimeExpr::Call {
                callee: RuntimeCallTarget::from_label(expr_label(call.callee())),
                args: call.args().iter().map(lower_runtime_call_arg).collect(),
            }),
        Expr::Index { target, index } => {
            lower_runtime_index_expr(target, index).unwrap_or_else(|| lower_runtime_expr(target))
        }
        Expr::Try(try_expr) => lower_runtime_expr(try_expr.operand()),
        Expr::Await(awaited) => lower_runtime_expr(awaited.operand()),
        Expr::Pipe { lhs, rhs } => lower_runtime_pipe_expr(lhs, rhs),
        _ => RuntimeExpr::Value(RuntimeValue::String(expr_label(expr))),
    }
}

/// Strict expression lowering for executable flow/runtime positions.
pub(crate) fn lower_runtime_expr_strict(expr: &Expr) -> Result<RuntimeExpr, String> {
    lower_runtime_expr_strict_with_helpers(expr, None)
}

pub(crate) fn lower_runtime_expr_strict_with_function_locals_and_pure(
    expr: &Expr,
    function_locals: &BTreeMap<String, usize>,
    helpers: RuntimePureHelperLookup<'_, '_, '_>,
) -> Result<RuntimeExpr, String> {
    lower_runtime_expr_strict_with_helpers(
        expr,
        Some(helpers.with_function_locals(function_locals)),
    )
}

pub(crate) fn lower_runtime_expr_strict_with_pure(
    expr: &Expr,
    helpers: RuntimePureHelperLookup<'_, '_, '_>,
) -> Result<RuntimeExpr, String> {
    lower_runtime_expr_strict_with_helpers(expr, Some(helpers))
}

pub(crate) fn lower_runtime_expr_strict_with_expected_type(
    expr: &Expr,
    expected_ty: Option<&TypeRef>,
    helpers: RuntimePureHelperLookup<'_, '_, '_>,
) -> Result<RuntimeExpr, String> {
    let evidence_expression_id = helpers.current_expression_id();
    if helpers.should_lower_partial_function_value(
        expr,
        evidence_expression_id,
        expected_ty.is_some_and(single_param_function_type),
    ) {
        helpers.next_expression_id();
        lower_partial_placeholder_function_expr(expr, Some(helpers))
    } else if let Some(lowered) = lower_expected_enum_record_constructor(expr, expected_ty, helpers)
    {
        lowered
    } else {
        lower_runtime_expr_strict_with_helpers(expr, Some(helpers))
    }
}

fn lower_runtime_expr_strict_with_helpers(
    expr: &Expr,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    let expression_id = helpers.and_then(RuntimePureHelperLookup::next_expression_id);
    let resolved_numeric_type =
        helpers.and_then(|helpers| helpers.resolved_numeric_type(expression_id));
    if helpers.is_some_and(|helpers| {
        helpers.should_lower_partial_function_value(expr, expression_id, false)
    }) {
        return lower_partial_placeholder_function_expr(expr, helpers);
    }
    if let Some(value) = lower_strict_negated_signed_min(expr, helpers) {
        return Ok(RuntimeExpr::Value(value));
    }
    match expr {
        Expr::Literal(literal) => {
            lower_runtime_literal(literal, resolved_numeric_type).map(RuntimeExpr::Value)
        }
        Expr::EntityRef(entity) => Ok(RuntimeExpr::EntityRef(entity_ref_label(entity))),
        Expr::Path(path) => lower_strict_path_expr(path.as_label(), helpers, expression_id),
        Expr::ShortVariant(name) => Ok(RuntimeExpr::Variant {
            path: None,
            name: name.to_string(),
            payload: None,
        }),
        Expr::Tuple(items) if items.is_empty() => Ok(RuntimeExpr::Value(RuntimeValue::Unit)),
        Expr::Tuple(items) => items
            .iter()
            .map(|item| lower_runtime_expr_strict_with_helpers(item, helpers))
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeExpr::Tuple),
        Expr::BracketSeq(items) => lower_runtime_bracket_seq_strict(items, helpers),
        Expr::NumericBracketSeq(seq) => {
            lower_runtime_numeric_bracket_seq(seq, resolved_numeric_type)
        }
        Expr::ArrayRepeat { value, len } => lower_runtime_array_repeat_strict(value, len, helpers),
        Expr::Range {
            start,
            end,
            inclusive,
        } => lower_runtime_range_expr(start.as_deref(), end.as_deref(), *inclusive, helpers),
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
            lower_runtime_record_expr_strict(fields, helpers)
        }
        Expr::Select(select) => {
            lower_strict_field_or_constant(expr, select.target(), select.member().as_str(), helpers)
        }
        Expr::Unary { op, expr } => Ok(RuntimeExpr::Unary {
            op: lower_runtime_unary_op(*op),
            expr: Box::new(lower_runtime_expr_strict_with_helpers(expr, helpers)?),
        }),
        Expr::Binary { lhs, op, rhs } => lower_strict_binary_expr(expr, lhs, *op, rhs, helpers),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => lower_strict_if_expr(condition, then_branch, else_branch.as_deref(), helpers),
        Expr::IfLet {
            pattern,
            expr,
            guard,
            then_branch,
            else_branch,
        } => lower_strict_if_let_expr(
            pattern,
            expr,
            guard.as_deref(),
            then_branch,
            else_branch.as_deref(),
            helpers,
        ),
        Expr::Match { scrutinee, arms } => lower_strict_match_expr(scrutinee, arms, helpers),
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => lower_strict_block_expr(statements, value.as_deref(), helpers),
        Expr::Closure { params, body, .. } => lower_strict_closure_expr(params, body, helpers),
        Expr::Call(call) => {
            lower_strict_call_expr(call.callee(), call.args(), helpers, expression_id)
        }
        Expr::DialogueCall { plan, .. } => Ok(lower_dialogue_call_value(plan.as_ref())),
        Expr::Index { target, index } => lower_strict_index_expr(target, index, helpers),
        Expr::Try(_) => unsupported_runtime_control_value(RuntimeControlValue::Try),
        Expr::Await(_) => unsupported_runtime_control_value(RuntimeControlValue::Await),
        Expr::Pipe { lhs, rhs } => lower_runtime_pipe_expr_strict(lhs, rhs, helpers, expression_id),
        Expr::Placeholder(Placeholder::PipeLeft) => helpers
            .and_then(RuntimePureHelperLookup::pipe_binding_name)
            .map(RuntimeExpr::Local)
            .ok_or_else(|| "pipe-left placeholder is outside a runtime pipe scope".to_owned()),
        Expr::Placeholder(Placeholder::Partial) => helpers
            .and_then(RuntimePureHelperLookup::partial_binding_name)
            .map(|name| RuntimeExpr::Local(name.to_owned()))
            .ok_or_else(|| "partial placeholder is outside a runtime binding scope".to_owned()),
        Expr::Thread { .. }
        | Expr::LifetimePath { .. }
        | Expr::Borrow(_)
        | Expr::Deref(_)
        | Expr::Raw(_) => unsupported_strict_runtime_expr(expr),
    }
}

fn lower_strict_path_expr(
    name: &str,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
    expression_id: Option<RuntimeTypedExpressionId>,
) -> Result<RuntimeExpr, String> {
    if let Some((path, name)) = constructor_path(name) {
        return Ok(RuntimeExpr::Variant {
            path,
            name,
            payload: None,
        });
    }
    if let Some(value) = helpers.and_then(|helpers| helpers.value_expr(name)) {
        return Ok(value);
    }
    if helpers
        .is_some_and(|helpers| helpers.has_function_value_reference_evidence(expression_id, name))
    {
        return Err(format!(
            "unsupported callable family `source_function_value_without_runtime_candidate`: \
             function `{name}` cannot be referenced as a runtime function value because it has no \
             executable helper or accepted source-function candidate"
        ));
    }
    Ok(RuntimeExpr::Local(name.to_owned()))
}

fn lower_strict_closure_expr(
    params: &[ClosureParam],
    body: &Expr,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    let params = runtime_closure_param_bindings(params);
    let body = lower_runtime_expr_strict_with_helpers(body, helpers)?;
    let body = params.iter().rev().fold(body, |body, param| {
        if let Some(pattern) = param.pattern {
            RuntimeExpr::Match {
                scrutinee: Box::new(RuntimeExpr::Local(param.name.clone())),
                arms: vec![RuntimeExprMatchArm {
                    pattern: lower_runtime_pattern(pattern),
                    guard: None,
                    value: body,
                }],
            }
        } else {
            body
        }
    });
    Ok(RuntimeExpr::Function {
        params: params.into_iter().map(|param| param.name).collect(),
        body: Box::new(body),
    })
}

#[derive(Clone, Debug)]
struct RuntimeClosureParamBinding<'a> {
    name: String,
    pattern: Option<&'a Pattern>,
}

fn runtime_closure_param_bindings(params: &[ClosureParam]) -> Vec<RuntimeClosureParamBinding<'_>> {
    params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            param.simple_ident().map_or_else(
                || RuntimeClosureParamBinding {
                    name: format!("$arcweft.closure.arg.{index}"),
                    pattern: Some(param.pattern()),
                },
                |name| RuntimeClosureParamBinding {
                    name: name.to_owned(),
                    pattern: None,
                },
            )
        })
        .collect()
}

fn lower_partial_placeholder_function_expr(
    expr: &Expr,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    const PARAM_NAME: &str = "__arcweft_partial";
    let scoped_helpers = helpers
        .unwrap_or_else(empty_runtime_lookup)
        .enter_partial_binding(PARAM_NAME);
    lower_runtime_expr_strict_with_helpers(
        partial_placeholder_body_expr(expr),
        Some(scoped_helpers),
    )
    .map(|body| RuntimeExpr::Function {
        params: vec![PARAM_NAME.to_owned()],
        body: Box::new(body),
    })
}

fn partial_placeholder_body_expr(expr: &Expr) -> &Expr {
    match expr {
        Expr::Tuple(items) if items.len() == 1 => &items[0],
        _ => expr,
    }
}

fn single_param_function_type(ty: &TypeRef) -> bool {
    matches!(ty, TypeRef::Function { params, .. } if params.len() == 1)
}

fn lower_strict_binary_expr(
    source: &Expr,
    lhs: &Expr,
    op: BinaryOp,
    rhs: &Expr,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    let Some(op) = lower_runtime_binary_op(op) else {
        return Err(format!(
            "unsupported runtime binary expression `{}`",
            expr_label(source)
        ));
    };
    Ok(RuntimeExpr::Binary {
        lhs: Box::new(lower_runtime_expr_strict_with_helpers(lhs, helpers)?),
        op,
        rhs: Box::new(lower_runtime_expr_strict_with_helpers(rhs, helpers)?),
    })
}

fn lower_strict_method_call_dispatch(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
    expression_id: Option<RuntimeTypedExpressionId>,
) -> Result<RuntimeExpr, String> {
    if let Some(arg_order) = helpers.and_then(|helpers| {
        helpers.data_last_method_fallback_arg_order(
            expression_id,
            runtime_method_name(method),
            args.len(),
        )
    }) {
        return lower_strict_data_last_method_fallback(receiver, method, args, helpers, arg_order);
    }
    match lower_strict_math_method_call(receiver, method, args, helpers)
        .or_else(|| lower_strict_std_float_method_call(receiver, method, args, helpers))
        .or_else(|| lower_strict_path_method_call(receiver, method, args, helpers))
        .or_else(|| lower_strict_external_namespace_method_call(receiver, method, args, helpers))
    {
        Some(lowered) => lowered,
        None => lower_strict_method_call_expr(receiver, method, args, helpers),
    }
}

fn lower_strict_data_last_method_fallback(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
    arg_order: &[RuntimeDataLastMethodFallbackArg],
) -> Result<RuntimeExpr, String> {
    validate_data_last_fallback_evidence(method, args.len(), arg_order)?;
    let first_stage = lower_data_last_fallback_first_stage(method, args, helpers)?;
    let receiver = lower_runtime_expr_strict_with_helpers(receiver, helpers)?;
    Ok(RuntimeExpr::Apply {
        callee: Box::new(first_stage),
        args: vec![receiver],
    })
}

fn validate_data_last_fallback_evidence(
    method: &str,
    arg_count: usize,
    arg_order: &[RuntimeDataLastMethodFallbackArg],
) -> Result<(), String> {
    let Some((last, first_stage)) = arg_order.split_last() else {
        return Err(format!(
            "data-last method fallback `{}` has empty staged argument evidence",
            runtime_method_name(method)
        ));
    };
    if last != &RuntimeDataLastMethodFallbackArg::Receiver {
        return Err(format!(
            "data-last method fallback `{}` must apply its receiver in a separate final stage",
            runtime_method_name(method)
        ));
    }
    let mut seen = vec![false; arg_count];
    for arg in first_stage {
        let RuntimeDataLastMethodFallbackArg::CallArg { index } = arg else {
            return Err(format!(
                "data-last method fallback `{}` contains an early receiver stage",
                runtime_method_name(method)
            ));
        };
        let Some(seen) = seen.get_mut(*index) else {
            return Err(format!(
                "data-last method fallback `{}` referenced missing argument #{}",
                runtime_method_name(method),
                index
            ));
        };
        if std::mem::replace(seen, true) {
            return Err(format!(
                "data-last method fallback `{}` referenced argument #{} more than once",
                runtime_method_name(method),
                index
            ));
        }
    }
    if seen.into_iter().any(|seen| !seen) {
        return Err(format!(
            "data-last method fallback `{}` omitted a first-stage source argument",
            runtime_method_name(method)
        ));
    }
    Ok(())
}

fn lower_data_last_fallback_first_stage(
    method: &str,
    args: &[CallArg],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    let method = runtime_method_name(method);
    if args.iter().any(|arg| matches!(arg, CallArg::Named { .. })) {
        if let Some(helper) = helpers.and_then(|helpers| helpers.helper(method)) {
            return lower_strict_pure_helper_named_call(method, args, helper, helpers);
        }
        if let Some(candidate) =
            helpers.and_then(|helpers| helpers.function_value_candidate(method))
        {
            return lower_strict_function_value_named_call(method, args, candidate, helpers);
        }
        return Err(format!(
            "data-last method fallback `{method}` has named arguments but no executable callable metadata"
        ));
    }

    let callee = helpers
        .and_then(|helpers| helpers.value_expr(method))
        .ok_or_else(|| {
            format!("data-last method fallback `{method}` has no executable function value")
        })?;
    if args.is_empty() {
        return Ok(callee);
    }
    let args = args
        .iter()
        .map(|arg| lower_strict_call_arg(arg, helpers))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RuntimeExpr::Apply {
        callee: Box::new(callee),
        args,
    })
}

fn lower_runtime_pipe_expr(lhs: &Expr, rhs: &Expr) -> RuntimeExpr {
    lower_runtime_pipe_expr_strict(lhs, rhs, Some(empty_runtime_lookup()), None)
        .unwrap_or_else(|error| RuntimeExpr::Value(RuntimeValue::String(error)))
}

fn lower_runtime_pipe_expr_strict(
    lhs: &Expr,
    rhs: &Expr,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
    expression_id: Option<RuntimeTypedExpressionId>,
) -> Result<RuntimeExpr, String> {
    if rhs.contains_pipe_left() {
        let lhs = lower_runtime_expr_strict_with_helpers(lhs, helpers)?;
        let scoped_helpers = helpers
            .unwrap_or_else(empty_runtime_lookup)
            .enter_pipe_binding();
        let binding = scoped_helpers
            .pipe_binding_name()
            .expect("entered pipe scope has a binding");
        let body = lower_runtime_expr_strict_with_helpers(rhs, Some(scoped_helpers))?;
        return Ok(RuntimeExpr::Let {
            name: binding,
            expr: Box::new(lhs),
            body: Box::new(body),
        });
    }

    reject_data_last_path_partial_without_runtime_candidate(rhs, helpers, expression_id)?;

    // Reserve the outer binding before constructing the RHS. Nested pipes in a
    // returned closure then receive a deeper impossible-to-author name instead
    // of accidentally reusing this binding's lexical identity.
    let scoped_helpers = helpers
        .unwrap_or_else(empty_runtime_lookup)
        .enter_pipe_binding();
    let binding = scoped_helpers
        .pipe_binding_name()
        .expect("entered pipe scope has a binding");
    // Construct RHS first to consume semantic expression evidence in the same
    // order as type checking. The lexical let still evaluates the LHS first.
    let callee = lower_runtime_expr_strict_with_helpers(rhs, Some(scoped_helpers))?;
    let lhs = lower_runtime_expr_strict_with_helpers(lhs, helpers)?;
    Ok(RuntimeExpr::Let {
        name: binding.clone(),
        expr: Box::new(lhs),
        body: Box::new(RuntimeExpr::Apply {
            callee: Box::new(callee),
            args: vec![RuntimeExpr::Local(binding)],
        }),
    })
}

fn lower_runtime_bracket_seq(items: &[Expr]) -> RuntimeExpr {
    let lowered = items.iter().map(lower_runtime_expr).collect::<Vec<_>>();
    fold_value_sequence(lowered)
}

fn lower_runtime_numeric_bracket_seq(
    seq: &NumericBracketSeq,
    resolved_type: Option<RuntimeNumericType>,
) -> Result<RuntimeExpr, String> {
    let target = resolved_type.unwrap_or_else(|| {
        seq.suffix()
            .map_or(RuntimeNumericType::I32, RuntimeNumericType::from)
    });
    let value = match target {
        RuntimeNumericType::I8 => {
            dense_integer_sequence(seq, target, i8::try_from, runtime_sequence_dense_i8)
        }
        RuntimeNumericType::I16 => {
            dense_integer_sequence(seq, target, i16::try_from, runtime_sequence_dense_i16)
        }
        RuntimeNumericType::I32 => {
            dense_integer_sequence(seq, target, i32::try_from, runtime_sequence_dense_i32)
        }
        RuntimeNumericType::I64 => {
            dense_integer_sequence(seq, target, i64::try_from, runtime_sequence_dense_i64)
        }
        RuntimeNumericType::I128 => {
            dense_integer_sequence(seq, target, i128::try_from, runtime_sequence_dense_i128)
        }
        RuntimeNumericType::ISize => {
            dense_integer_sequence(seq, target, i64::try_from, runtime_sequence_dense_isize)
        }
        RuntimeNumericType::U8 => {
            dense_integer_sequence(seq, target, u8::try_from, runtime_sequence_dense_u8)
        }
        RuntimeNumericType::U16 => {
            dense_integer_sequence(seq, target, u16::try_from, runtime_sequence_dense_u16)
        }
        RuntimeNumericType::U32 => {
            dense_integer_sequence(seq, target, u32::try_from, runtime_sequence_dense_u32)
        }
        RuntimeNumericType::U64 => {
            dense_integer_sequence(seq, target, u64::try_from, runtime_sequence_dense_u64)
        }
        RuntimeNumericType::U128 => dense_integer_sequence(
            seq,
            target,
            Ok::<u128, std::convert::Infallible>,
            runtime_sequence_dense_u128,
        ),
        RuntimeNumericType::USize => {
            dense_integer_sequence(seq, target, u64::try_from, runtime_sequence_dense_usize)
        }
        RuntimeNumericType::F32 | RuntimeNumericType::F64 => {
            Err("integer sequence resolved to a float item type".to_owned())
        }
    }?;
    Ok(RuntimeExpr::Value(value))
}

fn lower_runtime_numeric_bracket_seq_lossy(seq: &NumericBracketSeq) -> RuntimeExpr {
    lower_runtime_numeric_bracket_seq(seq, None).unwrap_or_else(|_| {
        RuntimeExpr::Value(runtime_sequence_from_literal_values(
            seq.literals()
                .iter()
                .map(|literal| RuntimeValue::String(literal.raw().to_owned()))
                .collect(),
        ))
    })
}

fn lower_choice_action_call(callee: &Expr, args: &[CallArg]) -> Option<RuntimeExpr> {
    if expr_label(callee) != "choice_action" {
        return None;
    }
    let [CallArg::Positional(choice)] = args else {
        return Some(RuntimeExpr::Value(RuntimeValue::String(format!(
            "choice_action({})",
            args.iter()
                .map(call_arg_label)
                .collect::<Vec<_>>()
                .join(", ")
        ))));
    };
    let choice = expr_label(choice).trim_start_matches('@').to_owned();
    Some(RuntimeExpr::Record(vec![
        runtime_field_expr(
            "id",
            RuntimeExpr::Value(RuntimeValue::String(format!(
                "action.select_choice.{choice}"
            ))),
        ),
        runtime_field_expr("target", RuntimeExpr::Value(RuntimeValue::String(choice))),
        runtime_field_expr(
            "action",
            RuntimeExpr::Value(RuntimeValue::String("select_choice".to_owned())),
        ),
        runtime_field_expr(
            "kind",
            RuntimeExpr::Value(RuntimeValue::String("semantic".to_owned())),
        ),
        runtime_field_expr("enabled", RuntimeExpr::Value(RuntimeValue::Bool(true))),
    ]))
}

fn runtime_field_expr(name: &str, value: RuntimeExpr) -> RuntimeFieldExpr {
    RuntimeFieldExpr {
        name: name.to_owned(),
        value,
    }
}

fn dense_integer_sequence<T, E>(
    seq: &NumericBracketSeq,
    target: RuntimeNumericType,
    convert: impl Fn(u128) -> Result<T, E>,
    wrap: impl Fn(Vec<T>) -> RuntimeValue,
) -> Result<RuntimeValue, String> {
    seq.literals()
        .iter()
        .map(|literal| {
            literal
                .magnitude()
                .map_err(|error| format!("invalid integer literal `{}`: {error}", literal.raw()))
                .and_then(|magnitude| {
                    convert(magnitude).map_err(|_| {
                        format!(
                            "integer literal `{}` is out of range for `{}`",
                            literal.raw(),
                            target.as_str()
                        )
                    })
                })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(wrap)
}

fn lower_runtime_bracket_seq_strict(
    items: &[Expr],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    let lowered = items
        .iter()
        .map(|item| lower_runtime_expr_strict_with_helpers(item, helpers))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(fold_value_sequence(lowered))
}

fn lower_runtime_range_expr(
    start: Option<&Expr>,
    end: Option<&Expr>,
    inclusive: bool,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    Ok(RuntimeExpr::Range {
        start: start
            .map(|start| lower_runtime_expr_strict_with_helpers(start, helpers))
            .transpose()?
            .map(Box::new),
        end: end
            .map(|end| lower_runtime_expr_strict_with_helpers(end, helpers))
            .transpose()?
            .map(Box::new),
        inclusive,
    })
}

fn lower_runtime_range_expr_lossy(
    start: Option<&Expr>,
    end: Option<&Expr>,
    inclusive: bool,
) -> RuntimeExpr {
    RuntimeExpr::Range {
        start: start.map(lower_runtime_expr).map(Box::new),
        end: end.map(lower_runtime_expr).map(Box::new),
        inclusive,
    }
}

fn lower_runtime_record_expr_strict(
    fields: &[(String, Expr)],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    fields
        .iter()
        .map(|(name, value)| {
            Ok(RuntimeFieldExpr {
                name: name.clone(),
                value: lower_runtime_expr_strict_with_helpers(value, helpers)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()
        .map(RuntimeExpr::Record)
}

fn fold_value_sequence(items: Vec<RuntimeExpr>) -> RuntimeExpr {
    if !items
        .iter()
        .all(|item| matches!(item, RuntimeExpr::Value(_)))
    {
        return RuntimeExpr::BracketSeq(items);
    }
    RuntimeExpr::Value(runtime_sequence_from_literal_values(
        items
            .into_iter()
            .filter_map(|item| match item {
                RuntimeExpr::Value(value) => Some(value),
                _ => None,
            })
            .collect(),
    ))
}

fn lower_runtime_field_expr(target: &Expr, field: &str) -> RuntimeExpr {
    record_field_ordinal(target, field).map_or_else(
        || RuntimeExpr::Field {
            target: Box::new(lower_runtime_expr(target)),
            field: field.to_owned(),
        },
        |ordinal| RuntimeExpr::ProjectRecord {
            target: Box::new(lower_runtime_expr(target)),
            ordinal,
        },
    )
}

fn lower_enum_variant_field(target: &Expr, field: &str) -> Option<RuntimeExpr> {
    let path = target.dotted_selector_label()?;
    is_uppercase_path_segment(&path)
        .then_some(field)
        .filter(|field| is_uppercase_path_segment(field))
        .map(|field| RuntimeExpr::Variant {
            path: Some(path),
            name: field.to_owned(),
            payload: None,
        })
}

fn lower_strict_field_or_constant(
    expr: &Expr,
    target: &Expr,
    field: &str,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    if let Some(value) = lower_enum_variant_field(target, field) {
        return Ok(value);
    }
    lower_std_float_constant(expr)
        .map(RuntimeExpr::Value)
        .map_or_else(|| lower_strict_field_expr(target, field, helpers), Ok)
}

fn lower_strict_field_expr(
    target: &Expr,
    field: &str,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    let target_expr = lower_runtime_expr_strict_with_helpers(target, helpers)?;
    Ok(if let Some(ordinal) = record_field_ordinal(target, field) {
        RuntimeExpr::ProjectRecord {
            target: Box::new(target_expr),
            ordinal,
        }
    } else {
        RuntimeExpr::Field {
            target: Box::new(target_expr),
            field: field.to_owned(),
        }
    })
}

fn record_field_ordinal(target: &Expr, field: &str) -> Option<usize> {
    let (Expr::Record { fields, .. } | Expr::RecordLiteral(fields)) = target else {
        return None;
    };
    fields
        .iter()
        .position(|(candidate, _)| candidate.as_str() == field)
}

fn lower_runtime_index_expr(target: &Expr, index: &Expr) -> Option<RuntimeExpr> {
    tuple_index_ordinal(target, index).map_or_else(
        || {
            Some(RuntimeExpr::MethodCall {
                receiver: Box::new(lower_runtime_expr(target)),
                method: "__index".to_owned(),
                args: vec![lower_runtime_expr(index)],
            })
        },
        |ordinal| {
            Some(RuntimeExpr::ProjectTuple {
                target: Box::new(lower_runtime_expr(target)),
                ordinal,
            })
        },
    )
}

fn lower_strict_index_expr(
    target: &Expr,
    index: &Expr,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    tuple_index_ordinal(target, index).map_or_else(
        || {
            Ok(RuntimeExpr::MethodCall {
                receiver: Box::new(lower_runtime_expr_strict_with_helpers(target, helpers)?),
                method: "__index".to_owned(),
                args: vec![lower_runtime_expr_strict_with_helpers(index, helpers)?],
            })
        },
        |ordinal| {
            lower_runtime_expr_strict_with_helpers(target, helpers).map(|target| {
                RuntimeExpr::ProjectTuple {
                    target: Box::new(target),
                    ordinal,
                }
            })
        },
    )
}

fn tuple_index_ordinal(target: &Expr, index: &Expr) -> Option<usize> {
    let Expr::Tuple(items) = target else {
        return None;
    };
    let ordinal = array_repeat_len(index)?;
    (ordinal < items.len()).then_some(ordinal)
}

fn runtime_method_name(method: &str) -> &str {
    method.split_once('<').map_or(method, |(name, _)| name)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeExternalNamespace {
    Conv2d,
    Data,
    Infer,
}

impl RuntimeExternalNamespace {
    fn from_receiver(receiver: &str) -> Option<Self> {
        match receiver {
            "conv2d" => Some(Self::Conv2d),
            "data" => Some(Self::Data),
            "infer" => Some(Self::Infer),
            _ => None,
        }
    }

    const fn as_label_prefix(self) -> &'static str {
        match self {
            Self::Conv2d => "conv2d",
            Self::Data => "data",
            Self::Infer => "infer",
        }
    }

    fn call_label(self, method: &str) -> String {
        format!("{}.{}", self.as_label_prefix(), runtime_method_name(method))
    }
}

fn lower_runtime_external_namespace_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
) -> Option<RuntimeExpr> {
    let Expr::Path(receiver) = receiver else {
        return None;
    };
    RuntimeExternalNamespace::from_receiver(receiver).map(|namespace| RuntimeExpr::Call {
        callee: RuntimeCallTarget::from_label(namespace.call_label(method)),
        args: args.iter().map(lower_runtime_call_arg).collect(),
    })
}

fn lower_runtime_path_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
) -> Option<RuntimeExpr> {
    let Expr::Path(receiver) = receiver else {
        return None;
    };
    let method = runtime_method_name(method);
    if receiver != "path" || !matches!(method, "save" | "asset" | "temp" | "export") {
        return None;
    }
    let [arg] = args else {
        return None;
    };
    if arg.name().is_some() || arg.is_spread() {
        return None;
    }
    Some(RuntimeExpr::Call {
        callee: RuntimeCallTarget::from_label(format!("path.{method}")),
        args: vec![lower_runtime_expr(arg.value())],
    })
}

fn lower_runtime_selected_call(callee: &Expr, args: &[CallArg]) -> Option<RuntimeExpr> {
    let Expr::Select(select) = callee else {
        return None;
    };
    let receiver = select.target();
    let method = select.member().as_str();
    lower_runtime_math_method_call(receiver, method, args)
        .or_else(|| lower_runtime_std_float_method_call(receiver, method, args))
        .or_else(|| lower_runtime_path_method_call(receiver, method, args))
        .or_else(|| lower_runtime_external_namespace_method_call(receiver, method, args))
        .or_else(|| {
            Some(RuntimeExpr::MethodCall {
                receiver: Box::new(lower_runtime_expr(receiver)),
                method: runtime_method_name(method).to_owned(),
                args: args.iter().map(lower_runtime_call_arg).collect(),
            })
        })
}

fn lower_runtime_math_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
) -> Option<RuntimeExpr> {
    let Expr::Path(receiver) = receiver else {
        return None;
    };
    let method = runtime_method_name(method);
    if receiver != "math"
        || !matches!(
            method,
            "matmul_f32"
                | "matrix_add_f32"
                | "tensor_add_f32"
                | "matmul_f64"
                | "matrix_add_f64"
                | "tensor_add_f64"
        )
    {
        return None;
    }
    Some(RuntimeExpr::Call {
        callee: RuntimeCallTarget::from_label(format!("math.{method}")),
        args: args.iter().map(lower_runtime_call_arg).collect(),
    })
}

fn lower_runtime_std_float_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
) -> Option<RuntimeExpr> {
    let receiver = expr_label(receiver);
    let method = runtime_method_name(method);
    if !matches!(receiver.as_str(), "std.f32" | "std.f64")
        || RuntimeCallTarget::from_label(format!("{receiver}.{method}"))
            .as_intrinsic()
            .is_none()
    {
        return None;
    }
    Some(RuntimeExpr::Call {
        callee: RuntimeCallTarget::from_label(format!("{receiver}.{method}")),
        args: args.iter().map(lower_runtime_call_arg).collect(),
    })
}

fn lower_strict_path_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Option<Result<RuntimeExpr, String>> {
    let Expr::Path(receiver) = receiver else {
        return None;
    };
    let method = runtime_method_name(method);
    if receiver != "path" || !matches!(method, "save" | "asset" | "temp" | "export") {
        return None;
    }
    let [arg] = args else {
        return None;
    };
    if arg.name().is_some() || arg.is_spread() {
        return None;
    }
    Some(
        lower_runtime_expr_strict_with_helpers(arg.value(), helpers).map(|arg| RuntimeExpr::Call {
            callee: RuntimeCallTarget::from_label(format!("path.{method}")),
            args: vec![arg],
        }),
    )
}

fn lower_strict_math_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Option<Result<RuntimeExpr, String>> {
    let Expr::Path(receiver) = receiver else {
        return None;
    };
    let method = runtime_method_name(method);
    if receiver != "math"
        || !matches!(
            method,
            "matmul_f32"
                | "matrix_add_f32"
                | "tensor_add_f32"
                | "matmul_f64"
                | "matrix_add_f64"
                | "tensor_add_f64"
        )
    {
        return None;
    }
    Some(
        args.iter()
            .map(|arg| lower_strict_call_arg(arg, helpers))
            .collect::<Result<Vec<_>, _>>()
            .map(|args| RuntimeExpr::Call {
                callee: RuntimeCallTarget::from_label(format!("math.{method}")),
                args,
            }),
    )
}

fn lower_strict_std_float_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Option<Result<RuntimeExpr, String>> {
    let receiver = expr_label(receiver);
    let method = runtime_method_name(method);
    if !matches!(receiver.as_str(), "std.f32" | "std.f64")
        || RuntimeCallTarget::from_label(format!("{receiver}.{method}"))
            .as_intrinsic()
            .is_none()
    {
        return None;
    }
    Some(
        args.iter()
            .map(|arg| lower_strict_call_arg(arg, helpers))
            .collect::<Result<Vec<_>, _>>()
            .map(|args| RuntimeExpr::Call {
                callee: RuntimeCallTarget::from_label(format!("{receiver}.{method}")),
                args,
            }),
    )
}

fn lower_strict_external_namespace_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Option<Result<RuntimeExpr, String>> {
    let Expr::Path(receiver) = receiver else {
        return None;
    };
    RuntimeExternalNamespace::from_receiver(receiver).map(|namespace| {
        args.iter()
            .map(|arg| lower_strict_call_arg(arg, helpers))
            .collect::<Result<Vec<_>, _>>()
            .map(|args| RuntimeExpr::Call {
                callee: RuntimeCallTarget::from_label(namespace.call_label(method)),
                args,
            })
    })
}

fn lower_strict_call_expr(
    callee: &Expr,
    args: &[CallArg],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
    expression_id: Option<RuntimeTypedExpressionId>,
) -> Result<RuntimeExpr, String> {
    if let Some(lowered) = lower_agent_path_constructor_call(callee, args, helpers) {
        return lowered;
    }
    if let Some(lowered) = lower_choice_action_call(callee, args) {
        return Ok(lowered);
    }
    if let Some(lowered) = lower_strict_intrinsic_function_call(callee, args, helpers) {
        return lowered;
    }
    if let Some(lowered) = lower_constructor_call(callee, args, helpers) {
        return Ok(lowered);
    }
    if let Expr::Select(select) = callee {
        return lower_strict_method_call_dispatch(
            select.target(),
            select.member().as_str(),
            args,
            helpers,
            expression_id,
        );
    }
    let path_callee = match callee {
        Expr::Path(path) => Some(path.as_label()),
        _ => None,
    };
    if let Some(callee) = path_callee {
        reject_signature_partial_without_helper(callee, args.len(), helpers, expression_id)?;
    }
    if let Some(callee) = path_callee
        && args.iter().any(|arg| matches!(arg, CallArg::Named { .. }))
        && let Some(helper) = helpers.and_then(|helpers| helpers.helper(callee))
    {
        return lower_strict_pure_helper_named_call(callee, args, helper, helpers);
    }
    if let Some(callee) = path_callee
        && args.iter().any(|arg| matches!(arg, CallArg::Named { .. }))
        && let Some(candidate) =
            helpers.and_then(|helpers| helpers.function_value_candidate(callee))
    {
        return lower_strict_function_value_named_call(callee, args, candidate, helpers);
    }
    let args = match path_callee {
        Some(callee) => lower_strict_named_callee_args(callee, args, helpers)?,
        None => args
            .iter()
            .map(|arg| lower_strict_call_arg(arg, helpers))
            .collect::<Result<Vec<_>, _>>()?,
    };
    if helpers.is_some_and(|helpers| {
        helpers.has_function_value_call_evidence(expression_id, path_callee, args.len())
    }) {
        let callee = if let Some(path) = path_callee {
            RuntimeExpr::Local(path.to_owned())
        } else {
            lower_runtime_expr_strict_with_helpers(callee, helpers)?
        };
        return Ok(RuntimeExpr::Apply {
            callee: Box::new(callee),
            args,
        });
    }
    let Expr::Path(path) = callee else {
        return lower_runtime_expr_strict_with_helpers(callee, helpers).map(|callee| {
            RuntimeExpr::Apply {
                callee: Box::new(callee),
                args,
            }
        });
    };
    let callee = path.as_label();
    Ok(lower_strict_named_call(callee, args, helpers))
}

fn reject_signature_partial_without_helper(
    callee: &str,
    arg_count: usize,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
    expression_id: Option<RuntimeTypedExpressionId>,
) -> Result<(), String> {
    if helpers.is_some_and(|helpers| {
        helpers.has_signature_partial_call_evidence(expression_id, callee, arg_count)
            && helpers.helper(callee).is_none()
            && helpers.function_value_candidate(callee).is_none()
    }) {
        return Err(unsupported_signature_partial(callee));
    }
    Ok(())
}

fn reject_data_last_path_partial_without_runtime_candidate(
    rhs: &Expr,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
    expression_id: Option<RuntimeTypedExpressionId>,
) -> Result<(), String> {
    let Expr::Path(path) = rhs else {
        return Ok(());
    };
    let callee = path.as_label();
    if helpers.is_some_and(|helpers| {
        helpers.has_partial_function_value_call_evidence(expression_id, None, 1)
            && helpers.value_expr(callee).is_none()
    }) {
        return Err(unsupported_signature_partial(callee));
    }
    Ok(())
}

fn unsupported_signature_partial(callee: &str) -> String {
    format!(
        "unsupported callable family `signature_partial_without_helper`: function `{callee}` partial application requires executable helper lowering; effectful, suspending, ABI-backed, or otherwise non-helper top-level callable allocation is not implemented"
    )
}

fn lower_strict_named_callee_args(
    callee: &str,
    args: &[CallArg],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<Vec<RuntimeExpr>, String> {
    if args.iter().any(|arg| matches!(arg, CallArg::Named { .. }))
        && let Some(helper) = helpers.and_then(|helpers| helpers.helper(callee))
    {
        return lower_strict_named_callable_args(
            "pure helper",
            callee,
            args,
            &helper.input_names,
            helpers,
        )
        .and_then(|lowering| match lowering {
            PureHelperNamedCallLowering::Exact(args) => Ok(args),
            PureHelperNamedCallLowering::Partial(_) => Err(format!(
                "pure helper `{callee}` named partial call must lower as a function expression"
            )),
        });
    }
    args.iter()
        .map(|arg| lower_strict_call_arg(arg, helpers))
        .collect::<Result<Vec<_>, _>>()
}

fn lower_strict_named_call(
    callee: &str,
    args: Vec<RuntimeExpr>,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> RuntimeExpr {
    if helpers
        .and_then(|helpers| helpers.local_function_arity(callee))
        .is_some()
    {
        return RuntimeExpr::Apply {
            callee: Box::new(RuntimeExpr::Local(callee.to_owned())),
            args,
        };
    }
    if let Some(helper) = helpers.and_then(|helpers| helpers.id(callee)) {
        if helpers
            .and_then(|helpers| helpers.arity(callee))
            .is_some_and(|arity| arity != args.len())
            && let Some(function) = helpers.and_then(|helpers| helpers.function_value(callee))
        {
            RuntimeExpr::Apply {
                callee: Box::new(function),
                args,
            }
        } else {
            RuntimeExpr::PureCall { helper, args }
        }
    } else if let Some(function) = helpers.and_then(|helpers| helpers.function_value(callee)) {
        RuntimeExpr::Apply {
            callee: Box::new(function),
            args,
        }
    } else {
        RuntimeExpr::Call {
            callee: RuntimeCallTarget::from_label(callee),
            args,
        }
    }
}

fn lower_strict_intrinsic_function_call(
    callee: &Expr,
    args: &[CallArg],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Option<Result<RuntimeExpr, String>> {
    let label = expr_label(callee);
    if helpers
        .and_then(|helpers| helpers.local_function_arity(&label))
        .is_some()
        || helpers.and_then(|helpers| helpers.id(&label)).is_some()
        || helpers
            .and_then(|helpers| helpers.function_value_candidate(&label))
            .is_some()
    {
        return None;
    }
    let target = RuntimeCallTarget::from_label(label);
    target.as_intrinsic()?;
    Some(
        args.iter()
            .map(|arg| lower_strict_call_arg(arg, helpers))
            .collect::<Result<Vec<_>, _>>()
            .map(|args| RuntimeExpr::Call {
                callee: target,
                args,
            }),
    )
}

fn lower_agent_path_constructor_call(
    callee: &Expr,
    args: &[CallArg],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Option<Result<RuntimeExpr, String>> {
    if !matches!(
        expr_label(callee).as_str(),
        "state_path" | "observation_path"
    ) {
        return None;
    }
    Some(match args {
        [arg] if arg.name().is_none() && !arg.is_spread() => {
            lower_runtime_expr_strict_with_helpers(arg.value(), helpers)
        }
        _ => Err(format!(
            "{} requires exactly one positional path argument",
            expr_label(callee)
        )),
    })
}

fn lower_strict_method_call_expr(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    if let Some(map) = lower_strict_map_method_call(receiver, method, args, helpers) {
        return map;
    }
    if let Some(filter) = lower_strict_filter_method_call(receiver, method, args, helpers) {
        return filter;
    }
    if runtime_method_name(method) == "sum" && args.is_empty() {
        return lower_runtime_expr_strict_with_helpers(receiver, helpers).map(|source| {
            RuntimeExpr::Sum {
                source: Box::new(source),
            }
        });
    }
    if runtime_method_name(method) == "summary" && args.is_empty() {
        return lower_runtime_expr_strict_with_helpers(receiver, helpers).map(|source| {
            RuntimeExpr::Field {
                target: Box::new(source),
                field: "summary".to_owned(),
            }
        });
    }
    Ok(RuntimeExpr::MethodCall {
        receiver: Box::new(lower_runtime_expr_strict_with_helpers(receiver, helpers)?),
        method: runtime_method_name(method).to_owned(),
        args: args
            .iter()
            .map(|arg| lower_strict_call_arg(arg, helpers))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_strict_map_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Option<Result<RuntimeExpr, String>> {
    if runtime_method_name(method) != "map" {
        return None;
    }
    let [arg] = args else {
        return None;
    };
    if arg.name().is_some() || arg.is_spread() {
        return None;
    }
    if expr_contains_partial_placeholder(arg.value()) {
        const PARAM_NAME: &str = "_item";
        let scoped_helpers = helpers
            .unwrap_or_else(empty_runtime_lookup)
            .enter_partial_binding(PARAM_NAME);
        return Some(
            lower_runtime_expr_strict_with_helpers(receiver, helpers).and_then(|source| {
                lower_runtime_expr_strict_with_helpers(arg.value(), Some(scoped_helpers)).map(
                    |body| RuntimeExpr::Map {
                        source: Box::new(source),
                        param: PARAM_NAME.to_owned(),
                        body: Box::new(body),
                    },
                )
            }),
        );
    }
    let Expr::Closure { params, body, .. } = arg.value() else {
        return None;
    };
    let [param] = params.as_slice() else {
        return Some(Err(
            "runtime `map` closures must bind exactly one parameter".to_owned(),
        ));
    };
    let Some(param_name) = param.simple_ident() else {
        return Some(Err(
            "runtime `map` closure parameter must bind a simple identifier".to_owned(),
        ));
    };
    Some(
        lower_runtime_expr_strict_with_helpers(receiver, helpers).and_then(|source| {
            lower_runtime_expr_strict_with_helpers(body, helpers).map(|body| RuntimeExpr::Map {
                source: Box::new(source),
                param: param_name.to_owned(),
                body: Box::new(body),
            })
        }),
    )
}

fn lower_strict_filter_method_call(
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Option<Result<RuntimeExpr, String>> {
    if runtime_method_name(method) != "filter" {
        return None;
    }
    let [arg] = args else {
        return None;
    };
    if arg.name().is_some() || arg.is_spread() {
        return None;
    }
    if expr_contains_partial_placeholder(arg.value()) {
        const PARAM_NAME: &str = "_item";
        let scoped_helpers = helpers
            .unwrap_or_else(empty_runtime_lookup)
            .enter_partial_binding(PARAM_NAME);
        return Some(
            lower_runtime_expr_strict_with_helpers(receiver, helpers).and_then(|source| {
                lower_runtime_expr_strict_with_helpers(arg.value(), Some(scoped_helpers)).map(
                    |body| RuntimeExpr::Filter {
                        source: Box::new(source),
                        param: PARAM_NAME.to_owned(),
                        body: Box::new(body),
                    },
                )
            }),
        );
    }
    let Expr::Closure { params, body, .. } = arg.value() else {
        return None;
    };
    let [param] = params.as_slice() else {
        return Some(Err(
            "runtime `filter` closures must bind exactly one parameter".to_owned(),
        ));
    };
    let Some(param_name) = param.simple_ident() else {
        return Some(Err(
            "runtime `filter` closure parameter must bind a simple identifier".to_owned(),
        ));
    };
    Some(
        lower_runtime_expr_strict_with_helpers(receiver, helpers).and_then(|source| {
            lower_runtime_expr_strict_with_helpers(body, helpers).map(|body| RuntimeExpr::Filter {
                source: Box::new(source),
                param: param_name.to_owned(),
                body: Box::new(body),
            })
        }),
    )
}

fn lower_runtime_call_arg(arg: &CallArg) -> RuntimeExpr {
    match arg {
        CallArg::Spread { value } => RuntimeExpr::SpreadArg(Box::new(lower_runtime_expr(value))),
        value => lower_runtime_expr(value.value()),
    }
}

fn lower_strict_call_arg(
    arg: &CallArg,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    match arg {
        CallArg::Spread { value } => Ok(RuntimeExpr::SpreadArg(Box::new(
            lower_runtime_expr_strict_with_helpers(value, helpers)?,
        ))),
        value => lower_runtime_expr_strict_with_helpers(value.value(), helpers),
    }
}

fn unsupported_strict_runtime_expr(expr: &Expr) -> Result<RuntimeExpr, String> {
    Err(format!(
        "unsupported runtime value expression `{}`",
        expr_label(expr)
    ))
}

#[derive(Clone, Copy)]
enum RuntimeControlValue {
    Try,
    Await,
}

fn unsupported_runtime_control_value(value: RuntimeControlValue) -> Result<RuntimeExpr, String> {
    let message = match value {
        RuntimeControlValue::Try => {
            "try expression requires a runtime error-propagation boundary and cannot lower as a pure value"
        }
        RuntimeControlValue::Await => {
            "await expression requires suspension-aware statement lowering and cannot lower as a pure value"
        }
    };
    Err(message.to_owned())
}

fn lower_dialogue_call_value(
    plan: Option<&arcweft_lang_syntax::ast::line_plan::LinePlan>,
) -> RuntimeExpr {
    let Some(plan) = plan else {
        return RuntimeExpr::Value(RuntimeValue::Unit);
    };
    let Some(out) = plan.items().iter().find_map(|item| match item {
        LinePlanItem::Out(expr) => Some(expr),
        _ => None,
    }) else {
        return RuntimeExpr::Value(RuntimeValue::Unit);
    };
    match out {
        Expr::Tuple(items) => RuntimeExpr::Tuple(
            items
                .iter()
                .map(|item| RuntimeExpr::Value(RuntimeValue::String(expr_label(item))))
                .collect(),
        ),
        expr => RuntimeExpr::Value(RuntimeValue::String(expr_label(expr))),
    }
}

fn lower_strict_block_value(
    value: Option<&Expr>,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    value.map_or_else(
        || Ok(RuntimeExpr::Value(RuntimeValue::Unit)),
        |value| lower_runtime_expr_strict_with_helpers(value, helpers),
    )
}

fn lower_strict_block_expr(
    statements: &[Stmt],
    value: Option<&Expr>,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    let body = lower_strict_block_value(value, helpers)?;
    statements.iter().rev().try_fold(body, |body, statement| {
        lower_strict_block_statement(statement, body, helpers)
    })
}

fn lower_strict_block_statement(
    statement: &Stmt,
    body: RuntimeExpr,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    match statement {
        Stmt::Let {
            pattern, ty, expr, ..
        } => {
            let name = pattern
                .simple_binding_name()
                .ok_or_else(|| format!("unsupported runtime let pattern `{pattern:?}`"))?
                .to_owned();
            let expr = if ty
                .as_ref()
                .is_some_and(|ty| single_param_function_type(ty.value()))
                && expr_contains_partial_placeholder(expr)
            {
                lower_partial_placeholder_function_expr(expr, helpers)?
            } else {
                lower_runtime_expr_strict_with_helpers(expr, helpers)?
            };
            Ok(RuntimeExpr::Let {
                name,
                expr: Box::new(expr),
                body: Box::new(body),
            })
        }
        Stmt::Assign { target, expr } => {
            let (target, field) = lower_direct_assignment_target(target.expr(), helpers)?;
            Ok(RuntimeExpr::AssignField {
                target: Box::new(target),
                field,
                expr: Box::new(lower_runtime_expr_strict_with_helpers(
                    expr.expr(),
                    helpers,
                )?),
                body: Box::new(body),
            })
        }
        Stmt::Return { expr, .. } => lower_runtime_expr_strict_with_helpers(expr, helpers),
        other => Err(format!("unsupported runtime block statement `{other:?}`")),
    }
}

fn lower_direct_assignment_target(
    target: &Expr,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<(RuntimeExpr, String), String> {
    let Expr::Select(select) = target else {
        return Err(format!(
            "unsupported runtime assignment target `{}`: only direct record fields are executable",
            expr_label(target)
        ));
    };
    let receiver = lower_runtime_expr_strict_with_helpers(select.target(), helpers)?;
    let field = select.member().as_str().to_owned();
    match receiver {
        RuntimeExpr::Local(_) => Ok((receiver, field)),
        RuntimeExpr::Field { .. }
        | RuntimeExpr::ProjectTuple { .. }
        | RuntimeExpr::ProjectRecord { .. } => Err(format!(
            "unsupported runtime assignment target `{}`: nested assignment targets require a future lvalue model",
            expr_label(target)
        )),
        other => Err(format!(
            "unsupported runtime assignment receiver `{other}`: assignment requires a local record value"
        )),
    }
}

fn lower_strict_if_expr(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    Ok(RuntimeExpr::If {
        condition: Box::new(lower_runtime_expr_strict_with_helpers(condition, helpers)?),
        then_expr: Box::new(lower_runtime_expr_strict_with_helpers(
            then_branch,
            helpers,
        )?),
        else_expr: Box::new(
            else_branch.map_or(Ok(RuntimeExpr::Value(RuntimeValue::Unit)), |else_branch| {
                lower_runtime_expr_strict_with_helpers(else_branch, helpers)
            })?,
        ),
    })
}

fn lower_strict_if_let_expr(
    pattern: &Pattern,
    expr: &Expr,
    guard: Option<&Expr>,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    Ok(RuntimeExpr::IfLet {
        pattern: lower_runtime_pattern(pattern),
        expr: Box::new(lower_runtime_expr_strict_with_helpers(expr, helpers)?),
        guard: guard
            .map(|guard| lower_runtime_expr_strict_with_helpers(guard, helpers))
            .transpose()?
            .map(Box::new),
        then_expr: Box::new(lower_runtime_expr_strict_with_helpers(
            then_branch,
            helpers,
        )?),
        else_expr: Box::new(
            else_branch.map_or(Ok(RuntimeExpr::Value(RuntimeValue::Unit)), |else_branch| {
                lower_runtime_expr_strict_with_helpers(else_branch, helpers)
            })?,
        ),
    })
}

fn lower_strict_match_expr(
    scrutinee: &Expr,
    arms: &[MatchExprArm],
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    Ok(RuntimeExpr::Match {
        scrutinee: Box::new(lower_runtime_expr_strict_with_helpers(scrutinee, helpers)?),
        arms: arms
            .iter()
            .map(|arm| {
                Ok(RuntimeExprMatchArm {
                    pattern: lower_runtime_pattern(arm.pattern()),
                    guard: arm
                        .guard()
                        .map(|guard| lower_runtime_expr_strict_with_helpers(guard, helpers))
                        .transpose()?,
                    value: lower_runtime_expr_strict_with_helpers(arm.value(), helpers)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
    })
}

fn constructor_path(path: &str) -> Option<(Option<String>, String)> {
    if let Some(name) = path.strip_prefix('.')
        && is_uppercase_path_segment(name)
    {
        return Some((None, name.to_owned()));
    }
    if let Some((prefix, name)) = path.rsplit_once('.')
        && is_uppercase_path_segment(prefix)
        && is_uppercase_path_segment(name)
    {
        return Some((Some(prefix.to_owned()), name.to_owned()));
    }
    let (prefix, name) = path
        .rsplit_once("::")
        .map_or((None, path), |(prefix, name)| {
            (Some(prefix.to_owned()), name)
        });
    let is_known_std_variant = matches!(name, "Ok" | "Err" | "Some" | "None");
    let is_uppercase_variant = name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase());
    (is_known_std_variant || is_uppercase_variant).then(|| (prefix, name.to_owned()))
}

fn is_uppercase_path_segment(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
}

fn lower_runtime_literal(
    literal: &Literal,
    resolved_type: Option<RuntimeNumericType>,
) -> Result<RuntimeValue, String> {
    Ok(match literal {
        Literal::String(value) => {
            RuntimeValue::String(DecodedStringLiteral::from_raw_body(value).into_string())
        }
        Literal::Char { value, .. } => RuntimeValue::Char(*value),
        Literal::Int(literal) => lower_runtime_int_literal(literal, resolved_type)?,
        Literal::Float { raw, suffix } => match resolved_type.unwrap_or_else(|| {
            suffix.map_or(RuntimeNumericType::F64, |suffix| match suffix {
                FloatSuffix::F32 => RuntimeNumericType::F32,
                FloatSuffix::F64 => RuntimeNumericType::F64,
            })
        }) {
            RuntimeNumericType::F32 => RuntimeValue::F32(parse_f32_literal(raw, *suffix)?),
            RuntimeNumericType::F64 => RuntimeValue::F64(parse_f64_literal(raw, *suffix)?),
            target => {
                return Err(format!(
                    "float literal `{raw}` resolved to non-float runtime type {target:?}"
                ));
            }
        },
        Literal::UnitNumber { raw, .. } => RuntimeValue::String(raw.clone()),
        Literal::Bool(value) => RuntimeValue::Bool(*value),
        Literal::Duration { .. } => duration_expr(&Expr::Literal(literal.clone())).map_or_else(
            || RuntimeValue::String(literal_label(literal)),
            RuntimeValue::Duration,
        ),
    })
}

fn lower_runtime_int_literal(
    literal: &IntLiteral,
    resolved_type: Option<RuntimeNumericType>,
) -> Result<RuntimeValue, String> {
    let magnitude = literal
        .magnitude()
        .map_err(|error| format!("invalid integer literal `{}`: {error}", literal.raw()))?;
    let target = resolved_type.unwrap_or_else(|| {
        literal
            .suffix()
            .map_or(RuntimeNumericType::I32, RuntimeNumericType::from)
    });
    let out_of_range = || {
        format!(
            "integer literal `{}` is out of range for `{}`",
            literal.raw(),
            target.as_str()
        )
    };
    match target {
        RuntimeNumericType::I8 => i8::try_from(magnitude)
            .map(RuntimeValue::i8)
            .map_err(|_| out_of_range()),
        RuntimeNumericType::I16 => i16::try_from(magnitude)
            .map(RuntimeValue::i16)
            .map_err(|_| out_of_range()),
        RuntimeNumericType::I32 => i32::try_from(magnitude)
            .map(RuntimeValue::i32)
            .map_err(|_| out_of_range()),
        RuntimeNumericType::I64 => i64::try_from(magnitude)
            .map(RuntimeValue::i64)
            .map_err(|_| out_of_range()),
        RuntimeNumericType::I128 => i128::try_from(magnitude)
            .map(RuntimeValue::i128)
            .map_err(|_| out_of_range()),
        RuntimeNumericType::ISize => i64::try_from(magnitude)
            .map(RuntimeValue::isize)
            .map_err(|_| out_of_range()),
        RuntimeNumericType::U8 => u8::try_from(magnitude)
            .map(RuntimeValue::u8)
            .map_err(|_| out_of_range()),
        RuntimeNumericType::U16 => u16::try_from(magnitude)
            .map(RuntimeValue::u16)
            .map_err(|_| out_of_range()),
        RuntimeNumericType::U32 => u32::try_from(magnitude)
            .map(RuntimeValue::u32)
            .map_err(|_| out_of_range()),
        RuntimeNumericType::U64 => u64::try_from(magnitude)
            .map(RuntimeValue::u64)
            .map_err(|_| out_of_range()),
        RuntimeNumericType::U128 => Ok(RuntimeValue::u128(magnitude)),
        RuntimeNumericType::USize => u64::try_from(magnitude)
            .map(RuntimeValue::usize)
            .map_err(|_| out_of_range()),
        RuntimeNumericType::F32 | RuntimeNumericType::F64 => Err(format!(
            "integer literal `{}` resolved to a float runtime type",
            literal.raw()
        )),
    }
}

fn lower_std_float_constant(expr: &Expr) -> Option<RuntimeValue> {
    Some(match expr_label(expr).as_str() {
        "std.f32.nan" => RuntimeValue::F32(f32::NAN),
        "std.f32.infinity" => RuntimeValue::F32(f32::INFINITY),
        "std.f32.neg_infinity" => RuntimeValue::F32(f32::NEG_INFINITY),
        "std.f32.epsilon" => RuntimeValue::F32(f32::EPSILON),
        "std.f32.min" => RuntimeValue::F32(f32::MIN),
        "std.f32.max" => RuntimeValue::F32(f32::MAX),
        "std.f32.pi" => RuntimeValue::F32(std::f32::consts::PI),
        "std.f32.tau" => RuntimeValue::F32(std::f32::consts::TAU),
        "std.f64.nan" => RuntimeValue::F64(f64::NAN),
        "std.f64.infinity" => RuntimeValue::F64(f64::INFINITY),
        "std.f64.neg_infinity" => RuntimeValue::F64(f64::NEG_INFINITY),
        "std.f64.epsilon" => RuntimeValue::F64(f64::EPSILON),
        "std.f64.min" => RuntimeValue::F64(f64::MIN),
        "std.f64.max" => RuntimeValue::F64(f64::MAX),
        "std.f64.pi" => RuntimeValue::F64(std::f64::consts::PI),
        "std.f64.tau" => RuntimeValue::F64(std::f64::consts::TAU),
        _ => return None,
    })
}

fn normalized_float_source(raw: &str, suffix: Option<FloatSuffix>) -> String {
    let number = suffix
        .and_then(|suffix| raw.strip_suffix(suffix.as_str()))
        .unwrap_or(raw);
    number.chars().filter(|ch| *ch != '_').collect::<String>()
}

fn parse_f32_literal(raw: &str, suffix: Option<FloatSuffix>) -> Result<f32, String> {
    normalized_float_source(raw, suffix)
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("float literal `{raw}` is invalid or overflows f32"))
}

fn parse_f64_literal(raw: &str, suffix: Option<FloatSuffix>) -> Result<f64, String> {
    normalized_float_source(raw, suffix)
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("float literal `{raw}` is invalid or overflows f64"))
}

fn negated_signed_min_value(
    expr: &Expr,
    resolved_type: Option<RuntimeNumericType>,
) -> Option<RuntimeValue> {
    let Expr::Literal(Literal::Int(literal)) = expr else {
        return None;
    };
    let magnitude = literal.magnitude().ok()?;
    let target = resolved_type.unwrap_or_else(|| {
        literal
            .suffix()
            .map_or(RuntimeNumericType::I32, RuntimeNumericType::from)
    });
    match (target, magnitude) {
        (RuntimeNumericType::I8, 128) => Some(RuntimeValue::i8(i8::MIN)),
        (RuntimeNumericType::I16, 32_768) => Some(RuntimeValue::i16(i16::MIN)),
        (RuntimeNumericType::I32, 2_147_483_648) => Some(RuntimeValue::i32(i32::MIN)),
        (RuntimeNumericType::I64, 9_223_372_036_854_775_808) => Some(RuntimeValue::i64(i64::MIN)),
        (RuntimeNumericType::ISize, 9_223_372_036_854_775_808) => {
            Some(RuntimeValue::isize(i64::MIN))
        }
        (RuntimeNumericType::I128, magnitude) if magnitude == (i128::MAX as u128) + 1 => {
            Some(RuntimeValue::i128(i128::MIN))
        }
        _ => None,
    }
}

fn lower_strict_negated_signed_min(
    expr: &Expr,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Option<RuntimeValue> {
    let Expr::Unary {
        op: UnaryOp::Neg,
        expr,
    } = expr
    else {
        return None;
    };
    let child_target =
        helpers.and_then(|helpers| helpers.resolved_numeric_type(helpers.current_expression_id()));
    let value = negated_signed_min_value(expr, child_target)?;
    helpers.and_then(RuntimePureHelperLookup::next_expression_id);
    Some(value)
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

fn lower_runtime_array_repeat(value: &Expr, len: &Expr) -> RuntimeExpr {
    let Some(len) = array_repeat_len(len) else {
        return RuntimeExpr::Value(RuntimeValue::String(expr_label(&Expr::ArrayRepeat {
            value: Box::new(value.clone()),
            len: Box::new(len.clone()),
        })));
    };
    let value = lower_runtime_expr(value);
    repeated_runtime_expr(value, len)
}

fn lower_runtime_array_repeat_strict(
    value: &Expr,
    len: &Expr,
    helpers: Option<RuntimePureHelperLookup<'_, '_, '_>>,
) -> Result<RuntimeExpr, String> {
    let Some(len) = array_repeat_len(len) else {
        return Err(format!(
            "array repeat length must be an integer constant in `{}`",
            expr_label(len)
        ));
    };
    lower_runtime_expr_strict_with_helpers(value, helpers)
        .map(|value| repeated_runtime_expr(value, len))
}

fn repeated_runtime_expr(value: RuntimeExpr, len: usize) -> RuntimeExpr {
    RuntimeExpr::RepeatSeq {
        value: Box::new(value),
        len,
    }
}

fn array_repeat_len(expr: &Expr) -> Option<usize> {
    match expr {
        Expr::Literal(Literal::Int(literal)) => literal
            .magnitude()
            .ok()
            .and_then(|value| usize::try_from(value).ok()),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
