//! Expression type-checking entry points and expression-kind dispatch.

use super::{
    BorrowLocalState, EntityRefSyntax, Expr, LifetimeScopeKind, Stmt, TypeCheckError, TypeChecker,
    TypeKind, YieldContext, array_len_matches, array_repeat_len_label, entity_kind, literal_type,
};

impl TypeChecker<'_> {
    pub(super) fn expect_expr_type(&mut self, expr: &Expr, expected: &TypeKind, context: &str) {
        let actual = self.check_expr(expr);
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
        match expr {
            Expr::Literal(literal) => Some(literal_type(literal)),
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
            Expr::Closure { body, .. } => {
                self.check_expr(body);
                None
            }
            Expr::Unary { op, expr } => Some(self.check_unary_expr(*op, expr)),
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

    fn in_seq_context(&self) -> bool {
        self.yield_stack
            .last()
            .is_some_and(|context| matches!(context, YieldContext::Seq { .. }))
    }

    fn check_entity_ref_expr(&mut self, entity: &EntityRefSyntax) -> Option<TypeKind> {
        entity
            .as_absolute()
            .and_then(entity_kind)
            .map(TypeKind::Ref)
            .or_else(|| {
                self.errors.push(TypeCheckError::new(format!(
                    "unknown entity reference kind: {}",
                    entity.body()
                )));
                None
            })
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
        self.locals.get(path).cloned().or_else(|| {
            self.env.symbol_type(path).cloned().or_else(|| {
                self.check_dotted_path_target(path).or_else(|| {
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

    fn check_dialogue_call_expr(
        &mut self,
        callee: &Expr,
        plan: Option<&arcweft_lang_syntax::LinePlan>,
    ) -> TypeKind {
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
        self.expect_expr_type(len, &TypeKind::Int, "array repeat length");

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
        if let Some(start) = start {
            self.check_expr(start);
        }
        if let Some(end) = end {
            self.check_expr(end);
        }
        TypeKind::Range
    }
}
