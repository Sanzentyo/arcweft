use arcweft_lang_hir::syntax::expr::{CallArg, Expr, MatchExprArm, Placeholder};

pub(crate) fn substitute_pipe_left(expr: &Expr, lhs: &Expr) -> Expr {
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
        Expr::Select(select) => Expr::select(
            substitute_pipe_left(select.target(), lhs),
            select.member().as_str().to_owned(),
        ),
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
        Expr::IfLet { .. } => substitute_pipe_left_if_let(expr, lhs),
        Expr::Match { .. } => substitute_pipe_left_match(expr, lhs),
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

fn substitute_pipe_left_if_let(expr: &Expr, lhs: &Expr) -> Expr {
    let Expr::IfLet {
        pattern,
        expr,
        guard,
        then_branch,
        else_branch,
    } = expr
    else {
        unreachable!("pipe-left if-let substitution expects an if-let expression")
    };
    Expr::IfLet {
        pattern: pattern.clone(),
        expr: Box::new(substitute_pipe_left(expr, lhs)),
        guard: guard
            .as_deref()
            .map(|guard| Box::new(substitute_pipe_left(guard, lhs))),
        then_branch: Box::new(substitute_pipe_left(then_branch, lhs)),
        else_branch: else_branch
            .as_deref()
            .map(|else_branch| Box::new(substitute_pipe_left(else_branch, lhs))),
    }
}

fn substitute_pipe_left_match(expr: &Expr, lhs: &Expr) -> Expr {
    let Expr::Match { scrutinee, arms } = expr else {
        unreachable!("pipe-left match substitution expects a match expression")
    };
    Expr::Match {
        scrutinee: Box::new(substitute_pipe_left(scrutinee, lhs)),
        arms: arms
            .iter()
            .map(|arm| {
                MatchExprArm::new(
                    arm.pattern().clone(),
                    arm.guard()
                        .map(|guard| Box::new(substitute_pipe_left(guard, lhs))),
                    Box::new(substitute_pipe_left(arm.value(), lhs)),
                )
            })
            .collect(),
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

pub(super) fn substitute_partial_placeholder(expr: &Expr, param_name: &str) -> Expr {
    match expr {
        Expr::Placeholder(Placeholder::Partial) => Expr::Path(param_name.into()),
        Expr::Tuple(items) => Expr::Tuple(
            items
                .iter()
                .map(|item| substitute_partial_placeholder(item, param_name))
                .collect(),
        ),
        Expr::BracketSeq(items) => Expr::BracketSeq(
            items
                .iter()
                .map(|item| substitute_partial_placeholder(item, param_name))
                .collect(),
        ),
        Expr::ArrayRepeat { value, len } => Expr::ArrayRepeat {
            value: Box::new(substitute_partial_placeholder(value, param_name)),
            len: Box::new(substitute_partial_placeholder(len, param_name)),
        },
        Expr::Call { callee, args } => Expr::Call {
            callee: Box::new(substitute_partial_placeholder(callee, param_name)),
            args: args
                .iter()
                .map(|arg| substitute_partial_placeholder_arg(arg, param_name))
                .collect(),
        },
        Expr::Select(select) => Expr::select(
            substitute_partial_placeholder(select.target(), param_name),
            select.member().as_str().to_owned(),
        ),
        Expr::Index { target, index } => Expr::Index {
            target: Box::new(substitute_partial_placeholder(target, param_name)),
            index: Box::new(substitute_partial_placeholder(index, param_name)),
        },
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(substitute_partial_placeholder(expr, param_name)),
        },
        Expr::Binary { lhs, op, rhs } => Expr::Binary {
            lhs: Box::new(substitute_partial_placeholder(lhs, param_name)),
            op: *op,
            rhs: Box::new(substitute_partial_placeholder(rhs, param_name)),
        },
        Expr::Record { path, fields } => Expr::Record {
            path: path.clone(),
            fields: fields
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        substitute_partial_placeholder(value, param_name),
                    )
                })
                .collect(),
        },
        Expr::RecordLiteral(fields) => Expr::RecordLiteral(
            fields
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        substitute_partial_placeholder(value, param_name),
                    )
                })
                .collect(),
        ),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => Expr::If {
            condition: Box::new(substitute_partial_placeholder(condition, param_name)),
            then_branch: Box::new(substitute_partial_placeholder(then_branch, param_name)),
            else_branch: else_branch.as_deref().map(|else_branch| {
                Box::new(substitute_partial_placeholder(else_branch, param_name))
            }),
        },
        Expr::IfLet { .. } => substitute_partial_placeholder_if_let(expr, param_name),
        Expr::Match { .. } => substitute_partial_placeholder_match(expr, param_name),
        Expr::Try { expr } => Expr::Try {
            expr: Box::new(substitute_partial_placeholder(expr, param_name)),
        },
        Expr::Await { expr, applies_try } => Expr::Await {
            expr: Box::new(substitute_partial_placeholder(expr, param_name)),
            applies_try: *applies_try,
        },
        _ => expr.clone(),
    }
}

fn substitute_partial_placeholder_if_let(expr: &Expr, param_name: &str) -> Expr {
    let Expr::IfLet {
        pattern,
        expr,
        guard,
        then_branch,
        else_branch,
    } = expr
    else {
        unreachable!("partial-placeholder if-let substitution expects an if-let expression")
    };
    Expr::IfLet {
        pattern: pattern.clone(),
        expr: Box::new(substitute_partial_placeholder(expr, param_name)),
        guard: guard
            .as_deref()
            .map(|guard| Box::new(substitute_partial_placeholder(guard, param_name))),
        then_branch: Box::new(substitute_partial_placeholder(then_branch, param_name)),
        else_branch: else_branch
            .as_deref()
            .map(|else_branch| Box::new(substitute_partial_placeholder(else_branch, param_name))),
    }
}

fn substitute_partial_placeholder_match(expr: &Expr, param_name: &str) -> Expr {
    let Expr::Match { scrutinee, arms } = expr else {
        unreachable!("partial-placeholder match substitution expects a match expression")
    };
    Expr::Match {
        scrutinee: Box::new(substitute_partial_placeholder(scrutinee, param_name)),
        arms: arms
            .iter()
            .map(|arm| {
                MatchExprArm::new(
                    arm.pattern().clone(),
                    arm.guard()
                        .map(|guard| Box::new(substitute_partial_placeholder(guard, param_name))),
                    Box::new(substitute_partial_placeholder(arm.value(), param_name)),
                )
            })
            .collect(),
    }
}

fn substitute_partial_placeholder_arg(arg: &CallArg, param_name: &str) -> CallArg {
    match arg {
        CallArg::Positional(value) => {
            CallArg::Positional(substitute_partial_placeholder(value, param_name))
        }
        CallArg::Named { name, value } => CallArg::Named {
            name: name.clone(),
            value: Box::new(substitute_partial_placeholder(value, param_name)),
        },
        CallArg::Spread { value } => CallArg::Spread {
            value: Box::new(substitute_partial_placeholder(value, param_name)),
        },
    }
}

pub(super) fn expr_contains_partial_placeholder(expr: &Expr) -> bool {
    match expr {
        Expr::Placeholder(Placeholder::Partial) => true,
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            items.iter().any(expr_contains_partial_placeholder)
        }
        Expr::ArrayRepeat { value, len }
        | Expr::Binary {
            lhs: value,
            rhs: len,
            ..
        } => expr_contains_partial_placeholder(value) || expr_contains_partial_placeholder(len),
        Expr::Call { callee, args } => {
            expr_contains_partial_placeholder(callee)
                || args.iter().any(call_arg_contains_partial_placeholder)
        }
        Expr::Select(select) => expr_contains_partial_placeholder(select.target()),
        Expr::Try { expr: target } => expr_contains_partial_placeholder(target),
        Expr::Index { target, index } => {
            expr_contains_partial_placeholder(target) || expr_contains_partial_placeholder(index)
        }
        Expr::Unary { expr, .. } | Expr::Await { expr, .. } => {
            expr_contains_partial_placeholder(expr)
        }
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => fields
            .iter()
            .any(|(_, value)| expr_contains_partial_placeholder(value)),
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
        _ => false,
    }
}

fn call_arg_contains_partial_placeholder(arg: &CallArg) -> bool {
    match arg {
        CallArg::Positional(value) => expr_contains_partial_placeholder(value),
        CallArg::Named { value, .. } | CallArg::Spread { value } => {
            expr_contains_partial_placeholder(value)
        }
    }
}
