use super::{BinaryOp, CallArg, Expr, ExprOp, MatchExprArm, is_ident_continue};
use crate::ast::common::TextRange;
use crate::ast::flow::{FlowItem, Stmt};
use crate::ast::line_plan::{LinePlan, LinePlanItem};

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
        Expr::Call { callee, args } => {
            if let Some((open, close)) = postfix_delimiter_bounds(source, '(', ')') {
                collect_expr_source_ranges_inner(callee, &source[..open], base, ranges);
                let inner = &source[open + 1..close];
                let inner_base = base + open + 1;
                for (arg, (arg_source, arg_base)) in args
                    .iter()
                    .zip(split_top_level_segments(inner, inner_base, ','))
                {
                    collect_call_arg_source_ranges(arg, arg_source, arg_base, ranges);
                }
            }
            true
        }
        Expr::Select(select) => {
            if let Some(dot) = find_last_top_level_char(source, '.') {
                collect_expr_source_ranges_inner(select.target(), &source[..dot], base, ranges);
            }
            true
        }
        Expr::DialogueCall { callee, plan, .. } => {
            if let Some((callee_source, callee_base, plan_body)) =
                dialogue_call_source_parts(source, base)
            {
                collect_expr_source_ranges_inner(callee, callee_source, callee_base, ranges);
                if let (Some(plan), Some((body_source, body_base))) = (plan, plan_body) {
                    collect_line_plan_source_ranges(plan, body_source, body_base, ranges);
                }
            }
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

type DialogueCallSourceParts<'a> = (&'a str, usize, Option<(&'a str, usize)>);

fn dialogue_call_source_parts(source: &str, base: usize) -> Option<DialogueCallSourceParts<'_>> {
    let (source, base) = trim_source_with_base(source, base);
    let content_open = find_top_level_char(source, '[')?;
    let content_close = matching_delimiter_end(source, content_open, '[', ']')?;
    let plan_source = source
        .get(content_close..)
        .and_then(|source| line_plan_body_source(source, base + content_close));
    Some((&source[..content_open], base, plan_source))
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
    for (item, (item_source, item_base)) in
        plan.items().iter().zip(split_top_level_lines(source, base))
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
        LinePlanItem::Assert { expr, .. } => {
            collect_assert_condition_source_ranges(expr, source, base, ranges);
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
        Expr::Await { expr, applies_try } => {
            if let Some(rest) = source.strip_prefix("try await") {
                collect_expr_source_ranges_inner(expr, rest, base + "try await".len(), ranges);
            } else if *applies_try && let Some(rest) = source.strip_prefix("await?") {
                collect_expr_source_ranges_inner(expr, rest, base + "await?".len(), ranges);
            } else if let Some(rest) = source.strip_prefix("await") {
                collect_expr_source_ranges_inner(expr, rest, base + "await".len(), ranges);
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
        Expr::MemoBlock { options, value, .. } => {
            collect_memo_option_source_ranges(options, source, base, ranges);
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
            collect_thread_expr_source_ranges(block.body(), source, base, ranges);
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
        && let Some((inner, inner_base)) = delimited_inner(source, base, '{', '}')
        && let Some((value_source, value_base)) = last_block_value_source(inner, inner_base)
    {
        collect_expr_source_ranges_inner(value, value_source, value_base, ranges);
    }
}

fn collect_memo_option_source_ranges<'a>(
    options: &'a [(String, Expr)],
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    let Some((args, args_base)) = memo_option_args_source(source, base) else {
        return;
    };
    for ((_, option), (value_source, value_base)) in options
        .iter()
        .zip(memo_option_value_sources(args, args_base))
    {
        collect_expr_source_ranges_inner(option, value_source, value_base, ranges);
    }
}

fn memo_option_args_source(source: &str, base: usize) -> Option<(&str, usize)> {
    let (source, base) = trim_source_with_base(source, base);
    let rest = source.strip_prefix("memo")?;
    let (rest, rest_base) = trim_source_with_base(rest, base + "memo".len());
    let args_end = matching_delimiter_end(rest, 0, '(', ')')?;
    Some((
        &rest['('.len_utf8()..args_end - ')'.len_utf8()],
        rest_base + '('.len_utf8(),
    ))
}

fn memo_option_value_sources(source: &str, base: usize) -> Vec<(&str, usize)> {
    split_top_level_segments(source, base, ',')
        .into_iter()
        .filter_map(|(segment, segment_base)| {
            let split = find_top_level_char(segment, '=')?;
            Some((&segment[split + '='.len_utf8()..], segment_base + split + 1))
        })
        .collect()
}

fn collect_thread_expr_source_ranges<'a>(
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
    for item in body {
        let FlowItem::Stmt(Stmt::Expr { expr, .. }) = item else {
            continue;
        };
        collect_expr_source_ranges_inner(expr, inner, inner_base, ranges);
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

fn collect_call_arg_source_ranges<'a>(
    arg: &'a CallArg,
    source: &str,
    base: usize,
    ranges: &mut Vec<ExprSourceRange<'a>>,
) {
    match arg {
        CallArg::Positional(expr) => collect_expr_source_ranges_inner(expr, source, base, ranges),
        CallArg::Named { value, .. } => {
            if let Some(eq) = find_top_level_char(source, '=') {
                collect_expr_source_ranges_inner(value, &source[eq + 1..], base + eq + 1, ranges);
            }
        }
        CallArg::Spread { value } => {
            let (source, base) = trim_source_with_base(source, base);
            if let Some(rest) = source.strip_prefix(ExprOp::Spread.as_str()) {
                collect_expr_source_ranges_inner(
                    value,
                    rest,
                    base + ExprOp::Spread.as_str().len(),
                    ranges,
                );
            } else if let Some(rest) = source.strip_suffix(ExprOp::Spread.as_str()) {
                collect_expr_source_ranges_inner(value, rest, base, ranges);
            }
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

fn trim_source_with_base(source: &str, base: usize) -> (&str, usize) {
    let start_trim = source.len() - source.trim_start().len();
    let source = &source[start_trim..];
    let end = source.trim_end().len();
    (&source[..end], base + start_trim)
}

fn delimited_inner(source: &str, base: usize, open: char, close: char) -> Option<(&str, usize)> {
    let (source, base) = trim_source_with_base(source, base);
    source
        .strip_prefix(open)?
        .strip_suffix(close)
        .map(|inner| (inner, base + open.len_utf8()))
}

fn postfix_delimiter_bounds(source: &str, open: char, close: char) -> Option<(usize, usize)> {
    let close_start = source
        .char_indices()
        .last()
        .filter(|(_, ch)| *ch == close)
        .map(|(index, _)| index)?;
    let mut state = SourceScanState::default();
    let mut result = None;
    for (index, ch) in source
        .char_indices()
        .take_while(|(index, _)| *index < close_start)
    {
        if state.is_top_level_before(ch) && ch == open {
            result = Some(index);
        }
        state.advance(ch);
    }
    result.map(|open_start| (open_start, close_start))
}

fn split_top_level_segments(source: &str, base: usize, delimiter: char) -> Vec<(&str, usize)> {
    let mut state = SourceScanState::default();
    let mut start = 0;
    let mut segments = Vec::new();
    for (index, ch) in source.char_indices() {
        if state.is_top_level_before(ch) && ch == delimiter {
            push_trimmed_segment(source, base, start, index, &mut segments);
            start = index + ch.len_utf8();
        }
        state.advance(ch);
    }
    push_trimmed_segment(source, base, start, source.len(), &mut segments);
    segments
}

fn split_top_level_lines(source: &str, base: usize) -> Vec<(&str, usize)> {
    let mut segments = Vec::new();
    let mut line_start = 0;
    for line in source.split_inclusive('\n') {
        let line_without_newline = line.strip_suffix('\n').unwrap_or(line);
        push_trimmed_segment(
            source,
            base,
            line_start,
            line_start + line_without_newline.len(),
            &mut segments,
        );
        line_start += line.len();
    }
    if line_start < source.len() {
        push_trimmed_segment(source, base, line_start, source.len(), &mut segments);
    }
    segments
}

fn push_trimmed_segment<'a>(
    source: &'a str,
    base: usize,
    start: usize,
    end: usize,
    segments: &mut Vec<(&'a str, usize)>,
) {
    let (segment, segment_base) = trim_source_with_base(&source[start..end], base + start);
    if !segment.is_empty() {
        segments.push((segment, segment_base));
    }
}

fn find_top_level_char(source: &str, target: char) -> Option<usize> {
    let mut state = SourceScanState::default();
    for (index, ch) in source.char_indices() {
        if state.is_top_level_before(ch) && ch == target {
            return Some(index);
        }
        state.advance(ch);
    }
    None
}

fn find_last_top_level_char(source: &str, target: char) -> Option<usize> {
    let mut state = SourceScanState::default();
    let mut result = None;
    for (index, ch) in source.char_indices() {
        if state.is_top_level_before(ch) && ch == target {
            result = Some(index);
        }
        state.advance(ch);
    }
    result
}

fn find_top_level_operator(source: &str, operator: &str) -> Option<(usize, usize)> {
    let mut state = SourceScanState::default();
    for (index, ch) in source.char_indices() {
        if state.is_top_level_before(ch)
            && source[index..].starts_with(operator)
            && operator_boundaries_match(source, index, operator)
        {
            return Some((index, index + operator.len()));
        }
        state.advance(ch);
    }
    None
}

fn find_last_top_level_operator(source: &str, operator: &str) -> Option<(usize, usize)> {
    let mut state = SourceScanState::default();
    let mut result = None;
    for (index, ch) in source.char_indices() {
        if state.is_top_level_before(ch)
            && source[index..].starts_with(operator)
            && operator_boundaries_match(source, index, operator)
        {
            result = Some((index, index + operator.len()));
        }
        state.advance(ch);
    }
    result
}

fn find_binary_operator(source: &str, op: BinaryOp) -> Option<(usize, usize)> {
    let operator = binary_op_source(op);
    if matches!(op, BinaryOp::Implies) {
        find_top_level_operator(source, operator)
    } else {
        find_last_top_level_operator(source, operator)
    }
}

fn binary_op_source(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Implies => "=>",
        BinaryOp::Or => "||",
        BinaryOp::And => "&&",
        BinaryOp::In => "in",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Gte => ">=",
        BinaryOp::Lte => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Lt => "<",
        BinaryOp::Merge => "&",
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
    }
}

fn operator_boundaries_match(source: &str, index: usize, operator: &str) -> bool {
    if operator == "in" {
        let before = source[..index].chars().next_back();
        let after = source[index + operator.len()..].chars().next();
        before.is_none_or(|ch| !is_ident_continue(ch))
            && after.is_none_or(|ch| !is_ident_continue(ch))
    } else {
        true
    }
}

fn find_top_level_keyword(source: &str, keyword: &str) -> Option<usize> {
    let mut state = SourceScanState::default();
    for (index, ch) in source.char_indices() {
        if state.is_top_level_before(ch)
            && source[index..].starts_with(keyword)
            && operator_boundaries_match(source, index, keyword)
        {
            return Some(index);
        }
        state.advance(ch);
    }
    None
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
    if let Some(rest) = rest.strip_prefix("->") {
        if let Some(open) = find_top_level_char(rest, '{') {
            return Some((&rest[open..], rest_base + open));
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

fn matching_delimiter_end(
    source: &str,
    open_start: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut depth = 0usize;
    for (index, ch) in source
        .char_indices()
        .skip_while(|(index, _)| *index < open_start)
    {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if in_char {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '\'' {
                in_char = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
        } else if ch == '\'' {
            in_char = true;
        } else if ch == open {
            depth += 1;
        } else if ch == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index + ch.len_utf8());
            }
        }
    }
    None
}

#[derive(Default)]
struct SourceScanState {
    paren: usize,
    bracket: usize,
    brace: usize,
    in_string: bool,
    in_char: bool,
    escaped: bool,
}

impl SourceScanState {
    fn is_top_level_before(&self, ch: char) -> bool {
        !self.in_string
            && !self.in_char
            && self.paren == 0
            && self.bracket == 0
            && self.brace == 0
            && !matches!(ch, ')' | ']' | '}')
    }

    fn advance(&mut self, ch: char) {
        if self.in_string {
            if self.escaped {
                self.escaped = false;
            } else if ch == '\\' {
                self.escaped = true;
            } else if ch == '"' {
                self.in_string = false;
            }
            return;
        }
        if self.in_char {
            if self.escaped {
                self.escaped = false;
            } else if ch == '\\' {
                self.escaped = true;
            } else if ch == '\'' {
                self.in_char = false;
            }
            return;
        }
        match ch {
            '"' => self.in_string = true,
            '\'' => self.in_char = true,
            '(' => self.paren += 1,
            ')' => self.paren = self.paren.saturating_sub(1),
            '[' => self.bracket += 1,
            ']' => self.bracket = self.bracket.saturating_sub(1),
            '{' => self.brace += 1,
            '}' => self.brace = self.brace.saturating_sub(1),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ast::pattern::Pattern,
        expr::{DottedPath, Expr, Literal, MatchExprArm},
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
                Expr::Literal(Literal::Int { raw, .. }) => Some((
                    raw.as_str(),
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

    fn int_literal(raw: &str, value: i64) -> Expr {
        Expr::Literal(Literal::Int {
            raw: raw.to_owned(),
            value,
            suffix: Some("i64".to_owned()),
        })
    }

    #[test]
    fn await_question_keeps_inner_expression_source_range_after_question_mark() {
        let source = "await? load_bg()";
        let expr = Expr::Await {
            expr: Box::new(Expr::Call {
                callee: Box::new(Expr::Path(DottedPath::single("load_bg"))),
                args: Vec::new(),
            }),
            applies_try: true,
        };
        let ranges = collect_expr_source_ranges(&expr, source, TextRange::new(0, source.len()));
        let labels = ranges
            .into_iter()
            .filter_map(|range| {
                let label = match range.expr() {
                    Expr::Call { .. } => "call",
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
}
