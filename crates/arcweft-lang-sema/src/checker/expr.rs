//! Expression type-checking entry points and expression-kind dispatch.

use super::helpers::{
    array_len_matches, array_repeat_len_label, collection_index_type, expr_path_label,
    first_arg_type, is_drop_name, let_else_bindings, numeric_literal_suffix_type,
    optional_type_kind_label, result_ok_type, stmts_diverge, type_kind_label,
    well_known_capacity_method_type, well_known_field_type, well_known_static_capacity_method_type,
};
use super::{
    BorrowLocalState, BorrowStateDelta, EntityKind, EntityRefSyntax, Expr, FunctionSignature,
    Pattern, Stmt, TypeCheckError, TypeChecker, TypeExpressionId, TypeJudgmentRule,
    TypeJudgmentSubject, TypeKind, TypedLoweringEvidence, TypedLoweringEvidenceKind, YieldContext,
    entity_syntax_kind,
};
use crate::diagnostics::TraitDiagnostic;
use crate::traits::TraitMethodResolution;
use arcweft_lang_syntax::ast::dialogue::DialogueContent;
use arcweft_lang_syntax::ast::flow::{AuthoredExpr, ThreadBlock};
use arcweft_lang_syntax::ast::line_plan::LinePlan;
use arcweft_lang_syntax::expr::{
    BinaryOp, CallArg, ComputationBlockKind, Literal, MatchExprArm, Placeholder, SelectExpr,
    UnaryOp,
};
use arcweft_lang_syntax::reference::{BorrowExpr, DerefExpr};

mod agent;
mod binary;
mod builtin;
mod callable;
mod closure;
mod enum_variant;
mod fx;
mod method_fallback;
mod partial;
mod path;
mod pipe;
mod range;
mod reduction;
mod registered_call;
mod signature_call;
mod support;

use super::line_plan::DialogueContentRangeMode;
use builtin::BuiltinCallSpec;
use partial::expr_contains_partial_placeholder;
use support::{
    BuiltinCollectionMethodCallOutcome, ChoicePatternCoverage, TraitMethodCallOutcome,
    agent_action_result_field_type, agent_action_target_field_type, agent_bbox_field_type,
    agent_capture_ref_field_type, agent_entity_ref_field_type, agent_observation_field_type,
    agent_observed_object_field_type, agent_resource_body_field_type, agent_resource_field_type,
    agent_result, choice_pattern_coverage, collection_index_key_type, expr_kind_name,
    has_multiple_numeric_choice_alternatives, inline_failure_builtin_variant_type,
    is_character_speaker_type, is_unit_number_type, join_branch_types, looks_like_os_absolute_path,
    spread_item_type, std_float_constant_type, trait_method_call_signature,
    unique_numeric_choice_alternative,
};

enum InherentMethodCallOutcome {
    Missing,
    Checked(Option<TypeKind>),
}

impl TypeChecker<'_> {
    pub(super) fn expect_expr_type(&mut self, expr: &Expr, expected: &TypeKind, context: &str) {
        let actual = self.check_expr_with_expected(expr, Some(expected));
        if !actual
            .as_ref()
            .is_some_and(|actual| self.types_compatible(expected, actual))
        {
            let actual = optional_type_kind_label(actual.as_ref());
            self.errors.push(TypeCheckError::new(format!(
                "{context} must have type {}, found {actual}",
                type_kind_label(expected)
            )));
        }
    }

    pub(super) fn check_expr(&mut self, expr: &Expr) -> Option<TypeKind> {
        self.check_expr_with_expected(expr, None)
    }

    pub(super) fn check_expr_with_expected(
        &mut self,
        expr: &Expr,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        let expression_id = TypeExpressionId::from_index(self.stats.expressions);
        self.stats.expressions += 1;
        let ty = if let Some(expected @ TypeKind::Function { .. }) = expected
            && !matches!(expr, Expr::Closure { .. })
            && expr_contains_partial_placeholder(expr)
        {
            self.check_partial_placeholder_abstraction_expr(expr, expected)
        } else if expected.is_none()
            && self.current_partial_placeholder_type().is_none()
            && !matches!(expr, Expr::Closure { .. })
            && expr_contains_partial_placeholder(expr)
        {
            self.check_inferred_partial_placeholder_abstraction_expr(expr)
                .or_else(|| self.check_expr_kind_with_expected(expr, expected, expression_id))
        } else {
            self.check_expr_kind_with_expected(expr, expected, expression_id)
        };
        if let Some(ty) = ty.as_ref() {
            let resolved_numeric_target = match (expr, ty) {
                (Expr::Literal(Literal::Int(_) | Literal::Float { .. }), ty)
                    if ty.is_integer() || ty.is_float() =>
                {
                    Some(ty.clone())
                }
                (
                    Expr::NumericBracketSeq(_),
                    TypeKind::Vec(item) | TypeKind::Array { item, .. },
                ) if item.is_integer() => Some(item.as_ref().clone()),
                _ => None,
            };
            if let Some(target) = resolved_numeric_target {
                self.record_typed_lowering_evidence(TypedLoweringEvidence::new(
                    expression_id,
                    TypedLoweringEvidenceKind::ResolvedNumericType { target },
                ));
            }
            self.record_function_expr_effect_callable(expr, ty);
            let source_range = self.source_range_for_expr(expr);
            self.record_type_judgment_with_source_range(
                TypeJudgmentSubject::Expr {
                    id: expression_id,
                    kind: expr_kind_name(expr),
                },
                expected.map_or(TypeJudgmentRule::Expr, |_| TypeJudgmentRule::Expected),
                ty.clone(),
                expected,
                source_range,
            );
            if let Some(expected) = expected
                && let Some(arity) = expected.function_arity()
                && ty.function_arity().is_some()
            {
                self.record_typed_lowering_evidence(TypedLoweringEvidence::new(
                    expression_id,
                    TypedLoweringEvidenceKind::ExpectedFunctionValue {
                        expected_ty: expected.clone(),
                        actual_ty: ty.clone(),
                        arity,
                    },
                ));
            } else if expected.is_none()
                && expr_contains_partial_placeholder(expr)
                && let Some(arity) = ty.function_arity()
            {
                self.record_typed_lowering_evidence(TypedLoweringEvidence::new(
                    expression_id,
                    TypedLoweringEvidenceKind::ExpectedFunctionValue {
                        expected_ty: ty.clone(),
                        actual_ty: ty.clone(),
                        arity,
                    },
                ));
            }
        }
        ty
    }

    fn check_expr_kind_with_expected(
        &mut self,
        expr: &Expr,
        expected: Option<&TypeKind>,
        expression_id: TypeExpressionId,
    ) -> Option<TypeKind> {
        match expr {
            Expr::Literal(literal) => {
                Some(self.check_literal_expr(literal, expected, expression_id))
            }
            Expr::EntityRef(entity) => {
                self.check_entity_ref_expr(entity, expected, self.source_range_for_expr(expr))
            }
            Expr::LifetimePath { key, optional } => self.check_lifetime_path_expr(key, *optional),
            Expr::Path(path) => {
                self.check_path_expr_with_expected(path.as_label(), expected, expression_id)
            }
            Expr::ShortVariant(name) => {
                Some(self.check_short_variant_expr(name.as_str(), expected))
            }
            Expr::Placeholder(placeholder) => self.check_placeholder_expr(*placeholder),
            Expr::Tuple(items) => Some(self.check_tuple_expr_with_expected(items, expected)),
            Expr::BracketSeq(items) => Some(self.check_bracket_seq_with_expected(items, expected)),
            Expr::NumericBracketSeq(seq) => {
                Some(self.check_numeric_bracket_seq_summary(seq, expected, expression_id))
            }
            Expr::ArrayRepeat { value, len } => {
                Some(self.check_array_repeat_expr(value, len, expected))
            }
            Expr::Call { callee, args } => {
                self.check_call_expr(callee, args, expected, expression_id)
            }
            Expr::Select(select) => self.check_select_expr(expr, select),
            Expr::DialogueCall {
                callee,
                content,
                plan,
            } => Some(self.check_dialogue_call_expr(callee, content, plan.as_ref())),
            Expr::Index { target, index } => self.check_index_expr(target, index),
            Expr::Pipe { lhs, rhs } => self.check_pipe_expr(lhs, rhs, expression_id),
            Expr::Try { expr } => self.check_try_expr(expr),
            Expr::Await { expr, applies_try } => self.check_await_expr_node(expr, *applies_try),
            Expr::Thread { block } => Some(self.check_thread_expr(block)),
            Expr::Range { start, end, .. } => {
                Some(self.check_range_expr(start.as_deref(), end.as_deref(), expected))
            }
            Expr::Record { path, fields } => Some(self.check_record_expr(path, fields, expected)),
            Expr::RecordLiteral(fields) => Some(self.check_record_literal_expr(fields)),
            Expr::Binary { lhs, op, rhs } => self.check_binary_expr(lhs, *op, rhs, expected),
            Expr::Closure {
                params,
                return_type,
                body,
            } => Some(self.check_closure_expr(
                params,
                return_type.as_ref(),
                body,
                expected,
                expression_id,
            )),
            Expr::Unary { op, expr } => Some(self.check_unary_expr(*op, expr, expected)),
            Expr::Borrow(borrow) => self.check_borrow_expr(borrow),
            Expr::Deref(deref) => self.check_deref_expr(deref),
            Expr::Block { statements, value } => {
                self.check_block_expr_with_expected(statements, value.as_deref(), expected)
            }
            Expr::ComputationBlock {
                kind,
                statements,
                value,
            } => self.check_computation_block(*kind, statements, value.as_deref()),
            Expr::NamedBlock {
                statements, value, ..
            } => self.check_block_expr_with_expected(statements, value.as_deref(), expected),
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => self.check_if_expr(condition, then_branch, else_branch.as_deref(), expected),
            Expr::IfLet {
                pattern,
                expr,
                guard,
                then_branch,
                else_branch,
            } => self.check_if_let_expr(
                pattern,
                expr,
                guard.as_deref(),
                then_branch,
                else_branch.as_deref(),
                expected,
            ),
            Expr::Match { scrutinee, arms } => self.check_match_expr(scrutinee, arms, expected),
            Expr::Raw(raw) => {
                self.errors.push(TypeCheckError::new(format!(
                    "raw expression is not type-checkable: {raw}"
                )));
                None
            }
        }
    }

    fn check_borrow_expr(&mut self, borrow: &BorrowExpr) -> Option<TypeKind> {
        self.check_expr(borrow.operand())
            .map(|inner| TypeKind::BorrowRef {
                kind: borrow.kind(),
                lifetime: None,
                inner: Box::new(inner),
            })
    }

    fn check_deref_expr(&mut self, deref: &DerefExpr) -> Option<TypeKind> {
        match self.check_expr(deref.operand()) {
            Some(TypeKind::BorrowRef { inner, .. }) => Some(*inner),
            Some(other) => {
                self.errors.push(TypeCheckError::new(format!(
                    "dereference operand must be a reference, found {}",
                    type_kind_label(&other)
                )));
                None
            }
            None => None,
        }
    }

    fn in_seq_context(&self) -> bool {
        self.yield_stack
            .last()
            .is_some_and(|context| matches!(context, YieldContext::Seq { .. }))
    }

    fn check_await_expr_node(&mut self, expr: &Expr, applies_try: bool) -> Option<TypeKind> {
        self.record_static_effect("control.suspend", "await");
        if self.in_seq_context() {
            self.errors.push(TypeCheckError::new(
                "`seq` blocks are pure and cannot await".to_owned(),
            ));
        }
        self.check_await_expr(expr, applies_try)
    }

    fn check_thread_expr(&mut self, block: &ThreadBlock) -> TypeKind {
        self.record_static_effect("control.spawn", "thread");
        self.check_thread_body(block.body());
        TypeKind::ThreadHandle(Box::new(TypeKind::Unit))
    }

    fn check_entity_ref_expr(
        &mut self,
        entity: &EntityRefSyntax,
        expected: Option<&TypeKind>,
        range: Option<arcweft_lang_syntax::ast::common::TextRange>,
    ) -> Option<TypeKind> {
        if let (Some(module), Some(absolute), Some(range)) =
            (&self.current_module, entity.as_absolute(), range)
        {
            let delimiter = if absolute.is_delimited() { 2 } else { 1 };
            let start = range.start().saturating_add(delimiter);
            let end = start.saturating_add(absolute.body().len());
            if end <= range.end() {
                self.project_entity_references
                    .push(super::ProjectEntityReference {
                        module: module.clone(),
                        name: absolute.body().to_owned(),
                        range: arcweft_lang_syntax::ast::common::TextRange::new(start, end),
                    });
            }
        }
        if let Some(ty) = self.symbol_type(entity.body()).cloned() {
            return Some(ty);
        }
        entity_syntax_kind(entity)
            .map(TypeKind::entity_ref)
            .or_else(|| {
                expected
                    .filter(|ty| matches!(ty, TypeKind::Ref(_)))
                    .cloned()
            })
            .or_else(|| {
                self.errors.push(TypeCheckError::new(format!(
                    "unknown entity reference kind: {}",
                    entity.body()
                )));
                None
            })
    }

    fn check_record_expr(
        &mut self,
        path: &str,
        fields: &[(String, Expr)],
        expected: Option<&TypeKind>,
    ) -> TypeKind {
        if let Some(expected) = expected
            && let Some(payload) = self.enum_variant_payload_for_path(expected, path)
        {
            self.check_enum_record_constructor_payload(path, fields, &payload);
            return expected.clone();
        }
        if let Some(expected_fields) = self.nominal_fields.get(path).cloned() {
            for (name, value) in fields {
                if let Some(expected) = expected_fields.get(name) {
                    self.expect_expr_type(
                        value,
                        expected,
                        &format!("record field `{path}.{name}`"),
                    );
                } else {
                    self.errors.push(TypeCheckError::new(format!(
                        "record `{path}` has no field `{name}`"
                    )));
                    self.check_expr(value);
                }
            }
            for required in expected_fields.keys() {
                if !fields.iter().any(|(name, _)| name == required) {
                    self.errors.push(TypeCheckError::new(format!(
                        "record `{path}` literal is missing field `{required}`"
                    )));
                }
            }
        } else {
            self.check_record_fields(fields);
        }
        TypeKind::Named(path.to_owned())
    }

    fn check_record_literal_expr(&mut self, fields: &[(String, Expr)]) -> TypeKind {
        self.check_record_fields(fields);
        TypeKind::Named("Record".to_owned())
    }

    fn check_dialogue_call_expr(
        &mut self,
        callee: &Expr,
        content: &DialogueContent,
        plan: Option<&LinePlan>,
    ) -> TypeKind {
        self.check_expr(callee);
        let marks = self.check_dialogue_content(
            content,
            false,
            DialogueContentRangeMode::PreRegisteredExpression,
        );
        if let Some(plan) = plan {
            self.line_mark_stack.push(marks);
            let output = self.check_line_plan_output_type(plan);
            self.line_mark_stack.pop();
            output.unwrap_or(TypeKind::Unit)
        } else {
            TypeKind::Unit
        }
    }

    fn check_tuple_expr_with_expected(
        &mut self,
        items: &[Expr],
        expected: Option<&TypeKind>,
    ) -> TypeKind {
        if items.is_empty() {
            return TypeKind::Unit;
        }
        let expected_items = match expected {
            Some(TypeKind::Tuple(expected_items)) if expected_items.len() == items.len() => {
                Some(expected_items.as_slice())
            }
            Some(TypeKind::Tuple(expected_items)) => {
                self.errors.push(TypeCheckError::new(format!(
                    "tuple expression length mismatch: expected {}, found {}",
                    expected_items.len(),
                    items.len()
                )));
                None
            }
            _ => None,
        };
        TypeKind::Tuple(
            items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| {
                    self.check_expr_with_expected(
                        item,
                        expected_items.and_then(|items| items.get(index)),
                    )
                })
                .collect(),
        )
    }

    fn check_bracket_seq_with_expected(
        &mut self,
        items: &[Expr],
        expected: Option<&TypeKind>,
    ) -> TypeKind {
        let expected_item = match expected {
            Some(TypeKind::Array { item, .. } | TypeKind::Vec(item)) => Some(item.as_ref()),
            _ => None,
        };
        let item_type = expected_item
            .and_then(|expected_item| {
                self.check_numeric_bracket_seq_fast_path(items, expected_item)
            })
            .unwrap_or_else(|| self.check_bracket_seq_item_type(items, expected_item));
        self.finish_bracket_seq_type(items.len(), item_type, expected)
    }

    fn check_bracket_seq_item_type(
        &mut self,
        items: &[Expr],
        expected_item: Option<&TypeKind>,
    ) -> TypeKind {
        let mut item_type = None;
        for item in items {
            let next_type = self
                .check_expr_with_expected(item, expected_item)
                .unwrap_or(TypeKind::Unit);
            let next_item_type = match expected_item {
                Some(expected) if self.types_compatible(expected, &next_type) => expected.clone(),
                _ => next_type,
            };
            match &item_type {
                Some(existing) if existing != &next_item_type => {
                    self.errors.push(TypeCheckError::new(format!(
                        "sequence literal items must have the same type, found {existing:?} and {next_item_type:?}"
                    )));
                }
                Some(_) => {}
                None => item_type = Some(next_item_type),
            }
        }
        item_type.unwrap_or(TypeKind::Unit)
    }

    fn check_numeric_bracket_seq_summary(
        &mut self,
        seq: &arcweft_lang_syntax::expr::NumericBracketSeq,
        expected: Option<&TypeKind>,
        expression_id: TypeExpressionId,
    ) -> TypeKind {
        let expected_item = match expected {
            Some(TypeKind::Array { item, .. } | TypeKind::Vec(item)) => Some(item.as_ref()),
            _ => None,
        };
        let item_type = if let Some(suffix) = seq.suffix() {
            TypeKind::from(suffix)
        } else if let Some(expected_item) = expected_item.filter(|ty| ty.is_integer()) {
            expected_item.clone()
        } else if let Some(expected_item) = expected_item
            && let Some(ty) = unique_numeric_choice_alternative(expected_item, TypeKind::is_integer)
        {
            ty
        } else if expected_item.is_some_and(|expected_item| {
            has_multiple_numeric_choice_alternatives(expected_item, TypeKind::is_integer)
        }) {
            self.errors.push(TypeCheckError::new(
                "unsuffixed integer sequence literal requires an expected integer item type"
                    .to_owned(),
            ));
            TypeKind::Named("_".to_owned())
        } else {
            self.record_numeric_fallback(
                expression_id,
                super::NumericFallbackKind::IntegerSequence,
                "integer sequence",
                TypeKind::I32,
            );
            TypeKind::I32
        };
        for literal in seq.literals() {
            self.validate_integer_literal(literal, &item_type);
        }
        self.finish_bracket_seq_type(seq.len(), item_type, expected)
    }

    fn check_numeric_bracket_seq_fast_path(
        &mut self,
        items: &[Expr],
        expected_item: &TypeKind,
    ) -> Option<TypeKind> {
        if items.is_empty() || !(expected_item.is_integer() || expected_item.is_float()) {
            return None;
        }
        if !items.iter().all(|item| {
            matches!(
                (item, expected_item.is_integer(), expected_item.is_float()),
                (Expr::Literal(Literal::Int(_)), true, _)
                    | (Expr::Literal(Literal::Float { .. }), _, true)
            )
        }) {
            return None;
        }

        let mut item_type = None;
        for item in items {
            let Expr::Literal(literal) = item else {
                return None;
            };
            let next_type = self.check_expected_numeric_literal(literal, expected_item);
            let next_item_type = if self.types_compatible(expected_item, &next_type) {
                expected_item.clone()
            } else {
                next_type
            };
            match &item_type {
                Some(existing) if existing != &next_item_type => {
                    self.errors.push(TypeCheckError::new(format!(
                        "sequence literal items must have the same type, found {existing:?} and {next_item_type:?}"
                    )));
                }
                Some(_) => {}
                None => item_type = Some(next_item_type),
            }
        }
        item_type
    }

    fn check_expected_numeric_literal(
        &mut self,
        literal: &Literal,
        expected_item: &TypeKind,
    ) -> TypeKind {
        match literal {
            Literal::Int(literal) if literal.suffix().is_none() && expected_item.is_integer() => {
                self.validate_integer_literal(literal, expected_item);
                expected_item.clone()
            }
            Literal::Float { suffix: None, .. } if expected_item.is_float() => {
                expected_item.clone()
            }
            Literal::Int(literal) => {
                let ty = TypeKind::from(
                    literal
                        .suffix()
                        .expect("unsuffixed integer expected case handled above"),
                );
                self.validate_integer_literal(literal, &ty);
                ty
            }
            Literal::Float {
                suffix: Some(suffix),
                ..
            } => {
                let Some(ty) = numeric_literal_suffix_type(Some(suffix.as_str())) else {
                    self.errors.push(TypeCheckError::new(format!(
                        "unknown float literal suffix `{suffix}`"
                    )));
                    return TypeKind::Named("_".to_owned());
                };
                if ty.is_float() || is_unit_number_type(&ty) {
                    ty
                } else {
                    self.errors.push(TypeCheckError::new(format!(
                        "float literal suffix must be a float type, found {}",
                        type_kind_label(&ty)
                    )));
                    TypeKind::Named("_".to_owned())
                }
            }
            Literal::UnitNumber { suffix, .. } => {
                numeric_literal_suffix_type(Some(suffix.as_str()))
                    .unwrap_or_else(|| TypeKind::Named("_".to_owned()))
            }
            _ => TypeKind::Named("_".to_owned()),
        }
    }

    fn finish_bracket_seq_type(
        &mut self,
        items_len: usize,
        item_type: TypeKind,
        expected: Option<&TypeKind>,
    ) -> TypeKind {
        if let Some(TypeKind::Array { item, len }) = expected {
            if !array_len_matches(len, items_len) {
                self.errors.push(TypeCheckError::new(format!(
                    "array literal length mismatch: expected {len}, found {items_len}"
                )));
            }
            if !self.types_compatible(item.as_ref(), &item_type) {
                self.errors.push(TypeCheckError::new(format!(
                    "array items must have type {}, found {}",
                    type_kind_label(item.as_ref()),
                    type_kind_label(&item_type)
                )));
            }
            return TypeKind::Array {
                item: item.clone(),
                len: len.clone(),
            };
        }
        if let Some(TypeKind::Vec(item)) = expected {
            if !self.types_compatible(item.as_ref(), &item_type) {
                self.errors.push(TypeCheckError::new(format!(
                    "vector items must have type {}, found {}",
                    type_kind_label(item.as_ref()),
                    type_kind_label(&item_type)
                )));
            }
            return TypeKind::Vec(item.clone());
        }
        TypeKind::Vec(Box::new(item_type))
    }

    fn check_array_repeat_expr(
        &mut self,
        value: &Expr,
        len: &Expr,
        expected: Option<&TypeKind>,
    ) -> TypeKind {
        let expected_item = match expected {
            Some(TypeKind::Array { item, .. }) => Some(item.as_ref()),
            _ => None,
        };
        let item_type = self
            .check_expr_with_expected(value, expected_item)
            .unwrap_or(TypeKind::Unit);
        let len_label = array_repeat_len_label(len).unwrap_or_else(|| {
            self.errors.push(TypeCheckError::new(
                "array repeat length must be an integer constant".to_owned(),
            ));
            "_".to_owned()
        });
        self.expect_expr_type(len, &TypeKind::I64, "array repeat length");

        if let Some(TypeKind::Array { item, len }) = expected {
            if len != &len_label && len_label != "_" {
                self.errors.push(TypeCheckError::new(format!(
                    "array repeat length mismatch: expected {len}, found {len_label}"
                )));
            }
            if !self.types_compatible(item.as_ref(), &item_type) {
                self.errors.push(TypeCheckError::new(format!(
                    "array repeat value must have type {}, found {}",
                    type_kind_label(item.as_ref()),
                    type_kind_label(&item_type)
                )));
            }
            return TypeKind::Array {
                item: item.clone(),
                len: len.clone(),
            };
        }

        TypeKind::Array {
            item: Box::new(item_type),
            len: len_label,
        }
    }

    fn check_record_fields(&mut self, fields: &[(String, Expr)]) {
        for (_, value) in fields {
            self.check_expr(value);
        }
    }

    fn check_call_expr(
        &mut self,
        callee: &Expr,
        args: &[CallArg],
        expected: Option<&TypeKind>,
        expression_id: TypeExpressionId,
    ) -> Option<TypeKind> {
        if let Some(name) = expr_path_label(callee)
            && self.fx.is_definition(&name)
        {
            self.errors.extend(self.fx.call_errors(&name, args));
        }
        if let Some(ty) = self.check_fx_constructor_call(callee, args) {
            return Some(ty);
        }
        if let Some(ty) = self.check_enum_variant_call_expr(callee, args, expected) {
            return Some(ty);
        }
        if let Some(ty) = self.check_builtin_call_expr(callee, args, expected) {
            return Some(ty);
        }
        if let Some(name) = expr_path_label(callee)
            && let Some(ty) = self.check_agent_intrinsic_call_name(&name, args)
        {
            return Some(ty);
        }
        if let Some(name) = expr_path_label(callee)
            && let Some(ty) = self.check_presentation_call(&name, args)
        {
            return Some(ty);
        }
        if let Some(name) = expr_path_label(callee)
            && let Some(ty) = well_known_static_capacity_method_type(&name)
        {
            self.check_untyped_function_args(&name, args);
            return Some(ty);
        }
        match self.check_registered_catalog_free_call(callee, args, expected, expression_id) {
            registered_call::RegisteredFreeCallOutcome::NotHandled => {}
            registered_call::RegisteredFreeCallOutcome::Checked(result) => return result,
        }
        if self.registered_world.is_none()
            && let Some(name) = expr_path_label(callee)
            && let Some(ty) = self.function_type(&name).cloned()
        {
            let signature = self.function_signature(&name).cloned();
            self.check_virtual_path_call(&name, args);
            if let Some(signature) = signature.filter(FunctionSignature::checks_args) {
                return Some(self.check_named_signature_call(
                    expression_id,
                    &name,
                    ty,
                    &signature,
                    args,
                    expected,
                ));
            }
            self.check_untyped_function_args(&name, args);
            self.check_function_effects(&name);
            self.last_checked_curried_signature_call = None;
            self.record_function_return_effect_result(&name, &ty);
            return Some(ty);
        }
        if let Expr::Path(name) = callee {
            return self.check_path_call_expr(name, args, expected, expression_id);
        }
        if let Expr::Select(select) = callee
            && let Some(ty) = self.check_selected_callee_call(select, args, expression_id)
        {
            return Some(ty);
        }
        let previous_closure_effect_callable = self.last_checked_closure_effect_callable.take();
        let previous_curried_signature_call = self.last_checked_curried_signature_call.take();
        let callee_ty = self.check_expr(callee);
        let callee_effect_callable = self.last_checked_closure_effect_callable.take();
        let callee_curried_signature_call = self.last_checked_curried_signature_call.take();
        self.last_checked_closure_effect_callable = previous_closure_effect_callable;
        self.last_checked_curried_signature_call = previous_curried_signature_call;
        match callee_ty {
            Some(TypeKind::Speaker(entity) | TypeKind::SpeakerPreset(entity)) => {
                for arg in args {
                    self.check_expr(arg.value());
                }
                Some(TypeKind::SpeakerPreset(entity))
            }
            Some(callee_ty @ TypeKind::Function { .. }) => {
                Some(self.check_known_function_value_call(
                    expression_id,
                    expr_path_label(callee).as_deref(),
                    callee_effect_callable,
                    callee_curried_signature_call.as_ref(),
                    args,
                    callee_ty,
                ))
            }
            other => {
                for arg in args {
                    self.check_expr(arg.value());
                }
                other
            }
        }
    }

    fn check_result_constructor_call(
        &mut self,
        constructor: &str,
        args: &[CallArg],
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        match constructor {
            "Ok" => {
                if let Some(expected @ TypeKind::Result { ok, .. }) = expected {
                    self.check_result_constructor_payload("Ok", args, ok);
                    return Some(expected.clone());
                }
                let arg_types = args
                    .iter()
                    .map(|arg| self.check_expr(arg.value()))
                    .collect::<Vec<_>>();
                Some(TypeKind::Result {
                    ok: Box::new(first_arg_type(&arg_types)),
                    error: Box::new(TypeKind::Named("_".to_owned())),
                })
            }
            "Err" => {
                if let Some(expected @ TypeKind::Result { error, .. }) = expected {
                    self.check_result_constructor_payload("Err", args, error);
                    return Some(expected.clone());
                }
                let arg_types = args
                    .iter()
                    .map(|arg| self.check_expr(arg.value()))
                    .collect::<Vec<_>>();
                Some(TypeKind::Result {
                    ok: Box::new(TypeKind::Named("_".to_owned())),
                    error: Box::new(first_arg_type(&arg_types)),
                })
            }
            _ => None,
        }
    }

    fn check_result_constructor_payload(
        &mut self,
        constructor: &str,
        args: &[CallArg],
        expected: &TypeKind,
    ) {
        if args.len() != 1 {
            self.errors.push(TypeCheckError::new(format!(
                "`{constructor}` requires exactly one positional payload"
            )));
        }
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    self.expect_expr_type(value, expected, &format!("{constructor} payload"));
                }
                CallArg::Named { name, value } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "`{constructor}` payload must be positional, got named `{name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "`{constructor}` payload cannot be spread"
                    )));
                    self.check_expr(value);
                }
            }
        }
    }

    fn check_builtin_call_expr(
        &mut self,
        callee: &Expr,
        args: &[CallArg],
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        let name = expr_path_label(callee)?;
        self.check_builtin_call_name(&name, args, expected)
    }

    fn check_builtin_call_name(
        &mut self,
        name: &str,
        args: &[CallArg],
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        match BuiltinCallSpec::resolve(name)? {
            BuiltinCallSpec::InlineFailureFallback => {
                Some(TypeKind::Named("InlineFailure".to_owned()))
            }
            BuiltinCallSpec::Color => {
                self.check_homogeneous_builtin_args(name, args, &TypeKind::String, 1);
                Some(TypeKind::Named("Color".to_owned()))
            }
            BuiltinCallSpec::FloatUnary => {
                self.check_homogeneous_builtin_args(name, args, &TypeKind::F32, 1);
                Some(TypeKind::F32)
            }
            BuiltinCallSpec::Never => {
                for arg in args {
                    self.check_expr(arg.value());
                }
                Some(TypeKind::Never)
            }
            BuiltinCallSpec::Reduction(kind) => {
                Some(self.check_reduction_constructor_call(kind, args, expected))
            }
            BuiltinCallSpec::Ensure => {
                self.check_assert_like_args(args, "ensure");
                Some(TypeKind::Unit)
            }
            BuiltinCallSpec::AssertLike => {
                self.check_assert_like_args(args, name);
                Some(TypeKind::Unit)
            }
            BuiltinCallSpec::Math(intrinsic) => {
                self.check_math_binary_args(args, intrinsic.operand_type());
                Some(intrinsic.return_type())
            }
            BuiltinCallSpec::StdFloat(intrinsic) => {
                let input = intrinsic.input_type();
                self.check_homogeneous_builtin_args(name, args, &input, intrinsic.arity());
                Some(intrinsic.output_type())
            }
            BuiltinCallSpec::Vector(arity) => {
                self.check_homogeneous_builtin_args(name, args, &TypeKind::F32, arity);
                Some(TypeKind::Named(format!("Vec{arity}")))
            }
        }
    }

    fn check_homogeneous_builtin_args(
        &mut self,
        name: &str,
        args: &[CallArg],
        expected: &TypeKind,
        arity: usize,
    ) {
        if args.len() != arity {
            self.errors.push(TypeCheckError::new(format!(
                "`{name}` expected {arity} positional argument(s), got {}",
                args.len()
            )));
        }
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    self.check_expr_with_expected(value, Some(expected));
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "`{name}` arguments must be positional, got named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "`{name}` arguments cannot be spread"
                    )));
                    self.check_expr(value);
                }
            }
        }
    }

    fn check_math_binary_args(&mut self, args: &[CallArg], type_name: &str) {
        if args.len() != 2 {
            self.errors.push(TypeCheckError::new(format!(
                "math kernel expected 2 positional arguments, got {}",
                args.len()
            )));
        }
        let expected = TypeKind::Named(type_name.to_owned());
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    self.check_expr_with_expected(value, Some(&expected));
                }
                CallArg::Named { name, value } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "math kernel arguments must be positional, got named `{name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "math kernel arguments cannot be spread".to_owned(),
                    ));
                    self.check_expr(value);
                }
            }
        }
    }

    fn check_assert_like_args(&mut self, args: &[CallArg], name: &str) {
        if let Some(condition) = args.first() {
            self.expect_expr_type(
                condition.value(),
                &TypeKind::Bool,
                &format!("{name} condition"),
            );
        } else {
            self.errors.push(TypeCheckError::new(format!(
                "{name} requires a condition argument"
            )));
        }
        for arg in args.iter().skip(1) {
            self.check_expr(arg.value());
        }
    }

    fn check_unary_expr(
        &mut self,
        op: UnaryOp,
        expr: &Expr,
        expected: Option<&TypeKind>,
    ) -> TypeKind {
        match op {
            UnaryOp::Not => {
                self.expect_expr_type(expr, &TypeKind::Bool, "not operand");
                TypeKind::Bool
            }
            UnaryOp::Neg => {
                let previous = self.allow_signed_min_literal;
                self.allow_signed_min_literal = matches!(expr, Expr::Literal(Literal::Int(_)));
                let operand_type = self.check_expr_with_expected(expr, expected);
                self.allow_signed_min_literal = previous;
                match operand_type {
                    Some(ty) if ty.is_signed_integer() || ty.is_float() => ty,
                    Some(TypeKind::Duration) => TypeKind::Duration,
                    other => {
                        self.errors.push(TypeCheckError::new(format!(
                            "negation operand must be a signed numeric type or Duration, found {}",
                            optional_type_kind_label(other.as_ref())
                        )));
                        TypeKind::Named("_".to_owned())
                    }
                }
            }
        }
    }

    fn check_selected_callee_call(
        &mut self,
        select: &SelectExpr,
        args: &[CallArg],
        expression_id: TypeExpressionId,
    ) -> Option<TypeKind> {
        let method = select.member().as_str();
        let method_name = method.split_once('<').map_or(method, |(name, _)| name);
        let receiver_type = self.check_expr(select.target());
        if is_drop_name(method_name) {
            for arg in args {
                self.check_expr(arg.value());
            }
            return Some(TypeKind::Unit);
        }
        receiver_type.and_then(|receiver_type| {
            self.check_typed_method_call(
                select.target(),
                &receiver_type,
                method_name,
                args,
                expression_id,
            )
        })
    }

    fn check_typed_method_call(
        &mut self,
        receiver: &Expr,
        receiver_type: &TypeKind,
        method_name: &str,
        args: &[CallArg],
        expression_id: TypeExpressionId,
    ) -> Option<TypeKind> {
        match self.check_inherent_method_call(receiver_type, method_name, args, expression_id) {
            InherentMethodCallOutcome::Missing => {}
            InherentMethodCallOutcome::Checked(return_type) => return return_type,
        }
        match self.check_trait_method_call(receiver_type, method_name, args) {
            TraitMethodCallOutcome::Missing => {}
            TraitMethodCallOutcome::Typed(return_type) => return Some(return_type),
            TraitMethodCallOutcome::Rejected => return None,
        }
        if let Some(return_type) = self.check_data_last_method_fallback(
            receiver,
            receiver_type,
            method_name,
            args,
            expression_id,
        ) {
            return Some(return_type);
        }
        self.check_untyped_method_args(args);
        if self.registered_world.is_none()
            && let Some(return_type) = self.env.method_type(receiver_type, method_name).cloned()
        {
            return Some(return_type);
        }
        self.errors.push(TypeCheckError::new(format!(
            "unknown method `{method_name}` on {}",
            type_kind_label(receiver_type)
        )));
        None
    }

    /// Resolves method families that are owned directly by the receiver type.
    ///
    /// This phase deliberately precedes visible trait methods and data-last
    /// callable fallback, preserving ordinary inherent-method shadowing.
    fn check_inherent_method_call(
        &mut self,
        receiver_type: &TypeKind,
        method_name: &str,
        args: &[CallArg],
        expression_id: TypeExpressionId,
    ) -> InherentMethodCallOutcome {
        if method_name == "traverse" {
            return InherentMethodCallOutcome::Checked(
                self.check_traverse_method_call(receiver_type, args),
            );
        }
        if method_name == "parallel" {
            return InherentMethodCallOutcome::Checked(
                self.check_parallel_method_call(receiver_type, args),
            );
        }
        match self.check_registered_catalog_method_call(
            receiver_type,
            method_name,
            args,
            expression_id,
        ) {
            registered_call::RegisteredMethodCallOutcome::NotHandled => {}
            registered_call::RegisteredMethodCallOutcome::Checked(return_type) => {
                return InherentMethodCallOutcome::Checked(return_type);
            }
        }
        if self.registered_world.is_none()
            && let Some(return_type) = self.check_env_method_call(receiver_type, method_name, args)
        {
            return InherentMethodCallOutcome::Checked(Some(return_type));
        }
        match self.check_builtin_collection_method_call(receiver_type, method_name, args) {
            BuiltinCollectionMethodCallOutcome::Missing => {}
            BuiltinCollectionMethodCallOutcome::Checked(return_type) => {
                return InherentMethodCallOutcome::Checked(return_type);
            }
        }
        if let Some(return_type) =
            self.check_presentation_handle_lifecycle_method(receiver_type, method_name, args)
        {
            return InherentMethodCallOutcome::Checked(Some(return_type));
        }
        if matches!(method_name, "clamp" | "min" | "max") && receiver_type.is_integer() {
            return InherentMethodCallOutcome::Checked(Some(
                self.check_integer_scalar_method_call(receiver_type.clone(), method_name, args),
            ));
        }
        match self.check_builtin_domain_method_call(receiver_type, method_name, args) {
            InherentMethodCallOutcome::Missing => {}
            checked @ InherentMethodCallOutcome::Checked(_) => return checked,
        }
        if let Some(return_type) =
            well_known_capacity_method_type(receiver_type, method_name, args.len())
        {
            let signature = FunctionSignature::return_only(return_type.clone());
            self.warn_if_data_last_method_fallback_shadowed(
                receiver_type,
                method_name,
                args,
                "inherent",
                &signature,
            );
            self.check_untyped_method_args(args);
            return InherentMethodCallOutcome::Checked(Some(return_type));
        }
        InherentMethodCallOutcome::Missing
    }

    /// Resolves Arcweft-owned domain values whose methods are part of the
    /// language/runtime surface rather than environment or trait metadata.
    fn check_builtin_domain_method_call(
        &mut self,
        receiver_type: &TypeKind,
        method_name: &str,
        args: &[CallArg],
    ) -> InherentMethodCallOutcome {
        if matches!(receiver_type, TypeKind::Named(name) if name == "FxSampleContext")
            && method_name == "ordinal_phase"
        {
            if !args.is_empty() {
                self.errors.push(TypeCheckError::new(
                    "FxSampleContext.ordinal_phase accepts no arguments".to_owned(),
                ));
                for arg in args {
                    self.check_expr(arg.value());
                }
            }
            return InherentMethodCallOutcome::Checked(Some(TypeKind::F32));
        }
        if method_name == "require_role" {
            return InherentMethodCallOutcome::Checked(
                self.check_agent_object_require_role_method_call(receiver_type, args),
            );
        }
        if method_name == "get" {
            return InherentMethodCallOutcome::Checked(
                self.check_map_get_method_call(receiver_type, args),
            );
        }
        if matches!(
            method_name,
            "eq" | "ne"
                | "not_eq"
                | "gt"
                | "greater"
                | "ge"
                | "greater_or_equal"
                | "lt"
                | "less"
                | "le"
                | "less_or_equal"
        ) && let TypeKind::Probe(inner) = receiver_type
        {
            return InherentMethodCallOutcome::Checked(Some(self.check_probe_compare_method(
                method_name,
                inner.as_ref(),
                args,
            )));
        }
        if receiver_type == &TypeKind::Named("Diagnostics".to_owned()) && method_name == "has_error"
        {
            return InherentMethodCallOutcome::Checked(Some(self.check_no_arg_method(
                "Diagnostics.has_error",
                args,
                TypeKind::Predicate,
            )));
        }
        if receiver_type == &TypeKind::RagContextPack && method_name == "summary" {
            return InherentMethodCallOutcome::Checked(Some(self.check_no_arg_method(
                "RagContextPack.summary",
                args,
                TypeKind::DisplayText,
            )));
        }
        if matches!(method_name, "context" | "with_context") {
            return InherentMethodCallOutcome::Checked(
                self.check_context_method_call(receiver_type.clone(), args),
            );
        }
        if method_name == "face" && is_character_speaker_type(receiver_type) {
            self.check_untyped_method_args(args);
            return InherentMethodCallOutcome::Checked(Some(TypeKind::CharacterPatch(
                EntityKind::Character,
            )));
        }
        if method_name == "say" && is_character_speaker_type(receiver_type) {
            self.check_untyped_method_args(args);
            return InherentMethodCallOutcome::Checked(Some(TypeKind::SpeakerPreset(
                EntityKind::Character,
            )));
        }
        InherentMethodCallOutcome::Missing
    }

    fn check_env_method_call(
        &mut self,
        receiver_type: &TypeKind,
        method_name: &str,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        let signature = self
            .env
            .method_signature(receiver_type, method_name)
            .cloned()?;
        self.warn_if_data_last_method_fallback_shadowed(
            receiver_type,
            method_name,
            args,
            "environment",
            &signature,
        );
        if signature.checks_args() {
            self.check_signature_call_args(method_name, &signature, args);
        } else {
            self.check_untyped_method_args(args);
        }
        Some(signature.return_type().clone())
    }

    fn check_presentation_handle_lifecycle_method(
        &mut self,
        receiver_type: &TypeKind,
        method_name: &str,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        if let TypeKind::Handle { name, .. } = receiver_type
            && matches!(
                method_name,
                "show" | "hide" | "unmount" | "release" | "destroy"
            )
        {
            return Some(self.check_no_arg_method(
                &format!("presentation handle `{name}` method `{method_name}`"),
                args,
                TypeKind::Unit,
            ));
        }
        if let TypeKind::Handle { name, .. } = receiver_type
            && name == "Overlay"
            && method_name == "pop"
        {
            return Some(self.check_no_arg_method(
                "presentation handle `Overlay` method `pop`",
                args,
                TypeKind::Unit,
            ));
        }
        None
    }

    fn check_builtin_collection_method_call(
        &mut self,
        receiver_type: &TypeKind,
        method_name: &str,
        args: &[CallArg],
    ) -> BuiltinCollectionMethodCallOutcome {
        match method_name {
            "len" => BuiltinCollectionMethodCallOutcome::Checked(
                self.check_sequence_len_method_call(receiver_type, args),
            ),
            "map" => BuiltinCollectionMethodCallOutcome::Checked(
                self.check_vec_map_method_call(receiver_type, args),
            ),
            "filter" => BuiltinCollectionMethodCallOutcome::Checked(
                self.check_vec_filter_method_call(receiver_type, args),
            ),
            "sum" => BuiltinCollectionMethodCallOutcome::Checked(
                self.check_vec_sum_method_call(receiver_type, args),
            ),
            "contains" => BuiltinCollectionMethodCallOutcome::Checked(Some(
                self.check_sequence_contains_method_call(receiver_type, args),
            )),
            _ => BuiltinCollectionMethodCallOutcome::Missing,
        }
    }

    fn check_trait_method_call(
        &mut self,
        receiver_type: &TypeKind,
        method_name: &str,
        args: &[CallArg],
    ) -> TraitMethodCallOutcome {
        match self.trait_catalog.resolve_method(
            receiver_type,
            method_name,
            &self.active_trait_predicates(),
        ) {
            TraitMethodResolution::Missing => TraitMethodCallOutcome::Missing,
            TraitMethodResolution::Inherent(method) => {
                let return_type = self.resolve_type_projection(method.return_type().clone());
                let signature = trait_method_call_signature(method.signature(), return_type);
                self.warn_if_data_last_method_fallback_shadowed(
                    receiver_type,
                    method_name,
                    args,
                    "inherent",
                    &signature,
                );
                self.check_signature_call_args(method_name, &signature, args);
                TraitMethodCallOutcome::Typed(signature.return_type().clone())
            }
            TraitMethodResolution::Unique {
                trait_id, method, ..
            } => {
                let return_type = self.resolve_type_projection(method.return_type().clone());
                let signature = trait_method_call_signature(method.signature(), return_type);
                let source = self.trait_catalog.trait_name(trait_id).map_or_else(
                    || "trait `<unknown-trait>`".to_owned(),
                    |name| format!("trait `{name}`"),
                );
                self.warn_if_data_last_method_fallback_shadowed(
                    receiver_type,
                    method_name,
                    args,
                    &source,
                    &signature,
                );
                self.check_signature_call_args(method_name, &signature, args);
                TraitMethodCallOutcome::Typed(signature.return_type().clone())
            }
            TraitMethodResolution::Ambiguous(candidates) => {
                self.errors.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::ambiguous_method(
                        method_name,
                        candidates
                            .iter()
                            .map(|candidate| candidate.trait_name.as_str())
                            .collect::<Vec<_>>(),
                    ),
                ));
                self.check_untyped_method_args(args);
                TraitMethodCallOutcome::Rejected
            }
        }
    }

    fn check_integer_scalar_method_call(
        &mut self,
        receiver_type: TypeKind,
        method_name: &str,
        args: &[CallArg],
    ) -> TypeKind {
        let expected_args = if method_name == "clamp" { 2 } else { 1 };
        if args.len() != expected_args {
            self.errors.push(TypeCheckError::new(format!(
                "integer {method_name} requires {expected_args} positional argument(s)"
            )));
            for arg in args {
                self.check_expr_with_expected(arg.value(), Some(&receiver_type));
            }
            return receiver_type;
        }
        for arg in args {
            if arg.name().is_some() || arg.is_spread() {
                self.errors.push(TypeCheckError::new(format!(
                    "integer {method_name} arguments must be positional"
                )));
            }
            self.expect_expr_type(
                arg.value(),
                &receiver_type,
                &format!("integer {method_name}"),
            );
        }
        receiver_type
    }

    fn check_probe_compare_method(
        &mut self,
        method_name: &str,
        expected: &TypeKind,
        args: &[CallArg],
    ) -> TypeKind {
        let [arg] = args else {
            self.errors.push(TypeCheckError::new(format!(
                "Probe.{method_name} requires exactly one positional argument"
            )));
            for arg in args {
                self.check_expr(arg.value());
            }
            return TypeKind::Predicate;
        };
        match arg {
            CallArg::Positional(value) => {
                self.expect_expr_type(
                    value,
                    expected,
                    &format!("Probe.{method_name} expected value"),
                );
            }
            CallArg::Named { name, value } => {
                self.errors.push(TypeCheckError::new(format!(
                    "Probe.{method_name} arguments must be positional, got named `{name}`"
                )));
                self.check_expr(value);
            }
            CallArg::Spread { value } => {
                self.errors.push(TypeCheckError::new(format!(
                    "Probe.{method_name} arguments cannot be spread"
                )));
                self.check_expr(value);
            }
        }
        TypeKind::Predicate
    }

    fn check_no_arg_method(
        &mut self,
        method_name: &str,
        args: &[CallArg],
        return_type: TypeKind,
    ) -> TypeKind {
        if !args.is_empty() {
            self.errors.push(TypeCheckError::new(format!(
                "{method_name} requires no arguments"
            )));
            for arg in args {
                self.check_expr(arg.value());
            }
        }
        return_type
    }

    fn check_context_method_call(
        &mut self,
        receiver_type: TypeKind,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        self.check_untyped_method_args(args);
        match receiver_type {
            TypeKind::Need { .. } => Some(receiver_type),
            TypeKind::Option(inner) => Some(TypeKind::Result {
                ok: inner,
                error: Box::new(TypeKind::Named("ArcError".to_owned())),
            }),
            TypeKind::Result { ok, .. } => Some(TypeKind::Result {
                ok,
                error: Box::new(TypeKind::Named("ArcError".to_owned())),
            }),
            _ => None,
        }
    }

    fn check_untyped_method_args(&mut self, args: &[CallArg]) {
        for arg in args {
            self.check_expr(arg.value());
        }
    }

    fn check_sequence_len_method_call(
        &mut self,
        receiver_type: &TypeKind,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        if !args.is_empty() {
            self.errors.push(TypeCheckError::new(
                "len does not accept arguments".to_owned(),
            ));
            for arg in args {
                self.check_expr(arg.value());
            }
            return None;
        }
        match receiver_type {
            TypeKind::String
            | TypeKind::Vec(_)
            | TypeKind::Seq(_)
            | TypeKind::Slice(_)
            | TypeKind::Array { .. } => Some(TypeKind::USize),
            other => {
                self.errors.push(TypeCheckError::new(format!(
                    "len receiver must be a string or iterable sequence, found {}",
                    type_kind_label(other)
                )));
                None
            }
        }
    }

    fn check_vec_sum_method_call(
        &mut self,
        receiver_type: &TypeKind,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        if !args.is_empty() {
            self.errors.push(TypeCheckError::new(
                "sum does not accept arguments".to_owned(),
            ));
            for arg in args {
                self.check_expr(arg.value());
            }
            return None;
        }
        match receiver_type {
            TypeKind::Vec(item)
            | TypeKind::Seq(item)
            | TypeKind::Slice(item)
            | TypeKind::Array { item, .. }
                if item.is_integer() =>
            {
                Some(TypeKind::I64)
            }
            TypeKind::Vec(item)
            | TypeKind::Seq(item)
            | TypeKind::Slice(item)
            | TypeKind::Array { item, .. } => {
                self.errors.push(TypeCheckError::new(format!(
                    "sum receiver items must be integers, found {item:?}"
                )));
                None
            }
            other => {
                self.errors.push(TypeCheckError::new(format!(
                    "sum receiver must be an iterable sequence, found {}",
                    type_kind_label(other)
                )));
                None
            }
        }
    }

    fn check_sequence_contains_method_call(
        &mut self,
        receiver_type: &TypeKind,
        args: &[CallArg],
    ) -> TypeKind {
        let Some(item) = spread_item_type(receiver_type) else {
            self.errors.push(TypeCheckError::new(format!(
                "contains receiver must be an iterable sequence, found {}",
                type_kind_label(receiver_type)
            )));
            for arg in args {
                self.check_expr(arg.value());
            }
            return TypeKind::Bool;
        };
        let [arg] = args else {
            self.errors.push(TypeCheckError::new(
                "contains requires exactly one positional argument".to_owned(),
            ));
            for arg in args {
                self.check_expr(arg.value());
            }
            return TypeKind::Bool;
        };
        match arg {
            CallArg::Positional(value) => {
                self.expect_expr_type(value, item, "contains item");
            }
            CallArg::Named { name, value } => {
                self.errors.push(TypeCheckError::new(format!(
                    "contains arguments must be positional, got named `{name}`"
                )));
                self.check_expr(value);
            }
            CallArg::Spread { value } => {
                self.errors.push(TypeCheckError::new(
                    "contains arguments cannot be spread".to_owned(),
                ));
                self.check_expr(value);
            }
        }
        TypeKind::Bool
    }

    fn check_map_get_method_call(
        &mut self,
        receiver_type: &TypeKind,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        let TypeKind::Map { key, value, .. } = receiver_type else {
            return None;
        };
        let [arg] = args else {
            self.errors.push(TypeCheckError::new(
                "get requires exactly one positional argument".to_owned(),
            ));
            for arg in args {
                self.check_expr(arg.value());
            }
            return Some(value.as_ref().clone());
        };
        match arg {
            CallArg::Positional(expr) => {
                self.expect_expr_type(expr, key.as_ref(), "map key");
            }
            CallArg::Named { name, value } => {
                self.errors.push(TypeCheckError::new(format!(
                    "get arguments must be positional, got named `{name}`"
                )));
                self.check_expr(value);
            }
            CallArg::Spread { value } => {
                self.errors.push(TypeCheckError::new(
                    "get arguments cannot be spread".to_owned(),
                ));
                self.check_expr(value);
            }
        }
        Some(value.as_ref().clone())
    }

    fn check_agent_object_require_role_method_call(
        &mut self,
        receiver_type: &TypeKind,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        let TypeKind::Vec(item) = receiver_type else {
            return None;
        };
        if item.as_ref() != &TypeKind::ObservedObject {
            return None;
        }
        let [arg] = args else {
            self.errors.push(TypeCheckError::new(
                "ObservedObject list require_role requires exactly one role string".to_owned(),
            ));
            for arg in args {
                self.check_expr(arg.value());
            }
            return Some(agent_result(TypeKind::ObservedObject));
        };
        match arg {
            CallArg::Positional(value) => {
                self.expect_expr_type(value, &TypeKind::String, "object role");
            }
            CallArg::Named { name, value } => {
                self.errors.push(TypeCheckError::new(format!(
                    "require_role arguments must be positional, got named `{name}`"
                )));
                self.check_expr(value);
            }
            CallArg::Spread { value } => {
                self.errors.push(TypeCheckError::new(
                    "require_role arguments cannot be spread".to_owned(),
                ));
                self.check_expr(value);
            }
        }
        Some(agent_result(TypeKind::ObservedObject))
    }

    fn check_traverse_method_call(
        &mut self,
        receiver_type: &TypeKind,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        let TypeKind::Vec(item) = receiver_type else {
            self.errors.push(TypeCheckError::new(format!(
                "traverse receiver must be Vec<T>, found {}",
                type_kind_label(receiver_type)
            )));
            return None;
        };
        let [arg] = args else {
            self.errors.push(TypeCheckError::new(
                "traverse requires exactly one task function".to_owned(),
            ));
            return None;
        };
        if arg.name().is_some() || arg.is_spread() {
            self.errors.push(TypeCheckError::new(
                "traverse task function must be a positional argument".to_owned(),
            ));
            return None;
        }
        let Some(function_name) = expr_path_label(arg.value()) else {
            self.errors.push(TypeCheckError::new(
                "traverse task function must be capability-qualified".to_owned(),
            ));
            return None;
        };
        let Some(TypeKind::Need { ready, error }) = self.function_type(&function_name).cloned()
        else {
            self.errors.push(TypeCheckError::new(format!(
                "traverse task function `{function_name}` must return Need<T, E>"
            )));
            return None;
        };
        if let Some(signature) = self.function_signature(&function_name).cloned()
            && let Some(first) = signature.params.first()
            && !self.types_compatible(&first.ty, item)
        {
            self.errors.push(TypeCheckError::new(format!(
                "traverse item type must match `{function_name}` first parameter {:?}, found {:?}",
                first.ty, item
            )));
        }
        self.check_function_effects(&function_name);
        Some(TypeKind::Need {
            ready: Box::new(TypeKind::Vec(ready)),
            error,
        })
    }

    fn check_parallel_method_call(
        &mut self,
        receiver_type: &TypeKind,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        let [arg] = args else {
            self.errors.push(TypeCheckError::new(
                "parallel requires exactly `limit = N`".to_owned(),
            ));
            return None;
        };
        if arg.name() != Some("limit") || arg.is_spread() {
            self.errors.push(TypeCheckError::new(
                "parallel requires a named `limit = N` argument".to_owned(),
            ));
            self.check_expr(arg.value());
            return None;
        }
        self.expect_expr_type(arg.value(), &TypeKind::I64, "parallel limit");
        match receiver_type {
            TypeKind::Need { .. } => Some(receiver_type.clone()),
            other => {
                self.errors.push(TypeCheckError::new(format!(
                    "parallel receiver must be Need<Vec<T>, E>, found {}",
                    type_kind_label(other)
                )));
                None
            }
        }
    }

    fn check_index_expr(&mut self, target: &Expr, index: &Expr) -> Option<TypeKind> {
        let target_type = self.check_expr(target);
        if let Some(expected_index) = target_type
            .as_ref()
            .and_then(collection_index_key_type)
            .or_else(|| {
                target_type
                    .as_ref()
                    .and_then(|target_type| self.env.index_type(target_type).map(|_| TypeKind::I64))
            })
        {
            self.expect_expr_type(index, &expected_index, "collection index");
        } else {
            self.check_expr(index);
        }
        target_type.and_then(|target_type| {
            collection_index_type(&target_type)
                .or_else(|| self.env.index_type(&target_type).cloned())
                .or_else(|| {
                    self.errors.push(TypeCheckError::new(format!(
                        "type {target_type:?} is not indexable"
                    )));
                    None
                })
        })
    }

    fn check_virtual_path_call(&mut self, callee: &str, args: &[CallArg]) {
        if !callee.starts_with("fs.") {
            return;
        }
        for arg in args {
            if let Expr::Literal(arcweft_lang_syntax::expr::Literal::String(path)) = arg.value()
                && looks_like_os_absolute_path(path)
            {
                self.errors.push(TypeCheckError::new(format!(
                    "filesystem capability `{callee}` requires a VirtualPath, not an OS absolute path `{path}`"
                )));
            }
        }
    }

    fn check_dotted_path_target(&mut self, path: &str) -> Option<TypeKind> {
        let (target, field) = path.rsplit_once('.')?;
        if let Some(field_type) = well_known_field_type(field) {
            return Some(field_type);
        }
        self.locals
            .get(target)
            .cloned()
            .or_else(|| self.env.symbol_type(target).cloned())
    }

    fn check_select_expr(&mut self, expr: &Expr, select: &SelectExpr) -> Option<TypeKind> {
        let target = select.target();
        let field = select.member().as_str();
        if let Some(path) = expr_path_label(expr) {
            if let Some(ty) = self.locals.get(&path).cloned() {
                return Some(ty);
            }
            if let Some(ty) = self.env.symbol_type(&path).cloned() {
                return Some(ty);
            }
            if let Some(ty) = std_float_constant_type(&path) {
                return Some(ty);
            }
            if let Some(ty) = inline_failure_builtin_variant_type(&path) {
                return Some(ty);
            }
        }
        let receiver_type = self.check_expr(target);
        if let Some(field_type) = receiver_type
            .as_ref()
            .and_then(|ty| self.nominal_field_type(ty, field))
        {
            return Some(field_type);
        }
        let field_type = match receiver_type.as_ref() {
            Some(TypeKind::Observation) => agent_observation_field_type(field),
            Some(TypeKind::ObservedObject) => agent_observed_object_field_type(field),
            Some(TypeKind::AgentBBox) => agent_bbox_field_type(field),
            Some(TypeKind::ActionTarget) => agent_action_target_field_type(field),
            Some(TypeKind::ActionResult) => agent_action_result_field_type(field),
            Some(TypeKind::CaptureRef) => agent_capture_ref_field_type(field),
            Some(TypeKind::AgentEntityMetadata) => Self::agent_entity_metadata_field_type(field),
            Some(TypeKind::AgentSourceAnchor) => Self::agent_source_anchor_field_type(field),
            Some(TypeKind::AgentProjectGraphNeighborhood) => {
                Self::agent_project_graph_neighborhood_field_type(field)
            }
            Some(TypeKind::AgentProjectGraphSymbol) => {
                Self::agent_project_graph_symbol_field_type(field)
            }
            Some(TypeKind::AgentProjectGraphEdge) => {
                Self::agent_project_graph_edge_field_type(field)
            }
            Some(TypeKind::AgentResource) => agent_resource_field_type(field),
            Some(TypeKind::AgentResourceBody) => agent_resource_body_field_type(field),
            Some(TypeKind::Ref(_)) => {
                agent_entity_ref_field_type(field).or_else(|| well_known_field_type(field))
            }
            Some(TypeKind::Map { value, .. }) => Some(value.as_ref().clone()),
            Some(TypeKind::Named(name)) if name == "HttpRequestContext" => match field {
                "method" | "path" | "body" => Some(TypeKind::String),
                _ => None,
            },
            _ => well_known_field_type(field),
        };
        if field_type.is_some() {
            return field_type;
        }
        let method_name = field.split_once('<').map_or(field, |(name, _)| name);
        if let Some(receiver_type) = receiver_type.as_ref()
            && self.reject_method_value_reference(receiver_type, method_name)
        {
            return Some(TypeKind::Named("_".to_owned()));
        }
        None
    }

    fn reject_method_value_reference(
        &mut self,
        receiver_type: &TypeKind,
        method_name: &str,
    ) -> bool {
        if self
            .env
            .method_signature(receiver_type, method_name)
            .is_some()
        {
            self.errors
                .push(TypeCheckError::unsupported_method_value_reference(
                    receiver_type.clone(),
                    method_name,
                    "environment method values need an explicit receiver-binding contract; call the method directly or wrap it in an explicit closure",
                ));
            return true;
        }
        match self.trait_catalog.resolve_method(
            receiver_type,
            method_name,
            &self.active_trait_predicates(),
        ) {
            TraitMethodResolution::Missing => false,
            TraitMethodResolution::Inherent(_) | TraitMethodResolution::Unique { .. } => {
                self.errors
                    .push(TypeCheckError::unsupported_method_value_reference(
                        receiver_type.clone(),
                        method_name,
                        "trait/impl method values need an explicit receiver-binding contract; call the method directly or wrap it in an explicit closure",
                    ));
                true
            }
            TraitMethodResolution::Ambiguous(candidates) => {
                self.errors.push(TypeCheckError::trait_diagnostic(
                    TraitDiagnostic::ambiguous_method(
                        method_name,
                        candidates
                            .iter()
                            .map(|candidate| candidate.trait_name.as_str())
                            .collect::<Vec<_>>(),
                    ),
                ));
                true
            }
        }
    }

    fn check_try_expr(&mut self, expr: &Expr) -> Option<TypeKind> {
        match self.check_expr(expr) {
            Some(TypeKind::Result { ok, error }) => {
                self.check_try_result_context(error.as_ref());
                Some(*ok)
            }
            Some(TypeKind::Option(inner)) => {
                self.check_try_option_context();
                Some(*inner)
            }
            Some(TypeKind::Named(name)) => result_ok_type(&name).or_else(|| {
                self.errors.push(TypeCheckError::new(format!(
                    "`?` requires Result<T, E> or Option<T>, found {name}"
                )));
                None
            }),
            Some(other) => {
                self.errors.push(TypeCheckError::new(format!(
                    "`?` requires Result<T, E> or Option<T>, found {}",
                    type_kind_label(&other)
                )));
                None
            }
            None => None,
        }
    }

    fn check_try_result_context(&mut self, actual_error: &TypeKind) {
        match self.expected_returns.last().cloned().flatten() {
            Some(TypeKind::Result { error, .. }) => {
                let expected_error = error.as_ref().clone();
                if !self.types_compatible(&expected_error, actual_error) {
                    self.errors.push(TypeCheckError::new(format!(
                        "`?` error type {actual_error:?} cannot be injected into return error type {expected_error:?}"
                    )));
                }
            }
            Some(return_ty) => {
                self.errors.push(TypeCheckError::new(format!(
                    "`?` on Result<T, E> requires an enclosing Result return, found {return_ty:?}"
                )));
            }
            None => {}
        }
    }

    fn check_try_option_context(&mut self) {
        match self.expected_returns.last().and_then(Option::as_ref) {
            Some(TypeKind::Option(_)) | None => {}
            Some(return_ty) => {
                self.errors.push(TypeCheckError::new(format!(
                    "`?` on Option<T> requires an enclosing Option return, found {return_ty:?}"
                )));
            }
        }
    }

    pub(super) fn check_block_expr(
        &mut self,
        statements: &[Stmt],
        value: Option<&Expr>,
    ) -> Option<TypeKind> {
        self.check_block_expr_with_expected(statements, value, None)
    }

    pub(super) fn check_authored_block_expr(
        &mut self,
        statements: &[Stmt],
        value: Option<&AuthoredExpr>,
    ) -> Option<TypeKind> {
        self.check_authored_block_expr_with_expected(statements, value, None)
    }

    pub(super) fn check_block_expr_with_expected(
        &mut self,
        statements: &[Stmt],
        value: Option<&Expr>,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        let ty = self.with_local_mutation_scope(|this| {
            for stmt in statements {
                this.check_stmt(stmt);
            }
            value.map_or(Some(TypeKind::Unit), |value| {
                this.check_expr_with_expected(value, expected)
            })
        });
        let ty = if value.is_none() && stmts_diverge(statements) {
            Some(TypeKind::Never)
        } else {
            ty
        };
        if let (Some(expected), Some(actual)) = (expected, ty.as_ref())
            && !self.types_compatible(expected, actual)
        {
            self.errors.push(TypeCheckError::new(format!(
                "block final value must have type {}, found {}",
                type_kind_label(expected),
                type_kind_label(actual)
            )));
        }
        self.reject_borrow_escape(ty.as_ref(), "block final value");
        ty
    }

    pub(super) fn check_authored_block_expr_with_expected(
        &mut self,
        statements: &[Stmt],
        value: Option<&AuthoredExpr>,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        let ty = self.with_local_mutation_scope(|this| {
            for stmt in statements {
                this.check_stmt(stmt);
            }
            value.map_or(Some(TypeKind::Unit), |value| {
                this.check_authored_expr_with_expected(value, expected)
            })
        });
        let ty = if value.is_none() && stmts_diverge(statements) {
            Some(TypeKind::Never)
        } else {
            ty
        };
        if let (Some(expected), Some(actual)) = (expected, ty.as_ref())
            && !self.types_compatible(expected, actual)
        {
            self.errors.push(TypeCheckError::new(format!(
                "block final value must have type {}, found {}",
                type_kind_label(expected),
                type_kind_label(actual)
            )));
        }
        self.reject_borrow_escape(ty.as_ref(), "block final value");
        ty
    }

    fn check_computation_block(
        &mut self,
        kind: ComputationBlockKind,
        statements: &[Stmt],
        value: Option<&Expr>,
    ) -> Option<TypeKind> {
        match kind {
            ComputationBlockKind::Result | ComputationBlockKind::Task => {
                self.check_block_expr(statements, value)
            }
            ComputationBlockKind::Seq => {
                self.yield_stack.push(YieldContext::Seq {
                    item_ty: None,
                    yield_count: 0,
                });
                self.check_block_expr(statements, value);
                let Some(YieldContext::Seq { item_ty, .. }) = self.yield_stack.pop() else {
                    return None;
                };
                Some(TypeKind::Seq(Box::new(item_ty.unwrap_or(TypeKind::Unit))))
            }
            ComputationBlockKind::Stream => {
                self.yield_stack.push(YieldContext::Seq {
                    item_ty: None,
                    yield_count: 0,
                });
                self.check_block_expr(statements, value);
                let Some(YieldContext::Seq { item_ty, .. }) = self.yield_stack.pop() else {
                    return None;
                };
                Some(TypeKind::Stream {
                    item: Box::new(item_ty.unwrap_or(TypeKind::Unit)),
                    error: Box::new(TypeKind::Unit),
                })
            }
        }
    }

    fn check_if_expr(
        &mut self,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: Option<&Expr>,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        self.expect_expr_type(condition, &TypeKind::Bool, "if expression condition");
        let base_borrow_checkpoint = self.checkpoint_borrow_state();
        let then_type = self.check_expr_with_expected(then_branch, expected);
        let then_borrow_state = self.capture_borrow_state_delta(base_borrow_checkpoint);
        self.restore_borrow_state(base_borrow_checkpoint);
        let else_type = else_branch.map_or(Some(TypeKind::Unit), |branch| {
            self.check_expr_with_expected(branch, expected)
        });
        let else_borrow_state = self.capture_borrow_state_delta(base_borrow_checkpoint);
        if else_branch.is_some() {
            self.merge_borrow_state_from_deltas(
                base_borrow_checkpoint,
                &[&then_borrow_state, &else_borrow_state],
            );
        } else {
            let unchanged_state = BorrowStateDelta::default();
            self.merge_borrow_state_from_deltas(
                base_borrow_checkpoint,
                &[&unchanged_state, &then_borrow_state],
            );
        }
        if let Some(expected) = expected {
            for (label, ty) in [("then", then_type.as_ref()), ("else", else_type.as_ref())] {
                if let Some(ty) = ty
                    && !self.types_compatible(expected, ty)
                {
                    self.errors.push(TypeCheckError::new(format!(
                        "if expression {label} branch must have type {}, found {}",
                        type_kind_label(expected),
                        type_kind_label(ty)
                    )));
                }
            }
            if then_type
                .as_ref()
                .is_some_and(|ty| self.types_compatible(expected, ty))
                && else_type
                    .as_ref()
                    .is_some_and(|ty| self.types_compatible(expected, ty))
            {
                return Some(expected.clone());
            }
        }
        match (then_type, else_type) {
            (Some(then_type), Some(else_type)) => Some(join_branch_types(then_type, else_type)),
            _ => None,
        }
    }

    fn check_match_expr(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchExprArm],
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        let scrutinee_type = self.check_expr(scrutinee);
        if arms.is_empty() {
            self.errors.push(TypeCheckError::new(
                "match expression must have at least one arm".to_owned(),
            ));
            return None;
        }

        let base_borrow_checkpoint = self.checkpoint_borrow_state();
        let mut arm_states = Vec::new();
        let mut inferred = None;
        let mut all_compatible_with_expected = expected.is_some();
        for arm in arms {
            self.restore_borrow_state(base_borrow_checkpoint);
            let local_snapshot = self
                .insert_scoped_locals(let_else_bindings(arm.pattern(), scrutinee_type.as_ref()));
            if let Some(guard) = arm.guard() {
                self.expect_expr_type(guard, &TypeKind::Bool, "match arm guard");
            }
            let arm_expected = expected.or(inferred.as_ref());
            let arm_type = self.check_expr_with_expected(arm.value(), arm_expected);
            self.restore_scoped_locals(local_snapshot);
            if let (Some(expected), Some(arm_ty)) = (expected, arm_type.as_ref())
                && !self.types_compatible(expected, arm_ty)
            {
                all_compatible_with_expected = false;
                self.errors.push(TypeCheckError::new(format!(
                    "match arm must have type {}, found {}",
                    type_kind_label(expected),
                    type_kind_label(arm_ty)
                )));
            }
            match (&inferred, arm_type) {
                (None, Some(ty)) => inferred = Some(ty),
                (Some(existing), Some(ty)) if existing == &ty => {}
                (Some(existing), Some(ty)) => {
                    inferred = Some(join_branch_types(existing.clone(), ty));
                }
                (_, None) => return None,
            }
            arm_states.push(self.capture_borrow_state_delta(base_borrow_checkpoint));
        }
        self.check_choice_match_exhaustive(
            scrutinee_type.as_ref(),
            arms.iter()
                .map(arcweft_lang_syntax::expr::MatchExprArm::pattern),
        );
        let arm_state_refs = arm_states.iter().collect::<Vec<_>>();
        self.merge_borrow_state_from_deltas(base_borrow_checkpoint, &arm_state_refs);
        if let (Some(expected), Some(actual)) = (expected, inferred.as_ref())
            && all_compatible_with_expected
            && self.types_compatible(expected, actual)
        {
            return Some(expected.clone());
        }
        inferred
    }

    fn check_if_let_expr(
        &mut self,
        pattern: &Pattern,
        expr: &Expr,
        guard: Option<&Expr>,
        then_branch: &Expr,
        else_branch: Option<&Expr>,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        let expr_type = self.check_expr(expr);
        let base_borrow_checkpoint = self.checkpoint_borrow_state();
        let local_snapshot =
            self.insert_scoped_locals(let_else_bindings(pattern, expr_type.as_ref()));
        if let Some(guard) = guard {
            self.expect_expr_type(guard, &TypeKind::Bool, "if-let expression guard");
        }
        let then_type = self.check_expr_with_expected(then_branch, expected);
        let then_borrow_state = self.capture_borrow_state_delta(base_borrow_checkpoint);
        self.restore_borrow_state(base_borrow_checkpoint);
        self.restore_scoped_locals(local_snapshot);

        let else_type = else_branch.map_or(Some(TypeKind::Unit), |branch| {
            self.check_expr_with_expected(branch, expected)
        });
        let else_borrow_state = self.capture_borrow_state_delta(base_borrow_checkpoint);
        if else_branch.is_some() {
            self.merge_borrow_state_from_deltas(
                base_borrow_checkpoint,
                &[&then_borrow_state, &else_borrow_state],
            );
        } else {
            let unchanged_state = BorrowStateDelta::default();
            self.merge_borrow_state_from_deltas(
                base_borrow_checkpoint,
                &[&unchanged_state, &then_borrow_state],
            );
        }
        if let Some(expected) = expected {
            for (label, ty) in [("then", then_type.as_ref()), ("else", else_type.as_ref())] {
                if let Some(ty) = ty
                    && !self.types_compatible(expected, ty)
                {
                    self.errors.push(TypeCheckError::new(format!(
                        "if-let expression {label} branch must have type {}, found {}",
                        type_kind_label(expected),
                        type_kind_label(ty)
                    )));
                }
            }
            if then_type
                .as_ref()
                .is_some_and(|ty| self.types_compatible(expected, ty))
                && else_type
                    .as_ref()
                    .is_some_and(|ty| self.types_compatible(expected, ty))
            {
                return Some(expected.clone());
            }
        }
        match (then_type, else_type) {
            (Some(then_type), Some(else_type)) => Some(join_branch_types(then_type, else_type)),
            _ => None,
        }
    }

    pub(super) fn check_choice_match_exhaustive<'a>(
        &mut self,
        scrutinee_type: Option<&TypeKind>,
        patterns: impl IntoIterator<Item = &'a Pattern>,
    ) {
        let Some(TypeKind::Choice(alternatives)) = scrutinee_type else {
            return;
        };
        let coverage = patterns
            .into_iter()
            .map(choice_pattern_coverage)
            .collect::<Vec<_>>();
        let missing = alternatives
            .iter()
            .filter(|alternative| {
                !coverage
                    .iter()
                    .any(|coverage| self.coverage_covers(coverage, alternative))
            })
            .map(type_kind_label)
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            self.errors.push(TypeCheckError::new(format!(
                "non-exhaustive match on anonymous sum; missing alternative(s): {}",
                missing.join(", ")
            )));
        }
    }

    fn coverage_covers(
        &mut self,
        coverage: &ChoicePatternCoverage,
        alternative: &TypeKind,
    ) -> bool {
        match coverage {
            ChoicePatternCoverage::All => true,
            ChoicePatternCoverage::Type(ty) => self.types_compatible(ty, alternative),
        }
    }
}
