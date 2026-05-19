//! Expression type-checking entry points and expression-kind dispatch.

use super::{Expr, TypeCheckError, TypeChecker, TypeKind, YieldContext, literal_type};

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
}
