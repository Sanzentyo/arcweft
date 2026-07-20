use super::{CallArg, Expr, ExprOp, MatchExprArm};
use crate::ast::common::TextRange;
use crate::ast::dialogue::{DialogueContent, DialogueToken};
use crate::ast::line_plan::{LinePlan, LinePlanItem};

mod scan;
mod thread_body;

use scan::{
    absolute_source_slice, braced_block_inner, delimited_inner, find_binary_operator,
    find_last_top_level_char, find_last_top_level_operator, find_top_level_char,
    find_top_level_keyword, find_top_level_operator, matching_delimiter_end,
    postfix_delimiter_bounds, push_trimmed_segment, split_top_level_lines,
    split_top_level_segments, trim_source_with_base,
};

/// Source range for one parsed expression node.
///
/// Ranges are reported in original-source byte offsets when the caller passes
/// the source slice's original range to [`collect_expr_source_ranges`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExprSourceRange<'a> {
    expr: &'a Expr,
    range: TextRange,
}

impl<'a> ExprSourceRange<'a> {
    /// Expression node covered by this source range.
    pub const fn expr(&self) -> &'a Expr {
        self.expr
    }

    /// Half-open byte range for the expression node.
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Collects best-effort structural source ranges for an expression subtree.
///
/// This is intentionally owned by the syntax crate because it needs to mirror
/// expression grammar boundaries such as postfix calls, selectors, pipes, and
/// argument lists. It does not resolve names or types.
pub fn collect_expr_source_ranges<'a>(
    expr: &'a Expr,
    source: &str,
    source_range: TextRange,
) -> Vec<ExprSourceRange<'a>> {
    let mut ranges = Vec::new();
    collect_expr_source_ranges_inner(expr, source, source_range.start(), &mut ranges);
    ranges
}

/// Collects the authored document ranges of dialogue-call content bodies.
///
/// `source` must be the original source slice covered by `source_range`. The
/// returned ranges exclude the outer `[` and `]` and surrounding whitespace.
/// Delimiters are resolved structurally, so content text repeated in the
/// callee cannot be mistaken for the dialogue body.
pub fn collect_dialogue_call_content_ranges(
    expr: &Expr,
    source: &str,
    source_range: TextRange,
) -> Vec<TextRange> {
    collect_expr_source_ranges(expr, source, source_range)
        .into_iter()
        .filter_map(|entry| {
            let Expr::DialogueCall { content, .. } = entry.expr() else {
                return None;
            };
            let start = entry.range().start().checked_sub(source_range.start())?;
            let end = entry.range().end().checked_sub(source_range.start())?;
            let dialogue_source = source.get(start..end)?;
            let (content_source, content_base) =
                dialogue_call_content_source(dialogue_source, entry.range().start())?;
            authored_dialogue_content_matches(content_source, content.raw())
                .then(|| TextRange::new(content_base, content_base + content_source.len()))
        })
        .collect()
}

fn authored_dialogue_content_matches(source: &str, parsed: &str) -> bool {
    source == parsed || source.replace("\r\n", "\n") == parsed
}

fn dialogue_call_content_source(source: &str, base: usize) -> Option<(&str, usize)> {
    let (source, base) = trim_source_with_base(source, base);
    let content_open = find_top_level_char(source, '[')?;
    let content_close = matching_delimiter_end(source, content_open, '[', ']')?;
    let (content, content_base) = trim_source_with_base(
        source.get(content_open + '['.len_utf8()..content_close - ']'.len_utf8())?,
        base + content_open + '['.len_utf8(),
    );
    Some((content, content_base))
}

fn collect_expr_source_ranges_inner<'a>(
    expr: &'a Expr,
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    let (source, base) = trim_source_with_base(source, base);
    if source.is_empty() {
        return;
    }
    ranges.push(ExprSourceRange {
        expr,
        range: TextRange::new(base, base + source.len()),
    });
    if collect_container_expr_source_ranges(expr, source, base, ranges) {
        return;
    }
    if collect_operator_expr_source_ranges(expr, source, base, ranges) {
        return;
    }
    collect_control_expr_source_ranges(expr, source, base, ranges);
}

fn collect_container_expr_source_ranges<'a>(
    expr: &'a Expr,
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) -> bool {
    match expr {
        Expr::Tuple(items) => {
            collect_delimited_items(items, source, base, '(', ')', ranges);
            true
        }
        Expr::BracketSeq(items) => {
            collect_delimited_items(items, source, base, '[', ']', ranges);
            true
        }
        Expr::ArrayRepeat { value, len } => {
            if let Some((inner, inner_base)) = delimited_inner(source, base, '[', ']')
                && let Some(split) = find_top_level_char(inner, ';')
            {
                collect_expr_source_ranges_inner(value, &inner[..split], inner_base, ranges);
                collect_expr_source_ranges_inner(
                    len,
                    &inner[split + 1..],
                    inner_base + split + 1,
                    ranges,
                );
            }
            true
        }
        Expr::Call(call) => {
            collect_call_source_ranges(call, source, base, ranges);
            true
        }
        Expr::Select(select) => {
            if let Some(dot) = find_last_top_level_char(source, '.') {
                collect_expr_source_ranges_inner(select.target(), &source[..dot], base, ranges);
            }
            true
        }
        Expr::DialogueCall {
            callee,
            content,
            plan,
        } => {
            collect_dialogue_call_source_ranges(
                callee,
                content,
                plan.as_ref(),
                source,
                base,
                ranges,
            );
            true
        }
        Expr::Index { target, index } => {
            if let Some((open, close)) = postfix_delimiter_bounds(source, '[', ']') {
                collect_expr_source_ranges_inner(target, &source[..open], base, ranges);
                collect_expr_source_ranges_inner(
                    index,
                    &source[open + 1..close],
                    base + open + 1,
                    ranges,
                );
            }
            true
        }
        Expr::Record { fields, .. } => {
            collect_record_field_ranges(fields, source, base, ranges);
            true
        }
        Expr::RecordLiteral(fields) => {
            if let Some((inner, inner_base)) = delimited_inner(source, base, '{', '}') {
                collect_field_value_ranges(fields, inner, inner_base, ranges);
            }
            true
        }
        _ => false,
    }
}

fn collect_call_source_ranges<'a>(
    call: &'a super::CallExpr,
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    if let Some(callee_source) = absolute_source_slice(source, base, call.callee_range()) {
        collect_expr_source_ranges_inner(
            call.callee(),
            callee_source,
            call.callee_range().start(),
            ranges,
        );
    }
    if let Some(parenthesized) = call.parenthesized_syntax() {
        for (arg, syntax) in call
            .args()
            .iter()
            .zip(parenthesized.argument_list().arguments())
        {
            if let Some(value_source) = absolute_source_slice(source, base, syntax.value_range()) {
                collect_expr_source_ranges_inner(
                    arg.value(),
                    value_source,
                    syntax.value_range().start(),
                    ranges,
                );
            }
        }
        return;
    }
    let Some(callback) = call.callback_block_syntax() else {
        return;
    };
    let [CallArg::Positional(closure @ Expr::Closure { body, .. })] = call.args() else {
        return;
    };
    ranges.push(ExprSourceRange {
        expr: closure,
        range: callback.callback().closure_range(),
    });
    if let Some(body_source) = absolute_source_slice(source, base, callback.callback().body_range())
    {
        thread_body::collect_callback_body_expr_source_ranges(
            body,
            body_source,
            callback.callback().body_range().start(),
            ranges,
        );
    }
}

fn collect_dialogue_call_source_ranges<'a>(
    callee: &'a Expr,
    content: &'a DialogueContent,
    plan: Option<&'a LinePlan>,
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    let Some(parts) = dialogue_call_source_parts(source, base) else {
        return;
    };
    collect_expr_source_ranges_inner(callee, parts.callee_source, parts.callee_base, ranges);
    collect_dialogue_content_source_ranges(
        content,
        parts.content_source,
        parts.content_base,
        ranges,
    );
    if let (Some(plan), Some((body_source, body_base))) = (plan, parts.plan_body) {
        collect_line_plan_source_ranges(plan, body_source, body_base, ranges);
    }
}

struct DialogueCallSourceParts<'a> {
    callee_source: &'a str,
    callee_base: usize,
    content_source: &'a str,
    content_base: usize,
    plan_body: Option<(&'a str, usize)>,
}

fn dialogue_call_source_parts(source: &str, base: usize) -> Option<DialogueCallSourceParts<'_>> {
    let (source, base) = trim_source_with_base(source, base);
    let content_open = find_top_level_char(source, '[')?;
    let content_close = matching_delimiter_end(source, content_open, '[', ']')?;
    let (content_source, content_base) = trim_source_with_base(
        source.get(content_open + '['.len_utf8()..content_close - ']'.len_utf8())?,
        base + content_open + '['.len_utf8(),
    );
    let plan_source = source
        .get(content_close..)
        .and_then(|source| line_plan_body_source(source, base + content_close));
    Some(DialogueCallSourceParts {
        callee_source: &source[..content_open],
        callee_base: base,
        content_source,
        content_base,
        plan_body: plan_source,
    })
}

fn collect_dialogue_content_source_ranges<'a>(
    content: &'a DialogueContent,
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    if !authored_dialogue_content_matches(source, content.raw()) {
        return;
    }
    for token in content.tokens() {
        let DialogueToken::Expr(expr) = token else {
            continue;
        };
        let range = expr.range().as_range();
        let Some(expr_source) = source.get(range.clone()) else {
            continue;
        };
        collect_expr_source_ranges_inner(expr.expr(), expr_source, base + range.start, ranges);
    }
}

fn line_plan_body_source(source: &str, base: usize) -> Option<(&str, usize)> {
    let (source, base) = trim_source_with_base(source, base);
    let rest = source.strip_prefix("with")?;
    let (rest, rest_base) = trim_source_with_base(rest, base + "with".len());
    if let Some(open) = find_top_level_char(rest, '{') {
        let close = matching_delimiter_end(rest, open, '{', '}')?;
        return Some((
            &rest[open + '{'.len_utf8()..close - '}'.len_utf8()],
            rest_base + open + '{'.len_utf8(),
        ));
    }
    let colon = find_top_level_char(rest, ':')?;
    Some(trim_source_with_base(
        &rest[colon + ':'.len_utf8()..],
        rest_base + colon + ':'.len_utf8(),
    ))
}

fn collect_line_plan_source_ranges<'a>(
    plan: &'a LinePlan,
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    for (item, (item_source, item_base)) in plan
        .items()
        .iter()
        .zip(split_line_plan_item_sources(source, base))
    {
        collect_line_plan_item_source_ranges(item, item_source, item_base, ranges);
    }
}

fn collect_line_plan_item_source_ranges<'a>(
    item: &'a LinePlanItem,
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    match item {
        LinePlanItem::Option { value, .. } => {
            collect_after_top_level_char(value, source, base, '=', ranges);
        }
        LinePlanItem::Let { expr, .. } => {
            collect_after_top_level_char(expr, source, base, '=', ranges);
        }
        LinePlanItem::Out(expr) => {
            if let Some((expr_source, expr_base)) = strip_line_plan_prefix(source, base, "out") {
                collect_expr_source_ranges_inner(expr, expr_source, expr_base, ranges);
            }
        }
        LinePlanItem::TimedCue { anchor, body } => {
            collect_timed_cue_source_ranges(anchor, body, source, base, ranges);
        }
        LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
            collect_line_plan_group_source_ranges(items, source, base, ranges);
        }
        LinePlanItem::TimelineAssert(assertion) => {
            collect_assert_condition_source_ranges(assertion.condition(), source, base, ranges);
        }
        LinePlanItem::Expr(expr) => {
            collect_expr_source_ranges_inner(expr, source, base, ranges);
        }
        _ => {}
    }
}

fn collect_after_top_level_char<'a>(
    expr: &'a Expr,
    source: &str,
    base: usize,
    delimiter: char,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    if let Some(split) = find_top_level_char(source, delimiter) {
        collect_expr_source_ranges_inner(
            expr,
            &source[split + delimiter.len_utf8()..],
            base + split + delimiter.len_utf8(),
            ranges,
        );
    }
}

fn strip_line_plan_prefix<'a>(
    source: &'a str,
    base: usize,
    prefix: &str,
) -> Option<(&'a str, usize)> {
    let (source, base) = trim_source_with_base(source, base);
    let rest = source.strip_prefix(prefix)?;
    Some(trim_source_with_base(rest, base + prefix.len()))
}

fn collect_timed_cue_source_ranges<'a>(
    anchor: &'a Expr,
    body: &'a Expr,
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    let Some(open) = find_top_level_char(source, '(') else {
        return;
    };
    let Some(close) = matching_delimiter_end(source, open, '(', ')') else {
        return;
    };
    collect_expr_source_ranges_inner(
        anchor,
        &source[open + '('.len_utf8()..close - ')'.len_utf8()],
        base + open + '('.len_utf8(),
        ranges,
    );
    collect_expr_source_ranges_inner(body, &source[close..], base + close, ranges);
}

fn collect_line_plan_group_source_ranges<'a>(
    items: &'a [LinePlanItem],
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    let Some(open) = find_top_level_char(source, '{') else {
        return;
    };
    let Some(close) = matching_delimiter_end(source, open, '{', '}') else {
        return;
    };
    let inner = &source[open + '{'.len_utf8()..close - '}'.len_utf8()];
    let inner_base = base + open + '{'.len_utf8();
    for (item, (item_source, item_base)) in
        items.iter().zip(split_top_level_lines(inner, inner_base))
    {
        collect_line_plan_item_source_ranges(item, item_source, item_base, ranges);
    }
}

fn collect_assert_condition_source_ranges<'a>(
    expr: &'a Expr,
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    if let Some((open, close)) = postfix_delimiter_bounds(source, '(', ')') {
        collect_expr_source_ranges_inner(expr, &source[open + 1..close], base + open + 1, ranges);
    }
}

fn collect_operator_expr_source_ranges<'a>(
    expr: &'a Expr,
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) -> bool {
    match expr {
        Expr::Pipe { lhs, rhs } => {
            if let Some((start, end)) = find_last_top_level_operator(source, "|>") {
                collect_expr_source_ranges_inner(lhs, &source[..start], base, ranges);
                collect_expr_source_ranges_inner(rhs, &source[end..], base + end, ranges);
            }
            true
        }
        Expr::Try { expr } => {
            if let Some(rest) = source.strip_prefix("try") {
                collect_expr_source_ranges_inner(expr, rest, base + "try".len(), ranges);
            } else if let Some(rest) = source.strip_suffix('?') {
                collect_expr_source_ranges_inner(expr, rest, base, ranges);
            }
            true
        }
        Expr::Await(awaited) => {
            let operand_range = awaited.source().operand();
            if let Some(operand_source) = absolute_source_slice(source, base, operand_range) {
                collect_expr_source_ranges_inner(
                    awaited.operand(),
                    operand_source,
                    operand_range.start(),
                    ranges,
                );
            }
            true
        }
        Expr::Range {
            start,
            end,
            inclusive,
        } => {
            let op = if *inclusive { "..=" } else { ".." };
            if let Some((op_start, op_end)) = find_top_level_operator(source, op) {
                if let Some(start) = start {
                    collect_expr_source_ranges_inner(start, &source[..op_start], base, ranges);
                }
                if let Some(end) = end {
                    collect_expr_source_ranges_inner(end, &source[op_end..], base + op_end, ranges);
                }
            }
            true
        }
        Expr::Binary { lhs, op, rhs } => {
            if let Some((op_start, op_end)) = find_binary_operator(source, *op) {
                collect_expr_source_ranges_inner(lhs, &source[..op_start], base, ranges);
                collect_expr_source_ranges_inner(rhs, &source[op_end..], base + op_end, ranges);
            }
            true
        }
        Expr::Closure { body, .. } => {
            if let Some((body_source, body_base)) = closure_body_source(source, base) {
                collect_expr_source_ranges_inner(body, body_source, body_base, ranges);
            }
            true
        }
        Expr::Unary { expr, .. } => {
            if let Some(rest) = source
                .strip_prefix('!')
                .or_else(|| source.strip_prefix('-'))
            {
                collect_expr_source_ranges_inner(expr, rest, base + 1, ranges);
            }
            true
        }
        _ => false,
    }
}

fn collect_control_expr_source_ranges<'a>(
    expr: &'a Expr,
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    match expr {
        Expr::Block { value, .. }
        | Expr::ComputationBlock { value, .. }
        | Expr::NamedBlock { value, .. } => {
            collect_block_value_source_ranges(value.as_deref(), source, base, ranges);
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            if let Some((condition_source, condition_base, then_source, then_base, else_source)) =
                if_expr_sources(source, base)
            {
                collect_expr_source_ranges_inner(
                    condition,
                    condition_source,
                    condition_base,
                    ranges,
                );
                collect_expr_source_ranges_inner(then_branch, then_source, then_base, ranges);
                if let (Some(else_branch), Some((else_source, else_base))) =
                    (else_branch, else_source)
                {
                    collect_expr_source_ranges_inner(else_branch, else_source, else_base, ranges);
                }
            }
        }
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => {
            if let Some((condition_source, condition_base, then_source, then_base, else_source)) =
                if_expr_sources(source, base)
            {
                if let Some((expr_source, expr_base, guard_source)) =
                    if_let_condition_sources(condition_source, condition_base)
                {
                    collect_expr_source_ranges_inner(expr, expr_source, expr_base, ranges);
                    if let (Some(guard), Some((guard_source, guard_base))) = (guard, guard_source) {
                        collect_expr_source_ranges_inner(guard, guard_source, guard_base, ranges);
                    }
                }
                collect_expr_source_ranges_inner(then_branch, then_source, then_base, ranges);
                if let (Some(else_branch), Some((else_source, else_base))) =
                    (else_branch, else_source)
                {
                    collect_expr_source_ranges_inner(else_branch, else_source, else_base, ranges);
                }
            }
        }
        Expr::Match { scrutinee, arms } => {
            collect_match_expr_source_ranges(scrutinee, arms, source, base, ranges);
        }
        Expr::Thread { block } => {
            thread_body::collect_thread_expr_source_ranges(block.body(), source, base, ranges);
        }
        _ => {}
    }
}

fn collect_block_value_source_ranges<'a>(
    value: Option<&'a Expr>,
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    if let Some(value) = value
        && let Some((inner, inner_base)) = braced_block_inner(source, base)
        && let Some((value_source, value_base)) = last_block_value_source(inner, inner_base)
    {
        collect_expr_source_ranges_inner(value, value_source, value_base, ranges);
    }
}

type IfLetConditionSources<'a> = (&'a str, usize, Option<(&'a str, usize)>);

fn if_let_condition_sources(source: &str, base: usize) -> Option<IfLetConditionSources<'_>> {
    let (source, base) = trim_source_with_base(source, base);
    let source = source.strip_prefix("let")?;
    let base = base + "let".len();
    let (source, base) = trim_source_with_base(source, base);
    let eq = find_top_level_char(source, '=')?;
    let value_source = &source[eq + 1..];
    let value_base = base + eq + 1;
    if let Some(when_start) = find_top_level_keyword(value_source, "when") {
        let (expr_source, expr_base) =
            trim_source_with_base(&value_source[..when_start], value_base);
        let (guard_source, guard_base) = trim_source_with_base(
            &value_source[when_start + "when".len()..],
            value_base + when_start + "when".len(),
        );
        Some((expr_source, expr_base, Some((guard_source, guard_base))))
    } else {
        let (expr_source, expr_base) = trim_source_with_base(value_source, value_base);
        Some((expr_source, expr_base, None))
    }
}

fn collect_match_expr_source_ranges<'a>(
    scrutinee: &'a Expr,
    arms: &'a [MatchExprArm],
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    let Some((scrutinee_source, scrutinee_base, body_source, body_base)) =
        match_expr_sources(source, base)
    else {
        return;
    };
    collect_expr_source_ranges_inner(scrutinee, scrutinee_source, scrutinee_base, ranges);
    for (arm, (arm_source, arm_base)) in arms
        .iter()
        .zip(split_top_level_lines(body_source, body_base))
    {
        if let Some((arrow_start, arrow_end)) = find_top_level_operator(arm_source, "=>") {
            if let Some(guard) = arm.guard()
                && let Some(guard_start) = find_top_level_keyword(&arm_source[..arrow_start], "if")
            {
                collect_expr_source_ranges_inner(
                    guard,
                    &arm_source[guard_start + "if".len()..arrow_start],
                    arm_base + guard_start + "if".len(),
                    ranges,
                );
            }
            collect_expr_source_ranges_inner(
                arm.value(),
                &arm_source[arrow_end..],
                arm_base + arrow_end,
                ranges,
            );
        }
    }
}

fn collect_delimited_items<'a>(
    items: &'a [Expr],
    source: &str,
    base: usize,
    open: char,
    close: char,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    if let Some((inner, inner_base)) = delimited_inner(source, base, open, close) {
        for (item, (item_source, item_base)) in items
            .iter()
            .zip(split_top_level_segments(inner, inner_base, ','))
        {
            collect_expr_source_ranges_inner(item, item_source, item_base, ranges);
        }
    }
}

fn collect_record_field_ranges<'a>(
    fields: &'a [(String, Expr)],
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    if let Some(open) = find_top_level_char(source, '{')
        && source.ends_with('}')
    {
        collect_field_value_ranges(
            fields,
            &source[open + 1..source.len() - 1],
            base + open + 1,
            ranges,
        );
    }
}

fn collect_field_value_ranges<'a>(
    fields: &'a [(String, Expr)],
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    for ((_, value), (field_source, field_base)) in fields
        .iter()
        .zip(split_top_level_segments(source, base, ','))
    {
        if let Some(split) = find_top_level_char(field_source, ':')
            .or_else(|| find_top_level_char(field_source, '='))
        {
            collect_expr_source_ranges_inner(
                value,
                &field_source[split + 1..],
                field_base + split + 1,
                ranges,
            );
        } else {
            collect_expr_source_ranges_inner(value, field_source, field_base, ranges);
        }
    }
}

fn split_line_plan_item_sources(source: &str, base: usize) -> Vec<(&str, usize)> {
    let lines = source_lines(source);
    let first_non_empty = lines
        .iter()
        .position(|line| !line.trimmed(source).is_empty());
    let first_indent_override = first_non_empty.and_then(|first| {
        let first_indent = lines[first].indent(source);
        (first_indent == 0)
            .then(|| {
                lines
                    .iter()
                    .enumerate()
                    .skip(first + 1)
                    .filter(|(_, line)| !line.trimmed(source).is_empty())
                    .map(|(_, line)| line.indent(source))
                    .min()
                    .unwrap_or(0)
            })
            .filter(|indent| *indent > 0)
    });
    let mut segments = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trimmed(source);
        if trimmed.is_empty() {
            index += 1;
            continue;
        }
        if line_plan_colon_block_head(trimmed) {
            let parent_indent =
                line.effective_indent(source, index, first_non_empty, first_indent_override);
            let start = line.trimmed_start(source);
            let mut end = line.trimmed_end(source);
            index += 1;
            while index < lines.len() {
                let child = lines[index];
                let child_trimmed = child.trimmed(source);
                if !child_trimmed.is_empty()
                    && child.effective_indent(source, index, first_non_empty, first_indent_override)
                        <= parent_indent
                {
                    break;
                }
                if !child_trimmed.is_empty() {
                    end = child.trimmed_end(source);
                }
                index += 1;
            }
            push_trimmed_segment(source, base, start, end, &mut segments);
        } else {
            push_trimmed_segment(
                source,
                base,
                line.start,
                line.end_without_newline,
                &mut segments,
            );
            index += 1;
        }
    }
    segments
}

#[derive(Clone, Copy)]
struct SourceLine {
    start: usize,
    end_without_newline: usize,
}

impl SourceLine {
    fn text(self, source: &str) -> &str {
        &source[self.start..self.end_without_newline]
    }

    fn trimmed(self, source: &str) -> &str {
        self.text(source).trim()
    }

    fn trimmed_start(self, source: &str) -> usize {
        self.start + self.text(source).len() - self.text(source).trim_start().len()
    }

    fn trimmed_end(self, source: &str) -> usize {
        self.start + self.text(source).trim_end().len()
    }

    fn indent(self, source: &str) -> usize {
        self.trimmed_start(source) - self.start
    }

    fn effective_indent(
        self,
        source: &str,
        index: usize,
        first_non_empty: Option<usize>,
        first_indent_override: Option<usize>,
    ) -> usize {
        if Some(index) == first_non_empty
            && let Some(indent) = first_indent_override
        {
            return indent;
        }
        self.indent(source)
    }
}

fn source_lines(source: &str) -> Vec<SourceLine> {
    let mut lines = Vec::new();
    let mut line_start = 0;
    for line in source.split_inclusive('\n') {
        let line_without_newline = line.strip_suffix('\n').unwrap_or(line);
        lines.push(SourceLine {
            start: line_start,
            end_without_newline: line_start + line_without_newline.len(),
        });
        line_start += line.len();
    }
    if line_start < source.len() {
        lines.push(SourceLine {
            start: line_start,
            end_without_newline: source.len(),
        });
    }
    lines
}

fn line_plan_colon_block_head(line: &str) -> bool {
    if line_plan_let_colon_head(line).is_some() {
        return true;
    }
    let Some(head) = line.strip_suffix(':').map(str::trim) else {
        return false;
    };
    head.starts_with("at(")
        || head == "init"
        || head.starts_with("thread")
        || head.starts_with("on ")
        || head.starts_with("cancel on ")
        || head.starts_with("defer")
        || head == "start"
        || head == "together"
        || head.starts_with("scope")
}

fn line_plan_let_colon_head(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("let ")?;
    let (pattern, expr) = split_top_level_binding(rest)?;
    let head = expr.trim().strip_suffix(':')?.trim();
    (head.starts_with("at(") || head.starts_with("scope")).then_some((pattern, head))
}

fn split_top_level_binding(source: &str) -> Option<(&str, &str)> {
    find_top_level_char(source, '=').map(|split| (&source[..split], &source[split + 1..]))
}

fn closure_body_source(source: &str, base: usize) -> Option<(&str, usize)> {
    let (source, base) = trim_source_with_base(source, base);
    let after_params = if source.starts_with(ExprOp::Or.as_str()) {
        ExprOp::Or.as_str().len()
    } else if let Some(rest) = source.strip_prefix('|') {
        rest.find('|')? + 2
    } else {
        return None;
    };
    let (rest, rest_base) = trim_source_with_base(&source[after_params..], base + after_params);
    if let Some(after_arrow) = rest.strip_prefix("->") {
        if let Some(open) = find_top_level_char(after_arrow, '{') {
            let body_base = rest_base + "->".len() + open;
            return Some((&after_arrow[open..], body_base));
        }
        return None;
    }
    Some((rest, rest_base))
}

fn last_block_value_source(source: &str, base: usize) -> Option<(&str, usize)> {
    split_top_level_segments(source, base, ';')
        .into_iter()
        .last()
}

type IfExprSources<'a> = (&'a str, usize, &'a str, usize, Option<(&'a str, usize)>);

fn if_expr_sources(source: &str, base: usize) -> Option<IfExprSources<'_>> {
    let rest = source.strip_prefix("if")?;
    let rest_base = base + "if".len();
    let then_start = find_top_level_char(rest, '{')?;
    let then_end = matching_delimiter_end(rest, then_start, '{', '}')?;
    let condition = &rest[..then_start];
    let then_source = &rest[then_start..then_end];
    let after_then = &rest[then_end..];
    let after_then_base = rest_base + then_end;
    let (after_then, after_then_base) = trim_source_with_base(after_then, after_then_base);
    let else_source = after_then
        .strip_prefix("else")
        .map(|source| trim_source_with_base(source, after_then_base + "else".len()));
    Some((
        condition,
        rest_base,
        then_source,
        rest_base + then_start,
        else_source,
    ))
}

fn match_expr_sources(source: &str, base: usize) -> Option<(&str, usize, &str, usize)> {
    let rest = source.strip_prefix("match")?;
    let rest_base = base + "match".len();
    let body_start = find_top_level_char(rest, '{')?;
    let body_end = matching_delimiter_end(rest, body_start, '{', '}')?;
    Some((
        &rest[..body_start],
        rest_base,
        &rest[body_start + 1..body_end - 1],
        rest_base + body_start + 1,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ast::pattern::Pattern,
        expr::{
            BinaryOp, DottedPath, Expr, IntLiteral, IntRadix, IntSuffix, Literal, MatchExprArm,
        },
        parser::parse_dialogue_content,
    };

    #[test]
    fn match_expression_arm_values_keep_source_ranges() {
        let source = "match ready {\n    true => 1001i64\n    false => 1002i64\n}";
        let expr = Expr::Match {
            scrutinee: Box::new(Expr::Path(DottedPath::single("ready"))),
            arms: vec![
                MatchExprArm::new(
                    Pattern::Ident("true".to_owned()),
                    None,
                    Box::new(int_literal("1001i64", 1001)),
                ),
                MatchExprArm::new(
                    Pattern::Ident("false".to_owned()),
                    None,
                    Box::new(int_literal("1002i64", 1002)),
                ),
            ],
        };
        let ranges = collect_expr_source_ranges(&expr, source, TextRange::new(0, source.len()));
        let labels = ranges
            .into_iter()
            .filter_map(|range| match range.expr() {
                Expr::Literal(Literal::Int(literal)) => Some((
                    literal.raw(),
                    &source[range.range().start()..range.range().end()],
                )),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(
            labels.contains(&("1001i64", "1001i64")),
            "labels: {labels:?}"
        );
        assert!(
            labels.contains(&("1002i64", "1002i64")),
            "labels: {labels:?}"
        );
    }

    fn int_literal(raw: &str, _value: i64) -> Expr {
        Expr::Literal(Literal::Int(IntLiteral::new(
            raw,
            IntRadix::Decimal,
            Some(IntSuffix::I64),
        )))
    }

    #[test]
    fn await_question_keeps_inner_expression_source_range_after_question_mark() {
        let source = "await? load_bg()";
        let expr = crate::expr::parse_expr(source).expect("authored await call parses");
        let ranges = collect_expr_source_ranges(&expr, source, TextRange::new(0, source.len()));
        let labels = ranges
            .into_iter()
            .filter_map(|range| {
                let label = match range.expr() {
                    Expr::Call(_) => "call",
                    Expr::Path(path) if path == "load_bg" => "path",
                    _ => return None,
                };
                Some((label, &source[range.range().start()..range.range().end()]))
            })
            .collect::<Vec<_>>();

        assert!(
            labels.contains(&("call", "load_bg()")),
            "labels: {labels:?}"
        );
        assert!(labels.contains(&("path", "load_bg")), "labels: {labels:?}");
    }

    #[test]
    fn dialogue_content_range_ignores_natural_apostrophes_around_rich_text_tags() {
        let content = "don't [fx warning()]stop[/fx] [.shake]now[/][p]";
        let source = format!("render('line.focus)[{content}]");
        let expr = Expr::DialogueCall {
            callee: Box::new(
                crate::expr::parse_expr("render('line.focus)")
                    .expect("authored dialogue callee parses"),
            ),
            content: Box::new(parse_dialogue_content(content)),
            plan: None,
        };

        let ranges =
            collect_dialogue_call_content_ranges(&expr, &source, TextRange::new(0, source.len()));

        assert_eq!(ranges.len(), 1, "ranges: {ranges:?}");
        assert_eq!(&source[ranges[0].as_range()], content);
    }

    #[test]
    fn line_plan_colon_let_block_does_not_absorb_following_items() {
        let source = "alice.say()[Choose again.]\n    with:\n        let cue = at(0.42s):\n            score + 3i64\n        out score + 2i64";
        let expr = Expr::DialogueCall {
            callee: Box::new(
                crate::expr::parse_expr("alice.say()").expect("authored dialogue callee parses"),
            ),
            content: Box::new(parse_dialogue_content("Choose again.")),
            plan: Some(crate::ast::line_plan::LinePlan::new(
                crate::ast::line_plan::BlockStyle::Indent,
                vec![
                    crate::ast::line_plan::LinePlanItem::Let {
                        pattern: Pattern::Ident("cue".to_owned()),
                        expr: Expr::NamedBlock {
                            name: "at(0.42s)".to_owned(),
                            statements: Vec::new(),
                            value: Some(Box::new(Expr::Binary {
                                lhs: Box::new(Expr::Path(DottedPath::single("score"))),
                                op: BinaryOp::Add,
                                rhs: Box::new(int_literal("3i64", 3)),
                            })),
                        },
                    },
                    crate::ast::line_plan::LinePlanItem::Out(Expr::Binary {
                        lhs: Box::new(Expr::Path(DottedPath::single("score"))),
                        op: BinaryOp::Add,
                        rhs: Box::new(int_literal("2i64", 2)),
                    }),
                ],
                TextRange::new(0, source.len()),
            )),
        };
        let ranges = collect_expr_source_ranges(&expr, source, TextRange::new(0, source.len()));
        let labels = ranges
            .into_iter()
            .map(|range| &source[range.range().start()..range.range().end()])
            .collect::<Vec<_>>();
        assert!(
            labels.contains(&"score + 2i64"),
            "following line-plan out value should remain a separate segment: {labels:?}"
        );
        assert!(
            labels.contains(&"3i64"),
            "line-plan named cue body should keep child expression ranges: {labels:?}"
        );
    }

    #[test]
    fn thread_expression_statement_sources_do_not_share_block_range() {
        let source = "thread compute {\n    first()\n    second()\n}";
        let first_start = source.find("first()").expect("fixture has first call");
        let second_start = source.find("second()").expect("fixture has second call");
        let expr = Expr::Thread {
            block: Box::new(crate::ast::flow::ThreadBlock::new(
                Vec::new(),
                Some("compute".to_owned()),
                vec![
                    crate::ast::flow::FlowItem::Stmt(crate::ast::flow::Stmt::Expr {
                        expr: crate::expr::parse_expr_at("first()", first_start)
                            .expect("authored first call parses"),
                        expr_source: Some("first()".to_owned()),
                        expr_range: Some(TextRange::new(
                            first_start,
                            first_start + "first()".len(),
                        )),
                    }),
                    crate::ast::flow::FlowItem::Stmt(crate::ast::flow::Stmt::Expr {
                        expr: crate::expr::parse_expr_at("second()", second_start)
                            .expect("authored second call parses"),
                        expr_source: Some("second()".to_owned()),
                        expr_range: Some(TextRange::new(
                            second_start,
                            second_start + "second()".len(),
                        )),
                    }),
                ],
            )),
        };
        let ranges = collect_expr_source_ranges(&expr, source, TextRange::new(0, source.len()));
        let labels = ranges
            .into_iter()
            .map(|range| &source[range.range().start()..range.range().end()])
            .collect::<Vec<_>>();
        assert!(
            labels.contains(&"first()"),
            "first thread body expression should keep its own statement source: {labels:?}"
        );
        assert!(
            labels.contains(&"second()"),
            "second thread body expression should keep its own statement source: {labels:?}"
        );
        assert!(
            !labels.contains(&"first()\n    second()"),
            "thread body expression statements must not share the whole block body range: {labels:?}"
        );
    }
}
