use super::{BinaryOp, CallArg, Expr, Literal, Placeholder, TypeCheckError, TypeChecker, TypeKind};
use crate::checker::helpers::{
    numeric_literal_suffix_type, optional_type_kind_label, type_kind_label,
};
use crate::env::FunctionSignature;

impl TypeChecker<'_> {
    pub(super) fn check_inferred_partial_placeholder_abstraction_expr(
        &mut self,
        expr: &Expr,
    ) -> Option<TypeKind> {
        if let Some(expected) = self.infer_partial_placeholder_function_type(expr) {
            self.check_partial_placeholder_abstraction_expr(expr, &expected)
        } else {
            None
        }
    }

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
        let body_type =
            self.check_expr_with_expected(partial_placeholder_body_expr(expr), Some(return_type));
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

    fn infer_partial_placeholder_function_type(&self, expr: &Expr) -> Option<TypeKind> {
        match partial_placeholder_body_expr(expr) {
            Expr::Binary { lhs, op, rhs } => {
                self.infer_partial_placeholder_binary_function_type(lhs, *op, rhs)
            }
            Expr::Call { callee, args } => {
                self.infer_partial_placeholder_call_function_type(callee, args)
            }
            _ => None,
        }
    }

    fn infer_partial_placeholder_binary_function_type(
        &self,
        lhs: &Expr,
        op: BinaryOp,
        rhs: &Expr,
    ) -> Option<TypeKind> {
        let lhs_has_placeholder = expr_contains_partial_placeholder(lhs);
        let rhs_has_placeholder = expr_contains_partial_placeholder(rhs);
        let operand_ty = match (lhs_has_placeholder, rhs_has_placeholder) {
            (true, false) => self.partial_inference_static_expr_type(rhs)?,
            (false, true) => self.partial_inference_static_expr_type(lhs)?,
            _ => return None,
        };
        let return_ty = inferred_binary_return_type(op, &operand_ty)?;
        Some(TypeKind::Function {
            params: vec![operand_ty],
            return_type: Box::new(return_ty),
        })
    }

    fn infer_partial_placeholder_call_function_type(
        &self,
        callee: &Expr,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        if let Expr::Path(name) = callee
            && let Some(TypeKind::Function {
                params,
                return_type,
            }) = self.symbol_type(name)
        {
            return partial_call_function_type_from_positional_args(
                args,
                params.iter(),
                return_type.as_ref(),
            );
        }
        let Expr::Path(name) = callee else {
            return None;
        };
        let signature = self.function_signature(name)?;
        if !signature.checks_args() {
            return None;
        }
        partial_call_function_type_from_signature_args(args, signature)
    }

    fn partial_inference_static_expr_type(&self, expr: &Expr) -> Option<TypeKind> {
        match expr {
            Expr::Literal(literal) => literal_type_for_partial_inference(literal),
            Expr::Path(path) => self.symbol_type(path.as_label()).cloned(),
            Expr::Tuple(items) if items.is_empty() => Some(TypeKind::Unit),
            _ => None,
        }
    }
}

fn partial_placeholder_body_expr(expr: &Expr) -> &Expr {
    match expr {
        Expr::Tuple(items) if items.len() == 1 => &items[0],
        _ => expr,
    }
}

fn partial_call_function_type_from_positional_args<'a>(
    args: &[CallArg],
    params: impl IntoIterator<Item = &'a TypeKind>,
    return_type: &TypeKind,
) -> Option<TypeKind> {
    let params = params.into_iter().collect::<Vec<_>>();
    let mut inferred_param = None;
    for (index, arg) in args.iter().enumerate() {
        if !expr_contains_partial_placeholder(arg.value()) {
            continue;
        }
        let CallArg::Positional(_) = arg else {
            return None;
        };
        let param = params.get(index)?;
        match &inferred_param {
            Some(existing) if existing != *param => return None,
            Some(_) => {}
            None => inferred_param = Some((*param).clone()),
        }
    }
    inferred_param.map(|param| TypeKind::Function {
        params: vec![param],
        return_type: Box::new(return_type.clone()),
    })
}

fn partial_call_function_type_from_signature_args(
    args: &[CallArg],
    signature: &FunctionSignature,
) -> Option<TypeKind> {
    let params = signature.params();
    let mut inferred_param = None;
    let mut provided = vec![false; params.len()];
    let mut positional_index = 0usize;

    for arg in args {
        if matches!(arg, CallArg::Spread { .. }) {
            return None;
        }
        match arg {
            CallArg::Positional(value) => {
                while positional_index < params.len() && provided[positional_index] {
                    positional_index += 1;
                }
                let param = params.get(positional_index)?;
                provided[positional_index] = true;
                positional_index += 1;
                if expr_contains_partial_placeholder(value) {
                    infer_partial_param_type(&mut inferred_param, param.ty())?;
                }
            }
            CallArg::Named { name, value } => {
                let index = params
                    .iter()
                    .position(|param| !param.is_rest() && param.name() == Some(name.as_str()))?;
                if provided[index] {
                    return None;
                }
                provided[index] = true;
                if expr_contains_partial_placeholder(value) {
                    infer_partial_param_type(&mut inferred_param, params[index].ty())?;
                }
            }
            CallArg::Spread { .. } => unreachable!("spread returned before arg dispatch"),
        }
    }

    inferred_param.map(|param| TypeKind::Function {
        params: vec![param],
        return_type: Box::new(signature.return_type().clone()),
    })
}

fn infer_partial_param_type(inferred_param: &mut Option<TypeKind>, param: &TypeKind) -> Option<()> {
    match inferred_param {
        Some(existing) if existing != param => None,
        Some(_) => Some(()),
        None => {
            *inferred_param = Some(param.clone());
            Some(())
        }
    }
}

fn literal_type_for_partial_inference(literal: &Literal) -> Option<TypeKind> {
    match literal {
        Literal::String(_) => Some(TypeKind::String),
        Literal::Char { .. } => Some(TypeKind::Char),
        Literal::Bool(_) => Some(TypeKind::Bool),
        Literal::Duration { .. } => Some(TypeKind::Duration),
        Literal::Int { suffix, .. } => suffix.as_deref().map_or(Some(TypeKind::I32), |suffix| {
            numeric_literal_suffix_type(Some(suffix))
        }),
        Literal::Float { suffix, .. } => suffix.map_or(Some(TypeKind::F64), |suffix| {
            numeric_literal_suffix_type(Some(suffix.as_str()))
        }),
        Literal::UnitNumber { suffix, .. } => numeric_literal_suffix_type(Some(suffix.as_str())),
    }
}

fn inferred_binary_return_type(op: BinaryOp, operand_ty: &TypeKind) -> Option<TypeKind> {
    match op {
        BinaryOp::Implies | BinaryOp::Or | BinaryOp::And if operand_ty == &TypeKind::Bool => {
            Some(TypeKind::Bool)
        }
        BinaryOp::Eq | BinaryOp::NotEq => Some(TypeKind::Bool),
        BinaryOp::Gte | BinaryOp::Lte | BinaryOp::Gt | BinaryOp::Lt
            if operand_ty.is_integer()
                || operand_ty.is_float()
                || operand_ty == &TypeKind::Duration =>
        {
            Some(TypeKind::Bool)
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            if operand_ty.is_integer()
                || operand_ty.is_float()
                || operand_ty == &TypeKind::Duration =>
        {
            Some(operand_ty.clone())
        }
        BinaryOp::Merge
        | BinaryOp::In
        | BinaryOp::Implies
        | BinaryOp::Or
        | BinaryOp::And
        | BinaryOp::Gte
        | BinaryOp::Lte
        | BinaryOp::Gt
        | BinaryOp::Lt
        | BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Rem => None,
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
        Expr::Select(select) => expr_contains_partial_placeholder(select.target()),
        Expr::Try { expr: target }
        | Expr::Await { expr: target, .. }
        | Expr::Unary { expr: target, .. } => expr_contains_partial_placeholder(target),
        Expr::Index { target, index } => {
            expr_contains_partial_placeholder(target) || expr_contains_partial_placeholder(index)
        }
        Expr::Call { callee, args } => {
            expr_contains_partial_placeholder(callee) || args.iter().any(call_arg_contains_partial)
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
        Stmt::Let { expr, .. } | Stmt::Expr { expr, .. } | Stmt::Return { expr, .. } => {
            expr_contains_partial_placeholder(expr)
        }
        Stmt::LetElse { expr, .. }
        | Stmt::Out { expr, .. }
        | Stmt::Break {
            expr: Some(expr), ..
        }
        | Stmt::Defer { expr, .. } => expr_contains_partial_placeholder(expr.expr()),
        Stmt::LetActionReceive { action, .. } => expr_contains_partial_placeholder(action.expr()),
        Stmt::Assign { target, expr }
        | Stmt::Signal {
            target,
            value: expr,
        }
        | Stmt::LifetimeSet { target, expr } => {
            expr_contains_partial_placeholder(target.expr())
                || expr_contains_partial_placeholder(expr.expr())
        }
        Stmt::Match { expr, .. } | Stmt::Goto(expr) | Stmt::Yield(expr) => {
            expr_contains_partial_placeholder(expr.expr())
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
            expr_contains_partial_placeholder(condition.expr())
                || body.iter().any(stmt_contains_partial_placeholder)
                || else_body.iter().any(stmt_contains_partial_placeholder)
        }
        Stmt::While { condition, body } => {
            expr_contains_partial_placeholder(condition.expr())
                || body.iter().any(stmt_contains_partial_placeholder)
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            expr_contains_partial_placeholder(expr.expr())
                || guard
                    .as_ref()
                    .is_some_and(|guard| expr_contains_partial_placeholder(guard.expr()))
                || body.iter().any(stmt_contains_partial_placeholder)
        }
        Stmt::For { source, body, .. } => {
            expr_contains_partial_placeholder(source.expr())
                || body.iter().any(stmt_contains_partial_placeholder)
        }
        _ => false,
    }
}

fn is_unknown_type(ty: &TypeKind) -> bool {
    matches!(ty, TypeKind::Named(name) if name == "_")
}
