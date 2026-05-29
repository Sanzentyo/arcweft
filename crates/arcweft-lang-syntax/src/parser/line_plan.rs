use std::borrow::Cow;

use crate::ast::common::TextRange;
use crate::ast::flow::{Stmt, ThreadBlock, ThreadModifier};
use crate::ast::items::RawSyntax;
use crate::ast::line_plan::{
    BlockStyle, CancelRuleSyntax, DeferOutcome, LinePlan, LinePlanItem, TriggerPattern,
};
use crate::cst::{
    find_matching_punctuation, find_top_level_punctuation, parse_flat_fence,
    split_top_level_punctuation, split_top_level_punctuation_once,
    split_top_level_punctuation_sequence_once,
};
use crate::expr::{Expr, parse_expr};
use crate::pattern::parse_pattern;

use super::headers::simple_error;
use super::{
    ParseError, collect_logical_block_items, indentation, parse_expr_lossy, parse_named_block_expr,
    parse_stmt, parse_stmt_lines, split_brace_item, split_top_level_binding,
};

pub(super) fn parse_trigger_pattern(source: &str) -> TriggerPattern {
    let source = source.trim();
    if let Some(trigger) = parse_trigger_call(source) {
        return trigger;
    }
    TriggerPattern::Expr(parse_expr_lossy(source))
}

fn parse_trigger_call(source: &str) -> Option<TriggerPattern> {
    let open = find_top_level_punctuation(source, '(')?;
    let close = find_matching_punctuation(source, open, '(', ')')?;
    if !source[close + ')'.len_utf8()..].trim().is_empty() {
        return None;
    }
    let name = source[..open].trim();
    let args = split_top_level_punctuation(&source[open + '('.len_utf8()..close], ',');
    match name {
        "input" => single_pattern(&args).map(TriggerPattern::Input),
        "event" | "item" | "error" => single_pattern(&args).map(TriggerPattern::Event),
        "mark" => single_pattern(&args).map(TriggerPattern::Mark),
        "select" => single_pattern(&args).map(TriggerPattern::Select),
        "task" => single_pattern(&args).map(TriggerPattern::Task),
        "scope" => single_pattern(&args).map(TriggerPattern::Scope),
        "timeout" => single_expr(&args).map(TriggerPattern::Timeout),
        "signal" => {
            let mut args = args;
            let target = args.first().map(|arg| parse_expr_lossy(arg.trim()))?;
            let value = (args.len() > 1).then(|| {
                let rest = args.drain(1..).collect::<Vec<_>>().join(", ");
                parse_pattern(rest.trim())
            });
            Some(TriggerPattern::Signal { target, value })
        }
        "disconnected" if args.is_empty() => {
            Some(TriggerPattern::Event(parse_pattern("disconnected")))
        }
        _ => None,
    }
}

fn single_pattern(args: &[&str]) -> Option<crate::ast::pattern::Pattern> {
    let [arg] = args else {
        return None;
    };
    Some(parse_pattern(arg.trim()))
}

fn single_expr(args: &[&str]) -> Option<Expr> {
    let [arg] = args else {
        return None;
    };
    Some(parse_expr_lossy(arg.trim()))
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

pub(super) fn parse_line_plan_body(
    style: BlockStyle,
    body: &str,
    range: TextRange,
    errors: &mut Vec<ParseError>,
) -> LinePlan {
    let normalized_body = normalize_line_plan_flat_blocks(body, range.start(), errors);
    let lines = collect_logical_block_items(&normalized_body);
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

fn normalize_line_plan_flat_blocks<'a>(
    source: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Cow<'a, str> {
    if !source
        .lines()
        .any(|line| parse_flat_fence(line.trim()).is_some())
    {
        return Cow::Borrowed(source);
    }
    if !line_plan_flat_fences_are_well_formed(source, base, errors) {
        return Cow::Borrowed(source);
    }
    let lines = source.lines().collect::<Vec<_>>();
    let mut index = 0;
    Cow::Owned(flat_fence_lines_to_brace_blocks(&lines, &mut index).join("\n"))
}

fn line_plan_flat_fences_are_well_formed(
    source: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> bool {
    let mut stack: Vec<OpenFlatFence> = Vec::new();
    let mut well_formed = true;
    let mut offset = 0;
    for line in source.lines() {
        let line_start = base + offset;
        offset += line.len() + '\n'.len_utf8();
        let leading = line.len() - line.trim_start().len();
        let trimmed = line.trim();
        let Some(fence) = parse_flat_fence(line.trim()) else {
            continue;
        };
        if !line_plan_flat_fence_kind_is_supported(fence.kind) {
            errors.push(simple_error(
                line_start + leading,
                trimmed.len(),
                "unknown flat fence kind",
                "=== init ===",
            ));
            well_formed = false;
            continue;
        }
        if fence.close {
            let Some(open) = stack.pop() else {
                errors.push(simple_error(
                    line_start + leading,
                    trimmed.len(),
                    "flat fence close mismatch; no matching open fence",
                    &format!("=== {} ===", fence.kind),
                ));
                well_formed = false;
                continue;
            };
            if open.kind != fence.kind {
                errors.push(simple_error(
                    line_start + leading,
                    trimmed.len(),
                    &format!(
                        "flat fence close mismatch; expected `=== /{} ===`",
                        open.kind
                    ),
                    &format!("=== /{} ===", open.kind),
                ));
                well_formed = false;
            }
        } else {
            stack.push(OpenFlatFence {
                kind: fence.kind.to_owned(),
                start: line_start + leading,
                len: trimmed.len(),
            });
        }
    }
    for open in stack {
        errors.push(simple_error(
            open.start,
            open.len,
            &format!("missing close fence `=== /{} ===`", open.kind),
            &format!("=== /{} ===", open.kind),
        ));
        well_formed = false;
    }
    well_formed
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OpenFlatFence {
    kind: String,
    start: usize,
    len: usize,
}

fn line_plan_flat_fence_kind_is_supported(kind: &str) -> bool {
    matches!(
        kind,
        "init" | "thread" | "on" | "cancel" | "defer" | "start" | "together" | "scope"
    )
}

fn flat_fence_lines_to_brace_blocks(lines: &[&str], index: &mut usize) -> Vec<String> {
    let mut output = Vec::new();
    while *index < lines.len() {
        let line = lines[*index];
        let Some(fence) = parse_flat_fence(line.trim()) else {
            output.push(line.to_owned());
            *index += 1;
            continue;
        };
        *index += 1;
        if fence.close {
            return output;
        }
        let head = if fence.head.is_empty() {
            fence.kind.to_owned()
        } else {
            format!("{} {}", fence.kind, fence.head)
        };
        output.push(format!("{head} {{"));
        output.extend(flat_fence_lines_to_brace_blocks(lines, index));
        output.push("}".to_owned());
    }
    output
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
        || head.starts_with("cancel on ")
        || head == "start"
        || head == "together"
        || head.starts_with("scope"))
    .then_some(head)
}

fn parse_line_plan_colon_item(head: &str, body: &str) -> LinePlanItem {
    if let Some(item) = parse_line_plan_block_item(head, body) {
        return item;
    }
    let source = format!("{head}:\n{body}");
    LinePlanItem::Raw(RawSyntax::line_plan_item(
        source,
        Some(TextRange::new(0, head.len() + body.len() + 2)),
    ))
}

fn parse_line_plan_item(line: &str) -> LinePlanItem {
    if let Some((head, body)) = split_brace_item(line) {
        if let Some(item) = parse_line_plan_block_item(head, body) {
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
    if line.starts_with("memo ") {
        return LinePlanItem::Raw(RawSyntax::line_plan_item(
            line,
            Some(TextRange::new(0, line.len())),
        ));
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
    if line.starts_with("wait(") {
        return true;
    }
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

fn parse_line_plan_block_item(head: &str, body: &str) -> Option<LinePlanItem> {
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
    if let Some(rest) = head.strip_prefix("cancel on ") {
        return Some(LinePlanItem::CancelRule(CancelRuleSyntax::new(
            parse_trigger_pattern(rest.trim()),
            parse_stmt_lines(body),
        )));
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

fn parse_line_plan_nested_items(source: &str) -> Vec<LinePlanItem> {
    let body = source
        .trim()
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or_else(|| source.trim());
    let mut errors = Vec::new();
    parse_line_plan_body(
        BlockStyle::Brace,
        body,
        TextRange::new(0, body.len()),
        &mut errors,
    )
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

fn normalize_timed_cue_body(source: &str) -> &str {
    source
        .trim_start_matches([':', ' ', '{'])
        .trim_end_matches('}')
        .trim()
}
