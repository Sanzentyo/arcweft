use arcweft_core::plan::FlowOp;
use arcweft_core::value::{RuntimeExpr, RuntimeExprMatchArm, RuntimeValue};
use std::sync::Arc;

pub(super) fn rewrite_known_record_projections_in_op(
    op: &mut FlowOp,
    env: &[(String, Vec<String>)],
) {
    match op {
        FlowOp::LetElse { expr, else_ops, .. } => {
            rewrite_known_record_projections_in_expr(expr, env);
            rewrite_known_record_projections_in_ops(else_ops, env);
        }
        FlowOp::If {
            condition,
            then_ops,
            else_ops,
        } => {
            rewrite_known_record_projections_in_expr(condition, env);
            rewrite_known_record_projections_in_ops(then_ops, env);
            rewrite_known_record_projections_in_ops(else_ops, env);
        }
        FlowOp::IfLet {
            expr,
            guard,
            then_ops,
            else_ops,
            ..
        } => {
            rewrite_known_record_projections_in_expr(expr, env);
            rewrite_known_record_projections_in_optional_expr(guard.as_mut(), env);
            rewrite_known_record_projections_in_ops(then_ops, env);
            rewrite_known_record_projections_in_ops(else_ops, env);
        }
        FlowOp::Match { scrutinee, arms } => {
            rewrite_known_record_projections_in_expr(scrutinee, env);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    rewrite_known_record_projections_in_expr(guard, env);
                }
                rewrite_known_record_projections_in_ops(&mut arm.ops, env);
            }
        }
        FlowOp::Loop { body }
        | FlowOp::LetLoop { body, .. }
        | FlowOp::Thread { body, .. }
        | FlowOp::Scope(body) => rewrite_known_record_projections_in_ops(body, env),
        FlowOp::LoopNext { body } | FlowOp::ForNext { body, .. } => {
            rewrite_known_record_projections_in_ops(Arc::make_mut(body), env);
        }
        FlowOp::While { condition, body } => {
            rewrite_known_record_projections_in_expr(condition, env);
            rewrite_known_record_projections_in_ops(body, env);
        }
        FlowOp::WhileNext { condition, body } => {
            rewrite_known_record_projections_in_expr(condition, env);
            rewrite_known_record_projections_in_ops(Arc::make_mut(body), env);
        }
        FlowOp::WhileLet {
            expr, guard, body, ..
        } => {
            rewrite_known_record_projections_in_expr(expr, env);
            rewrite_known_record_projections_in_optional_expr(guard.as_mut(), env);
            rewrite_known_record_projections_in_ops(body, env);
        }
        FlowOp::WhileLetNext {
            expr, guard, body, ..
        } => {
            rewrite_known_record_projections_in_expr(expr, env);
            rewrite_known_record_projections_in_optional_expr(guard.as_mut(), env);
            rewrite_known_record_projections_in_ops(Arc::make_mut(body), env);
        }
        FlowOp::For { source, body, .. } => {
            rewrite_known_record_projections_in_expr(source, env);
            rewrite_known_record_projections_in_ops(body, env);
        }
        FlowOp::AwaitMany { target, .. } => {
            rewrite_known_record_projections_in_expr(&mut target.source, env);
        }
        FlowOp::HostCall { target, .. } => {
            rewrite_known_record_projections_in_exprs(&mut target.args, env);
        }
        FlowOp::LetScope { ops, value, .. } => {
            rewrite_known_record_projections_in_ops(ops, env);
            rewrite_known_record_projections_in_expr(value, env);
        }
        FlowOp::Let { expr, .. }
        | FlowOp::Break(Some(expr))
        | FlowOp::GotoExpr(expr)
        | FlowOp::ReturnExpr(expr)
        | FlowOp::ExitScopeBind { expr, .. } => rewrite_known_record_projections_in_expr(expr, env),
        FlowOp::Bind(_)
        | FlowOp::Dialogue { .. }
        | FlowOp::Choice { .. }
        | FlowOp::Await { .. }
        | FlowOp::Effect(_)
        | FlowOp::RegisterCleanup { .. }
        | FlowOp::CancelCleanup { .. }
        | FlowOp::EnterScope
        | FlowOp::ExitScope
        | FlowOp::Break(None)
        | FlowOp::Continue
        | FlowOp::Goto(_)
        | FlowOp::Return(_)
        | FlowOp::Noop => {}
    }
}

fn rewrite_known_record_projections_in_ops(ops: &mut [FlowOp], env: &[(String, Vec<String>)]) {
    for op in ops {
        rewrite_known_record_projections_in_op(op, env);
    }
}

fn rewrite_known_record_projections_in_exprs(
    exprs: &mut [RuntimeExpr],
    env: &[(String, Vec<String>)],
) {
    for expr in exprs {
        rewrite_known_record_projections_in_expr(expr, env);
    }
}

fn rewrite_known_record_projections_in_optional_expr(
    expr: Option<&mut RuntimeExpr>,
    env: &[(String, Vec<String>)],
) {
    if let Some(expr) = expr {
        rewrite_known_record_projections_in_expr(expr, env);
    }
}

fn rewrite_known_record_projections_in_expr(expr: &mut RuntimeExpr, env: &[(String, Vec<String>)]) {
    match expr {
        RuntimeExpr::Field { target, field } => {
            if let Some(rewritten) = rewrite_known_record_projection_field(target, field, env) {
                *expr = rewritten;
            }
        }
        RuntimeExpr::Let { expr, body, .. } => {
            rewrite_known_record_projections_in_expr(expr, env);
            rewrite_known_record_projections_in_expr(body, env);
        }
        RuntimeExpr::AssignField {
            target, expr, body, ..
        } => {
            rewrite_known_record_projections_in_expr(target, env);
            rewrite_known_record_projections_in_expr(expr, env);
            rewrite_known_record_projections_in_expr(body, env);
        }
        RuntimeExpr::Tuple(items) | RuntimeExpr::BracketSeq(items) => {
            for item in items {
                rewrite_known_record_projections_in_expr(item, env);
            }
        }
        RuntimeExpr::RepeatSeq { value, .. }
        | RuntimeExpr::ProjectTuple { target: value, .. }
        | RuntimeExpr::ProjectRecord { target: value, .. }
        | RuntimeExpr::SpreadArg(value)
        | RuntimeExpr::Sum { source: value }
        | RuntimeExpr::Unary { expr: value, .. } => {
            rewrite_known_record_projections_in_expr(value, env);
        }
        RuntimeExpr::Range { start, end, .. } => {
            rewrite_known_record_projections_in_optional_expr(start.as_deref_mut(), env);
            rewrite_known_record_projections_in_optional_expr(end.as_deref_mut(), env);
        }
        RuntimeExpr::Record(fields) => {
            for field in fields {
                rewrite_known_record_projections_in_expr(&mut field.value, env);
            }
        }
        RuntimeExpr::Variant { payload, .. } => {
            if let Some(payload) = payload {
                rewrite_known_record_projections_in_expr(payload, env);
            }
        }
        RuntimeExpr::Call { args, .. } | RuntimeExpr::PureCall { args, .. } => {
            for arg in args {
                rewrite_known_record_projections_in_expr(arg, env);
            }
        }
        RuntimeExpr::Function { params, body } => {
            let env = record_projection_env_without_bindings(env, params);
            rewrite_known_record_projections_in_expr(body, &env);
        }
        RuntimeExpr::Apply { callee, args } => {
            rewrite_known_record_projections_in_apply(callee, args, env);
        }
        RuntimeExpr::MethodCall { receiver, args, .. }
        | RuntimeExpr::TraitCall { receiver, args, .. } => {
            rewrite_known_record_projections_in_receiver_args(receiver, args, env);
        }
        RuntimeExpr::Map {
            source,
            param,
            body,
        }
        | RuntimeExpr::Filter {
            source,
            param,
            body,
        } => rewrite_known_record_projections_in_scoped_body(source, param, body, env),
        RuntimeExpr::Binary { lhs, rhs, .. } => {
            rewrite_known_record_projections_in_expr(lhs, env);
            rewrite_known_record_projections_in_expr(rhs, env);
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => rewrite_known_record_projections_in_if(condition, then_expr, else_expr, env),
        RuntimeExpr::IfLet {
            expr,
            guard,
            then_expr,
            else_expr,
            ..
        } => rewrite_known_record_projections_in_if_let(
            expr,
            guard.as_deref_mut(),
            then_expr,
            else_expr,
            env,
        ),
        RuntimeExpr::Match { scrutinee, arms } => {
            rewrite_known_record_projections_in_match(scrutinee, arms, env);
        }
        RuntimeExpr::Value(_) | RuntimeExpr::Local(_) | RuntimeExpr::EntityRef(_) => {}
    }
}

fn rewrite_known_record_projection_field(
    target: &mut Box<RuntimeExpr>,
    field: &str,
    env: &[(String, Vec<String>)],
) -> Option<RuntimeExpr> {
    rewrite_known_record_projections_in_expr(target, env);
    let RuntimeExpr::Local(name) = target.as_ref() else {
        return None;
    };
    record_field_ordinal(env, name, field).map(|ordinal| {
        let target = std::mem::replace(target, Box::new(RuntimeExpr::Value(RuntimeValue::Unit)));
        RuntimeExpr::ProjectRecord { target, ordinal }
    })
}

fn rewrite_known_record_projections_in_scoped_body(
    source: &mut RuntimeExpr,
    param: &str,
    body: &mut RuntimeExpr,
    env: &[(String, Vec<String>)],
) {
    rewrite_known_record_projections_in_expr(source, env);
    let env = record_projection_env_without_binding(env, param);
    rewrite_known_record_projections_in_expr(body, &env);
}

fn rewrite_known_record_projections_in_apply(
    callee: &mut RuntimeExpr,
    args: &mut [RuntimeExpr],
    env: &[(String, Vec<String>)],
) {
    rewrite_known_record_projections_in_expr(callee, env);
    for arg in args {
        rewrite_known_record_projections_in_expr(arg, env);
    }
}

fn rewrite_known_record_projections_in_if_let(
    expr: &mut RuntimeExpr,
    guard: Option<&mut RuntimeExpr>,
    then_expr: &mut RuntimeExpr,
    else_expr: &mut RuntimeExpr,
    env: &[(String, Vec<String>)],
) {
    rewrite_known_record_projections_in_expr(expr, env);
    if let Some(guard) = guard {
        rewrite_known_record_projections_in_expr(guard, env);
    }
    rewrite_known_record_projections_in_expr(then_expr, env);
    rewrite_known_record_projections_in_expr(else_expr, env);
}

fn rewrite_known_record_projections_in_if(
    condition: &mut RuntimeExpr,
    then_expr: &mut RuntimeExpr,
    else_expr: &mut RuntimeExpr,
    env: &[(String, Vec<String>)],
) {
    rewrite_known_record_projections_in_expr(condition, env);
    rewrite_known_record_projections_in_expr(then_expr, env);
    rewrite_known_record_projections_in_expr(else_expr, env);
}

fn rewrite_known_record_projections_in_match(
    scrutinee: &mut RuntimeExpr,
    arms: &mut [RuntimeExprMatchArm],
    env: &[(String, Vec<String>)],
) {
    rewrite_known_record_projections_in_expr(scrutinee, env);
    for arm in arms {
        if let Some(guard) = &mut arm.guard {
            rewrite_known_record_projections_in_expr(guard, env);
        }
        rewrite_known_record_projections_in_expr(&mut arm.value, env);
    }
}

fn record_projection_env_without_bindings(
    env: &[(String, Vec<String>)],
    names: &[String],
) -> Vec<(String, Vec<String>)> {
    env.iter()
        .filter(|(name, _)| !names.iter().any(|bound| bound == name))
        .cloned()
        .collect()
}

fn record_projection_env_without_binding(
    env: &[(String, Vec<String>)],
    name: &str,
) -> Vec<(String, Vec<String>)> {
    env.iter()
        .filter(|(candidate, _)| candidate != name)
        .cloned()
        .collect()
}

fn rewrite_known_record_projections_in_receiver_args(
    receiver: &mut RuntimeExpr,
    args: &mut [RuntimeExpr],
    env: &[(String, Vec<String>)],
) {
    rewrite_known_record_projections_in_expr(receiver, env);
    for arg in args {
        rewrite_known_record_projections_in_expr(arg, env);
    }
}

fn record_field_ordinal(env: &[(String, Vec<String>)], name: &str, field: &str) -> Option<usize> {
    env.iter()
        .rev()
        .find(|(candidate, _)| candidate == name)
        .and_then(|(_, fields)| fields.iter().position(|candidate| candidate == field))
}
