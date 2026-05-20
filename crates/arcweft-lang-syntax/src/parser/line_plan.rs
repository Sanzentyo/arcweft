use crate::ast::common::TextRange;
use crate::ast::flow::{Stmt, ThreadBlock, ThreadModifier};
use crate::ast::items::RawSyntax;
use crate::ast::line_plan::{
    BlockStyle, CancelRuleSyntax, DeferOutcome, LinePlan, LinePlanItem, TriggerPattern,
};
use crate::cst::{
    find_matching_punctuation, find_top_level_punctuation, split_top_level_punctuation_once,
    split_top_level_punctuation_sequence_once, split_top_level_whitespace,
};
use crate::expr::{Expr, parse_expr};
use crate::pattern::parse_pattern;

use super::{
    collect_logical_block_items, indentation, parse_expr_lossy, parse_named_block_expr, parse_stmt,
    parse_stmt_lines, split_brace_item, split_top_level_binding,
};

pub(super) fn parse_trigger_pattern(source: &str) -> TriggerPattern {
    let source = source.trim();
    if let Some(rest) = source.strip_prefix("input ") {
        return TriggerPattern::Input(parse_pattern(rest.trim()));
    }
    if let Some(rest) = source.strip_prefix("event ") {
        return TriggerPattern::Event(parse_pattern(rest.trim()));
    }
    if let Some(rest) = source.strip_prefix("item ") {
        return TriggerPattern::Event(parse_pattern(rest.trim()));
    }
    if let Some(rest) = source.strip_prefix("error ") {
        return TriggerPattern::Event(parse_pattern(rest.trim()));
    }
    if source == "disconnected" {
        return TriggerPattern::Event(parse_pattern(source));
    }
    if let Some(rest) = source.strip_prefix("signal ") {
        let mut parts = split_top_level_whitespace(rest);
        let target = parts.first().map_or_else(
            || Expr::Raw(rest.to_owned()),
            |target| parse_expr_lossy(target),
        );
        let value = (parts.len() > 1)
            .then(|| parse_pattern(parts.drain(1..).collect::<Vec<_>>().join(" ").trim()));
        return TriggerPattern::Signal { target, value };
    }
    if let Some(rest) = source.strip_prefix("timeout ") {
        return TriggerPattern::Timeout(parse_expr_lossy(rest.trim()));
    }
    if let Some(rest) = source.strip_prefix("mark ") {
        return TriggerPattern::Mark(parse_pattern(rest.trim()));
    }
    if let Some(rest) = source.strip_prefix("select ") {
        return TriggerPattern::Select(parse_pattern(rest.trim()));
    }
    if let Some(rest) = source.strip_prefix("task ") {
        return TriggerPattern::Task(parse_pattern(rest.trim()));
    }
    if let Some(rest) = source.strip_prefix("scope ") {
        return TriggerPattern::Scope(parse_pattern(rest.trim()));
    }
    if source.starts_with('.') {
        return TriggerPattern::Mark(parse_pattern(source));
    }
    TriggerPattern::Expr(parse_expr_lossy(source))
}

pub(super) fn parse_defer_outcome(head: &str) -> Option<DeferOutcome> {
    let rest = head.trim().strip_prefix("defer")?.trim();
    if rest.is_empty() {
        return Some(DeferOutcome::Always);
    }
    let outcome = rest.strip_prefix("on")?.trim();
    match outcome {
        "completed" => Some(DeferOutcome::Completed),
        "cancelled" => Some(DeferOutcome::Cancelled),
        "failed" => Some(DeferOutcome::Failed),
        _ => None,
    }
}

pub(super) fn parse_line_plan_body(style: BlockStyle, body: &str, range: TextRange) -> LinePlan {
    let lines = collect_logical_block_items(body);
    let mut items = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = &lines[index];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }
        if is_multiline_timed_cue_header(trimmed) {
            let cue_indent = indentation(line);
            let mut body_lines = Vec::new();
            index += 1;
            while index < lines.len() {
                let child = &lines[index];
                let child_trimmed = child.trim();
                if !child_trimmed.is_empty() && indentation(child.as_str()) <= cue_indent {
                    break;
                }
                if !child_trimmed.is_empty() {
                    body_lines.push(child_trimmed);
                }
                index += 1;
            }
            let body = body_lines.join(" ");
            items.push(parse_line_plan_item(&format!("{trimmed} {body}")));
            continue;
        }
        if let Some((pattern, head)) = line_plan_let_colon_head(trimmed) {
            let cue_indent = indentation(line);
            let mut body_lines = Vec::new();
            index += 1;
            while index < lines.len() {
                let child = &lines[index];
                let child_trimmed = child.trim();
                if !child_trimmed.is_empty() && indentation(child.as_str()) <= cue_indent {
                    break;
                }
                if !child_trimmed.is_empty() {
                    body_lines.push(child.as_str());
                }
                index += 1;
            }
            items.push(LinePlanItem::Let {
                pattern: parse_pattern(pattern.trim()),
                expr: parse_named_block_expr(head, &body_lines.join("\n")),
            });
            continue;
        }
        if let Some(head) = line_plan_colon_head(trimmed) {
            let cue_indent = indentation(line);
            let mut body_lines = Vec::new();
            index += 1;
            while index < lines.len() {
                let child = &lines[index];
                let child_trimmed = child.trim();
                if !child_trimmed.is_empty() && indentation(child.as_str()) <= cue_indent {
                    break;
                }
                if !child_trimmed.is_empty() {
                    body_lines.push(child.as_str());
                }
                index += 1;
            }
            items.push(parse_line_plan_colon_item(head, &body_lines.join("\n")));
            continue;
        }
        items.push(parse_line_plan_item(trimmed));
        index += 1;
    }
    LinePlan::new(style, items, range)
}

fn is_multiline_timed_cue_header(line: &str) -> bool {
    line.starts_with("at(") && line.ends_with(':')
}

fn line_plan_let_colon_head(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("let ")?;
    let (pattern, expr) = split_top_level_binding(rest)?;
    let head = expr.trim().strip_suffix(':')?.trim();
    (head.starts_with("at(") || head.starts_with("scope")).then_some((pattern, head))
}

fn line_plan_colon_head(line: &str) -> Option<&str> {
    let head = line.strip_suffix(':')?.trim();
    (head == "init"
        || parse_defer_outcome(head).is_some()
        || head.starts_with("thread")
        || head.starts_with("on ")
        || head.starts_with("scope"))
    .then_some(head)
}

fn parse_line_plan_colon_item(head: &str, body: &str) -> LinePlanItem {
    if head == "init" {
        return LinePlanItem::Init(parse_stmt_lines(body));
    }
    if let Some(outcome) = parse_defer_outcome(head) {
        return LinePlanItem::Stmt(Stmt::DeferBlock {
            outcome,
            statements: parse_stmt_lines(body),
        });
    }
    if let Some(rest) = head.strip_prefix("thread") {
        return LinePlanItem::Thread(parse_thread_block(&format!("thread{rest}"), body));
    }
    if let Some(rest) = head.strip_prefix("on ") {
        return LinePlanItem::On {
            trigger: parse_trigger_pattern(rest.trim()),
            body: parse_stmt_lines(body),
        };
    }
    if head.starts_with("scope") {
        return LinePlanItem::Stmt(Stmt::Expr(parse_named_block_expr(head, body)));
    }
    let source = format!("{head}:\n{body}");
    LinePlanItem::Raw(RawSyntax::line_plan_item(
        source,
        Some(TextRange::new(0, head.len() + body.len() + 2)),
    ))
}

fn parse_line_plan_item(line: &str) -> LinePlanItem {
    if let Some((head, body)) = split_brace_item(line) {
        if let Some(item) = parse_line_plan_braced_item(head, body) {
            return item;
        }
    }
    if let Some(rest) = line.strip_prefix("out ") {
        return LinePlanItem::Out(parse_expr_lossy(rest.trim()));
    }
    if let Some(rest) = line.strip_prefix("let ") {
        if let Some((pattern, expr)) = split_top_level_binding(rest) {
            return LinePlanItem::Let {
                pattern: parse_pattern(pattern.trim()),
                expr: parse_expr_lossy(expr.trim()),
            };
        }
    }
    if let Some(rest) = line.strip_prefix("defer ") {
        if rest.trim_start().starts_with("on ") {
            return LinePlanItem::Raw(RawSyntax::line_plan_item(
                line,
                Some(TextRange::new(0, line.len())),
            ));
        }
        return LinePlanItem::Stmt(Stmt::Defer {
            outcome: DeferOutcome::Always,
            expr: parse_expr_lossy(rest.trim()),
        });
    }
    if let Some(rest) = line.strip_prefix("cancel on ") {
        if let Some((head, body)) = split_brace_item(line) {
            if let Some(trigger) = head.strip_prefix("cancel on ") {
                return LinePlanItem::CancelRule(CancelRuleSyntax::new(
                    parse_trigger_pattern(trigger.trim()),
                    parse_stmt_lines(body.trim()),
                ));
            }
        }
        let (trigger, action) =
            split_top_level_punctuation_sequence_once(rest, &["=", ">"]).unwrap_or((rest, ""));
        return LinePlanItem::CancelRule(CancelRuleSyntax::new(
            parse_trigger_pattern(trigger.trim()),
            parse_line_plan_cancel_action(action.trim()),
        ));
    }
    if let Some(rest) = line.strip_prefix("on ")
        && let Some((trigger, body)) = rest.split_once("=>")
    {
        return LinePlanItem::On {
            trigger: parse_trigger_pattern(trigger.trim()),
            body: vec![Stmt::Expr(parse_expr_lossy(body.trim()))],
        };
    }
    if line.starts_with("at(")
        && let Some(open) = find_top_level_punctuation(line, '(')
        && let Some(close) = find_matching_punctuation(line, open, '(', ')')
    {
        let anchor = &line[open + '('.len_utf8()..close];
        let body = &line[close + ')'.len_utf8()..];
        if body.trim_start().starts_with('[') {
            return LinePlanItem::Raw(RawSyntax::line_plan_item(
                line,
                Some(TextRange::new(0, line.len())),
            ));
        }
        return LinePlanItem::TimedCue {
            anchor: parse_expr_lossy(anchor.trim()),
            body: parse_expr_lossy(normalize_timed_cue_body(body)),
        };
    }
    if let Some(rest) = line.strip_prefix("start ") {
        return LinePlanItem::StartGroup(parse_line_plan_nested_items(rest.trim()));
    }
    if let Some(rest) = line.strip_prefix("together ") {
        return LinePlanItem::TogetherGroup(parse_line_plan_nested_items(rest.trim()));
    }
    if let Some(rest) = line.strip_prefix("memo ") {
        return parse_line_plan_memo(rest.trim());
    }
    if is_line_plan_statement(line) {
        return LinePlanItem::Stmt(parse_stmt(line));
    }
    if let Some((name, value)) = split_top_level_punctuation_once(line, '=') {
        return LinePlanItem::Option {
            name: name.trim().to_owned(),
            value: parse_expr_lossy(value.trim()),
        };
    }
    if let Ok(expr) = parse_expr(line) {
        if let Some(assertion) = parse_assert_call(&expr) {
            return assertion;
        }
        return LinePlanItem::Expr(expr);
    }
    LinePlanItem::Raw(RawSyntax::line_plan_item(
        line,
        Some(TextRange::new(0, line.len())),
    ))
}

fn is_line_plan_statement(line: &str) -> bool {
    matches!(
        line.split_whitespace().next(),
        Some(
            "signal"
                | "wait"
                | "return"
                | "goto"
                | "yield"
                | "close"
                | "select"
                | "break"
                | "continue"
        )
    )
}

fn parse_line_plan_braced_item(head: &str, body: &str) -> Option<LinePlanItem> {
    if head == "init" {
        return Some(LinePlanItem::Init(parse_stmt_lines(body)));
    }
    if let Some(outcome) = parse_defer_outcome(head) {
        return Some(LinePlanItem::Stmt(Stmt::DeferBlock {
            outcome,
            statements: parse_stmt_lines(body),
        }));
    }
    if let Some(rest) = head.strip_prefix("thread") {
        return Some(LinePlanItem::Thread(parse_thread_block(
            &format!("thread{rest}"),
            body,
        )));
    }
    if let Some(rest) = head.strip_prefix("on ") {
        return Some(LinePlanItem::On {
            trigger: parse_trigger_pattern(rest.trim()),
            body: parse_stmt_lines(body),
        });
    }
    if head == "start" {
        return Some(LinePlanItem::StartGroup(parse_line_plan_nested_items(body)));
    }
    if head == "together" {
        return Some(LinePlanItem::TogetherGroup(parse_line_plan_nested_items(
            body,
        )));
    }
    if head.starts_with("scope") {
        return Some(LinePlanItem::Stmt(Stmt::Expr(parse_named_block_expr(
            head, body,
        ))));
    }
    None
}

fn parse_assert_call(expr: &Expr) -> Option<LinePlanItem> {
    let Expr::Call { callee, args } = expr else {
        return None;
    };
    let Expr::Path(name) = callee.as_ref() else {
        return None;
    };
    let debug = match name.as_str() {
        "assert" => false,
        "debug_assert" => true,
        _ => return None,
    };
    let [condition] = args.as_slice() else {
        return None;
    };
    Some(LinePlanItem::Assert {
        debug,
        expr: condition.clone(),
    })
}

pub(super) fn parse_thread_block(head: &str, body: &str) -> ThreadBlock {
    let rest = head.trim().strip_prefix("thread").unwrap_or(head).trim();
    let mut modifiers = Vec::new();
    let mut parts = rest.split_whitespace().collect::<Vec<_>>();
    if matches!(parts.first(), Some(&"detached")) {
        modifiers.push(ThreadModifier::Detached);
        parts.remove(0);
    }
    let name = nonempty_string(&parts.join(" "));
    ThreadBlock::new(modifiers, name, parse_stmt_lines(body))
}

pub(super) fn nonempty_string(source: &str) -> Option<String> {
    (!source.is_empty()).then(|| source.to_owned())
}

fn parse_line_plan_memo(source: &str) -> LinePlanItem {
    let mut parts = split_scenario_args(source);
    if parts.is_empty() {
        return LinePlanItem::Raw(RawSyntax::line_plan_item(
            "memo",
            Some(TextRange::new(0, "memo".len())),
        ));
    }
    let name = parts.remove(0).to_owned();
    let options = parts
        .into_iter()
        .filter_map(|part| {
            split_top_level_punctuation_once(part, '=')
                .map(|(name, value)| (name.to_owned(), parse_expr_lossy(value)))
        })
        .collect();
    LinePlanItem::Memo { name, options }
}

fn parse_line_plan_nested_items(source: &str) -> Vec<LinePlanItem> {
    let body = source
        .trim()
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or_else(|| source.trim());
    parse_line_plan_body(BlockStyle::Brace, body, TextRange::new(0, body.len()))
        .items()
        .to_vec()
}

fn parse_line_plan_cancel_action(action: &str) -> Vec<Stmt> {
    if action.is_empty() {
        Vec::new()
    } else {
        parse_stmt_lines(action)
    }
}

fn split_scenario_args(source: &str) -> Vec<&str> {
    split_top_level_whitespace(source)
}

fn normalize_timed_cue_body(source: &str) -> &str {
    source
        .trim_start_matches([':', ' ', '{'])
        .trim_end_matches('}')
        .trim()
}
