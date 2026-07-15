use super::{
    Expr, ExprSourceRange, collect_expr_source_ranges_inner, postfix_delimiter_bounds,
    split_top_level_lines,
};
use crate::ast::common::TextRange;
use crate::ast::flow::{AuthoredExpr, FlowItem, Stmt, WaitTarget};

pub(super) fn collect_thread_expr_source_ranges<'a>(
    body: &'a [FlowItem],
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    let Some((open, close)) = postfix_delimiter_bounds(source, '{', '}') else {
        return;
    };
    let inner = &source[open + '{'.len_utf8()..close];
    let inner_base = base + open + '{'.len_utf8();
    let item_sources = split_top_level_lines(inner, inner_base);
    for (index, item) in body.iter().enumerate() {
        let (item_source, item_base) = item_sources
            .get(index)
            .copied()
            .unwrap_or((inner, inner_base));
        collect_flow_item_source_ranges(item, item_source, item_base, ranges);
    }
}

fn collect_flow_item_source_ranges<'a>(
    item: &'a FlowItem,
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    match item {
        FlowItem::Stmt(stmt) => collect_stmt_source_ranges(stmt, source, base, ranges),
        FlowItem::If(block) => {
            collect_authored_expr_source_ranges(block.condition_authored(), ranges);
            collect_flow_item_list_source_ranges(block.body(), source, base, ranges);
            collect_flow_item_list_source_ranges(block.else_body(), source, base, ranges);
        }
        FlowItem::IfLet(block) => {
            collect_authored_expr_source_ranges(block.expr_authored(), ranges);
            if let Some(guard) = block.guard_authored() {
                collect_authored_expr_source_ranges(guard, ranges);
            }
            collect_flow_item_list_source_ranges(block.body(), source, base, ranges);
            collect_flow_item_list_source_ranges(block.else_body(), source, base, ranges);
        }
        FlowItem::Match(block) => {
            collect_authored_expr_source_ranges(block.expr_authored(), ranges);
            for arm in block.arms() {
                if let Some(guard) = arm.guard_authored() {
                    collect_authored_expr_source_ranges(guard, ranges);
                }
                collect_flow_item_list_source_ranges(arm.body(), source, base, ranges);
            }
        }
        FlowItem::Loop(block) => {
            collect_flow_item_list_source_ranges(block.body(), source, base, ranges);
        }
        FlowItem::While(block) => {
            collect_authored_expr_source_ranges(block.condition_authored(), ranges);
            collect_flow_item_list_source_ranges(block.body(), source, base, ranges);
        }
        FlowItem::WhileLet(block) => {
            collect_authored_expr_source_ranges(block.expr_authored(), ranges);
            if let Some(guard) = block.guard_authored() {
                collect_authored_expr_source_ranges(guard, ranges);
            }
            collect_flow_item_list_source_ranges(block.body(), source, base, ranges);
        }
        FlowItem::For(block) => {
            collect_authored_expr_source_ranges(block.source_authored(), ranges);
            collect_flow_item_list_source_ranges(block.body(), source, base, ranges);
        }
        FlowItem::Select(block) => {
            for branch in block.branches() {
                collect_flow_item_list_source_ranges(branch.body(), source, base, ranges);
            }
        }
        FlowItem::SourceLocale(block) => {
            collect_flow_item_list_source_ranges(block.body(), source, base, ranges);
        }
        FlowItem::Scope(block) => {
            collect_flow_item_list_source_ranges(block.body(), source, base, ranges);
        }
        FlowItem::AwaitWith(await_with) => {
            collect_authored_expr_source_ranges(await_with.expr_authored(), ranges);
            for branch in await_with.branches() {
                collect_flow_item_list_source_ranges(branch.body(), source, base, ranges);
            }
        }
        FlowItem::SpeakerLine(_)
        | FlowItem::ContentCall(_)
        | FlowItem::Choice(_)
        | FlowItem::Include(_)
        | FlowItem::Raw(_) => {}
    }
}

fn collect_flow_item_list_source_ranges<'a>(
    items: &'a [FlowItem],
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    let item_sources = split_top_level_lines(source, base);
    for (index, item) in items.iter().enumerate() {
        let (item_source, item_base) = item_sources.get(index).copied().unwrap_or((source, base));
        collect_flow_item_source_ranges(item, item_source, item_base, ranges);
    }
}

fn collect_stmt_source_ranges<'a>(
    stmt: &'a Stmt,
    fallback_source: &str,
    fallback_base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    match stmt {
        Stmt::Let {
            expr,
            expr_source,
            expr_range,
            ..
        }
        | Stmt::Return {
            expr,
            expr_source,
            expr_range,
        } => {
            collect_optional_source_expr_ranges(expr, expr_source.as_deref(), *expr_range, ranges);
        }
        Stmt::Expr {
            expr,
            expr_source,
            expr_range,
        } => {
            if !collect_optional_source_expr_ranges(
                expr,
                expr_source.as_deref(),
                *expr_range,
                ranges,
            ) {
                collect_expr_source_ranges_inner(expr, fallback_source, fallback_base, ranges);
            }
        }
        Stmt::Assign { target, expr } => {
            collect_authored_expr_source_ranges(target, ranges);
            collect_authored_expr_source_ranges(expr, ranges);
        }
        Stmt::LetActionReceive { action, .. }
        | Stmt::Out { expr: action, .. }
        | Stmt::Goto(action)
        | Stmt::Defer { expr: action, .. }
        | Stmt::Yield(action)
        | Stmt::Close(action)
        | Stmt::Select(action) => collect_authored_expr_source_ranges(action, ranges),
        Stmt::Thread(thread) => {
            collect_flow_item_list_source_ranges(
                thread.body(),
                fallback_source,
                fallback_base,
                ranges,
            );
        }
        Stmt::DeferBlock { statements, .. }
        | Stmt::On {
            body: statements, ..
        }
        | Stmt::UnsafeLifetime {
            body: statements, ..
        }
        | Stmt::Loop { body: statements } => {
            collect_stmt_list_source_ranges(statements, fallback_source, fallback_base, ranges);
        }
        Stmt::Wait(WaitTarget::Duration(expr) | WaitTarget::Expr(expr)) => {
            collect_authored_expr_source_ranges(expr, ranges);
        }
        Stmt::Signal { target, value }
        | Stmt::LifetimeSet {
            target,
            expr: value,
        } => {
            collect_authored_expr_source_ranges(target, ranges);
            collect_authored_expr_source_ranges(value, ranges);
        }
        Stmt::If { .. }
        | Stmt::While { .. }
        | Stmt::WhileLet { .. }
        | Stmt::For { .. }
        | Stmt::Match { .. }
        | Stmt::Break { .. } => {
            collect_branching_stmt_source_ranges(stmt, fallback_source, fallback_base, ranges);
        }
        Stmt::LetElse {
            expr, else_body, ..
        } => {
            collect_authored_expr_source_ranges(expr, ranges);
            collect_stmt_list_source_ranges(else_body, fallback_source, fallback_base, ranges);
        }
        Stmt::LetChoice { .. }
        | Stmt::LetScope { .. }
        | Stmt::LetLoop { .. }
        | Stmt::LetAwait { .. }
        | Stmt::Continue { .. }
        | Stmt::Raw(_) => {}
    }
}

fn collect_branching_stmt_source_ranges<'a>(
    stmt: &'a Stmt,
    fallback_source: &str,
    fallback_base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    match stmt {
        Stmt::If {
            condition,
            body,
            else_body,
        } => {
            collect_authored_expr_source_ranges(condition, ranges);
            collect_stmt_list_source_ranges(body, fallback_source, fallback_base, ranges);
            collect_stmt_list_source_ranges(else_body, fallback_source, fallback_base, ranges);
        }
        Stmt::While { condition, body } => {
            collect_authored_expr_source_ranges(condition, ranges);
            collect_stmt_list_source_ranges(body, fallback_source, fallback_base, ranges);
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            collect_authored_expr_source_ranges(expr, ranges);
            if let Some(guard) = guard {
                collect_authored_expr_source_ranges(guard, ranges);
            }
            collect_stmt_list_source_ranges(body, fallback_source, fallback_base, ranges);
        }
        Stmt::For { source, body, .. } => {
            collect_authored_expr_source_ranges(source, ranges);
            collect_stmt_list_source_ranges(body, fallback_source, fallback_base, ranges);
        }
        Stmt::Match { expr, arms } => {
            collect_authored_expr_source_ranges(expr, ranges);
            for arm in arms {
                if let Some(guard) = arm.guard_authored() {
                    collect_authored_expr_source_ranges(guard, ranges);
                }
                collect_stmt_list_source_ranges(arm.body(), fallback_source, fallback_base, ranges);
            }
        }
        Stmt::Break {
            expr: Some(expr), ..
        } => collect_authored_expr_source_ranges(expr, ranges),
        _ => {}
    }
}

fn collect_stmt_list_source_ranges<'a>(
    stmts: &'a [Stmt],
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    let stmt_sources = split_top_level_lines(source, base);
    for (index, stmt) in stmts.iter().enumerate() {
        let (stmt_source, stmt_base) = stmt_sources.get(index).copied().unwrap_or((source, base));
        collect_stmt_source_ranges(stmt, stmt_source, stmt_base, ranges);
    }
}

fn collect_authored_expr_source_ranges<'a>(
    authored: &'a AuthoredExpr,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    collect_optional_source_expr_ranges(
        authored.expr(),
        authored.source(),
        authored.range(),
        ranges,
    );
}

fn collect_optional_source_expr_ranges<'a>(
    expr: &'a Expr,
    source: Option<&str>,
    range: Option<TextRange>,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) -> bool {
    if let (Some(source), Some(range)) = (source, range) {
        collect_expr_source_ranges_inner(expr, source, range.start(), ranges);
        true
    } else {
        false
    }
}
