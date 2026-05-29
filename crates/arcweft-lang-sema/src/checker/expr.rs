//! Expression type-checking entry points and expression-kind dispatch.

use super::helpers::{
    array_len_matches, array_repeat_len_label, collection_index_type, expr_path_label,
    first_arg_type, is_drop_name, let_else_bindings, named_type_label, numeric_literal_suffix_type,
    result_ok_type, well_known_capacity_method_type, well_known_field_type,
    well_known_runtime_method_type,
};
use super::{
    BorrowLocalState, EntityKind, EntityRefSyntax, Expr, LifetimeScopeKind, Pattern, Stmt,
    TypeCheckError, TypeChecker, TypeJudgmentRule, TypeJudgmentSubject, TypeKind, YieldContext,
    entity_kind,
};
use arcweft_lang_syntax::ast::line_plan::LinePlan;
use arcweft_lang_syntax::expr::{BinaryOp, ComputationBlockKind, MatchExprArm, UnaryOp};

impl TypeChecker<'_> {
    pub(super) fn expect_expr_type(&mut self, expr: &Expr, expected: &TypeKind, context: &str) {
        let actual = self.check_expr_with_expected(expr, Some(expected));
        if let Some(actual) = actual.as_ref() {
            self.record_type_judgment(
                TypeJudgmentSubject::Expected {
                    context: context.to_owned(),
                },
                TypeJudgmentRule::Expected,
                actual.clone(),
                Some(expected),
            );
        }
        if actual.as_ref() != Some(expected) {
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
        let ty = match expr {
            Expr::Literal(literal) => Some(self.check_literal_expr(literal, expected)),
            Expr::EntityRef(entity) => self.check_entity_ref_expr(entity),
            Expr::LifetimePath { key, optional } => self.check_lifetime_path_expr(key, *optional),
            Expr::Path(path) => self.check_path_expr(path),
            Expr::Placeholder(_) => None,
            Expr::Tuple(items) => Some(self.check_tuple_expr(items)),
            Expr::BracketSeq(items) => Some(self.check_bracket_seq_with_expected(items, expected)),
            Expr::ArrayRepeat { value, len } => {
                Some(self.check_array_repeat_expr(value, len, expected))
            }
            Expr::Call { callee, args } => self.check_call_expr(callee, args),
            Expr::NamedArg { value, .. } => self.check_expr(value),
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
        };
        if let Some(ty) = ty.as_ref() {
            self.record_type_judgment(
                TypeJudgmentSubject::Expr {
                    kind: expr_kind_name(expr).to_owned(),
                },
                expected.map_or(TypeJudgmentRule::Expr, |_| TypeJudgmentRule::Expected),
                ty.clone(),
                expected,
            );
        }
        ty
    }

    fn check_closure_expr(&mut self, params: &[String], body: &Expr) -> Option<TypeKind> {
        let previous = params
            .iter()
            .map(|param| {
                (
                    param.clone(),
                    self.locals.insert(param.clone(), TypeKind::I64),
                )
            })
            .collect::<Vec<_>>();
        self.check_expr(body);
        for (param, old) in previous {
            if let Some(old) = old {
                self.locals.insert(param, old);
            } else {
                self.locals.remove(&param);
            }
        }
        None
    }

    fn in_seq_context(&self) -> bool {
        self.yield_stack
            .last()
            .is_some_and(|context| matches!(context, YieldContext::Seq { .. }))
    }

    fn check_entity_ref_expr(&mut self, entity: &EntityRefSyntax) -> Option<TypeKind> {
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
            .map(TypeKind::Ref)
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
                    let Some(ty) = numeric_literal_suffix_type(Some(suffix)) else {
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
                } else {
                    self.errors.push(TypeCheckError::new(
                        "unsuffixed integer literal requires an expected integer type".to_owned(),
                    ));
                    TypeKind::Named("_".to_owned())
                }
            }
            arcweft_lang_syntax::expr::Literal::Float { suffix, .. } => {
                if let Some(suffix) = suffix {
                    let Some(ty) = numeric_literal_suffix_type(Some(suffix)) else {
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
                } else {
                    self.errors.push(TypeCheckError::new(
                        "unsuffixed float literal requires an expected float type".to_owned(),
                    ));
                    TypeKind::Named("_".to_owned())
                }
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
        let mut item_type = None;
        let expected_item = match expected {
            Some(TypeKind::Array { item, .. } | TypeKind::Vec(item)) => Some(item.as_ref()),
            _ => None,
        };
        for item in items {
            let next_type = self
                .check_expr_with_expected(item, expected_item)
                .unwrap_or(TypeKind::Unit);
            match &item_type {
                Some(existing) if existing != &next_type => {
                    self.errors.push(TypeCheckError::new(format!(
                        "sequence literal items must have the same type, found {existing:?} and {next_type:?}"
                    )));
                }
                Some(_) => {}
                None => item_type = Some(next_type),
            }
        }
        let item_type = item_type.unwrap_or(TypeKind::Unit);
        if let Some(TypeKind::Array { item, len }) = expected {
            if !array_len_matches(len, items.len()) {
                self.errors.push(TypeCheckError::new(format!(
                    "array literal length mismatch: expected {len}, found {}",
                    items.len()
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

    fn check_call_expr(&mut self, callee: &Expr, args: &[Expr]) -> Option<TypeKind> {
        if let Some(ty) = self.check_builtin_call_expr(callee, args) {
            return Some(ty);
        }
        if let Some(name) = expr_path_label(callee)
            && let Some(ty) = self
                .function_type(&name)
                .cloned()
                .or_else(|| well_known_runtime_method_type(&name))
        {
            self.check_virtual_path_call(&name, args);
            self.check_function_effects(&name);
            let checked_args = if name == "event.emit" {
                args.iter().skip(1).collect::<Vec<_>>()
            } else {
                args.iter().collect::<Vec<_>>()
            };
            for arg in checked_args {
                self.check_expr(arg);
            }
            return Some(ty);
        }
        if let Expr::Path(name) = callee {
            if let Some(ty) = self.check_presentation_call(name, args) {
                return Some(ty);
            }
            if matches!(name.as_str(), "promote" | "promote_unchecked") {
                for arg in args
                    .iter()
                    .filter(|arg| matches!(arg, Expr::NamedArg { .. }))
                {
                    self.check_expr(arg);
                }
                return Some(TypeKind::Named("Promoted".to_owned()));
            }
            if name == "assume" {
                return Some(TypeKind::Unit);
            }
            if self.symbol_type(name) == Some(&TypeKind::Ref(EntityKind::Character)) {
                for arg in args {
                    self.check_expr(arg);
                }
                return Some(TypeKind::SpeakerPreset(EntityKind::Character));
            }
            if self.symbol_type(name) == Some(&TypeKind::SpeakerPreset(EntityKind::Character)) {
                for arg in args {
                    self.check_expr(arg);
                }
                return Some(TypeKind::SpeakerPreset(EntityKind::Character));
            }
            let arg_types = args
                .iter()
                .map(|arg| self.check_expr(arg))
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
                    self.check_expr(arg);
                }
                Some(TypeKind::SpeakerPreset(entity))
            }
            other => {
                for arg in args {
                    self.check_expr(arg);
                }
                other
            }
        }
    }

    fn check_builtin_call_expr(&mut self, callee: &Expr, args: &[Expr]) -> Option<TypeKind> {
        let name = expr_path_label(callee)?;
        match name.as_str() {
            "panic" | "fail" | "bail" => {
                for arg in args {
                    self.check_expr(arg);
                }
                Some(TypeKind::Never)
            }
            "ensure" => {
                self.check_assert_like_args(args, "ensure");
                Some(TypeKind::Unit)
            }
            "assert" | "debug_assert" => {
                self.check_assert_like_args(args, name.as_str());
                Some(TypeKind::Unit)
            }
            _ => None,
        }
    }

    fn check_assert_like_args(&mut self, args: &[Expr], name: &str) {
        if let Some(condition) = args.first() {
            self.expect_expr_type(condition, &TypeKind::Bool, &format!("{name} condition"));
        } else {
            self.errors.push(TypeCheckError::new(format!(
                "{name} requires a condition argument"
            )));
        }
        for arg in args.iter().skip(1) {
            self.check_expr(arg);
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
        args: &[Expr],
    ) -> Option<TypeKind> {
        let method_name = method.split_once('<').map_or(method, |(name, _)| name);
        if let Expr::Path(receiver_path) = receiver {
            let dotted = format!("{receiver_path}.{method_name}");
            if let Some(ty) = self
                .function_type(&dotted)
                .cloned()
                .or_else(|| well_known_runtime_method_type(&dotted))
            {
                self.check_virtual_path_call(&dotted, args);
                self.check_function_effects(&dotted);
                let checked_args = if dotted == "event.emit" {
                    args.iter().skip(1).collect::<Vec<_>>()
                } else {
                    args.iter().collect::<Vec<_>>()
                };
                for arg in checked_args {
                    self.check_expr(arg);
                }
                return Some(ty);
            }
        }
        let receiver_type = self.check_expr(receiver);
        if is_drop_name(method_name) {
            for arg in args {
                self.check_expr(arg);
            }
            return Some(TypeKind::Unit);
        }
        receiver_type.and_then(|receiver_type| {
            if matches!(method_name, "context" | "with_context") {
                for arg in args {
                    self.check_expr(arg);
                }
                return match receiver_type {
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
                };
            }
            if method_name == "face"
                && matches!(
                    receiver_type,
                    TypeKind::Ref(EntityKind::Character)
                        | TypeKind::Speaker(EntityKind::Character)
                        | TypeKind::SpeakerPreset(EntityKind::Character)
                )
            {
                for arg in args {
                    self.check_expr(arg);
                }
                return Some(TypeKind::CharacterPatch(EntityKind::Character));
            }
            if method_name == "say"
                && matches!(
                    receiver_type,
                    TypeKind::Ref(EntityKind::Character)
                        | TypeKind::Speaker(EntityKind::Character)
                        | TypeKind::SpeakerPreset(EntityKind::Character)
                )
            {
                for arg in args {
                    self.check_expr(arg);
                }
                return Some(TypeKind::SpeakerPreset(EntityKind::Character));
            }
            for arg in args {
                self.check_expr(arg);
            }
            self.env
                .method_type(&receiver_type, method_name)
                .cloned()
                .or_else(|| {
                    well_known_capacity_method_type(&receiver_type, method_name, args.len())
                })
                .or_else(|| {
                    self.errors.push(TypeCheckError::new(format!(
                        "unknown method `{method_name}` on {receiver_type:?}"
                    )));
                    None
                })
        })
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

    fn check_virtual_path_call(&mut self, callee: &str, args: &[Expr]) {
        if !callee.starts_with("fs.") {
            return;
        }
        for arg in args {
            if let Expr::Literal(arcweft_lang_syntax::expr::Literal::String(path)) = arg
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
        }
        if let Some(field_type) = well_known_field_type(field) {
            self.check_expr(target);
            return Some(field_type);
        }
        match self.check_expr(target) {
            Some(TypeKind::Map { value, .. }) => Some(*value),
            Some(TypeKind::Named(name)) if name == "HttpRequestContext" => match field {
                "method" | "path" | "body" => Some(TypeKind::String),
                _ => None,
            },
            _ => None,
        }
    }

    fn check_try_expr(&mut self, expr: &Expr) -> Option<TypeKind> {
        match self.check_expr(expr) {
            Some(TypeKind::Result { ok, .. }) => Some(*ok),
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
        let outer_locals = self.locals.clone();
        for stmt in statements {
            self.check_stmt(stmt);
        }
        let ty = value.map_or(Some(TypeKind::Unit), |value| {
            self.check_expr_with_expected(value, expected)
        });
        self.reject_borrow_escape(ty.as_ref(), "block final value");
        self.locals = outer_locals;
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
        let base_borrow_snapshot = self.snapshot_borrow_state();
        let then_type = self.check_expr(then_branch);
        let then_borrow_state = self.snapshot_borrow_state();
        self.restore_borrow_state(base_borrow_snapshot.clone());
        let else_type = else_branch.and_then(|branch| self.check_expr(branch));
        let else_borrow_state = self.snapshot_borrow_state();
        if else_branch.is_some() {
            self.merge_borrow_state_from_paths(
                &base_borrow_snapshot,
                &[then_borrow_state, else_borrow_state],
            );
        } else {
            self.merge_borrow_state_from_paths(
                &base_borrow_snapshot,
                &[base_borrow_snapshot.clone(), then_borrow_state],
            );
        }
        match (then_type, else_type) {
            (Some(TypeKind::Never), Some(else_type)) => Some(else_type),
            (Some(then_type), Some(TypeKind::Never)) => Some(then_type),
            (Some(then_type), Some(else_type)) if then_type == else_type => Some(then_type),
            (Some(then_type), Some(else_type)) => {
                self.errors.push(TypeCheckError::new(format!(
                    "if expression branches must have the same type, found {then_type:?} and {else_type:?}"
                )));
                None
            }
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

        let base_borrow_snapshot = self.snapshot_borrow_state();
        let mut arm_states = Vec::new();
        let mut inferred = None;
        for arm in arms {
            self.restore_borrow_state(base_borrow_snapshot.clone());
            let outer_locals = self.locals.clone();
            for (name, ty) in let_else_bindings(arm.pattern(), scrutinee_type.as_ref()) {
                self.locals.insert(name, ty);
            }
            if let Some(guard) = arm.guard() {
                self.expect_expr_type(guard, &TypeKind::Bool, "match arm guard");
            }
            let arm_type = self.check_expr_with_expected(arm.value(), inferred.as_ref());
            self.locals = outer_locals;
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
                    self.errors.push(TypeCheckError::new(format!(
                        "match expression arms must have the same type, found {existing:?} and {ty:?}"
                    )));
                    return None;
                }
                (_, None) => return None,
            }
            arm_states.push(self.snapshot_borrow_state());
        }
        self.merge_borrow_state_from_paths(&base_borrow_snapshot, &arm_states);
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

        let base_borrow_snapshot = self.snapshot_borrow_state();
        let outer_locals = self.locals.clone();
        for (name, ty) in let_else_bindings(pattern, expr_type.as_ref()) {
            self.locals.insert(name, ty);
        }
        let then_type = self.check_expr(then_branch);
        let then_borrow_state = self.snapshot_borrow_state();
        self.restore_borrow_state(base_borrow_snapshot.clone());
        self.locals = outer_locals;

        let else_type = else_branch.and_then(|branch| self.check_expr(branch));
        let else_borrow_state = self.snapshot_borrow_state();
        if else_branch.is_some() {
            self.merge_borrow_state_from_paths(
                &base_borrow_snapshot,
                &[then_borrow_state, else_borrow_state],
            );
        } else {
            self.merge_borrow_state_from_paths(
                &base_borrow_snapshot,
                &[base_borrow_snapshot.clone(), then_borrow_state],
            );
        }
        match (then_type, else_type) {
            (Some(then_type), Some(else_type)) if then_type == else_type => Some(then_type),
            (Some(then_type), Some(else_type)) => {
                self.errors.push(TypeCheckError::new(format!(
                    "if-let expression branches must have the same type, found {then_type:?} and {else_type:?}"
                )));
                None
            }
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
        Expr::ArrayRepeat { .. } => "array_repeat",
        Expr::Call { .. } => "call",
        Expr::NamedArg { .. } => "named_arg",
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

fn map_key_type_from_name(name: &str) -> Option<TypeKind> {
    let (_, args) = name.split_once('<')?;
    let args = args.strip_suffix('>')?;
    let (key, _) = args.split_once(',')?;
    Some(match key.trim() {
        "Character" | "Ref<Character>" => TypeKind::Ref(EntityKind::Character),
        other => named_type_label(other),
    })
}

fn is_unit_number_type(ty: &TypeKind) -> bool {
    matches!(ty, TypeKind::Named(name) if matches!(
        name.as_str(),
        "Length" | "Angle" | "AudioLevel" | "Tempo"
    ))
}

fn looks_like_os_absolute_path(path: &str) -> bool {
    path.starts_with('/')
        || path.starts_with('\\')
        || path.as_bytes().get(1).is_some_and(|byte| *byte == b':')
}
