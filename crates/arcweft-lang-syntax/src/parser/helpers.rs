//! Shared parser helpers that are not tied to a single grammar family.

use super::Parser;
use super::control_flow::parse_named_block_expr;
use super::headers::{
    parse_required_entity_ref_syntax, parse_required_id_ref, parse_visibility_prefix, simple_error,
};
use super::line_plan::parse_line_plan_body;
use super::recovery::ParseError;
use super::statements::parse_label_ref;
use crate::ast::{
    common::{DocBlock, TextRange, UseItem, UseMode},
    dialogue::{LineArg, LineOptions, LineOptionsInit},
    ids::{EntityRefSyntax, IdRef, WikiLink},
    items::Attribute,
    line_plan::{BlockStyle, LinePlan},
    pattern::Pattern,
};
use crate::cst::{
    collect_wiki_link_ranges, split_top_level_keyword_once, split_top_level_punctuation,
    split_top_level_punctuation_once,
};
use crate::cst::{find_matching_punctuation, find_top_level_punctuation};
use crate::expr::{ComputationBlockKind, Expr, parse_expr};
use crate::pattern::parse_pattern;
use crate::types::parse_type_ref;

pub(super) enum OptionalLabel {
    None,
    Some(String),
}

#[derive(Default)]
pub(super) struct PendingDocLines {
    start_line: Option<usize>,
    lines: Vec<String>,
}

impl OptionalLabel {
    pub(super) fn into_option(self) -> Option<String> {
        match self {
            Self::None => None,
            Self::Some(label) => Some(label),
        }
    }
}

impl PendingDocLines {
    pub(super) fn push_if_doc(&mut self, line: &str, line_index: usize) -> bool {
        let Some(text) = line.strip_prefix("///") else {
            return false;
        };
        if self.start_line.is_none() {
            self.start_line = Some(line_index);
        }
        self.lines
            .push(text.strip_prefix(' ').unwrap_or(text).to_owned());
        true
    }

    pub(super) fn take(&mut self) -> Option<DocBlock> {
        if self.lines.is_empty() {
            return None;
        }
        let start = self.start_line.take().unwrap_or(0);
        let end = start + self.lines.len();
        let text = core::mem::take(&mut self.lines).join("\n");
        Some(DocBlock::new(text, TextRange::new(start, end)))
    }
}

pub(super) fn parse_use_line(trimmed: &str, range: TextRange) -> Option<UseItem> {
    let (visibility, rest) = parse_visibility_prefix(trimmed);
    let rest = rest.trim_start();
    let (mode, tree) = if let Some(tree) = rest.strip_prefix("lazy use ") {
        (Some(UseMode::Lazy), tree)
    } else if let Some(tree) = rest.strip_prefix("eager use ") {
        (Some(UseMode::Eager), tree)
    } else {
        (None, rest.strip_prefix("use ")?)
    };
    Some(UseItem::new(
        visibility,
        mode,
        normalize_module_path(tree.trim()),
        range,
    ))
}

pub(super) fn normalize_module_path(path: &str) -> String {
    path.strip_prefix("parent::")
        .map_or_else(|| path.to_owned(), |tail| format!("super::{tail}"))
}

pub(super) fn is_relative_id_path(path: &str) -> bool {
    let trimmed = path.trim_start();
    trimmed.starts_with('.') || trimmed.starts_with("@.") || trimmed.starts_with("@super.")
}

pub(super) fn parse_attribute(trimmed: &str, range: TextRange) -> Option<Attribute> {
    let rest = trimmed.strip_prefix("#[")?.strip_suffix(']')?.trim();
    if !rest.contains('(') {
        return Some(Attribute::new(rest.to_owned(), None, range));
    }
    let open = find_top_level_punctuation(rest, '(')?;
    let close = find_matching_punctuation(rest, open, '(', ')')?;
    (rest[close + ')'.len_utf8()..].trim().is_empty()).then_some(())?;
    let name = rest[..open].trim().to_owned();
    let args = rest[open + 1..close].trim();
    Some(Attribute::new(
        name,
        (!args.is_empty()).then(|| args.to_owned()),
        range,
    ))
}

pub(super) fn source_take(parser: &mut Parser) -> String {
    core::mem::take(&mut parser.source)
}

pub(super) fn collect_wiki_links(source: &str) -> Vec<WikiLink> {
    collect_wiki_link_ranges(source)
        .into_iter()
        .map(|(body, start, end)| WikiLink::new(body.to_owned(), TextRange::new(start, end)))
        .collect()
}

pub(super) fn parse_binding_pattern(source: &str) -> (Pattern, Option<crate::types::TypeRef>) {
    split_top_level_punctuation_once(source, ':').map_or_else(
        || (parse_pattern(source.trim()), None),
        |(pattern, ty)| {
            let parsed_ty = parse_type_ref(ty.trim()).ok();
            (parse_pattern(pattern.trim()), parsed_ty)
        },
    )
}

pub(super) fn is_expression_statement_call(trimmed: &str) -> bool {
    if find_top_level_punctuation(trimmed, ':').is_some()
        || find_top_level_punctuation(trimmed, '[').is_some()
    {
        return false;
    }
    matches!(
        crate::expr::parse_expr(trimmed),
        Ok(Expr::Call { .. } | Expr::MethodCall { .. })
    )
}

pub(super) fn parse_line_options(
    args: Option<&str>,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> LineOptions {
    let Some(args) = args else {
        return LineOptions::default();
    };
    let mut state = LineOptionsParseState::default();
    let mut consumed_positional_look = false;
    for arg in split_comma_args(args) {
        let Some((name, value)) = split_top_level_punctuation_once(arg, '=') else {
            if consumed_positional_look {
                errors.push(simple_error(
                    base,
                    arg.len(),
                    "only the first positional dialogue line option may be used as `look`",
                    "look = expr",
                ));
                continue;
            }
            consumed_positional_look = true;
            state.look = Some(parse_expr_lossy(arg.trim()));
            continue;
        };
        parse_named_line_option(
            &mut state,
            name.trim(),
            value.trim(),
            arg.len(),
            base,
            errors,
        );
    }
    LineOptions::new(LineOptionsInit {
        id: state.id,
        text_key: state.text_key,
        voice: state.voice,
        look: state.look,
        stage: state.stage,
        portrait: state.portrait,
        focus: state.focus,
        cleanup: state.cleanup,
        window: state.window,
        source_locale: state.source_locale,
        hooks: state.hooks,
        style: state.style,
        args: state.line_args,
    })
}

#[derive(Default)]
struct LineOptionsParseState {
    id: Option<IdRef>,
    text_key: Option<IdRef>,
    voice: Option<Expr>,
    look: Option<Expr>,
    stage: Option<Expr>,
    portrait: Option<Expr>,
    focus: Option<Expr>,
    cleanup: Option<Expr>,
    window: Option<EntityRefSyntax>,
    source_locale: Option<String>,
    hooks: Vec<Expr>,
    style: Option<Expr>,
    line_args: Vec<LineArg>,
}

fn parse_named_line_option(
    state: &mut LineOptionsParseState,
    name: &str,
    value: &str,
    arg_len: usize,
    base: usize,
    errors: &mut Vec<ParseError>,
) {
    match name {
        "id" => state.id = parse_required_id_ref(value, base, errors).map(|(entity, _)| entity),
        "text_key" => {
            state.text_key = parse_required_id_ref(value, base, errors).map(|(entity, _)| entity);
        }
        "voice" => state.voice = Some(parse_expr_lossy(value)),
        "look" => state.look = Some(parse_expr_lossy(value)),
        "face" => errors.push(simple_error(
            base,
            arg_len,
            "`face` is not a canonical dialogue line option",
            "use `look = expr` or the first positional look option",
        )),
        "stage" => state.stage = Some(parse_expr_lossy(value)),
        "portrait" => state.portrait = Some(parse_expr_lossy(value)),
        "focus" => state.focus = Some(parse_expr_lossy(value)),
        "cleanup" => state.cleanup = Some(parse_expr_lossy(value)),
        "window" => {
            state.window =
                parse_required_entity_ref_syntax(value, base, errors).map(|(entity, _)| entity);
        }
        "source_locale" => state.source_locale = Some(value.to_owned()),
        "hooks" => push_line_hooks(&mut state.hooks, parse_expr_lossy(value)),
        "style" => state.style = Some(parse_expr_lossy(value)),
        name => state
            .line_args
            .push(LineArg::new(name.to_owned(), parse_expr_lossy(value))),
    }
}

fn push_line_hooks(hooks: &mut Vec<Expr>, expr: Expr) {
    if let Expr::BracketSeq(items) = expr {
        hooks.extend(items);
    } else {
        hooks.push(expr);
    }
}

pub(super) fn split_comma_args(source: &str) -> Vec<&str> {
    split_top_level_punctuation(source, ',')
}

pub(super) fn collect_logical_block_items(body: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut depth = 0_i32;

    for raw_line in body.lines().filter(|line| !line.trim().is_empty()) {
        let trimmed = raw_line.trim_start();
        // Method-chain continuation lines belong to the preceding logical item.
        // Without this, multi-line `Option.context(...)?` or `Need.context(...)`
        // parses the dot line as an unrelated raw flow item.
        if current.is_empty()
            && trimmed.starts_with('.')
            && let Some(previous) = lines.pop()
        {
            current = previous;
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(raw_line);
        depth += crate::cst::punctuation_delta(raw_line, '{', '}');
        depth += crate::cst::punctuation_delta(raw_line, '(', ')');
        depth += crate::cst::punctuation_delta(raw_line, '[', ']');
        if depth <= 0 {
            lines.push(core::mem::take(&mut current));
            depth = 0;
        }
    }
    if !current.trim().is_empty() {
        lines.push(current);
    }
    lines
}

pub(super) fn split_brace_item(source: &str) -> Option<(&str, &str)> {
    let open = find_top_level_punctuation(source, '{')?;
    let close = find_matching_punctuation(source, open, '{', '}')?;
    (source[close + '}'.len_utf8()..].trim().is_empty())
        .then(|| (source[..open].trim(), source[open + 1..close].trim()))
}

pub(super) fn split_speaker_line(trimmed: &str) -> Option<(String, Option<String>, &str)> {
    let colon = find_top_level_colon(trimmed)?;
    if has_top_level_square(&trimmed[..colon]) || trimmed[..colon].contains("->") {
        return None;
    }
    let head = trimmed[..colon].trim();
    let content = trimmed[colon + 1..].trim();
    if head.is_empty() || head.starts_with("cancel ") || head.starts_with("at(") {
        return None;
    }
    let (speaker, args) = split_call_head(head);
    Some((speaker, args, content))
}

fn has_top_level_square(input: &str) -> bool {
    let mut depth = 0_i32;
    let mut in_string = false;
    for ch in input.chars() {
        match ch {
            '"' => in_string = !in_string,
            '[' if depth == 0 && !in_string => return true,
            '(' | '{' | '[' if !in_string => depth += 1,
            ')' | '}' | ']' if !in_string => depth -= 1,
            _ => {}
        }
    }
    false
}

fn find_top_level_colon(input: &str) -> Option<usize> {
    find_top_level_punctuation(input, ':')
}

pub(super) fn split_call_head(head: &str) -> (String, Option<String>) {
    let head = head.trim();
    if let Some(open) = find_top_level_punctuation(head, '(')
        && let Some(close) = find_matching_punctuation(head, open, '(', ')')
        && head[close + ')'.len_utf8()..].trim().is_empty()
    {
        return (
            head[..open].trim().to_owned(),
            Some(head[open + 1..close].trim().to_owned()),
        );
    }
    (head.to_owned(), None)
}

pub(super) fn find_content_bracket(text: &str) -> Option<usize> {
    let open = find_top_level_punctuation(text, '[')?;
    (!text[..open].trim_end().ends_with('#')).then_some(open)
}

pub(super) fn attach_line_plan_label(plan: LinePlan, label: Option<String>) -> LinePlan {
    if let Some(label) = label {
        plan.with_label(label)
    } else {
        plan
    }
}

pub(super) fn parse_line_plan_attachment(
    style: BlockStyle,
    body: &str,
    range: TextRange,
    label: Option<String>,
) -> LinePlan {
    attach_line_plan_label(parse_line_plan_body(style, body, range), label)
}

pub(super) fn flat_block_head(kind: &str, head: &str) -> String {
    if head.is_empty() {
        kind.to_owned()
    } else {
        format!("{kind} {head}")
    }
}

pub(super) fn parse_with_indent_label(trimmed: &str) -> Option<OptionalLabel> {
    if trimmed == "with:" {
        return Some(OptionalLabel::None);
    }
    let label = trimmed.strip_prefix("with ")?.strip_suffix(':')?.trim();
    parse_label_ref(label)
        .and_then(|(label, tail)| tail.trim().is_empty().then_some(OptionalLabel::Some(label)))
}

pub(super) fn parse_inline_with_colon_plan(trimmed: &str) -> Option<(Option<String>, &str)> {
    let rest = trimmed.strip_prefix("with")?.trim_start();
    if let Some(body) = rest.strip_prefix(':') {
        let body = body.trim();
        return (!body.is_empty()).then_some((None, body));
    }
    let (label, tail) = parse_label_ref(rest)?;
    let body = tail.trim_start().strip_prefix(':')?.trim();
    (!body.is_empty()).then_some((Some(label), body))
}

pub(super) fn is_with_brace_head(trimmed: &str) -> bool {
    trimmed == "with"
        || trimmed.starts_with("with {")
        || trimmed == "with{"
        || trimmed.starts_with("with '")
        || trimmed.starts_with("with'")
}

pub(super) fn parse_with_brace_label(head: &str) -> Option<String> {
    let label = head.strip_prefix("with")?.trim();
    parse_label_ref(label).and_then(|(label, tail)| tail.trim().is_empty().then_some(label))
}

pub(super) fn split_optional_block_label(head: &str) -> (Option<String>, &str) {
    labeled_head_tail(head).map_or((None, head), |tail| {
        let label = head
            .trim_start()
            .strip_prefix('\'')
            .and_then(|rest| split_top_level_punctuation_once(rest, ':'))
            .map(|(label, _)| label.trim().to_owned())
            .unwrap_or_default();
        (Some(label), tail)
    })
}

fn labeled_head_tail(head: &str) -> Option<&str> {
    let rest = head.trim_start().strip_prefix('\'')?;
    let (_, tail) = split_top_level_punctuation_once(rest, ':')?;
    Some(tail.trim_start())
}

pub(super) fn parse_expr_lossy(source: &str) -> crate::expr::Expr {
    let normalized = normalize_dot_continuations(source);
    let source = normalized.trim();
    if let Some(value) = parse_raw_string_literal(source) {
        return crate::expr::Expr::Literal(crate::expr::Literal::String(value));
    }
    if let Some(expr) = parse_static_generic_call(source) {
        return expr;
    }
    if let Some((head, body)) = split_brace_item(source) {
        let name = head.trim();
        if is_plain_block_callee(name) {
            return parse_named_block_expr(name, body);
        }
    }
    parse_expr(source).unwrap_or_else(|_| crate::expr::Expr::Raw(source.to_owned()))
}

fn normalize_dot_continuations(source: &str) -> String {
    let mut lines = source.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut normalized = first.trim().to_owned();
    for line in lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with('.') {
            normalized.push_str(trimmed);
        } else {
            normalized.push(' ');
            normalized.push_str(trimmed);
        }
    }
    normalized
}

fn parse_static_generic_call(source: &str) -> Option<crate::expr::Expr> {
    let open = find_top_level_punctuation(source, '(')?;
    let close = find_matching_punctuation(source, open, '(', ')')?;
    if !source[close + ')'.len_utf8()..].trim().is_empty() {
        return None;
    }
    let callee = source[..open].trim();
    if !(callee.contains('<') && callee.contains("::")) {
        return None;
    }
    Some(crate::expr::Expr::Call {
        callee: Box::new(crate::expr::Expr::Path(callee.to_owned())),
        args: split_comma_args(&source[open + '('.len_utf8()..close])
            .into_iter()
            .map(parse_expr_lossy)
            .collect(),
    })
}

fn parse_raw_string_literal(source: &str) -> Option<String> {
    let rest = source.strip_prefix('r')?;
    let hashes = rest.chars().take_while(|ch| *ch == '#').count();
    let quote_start = 1 + hashes;
    if !source.get(quote_start..)?.starts_with('"') {
        return None;
    }
    let closing = format!("\"{}", "#".repeat(hashes));
    let body_start = quote_start + '"'.len_utf8();
    let body_end = source.get(body_start..)?.strip_suffix(&closing)?;
    Some(body_end.to_owned())
}

fn is_plain_block_callee(source: &str) -> bool {
    !source.is_empty()
        && source
            .chars()
            .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | ':'))
        && source
            .chars()
            .next()
            .is_some_and(|ch| ch.is_lowercase() || ch == '_')
}

pub(super) fn is_typed_stmt(trimmed: &str) -> bool {
    matches!(
        trimmed.split_whitespace().next(),
        Some(
            "let"
                | "match"
                | "if"
                | "for"
                | "return"
                | "out"
                | "goto"
                | "thread"
                | "defer"
                | "yield"
                | "signal"
                | "close"
                | "break"
                | "continue"
        )
    )
}

pub(super) fn parse_memo_block_options(source: &str) -> Option<Vec<(String, Expr)>> {
    let args = source
        .trim()
        .strip_prefix("memo(")?
        .trim_end()
        .strip_suffix(')')?;
    Some(
        split_comma_args(args)
            .into_iter()
            .filter_map(|part| {
                split_top_level_punctuation_once(part, '=')
                    .map(|(name, value)| (name.trim().to_owned(), parse_expr_lossy(value.trim())))
            })
            .collect(),
    )
}

pub(super) fn parse_computation_block_kind(source: &str) -> Option<ComputationBlockKind> {
    match source {
        "result" => Some(ComputationBlockKind::Result),
        "task" => Some(ComputationBlockKind::Task),
        "seq" => Some(ComputationBlockKind::Seq),
        "stream" => Some(ComputationBlockKind::Stream),
        _ => None,
    }
}

pub(super) fn split_top_level_binding(source: &str) -> Option<(&str, &str)> {
    split_top_level_punctuation_once(source, '=')
}

pub(super) fn parse_expr_with_inline_line_plan(source: &str) -> Expr {
    let Some((expr_source, trailing_plan)) = split_inline_dialogue_line_plan(source) else {
        return parse_dialogue_call_expr_surface(source)
            .unwrap_or_else(|| parse_expr_lossy(source));
    };
    let mut expr = parse_dialogue_call_expr_source(expr_source.trim())
        .unwrap_or_else(|| parse_expr_lossy(expr_source.trim()));
    let Some(plan) = parse_inline_line_plan_source(trailing_plan) else {
        return parse_expr_lossy(source);
    };
    if attach_plan_to_dialogue_expr(&mut expr, plan) {
        expr
    } else {
        parse_expr_lossy(source)
    }
}

fn parse_dialogue_call_expr_surface(source: &str) -> Option<Expr> {
    if !looks_like_dialogue_call_expr_surface(source) {
        return None;
    }
    parse_dialogue_call_expr_source(source)
}

fn looks_like_dialogue_call_expr_surface(source: &str) -> bool {
    let source = source.trim();
    let source = source.strip_prefix("try ").map_or(source, str::trim);
    let Some(open) = find_content_bracket(source) else {
        return false;
    };
    let Some(close) = find_matching_punctuation(source, open, '[', ']') else {
        return false;
    };
    if !source[close + 1..].trim().is_empty() {
        return false;
    }
    let callee = source[..open].trim();
    let content = source[open + 1..close].trim();
    // In expression position, `target[expr]` is an index unless the target is a
    // call-like dialogue surface or the bracket payload is not a valid typed
    // expression. This keeps `nums[0]` out of dialogue parsing while preserving
    // `alice.say()[text]` and `alice[おはよう。[p]]` surfaces.
    callee.contains('(') || parse_expr(content).is_err()
}

pub(super) fn parse_dialogue_call_expr_source(source: &str) -> Option<Expr> {
    if let Some(rest) = source.trim().strip_prefix("try ") {
        return Some(Expr::Try {
            expr: Box::new(parse_dialogue_call_expr_source(rest.trim())?),
        });
    }
    let open = find_content_bracket(source)?;
    let close = find_matching_punctuation(source, open, '[', ']')?;
    if !source[close + 1..].trim().is_empty() {
        return None;
    }
    let callee = source[..open].trim();
    if callee.is_empty() {
        return None;
    }
    let content = source[open + 1..close].trim();
    Some(Expr::DialogueCall {
        callee: Box::new(parse_expr_lossy(callee)),
        content: content.to_owned(),
        plan: None,
    })
}

pub(super) fn attach_plan_to_dialogue_expr(expr: &mut Expr, line_plan: LinePlan) -> bool {
    match expr {
        Expr::DialogueCall { plan, .. } => {
            *plan = Some(line_plan);
            true
        }
        Expr::Try { expr } => attach_plan_to_dialogue_expr(expr, line_plan),
        _ => false,
    }
}

pub(super) fn contains_dialogue_expr(expr: &Expr) -> bool {
    match expr {
        Expr::DialogueCall { .. } => true,
        Expr::Try { expr } => contains_dialogue_expr(expr),
        _ => false,
    }
}

fn split_inline_dialogue_line_plan(source: &str) -> Option<(&str, &str)> {
    let (head, tail) = split_top_level_keyword_once(source, "with");
    let tail = tail?;
    if matches!(tail.trim_start().chars().next(), Some(':' | '{' | '\'')) {
        let head_end = head.trim_end().len();
        Some((&source[..head_end], source[head_end..].trim_start()))
    } else {
        None
    }
}

fn parse_inline_line_plan_source(source: &str) -> Option<LinePlan> {
    if is_with_brace_head(source) {
        let (head, body) = split_brace_item(source)?;
        return Some(parse_line_plan_attachment(
            BlockStyle::Brace,
            body,
            TextRange::new(0, source.len()),
            parse_with_brace_label(head.trim()),
        ));
    }
    parse_inline_with_colon_plan(source).map(|(label, body)| {
        parse_line_plan_attachment(
            BlockStyle::Indent,
            body,
            TextRange::new(0, source.len()),
            label,
        )
    })
}

pub(super) fn indentation(text: &str) -> usize {
    text.chars().take_while(|ch| ch.is_whitespace()).count()
}
