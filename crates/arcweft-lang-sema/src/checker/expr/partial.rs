use super::{CallArg, Expr, Placeholder, TypeCheckError, TypeChecker, TypeKind};
use crate::checker::helpers::{optional_type_kind_label, type_kind_label};

impl TypeChecker<'_> {
    pub(super) fn check_partial_placeholder_abstraction_expr(
        &mut self,
        expr: &Expr,
        expected: &TypeKind,
    ) -> Option<TypeKind> {
        let TypeKind::Function {
            params,
            return_type,
        } = expected
        else {
            self.errors.push(TypeCheckError::new(
                "`_` placeholder requires an expected function type".to_owned(),
            ));
            return None;
        };
        let [param] = params.as_slice() else {
            self.errors.push(TypeCheckError::new(format!(
                "`_` placeholder abstraction currently requires exactly one function parameter, found {}",
                params.len()
            )));
            return None;
        };

        self.partial_placeholder_stack.push(param.clone());
        let body_type = self.check_expr_with_expected(expr, Some(return_type));
        self.partial_placeholder_stack.pop();

        if let Some(body_type) = body_type.as_ref()
            && !is_unknown_type(return_type)
            && !self.types_compatible(return_type, body_type)
        {
            self.errors.push(TypeCheckError::new(format!(
                "`_` placeholder abstraction must return {}, found {}",
                type_kind_label(return_type),
                type_kind_label(body_type)
            )));
        }

        let inferred_return = if is_unknown_type(return_type) {
            body_type.unwrap_or_else(|| return_type.as_ref().clone())
        } else {
            return_type.as_ref().clone()
        };
        Some(TypeKind::Function {
            params: params.clone(),
            return_type: Box::new(inferred_return),
        })
    }

    pub(super) fn current_partial_placeholder_type(&self) -> Option<TypeKind> {
        self.partial_placeholder_stack.last().cloned()
    }

    pub(super) fn reject_partial_placeholder_without_expected_type(&mut self) -> Option<TypeKind> {
        self.errors.push(TypeCheckError::new(format!(
            "`_` placeholder requires an expected function type, found {}",
            optional_type_kind_label(None)
        )));
        None
    }
}

pub(super) fn expr_contains_partial_placeholder(expr: &Expr) -> bool {
    match expr {
        Expr::Placeholder(Placeholder::Partial) => true,
        Expr::Placeholder(Placeholder::PipeLeft)
        | Expr::Closure { .. }
        | Expr::NumericBracketSeq(_)
        | Expr::Literal(_)
        | Expr::EntityRef(_)
        | Expr::Path(_)
        | Expr::LifetimePath { .. }
        | Expr::ShortVariant(_)
        | Expr::Raw(_) => false,
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            items.iter().any(expr_contains_partial_placeholder)
        }
        Expr::ArrayRepeat { value, len } => {
            expr_contains_partial_placeholder(value) || expr_contains_partial_placeholder(len)
        }
        Expr::Range { start, end, .. } => {
            start
                .as_deref()
                .is_some_and(expr_contains_partial_placeholder)
                || end
                    .as_deref()
                    .is_some_and(expr_contains_partial_placeholder)
        }
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => fields
            .iter()
            .any(|(_, value)| expr_contains_partial_placeholder(value)),
        Expr::Field { target, .. }
        | Expr::Try { expr: target }
        | Expr::Await { expr: target, .. }
        | Expr::Unary { expr: target, .. } => expr_contains_partial_placeholder(target),
        Expr::Index { target, index } => {
            expr_contains_partial_placeholder(target) || expr_contains_partial_placeholder(index)
        }
        Expr::Call { callee, args } => {
            expr_contains_partial_placeholder(callee) || args.iter().any(call_arg_contains_partial)
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_contains_partial_placeholder(receiver)
                || args.iter().any(call_arg_contains_partial)
        }
        Expr::DialogueCall { callee, .. } => expr_contains_partial_placeholder(callee),
        Expr::Pipe { lhs, rhs } | Expr::Binary { lhs, rhs, .. } => {
            expr_contains_partial_placeholder(lhs) || expr_contains_partial_placeholder(rhs)
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_contains_partial_placeholder(condition)
                || expr_contains_partial_placeholder(then_branch)
                || else_branch
                    .as_deref()
                    .is_some_and(expr_contains_partial_placeholder)
        }
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => {
            expr_contains_partial_placeholder(expr)
                || guard
                    .as_deref()
                    .is_some_and(expr_contains_partial_placeholder)
                || expr_contains_partial_placeholder(then_branch)
                || else_branch
                    .as_deref()
                    .is_some_and(expr_contains_partial_placeholder)
        }
        Expr::Match { scrutinee, arms } => {
            expr_contains_partial_placeholder(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard().is_some_and(expr_contains_partial_placeholder)
                        || expr_contains_partial_placeholder(arm.value())
                })
        }
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        }
        | Expr::MemoBlock {
            statements, value, ..
        } => {
            statements.iter().any(stmt_contains_partial_placeholder)
                || value
                    .as_deref()
                    .is_some_and(expr_contains_partial_placeholder)
        }
        Expr::Thread { block } => block
            .body()
            .iter()
            .any(flow_item_contains_partial_placeholder),
    }
}

fn call_arg_contains_partial(arg: &CallArg) -> bool {
    expr_contains_partial_placeholder(arg.value())
}

fn flow_item_contains_partial_placeholder(item: &arcweft_lang_syntax::ast::flow::FlowItem) -> bool {
    match item {
        arcweft_lang_syntax::ast::flow::FlowItem::Stmt(stmt) => {
            stmt_contains_partial_placeholder(stmt)
        }
        arcweft_lang_syntax::ast::flow::FlowItem::Scope(scope) => scope
            .body()
            .iter()
            .any(flow_item_contains_partial_placeholder),
        _ => false,
    }
}

fn stmt_contains_partial_placeholder(stmt: &arcweft_lang_syntax::ast::flow::Stmt) -> bool {
    use arcweft_lang_syntax::ast::flow::Stmt;

    match stmt {
        Stmt::Let { expr, .. }
        | Stmt::Assign { expr, .. }
        | Stmt::LetElse { expr, .. }
        | Stmt::LetActionReceive { action: expr, .. }
        | Stmt::Expr(expr)
        | Stmt::Return(expr)
        | Stmt::Out { expr, .. }
        | Stmt::Goto(expr)
        | Stmt::Defer { expr, .. }
        | Stmt::Yield(expr)
        | Stmt::Match { expr, .. } => expr_contains_partial_placeholder(expr),
        Stmt::Signal { target, value } => {
            expr_contains_partial_placeholder(target) || expr_contains_partial_placeholder(value)
        }
        Stmt::LifetimeSet { target, expr } => {
            expr_contains_partial_placeholder(target) || expr_contains_partial_placeholder(expr)
        }
        Stmt::UnsafeLifetime { body, .. }
        | Stmt::DeferBlock {
            statements: body, ..
        } => body.iter().any(stmt_contains_partial_placeholder),
        Stmt::If {
            condition,
            body,
            else_body,
        } => {
            expr_contains_partial_placeholder(condition)
                || body.iter().any(stmt_contains_partial_placeholder)
                || else_body.iter().any(stmt_contains_partial_placeholder)
        }
        Stmt::While { condition, body } => {
            expr_contains_partial_placeholder(condition)
                || body.iter().any(stmt_contains_partial_placeholder)
        }
        Stmt::For { source, body, .. } => {
            expr_contains_partial_placeholder(source)
                || body.iter().any(stmt_contains_partial_placeholder)
        }
        _ => false,
    }
}

fn is_unknown_type(ty: &TypeKind) -> bool {
    matches!(ty, TypeKind::Named(name) if name == "_")
}
