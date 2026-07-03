use arcweft_lang_syntax::{
    ast::{
        dialogue::DialogueContent,
        flow::{FlowItem, Stmt},
        items::Item,
    },
    expr::Expr,
    source::ParsedSource,
};
use std::ops::Range;

use crate::dialogue_sugar::dialogue_content_source_base;

pub(crate) fn collect_dialogue_content_ranges(
    source: &str,
    parsed: &ParsedSource,
) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    for item in parsed.typed_tree().items() {
        collect_dialogue_content_ranges_from_item(source, item, &mut ranges);
    }
    ranges
}

fn collect_dialogue_content_ranges_from_item(
    source: &str,
    item: &Item,
    ranges: &mut Vec<Range<usize>>,
) {
    match item {
        Item::Flow(flow) => {
            for item in flow.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        Item::FlowItem(item) => {
            collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
        }
        _ => {}
    }
}

fn collect_dialogue_content_ranges_from_flow_item(
    source: &str,
    item: &FlowItem,
    ranges: &mut Vec<Range<usize>>,
) {
    match item {
        FlowItem::SpeakerLine(line) => push_dialogue_content_range(source, line.content(), ranges),
        FlowItem::ContentCall(call) => push_dialogue_content_range(source, call.content(), ranges),
        FlowItem::Scope(scope) => {
            for item in scope.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::If(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::IfLet(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::Match(block) => {
            for arm in block.arms() {
                for item in arm.body() {
                    collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
                }
            }
        }
        FlowItem::Loop(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::While(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::WhileLet(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::For(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::Select(block) => {
            for branch in block.branches() {
                for item in branch.body() {
                    collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
                }
            }
        }
        FlowItem::BorrowBlock(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::SourceLocale(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::AwaitWith(await_with) => {
            for branch in await_with.branches() {
                for item in branch.body() {
                    collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
                }
            }
        }
        FlowItem::Stmt(stmt) => collect_dialogue_content_ranges_from_stmt(source, stmt, ranges),
        FlowItem::Choice(_) | FlowItem::Include(_) | FlowItem::Raw(_) => {}
    }
}

fn collect_dialogue_content_ranges_from_stmt(
    source: &str,
    stmt: &Stmt,
    ranges: &mut Vec<Range<usize>>,
) {
    match stmt {
        Stmt::Let {
            expr,
            expr_source,
            expr_range,
            ..
        } => collect_dialogue_content_ranges_from_expr(
            expr,
            expr_source.as_deref(),
            expr_range.as_ref(),
            ranges,
        ),
        Stmt::LetElse { else_body, .. } => {
            for stmt in else_body {
                collect_dialogue_content_ranges_from_stmt(source, stmt, ranges);
            }
        }
        Stmt::Assign { target, expr } => {
            collect_dialogue_content_ranges_from_expr(target, None, None, ranges);
            collect_dialogue_content_ranges_from_expr(expr, None, None, ranges);
        }
        Stmt::LetScope { scope, .. } => {
            for stmt in scope.statements() {
                collect_dialogue_content_ranges_from_stmt(source, stmt, ranges);
            }
        }
        Stmt::LetLoop { block, .. } => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        Stmt::LetAwait { await_with, .. } => {
            for branch in await_with.branches() {
                for item in branch.body() {
                    collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
                }
            }
        }
        Stmt::Thread(thread) => {
            for item in thread.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        Stmt::DeferBlock { statements, .. }
        | Stmt::On {
            body: statements, ..
        }
        | Stmt::UnsafeLifetime {
            body: statements, ..
        }
        | Stmt::Loop {
            body: statements, ..
        }
        | Stmt::While {
            body: statements, ..
        }
        | Stmt::WhileLet {
            body: statements, ..
        }
        | Stmt::For {
            body: statements, ..
        } => {
            for stmt in statements {
                collect_dialogue_content_ranges_from_stmt(source, stmt, ranges);
            }
        }
        Stmt::If {
            body, else_body, ..
        } => {
            for stmt in body {
                collect_dialogue_content_ranges_from_stmt(source, stmt, ranges);
            }
            for stmt in else_body {
                collect_dialogue_content_ranges_from_stmt(source, stmt, ranges);
            }
        }
        Stmt::Match { arms, .. } => {
            for arm in arms {
                for stmt in arm.body() {
                    collect_dialogue_content_ranges_from_stmt(source, stmt, ranges);
                }
            }
        }
        Stmt::LetChoice { .. }
        | Stmt::Return(_)
        | Stmt::Out { .. }
        | Stmt::Goto(_)
        | Stmt::Defer { .. }
        | Stmt::Yield(_)
        | Stmt::Signal { .. }
        | Stmt::LifetimeSet { .. }
        | Stmt::Wait(_)
        | Stmt::Close(_)
        | Stmt::Select(_)
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Expr(_)
        | Stmt::Raw(_) => {}
    }
}

fn collect_dialogue_content_ranges_from_expr(
    expr: &Expr,
    expr_source: Option<&str>,
    expr_range: Option<&arcweft_lang_syntax::ast::common::TextRange>,
    ranges: &mut Vec<Range<usize>>,
) {
    match expr {
        Expr::DialogueCall { content, .. } => {
            let (Some(expr_source), Some(expr_range)) = (expr_source, expr_range) else {
                return;
            };
            let Some(content_start) = expr_source.find(content) else {
                return;
            };
            let start = expr_range.start() + content_start;
            ranges.push(start..start + content.len());
        }
        Expr::Try { expr } => {
            collect_dialogue_content_ranges_from_expr(expr, expr_source, expr_range, ranges);
        }
        _ => {}
    }
}

fn push_dialogue_content_range(
    source: &str,
    content: &DialogueContent,
    ranges: &mut Vec<Range<usize>>,
) {
    let Some(base) = dialogue_content_source_base(source, content) else {
        return;
    };
    ranges.push(base..base + content.raw().len());
}
