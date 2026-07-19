use crate::app::bundle_view::normalize_view_call;
use arcweft_lang_hir::model::{HirFlowItem, HirModule, HirTopLevelDecl};
use arcweft_lang_syntax::{
    ast::flow::{FlowItem, Stmt, WaitTarget},
    expr::{CallArg, Expr},
};
use std::collections::BTreeSet;

pub(super) fn mounted_view_ids(module: &HirModule) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for declaration in module.declarations() {
        let HirTopLevelDecl::DialogueDefaults(defaults) = declaration else {
            continue;
        };
        for assignment in defaults
            .assignments()
            .iter()
            .filter(|assignment| assignment.path().dotted() == "view")
        {
            ids.insert(normalize_view_call(assignment.value()));
        }
    }
    for flow in module.flows() {
        collect_mounted_view_ids(flow.body(), &mut ids);
    }
    collect_mounted_view_ids(module.top_level_items(), &mut ids);
    include_nested_view_dependencies(module, &mut ids);
    ids
}

fn include_nested_view_dependencies(module: &HirModule, ids: &mut BTreeSet<String>) {
    loop {
        let mut changed = false;
        for declaration in module.declarations() {
            let HirTopLevelDecl::EntityDecl(view) = declaration else {
                continue;
            };
            if !mounted_view_matches(ids, view.id().body()) {
                continue;
            }
            let Some(body) = view.view_body().and_then(|body| body.view()) else {
                continue;
            };
            for call in body.view_calls() {
                changed |= ids.insert(normalize_view_call(call.view()));
            }
        }
        if !changed {
            break;
        }
    }
}

pub(super) fn mounted_view_matches(mounted: &BTreeSet<String>, declared: &str) -> bool {
    mounted
        .iter()
        .any(|mounted| view_ids_equivalent(mounted, declared))
}

fn view_ids_equivalent(left: &str, right: &str) -> bool {
    left == right || view_id_suffix(left) == view_id_suffix(right)
}

fn view_id_suffix(value: &str) -> &str {
    value.strip_prefix("view.").unwrap_or(value)
}

fn collect_mounted_view_ids(items: &[HirFlowItem], ids: &mut BTreeSet<String>) {
    for item in items {
        match item {
            HirFlowItem::Stmt(stmt) => collect_mounted_view_ids_from_stmt(stmt, ids),
            HirFlowItem::LetScope { scope, .. } => {
                for stmt in scope.statements() {
                    collect_mounted_view_ids_from_stmt(stmt, ids);
                }
                if let Some(value) = scope.value() {
                    collect_mounted_view_ids_from_expr(value, ids);
                }
            }
            HirFlowItem::Thread(thread) => collect_mounted_view_ids(thread.body(), ids),
            HirFlowItem::If(block) => {
                collect_mounted_view_ids(block.body(), ids);
                collect_mounted_view_ids(block.else_body(), ids);
            }
            HirFlowItem::IfLet(block) => {
                collect_mounted_view_ids_from_expr(block.expr(), ids);
                if let Some(guard) = block.guard() {
                    collect_mounted_view_ids_from_expr(guard, ids);
                }
                collect_mounted_view_ids(block.body(), ids);
                collect_mounted_view_ids(block.else_body(), ids);
            }
            HirFlowItem::Match(block) => {
                collect_mounted_view_ids_from_expr(block.expr(), ids);
                for arm in block.arms() {
                    if let Some(guard) = arm.guard() {
                        collect_mounted_view_ids_from_expr(guard, ids);
                    }
                    collect_mounted_view_ids(arm.body(), ids);
                }
            }
            HirFlowItem::Loop(block) | HirFlowItem::LetLoop { block, .. } => {
                collect_mounted_view_ids(block.body(), ids);
            }
            HirFlowItem::While(block) => {
                collect_mounted_view_ids_from_expr(block.condition(), ids);
                collect_mounted_view_ids(block.body(), ids);
            }
            HirFlowItem::WhileLet(block) => {
                collect_mounted_view_ids_from_expr(block.expr(), ids);
                if let Some(guard) = block.guard() {
                    collect_mounted_view_ids_from_expr(guard, ids);
                }
                collect_mounted_view_ids(block.body(), ids);
            }
            HirFlowItem::For(block) => {
                collect_mounted_view_ids_from_expr(block.source(), ids);
                collect_mounted_view_ids(block.body(), ids);
            }
            HirFlowItem::Select(block) => {
                for branch in block.branches() {
                    collect_mounted_view_ids(branch.body(), ids);
                }
            }
            HirFlowItem::SourceLocale(block) => collect_mounted_view_ids(block.body(), ids),
            HirFlowItem::Scope(block) => collect_mounted_view_ids(block.body(), ids),
            HirFlowItem::Await(block)
            | HirFlowItem::LetAwait {
                await_with: block, ..
            } => {
                collect_mounted_view_ids_from_expr(block.expr(), ids);
                for branch in block.branches() {
                    collect_mounted_view_ids(branch.body(), ids);
                }
            }
            HirFlowItem::Dialogue(dialogue) => {
                if let Some(view) = dialogue.view() {
                    ids.insert(format!("view.{}", view.body().trim_start_matches("view.")));
                }
            }
            HirFlowItem::Choice(_) | HirFlowItem::LetChoice { .. } | HirFlowItem::Include(_) => {}
        }
    }
}

fn collect_mounted_view_ids_from_stmt(stmt: &Stmt, ids: &mut BTreeSet<String>) {
    match stmt {
        Stmt::Assertion(assertion) => collect_mounted_view_ids_from_assertion(assertion, ids),
        Stmt::Let { expr, .. } | Stmt::Return { expr, .. } | Stmt::Expr { expr, .. } => {
            collect_mounted_view_ids_from_expr(expr, ids);
        }
        Stmt::Out { expr, .. }
        | Stmt::Defer { expr, .. }
        | Stmt::Goto(expr)
        | Stmt::Yield(expr)
        | Stmt::Close(expr)
        | Stmt::Select(expr) => {
            collect_mounted_view_ids_from_expr(expr.expr(), ids);
        }
        Stmt::Assign { target, expr }
        | Stmt::LifetimeSet { target, expr }
        | Stmt::Signal {
            target,
            value: expr,
        } => {
            collect_mounted_view_ids_from_expr(target.expr(), ids);
            collect_mounted_view_ids_from_expr(expr.expr(), ids);
        }
        Stmt::LetElse {
            expr, else_body, ..
        } => {
            collect_mounted_view_ids_from_expr(expr.expr(), ids);
            collect_mounted_view_ids_from_stmts(else_body, ids);
        }
        Stmt::LetScope { scope, .. } => {
            collect_mounted_view_ids_from_stmts(scope.statements(), ids);
            if let Some(value) = scope.value() {
                collect_mounted_view_ids_from_expr(value, ids);
            }
        }
        Stmt::LetActionReceive { action, .. } => {
            collect_mounted_view_ids_from_expr(action.expr(), ids);
        }
        Stmt::Thread(thread) => {
            collect_mounted_view_ids_from_syntax_flow_items(thread.body(), ids);
        }
        Stmt::DeferBlock { statements, .. } => {
            collect_mounted_view_ids_from_stmts(statements, ids);
        }
        Stmt::Wait(target) => match target {
            WaitTarget::Duration(expr) | WaitTarget::Expr(expr) => {
                collect_mounted_view_ids_from_expr(expr.expr(), ids);
            }
        },
        Stmt::On { body, .. } | Stmt::UnsafeLifetime { body, .. } | Stmt::Loop { body } => {
            collect_mounted_view_ids_from_stmts(body, ids);
        }
        Stmt::If {
            condition,
            body,
            else_body,
        } => {
            collect_mounted_view_ids_from_expr(condition.expr(), ids);
            collect_mounted_view_ids_from_stmts(body, ids);
            collect_mounted_view_ids_from_stmts(else_body, ids);
        }
        Stmt::While { condition, body } => {
            collect_mounted_view_ids_from_expr(condition.expr(), ids);
            collect_mounted_view_ids_from_stmts(body, ids);
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            collect_mounted_view_ids_from_expr(expr.expr(), ids);
            if let Some(guard) = guard {
                collect_mounted_view_ids_from_expr(guard.expr(), ids);
            }
            collect_mounted_view_ids_from_stmts(body, ids);
        }
        Stmt::For { source, body, .. } => {
            collect_mounted_view_ids_from_expr(source.expr(), ids);
            collect_mounted_view_ids_from_stmts(body, ids);
        }
        Stmt::Match { expr, arms } => {
            collect_mounted_view_ids_from_expr(expr.expr(), ids);
            for arm in arms {
                if let Some(guard) = arm.guard() {
                    collect_mounted_view_ids_from_expr(guard, ids);
                }
                collect_mounted_view_ids_from_stmts(arm.body(), ids);
            }
        }
        Stmt::LetLoop { block, .. } => {
            collect_mounted_view_ids_from_syntax_flow_items(block.body(), ids);
        }
        Stmt::LetAwait { await_with, .. } => {
            collect_mounted_view_ids_from_expr(await_with.expr(), ids);
            for branch in await_with.branches() {
                collect_mounted_view_ids_from_syntax_flow_items(branch.body(), ids);
            }
        }
        Stmt::LetChoice { .. } | Stmt::Break { .. } | Stmt::Continue { .. } | Stmt::Raw(_) => {}
    }
}

fn collect_mounted_view_ids_from_assertion(
    assertion: &arcweft_lang_syntax::assertion::AssertionStmt,
    ids: &mut BTreeSet<String>,
) {
    for condition in assertion.conditions() {
        collect_mounted_view_ids_from_expr(condition, ids);
    }
}

fn collect_mounted_view_ids_from_stmts(stmts: &[Stmt], ids: &mut BTreeSet<String>) {
    for stmt in stmts {
        collect_mounted_view_ids_from_stmt(stmt, ids);
    }
}

fn collect_mounted_view_ids_from_syntax_flow_items(items: &[FlowItem], ids: &mut BTreeSet<String>) {
    for item in items {
        if let FlowItem::Stmt(stmt) = item {
            collect_mounted_view_ids_from_stmt(stmt, ids);
        }
    }
}

fn collect_mounted_view_ids_from_expr(expr: &Expr, ids: &mut BTreeSet<String>) {
    if let Some(id) = mounted_view_id_from_expr(expr) {
        ids.insert(id);
    }
    match expr {
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            for item in items {
                collect_mounted_view_ids_from_expr(item, ids);
            }
        }
        Expr::ArrayRepeat { value, len } => {
            collect_mounted_view_ids_from_expr(value, ids);
            collect_mounted_view_ids_from_expr(len, ids);
        }
        Expr::Call(call) => {
            collect_mounted_view_ids_from_expr(call.callee(), ids);
            collect_mounted_view_ids_from_call_args(call.args(), ids);
        }
        Expr::Select(select) => collect_mounted_view_ids_from_expr(select.target(), ids),
        Expr::Index { target, index } => {
            collect_mounted_view_ids_from_expr(target, ids);
            collect_mounted_view_ids_from_expr(index, ids);
        }
        Expr::DialogueCall { callee, .. } => collect_mounted_view_ids_from_expr(callee, ids),
        Expr::Pipe { lhs, rhs } | Expr::Binary { lhs, rhs, .. } => {
            collect_mounted_view_ids_from_expr(lhs, ids);
            collect_mounted_view_ids_from_expr(rhs, ids);
        }
        Expr::Try { expr } | Expr::Await { expr, .. } | Expr::Unary { expr, .. } => {
            collect_mounted_view_ids_from_expr(expr, ids);
        }
        Expr::Borrow(borrow) => collect_mounted_view_ids_from_expr(borrow.operand(), ids),
        Expr::Deref(deref) => collect_mounted_view_ids_from_expr(deref.operand(), ids),
        Expr::Thread { block } => {
            collect_mounted_view_ids_from_syntax_flow_items(block.body(), ids);
        }
        Expr::Range { start, end, .. } => collect_optional_exprs(
            [start.as_deref(), end.as_deref()].into_iter().flatten(),
            ids,
        ),
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
            for (_, value) in fields {
                collect_mounted_view_ids_from_expr(value, ids);
            }
        }
        Expr::Closure { body, .. } => collect_mounted_view_ids_from_expr(body, ids),
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => collect_mounted_view_ids_from_block_expr(statements, value.as_deref(), ids),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => collect_mounted_view_ids_from_if_expr(
            condition,
            then_branch,
            else_branch.as_deref(),
            ids,
        ),
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => collect_mounted_view_ids_from_if_let_expr(
            expr,
            guard.as_deref(),
            then_branch,
            else_branch.as_deref(),
            ids,
        ),
        Expr::Match { scrutinee, arms } => {
            collect_mounted_view_ids_from_expr(scrutinee, ids);
            for arm in arms {
                collect_optional_exprs(arm.guard().into_iter(), ids);
                collect_mounted_view_ids_from_expr(arm.value(), ids);
            }
        }
        Expr::Literal(_)
        | Expr::EntityRef(_)
        | Expr::LifetimePath { .. }
        | Expr::Path(_)
        | Expr::ShortVariant(_)
        | Expr::Placeholder(_)
        | Expr::NumericBracketSeq(_)
        | Expr::Raw(_) => {}
    }
}

fn collect_optional_exprs<'a>(values: impl Iterator<Item = &'a Expr>, ids: &mut BTreeSet<String>) {
    for value in values {
        collect_mounted_view_ids_from_expr(value, ids);
    }
}

fn collect_mounted_view_ids_from_block_expr(
    statements: &[Stmt],
    value: Option<&Expr>,
    ids: &mut BTreeSet<String>,
) {
    collect_mounted_view_ids_from_stmts(statements, ids);
    collect_optional_exprs(value.into_iter(), ids);
}

fn collect_mounted_view_ids_from_if_expr(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    ids: &mut BTreeSet<String>,
) {
    collect_mounted_view_ids_from_expr(condition, ids);
    collect_mounted_view_ids_from_expr(then_branch, ids);
    collect_optional_exprs(else_branch.into_iter(), ids);
}

fn collect_mounted_view_ids_from_if_let_expr(
    expr: &Expr,
    guard: Option<&Expr>,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    ids: &mut BTreeSet<String>,
) {
    collect_mounted_view_ids_from_expr(expr, ids);
    collect_optional_exprs(guard.into_iter(), ids);
    collect_mounted_view_ids_from_expr(then_branch, ids);
    collect_optional_exprs(else_branch.into_iter(), ids);
}

fn collect_mounted_view_ids_from_call_args(args: &[CallArg], ids: &mut BTreeSet<String>) {
    for arg in args {
        match arg {
            CallArg::Positional(expr) => collect_mounted_view_ids_from_expr(expr, ids),
            CallArg::Named { value, .. } | CallArg::Spread { value } => {
                collect_mounted_view_ids_from_expr(value, ids);
            }
        }
    }
}

fn mounted_view_id_from_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Raw(source) => {
            if let Some(id) = mounted_view_id_from_source(source) {
                return Some(id);
            }
        }
        Expr::Path(source) => {
            if let Some(id) = mounted_view_id_from_source(source.as_label()) {
                return Some(id);
            }
        }
        Expr::ShortVariant(source) => {
            if let Some(id) = mounted_view_id_from_source(source.as_str()) {
                return Some(id);
            }
        }
        _ => {}
    }
    let Expr::Call(call) = expr else {
        return None;
    };
    let callee = match call.callee() {
        Expr::Path(callee) => callee.as_label(),
        Expr::Raw(callee) => callee.as_str(),
        _ => return None,
    };
    (callee == "view").then(|| {
        call.args().iter().find_map(|arg| match arg {
            CallArg::Positional(Expr::EntityRef(reference)) => Some(reference.canonical_body()),
            CallArg::Named { .. } | CallArg::Spread { .. } | CallArg::Positional(_) => None,
        })
    })?
}

fn mounted_view_id_from_source(source: &str) -> Option<String> {
    let rest = source.trim().strip_prefix("view")?;
    if rest
        .chars()
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return None;
    }
    let rest = rest.trim();
    let arg = rest
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(rest)
        .split(',')
        .next()?
        .trim();
    let arg = arg
        .strip_prefix("@view:.")
        .map(|suffix| format!("view.{suffix}"))
        .or_else(|| arg.strip_prefix("@view:").map(str::to_owned))
        .or_else(|| {
            arg.strip_prefix("@view.")
                .map(|suffix| format!("view.{suffix}"))
        })
        .or_else(|| arg.strip_prefix('@').map(str::to_owned))
        .unwrap_or_else(|| arg.to_owned());
    (!arg.is_empty()).then_some(arg)
}
