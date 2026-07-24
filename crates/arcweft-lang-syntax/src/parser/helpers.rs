//! Shared parser helpers that are not tied to a single grammar family.

use std::borrow::Cow;

use super::control_flow::parse_named_block_expr;
use super::headers::{
    parse_required_entity_ref_syntax, parse_required_id_ref, parse_visibility_prefix, simple_error,
};
use super::line_plan::parse_line_plan_body;
use super::line_plan::parse_line_plan_body_with_body_base;
use super::recovery::{ParseError, ParseErrorKind, RecoveryEdit, RecoverySuggestion};
use super::statements::parse_label_ref;
use super::{Parser, parse_dialogue_content};
use crate::ast::{
    common::{DocBlock, TextRange, UseItem, UseTree},
    dialogue::{LineArg, LineOptions, LineOptionsInit},
    ids::{EntityRefSyntax, IdRef, WikiLink},
    items::Attribute,
    line_plan::{BlockStyle, LinePlan},
    pattern::Pattern,
};
use crate::cst::{ArcweftPunctuation, SyntaxParseStats};
use crate::cst::{
    CstPunctuationScan, collect_wiki_link_ranges, contains_arcweft_punctuation,
    split_top_level_keyword_once, split_top_level_punctuation, split_top_level_punctuation_once,
};
use crate::cst::{
    find_matching_punctuation, find_top_level_matching_punctuation, find_top_level_punctuation,
};
use crate::expr::{
    CallRecoveryBoundarySyntax, ComputationBlockKind, Expr, ExprRecoveryDiagnostic, parse_expr,
    parse_expr_fragment_recovering_at, parse_expr_with_stats,
};
use crate::pattern::parse_pattern_at;
use crate::types::{AuthoredTypeRef, parse_type_ref};

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

pub(super) fn parse_use_line(
    trimmed: &str,
    range: TextRange,
) -> Result<Option<UseItem>, crate::ast::common::UseTreeError> {
    let (visibility, rest) = parse_visibility_prefix(trimmed);
    let rest = rest.trim_start();
    let Some(tree) = rest.strip_prefix("use ") else {
        return Ok(None);
    };
    let tree = tree.trim();
    let tree_offset = tree.as_ptr() as usize - trimmed.as_ptr() as usize;
    Ok(Some(UseItem::new(
        visibility,
        UseTree::parse_at(tree, range.start() + tree_offset)?,
        range,
    )))
}

pub(super) fn normalize_module_path(path: &str) -> String {
    path.strip_prefix("parent.")
        .map_or_else(|| path.to_owned(), |tail| format!("super.{tail}"))
}

pub(super) fn is_relative_id_path(path: &str) -> bool {
    let trimmed = path.trim_start();
    trimmed.starts_with('.') || trimmed.starts_with("@.") || trimmed.starts_with("@super.")
}

pub(super) fn parse_outer_attribute(trimmed: &str, range: TextRange) -> Option<Attribute> {
    parse_attribute_with_prefix(trimmed, "#[", range)
}

pub(super) fn parse_inner_attribute(trimmed: &str, range: TextRange) -> Option<Attribute> {
    parse_attribute_with_prefix(trimmed, "#![", range)
}

fn parse_attribute_with_prefix(trimmed: &str, prefix: &str, range: TextRange) -> Option<Attribute> {
    let rest = trimmed.strip_prefix(prefix)?.strip_suffix(']')?.trim();
    if !rest.contains('(') {
        return Some(Attribute::new(rest.to_owned(), None, range));
    }
    let (open, close) = find_top_level_matching_punctuation(rest, '(', ')')?;
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
    parser.source.to_owned()
}

pub(super) fn collect_wiki_links(source: &str) -> Vec<WikiLink> {
    collect_wiki_link_ranges(source)
        .into_iter()
        .map(|(body, start, end)| WikiLink::new(body.to_owned(), TextRange::new(start, end)))
        .collect()
}

pub(super) fn parse_binding_pattern(
    source: &str,
    base: usize,
) -> (Pattern, Option<AuthoredTypeRef>) {
    let trimmed = source.trim();
    let base = base + source.len().saturating_sub(source.trim_start().len());
    split_top_level_punctuation_once(trimmed, ':').map_or_else(
        || (parse_pattern_at(trimmed, base), None),
        |(pattern, ty)| {
            let pattern_source = pattern.trim();
            let pattern = parse_pattern_at(
                pattern_source,
                base + subslice_offset(trimmed, pattern_source),
            );
            let type_source = ty.trim();
            let parsed_ty = parse_type_ref(type_source).ok().map(|mut ty| {
                ty.rebase(base + subslice_offset(trimmed, type_source));
                ty
            });
            (pattern, parsed_ty)
        },
    )
}

fn subslice_offset(source: &str, fragment: &str) -> usize {
    (fragment.as_ptr() as usize).saturating_sub(source.as_ptr() as usize)
}

pub(super) fn parse_type_ref_or_error(
    source: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> AuthoredTypeRef {
    let trimmed = source.trim();
    let leading = source.len().saturating_sub(source.trim_start().len());
    match parse_type_ref(trimmed) {
        Ok(mut ty) => {
            ty.rebase(base + leading);
            ty
        }
        Err(error) => {
            let recovery_index = u32::try_from(errors.len())
                .expect("syntax diagnostic count is bounded below u32::MAX");
            errors.push(simple_error(
                base + leading,
                trimmed.len(),
                &error.to_string(),
                "a canonical Arcweft type",
            ));
            let mut recovery =
                AuthoredTypeRef::recovery(recovery_index, TextRange::new(0, trimmed.len()));
            recovery.rebase(base + leading);
            recovery
        }
    }
}

pub(super) fn validate_let_type_ascriptions(source: &str) -> Vec<ParseError> {
    let mut errors = Vec::new();
    let mut line_base = 0;
    for line_with_ending in source.split_inclusive('\n') {
        let line = line_with_ending.trim_end_matches(['\r', '\n']);
        for (let_offset, _) in line.match_indices("let ") {
            let boundary = let_offset == 0
                || line[..let_offset]
                    .chars()
                    .next_back()
                    .is_some_and(|ch| ch.is_whitespace() || matches!(ch, '{' | ';'));
            if !boundary {
                continue;
            }
            let binding_source = &line[let_offset + "let ".len()..];
            let Some((binding, _)) = split_top_level_binding(binding_source) else {
                continue;
            };
            let Some((_, ty)) = split_top_level_punctuation_once(binding, ':') else {
                continue;
            };
            let type_offset = binding
                .rfind(ty)
                .map_or(let_offset + "let ".len(), |offset| {
                    let_offset + "let ".len() + offset
                });
            let _ = parse_type_ref_or_error(ty, line_base + type_offset, &mut errors);
        }
        line_base += line_with_ending.len();
    }
    errors
}

pub(super) fn is_expression_statement_call(trimmed: &str) -> bool {
    if find_top_level_punctuation(trimmed, '[').is_some() {
        return false;
    }
    matches!(crate::expr::parse_expr(trimmed), Ok(Expr::Call(_)))
}

pub(super) fn parse_line_options(
    args: Option<(&str, usize)>,
    errors: &mut Vec<ParseError>,
) -> LineOptions {
    let Some((args, args_start_base)) = args else {
        return LineOptions::default();
    };
    let mut state = LineOptionsParseState::default();
    let mut consumed_positional_look = false;
    let mut search_start = 0usize;
    for arg in split_comma_args(args) {
        let arg_start = args[search_start..]
            .find(arg)
            .map_or(search_start, |relative| search_start + relative);
        let arg_source_base = args_start_base + arg_start;
        search_start = arg_start + arg.len();
        let Some((name, value)) = split_top_level_punctuation_once(arg, '=') else {
            if consumed_positional_look {
                errors.push(simple_error(
                    arg_source_base,
                    arg.len(),
                    "only the first positional dialogue line option may be used as `look`",
                    "look = expr",
                ));
                continue;
            }
            consumed_positional_look = true;
            let trimmed = arg.trim();
            let leading = arg.len() - arg.trim_start().len();
            state.look = Some(parse_line_option_expr(
                trimmed,
                arg_source_base + leading,
                errors,
            ));
            continue;
        };
        parse_named_line_option(
            &mut state,
            name,
            value,
            arg_source_base + subslice_offset(arg, value),
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
        view: state.view,
        source_locale: state.source_locale,
        hooks: state.hooks,
        style: state.style,
        style_raw: state.style_raw,
        style_range: state.style_range,
        rich_text: state.rich_text,
        rich_text_raw: state.rich_text_raw,
        rich_text_range: state.rich_text_range,
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
    view: Option<EntityRefSyntax>,
    source_locale: Option<String>,
    hooks: Vec<Expr>,
    style: Option<Expr>,
    style_raw: Option<String>,
    style_range: Option<TextRange>,
    rich_text: Option<Expr>,
    rich_text_raw: Option<String>,
    rich_text_range: Option<TextRange>,
    line_args: Vec<LineArg>,
}

fn parse_named_line_option(
    state: &mut LineOptionsParseState,
    name_raw: &str,
    value_raw: &str,
    value_start: usize,
    errors: &mut Vec<ParseError>,
) {
    let name = name_raw.trim();
    let value = value_raw.trim();
    let value_range = TextRange::new(value_start, value_start + value.len());
    match name {
        "id" => {
            state.id = parse_required_id_ref(value, value_start, errors).map(|(entity, _)| entity);
        }
        "text_key" => {
            state.text_key =
                parse_required_id_ref(value, value_start, errors).map(|(entity, _)| entity);
        }
        "voice" => state.voice = Some(parse_line_option_expr(value, value_start, errors)),
        "look" => state.look = Some(parse_line_option_expr(value, value_start, errors)),
        "stage" => state.stage = Some(parse_line_option_expr(value, value_start, errors)),
        "portrait" => state.portrait = Some(parse_line_option_expr(value, value_start, errors)),
        "focus" => state.focus = Some(parse_line_option_expr(value, value_start, errors)),
        "cleanup" => state.cleanup = Some(parse_line_option_expr(value, value_start, errors)),
        "view" => {
            state.view = parse_required_entity_ref_syntax(value, value_start, errors)
                .map(|(entity, _)| entity);
        }
        "source_locale" => state.source_locale = Some(value.to_owned()),
        "hooks" => push_line_hooks(
            &mut state.hooks,
            parse_line_option_expr(value, value_start, errors),
        ),
        "style" => {
            state.style = Some(parse_line_option_expr(value, value_start, errors));
            state.style_raw = Some(value.to_owned());
            state.style_range = Some(value_range);
        }
        "rich_text" => {
            state.rich_text = Some(parse_line_option_expr(value, value_start, errors));
            state.rich_text_raw = Some(value.to_owned());
            state.rich_text_range = Some(value_range);
        }
        name => state.line_args.push(LineArg::new(
            name.to_owned(),
            parse_line_option_expr(value, value_start, errors),
            value.to_owned(),
            value_range,
        )),
    }
}

fn parse_line_option_expr(source: &str, base: usize, errors: &mut Vec<ParseError>) -> Expr {
    parse_owned_expr_recovering(source, base, None, errors)
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

pub(super) struct LogicalBlockItem<'a> {
    pub(super) source: Cow<'a, str>,
    pub(super) base: usize,
}

pub(super) fn collect_logical_block_items(body: &str) -> Vec<Cow<'_, str>> {
    collect_logical_block_items_with_base(body, 0)
        .into_iter()
        .map(|item| item.source)
        .collect()
}

pub(super) fn collect_logical_block_items_with_base(
    body: &str,
    body_base: usize,
) -> Vec<LogicalBlockItem<'_>> {
    let mut lines: Vec<LogicalBlockItem<'_>> = Vec::new();
    let mut current: Option<(String, usize)> = None;
    let mut depth = 0_i32;
    let line_deltas = CstPunctuationScan::new(body).line_deltas(body);
    let mut line_base = body_base;

    for (line_index, line_with_ending) in body.split_inclusive('\n').enumerate() {
        let raw_line = if let Some(without_lf) = line_with_ending.strip_suffix('\n') {
            without_lf.strip_suffix('\r').unwrap_or(without_lf)
        } else {
            line_with_ending
        };
        let raw_line_base = line_base;
        line_base += line_with_ending.len();
        if raw_line.trim().is_empty() {
            continue;
        }
        let trimmed = raw_line.trim_start();
        let trimmed_base = raw_line_base + raw_line.len() - trimmed.len();
        // Method-chain continuation lines belong to the preceding logical item.
        // Without this, multi-line `Option.context(...)?` or `Need.context(...)`
        // parses the dot line as an unrelated raw flow item.
        if current.is_none()
            && trimmed.starts_with('.')
            && let Some(previous) = lines.pop()
        {
            current = Some((previous.source.into_owned(), previous.base));
        }
        if let Some((current, _)) = current.as_mut() {
            if !current.trim().is_empty() {
                current.push('\n');
            }
            current.push_str(raw_line);
        }
        let deltas = line_deltas.get(line_index).copied().unwrap_or_default();
        depth += deltas.brace + deltas.paren + deltas.bracket;
        if depth <= 0 {
            let item = current.take().map_or_else(
                || LogicalBlockItem {
                    source: Cow::Borrowed(trimmed),
                    base: trimmed_base,
                },
                |(source, base)| trimmed_logical_block_item(&source, base),
            );
            lines.push(item);
            depth = 0;
        } else if current.is_none() {
            current = Some((trimmed.to_owned(), trimmed_base));
        }
    }
    if let Some((current, base)) = current
        && !current.trim().is_empty()
    {
        lines.push(trimmed_logical_block_item(&current, base));
    }
    lines
}

fn trimmed_logical_block_item(source: &str, base: usize) -> LogicalBlockItem<'static> {
    let start_trim = source.len() - source.trim_start().len();
    let end = source.trim_end().len();
    LogicalBlockItem {
        source: Cow::Owned(source[start_trim..end].to_owned()),
        base: base + start_trim,
    }
}

pub(super) fn split_brace_item(source: &str) -> Option<(&str, &str)> {
    let punctuation = CstPunctuationScan::new(source);
    split_brace_item_with_scan(source, &punctuation)
}

pub(super) fn split_brace_item_with_scan<'a>(
    source: &'a str,
    punctuation: &CstPunctuationScan<'a>,
) -> Option<(&'a str, &'a str)> {
    let open = punctuation.find_top_level_punctuation('{')?;
    let close = punctuation.find_matching_punctuation(open, '{', '}')?;
    (source[close + '}'.len_utf8()..].trim().is_empty())
        .then(|| (source[..open].trim(), source[open + 1..close].trim()))
}

pub(super) struct SpeakerLineParts<'a> {
    pub(super) speaker: String,
    pub(super) arguments: Option<(String, std::ops::Range<usize>)>,
    pub(super) inline_content: &'a str,
    pub(super) inline_content_range: std::ops::Range<usize>,
    pub(super) head_range: std::ops::Range<usize>,
}

pub(super) fn split_speaker_line(trimmed: &str) -> Option<SpeakerLineParts<'_>> {
    let colon = find_top_level_colon(trimmed)?;
    if has_top_level_square(&trimmed[..colon])
        || contains_arcweft_punctuation(&trimmed[..colon], ArcweftPunctuation::ThinArrow)
    {
        return None;
    }
    let head = trimmed[..colon].trim();
    let head_start = trimmed[..colon].find(head).unwrap_or(0);
    let content_source = &trimmed[colon + 1..];
    let content_leading = content_source.len() - content_source.trim_start().len();
    let content = content_source.trim();
    if head.is_empty() || head.starts_with("cancel ") || head.starts_with("at(") {
        return None;
    }
    let (speaker, args) = split_call_head(head);
    let arguments = args.map(|(args, relative)| {
        let start = head_start + relative;
        let end = start + args.len();
        (args, start..end)
    });
    Some(SpeakerLineParts {
        speaker,
        arguments,
        inline_content: content,
        inline_content_range: colon + 1 + content_leading
            ..colon + 1 + content_leading + content.len(),
        head_range: head_start..head_start + head.len(),
    })
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

pub(super) fn split_call_head(head: &str) -> (String, Option<(String, usize)>) {
    let head = head.trim();
    if let Some((open, close)) = find_top_level_matching_punctuation(head, '(', ')')
        && head[close + ')'.len_utf8()..].trim().is_empty()
    {
        let args_source = &head[open + 1..close];
        let args_trimmed = args_source.trim();
        let args_leading = args_source.len() - args_source.trim_start().len();
        return (
            head[..open].trim().to_owned(),
            Some((
                args_trimmed.to_owned(),
                open + '('.len_utf8() + args_leading,
            )),
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
    errors: &mut Vec<ParseError>,
) -> LinePlan {
    attach_line_plan_label(parse_line_plan_body(style, body, range, errors), label)
}

pub(super) fn parse_line_plan_attachment_with_body_base(
    style: BlockStyle,
    body: &str,
    body_base: usize,
    range: TextRange,
    label: Option<String>,
    errors: &mut Vec<ParseError>,
) -> LinePlan {
    attach_line_plan_label(
        parse_line_plan_body_with_body_base(style, body, body_base, range, errors),
        label,
    )
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
    parse_expr_lossy_with_stats(source, None)
}

pub(super) fn parse_owned_expr_recovering(
    source: &str,
    base: usize,
    stats: Option<&mut SyntaxParseStats>,
    errors: &mut Vec<ParseError>,
) -> crate::expr::Expr {
    let trimmed = source.trim();
    match parse_expr_fragment_recovering_at(
        source,
        base,
        CallRecoveryBoundarySyntax::EndOfExpression,
    ) {
        Ok(parsed) => {
            if let Some(stats) = stats {
                if let Some(total) = stats
                    .numeric_seq_summaries
                    .checked_add(parsed.stats.numeric_seq_summaries())
                {
                    stats.numeric_seq_summaries = total;
                } else {
                    errors.push(ParseError::new(
                        parsed.range,
                        Vec::new(),
                        None,
                        "expression parse statistic overflowed".to_owned(),
                        Vec::new(),
                    ));
                    return crate::expr::Expr::Raw(trimmed.to_owned());
                }
            }
            for diagnostic in &parsed.diagnostics {
                retain_expr_recovery_diagnostic(diagnostic, errors);
            }
            parsed.expr
        }
        Err(error) => {
            errors.push(ParseError::from_expression(
                &error,
                vec!["expression".to_owned()],
            ));
            crate::expr::Expr::Raw(trimmed.to_owned())
        }
    }
}

pub(super) fn retain_expr_recovery_diagnostic(
    diagnostic: &crate::expr::ExprParseError,
    errors: &mut Vec<ParseError>,
) {
    let Some(recovery) = diagnostic.recovery_diagnostic() else {
        return;
    };
    let error = match recovery {
        ExprRecoveryDiagnostic::MissingCallClose { open_paren } => ParseError::new_with_kind(
            ParseErrorKind::from_expression(diagnostic),
            diagnostic.range(),
            vec![")".to_owned()],
            None,
            diagnostic.to_string(),
            vec![
                RecoverySuggestion::new("insert the missing `)`")
                    .with_edit(RecoveryEdit::new(diagnostic.range(), ")")),
            ],
        )
        .with_related(open_paren, Some("argument list opens here".to_owned())),
        ExprRecoveryDiagnostic::RecoveredCallArgument => ParseError::new_with_kind(
            ParseErrorKind::from_expression(diagnostic),
            diagnostic.range(),
            vec!["expression".to_owned()],
            None,
            diagnostic.to_string(),
            vec![RecoverySuggestion::new(
                "replace the malformed argument with a valid expression",
            )],
        ),
        ExprRecoveryDiagnostic::RecoveredTypeCallee => ParseError::new_with_kind(
            ParseErrorKind::from_expression(diagnostic),
            diagnostic.range(),
            vec!["callee".to_owned()],
            None,
            diagnostic.to_string(),
            vec![RecoverySuggestion::new(
                "replace the malformed callee with a valid expression",
            )],
        ),
    };
    errors.push(error);
}

pub(super) fn parse_expr_lossy_with_stats(
    source: &str,
    stats: Option<&mut SyntaxParseStats>,
) -> crate::expr::Expr {
    let source = source.trim();
    if let Some((head, body)) = split_brace_item(source) {
        let name = head.trim();
        if is_plain_block_callee(name) {
            return parse_named_block_expr(name, body);
        }
    }
    match parse_expr_with_stats(source) {
        Ok(parsed) => {
            if let Some(stats) = stats {
                stats.numeric_seq_summaries += parsed.stats.numeric_seq_summaries();
            }
            parsed.expr
        }
        Err(error) => {
            if error.contains_kind(crate::expr::ExprParseErrorKind::PrefixDepthLimit)
                && let Some(stats) = stats
            {
                stats.checked_add_prefix_depth_limit_failures(1);
            }
            crate::expr::Expr::Raw(source.to_owned())
        }
    }
}

pub(super) fn is_plain_block_callee(source: &str) -> bool {
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
                | "scope"
                | "defer"
                | "yield"
                | "signal"
                | "close"
                | "break"
                | "continue"
        )
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

pub(super) fn parse_expr_with_inline_line_plan_with_stats(
    source: &str,
    mut stats: Option<&mut SyntaxParseStats>,
) -> Expr {
    let Some((expr_source, trailing_plan)) = split_inline_dialogue_line_plan(source) else {
        if let Some(expr) = parse_dialogue_call_expr_surface(source, stats.as_deref_mut()) {
            return expr;
        }
        return parse_expr_lossy_with_stats(source, stats);
    };
    let Some(plan) = parse_inline_line_plan_source(trailing_plan) else {
        return parse_expr_lossy_with_stats(source, stats);
    };
    parse_dialogue_call_expr_source_with_plan(expr_source.trim(), Some(plan))
        .unwrap_or_else(|| parse_expr_lossy_with_stats(source, stats))
}

fn parse_dialogue_call_expr_surface(
    source: &str,
    stats: Option<&mut SyntaxParseStats>,
) -> Option<Expr> {
    if !looks_like_dialogue_call_expr_surface(source, stats) {
        return None;
    }
    parse_dialogue_call_expr_source(source)
}

fn looks_like_dialogue_call_expr_surface(
    source: &str,
    stats: Option<&mut SyntaxParseStats>,
) -> bool {
    let source = source.trim();
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
    if callee.contains('(') || !content_may_be_typed_expr(content) {
        return true;
    }
    if let Some(stats) = stats {
        stats.dialogue_rescue_expr_parse_attempts += 1;
    }
    parse_expr(content).is_err()
}

fn content_may_be_typed_expr(source: &str) -> bool {
    let Some(first) = source.trim_start().chars().next() else {
        return false;
    };
    first.is_ascii_alphanumeric()
        || matches!(first, '"' | '@' | '(' | '[' | '{' | '.' | '_' | '-' | '!')
}

pub(super) fn parse_dialogue_call_expr_source(source: &str) -> Option<Expr> {
    parse_dialogue_call_expr_source_with_plan(source, None)
}

fn parse_dialogue_call_expr_source_with_plan(source: &str, plan: Option<LinePlan>) -> Option<Expr> {
    let source = source.trim();
    let open = find_content_bracket(source)?;
    let close = find_matching_punctuation(source, open, '[', ']')?;
    if !source[close + 1..].trim().is_empty() {
        return None;
    }
    if source[..open].trim().is_empty() {
        return None;
    }
    let content = source[open + 1..close].trim();
    crate::expr::parse_dialogue_context_expr(
        source,
        open,
        close + ']'.len_utf8(),
        parse_dialogue_content(content),
        plan,
    )
    .ok()
    .map(|parsed| parsed.expr)
}

pub(super) fn attach_plan_to_dialogue_expr(expr: &mut Expr, line_plan: LinePlan) -> bool {
    match expr {
        Expr::DialogueCall { plan, .. } => {
            *plan = Some(line_plan);
            true
        }
        Expr::Try(try_expr) => attach_plan_to_dialogue_expr(try_expr.operand_mut(), line_plan),
        _ => false,
    }
}

pub(super) fn contains_dialogue_expr(expr: &Expr) -> bool {
    match expr {
        Expr::DialogueCall { .. } => true,
        Expr::Try(try_expr) => contains_dialogue_expr(try_expr.operand()),
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
            &mut Vec::new(),
        ));
    }
    parse_inline_with_colon_plan(source).map(|(label, body)| {
        parse_line_plan_attachment(
            BlockStyle::Indent,
            body,
            TextRange::new(0, source.len()),
            label,
            &mut Vec::new(),
        )
    })
}

pub(super) fn indentation(text: &str) -> usize {
    text.chars().take_while(|ch| ch.is_whitespace()).count()
}

#[cfg(test)]
mod tests {
    use super::{
        collect_logical_block_items, content_may_be_typed_expr,
        parse_expr_with_inline_line_plan_with_stats,
    };
    use crate::ast::common::TextRange;
    use crate::cst::SyntaxParseStats;
    use crate::expr::{Expr, collect_dialogue_call_content_ranges};
    use std::borrow::Cow;

    #[test]
    fn dialogue_rescue_skips_expression_parse_for_obvious_text() {
        assert!(!content_may_be_typed_expr("おはよう。[p]"));
        assert!(content_may_be_typed_expr("0"));
        assert!(content_may_be_typed_expr("name"));
    }

    #[test]
    fn dialogue_rescue_stats_count_only_expression_disambiguation_attempts() {
        let mut stats = SyntaxParseStats::default();
        let obvious =
            parse_expr_with_inline_line_plan_with_stats("alice[おはよう。[p]]", Some(&mut stats));
        assert!(matches!(obvious, Expr::DialogueCall { .. }));
        assert_eq!(stats.dialogue_rescue_expr_parse_attempts, 0);

        let indexed = parse_expr_with_inline_line_plan_with_stats("items[0]", Some(&mut stats));
        assert!(!matches!(indexed, Expr::DialogueCall { .. }));
        assert_eq!(stats.dialogue_rescue_expr_parse_attempts, 1);
    }

    #[test]
    fn general_try_wraps_the_typed_dialogue_primary() {
        let parsed = parse_expr_with_inline_line_plan_with_stats("try alice[おはよう。]", None);
        let Expr::Try(try_expr) = parsed else {
            panic!("expected typed general try expression");
        };
        assert_eq!(
            try_expr.source().whole(),
            crate::ast::common::TextRange::new(0, 26)
        );
        assert_eq!(
            try_expr.source().operand(),
            crate::ast::common::TextRange::new(4, 26)
        );
        assert!(matches!(try_expr.operand(), Expr::DialogueCall { .. }));
    }

    #[test]
    fn general_try_preserves_dialogue_content_after_a_nested_call_callee() {
        let source = "try render(\"[.shake]effect[/][p]\")()[[.shake]effect[/][p]]";
        let parsed = parse_expr_with_inline_line_plan_with_stats(source, None);
        let Expr::Try(try_expr) = &parsed else {
            panic!("expected typed general try expression, got {parsed:?}");
        };
        assert!(matches!(try_expr.operand(), Expr::DialogueCall { .. }));

        let document_base = 47;
        let ranges = collect_dialogue_call_content_ranges(
            &parsed,
            source,
            TextRange::new(document_base, document_base + source.len()),
        );
        assert_eq!(ranges.len(), 1, "dialogue content ranges: {ranges:?}");
        let relative = TextRange::new(
            ranges[0].start() - document_base,
            ranges[0].end() - document_base,
        );
        assert_eq!(&source[relative.as_range()], "[.shake]effect[/][p]");
    }

    #[test]
    fn logical_block_items_borrow_single_line_items() {
        let items = collect_logical_block_items("let a = 1\nlet b = 2");

        assert_eq!(
            items,
            [Cow::Borrowed("let a = 1"), Cow::Borrowed("let b = 2")]
        );
    }

    #[test]
    fn logical_block_items_own_multiline_items_only() {
        let items = collect_logical_block_items("let a = call(\n    1\n)\nlet b = 2");

        assert_eq!(items.len(), 2);
        assert!(matches!(items[0], Cow::Owned(_)));
        assert!(matches!(items[1], Cow::Borrowed("let b = 2")));
    }
}
