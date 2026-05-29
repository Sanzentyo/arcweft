use crate::fact_layer::{
    ResourceAccess, resource_accesses_from_expr, resource_write_for_lifetime,
    resource_write_for_signal,
};
use arcweft_lang_hir::syntax::{
    ast::{
        flow::{Stmt, WaitTarget},
        line_plan::LinePlanItem,
    },
    expr::{Expr, LifetimeKey, LifetimeScopeKind, Literal},
};
use std::collections::{BTreeSet, HashSet};

pub(super) fn stmts_contain_unchecked_promotion(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_contains_unchecked_promotion)
}

fn stmt_contains_unchecked_promotion(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { expr, .. }
        | Stmt::LetElse { expr, .. }
        | Stmt::Return(expr)
        | Stmt::Out { expr, .. }
        | Stmt::Goto(expr)
        | Stmt::Defer { expr, .. }
        | Stmt::Yield(expr)
        | Stmt::Expr(expr)
        | Stmt::Close(expr)
        | Stmt::Select(expr)
        | Stmt::Break {
            expr: Some(expr), ..
        }
        | Stmt::Wait(WaitTarget::Duration(expr) | WaitTarget::Expr(expr)) => {
            expr_contains_unchecked_promotion(expr)
        }
        Stmt::Signal { target, value }
        | Stmt::LifetimeSet {
            target,
            expr: value,
        } => expr_contains_unchecked_promotion(target) || expr_contains_unchecked_promotion(value),
        Stmt::LetChoice { .. }
        | Stmt::LetScope { .. }
        | Stmt::LetLoop { .. }
        | Stmt::LetAwait { .. }
        | Stmt::Break { expr: None, .. }
        | Stmt::Continue { .. }
        | Stmt::Raw(_) => false,
        Stmt::Thread(thread) => stmts_contain_unchecked_promotion(thread.body()),
        Stmt::DeferBlock { statements, .. }
        | Stmt::On {
            body: statements, ..
        }
        | Stmt::UnsafeLifetime {
            body: statements, ..
        }
        | Stmt::If {
            body: statements, ..
        }
        | Stmt::Loop { body: statements }
        | Stmt::While {
            body: statements, ..
        }
        | Stmt::WhileLet {
            body: statements, ..
        }
        | Stmt::For {
            body: statements, ..
        } => stmts_contain_unchecked_promotion(statements),
        Stmt::Match { arms, .. } => arms
            .iter()
            .any(|arm| stmts_contain_unchecked_promotion(arm.body())),
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "expression traversal mirrors Expr so unsafe audit coverage stays auditable"
)]
fn expr_contains_unchecked_promotion(expr: &Expr) -> bool {
    match expr {
        Expr::Call { callee, args } => {
            matches!(callee.as_ref(), Expr::Path(path) if path == "promote_unchecked")
                || expr_contains_unchecked_promotion(callee)
                || args
                    .iter()
                    .any(|arg| expr_contains_unchecked_promotion(arg.value()))
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => {
            method == "promote_unchecked"
                || expr_contains_unchecked_promotion(receiver)
                || args
                    .iter()
                    .any(|arg| expr_contains_unchecked_promotion(arg.value()))
        }
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            items.iter().any(expr_contains_unchecked_promotion)
        }
        Expr::ArrayRepeat { value, len } => {
            expr_contains_unchecked_promotion(value) || expr_contains_unchecked_promotion(len)
        }
        Expr::Field { target: value, .. }
        | Expr::Try { expr: value }
        | Expr::Await { expr: value, .. }
        | Expr::Unary { expr: value, .. }
        | Expr::Closure { body: value, .. } => expr_contains_unchecked_promotion(value),
        Expr::Binary { lhs, rhs, .. }
        | Expr::Pipe { lhs, rhs }
        | Expr::Index {
            target: lhs,
            index: rhs,
        } => expr_contains_unchecked_promotion(lhs) || expr_contains_unchecked_promotion(rhs),
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => fields
            .iter()
            .any(|(_, value)| expr_contains_unchecked_promotion(value)),
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
            stmts_contain_unchecked_promotion(statements)
                || value
                    .as_ref()
                    .is_some_and(|value| expr_contains_unchecked_promotion(value))
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_contains_unchecked_promotion(condition)
                || expr_contains_unchecked_promotion(then_branch)
                || else_branch
                    .as_ref()
                    .is_some_and(|value| expr_contains_unchecked_promotion(value))
        }
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => {
            expr_contains_unchecked_promotion(expr)
                || guard
                    .as_ref()
                    .is_some_and(|guard| expr_contains_unchecked_promotion(guard))
                || expr_contains_unchecked_promotion(then_branch)
                || else_branch
                    .as_ref()
                    .is_some_and(|value| expr_contains_unchecked_promotion(value))
        }
        Expr::Match { scrutinee, arms } => {
            expr_contains_unchecked_promotion(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard().is_some_and(expr_contains_unchecked_promotion)
                        || expr_contains_unchecked_promotion(arm.value())
                })
        }
        Expr::Thread { block } => stmts_contain_unchecked_promotion(block.body()),
        Expr::Range { start, end, .. } => {
            start
                .as_ref()
                .is_some_and(|value| expr_contains_unchecked_promotion(value))
                || end
                    .as_ref()
                    .is_some_and(|value| expr_contains_unchecked_promotion(value))
        }
        Expr::DialogueCall { callee, plan, .. } => {
            expr_contains_unchecked_promotion(callee)
                || plan.as_ref().is_some_and(|plan| {
                    plan.items()
                        .iter()
                        .any(line_plan_item_contains_unchecked_promotion)
                })
        }
        Expr::Literal(_)
        | Expr::Path(_)
        | Expr::Placeholder(_)
        | Expr::EntityRef(_)
        | Expr::LifetimePath { .. }
        | Expr::Raw(_) => false,
    }
}

fn line_plan_item_contains_unchecked_promotion(item: &LinePlanItem) -> bool {
    match item {
        LinePlanItem::Init(stmts) => stmts_contain_unchecked_promotion(stmts),
        LinePlanItem::Thread(thread) => stmts_contain_unchecked_promotion(thread.body()),
        LinePlanItem::On { body, .. } => stmts_contain_unchecked_promotion(body),
        LinePlanItem::Let { expr, .. }
        | LinePlanItem::Option { value: expr, .. }
        | LinePlanItem::Assert { expr, .. }
        | LinePlanItem::Expr(expr)
        | LinePlanItem::Out(expr) => expr_contains_unchecked_promotion(expr),
        LinePlanItem::Stmt(stmt) => stmt_contains_unchecked_promotion(stmt),
        LinePlanItem::TimedCue { anchor, body } => {
            expr_contains_unchecked_promotion(anchor) || expr_contains_unchecked_promotion(body)
        }
        LinePlanItem::CancelRule(rule) => stmts_contain_unchecked_promotion(rule.action()),
        LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => items
            .iter()
            .any(line_plan_item_contains_unchecked_promotion),
        LinePlanItem::Raw(_) => false,
    }
}

pub(super) fn is_must_drop_key(key: &LifetimeKey) -> bool {
    key.scope() == &LifetimeScopeKind::Line
        && key.path().first().is_some_and(|part| part == "focus")
}

pub(super) fn is_drop_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Path(path) if matches!(path.as_str(), "drop" | "drop_optional" | "on_drop")
    ) || matches!(
        expr,
        Expr::Call { callee, .. }
            if matches!(callee.as_ref(), Expr::Path(path) if matches!(path.as_str(), "drop" | "drop_optional" | "on_drop"))
    )
}

pub(super) fn write_accesses_in_stmts(stmts: &[Stmt]) -> BTreeSet<ResourceAccess> {
    let mut accesses = BTreeSet::new();
    for stmt in stmts {
        collect_stmt_write_accesses(stmt, &mut accesses);
    }
    accesses
}

pub(super) fn write_accesses_in_line_plan_item(item: &LinePlanItem) -> BTreeSet<ResourceAccess> {
    let mut accesses = BTreeSet::new();
    collect_line_plan_item_write_accesses(item, &mut accesses);
    accesses
}

fn collect_line_plan_item_write_accesses(
    item: &LinePlanItem,
    accesses: &mut BTreeSet<ResourceAccess>,
) {
    match item {
        LinePlanItem::Init(stmts) => {
            for stmt in stmts {
                collect_stmt_write_accesses(stmt, accesses);
            }
        }
        LinePlanItem::Thread(thread) => {
            for stmt in thread.body() {
                collect_stmt_write_accesses(stmt, accesses);
            }
        }
        LinePlanItem::On { body, .. } => {
            for stmt in body {
                collect_stmt_write_accesses(stmt, accesses);
            }
        }
        LinePlanItem::Stmt(stmt) => collect_stmt_write_accesses(stmt, accesses),
        LinePlanItem::TimedCue { body, .. } | LinePlanItem::Expr(body) => {
            collect_expr_write_accesses(body, accesses);
        }
        LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
            for item in items {
                collect_line_plan_item_write_accesses(item, accesses);
            }
        }
        _ => {}
    }
}

pub(super) fn line_plan_child_name(item: &LinePlanItem, index: usize) -> String {
    match item {
        LinePlanItem::Thread(thread) => thread
            .name()
            .map_or_else(|| format!("thread#{index}"), str::to_owned),
        LinePlanItem::On { .. } => format!("on#{index}"),
        LinePlanItem::TimedCue { .. } => format!("at#{index}"),
        LinePlanItem::StartGroup(_) => format!("start#{index}"),
        LinePlanItem::TogetherGroup(_) => format!("together#{index}"),
        _ => format!("item#{index}"),
    }
}

pub(super) fn is_line_plan_child_item(item: &LinePlanItem) -> bool {
    matches!(
        item,
        LinePlanItem::Thread(_)
            | LinePlanItem::On { .. }
            | LinePlanItem::TimedCue { .. }
            | LinePlanItem::StartGroup(_)
            | LinePlanItem::TogetherGroup(_)
    )
}

pub(super) fn drop_keys_in_stmts(stmts: &[Stmt]) -> HashSet<LifetimeKey> {
    let mut keys = HashSet::new();
    for stmt in stmts {
        collect_stmt_drop_keys(stmt, &mut keys);
    }
    keys
}

fn collect_stmt_drop_keys(stmt: &Stmt, keys: &mut HashSet<LifetimeKey>) {
    match stmt {
        Stmt::Expr(expr) | Stmt::Defer { expr, .. } => collect_expr_drop_keys(expr, keys),
        Stmt::DeferBlock { statements, .. }
        | Stmt::If {
            body: statements, ..
        }
        | Stmt::Loop { body: statements }
        | Stmt::While {
            body: statements, ..
        }
        | Stmt::WhileLet {
            body: statements, ..
        }
        | Stmt::For {
            body: statements, ..
        }
        | Stmt::On {
            body: statements, ..
        }
        | Stmt::UnsafeLifetime {
            body: statements, ..
        } => {
            for stmt in statements {
                collect_stmt_drop_keys(stmt, keys);
            }
        }
        Stmt::Match { arms, .. } => {
            for arm in arms {
                for stmt in arm.body() {
                    collect_stmt_drop_keys(stmt, keys);
                }
            }
        }
        _ => {}
    }
}

pub(super) fn drop_keys_in_expr(expr: &Expr) -> HashSet<LifetimeKey> {
    let mut keys = HashSet::new();
    collect_expr_drop_keys(expr, &mut keys);
    keys
}

fn collect_expr_drop_keys(expr: &Expr, keys: &mut HashSet<LifetimeKey>) {
    match expr {
        Expr::Pipe { lhs, rhs } if is_drop_expr(rhs) => {
            if let Expr::LifetimePath { key, .. } = lhs.as_ref() {
                keys.insert(key.clone());
            }
            collect_expr_drop_keys(rhs, keys);
        }
        Expr::Call { callee, args } if matches!(callee.as_ref(), Expr::Path(path) if matches!(path.as_str(), "drop" | "drop_optional" | "on_drop")) => {
            for arg in args {
                if let Expr::LifetimePath { key, .. } = arg.value() {
                    keys.insert(key.clone());
                }
            }
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
        } if matches!(method.as_str(), "drop" | "drop_optional" | "on_drop") => {
            if let Expr::LifetimePath { key, .. } = receiver.as_ref() {
                keys.insert(key.clone());
            }
            for arg in args {
                collect_expr_drop_keys(arg.value(), keys);
            }
        }
        Expr::Call { callee, args } => {
            collect_expr_drop_keys(callee, keys);
            for arg in args {
                collect_expr_drop_keys(arg.value(), keys);
            }
        }
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            for item in items {
                collect_expr_drop_keys(item, keys);
            }
        }
        Expr::Field { target: value, .. }
        | Expr::Try { expr: value }
        | Expr::Await { expr: value, .. }
        | Expr::Unary { expr: value, .. } => collect_expr_drop_keys(value, keys),
        Expr::MethodCall { receiver, args, .. } => {
            collect_expr_drop_keys(receiver, keys);
            for arg in args {
                collect_expr_drop_keys(arg.value(), keys);
            }
        }
        Expr::Binary { lhs, rhs, .. }
        | Expr::Pipe { lhs, rhs }
        | Expr::Index {
            target: lhs,
            index: rhs,
        } => {
            collect_expr_drop_keys(lhs, keys);
            collect_expr_drop_keys(rhs, keys);
        }
        _ => {}
    }
}

fn collect_stmt_write_accesses(stmt: &Stmt, accesses: &mut BTreeSet<ResourceAccess>) {
    match stmt {
        Stmt::LifetimeSet {
            target: Expr::LifetimePath { key, .. },
            ..
        } => {
            accesses.insert(resource_write_for_lifetime(key.as_dotted()));
        }
        Stmt::Signal { target, .. } => {
            accesses.insert(resource_write_for_signal(expr_label(target)));
        }
        Stmt::Expr(expr) | Stmt::Defer { expr, .. } => {
            collect_expr_write_accesses(expr, accesses);
        }
        Stmt::DeferBlock { statements, .. } => {
            for stmt in statements {
                collect_stmt_write_accesses(stmt, accesses);
            }
        }
        Stmt::On { .. } | Stmt::Thread(_) => {
            let body = match stmt {
                Stmt::On { body, .. } => body.as_slice(),
                Stmt::Thread(thread) => thread.body(),
                _ => &[],
            };
            for stmt in body {
                collect_stmt_write_accesses(stmt, accesses);
            }
        }
        _ => {}
    }
}

fn collect_expr_write_accesses(expr: &Expr, accesses: &mut BTreeSet<ResourceAccess>) {
    accesses.extend(resource_accesses_from_expr(expr));
    match expr {
        Expr::Call { args, .. } => {
            for arg in args {
                collect_expr_write_accesses(arg.value(), accesses);
            }
        }
        Expr::Tuple(args) | Expr::BracketSeq(args) => {
            for arg in args {
                collect_expr_write_accesses(arg, accesses);
            }
        }
        Expr::ArrayRepeat { value, len } => {
            collect_expr_write_accesses(value, accesses);
            collect_expr_write_accesses(len, accesses);
        }
        Expr::Field { target: value, .. }
        | Expr::Try { expr: value }
        | Expr::Await { expr: value, .. }
        | Expr::Unary { expr: value, .. } => collect_expr_write_accesses(value, accesses),
        Expr::MethodCall { receiver, args, .. } => {
            collect_expr_write_accesses(receiver, accesses);
            for arg in args {
                collect_expr_write_accesses(arg.value(), accesses);
            }
        }
        Expr::Binary { lhs, rhs, .. }
        | Expr::Pipe { lhs, rhs }
        | Expr::Index {
            target: lhs,
            index: rhs,
        } => {
            collect_expr_write_accesses(lhs, accesses);
            collect_expr_write_accesses(rhs, accesses);
        }
        _ => {}
    }
}

pub(super) fn thread_result_type_labels(stmts: &[Stmt], can_fallthrough: bool) -> BTreeSet<String> {
    let mut labels = BTreeSet::new();
    for stmt in stmts {
        collect_thread_result_type_labels(stmt, &mut labels);
    }
    if can_fallthrough {
        labels.insert("Unit".to_owned());
    }
    if labels.len() > 1 {
        labels.remove("Unknown");
    }
    labels
}

fn collect_thread_result_type_labels(stmt: &Stmt, labels: &mut BTreeSet<String>) {
    match stmt {
        Stmt::Out { expr, .. } => {
            labels.insert(expr_type_label(expr));
        }
        Stmt::If { .. }
        | Stmt::Loop { .. }
        | Stmt::While { .. }
        | Stmt::WhileLet { .. }
        | Stmt::For { .. } => {
            let body = match stmt {
                Stmt::If { body, .. }
                | Stmt::Loop { body }
                | Stmt::While { body, .. }
                | Stmt::WhileLet { body, .. }
                | Stmt::For { body, .. } => body.as_slice(),
                _ => &[],
            };
            for stmt in body {
                collect_thread_result_type_labels(stmt, labels);
            }
        }
        Stmt::Match { arms, .. } => {
            for arm in arms {
                for stmt in arm.body() {
                    collect_thread_result_type_labels(stmt, labels);
                }
            }
        }
        Stmt::LetElse { else_body, .. } => {
            for stmt in else_body {
                collect_thread_result_type_labels(stmt, labels);
            }
        }
        _ => {}
    }
}

fn expr_type_label(expr: &Expr) -> String {
    match expr {
        Expr::Literal(Literal::String(_)) => "String".to_owned(),
        Expr::Literal(Literal::Char { .. }) => "Char".to_owned(),
        Expr::Literal(Literal::Int { suffix, .. }) => {
            suffix.as_deref().unwrap_or("unsuffixed-int").to_owned()
        }
        Expr::Literal(Literal::Float { suffix, .. }) => {
            suffix.as_deref().unwrap_or("unsuffixed-float").to_owned()
        }
        Expr::Literal(Literal::Bool(_)) => "Bool".to_owned(),
        Expr::Literal(Literal::Duration { .. }) => "Duration".to_owned(),
        Expr::Tuple(items) => {
            let labels = items.iter().map(expr_type_label).collect::<Vec<_>>();
            format!("({})", labels.join(", "))
        }
        Expr::BracketSeq(items) => items.first().map_or_else(
            || "Vec<Unknown>".to_owned(),
            |item| format!("Vec<{}>", expr_type_label(item)),
        ),
        Expr::ArrayRepeat { value, len } => {
            format!("Array<{}, {}>", expr_type_label(value), expr_label(len))
        }
        Expr::EntityRef(_) => "EntityRef".to_owned(),
        _ => "Unknown".to_owned(),
    }
}

fn expr_label(expr: &Expr) -> String {
    match expr {
        Expr::Path(path) => path.clone(),
        Expr::EntityRef(entity) => entity.body().to_owned(),
        Expr::LifetimePath { key, .. } => key.as_dotted(),
        Expr::Literal(literal) => format!("{literal:?}"),
        _ => format!("{expr:?}"),
    }
}
