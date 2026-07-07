use super::{CallArg, Expr, Placeholder, TypeCheckError, TypeChecker, TypeKind};

impl TypeChecker<'_> {
    pub(super) fn check_placeholder_expr(&mut self, placeholder: Placeholder) -> Option<TypeKind> {
        match placeholder {
            Placeholder::PipeLeft => {
                self.errors.push(TypeCheckError::new(
                    "`^` can only appear inside the right-hand side of a pipe expression"
                        .to_owned(),
                ));
                None
            }
            Placeholder::Partial => self
                .current_partial_placeholder_type()
                .or_else(|| self.reject_partial_placeholder_without_expected_type()),
        }
    }

    pub(super) fn check_pipe_expr(&mut self, lhs: &Expr, rhs: &Expr) -> Option<TypeKind> {
        if self.check_lifetime_pipe(lhs, rhs).is_some() {
            return Some(TypeKind::Unit);
        }
        let lowered = if expr_contains_pipe_left(rhs) {
            substitute_pipe_left(rhs, lhs)
        } else {
            data_last_pipe_call(lhs, rhs)
        };
        self.check_expr(&lowered)
    }
}

fn data_last_pipe_call(lhs: &Expr, rhs: &Expr) -> Expr {
    if let Some((method, args)) = data_last_collection_method(rhs) {
        return Expr::MethodCall {
            receiver: Box::new(lhs.clone()),
            method: method.to_owned(),
            args: args.to_vec(),
        };
    }
    if let Expr::Call { callee, args } = rhs {
        return Expr::Call {
            callee: callee.clone(),
            args: args
                .iter()
                .cloned()
                .chain(std::iter::once(CallArg::Positional(lhs.clone())))
                .collect(),
        };
    }
    Expr::Call {
        callee: Box::new(rhs.clone()),
        args: vec![CallArg::Positional(lhs.clone())],
    }
}

fn data_last_collection_method(rhs: &Expr) -> Option<(&str, &[CallArg])> {
    let Expr::Call { callee, args } = rhs else {
        return None;
    };
    let Expr::Path(path) = callee.as_ref() else {
        return None;
    };
    let method = path.as_label();
    matches!(method, "map" | "filter").then_some((method, args.as_slice()))
}

fn substitute_pipe_left(expr: &Expr, lhs: &Expr) -> Expr {
    match expr {
        Expr::Placeholder(Placeholder::PipeLeft) => lhs.clone(),
        Expr::Tuple(items) => Expr::Tuple(
            items
                .iter()
                .map(|item| substitute_pipe_left(item, lhs))
                .collect(),
        ),
        Expr::BracketSeq(items) => Expr::BracketSeq(
            items
                .iter()
                .map(|item| substitute_pipe_left(item, lhs))
                .collect(),
        ),
        Expr::ArrayRepeat { value, len } => Expr::ArrayRepeat {
            value: Box::new(substitute_pipe_left(value, lhs)),
            len: Box::new(substitute_pipe_left(len, lhs)),
        },
        Expr::Call { callee, args } => Expr::Call {
            callee: Box::new(substitute_pipe_left(callee, lhs)),
            args: args
                .iter()
                .map(|arg| substitute_pipe_left_arg(arg, lhs))
                .collect(),
        },
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => Expr::MethodCall {
            receiver: Box::new(substitute_pipe_left(receiver, lhs)),
            method: method.clone(),
            args: args
                .iter()
                .map(|arg| substitute_pipe_left_arg(arg, lhs))
                .collect(),
        },
        Expr::Field { target, field } => Expr::Field {
            target: Box::new(substitute_pipe_left(target, lhs)),
            field: field.clone(),
        },
        Expr::Index { target, index } => Expr::Index {
            target: Box::new(substitute_pipe_left(target, lhs)),
            index: Box::new(substitute_pipe_left(index, lhs)),
        },
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(substitute_pipe_left(expr, lhs)),
        },
        Expr::Binary { lhs: left, op, rhs } => Expr::Binary {
            lhs: Box::new(substitute_pipe_left(left, lhs)),
            op: *op,
            rhs: Box::new(substitute_pipe_left(rhs, lhs)),
        },
        Expr::Record { path, fields } => Expr::Record {
            path: path.clone(),
            fields: fields
                .iter()
                .map(|(name, value)| (name.clone(), substitute_pipe_left(value, lhs)))
                .collect(),
        },
        Expr::RecordLiteral(fields) => Expr::RecordLiteral(
            fields
                .iter()
                .map(|(name, value)| (name.clone(), substitute_pipe_left(value, lhs)))
                .collect(),
        ),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => Expr::If {
            condition: Box::new(substitute_pipe_left(condition, lhs)),
            then_branch: Box::new(substitute_pipe_left(then_branch, lhs)),
            else_branch: else_branch
                .as_deref()
                .map(|else_branch| Box::new(substitute_pipe_left(else_branch, lhs))),
        },
        Expr::Try { expr } => Expr::Try {
            expr: Box::new(substitute_pipe_left(expr, lhs)),
        },
        Expr::Await { expr, applies_try } => Expr::Await {
            expr: Box::new(substitute_pipe_left(expr, lhs)),
            applies_try: *applies_try,
        },
        Expr::Closure {
            params,
            return_type,
            body,
        } => Expr::Closure {
            params: params.clone(),
            return_type: return_type.clone(),
            body: Box::new(substitute_pipe_left(body, lhs)),
        },
        _ => expr.clone(),
    }
}

fn substitute_pipe_left_arg(arg: &CallArg, lhs: &Expr) -> CallArg {
    match arg {
        CallArg::Positional(value) => CallArg::Positional(substitute_pipe_left(value, lhs)),
        CallArg::Named { name, value } => CallArg::Named {
            name: name.clone(),
            value: Box::new(substitute_pipe_left(value, lhs)),
        },
        CallArg::Spread { value } => CallArg::Spread {
            value: Box::new(substitute_pipe_left(value, lhs)),
        },
    }
}

fn expr_contains_pipe_left(expr: &Expr) -> bool {
    match expr {
        Expr::Placeholder(Placeholder::PipeLeft) => true,
        Expr::Tuple(items) | Expr::BracketSeq(items) => items.iter().any(expr_contains_pipe_left),
        Expr::ArrayRepeat { value, len }
        | Expr::Binary {
            lhs: value,
            rhs: len,
            ..
        } => expr_contains_pipe_left(value) || expr_contains_pipe_left(len),
        Expr::Call { callee, args } => {
            expr_contains_pipe_left(callee) || args.iter().any(call_arg_contains_pipe_left)
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_contains_pipe_left(receiver) || args.iter().any(call_arg_contains_pipe_left)
        }
        Expr::Field { target, .. } | Expr::Try { expr: target } => expr_contains_pipe_left(target),
        Expr::Index { target, index } => {
            expr_contains_pipe_left(target) || expr_contains_pipe_left(index)
        }
        Expr::Unary { expr, .. } | Expr::Await { expr, .. } | Expr::Closure { body: expr, .. } => {
            expr_contains_pipe_left(expr)
        }
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => fields
            .iter()
            .any(|(_, value)| expr_contains_pipe_left(value)),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_contains_pipe_left(condition)
                || expr_contains_pipe_left(then_branch)
                || else_branch.as_deref().is_some_and(expr_contains_pipe_left)
        }
        _ => false,
    }
}

fn call_arg_contains_pipe_left(arg: &CallArg) -> bool {
    match arg {
        CallArg::Positional(value) => expr_contains_pipe_left(value),
        CallArg::Named { value, .. } | CallArg::Spread { value } => expr_contains_pipe_left(value),
    }
}
