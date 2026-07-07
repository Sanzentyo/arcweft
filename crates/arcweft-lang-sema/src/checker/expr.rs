//! Expression type-checking entry points and expression-kind dispatch.

use super::helpers::{
    array_len_matches, array_repeat_len_label, collection_index_type, expr_path_label,
    first_arg_type, is_drop_name, let_else_bindings, numeric_literal_suffix_type,
    optional_type_kind_label, result_ok_type, stmts_diverge, type_kind_label,
    well_known_capacity_method_type, well_known_field_type, well_known_runtime_method_type,
};
use super::{
    BorrowLocalState, BorrowStateDelta, EntityKind, EntityRefSyntax, Expr, FunctionSignature,
    LifetimeScopeKind, Pattern, Stmt, TypeCheckError, TypeChecker, TypeExpressionId,
    TypeJudgmentRule, TypeJudgmentSubject, TypeKind, TypedLoweringEvidence,
    TypedLoweringEvidenceKind, YieldContext, entity_syntax_kind,
};
use crate::diagnostics::TraitDiagnostic;
use crate::traits::TraitMethodResolution;
use arcweft_lang_syntax::ast::flow::ThreadBlock;
use arcweft_lang_syntax::ast::line_plan::LinePlan;
use arcweft_lang_syntax::expr::{
    BinaryOp, CallArg, ComputationBlockKind, Literal, MatchExprArm, Placeholder, UnaryOp,
};

mod agent;
mod builtin;
mod callable;
mod closure;
mod method_fallback;
mod partial;
mod pipe;
mod range;
mod signature_call;
mod support;

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
    rhs_expected_type_for_binary, spread_item_type, std_float_constant_type,
    trait_method_call_signature, unique_numeric_choice_alternative,
};

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
            self.record_type_judgment(
                TypeJudgmentSubject::Expr {
                    id: expression_id,
                    kind: expr_kind_name(expr),
                },
                expected.map_or(TypeJudgmentRule::Expr, |_| TypeJudgmentRule::Expected),
                ty.clone(),
                expected,
            );
            if let Some(expected) = expected
                && let Some(arity) = expected.function_arity()
                && ty.function_arity().is_some()
            {
                self.record_typed_lowering_evidence(TypedLoweringEvidence {
                    expression_id,
                    kind: TypedLoweringEvidenceKind::ExpectedFunctionValue {
                        expected_ty: expected.clone(),
                        actual_ty: ty.clone(),
                        arity,
                    },
                });
            } else if expected.is_none()
                && expr_contains_partial_placeholder(expr)
                && let Some(arity) = ty.function_arity()
            {
                self.record_typed_lowering_evidence(TypedLoweringEvidence {
                    expression_id,
                    kind: TypedLoweringEvidenceKind::ExpectedFunctionValue {
                        expected_ty: ty.clone(),
                        actual_ty: ty.clone(),
                        arity,
                    },
                });
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
            Expr::Literal(literal) => Some(self.check_literal_expr(literal, expected)),
            Expr::EntityRef(entity) => self.check_entity_ref_expr(entity),
            Expr::LifetimePath { key, optional } => self.check_lifetime_path_expr(key, *optional),
            Expr::Path(path) => self.check_path_expr_with_expected(path.as_label(), expected),
            Expr::ShortVariant(name) => {
                Some(self.check_short_variant_expr(name.as_str(), expected))
            }
            Expr::Placeholder(placeholder) => self.check_placeholder_expr(*placeholder),
            Expr::Tuple(items) => Some(self.check_tuple_expr_with_expected(items, expected)),
            Expr::BracketSeq(items) => Some(self.check_bracket_seq_with_expected(items, expected)),
            Expr::NumericBracketSeq(seq) => {
                Some(self.check_numeric_bracket_seq_summary(seq, expected))
            }
            Expr::ArrayRepeat { value, len } => {
                Some(self.check_array_repeat_expr(value, len, expected))
            }
            Expr::Call { callee, args } => {
                self.check_call_expr(callee, args, expected, expression_id)
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
            } => self.check_method_call_expr(receiver, method, args, expression_id),
            Expr::Field { target, field } => self.check_field_expr(expr, target, field),
            Expr::DialogueCall { callee, plan, .. } => {
                Some(self.check_dialogue_call_expr(callee, plan.as_ref()))
            }
            Expr::Index { target, index } => self.check_index_expr(target, index),
            Expr::Pipe { lhs, rhs } => self.check_pipe_expr(lhs, rhs),
            Expr::Try { expr } => self.check_try_expr(expr),
            Expr::Await { expr, applies_try } => self.check_await_expr_node(expr, *applies_try),
            Expr::Thread { block } => Some(self.check_thread_expr(block)),
            Expr::Range { start, end, .. } => {
                Some(self.check_range_expr(start.as_deref(), end.as_deref(), expected))
            }
            Expr::Record { path, fields } => Some(self.check_record_expr(path, fields)),
            Expr::RecordLiteral(fields) => Some(self.check_record_literal_expr(fields)),
            Expr::Binary { lhs, op, rhs } => self.check_binary_expr(lhs, *op, rhs),
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
            Expr::MemoBlock {
                options,
                statements,
                value,
            } => self.check_memo_block_expr(options, statements, value.as_deref(), expected),
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

    fn check_entity_ref_expr(&mut self, entity: &EntityRefSyntax) -> Option<TypeKind> {
        if let Some(ty) = self.symbol_type(entity.body()).cloned() {
            return Some(ty);
        }
        entity_syntax_kind(entity)
            .map(TypeKind::entity_ref)
            .or_else(|| {
                self.errors.push(TypeCheckError::new(format!(
                    "unknown entity reference kind: {}",
                    entity.body()
                )));
                None
            })
    }

    fn check_literal_expr(
        &mut self,
        literal: &arcweft_lang_syntax::expr::Literal,
        expected: Option<&TypeKind>,
    ) -> TypeKind {
        match literal {
            arcweft_lang_syntax::expr::Literal::String(_) => TypeKind::String,
            arcweft_lang_syntax::expr::Literal::Char { .. } => TypeKind::Char,
            arcweft_lang_syntax::expr::Literal::Bool(_) => TypeKind::Bool,
            arcweft_lang_syntax::expr::Literal::Duration { .. } => TypeKind::Duration,
            arcweft_lang_syntax::expr::Literal::Int { suffix, .. } => {
                if let Some(suffix) = suffix {
                    let Some(ty) = numeric_literal_suffix_type(Some(suffix.as_str())) else {
                        self.errors.push(TypeCheckError::new(format!(
                            "unknown integer literal suffix `{suffix}`"
                        )));
                        return TypeKind::Named("_".to_owned());
                    };
                    if ty.is_integer() || is_unit_number_type(&ty) {
                        ty
                    } else {
                        self.errors.push(TypeCheckError::new(format!(
                            "integer literal suffix must be an integer type, found {ty:?}"
                        )));
                        TypeKind::Named("_".to_owned())
                    }
                } else if let Some(expected) = expected.filter(|ty| ty.is_integer()) {
                    expected.clone()
                } else if let Some(expected) = expected
                    && let Some(ty) =
                        unique_numeric_choice_alternative(expected, TypeKind::is_integer)
                {
                    ty
                } else if expected.is_some_and(|expected| {
                    has_multiple_numeric_choice_alternatives(expected, TypeKind::is_integer)
                }) {
                    self.errors.push(TypeCheckError::new(
                        "unsuffixed integer literal requires an expected integer type".to_owned(),
                    ));
                    TypeKind::Named("_".to_owned())
                } else {
                    TypeKind::I32
                }
            }
            arcweft_lang_syntax::expr::Literal::Float { suffix, .. } => {
                if let Some(suffix) = suffix {
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
                            "float literal suffix must be a float type, found {ty:?}"
                        )));
                        TypeKind::Named("_".to_owned())
                    }
                } else if let Some(expected) = expected.filter(|ty| ty.is_float()) {
                    expected.clone()
                } else if let Some(expected) = expected
                    && let Some(ty) =
                        unique_numeric_choice_alternative(expected, TypeKind::is_float)
                {
                    ty
                } else if expected.is_some_and(|expected| {
                    has_multiple_numeric_choice_alternatives(expected, TypeKind::is_float)
                }) {
                    self.errors.push(TypeCheckError::new(
                        "unsuffixed float literal requires an expected float type".to_owned(),
                    ));
                    TypeKind::Named("_".to_owned())
                } else {
                    TypeKind::F64
                }
            }
            arcweft_lang_syntax::expr::Literal::UnitNumber { suffix, .. } => {
                numeric_literal_suffix_type(Some(suffix.as_str()))
                    .unwrap_or_else(|| TypeKind::Named("_".to_owned()))
            }
        }
    }

    fn check_path_expr_with_expected(
        &mut self,
        path: &str,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        if let Some(ty) = self.expected_short_variant_type(path, expected) {
            return Some(ty);
        }
        if path == "None"
            && let Some(expected @ TypeKind::Option(_)) = expected
        {
            return Some(expected.clone());
        }
        self.check_path_expr(path)
    }

    fn check_short_variant_expr(&mut self, variant: &str, expected: Option<&TypeKind>) -> TypeKind {
        let label = format!(".{variant}");
        if let Some(ty) = self.symbol_type(&label).cloned() {
            return ty;
        }
        if let Some(ty) = self.expected_short_variant_type(variant, expected) {
            return ty;
        }
        TypeKind::Named("Variant".to_owned())
    }

    fn expected_short_variant_type(
        &self,
        path: &str,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        let variant = path.strip_prefix('.').unwrap_or(path);
        match expected? {
            TypeKind::Choice(alternatives) => {
                let mut matches = alternatives
                    .iter()
                    .filter(|ty| self.env.enum_has_variant(ty, variant));
                let selected = matches.next()?;
                matches.next().is_none().then(|| selected.clone())
            }
            ty if self.env.enum_has_variant(ty, variant) => Some(ty.clone()),
            _ => None,
        }
    }

    fn check_path_expr(&mut self, path: &str) -> Option<TypeKind> {
        if let Some(state) = self.borrow_local_lifetimes.get(path) {
            match state {
                BorrowLocalState::Dropped => self.errors.push(TypeCheckError::new(format!(
                    "borrowed local `{path}` was used after it was dropped"
                ))),
                BorrowLocalState::MaybeDropped(_) => {
                    self.errors.push(TypeCheckError::new(format!(
                        "borrowed local `{path}` may have been dropped on another control-flow path"
                    )));
                }
                BorrowLocalState::Live(_) => {}
            }
        }
        if let Some(ty) = self.symbol_type_with_capture(path) {
            return Some(ty);
        }
        if let Some(ty) = self.function_value_type(path) {
            return Some(ty);
        }
        if let Some(ty) = self.check_dotted_path_target(path) {
            return Some(ty);
        }
        if path == "None" {
            return Some(TypeKind::Option(Box::new(TypeKind::Named("_".to_owned()))));
        }
        if path == "asset" {
            return Some(TypeKind::Named("AssetApi".to_owned()));
        }
        if path == "voice" {
            return Some(TypeKind::Named("VoiceApi".to_owned()));
        }
        if path == "state" {
            return Some(TypeKind::Named("GameState".to_owned()));
        }
        if path == "line" {
            return Some(TypeKind::Named("LineContext".to_owned()));
        }
        if path == "auto" {
            return Some(TypeKind::Named("Auto".to_owned()));
        }
        if matches!(path, "InlineFailure" | "InlineFallback" | "FallbackStyle") {
            return Some(TypeKind::Named(format!("{path}Namespace")));
        }
        // Short enum-variant expressions such as `.Instant` rely
        // on expected type resolution in the full checker. The
        // Phase 1 checker preserves unknown short variants as
        // variant values after registered symbols and patch names
        // had a chance to resolve.
        if path.starts_with('.') {
            return Some(TypeKind::Named("Variant".to_owned()));
        }
        self.errors
            .push(TypeCheckError::new(format!("unknown symbol `{path}`")));
        None
    }

    fn check_record_expr(&mut self, path: &str, fields: &[(String, Expr)]) -> TypeKind {
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

    fn check_dialogue_call_expr(&mut self, callee: &Expr, plan: Option<&LinePlan>) -> TypeKind {
        self.check_expr(callee);
        if let Some(plan) = plan {
            self.available_lifetimes.push(LifetimeScopeKind::Line);
            let output = self.check_line_plan_output_type(plan);
            self.available_lifetimes.pop();
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
    ) -> TypeKind {
        let expected_item = match expected {
            Some(TypeKind::Array { item, .. } | TypeKind::Vec(item)) => Some(item.as_ref()),
            _ => None,
        };
        let item_type = if let Some(suffix) = seq.suffix() {
            let ty = if let Some(ty) = numeric_literal_suffix_type(Some(suffix)) {
                ty
            } else {
                self.errors.push(TypeCheckError::new(format!(
                    "unknown integer literal suffix `{suffix}`"
                )));
                TypeKind::Named("_".to_owned())
            };
            if ty.is_integer() || is_unit_number_type(&ty) {
                ty
            } else {
                self.errors.push(TypeCheckError::new(format!(
                    "integer literal suffix must be an integer type, found {ty:?}"
                )));
                TypeKind::Named("_".to_owned())
            }
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
            TypeKind::I32
        };
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
                (Expr::Literal(Literal::Int { .. }), true, _)
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
            Literal::Int { suffix: None, .. } if expected_item.is_integer() => {
                expected_item.clone()
            }
            Literal::Float { suffix: None, .. } if expected_item.is_float() => {
                expected_item.clone()
            }
            Literal::Int {
                suffix: Some(suffix),
                ..
            } => {
                let Some(ty) = numeric_literal_suffix_type(Some(suffix.as_str())) else {
                    self.errors.push(TypeCheckError::new(format!(
                        "unknown integer literal suffix `{suffix}`"
                    )));
                    return TypeKind::Named("_".to_owned());
                };
                if ty.is_integer() || is_unit_number_type(&ty) {
                    ty
                } else {
                    self.errors.push(TypeCheckError::new(format!(
                        "integer literal suffix must be an integer type, found {ty:?}"
                    )));
                    TypeKind::Named("_".to_owned())
                }
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

    fn check_memo_block_expr(
        &mut self,
        options: &[(String, Expr)],
        statements: &[Stmt],
        value: Option<&Expr>,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        for (_, option) in options {
            self.check_expr(option);
        }
        self.check_block_expr_with_expected(statements, value, expected)
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
        if let Some(ty) = self.check_builtin_call_expr(callee, args) {
            return Some(ty);
        }
        if let Some(name) = expr_path_label(callee)
            && let Some(ty) = self.check_agent_intrinsic_call_name(&name, args)
        {
            return Some(ty);
        }
        if let Expr::Path(name) = callee
            && let Some(ty) = self.check_presentation_call(name, args)
        {
            return Some(ty);
        }
        if let Some(name) = expr_path_label(callee)
            && let Some(ty) = self
                .function_type(&name)
                .cloned()
                .or_else(|| well_known_runtime_method_type(&name))
        {
            let signature = self.function_signature(&name).cloned();
            self.check_virtual_path_call(&name, args);
            self.check_function_effects(&name);
            if let Some(signature) = signature.filter(FunctionSignature::checks_args) {
                self.check_signature_call_args(&name, &signature, args);
            } else {
                self.check_untyped_function_args(&name, args);
            }
            return Some(ty);
        }
        if let Expr::Path(name) = callee {
            return self.check_path_call_expr(name, args, expected, expression_id);
        }
        match self.check_expr(callee) {
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

    fn check_builtin_call_expr(&mut self, callee: &Expr, args: &[CallArg]) -> Option<TypeKind> {
        let name = expr_path_label(callee)?;
        self.check_builtin_call_name(&name, args)
    }

    fn check_builtin_call_name(&mut self, name: &str, args: &[CallArg]) -> Option<TypeKind> {
        match BuiltinCallSpec::resolve(name)? {
            BuiltinCallSpec::InlineFailureFallback => {
                Some(TypeKind::Named("InlineFailure".to_owned()))
            }
            BuiltinCallSpec::Never => {
                for arg in args {
                    self.check_expr(arg.value());
                }
                Some(TypeKind::Never)
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
            UnaryOp::Neg => match self.check_expr_with_expected(expr, expected) {
                Some(ty) if ty.is_integer() || ty.is_float() => ty,
                Some(TypeKind::Duration) => TypeKind::Duration,
                other => {
                    self.errors.push(TypeCheckError::new(format!(
                        "negation operand must be numeric or Duration, found {}",
                        optional_type_kind_label(other.as_ref())
                    )));
                    TypeKind::Named("_".to_owned())
                }
            },
        }
    }

    fn check_method_call_expr(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[CallArg],
        expression_id: TypeExpressionId,
    ) -> Option<TypeKind> {
        let method_name = method.split_once('<').map_or(method, |(name, _)| name);
        if let Some(receiver_path) = expr_path_label(receiver) {
            let dotted = format!("{receiver_path}.{method_name}");
            if BuiltinCallSpec::resolve(&dotted).is_some() {
                return self.check_builtin_call_name(&dotted, args);
            }
            if receiver_path == "InlineFailure" && method_name == "fallback" {
                return Some(TypeKind::Named("InlineFailure".to_owned()));
            }
            if let Some(ty) = self.check_agent_intrinsic_call_name(&dotted, args) {
                return Some(ty);
            }
            if let Some(ty) = self
                .function_type(&dotted)
                .cloned()
                .or_else(|| well_known_runtime_method_type(&dotted))
            {
                let signature = self.function_signature(&dotted).cloned();
                self.check_virtual_path_call(&dotted, args);
                self.check_function_effects(&dotted);
                if let Some(signature) = signature.filter(FunctionSignature::checks_args) {
                    self.check_signature_call_args(&dotted, &signature, args);
                } else {
                    self.check_untyped_function_args(&dotted, args);
                }
                return Some(ty);
            }
        }
        let receiver_type = self.check_expr(receiver);
        if is_drop_name(method_name) {
            for arg in args {
                self.check_expr(arg.value());
            }
            return Some(TypeKind::Unit);
        }
        receiver_type.and_then(|receiver_type| {
            self.check_typed_method_call(receiver_type, method_name, args, expression_id)
        })
    }

    fn check_typed_method_call(
        &mut self,
        receiver_type: TypeKind,
        method_name: &str,
        args: &[CallArg],
        expression_id: TypeExpressionId,
    ) -> Option<TypeKind> {
        if method_name == "traverse" {
            return self.check_traverse_method_call(&receiver_type, args);
        }
        if method_name == "parallel" {
            return self.check_parallel_method_call(&receiver_type, args);
        }
        if let Some(signature) = self
            .env
            .method_signature(&receiver_type, method_name)
            .cloned()
        {
            if signature.checks_args() {
                self.check_signature_call_args(method_name, &signature, args);
            } else {
                self.check_untyped_method_args(args);
            }
            return Some(signature.return_type().clone());
        }
        match self.check_builtin_collection_method_call(&receiver_type, method_name, args) {
            BuiltinCollectionMethodCallOutcome::Missing => {}
            BuiltinCollectionMethodCallOutcome::Checked(return_type) => return return_type,
        }
        if let Some(return_type) =
            self.check_presentation_handle_lifecycle_method(&receiver_type, method_name, args)
        {
            return Some(return_type);
        }
        if matches!(method_name, "clamp" | "min" | "max") && receiver_type.is_integer() {
            return Some(self.check_integer_scalar_method_call(receiver_type, method_name, args));
        }
        if method_name == "require_role" {
            return self.check_agent_object_require_role_method_call(&receiver_type, args);
        }
        if method_name == "get" {
            return self.check_map_get_method_call(&receiver_type, args);
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
        ) && let TypeKind::Probe(inner) = &receiver_type
        {
            return Some(self.check_probe_compare_method(method_name, inner.as_ref(), args));
        }
        if receiver_type == TypeKind::Named("Diagnostics".to_owned()) && method_name == "has_error"
        {
            return Some(self.check_no_arg_method(
                "Diagnostics.has_error",
                args,
                TypeKind::Predicate,
            ));
        }
        if receiver_type == TypeKind::RagContextPack && method_name == "summary" {
            return Some(self.check_no_arg_method(
                "RagContextPack.summary",
                args,
                TypeKind::DisplayText,
            ));
        }
        if matches!(method_name, "context" | "with_context") {
            return self.check_context_method_call(receiver_type, args);
        }
        if method_name == "face" && is_character_speaker_type(&receiver_type) {
            self.check_untyped_method_args(args);
            return Some(TypeKind::CharacterPatch(EntityKind::Character));
        }
        if method_name == "say" && is_character_speaker_type(&receiver_type) {
            self.check_untyped_method_args(args);
            return Some(TypeKind::SpeakerPreset(EntityKind::Character));
        }
        match self.check_trait_method_call(&receiver_type, method_name, args) {
            TraitMethodCallOutcome::Missing => {}
            TraitMethodCallOutcome::Typed(return_type) => return Some(return_type),
            TraitMethodCallOutcome::Rejected => return None,
        }
        if let Some(return_type) =
            self.check_data_last_method_fallback(&receiver_type, method_name, args, expression_id)
        {
            return Some(return_type);
        }
        self.check_untyped_method_args(args);
        self.env
            .method_type(&receiver_type, method_name)
            .cloned()
            .or_else(|| well_known_capacity_method_type(&receiver_type, method_name, args.len()))
            .or_else(|| {
                self.errors.push(TypeCheckError::new(format!(
                    "unknown method `{method_name}` on {}",
                    type_kind_label(&receiver_type)
                )));
                None
            })
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
            TraitMethodResolution::Inherent(method)
            | TraitMethodResolution::Unique { method, .. } => {
                let return_type = self.resolve_type_projection(method.return_type().clone());
                let signature = trait_method_call_signature(method.signature(), return_type);
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

    fn check_field_expr(&mut self, expr: &Expr, target: &Expr, field: &str) -> Option<TypeKind> {
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
        match receiver_type {
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
            Some(TypeKind::Map { value, .. }) => Some(*value),
            Some(TypeKind::Named(name)) if name == "HttpRequestContext" => match field {
                "method" | "path" | "body" => Some(TypeKind::String),
                _ => None,
            },
            _ => well_known_field_type(field),
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
        if let Some(guard) = guard {
            self.expect_expr_type(guard, &TypeKind::Bool, "if-let expression guard");
        }

        let base_borrow_checkpoint = self.checkpoint_borrow_state();
        let local_snapshot =
            self.insert_scoped_locals(let_else_bindings(pattern, expr_type.as_ref()));
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

    fn check_binary_expr(&mut self, lhs: &Expr, op: BinaryOp, rhs: &Expr) -> Option<TypeKind> {
        let lhs_type = self.check_expr(lhs);
        if op == BinaryOp::In {
            return self.check_in_binary_expr(lhs_type.as_ref(), rhs);
        }
        let rhs_expected = rhs_expected_type_for_binary(op, lhs_type.as_ref());
        let rhs_type = self.check_expr_with_expected(rhs, rhs_expected);
        match op {
            BinaryOp::In => unreachable!("`in` is handled before rhs expected-type selection"),
            BinaryOp::Implies | BinaryOp::Or | BinaryOp::And => {
                if lhs_type != Some(TypeKind::Bool) || rhs_type != Some(TypeKind::Bool) {
                    self.errors.push(TypeCheckError::new(format!(
                        "logical contract expression must use bool operands, found {} and {}",
                        optional_type_kind_label(lhs_type.as_ref()),
                        optional_type_kind_label(rhs_type.as_ref())
                    )));
                    return None;
                }
                Some(TypeKind::Bool)
            }
            BinaryOp::Eq | BinaryOp::NotEq => match (lhs_type.as_ref(), rhs_type.as_ref()) {
                (Some(lhs), Some(rhs))
                    if self.types_compatible(lhs, rhs) || self.types_compatible(rhs, lhs) =>
                {
                    Some(TypeKind::Bool)
                }
                _ => {
                    self.errors.push(TypeCheckError::new(format!(
                        "equality operands must be compatible, found {} and {}",
                        optional_type_kind_label(lhs_type.as_ref()),
                        optional_type_kind_label(rhs_type.as_ref())
                    )));
                    None
                }
            },
            BinaryOp::Gte | BinaryOp::Lte | BinaryOp::Gt | BinaryOp::Lt => {
                match (lhs_type.as_ref(), rhs_type.as_ref()) {
                    (Some(lhs), Some(rhs))
                        if lhs == rhs
                            && (lhs.is_integer()
                                || lhs.is_float()
                                || lhs == &TypeKind::Duration) =>
                    {
                        Some(TypeKind::Bool)
                    }
                    _ => {
                        self.errors.push(TypeCheckError::new(format!(
                            "ordering operands must have the same ordered scalar type, found {} and {}",
                            optional_type_kind_label(lhs_type.as_ref()),
                            optional_type_kind_label(rhs_type.as_ref())
                        )));
                        None
                    }
                }
            }
            BinaryOp::Merge => match (lhs_type, rhs_type) {
                (Some(TypeKind::CharacterPatch(lhs)), Some(TypeKind::CharacterPatch(rhs)))
                    if lhs == rhs =>
                {
                    Some(TypeKind::CharacterPatch(lhs))
                }
                (Some(TypeKind::FocusPatch), Some(TypeKind::FocusPatch)) => {
                    Some(TypeKind::FocusPatch)
                }
                (lhs, rhs) => {
                    self.errors.push(TypeCheckError::new(format!(
                        "merge operator `&` requires compatible patch operands, found {} and {}",
                        optional_type_kind_label(lhs.as_ref()),
                        optional_type_kind_label(rhs.as_ref())
                    )));
                    None
                }
            },
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                if lhs_type == Some(TypeKind::Duration) && rhs_type == Some(TypeKind::Duration) {
                    Some(TypeKind::Duration)
                } else if matches!(
                    (&lhs_type, &rhs_type),
                    (Some(lhs), Some(rhs)) if lhs == rhs && (lhs.is_integer() || lhs.is_float())
                ) {
                    lhs_type
                } else {
                    self.errors.push(TypeCheckError::new(format!(
                        "arithmetic expression operands must have a supported numeric or Duration type, found {} and {}",
                        optional_type_kind_label(lhs_type.as_ref()),
                        optional_type_kind_label(rhs_type.as_ref())
                    )));
                    None
                }
            }
        }
    }

    fn check_in_binary_expr(
        &mut self,
        lhs_type: Option<&TypeKind>,
        rhs: &Expr,
    ) -> Option<TypeKind> {
        let expected_range = lhs_type
            .filter(|ty| ty.is_integer())
            .cloned()
            .map(|ty| TypeKind::Range(Box::new(ty)));
        let rhs_type = self.check_expr_with_expected(rhs, expected_range.as_ref());
        let Some(TypeKind::Range(item_type)) = rhs_type.as_ref() else {
            self.errors.push(TypeCheckError::new(format!(
                "`in` expression requires a range on the right, found {}",
                optional_type_kind_label(rhs_type.as_ref())
            )));
            return None;
        };
        if let Some(lhs_type) = lhs_type
            && !self.types_compatible(item_type, lhs_type)
        {
            self.errors.push(TypeCheckError::new(format!(
                "`in` expression left operand must have range item type {}, found {}",
                type_kind_label(item_type),
                type_kind_label(lhs_type)
            )));
            return None;
        }
        Some(TypeKind::Bool)
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
