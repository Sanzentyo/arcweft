//! Expression type-checking entry points and expression-kind dispatch.

use super::helpers::{
    array_len_matches, array_repeat_len_label, collection_index_type, expr_path_label,
    first_arg_type, is_drop_name, let_else_bindings, named_type_label, numeric_literal_suffix_type,
    result_ok_type, type_kind_label, type_ref_kind, well_known_capacity_method_type,
    well_known_field_type, well_known_runtime_method_type,
};
use super::{
    AgentActionEnvParam, BorrowLocalState, BorrowStateDelta, EntityKind, EntityRefSyntax, Expr,
    FunctionParam, FunctionSignature, LifetimeScopeKind, MapKind, Pattern, Stmt, TypeCheckError,
    TypeChecker, TypeJudgmentRule, TypeJudgmentSubject, TypeKind, YieldContext, entity_kind,
    normalize_choice_type,
};
use arcweft_lang_syntax::ast::line_plan::LinePlan;
use arcweft_lang_syntax::expr::{
    BinaryOp, CallArg, ComputationBlockKind, Literal, MatchExprArm, UnaryOp,
};

impl TypeChecker<'_> {
    pub(super) fn expect_expr_type(&mut self, expr: &Expr, expected: &TypeKind, context: &str) {
        let actual = self.check_expr_with_expected(expr, Some(expected));
        if !actual
            .as_ref()
            .is_some_and(|actual| self.types_compatible(expected, actual))
        {
            self.errors.push(TypeCheckError::new(format!(
                "{context} must have type {expected:?}, found {actual:?}"
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
        self.stats.expressions += 1;
        let ty = self.check_expr_kind_with_expected(expr, expected);
        if let Some(ty) = ty.as_ref() {
            self.record_type_judgment(
                TypeJudgmentSubject::Expr {
                    kind: expr_kind_name(expr),
                },
                expected.map_or(TypeJudgmentRule::Expr, |_| TypeJudgmentRule::Expected),
                ty.clone(),
                expected,
            );
        }
        ty
    }

    fn check_expr_kind_with_expected(
        &mut self,
        expr: &Expr,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        match expr {
            Expr::Literal(literal) => Some(self.check_literal_expr(literal, expected)),
            Expr::EntityRef(entity) => self.check_entity_ref_expr(entity),
            Expr::LifetimePath { key, optional } => self.check_lifetime_path_expr(key, *optional),
            Expr::Path(path) => self.check_path_expr(path),
            Expr::Placeholder(_) => None,
            Expr::Tuple(items) => Some(self.check_tuple_expr(items)),
            Expr::BracketSeq(items) => Some(self.check_bracket_seq_with_expected(items, expected)),
            Expr::NumericBracketSeq(seq) => {
                Some(self.check_numeric_bracket_seq_summary(seq, expected))
            }
            Expr::ArrayRepeat { value, len } => {
                Some(self.check_array_repeat_expr(value, len, expected))
            }
            Expr::Call { callee, args } => self.check_call_expr(callee, args),
            Expr::MethodCall {
                receiver,
                method,
                args,
            } => self.check_method_call_expr(receiver, method, args),
            Expr::Field { target, field } => self.check_field_expr(expr, target, field),
            Expr::DialogueCall { callee, plan, .. } => {
                Some(self.check_dialogue_call_expr(callee, plan.as_ref()))
            }
            Expr::Index { target, index } => self.check_index_expr(target, index),
            Expr::Pipe { lhs, rhs } => self.check_pipe_expr(lhs, rhs),
            Expr::Try { expr } => self.check_try_expr(expr),
            Expr::Await { expr, applies_try } => {
                if self.in_seq_context() {
                    self.errors.push(TypeCheckError::new(
                        "`seq` blocks are pure and cannot await".to_owned(),
                    ));
                }
                self.check_await_expr(expr, *applies_try)
            }
            Expr::Thread { block } => {
                self.check_thread_body(block.body());
                Some(TypeKind::ThreadHandle(Box::new(TypeKind::Unit)))
            }
            Expr::Range { start, end, .. } => {
                Some(self.check_range_expr(start.as_deref(), end.as_deref()))
            }
            Expr::Record { path, fields } => Some(self.check_record_expr(path, fields)),
            Expr::RecordLiteral(fields) => Some(self.check_record_literal_expr(fields)),
            Expr::Binary { lhs, op, rhs } => self.check_binary_expr(lhs, *op, rhs),
            Expr::Closure { params, body } => self.check_closure_expr(params, body),
            Expr::Unary { op, expr } => Some(self.check_unary_expr(*op, expr, expected)),
            Expr::Block { statements, value } => {
                self.check_block_expr(statements, value.as_deref())
            }
            Expr::ComputationBlock {
                kind,
                statements,
                value,
            } => self.check_computation_block(*kind, statements, value.as_deref()),
            Expr::NamedBlock {
                statements, value, ..
            } => self.check_block_expr(statements, value.as_deref()),
            Expr::MemoBlock {
                options,
                statements,
                value,
            } => self.check_memo_block_expr(options, statements, value.as_deref()),
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => self.check_if_expr(condition, then_branch, else_branch.as_deref()),
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
            ),
            Expr::Match { scrutinee, arms } => self.check_match_expr(scrutinee, arms),
            Expr::Raw(raw) => {
                self.errors.push(TypeCheckError::new(format!(
                    "raw expression is not type-checkable: {raw}"
                )));
                None
            }
        }
    }

    fn check_closure_expr(&mut self, params: &[String], body: &Expr) -> Option<TypeKind> {
        let local_snapshot =
            self.insert_scoped_locals(params.iter().map(|param| (param.clone(), TypeKind::I64)));
        self.check_expr(body);
        self.restore_scoped_locals(local_snapshot);
        None
    }

    fn in_seq_context(&self) -> bool {
        self.yield_stack
            .last()
            .is_some_and(|context| matches!(context, YieldContext::Seq { .. }))
    }

    fn check_entity_ref_expr(&mut self, entity: &EntityRefSyntax) -> Option<TypeKind> {
        if let Some(ty) = self.symbol_type(entity.body()).cloned() {
            return Some(ty);
        }
        entity
            .as_absolute()
            .and_then(entity_kind)
            .or_else(|| {
                entity.family_relative_ref().and_then(|relative| {
                    entity_kind(&arcweft_lang_syntax::ast::ids::EntityRef::new(
                        format!("{}._", relative.family()),
                        false,
                        *relative.range(),
                    ))
                })
            })
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
                } else {
                    self.errors.push(TypeCheckError::new(
                        "unsuffixed integer literal requires an expected integer type".to_owned(),
                    ));
                    TypeKind::Named("_".to_owned())
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
                } else {
                    self.errors.push(TypeCheckError::new(
                        "unsuffixed float literal requires an expected float type".to_owned(),
                    ));
                    TypeKind::Named("_".to_owned())
                }
            }
            arcweft_lang_syntax::expr::Literal::UnitNumber { suffix, .. } => {
                numeric_literal_suffix_type(Some(suffix.as_str()))
                    .unwrap_or_else(|| TypeKind::Named("_".to_owned()))
            }
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
        self.symbol_type(path).cloned().or_else(|| {
            self.check_dotted_path_target(path).or_else(|| {
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
            })
        })
    }

    fn check_pipe_expr(&mut self, lhs: &Expr, rhs: &Expr) -> Option<TypeKind> {
        if self.check_lifetime_pipe(lhs, rhs).is_some() {
            return Some(TypeKind::Unit);
        }
        self.check_expr(lhs);
        self.check_expr(rhs)
    }

    fn check_record_expr(&mut self, path: &str, fields: &[(String, Expr)]) -> TypeKind {
        self.check_record_fields(fields);
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

    fn check_tuple_expr(&mut self, items: &[Expr]) -> TypeKind {
        if items.is_empty() {
            return TypeKind::Unit;
        }
        TypeKind::Tuple(
            items
                .iter()
                .filter_map(|item| self.check_expr(item))
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
        } else if let Some(expected) = expected
            && let Some(ty) = unique_numeric_choice_alternative(expected, TypeKind::is_integer)
        {
            ty
        } else {
            self.errors.push(TypeCheckError::new(
                "unsuffixed integer sequence literal requires an expected integer type".to_owned(),
            ));
            TypeKind::Named("_".to_owned())
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
                        "float literal suffix must be a float type, found {ty:?}"
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
            if item.as_ref() != &item_type {
                self.errors.push(TypeCheckError::new(format!(
                    "array items must have type {:?}, found {item_type:?}",
                    item.as_ref()
                )));
            }
            return TypeKind::Array {
                item: item.clone(),
                len: len.clone(),
            };
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
            if item.as_ref() != &item_type {
                self.errors.push(TypeCheckError::new(format!(
                    "array repeat value must have type {:?}, found {item_type:?}",
                    item.as_ref()
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
    ) -> Option<TypeKind> {
        for (_, option) in options {
            self.check_expr(option);
        }
        self.check_block_expr(statements, value)
    }

    fn check_record_fields(&mut self, fields: &[(String, Expr)]) {
        for (_, value) in fields {
            self.check_expr(value);
        }
    }

    fn check_range_expr(&mut self, start: Option<&Expr>, end: Option<&Expr>) -> TypeKind {
        let start_type = start.and_then(|start| self.check_expr(start));
        if let Some(end) = end {
            self.check_expr_with_expected(end, start_type.as_ref());
        }
        TypeKind::Range
    }

    fn check_call_expr(&mut self, callee: &Expr, args: &[CallArg]) -> Option<TypeKind> {
        if let Some(ty) = self.check_builtin_call_expr(callee, args) {
            return Some(ty);
        }
        if let Some(name) = expr_path_label(callee)
            && let Some(ty) = self.check_agent_intrinsic_call_name(&name, args)
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
            if let Some(ty) = self.check_presentation_call(name, args) {
                return Some(ty);
            }
            if matches!(name.as_str(), "promote" | "promote_unchecked") {
                for arg in args.iter().filter_map(|arg| match arg {
                    CallArg::Named { value, .. } => Some(value.as_ref()),
                    CallArg::Positional(_) | CallArg::Spread { .. } => None,
                }) {
                    self.check_expr(arg);
                }
                return Some(TypeKind::Named("Promoted".to_owned()));
            }
            if name == "assume" {
                return Some(TypeKind::Unit);
            }
            if self.symbol_type(name) == Some(&TypeKind::entity_ref(EntityKind::Character)) {
                for arg in args {
                    self.check_expr(arg.value());
                }
                return Some(TypeKind::SpeakerPreset(EntityKind::Character));
            }
            if self.symbol_type(name) == Some(&TypeKind::SpeakerPreset(EntityKind::Character)) {
                for arg in args {
                    self.check_expr(arg.value());
                }
                return Some(TypeKind::SpeakerPreset(EntityKind::Character));
            }
            let arg_types = args
                .iter()
                .map(|arg| self.check_expr(arg.value()))
                .collect::<Vec<_>>();
            if name == "Ok" {
                return Some(TypeKind::Result {
                    ok: Box::new(first_arg_type(&arg_types)),
                    error: Box::new(TypeKind::Named("_".to_owned())),
                });
            }
            if name == "Err" {
                return Some(TypeKind::Result {
                    ok: Box::new(TypeKind::Named("_".to_owned())),
                    error: Box::new(first_arg_type(&arg_types)),
                });
            }
            if name == "Some" {
                return Some(TypeKind::Option(Box::new(first_arg_type(&arg_types))));
            }
            return self.function_type(name).cloned().or_else(|| {
                self.errors
                    .push(TypeCheckError::new(format!("unknown function `{name}`")));
                None
            });
        }
        match self.check_expr(callee) {
            Some(TypeKind::Speaker(entity) | TypeKind::SpeakerPreset(entity)) => {
                for arg in args {
                    self.check_expr(arg.value());
                }
                Some(TypeKind::SpeakerPreset(entity))
            }
            other => {
                for arg in args {
                    self.check_expr(arg.value());
                }
                other
            }
        }
    }

    fn check_agent_intrinsic_call_name(
        &mut self,
        name: &str,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        match name {
            "expect" => Some(self.check_agent_assert_intrinsic(name, args, "expect")),
            "deny" => Some(self.check_agent_assert_intrinsic(name, args, "deny")),
            "checkpoint" => Some(self.check_agent_record_text_intrinsic(
                name,
                args,
                "checkpoint name",
                &TypeKind::String,
            )),
            "note" => Some(self.check_agent_record_text_intrinsic(
                name,
                args,
                "note text",
                &TypeKind::DisplayText,
            )),
            "attach" => Some(self.check_agent_attach_intrinsic(name, args)),
            "choice_action" => Some(self.check_agent_choice_action_intrinsic(name, args)),
            "viewport" => {
                Some(self.check_agent_no_arg_intrinsic(name, args, TypeKind::CaptureTarget))
            }
            "layer" => Some(self.check_agent_layer_intrinsic(name, args)),
            "object" => Some(self.check_agent_object_intrinsic(name, args)),
            "capture" => Some(self.check_agent_capture_intrinsic(name, args)),
            "read_resource" => Some(self.check_agent_read_resource_intrinsic(name, args)),
            "signal" => {
                Some(self.check_agent_probe_intrinsic(name, args, &EntityKind::Signal, "signal"))
            }
            "metric" => {
                Some(self.check_agent_probe_intrinsic(name, args, &EntityKind::Metric, "metric"))
            }
            "state" => Some(self.check_agent_path_probe_intrinsic(
                name,
                args,
                "debug state path",
                &TypeKind::String,
            )),
            "observation" => Some(self.check_agent_path_probe_intrinsic(
                name,
                args,
                "observation field path",
                &TypeKind::String,
            )),
            "diagnostics" => Some(self.check_agent_no_arg_intrinsic(
                name,
                args,
                TypeKind::Named("Diagnostics".to_owned()),
            )),
            "exists" => Some(self.check_agent_exists_intrinsic(name, args)),
            "all" | "any" => Some(self.check_agent_predicate_list_intrinsic(name, args)),
            "not" => Some(self.check_agent_not_predicate_intrinsic(name, args)),
            "wait" => Some(self.check_agent_wait_intrinsic(name, args)),
            "advance_text" => {
                self.check_function_effects(name);
                Some(self.check_agent_no_arg_intrinsic(
                    name,
                    args,
                    agent_result(TypeKind::ActionResult),
                ))
            }
            "viewport_point" => Some(self.check_agent_viewport_point_intrinsic(name, args)),
            "pointer.click" => Some(self.check_agent_pointer_click_intrinsic(name, args)),
            "invoke" => Some(self.check_agent_invoke_intrinsic(name, args)),
            "rag.query" => Some(self.check_agent_rag_query_intrinsic(name, args)),
            _ => None,
        }
    }

    fn check_agent_assert_intrinsic(
        &mut self,
        name: &str,
        args: &[CallArg],
        context: &str,
    ) -> TypeKind {
        let mut condition_seen = false;
        let mut positional_index = 0usize;
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    match positional_index {
                        0 => {
                            condition_seen = true;
                            self.expect_expr_type(value, &TypeKind::Bool, context);
                        }
                        1 => {
                            self.expect_expr_type(value, &TypeKind::String, "assertion message");
                        }
                        _ => {
                            self.errors.push(TypeCheckError::new(format!(
                                "{name} received too many positional arguments"
                            )));
                            self.check_expr(value);
                        }
                    }
                    positional_index += 1;
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "message" => {
                    self.expect_expr_type(value, &TypeKind::String, "assertion message");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "{name} has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "{name} does not accept spread arguments"
                    )));
                    self.check_expr(value);
                }
            }
        }
        if !condition_seen {
            self.errors.push(TypeCheckError::new(format!(
                "{name} requires a condition argument"
            )));
        }
        TypeKind::Unit
    }

    fn check_agent_record_text_intrinsic(
        &mut self,
        name: &str,
        args: &[CallArg],
        context: &str,
        expected: &TypeKind,
    ) -> TypeKind {
        self.check_function_effects(name);
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::Unit;
        };
        self.expect_expr_type(arg, expected, context);
        TypeKind::Unit
    }

    fn check_agent_attach_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        self.check_function_effects(name);
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::Unit;
        };
        self.expect_expr_type(arg, &agent_attach_resource_type(), "attach resource");
        TypeKind::Unit
    }

    fn check_agent_choice_action_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::ActionTarget;
        };
        self.expect_expr_type(
            arg,
            &TypeKind::entity_ref(EntityKind::ChoiceOption),
            "choice_action choice",
        );
        TypeKind::ActionTarget
    }

    fn check_agent_no_arg_intrinsic(
        &mut self,
        name: &str,
        args: &[CallArg],
        return_type: TypeKind,
    ) -> TypeKind {
        if !args.is_empty() {
            self.errors.push(TypeCheckError::new(format!(
                "{name} does not accept arguments"
            )));
            for arg in args {
                self.check_expr(arg.value());
            }
        }
        return_type
    }

    fn check_agent_layer_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::CaptureTarget;
        };
        self.expect_expr_type(
            arg,
            &TypeKind::entity_ref(EntityKind::Layer),
            "layer target",
        );
        TypeKind::CaptureTarget
    }

    fn check_agent_object_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::CaptureTarget;
        };
        self.expect_expr_type(
            arg,
            &TypeKind::Named("ObservedObjectId".to_owned()),
            "object id",
        );
        TypeKind::CaptureTarget
    }

    fn check_agent_capture_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        self.check_function_effects(name);
        let mut target_seen = false;
        let mut positional_index = 0usize;
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    if positional_index == 0 {
                        target_seen = true;
                        self.expect_expr_type(value, &TypeKind::CaptureTarget, "capture target");
                    } else {
                        self.errors.push(TypeCheckError::new(
                            "capture received too many positional arguments".to_owned(),
                        ));
                        self.check_expr(value);
                    }
                    positional_index += 1;
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "name" => {
                    self.expect_expr_type(value, &TypeKind::String, "capture name");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "format" || arg_name == "kind" => {
                    self.check_expr(value);
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "capture has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "capture does not accept spread arguments".to_owned(),
                    ));
                    self.check_expr(value);
                }
            }
        }
        if !target_seen {
            self.errors.push(TypeCheckError::new(
                "capture requires a target argument".to_owned(),
            ));
        }
        agent_result(TypeKind::CaptureRef)
    }

    fn check_agent_viewport_point_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        let mut x_seen = false;
        let mut y_seen = false;
        let mut positional_index = 0usize;
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    match positional_index {
                        0 => {
                            x_seen = true;
                            self.expect_expr_type(value, &TypeKind::U32, "viewport_point x");
                        }
                        1 => {
                            y_seen = true;
                            self.expect_expr_type(value, &TypeKind::U32, "viewport_point y");
                        }
                        _ => {
                            self.errors.push(TypeCheckError::new(
                                "viewport_point received too many positional arguments".to_owned(),
                            ));
                            self.check_expr(value);
                        }
                    }
                    positional_index += 1;
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "x" => {
                    x_seen = true;
                    self.expect_expr_type(value, &TypeKind::U32, "viewport_point x");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "y" => {
                    y_seen = true;
                    self.expect_expr_type(value, &TypeKind::U32, "viewport_point y");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "{name} has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "viewport_point does not accept spread arguments".to_owned(),
                    ));
                    self.check_expr(value);
                }
            }
        }
        if !x_seen {
            self.errors
                .push(TypeCheckError::new("viewport_point requires x".to_owned()));
        }
        if !y_seen {
            self.errors
                .push(TypeCheckError::new("viewport_point requires y".to_owned()));
        }
        TypeKind::Named("ViewportPoint".to_owned())
    }

    fn check_agent_pointer_click_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        self.check_function_effects(name);
        let mut point_seen = false;
        let mut positional_index = 0usize;
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    if positional_index == 0 {
                        point_seen = true;
                        self.expect_expr_type(
                            value,
                            &TypeKind::Named("ViewportPoint".to_owned()),
                            "pointer.click point",
                        );
                    } else {
                        self.errors.push(TypeCheckError::new(
                            "pointer.click received too many positional arguments".to_owned(),
                        ));
                        self.check_expr(value);
                    }
                    positional_index += 1;
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "point" => {
                    point_seen = true;
                    self.expect_expr_type(
                        value,
                        &TypeKind::Named("ViewportPoint".to_owned()),
                        "pointer.click point",
                    );
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "button" => {
                    self.expect_expr_type(value, &TypeKind::ActionName, "pointer.click button");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "pointer.click has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "pointer.click does not accept spread arguments".to_owned(),
                    ));
                    self.check_expr(value);
                }
            }
        }
        if !point_seen {
            self.errors.push(TypeCheckError::new(
                "pointer.click requires a point argument".to_owned(),
            ));
        }
        agent_result(TypeKind::ActionResult)
    }

    fn check_agent_read_resource_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        self.check_function_effects(name);
        let mut uri_seen = false;
        let mut positional_index = 0usize;
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    if positional_index == 0 {
                        uri_seen = true;
                        self.expect_expr_type(value, &TypeKind::String, "resource uri");
                    } else {
                        self.errors.push(TypeCheckError::new(
                            "read_resource received too many positional arguments".to_owned(),
                        ));
                        self.check_expr(value);
                    }
                    positional_index += 1;
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "uri" => {
                    if uri_seen {
                        self.errors.push(TypeCheckError::new(
                            "read_resource received uri more than once".to_owned(),
                        ));
                    }
                    uri_seen = true;
                    self.expect_expr_type(value, &TypeKind::String, "resource uri");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "read_resource has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "read_resource does not accept spread arguments".to_owned(),
                    ));
                    self.check_expr(value);
                }
            }
        }
        if !uri_seen {
            self.errors.push(TypeCheckError::new(
                "read_resource requires a uri argument".to_owned(),
            ));
        }
        agent_result(TypeKind::AgentResource)
    }

    fn check_agent_probe_intrinsic(
        &mut self,
        name: &str,
        args: &[CallArg],
        expected_kind: &EntityKind,
        context: &str,
    ) -> TypeKind {
        self.check_function_effects(name);
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::Probe(Box::new(TypeKind::Named("_".to_owned())));
        };
        match self.check_expr(arg) {
            Some(TypeKind::Ref(entity)) if entity.kind() == expected_kind => {
                if let Some(value) = entity.value() {
                    TypeKind::Probe(Box::new(value.clone()))
                } else {
                    self.errors.push(TypeCheckError::new(format!(
                        "{context} probe requires a payload type in the project semantic index"
                    )));
                    TypeKind::Probe(Box::new(TypeKind::Named("_".to_owned())))
                }
            }
            Some(TypeKind::Ref(entity)) => {
                self.errors.push(TypeCheckError::new(format!(
                    "{context} probe argument must be a {expected_kind:?} reference, found {:?}",
                    entity.kind()
                )));
                TypeKind::Probe(Box::new(TypeKind::Named("_".to_owned())))
            }
            Some(actual) => {
                self.errors.push(TypeCheckError::new(format!(
                    "{context} probe argument must be a {expected_kind:?} reference, found {actual:?}"
                )));
                TypeKind::Probe(Box::new(TypeKind::Named("_".to_owned())))
            }
            None => TypeKind::Probe(Box::new(TypeKind::Named("_".to_owned()))),
        }
    }

    fn check_agent_path_probe_intrinsic(
        &mut self,
        name: &str,
        args: &[CallArg],
        context: &str,
        expected_path: &TypeKind,
    ) -> TypeKind {
        self.check_function_effects(name);
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::Probe(Box::new(TypeKind::AgentValue));
        };
        self.expect_expr_type(arg, expected_path, context);
        TypeKind::Probe(Box::new(TypeKind::AgentValue))
    }

    fn check_agent_exists_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::Predicate;
        };
        match self.check_expr(arg) {
            Some(TypeKind::Probe(_)) | None => {}
            Some(actual) => self.errors.push(TypeCheckError::new(format!(
                "exists argument must be a Probe, found {actual:?}"
            ))),
        }
        TypeKind::Predicate
    }

    fn check_agent_predicate_list_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        if args.is_empty() {
            self.errors.push(TypeCheckError::new(format!(
                "{name} requires at least one predicate argument"
            )));
        }
        if let [CallArg::Positional(Expr::BracketSeq(items))] = args {
            if items.is_empty() {
                self.errors.push(TypeCheckError::new(format!(
                    "{name} predicate list cannot be empty"
                )));
            }
            for item in items {
                self.expect_expr_type(item, &TypeKind::Predicate, name);
            }
            return TypeKind::Predicate;
        }
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    self.expect_expr_type(value, &TypeKind::Predicate, name);
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "{name} arguments must be positional, got named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "{name} arguments cannot be spread"
                    )));
                    self.check_expr(value);
                }
            }
        }
        TypeKind::Predicate
    }

    fn check_agent_not_predicate_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        let Some(arg) = self.single_positional_agent_arg(name, args) else {
            return TypeKind::Predicate;
        };
        self.expect_expr_type(arg, &TypeKind::Predicate, "not predicate");
        TypeKind::Predicate
    }

    fn check_agent_wait_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        self.check_function_effects(name);
        let mut predicate_seen = false;
        let mut timeout_seen = false;
        let mut positional_index = 0usize;

        for arg in args {
            match arg {
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "timeout" => {
                    timeout_seen = true;
                    self.expect_expr_type(value, &TypeKind::Duration, "wait timeout");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "stable_frames" || arg_name == "poll_frames" => {
                    self.expect_expr_type(value, &TypeKind::U32, &format!("wait {arg_name}"));
                    self.check_wait_positive_u32_literal(arg_name, value);
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "wait has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "wait does not accept spread arguments".to_owned(),
                    ));
                    self.check_expr(value);
                }
                CallArg::Positional(value) => {
                    match positional_index {
                        0 => {
                            predicate_seen = true;
                            self.expect_expr_type(value, &TypeKind::Predicate, "wait predicate");
                        }
                        1 => {
                            timeout_seen = true;
                            self.expect_expr_type(value, &TypeKind::Duration, "wait timeout");
                        }
                        _ => {
                            self.errors.push(TypeCheckError::new(
                                "wait received too many positional arguments".to_owned(),
                            ));
                            self.check_expr(value);
                        }
                    }
                    positional_index += 1;
                }
            }
        }
        if !predicate_seen {
            self.errors.push(TypeCheckError::new(
                "wait requires a predicate argument".to_owned(),
            ));
        }
        if !timeout_seen {
            self.errors
                .push(TypeCheckError::new("wait requires timeout".to_owned()));
        }
        TypeKind::Result {
            ok: Box::new(TypeKind::Observation),
            error: Box::new(TypeKind::Named("WaitError".to_owned())),
        }
    }

    fn check_agent_invoke_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        self.check_function_effects(name);
        let parsed = self.collect_agent_invoke_args(name, args);
        let target_id = parsed
            .target
            .and_then(|target| self.check_agent_invoke_target(target));
        let action_name = parsed
            .action
            .and_then(|action| self.check_agent_action_name(action));
        self.finish_agent_invoke(parsed, target_id, action_name)
    }

    fn collect_agent_invoke_args<'a>(
        &mut self,
        name: &str,
        args: &'a [CallArg],
    ) -> AgentInvokeArgs<'a> {
        let mut target = None;
        let mut action = None;
        let mut action_args = None;
        let mut positional_index = 0usize;

        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    match positional_index {
                        0 => {
                            set_agent_arg_slot(
                                &mut target,
                                value,
                                name,
                                "target",
                                &mut self.errors,
                            );
                        }
                        1 => {
                            set_agent_arg_slot(
                                &mut action,
                                value,
                                name,
                                "action",
                                &mut self.errors,
                            );
                        }
                        2 => set_agent_arg_slot(
                            &mut action_args,
                            value,
                            name,
                            "args",
                            &mut self.errors,
                        ),
                        _ => {
                            self.errors.push(TypeCheckError::new(
                                "invoke received too many positional arguments".to_owned(),
                            ));
                            self.check_expr(value);
                        }
                    }
                    positional_index += 1;
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "target" => {
                    set_agent_arg_slot(&mut target, value, name, arg_name, &mut self.errors);
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "action" => {
                    set_agent_arg_slot(&mut action, value, name, arg_name, &mut self.errors);
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "args" => {
                    set_agent_arg_slot(&mut action_args, value, name, arg_name, &mut self.errors);
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "invoke has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "invoke does not accept spread arguments".to_owned(),
                    ));
                    self.check_expr(value);
                }
            }
        }
        AgentInvokeArgs {
            target,
            action,
            action_args,
        }
    }

    fn finish_agent_invoke(
        &mut self,
        parsed: AgentInvokeArgs<'_>,
        target_id: Option<String>,
        action_name: Option<String>,
    ) -> TypeKind {
        if let (Some(target_id), Some(action_name)) = (target_id, action_name) {
            return self.check_resolved_agent_invoke(parsed.action_args, &target_id, &action_name);
        }
        if parsed.target.is_none() {
            self.errors.push(TypeCheckError::new(
                "invoke requires a target argument".to_owned(),
            ));
        }
        if parsed.action.is_none() {
            self.errors.push(TypeCheckError::new(
                "invoke requires an action argument".to_owned(),
            ));
        }
        if let Some(args) = parsed.action_args {
            self.check_agent_invoke_args(args, &[]);
        }
        agent_result(TypeKind::ActionResult)
    }

    fn check_resolved_agent_invoke(
        &mut self,
        action_args: Option<&Expr>,
        target_id: &str,
        action_name: &str,
    ) -> TypeKind {
        let Some(actions) = self.env.agent_actions(target_id) else {
            self.errors.push(TypeCheckError::new(format!(
                "invoke target `{target_id}` exposes no Agent actions"
            )));
            if let Some(args) = action_args {
                self.check_agent_invoke_args(args, &[]);
            }
            return agent_result(TypeKind::ActionResult);
        };
        let Some(signature) = actions
            .iter()
            .find(|signature| signature.action() == action_name)
            .cloned()
        else {
            self.errors.push(TypeCheckError::new(format!(
                "invoke target `{target_id}` has no Agent action `{action_name}`"
            )));
            if let Some(args) = action_args {
                self.check_agent_invoke_args(args, &[]);
            }
            return agent_result(TypeKind::ActionResult);
        };
        if let Some(args) = action_args {
            self.check_agent_invoke_args(args, signature.params());
        } else {
            self.check_agent_invoke_missing_args(target_id, action_name, signature.params());
        }
        agent_result(signature.return_type().clone())
    }

    fn check_agent_invoke_target(&mut self, target: &Expr) -> Option<String> {
        let actual = self.check_expr(target);
        if !actual
            .as_ref()
            .is_some_and(|ty| matches!(ty, TypeKind::Ref(_)))
        {
            self.errors.push(TypeCheckError::new(format!(
                "invoke target must be an entity reference, found {actual:?}"
            )));
        }
        match target {
            Expr::EntityRef(entity) => Some(entity.body().to_owned()),
            _ => None,
        }
    }

    fn check_agent_action_name(&mut self, action: &Expr) -> Option<String> {
        match action {
            Expr::Path(path) => Some(path.strip_prefix('.').unwrap_or(path).to_owned()),
            Expr::Literal(Literal::String(value)) => Some(value.clone()),
            _ => {
                self.errors.push(TypeCheckError::new(
                    "invoke action must be an ActionName literal such as `.open`".to_owned(),
                ));
                self.check_expr(action);
                None
            }
        }
    }

    fn check_agent_invoke_missing_args(
        &mut self,
        target_id: &str,
        action_name: &str,
        expected_params: &[AgentActionEnvParam],
    ) {
        let missing = expected_params
            .iter()
            .filter(|param| !param.has_default())
            .map(AgentActionEnvParam::name)
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            self.errors.push(TypeCheckError::new(format!(
                "invoke action `{action_name}` on `{target_id}` requires arg(s): {}",
                missing.join(", ")
            )));
        }
    }

    fn check_agent_invoke_args(&mut self, args: &Expr, expected_params: &[AgentActionEnvParam]) {
        if let Expr::RecordLiteral(fields) = args {
            self.check_agent_invoke_record_args(fields, expected_params);
            return;
        }

        let expected = TypeKind::Map {
            kind: MapKind::Sorted,
            key: Box::new(TypeKind::String),
            value: Box::new(TypeKind::AgentValue),
        };
        self.expect_expr_type(args, &expected, "invoke args");
    }

    fn check_agent_invoke_record_args(
        &mut self,
        fields: &[(String, Expr)],
        expected_params: &[AgentActionEnvParam],
    ) {
        let mut seen = std::collections::HashSet::new();
        for (field, value) in fields {
            if !seen.insert(field.as_str()) {
                self.errors.push(TypeCheckError::new(format!(
                    "invoke arg `{field}` was provided more than once"
                )));
            }
            let Some(param) = expected_params
                .iter()
                .find(|param| param.name() == field.as_str())
            else {
                self.errors.push(TypeCheckError::new(format!(
                    "invoke action has no arg named `{field}`"
                )));
                self.expect_expr_type(
                    value,
                    &TypeKind::AgentValue,
                    &format!("invoke arg `{field}`"),
                );
                continue;
            };
            self.expect_expr_type(value, param.ty(), &format!("invoke arg `{field}`"));
        }
        for param in expected_params
            .iter()
            .filter(|param| !param.has_default())
            .filter(|param| !seen.contains(param.name()))
        {
            self.errors.push(TypeCheckError::new(format!(
                "invoke action missing required arg `{}`",
                param.name()
            )));
        }
    }

    fn check_agent_rag_query_intrinsic(&mut self, name: &str, args: &[CallArg]) -> TypeKind {
        self.check_function_effects(name);
        let mut query_seen = false;
        let mut positional_index = 0usize;
        for arg in args {
            match arg {
                CallArg::Positional(value) => {
                    if positional_index == 0 {
                        query_seen = true;
                        self.expect_expr_type(value, &TypeKind::String, "rag query");
                    } else {
                        self.errors.push(TypeCheckError::new(
                            "rag.query received too many positional arguments".to_owned(),
                        ));
                        self.check_expr(value);
                    }
                    positional_index += 1;
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "roots" => {
                    self.check_agent_rag_roots_arg(value);
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "graph_depth" => {
                    self.expect_expr_type(value, &TypeKind::U32, "rag graph_depth");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } if arg_name == "limit" => {
                    self.expect_expr_type(value, &TypeKind::USize, "rag limit");
                }
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "rag.query has no parameter named `{arg_name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Spread { value } => {
                    self.errors.push(TypeCheckError::new(
                        "rag.query does not accept spread arguments".to_owned(),
                    ));
                    self.check_expr(value);
                }
            }
        }
        if !query_seen {
            self.errors.push(TypeCheckError::new(
                "rag.query requires a query argument".to_owned(),
            ));
        }
        TypeKind::Result {
            ok: Box::new(TypeKind::RagContextPack),
            error: Box::new(TypeKind::Named("RagError".to_owned())),
        }
    }

    fn check_agent_rag_roots_arg(&mut self, value: &Expr) {
        if let Expr::BracketSeq(items) = value {
            for item in items {
                self.expect_agent_rag_root_expr(item);
            }
            return;
        }

        let Some(actual) = self.check_expr(value) else {
            return;
        };
        let Some(item) = spread_item_type(&actual) else {
            self.errors.push(TypeCheckError::new(format!(
                "rag.query roots must be a sequence of entity references, found {actual:?}"
            )));
            return;
        };
        if !matches!(item, TypeKind::Ref(_)) {
            self.errors.push(TypeCheckError::new(format!(
                "rag.query roots items must be entity references, found {item:?}"
            )));
        }
    }

    fn expect_agent_rag_root_expr(&mut self, value: &Expr) {
        match self.check_expr(value) {
            Some(TypeKind::Ref(_)) | None => {}
            Some(actual) => self.errors.push(TypeCheckError::new(format!(
                "rag.query roots items must be entity references, found {actual:?}"
            ))),
        }
    }

    fn check_wait_positive_u32_literal(&mut self, name: &str, value: &Expr) {
        if let Expr::Literal(Literal::Int { value: literal, .. }) = value
            && *literal < 1
        {
            self.errors.push(TypeCheckError::new(format!(
                "wait {name} must be at least 1"
            )));
        }
    }

    fn single_positional_agent_arg<'a>(
        &mut self,
        name: &str,
        args: &'a [CallArg],
    ) -> Option<&'a Expr> {
        let mut positional = args.iter().filter_map(|arg| match arg {
            CallArg::Positional(value) => Some(value),
            CallArg::Named {
                name: arg_name,
                value,
            } => {
                self.errors.push(TypeCheckError::new(format!(
                    "{name} arguments must be positional, got named `{arg_name}`"
                )));
                self.check_expr(value);
                None
            }
            CallArg::Spread { value } => {
                self.errors.push(TypeCheckError::new(format!(
                    "{name} arguments cannot be spread"
                )));
                self.check_expr(value);
                None
            }
        });
        let first = positional.next();
        if positional.next().is_some() {
            self.errors.push(TypeCheckError::new(format!(
                "{name} requires exactly one positional argument"
            )));
        }
        if first.is_none() {
            self.errors.push(TypeCheckError::new(format!(
                "{name} requires one positional argument"
            )));
        }
        first
    }

    fn check_builtin_call_expr(&mut self, callee: &Expr, args: &[CallArg]) -> Option<TypeKind> {
        let name = expr_path_label(callee)?;
        self.check_builtin_call_name(&name, args)
    }

    fn check_builtin_call_name(&mut self, name: &str, args: &[CallArg]) -> Option<TypeKind> {
        if let Some(ty) = self.check_std_float_call_name(name, args) {
            return Some(ty);
        }
        match name {
            "fallback" | "InlineFailure.fallback" => {
                Some(TypeKind::Named("InlineFailure".to_owned()))
            }
            "panic" | "fail" | "bail" => {
                for arg in args {
                    self.check_expr(arg.value());
                }
                Some(TypeKind::Never)
            }
            "ensure" => {
                self.check_assert_like_args(args, "ensure");
                Some(TypeKind::Unit)
            }
            "assert" | "debug_assert" => {
                self.check_assert_like_args(args, name);
                Some(TypeKind::Unit)
            }
            "math.matmul_f32" | "math.matrix_add_f32" => {
                self.check_math_binary_args(args, "MatrixF32");
                Some(TypeKind::Named("MatrixF32".to_owned()))
            }
            "math.tensor_add_f32" => {
                self.check_math_binary_args(args, "TensorF32");
                Some(TypeKind::Named("TensorF32".to_owned()))
            }
            "math.matmul_f64" | "math.matrix_add_f64" => {
                self.check_math_binary_args(args, "MatrixF64");
                Some(TypeKind::Named("MatrixF64".to_owned()))
            }
            "math.tensor_add_f64" => {
                self.check_math_binary_args(args, "TensorF64");
                Some(TypeKind::Named("TensorF64".to_owned()))
            }
            _ => None,
        }
    }

    fn check_std_float_call_name(&mut self, name: &str, args: &[CallArg]) -> Option<TypeKind> {
        let (input, output, arity) = match name {
            "std.f32.abs" | "std.f32.floor" | "std.f32.ceil" | "std.f32.round"
            | "std.f32.trunc" | "std.f32.fract" | "std.f32.sqrt" | "std.f32.sin"
            | "std.f32.cos" | "std.f32.tan" | "std.f32.exp" | "std.f32.exp2" | "std.f32.ln"
            | "std.f32.log2" | "std.f32.log10" => (TypeKind::F32, TypeKind::F32, 1),
            "std.f32.powf" | "std.f32.atan2" => (TypeKind::F32, TypeKind::F32, 2),
            "std.f32.mul_add" => (TypeKind::F32, TypeKind::F32, 3),
            "std.f32.is_nan"
            | "std.f32.is_infinite"
            | "std.f32.is_finite"
            | "std.f32.is_sign_positive"
            | "std.f32.is_sign_negative" => (TypeKind::F32, TypeKind::Bool, 1),
            "std.f32.to_bits" => (TypeKind::F32, TypeKind::U32, 1),
            "std.f32.from_bits" => (TypeKind::U32, TypeKind::F32, 1),
            "std.f32.to_f64" => (TypeKind::F32, TypeKind::F64, 1),
            "std.f64.abs" | "std.f64.floor" | "std.f64.ceil" | "std.f64.round"
            | "std.f64.trunc" | "std.f64.fract" | "std.f64.sqrt" | "std.f64.sin"
            | "std.f64.cos" | "std.f64.tan" | "std.f64.exp" | "std.f64.exp2" | "std.f64.ln"
            | "std.f64.log2" | "std.f64.log10" => (TypeKind::F64, TypeKind::F64, 1),
            "std.f64.powf" | "std.f64.atan2" => (TypeKind::F64, TypeKind::F64, 2),
            "std.f64.mul_add" => (TypeKind::F64, TypeKind::F64, 3),
            "std.f64.is_nan"
            | "std.f64.is_infinite"
            | "std.f64.is_finite"
            | "std.f64.is_sign_positive"
            | "std.f64.is_sign_negative" => (TypeKind::F64, TypeKind::Bool, 1),
            "std.f64.to_bits" => (TypeKind::F64, TypeKind::U64, 1),
            "std.f64.from_bits" => (TypeKind::U64, TypeKind::F64, 1),
            "std.f64.to_f32" => (TypeKind::F64, TypeKind::F32, 1),
            _ => return None,
        };
        self.check_homogeneous_builtin_args(name, args, &input, arity);
        Some(output)
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

    fn check_untyped_function_args(&mut self, name: &str, args: &[CallArg]) {
        let checked_args = if name == "event.emit" {
            args.iter().skip(1).collect::<Vec<_>>()
        } else {
            args.iter().collect::<Vec<_>>()
        };
        for arg in checked_args {
            self.check_expr(arg.value());
        }
    }

    fn check_signature_call_args(
        &mut self,
        name: &str,
        signature: &FunctionSignature,
        args: &[CallArg],
    ) {
        let fixed = signature
            .params
            .iter()
            .filter(|param| !param.is_rest())
            .collect::<Vec<_>>();
        let rest = signature.params.iter().find(|param| param.is_rest());
        let mut provided_fixed = vec![false; fixed.len()];
        let mut positional_index = 0;

        for arg in args {
            match arg {
                CallArg::Named {
                    name: arg_name,
                    value,
                } => {
                    self.check_named_signature_arg(
                        name,
                        arg_name,
                        value,
                        &fixed,
                        rest,
                        &mut provided_fixed,
                    );
                }
                CallArg::Spread { value } => {
                    self.check_signature_spread_arg(name, value, rest, &fixed, &provided_fixed);
                }
                CallArg::Positional(positional) => {
                    while positional_index < fixed.len() && provided_fixed[positional_index] {
                        positional_index += 1;
                    }
                    if let Some(param) = fixed.get(positional_index) {
                        provided_fixed[positional_index] = true;
                        let label = signature_param_label(param, positional_index);
                        positional_index += 1;
                        self.expect_signature_arg_type(name, &label, positional, &param.ty);
                    } else if let Some(param) = rest {
                        let label = param.name.as_deref().unwrap_or("#rest");
                        self.expect_signature_arg_type(name, label, positional, &param.ty);
                    } else {
                        self.errors.push(TypeCheckError::new(format!(
                            "function `{name}` received too many positional arguments"
                        )));
                        self.check_expr(positional);
                    }
                }
            }
        }

        for (index, param) in fixed.iter().enumerate() {
            if !provided_fixed[index] && !param.has_default {
                let label = param
                    .name
                    .as_deref()
                    .map_or_else(|| format!("#{index}"), ToOwned::to_owned);
                self.errors.push(TypeCheckError::new(format!(
                    "function `{name}` missing required argument `{label}`"
                )));
            }
        }
    }

    fn check_signature_spread_arg(
        &mut self,
        function_name: &str,
        value: &Expr,
        rest: Option<&FunctionParam>,
        fixed: &[&FunctionParam],
        provided_fixed: &[bool],
    ) {
        let Some(rest) = rest else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` does not accept spread arguments"
            )));
            self.check_expr(value);
            return;
        };
        if fixed
            .iter()
            .zip(provided_fixed.iter().copied())
            .any(|(param, provided)| !provided && !param.has_default)
        {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` spread argument must appear after required fixed arguments"
            )));
            self.check_expr(value);
            return;
        }
        let actual = self.check_expr(value);
        let Some(actual) = actual.as_ref() else {
            return;
        };
        let Some(item) = spread_item_type(actual) else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` spread argument must have sequence type for rest parameter `{}`",
                rest.name.as_deref().unwrap_or("#rest")
            )));
            return;
        };
        if !self.types_compatible(&rest.ty, item) {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` spread items must have type {:?}, found {:?}",
                rest.ty, item
            )));
        }
    }

    fn check_named_signature_arg(
        &mut self,
        function_name: &str,
        arg_name: &str,
        value: &Expr,
        fixed: &[&FunctionParam],
        rest: Option<&FunctionParam>,
        provided_fixed: &mut [bool],
    ) {
        if rest.and_then(|param| param.name.as_deref()) == Some(arg_name) {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` rest parameter `{arg_name}` is positional-only"
            )));
            self.check_expr(value);
            return;
        }
        let Some(index) = fixed
            .iter()
            .position(|param| param.name.as_deref() == Some(arg_name))
        else {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` has no parameter named `{arg_name}`"
            )));
            self.check_expr(value);
            return;
        };
        if provided_fixed[index] {
            self.errors.push(TypeCheckError::new(format!(
                "function `{function_name}` argument `{arg_name}` was provided more than once"
            )));
        }
        provided_fixed[index] = true;
        self.expect_signature_arg_type(function_name, arg_name, value, &fixed[index].ty);
    }

    fn expect_signature_arg_type(
        &mut self,
        function_name: &str,
        arg_label: &str,
        arg: &Expr,
        expected: &TypeKind,
    ) {
        let actual = self.check_expr_with_expected(arg, Some(expected));
        if let Some(actual) = actual.as_ref()
            && !self.types_compatible(expected, actual)
        {
            self.errors.push(TypeCheckError::argument_type_mismatch(
                function_name,
                arg_label,
                expected.clone(),
                actual.clone(),
            ));
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
                        "negation operand must be numeric or Duration, found {other:?}"
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
    ) -> Option<TypeKind> {
        let method_name = method.split_once('<').map_or(method, |(name, _)| name);
        if let Some(receiver_path) = expr_path_label(receiver) {
            let dotted = format!("{receiver_path}.{method_name}");
            if receiver_path == "math" {
                return self.check_builtin_call_name(&dotted, args);
            }
            if matches!(receiver_path.as_str(), "std.f32" | "std.f64") {
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
            self.check_typed_method_call(receiver_type, method_name, args)
        })
    }

    fn check_typed_method_call(
        &mut self,
        receiver_type: TypeKind,
        method_name: &str,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        if method_name == "traverse" {
            return self.check_traverse_method_call(&receiver_type, args);
        }
        if method_name == "parallel" {
            return self.check_parallel_method_call(&receiver_type, args);
        }
        if method_name == "len" {
            return self.check_sequence_len_method_call(&receiver_type, args);
        }
        if method_name == "map" {
            return self.check_vec_map_method_call(&receiver_type, args);
        }
        if method_name == "sum" {
            return self.check_vec_sum_method_call(&receiver_type, args);
        }
        if method_name == "contains" {
            return Some(self.check_sequence_contains_method_call(&receiver_type, args));
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
        if let Some(signature) = self
            .env
            .method_signature(&receiver_type, method_name)
            .filter(|signature| signature.checks_args())
            .cloned()
        {
            self.check_signature_call_args(method_name, &signature, args);
            return Some(signature.return_type().clone());
        }
        self.check_untyped_method_args(args);
        self.env
            .method_type(&receiver_type, method_name)
            .cloned()
            .or_else(|| well_known_capacity_method_type(&receiver_type, method_name, args.len()))
            .or_else(|| {
                self.errors.push(TypeCheckError::new(format!(
                    "unknown method `{method_name}` on {receiver_type:?}"
                )));
                None
            })
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

    fn check_vec_map_method_call(
        &mut self,
        receiver_type: &TypeKind,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        let Some(item) = spread_item_type(receiver_type) else {
            self.errors.push(TypeCheckError::new(format!(
                "map receiver must be an iterable sequence, found {receiver_type:?}"
            )));
            for arg in args {
                self.check_expr(arg.value());
            }
            return None;
        };
        let [arg] = args else {
            self.errors.push(TypeCheckError::new(
                "map requires exactly one closure".to_owned(),
            ));
            for arg in args {
                self.check_expr(arg.value());
            }
            return None;
        };
        if arg.name().is_some() || arg.is_spread() {
            self.errors.push(TypeCheckError::new(
                "map requires one positional closure argument".to_owned(),
            ));
            self.check_expr(arg.value());
            return None;
        }
        let Expr::Closure { params, body } = arg.value() else {
            self.errors.push(TypeCheckError::new(
                "map requires a closure argument".to_owned(),
            ));
            self.check_expr(arg.value());
            return None;
        };
        let [param] = params.as_slice() else {
            self.errors.push(TypeCheckError::new(
                "map closures must bind exactly one parameter".to_owned(),
            ));
            return None;
        };
        let snapshot = self.insert_scoped_locals([(param.clone(), item.clone())]);
        let body_type = self.check_expr(body);
        self.restore_scoped_locals(snapshot);
        body_type.map(|ty| TypeKind::Vec(Box::new(ty)))
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
            TypeKind::Vec(_) | TypeKind::Seq(_) | TypeKind::Slice(_) | TypeKind::Array { .. } => {
                Some(TypeKind::USize)
            }
            other => {
                self.errors.push(TypeCheckError::new(format!(
                    "len receiver must be an iterable sequence, found {other:?}"
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
                    "sum receiver must be an iterable sequence, found {other:?}"
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
                "contains receiver must be an iterable sequence, found {receiver_type:?}"
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
                "traverse receiver must be Vec<T>, found {receiver_type:?}"
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
                    "parallel receiver must be Need<Vec<T>, E>, found {other:?}"
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
        match self.check_expr(target) {
            Some(TypeKind::Observation) => agent_observation_field_type(field),
            Some(TypeKind::ObservedObject) => agent_observed_object_field_type(field),
            Some(TypeKind::AgentBBox) => agent_bbox_field_type(field),
            Some(TypeKind::ActionTarget) => agent_action_target_field_type(field),
            Some(TypeKind::ActionResult) => agent_action_result_field_type(field),
            Some(TypeKind::CaptureRef) => agent_capture_ref_field_type(field),
            Some(TypeKind::AgentResource) => agent_resource_field_type(field),
            Some(TypeKind::AgentResourceBody) => agent_resource_body_field_type(field),
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
                self.check_try_error(error.as_ref());
                Some(*ok)
            }
            Some(TypeKind::Named(name)) => result_ok_type(&name).or_else(|| {
                self.errors.push(TypeCheckError::new(format!(
                    "`?` requires Result<T, E> or Option<T>, found Named({name:?})"
                )));
                None
            }),
            Some(other) => {
                self.errors.push(TypeCheckError::new(format!(
                    "`?` requires Result<T, E> or Option<T>, found {other:?}"
                )));
                None
            }
            None => None,
        }
    }

    fn check_try_error(&mut self, actual_error: &TypeKind) {
        let Some(TypeKind::Result { error, .. }) = self.expected_returns.last() else {
            return;
        };
        let expected_error = error.as_ref().clone();
        if !self.types_compatible(&expected_error, actual_error) {
            self.errors.push(TypeCheckError::new(format!(
                "`?` error type {actual_error:?} cannot be injected into return error type {expected_error:?}"
            )));
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
    ) -> Option<TypeKind> {
        self.expect_expr_type(condition, &TypeKind::Bool, "if expression condition");
        let base_borrow_checkpoint = self.checkpoint_borrow_state();
        let then_type = self.check_expr(then_branch);
        let then_borrow_state = self.capture_borrow_state_delta(base_borrow_checkpoint);
        self.restore_borrow_state(base_borrow_checkpoint);
        let else_type = else_branch.and_then(|branch| self.check_expr(branch));
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
        match (then_type, else_type) {
            (Some(TypeKind::Never), Some(else_type)) => Some(else_type),
            (Some(then_type), Some(TypeKind::Never)) => Some(then_type),
            (Some(then_type), Some(else_type)) => Some(join_branch_types(then_type, else_type)),
            _ => None,
        }
    }

    fn check_match_expr(&mut self, scrutinee: &Expr, arms: &[MatchExprArm]) -> Option<TypeKind> {
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
        for arm in arms {
            self.restore_borrow_state(base_borrow_checkpoint);
            let local_snapshot = self
                .insert_scoped_locals(let_else_bindings(arm.pattern(), scrutinee_type.as_ref()));
            if let Some(guard) = arm.guard() {
                self.expect_expr_type(guard, &TypeKind::Bool, "match arm guard");
            }
            let arm_type = self.check_expr_with_expected(arm.value(), inferred.as_ref());
            self.restore_scoped_locals(local_snapshot);
            match (&inferred, arm_type) {
                (None, Some(ty)) => inferred = Some(ty),
                (Some(existing), Some(ty)) if existing == &ty => {}
                (Some(existing), Some(TypeKind::Never)) => {
                    inferred = Some(existing.clone());
                }
                (Some(TypeKind::Never), Some(ty)) => {
                    inferred = Some(ty);
                }
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
        inferred
    }

    fn check_if_let_expr(
        &mut self,
        pattern: &Pattern,
        expr: &Expr,
        guard: Option<&Expr>,
        then_branch: &Expr,
        else_branch: Option<&Expr>,
    ) -> Option<TypeKind> {
        let expr_type = self.check_expr(expr);
        if let Some(guard) = guard {
            self.expect_expr_type(guard, &TypeKind::Bool, "if-let expression guard");
        }

        let base_borrow_checkpoint = self.checkpoint_borrow_state();
        let local_snapshot =
            self.insert_scoped_locals(let_else_bindings(pattern, expr_type.as_ref()));
        let then_type = self.check_expr(then_branch);
        let then_borrow_state = self.capture_borrow_state_delta(base_borrow_checkpoint);
        self.restore_borrow_state(base_borrow_checkpoint);
        self.restore_scoped_locals(local_snapshot);

        let else_type = else_branch.and_then(|branch| self.check_expr(branch));
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
        match (then_type, else_type) {
            (Some(then_type), Some(else_type)) => Some(join_branch_types(then_type, else_type)),
            _ => None,
        }
    }

    fn check_binary_expr(&mut self, lhs: &Expr, op: BinaryOp, rhs: &Expr) -> Option<TypeKind> {
        let lhs_type = self.check_expr(lhs);
        let rhs_expected = rhs_expected_type_for_binary(op, lhs_type.as_ref());
        let rhs_type = self.check_expr_with_expected(rhs, rhs_expected);
        match op {
            BinaryOp::In => {
                if rhs_type != Some(TypeKind::Range) {
                    self.errors.push(TypeCheckError::new(format!(
                        "`in` expression requires a range on the right, found {rhs_type:?}"
                    )));
                    return None;
                }
                Some(TypeKind::Bool)
            }
            BinaryOp::Implies | BinaryOp::Or | BinaryOp::And => {
                if lhs_type != Some(TypeKind::Bool) || rhs_type != Some(TypeKind::Bool) {
                    self.errors.push(TypeCheckError::new(format!(
                        "logical contract expression must use Bool operands, found {lhs_type:?} and {rhs_type:?}"
                    )));
                    return None;
                }
                Some(TypeKind::Bool)
            }
            BinaryOp::Eq
            | BinaryOp::NotEq
            | BinaryOp::Gte
            | BinaryOp::Lte
            | BinaryOp::Gt
            | BinaryOp::Lt => Some(TypeKind::Bool),
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
                        "merge operator `&` requires compatible patch operands, found {lhs:?} and {rhs:?}"
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
                        "arithmetic expression operands must have a supported numeric or Duration type, found {lhs_type:?} and {rhs_type:?}"
                    )));
                    None
                }
            }
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

enum ChoicePatternCoverage {
    All,
    Type(TypeKind),
}

#[derive(Clone, Copy)]
struct AgentInvokeArgs<'a> {
    target: Option<&'a Expr>,
    action: Option<&'a Expr>,
    action_args: Option<&'a Expr>,
}

fn choice_pattern_coverage(pattern: &Pattern) -> ChoicePatternCoverage {
    match pattern {
        Pattern::Typed { ty, .. } => ChoicePatternCoverage::Type(type_ref_kind(ty)),
        Pattern::Whole { pattern, .. } => choice_pattern_coverage(pattern),
        Pattern::Ident(_) | Pattern::MutIdent(_) | Pattern::Discard => ChoicePatternCoverage::All,
        Pattern::Literal(_)
        | Pattern::Entity(_)
        | Pattern::Variant { .. }
        | Pattern::Tuple(_)
        | Pattern::Record { .. }
        | Pattern::BracketSeq { .. }
        | Pattern::Raw(_) => ChoicePatternCoverage::Type(TypeKind::Never),
    }
}

fn unique_numeric_choice_alternative(
    expected: &TypeKind,
    predicate: impl Fn(&TypeKind) -> bool,
) -> Option<TypeKind> {
    let TypeKind::Choice(alternatives) = expected else {
        return None;
    };
    let mut compatible_alternatives = alternatives
        .iter()
        .filter(|alternative| predicate(alternative));
    let selected = compatible_alternatives.next()?;
    compatible_alternatives
        .next()
        .is_none()
        .then(|| selected.clone())
}

fn spread_item_type(ty: &TypeKind) -> Option<&TypeKind> {
    match ty {
        TypeKind::Vec(item)
        | TypeKind::Seq(item)
        | TypeKind::Slice(item)
        | TypeKind::Array { item, .. } => Some(item),
        _ => None,
    }
}

fn join_branch_types(left: TypeKind, right: TypeKind) -> TypeKind {
    if left == right {
        left
    } else {
        normalize_choice_type(vec![left, right])
    }
}

fn rhs_expected_type_for_binary(op: BinaryOp, lhs_type: Option<&TypeKind>) -> Option<&TypeKind> {
    let lhs_type = lhs_type?;
    match op {
        BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Rem
        | BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Gte
        | BinaryOp::Lte
        | BinaryOp::Gt
        | BinaryOp::Lt
        | BinaryOp::In
            if lhs_type.is_integer() || lhs_type.is_float() || lhs_type == &TypeKind::Duration =>
        {
            Some(lhs_type)
        }
        _ => None,
    }
}

fn expr_kind_name(expr: &Expr) -> &'static str {
    match expr {
        Expr::Literal(_) => "literal",
        Expr::EntityRef(_) => "entity_ref",
        Expr::LifetimePath { .. } => "lifetime_path",
        Expr::Path(_) => "path",
        Expr::Placeholder(_) => "placeholder",
        Expr::Tuple(_) => "tuple",
        Expr::BracketSeq(_) => "bracket_seq",
        Expr::NumericBracketSeq(_) => "numeric_bracket_seq",
        Expr::ArrayRepeat { .. } => "array_repeat",
        Expr::Call { .. } => "call",
        Expr::MethodCall { .. } => "method_call",
        Expr::Field { .. } => "field",
        Expr::DialogueCall { .. } => "dialogue_call",
        Expr::Index { .. } => "index",
        Expr::Pipe { .. } => "pipe",
        Expr::Try { .. } => "try",
        Expr::Await { .. } => "await",
        Expr::Thread { .. } => "thread",
        Expr::Range { .. } => "range",
        Expr::Record { .. } => "record",
        Expr::RecordLiteral(_) => "record_literal",
        Expr::Binary { .. } => "binary",
        Expr::Closure { .. } => "closure",
        Expr::Unary { .. } => "unary",
        Expr::Block { .. } => "block",
        Expr::ComputationBlock { .. } => "computation_block",
        Expr::NamedBlock { .. } => "named_block",
        Expr::MemoBlock { .. } => "memo_block",
        Expr::If { .. } => "if",
        Expr::IfLet { .. } => "if_let",
        Expr::Match { .. } => "match",
        Expr::Raw(_) => "raw",
    }
}

fn collection_index_key_type(target_type: &TypeKind) -> Option<TypeKind> {
    match target_type {
        TypeKind::Vec(_) | TypeKind::Array { .. } | TypeKind::Slice(_) | TypeKind::String => {
            Some(TypeKind::I64)
        }
        TypeKind::Map { key, .. } => Some(key.as_ref().clone()),
        TypeKind::Named(name) => map_key_type_from_name(name),
        _ => None,
    }
}

fn agent_observation_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "tick" => TypeKind::U64,
        "frame_id" | "state_hash" | "render_hash" => TypeKind::String,
        "actions" => TypeKind::Vec(Box::new(TypeKind::ActionTarget)),
        "objects" => TypeKind::Vec(Box::new(TypeKind::ObservedObject)),
        "signals" => TypeKind::Map {
            kind: MapKind::BTree,
            key: Box::new(TypeKind::AgentValue),
            value: Box::new(TypeKind::AgentValue),
        },
        _ => return None,
    })
}

fn agent_observed_object_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "id" => TypeKind::Named("ObservedObjectId".to_owned()),
        "parent_id" | "entity" | "layer" | "role" | "text" => TypeKind::String,
        "visible" | "enabled" => TypeKind::Bool,
        "bbox" => TypeKind::AgentBBox,
        _ => return None,
    })
}

fn agent_bbox_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "space" => TypeKind::String,
        "x" | "y" | "width" | "height" => TypeKind::U32,
        _ => return None,
    })
}

fn agent_action_result_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "accepted" => TypeKind::Bool,
        "before_tick" | "after_tick" => TypeKind::U64,
        "before_state_hash" | "after_state_hash" => TypeKind::String,
        _ => return None,
    })
}

fn agent_action_target_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "id" | "target" | "action" | "kind" => TypeKind::String,
        "enabled" => TypeKind::Bool,
        _ => return None,
    })
}

fn agent_capture_ref_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "uri" | "content_hash" | "media_type" => TypeKind::String,
        "byte_len" => TypeKind::U64,
        _ => return None,
    })
}

fn agent_resource_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "uri" | "kind" | "mime_type" | "hash" => TypeKind::String,
        "body" => TypeKind::AgentResourceBody,
        _ => return None,
    })
}

fn agent_resource_body_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "kind" | "json" | "text" | "base64" | "encoding" => TypeKind::String,
        "value" => TypeKind::AgentValue,
        _ => return None,
    })
}

fn agent_attach_resource_type() -> TypeKind {
    TypeKind::Choice(vec![TypeKind::CaptureRef, TypeKind::AgentResource])
}

fn agent_result(ok: TypeKind) -> TypeKind {
    TypeKind::Result {
        ok: Box::new(ok),
        error: Box::new(TypeKind::Named("AgentError".to_owned())),
    }
}

fn set_agent_arg_slot<'a>(
    slot: &mut Option<&'a Expr>,
    value: &'a Expr,
    function_name: &str,
    arg_name: &str,
    errors: &mut Vec<TypeCheckError>,
) {
    if slot.replace(value).is_some() {
        errors.push(TypeCheckError::new(format!(
            "{function_name} argument `{arg_name}` was provided more than once"
        )));
    }
}

fn signature_param_label(param: &FunctionParam, index: usize) -> String {
    param
        .name
        .as_deref()
        .map_or_else(|| format!("#{index}"), ToOwned::to_owned)
}

fn map_key_type_from_name(name: &str) -> Option<TypeKind> {
    let (_, args) = name.split_once('<')?;
    let args = args.strip_suffix('>')?;
    let (key, _) = args.split_once(',')?;
    Some(match key.trim() {
        "Character" | "Ref<Character>" => TypeKind::entity_ref(EntityKind::Character),
        other => named_type_label(other),
    })
}

fn is_character_speaker_type(ty: &TypeKind) -> bool {
    ty.is_entity_ref_kind(&EntityKind::Character)
        || matches!(
            ty,
            TypeKind::Speaker(EntityKind::Character)
                | TypeKind::SpeakerPreset(EntityKind::Character)
        )
}

fn is_unit_number_type(ty: &TypeKind) -> bool {
    matches!(ty, TypeKind::Named(name) if matches!(
        name.as_str(),
        "Length" | "Angle" | "AudioLevel" | "Tempo"
    ))
}

fn std_float_constant_type(path: &str) -> Option<TypeKind> {
    Some(match path {
        "std.f32.nan"
        | "std.f32.infinity"
        | "std.f32.neg_infinity"
        | "std.f32.epsilon"
        | "std.f32.min"
        | "std.f32.max"
        | "std.f32.pi"
        | "std.f32.tau" => TypeKind::F32,
        "std.f64.nan"
        | "std.f64.infinity"
        | "std.f64.neg_infinity"
        | "std.f64.epsilon"
        | "std.f64.min"
        | "std.f64.max"
        | "std.f64.pi"
        | "std.f64.tau" => TypeKind::F64,
        _ => return None,
    })
}

fn inline_failure_builtin_variant_type(path: &str) -> Option<TypeKind> {
    Some(match path {
        "InlineFailure.fail" | "InlineFailure.line_error" | "InlineFailure.discard" => {
            TypeKind::Named("InlineFailure".to_owned())
        }
        "InlineFallback.expr_source"
        | "InlineFallback.call_source"
        | "InlineFallback.value_plain" => TypeKind::Named("InlineFallback".to_owned()),
        "FallbackStyle.plain" | "FallbackStyle.inherit" => {
            TypeKind::Named("FallbackStyle".to_owned())
        }
        _ => return None,
    })
}

fn looks_like_os_absolute_path(path: &str) -> bool {
    path.starts_with('/')
        || path.starts_with('\\')
        || path.as_bytes().get(1).is_some_and(|byte| *byte == b':')
}
