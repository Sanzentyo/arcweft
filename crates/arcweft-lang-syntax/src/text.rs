mod dialogue_opaque;
mod rich_text_tag;

pub(crate) use dialogue_opaque::scan_dialogue_opaque_surface;

pub use rich_text_tag::{
    DialogueTagBoundary, MAX_RICH_TEXT_CONTENT_ARGUMENTS, MAX_RICH_TEXT_CONTENT_TAGS,
    MAX_RICH_TEXT_TAG_ARGUMENTS, MAX_RICH_TEXT_TAG_BODY_BYTES, MAX_RICH_TEXT_TAG_KEY_BYTES,
    MAX_RICH_TEXT_TAG_VALUE_BYTES, find_dialogue_tag_boundary,
};
pub(crate) use rich_text_tag::{
    ScannedTagArgValue, ScannedTagArgValueSurface, ScannedTagArgument, ScannedTagArguments,
    find_dialogue_tag_boundary_before, is_rich_text_whitespace, scan_tag_arg_value,
    scan_tag_arguments, trim_rich_text_whitespace, utf8_boundary_at_or_before,
};
use rich_text_tag::{parse_tag, parse_tag_arguments, split_tag_name_attrs, tag_arg_value};

use crate::ast::{
    common::TextRange,
    dialogue::{
        DialogueEndTag, DialogueExpr, DialogueTag, DialogueTagArg, DialogueTagArgValueSurface,
        DialogueTagPayload, DialogueTagRanges, DialogueToken,
    },
};
use crate::expr::{CallArg, Expr, Literal, parse_expr};

/// Parsed dialogue-text tokens plus recoverable text-mode diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueTextParse {
    tokens: Vec<DialogueToken>,
    diagnostics: Vec<DialogueTextDiagnostic>,
}

/// A recoverable diagnostic produced while tokenizing dialogue text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueTextDiagnostic {
    code: DialogueTextDiagnosticCode,
    range: TextRange,
    message: String,
    recovery: String,
}

/// Stable syntax diagnostic identity for dialogue-text parsing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogueTextDiagnosticCode {
    DialogueText,
    RichTextAttributeUnterminatedQuote,
    RichTextAttributeInvalidEscape,
    RichTextAttributeEmptyKey,
    RichTextAttributeInvalidKey,
    RichTextAttributeMissingValue,
    RichTextTagBodyTooLong,
    RichTextAttributeTooMany,
    RichTextAttributeKeyTooLong,
    RichTextAttributeValueTooLong,
    RichTextContentTagLimit,
    RichTextContentArgumentLimit,
}

impl DialogueTextDiagnosticCode {
    /// Stable diagnostic code used by compiler and tooling layers.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DialogueText => "syntax.dialogue.text",
            Self::RichTextAttributeUnterminatedQuote => {
                "syntax.rich_text.attribute.unterminated_quote"
            }
            Self::RichTextAttributeInvalidEscape => "syntax.rich_text.attribute.invalid_escape",
            Self::RichTextAttributeEmptyKey => "syntax.rich_text.attribute.empty_key",
            Self::RichTextAttributeInvalidKey => "syntax.rich_text.attribute.invalid_key",
            Self::RichTextAttributeMissingValue => "syntax.rich_text.attribute.missing_value",
            Self::RichTextTagBodyTooLong => "syntax.rich_text.tag.body_too_long",
            Self::RichTextAttributeTooMany => "syntax.rich_text.attribute.too_many",
            Self::RichTextAttributeKeyTooLong => "syntax.rich_text.attribute.key_too_long",
            Self::RichTextAttributeValueTooLong => "syntax.rich_text.attribute.value_too_long",
            Self::RichTextContentTagLimit => "syntax.rich_text.content.tag_limit",
            Self::RichTextContentArgumentLimit => "syntax.rich_text.content.argument_limit",
        }
    }
}

/// Parses dialogue-text mode into tokens.
///
/// This tokenizer is deliberately permissive: malformed tags are kept as text
/// so the higher-level parser can continue and attach diagnostics to the
/// surrounding line.
pub fn parse_dialogue_tokens(source: &str) -> Vec<DialogueToken> {
    parse_dialogue_text(source).into_tokens()
}

/// Parses dialogue-text mode into tokens and recoverable diagnostics.
///
/// Dialogue text has its own markup surface. This parser keeps malformed
/// authoring sugar as literal text and emits diagnostics instead of losing
/// source, which keeps localization extraction and editor tooling stable.
#[must_use]
pub fn parse_dialogue_text(source: &str) -> DialogueTextParse {
    let mut output = DialogueTextAccumulator::default();
    let mut chars = source.char_indices().peekable();

    while let Some((index, ch)) = chars.next() {
        match ch {
            '\\' => {
                if let Some((_, escaped)) = chars.next() {
                    output.flush_text();
                    output.tokens.push(DialogueToken::Escape(escaped));
                } else {
                    output.text.push(ch);
                }
            }
            '|' => {
                if let Some((ruby_token, consumed_to)) = parse_ascii_explicit_ruby(source, index)
                    .or_else(|| parse_ascii_compact_ruby(source, index))
                {
                    output.flush_text();
                    output.tokens.push(ruby_token);
                    skip_to(&mut chars, consumed_to);
                } else {
                    if let Some(diagnostic) = compact_ruby_diagnostic(source, index) {
                        output.diagnostics.push(diagnostic);
                    }
                    output.text.push(ch);
                }
            }
            '｜' => {
                if let Some((ruby_token, consumed_to)) = parse_natural_ruby(source, index) {
                    output.flush_text();
                    output.tokens.push(ruby_token);
                    skip_to(&mut chars, consumed_to);
                } else {
                    output.text.push(ch);
                }
            }
            '#' if chars.peek().is_some_and(|(_, next)| *next == '[') => {
                let _ = chars.next();
                if let Some((expr, consumed_to)) = take_balanced_bracket(source, index + 2) {
                    output.flush_text();
                    output
                        .tokens
                        .push(parse_dialogue_expr_token(&expr, index + 2));
                    skip_to(&mut chars, consumed_to);
                } else {
                    output.text.push_str("#[");
                }
            }
            '$' if chars.peek().is_some_and(|(_, next)| *next == '(') => {
                let _ = chars.next();
                if let Some((expr, consumed_to)) = take_balanced_paren(source, index + 2) {
                    output.flush_text();
                    output
                        .tokens
                        .push(parse_dialogue_expr_token(&expr, index + 2));
                    skip_to(&mut chars, consumed_to);
                } else {
                    output.text.push_str("$(");
                }
            }
            '[' => {
                if let Some(consumed_to) = output.parse_open_tag(source, index) {
                    skip_to(&mut chars, consumed_to);
                } else {
                    output.text.push(ch);
                }
            }
            _ => output.text.push(ch),
        }
    }

    output.finish()
}

#[derive(Default)]
struct DialogueTextAccumulator {
    tokens: Vec<DialogueToken>,
    diagnostics: Vec<DialogueTextDiagnostic>,
    text: String,
    rich_text_tag_count: usize,
    rich_text_argument_count: usize,
    rich_text_tag_limit_exhausted: bool,
    rich_text_argument_limit_exhausted: bool,
}

#[derive(Clone, Copy)]
enum RichTextContentLimit {
    Tags,
    Arguments,
}

impl DialogueTextAccumulator {
    fn flush_text(&mut self) {
        flush_text(&mut self.text, &mut self.tokens);
    }

    fn finish(mut self) -> DialogueTextParse {
        self.flush_text();
        DialogueTextParse::new(self.tokens, self.diagnostics)
    }

    fn parse_open_tag(&mut self, source: &str, index: usize) -> Option<usize> {
        if self.rich_text_tag_limit_exhausted
            || self.rich_text_tag_count >= MAX_RICH_TEXT_CONTENT_TAGS
        {
            let consumed_to = find_dialogue_tag_boundary(source, index)?.end();
            return Some(self.retain_limited_markup(
                source,
                index,
                consumed_to,
                RichTextContentLimit::Tags,
            ));
        }
        if let Some((ruby, consumed_to)) = parse_bracket_ruby(source, index) {
            return Some(self.push_single_tag(ruby, consumed_to));
        }
        if let Some((raw, consumed_to)) = parse_raw_span(source, index) {
            return Some(self.push_single_tag(DialogueToken::Raw(raw), consumed_to));
        }
        if let Some((raw, consumed_to)) = parse_inline_raw_span(source, index) {
            return Some(self.push_single_tag(DialogueToken::Raw(raw), consumed_to));
        }
        if let Some((tokens, consumed_to)) = parse_inline_style_span(source, index) {
            return Some(self.push_inline_span(source, index, tokens, consumed_to));
        }

        let remaining =
            MAX_RICH_TEXT_CONTENT_ARGUMENTS.saturating_sub(self.rich_text_argument_count);
        let mut parsed = parse_tag(source, index, remaining)?;
        let reports_argument_limit = parsed.diagnostics.iter().any(|diagnostic| {
            diagnostic.code() == DialogueTextDiagnosticCode::RichTextContentArgumentLimit
        });
        if self.rich_text_argument_limit_exhausted {
            parsed.diagnostics.retain(|diagnostic| {
                diagnostic.code() != DialogueTextDiagnosticCode::RichTextContentArgumentLimit
            });
        } else if reports_argument_limit {
            self.rich_text_argument_limit_exhausted = true;
        }
        self.flush_text();
        self.diagnostics.extend(parsed.diagnostics);
        self.rich_text_tag_count += usize::from(is_rich_text_tag_token(&parsed.token));
        self.rich_text_argument_count +=
            dialogue_tag_argument_count(&parsed.token).unwrap_or_default();
        self.tokens.push(parsed.token);
        Some(parsed.consumed_to)
    }

    fn push_single_tag(&mut self, token: DialogueToken, consumed_to: usize) -> usize {
        self.flush_text();
        self.tokens.push(token);
        self.rich_text_tag_count += 1;
        consumed_to
    }

    fn push_inline_span(
        &mut self,
        source: &str,
        index: usize,
        tokens: Vec<DialogueToken>,
        consumed_to: usize,
    ) -> usize {
        let added_tags = tokens
            .iter()
            .filter(|token| is_rich_text_tag_token(token))
            .count();
        let added_arguments = tokens
            .iter()
            .filter_map(dialogue_tag_argument_count)
            .sum::<usize>();
        if added_tags > MAX_RICH_TEXT_CONTENT_TAGS.saturating_sub(self.rich_text_tag_count) {
            return self.retain_limited_markup(
                source,
                index,
                consumed_to,
                RichTextContentLimit::Tags,
            );
        }
        if added_arguments
            > MAX_RICH_TEXT_CONTENT_ARGUMENTS.saturating_sub(self.rich_text_argument_count)
        {
            return self.retain_limited_markup(
                source,
                index,
                consumed_to,
                RichTextContentLimit::Arguments,
            );
        }

        self.flush_text();
        self.rich_text_tag_count += added_tags;
        self.rich_text_argument_count += added_arguments;
        self.tokens.extend(tokens);
        consumed_to
    }

    fn retain_limited_markup(
        &mut self,
        source: &str,
        start: usize,
        end: usize,
        limit: RichTextContentLimit,
    ) -> usize {
        let (code, message, recovery, already_exhausted) = match limit {
            RichTextContentLimit::Tags => (
                DialogueTextDiagnosticCode::RichTextContentTagLimit,
                format!(
                    "dialogue content has more than {MAX_RICH_TEXT_CONTENT_TAGS} RichText tags"
                ),
                "split the dialogue content or remove excess tags",
                core::mem::replace(&mut self.rich_text_tag_limit_exhausted, true),
            ),
            RichTextContentLimit::Arguments => (
                DialogueTextDiagnosticCode::RichTextContentArgumentLimit,
                format!(
                    "dialogue content has more than {MAX_RICH_TEXT_CONTENT_ARGUMENTS} RichText arguments"
                ),
                "split the dialogue content or remove excess arguments",
                core::mem::replace(&mut self.rich_text_argument_limit_exhausted, true),
            ),
        };
        if !already_exhausted {
            self.diagnostics.push(DialogueTextDiagnostic::with_code(
                code,
                TextRange::new(start, end),
                message,
                recovery,
            ));
        }
        self.text.push_str(&source[start..end]);
        end
    }
}

fn is_rich_text_tag_token(token: &DialogueToken) -> bool {
    matches!(
        token,
        DialogueToken::Tag(_)
            | DialogueToken::InferredTag(_)
            | DialogueToken::Mark(_)
            | DialogueToken::EndTag(_)
            | DialogueToken::InferredEndTag
    )
}

fn dialogue_tag_argument_count(token: &DialogueToken) -> Option<usize> {
    match token {
        DialogueToken::Tag(tag) | DialogueToken::InferredTag(tag) => Some(tag.arguments().len()),
        DialogueToken::Text(_)
        | DialogueToken::Raw(_)
        | DialogueToken::Mark(_)
        | DialogueToken::EndTag(_)
        | DialogueToken::InferredEndTag
        | DialogueToken::Expr(_)
        | DialogueToken::Ruby { .. }
        | DialogueToken::Escape(_) => None,
    }
}

impl DialogueTextParse {
    fn new(tokens: Vec<DialogueToken>, diagnostics: Vec<DialogueTextDiagnostic>) -> Self {
        Self {
            tokens,
            diagnostics,
        }
    }

    /// Dialogue tokens emitted from text mode.
    pub fn tokens(&self) -> &[DialogueToken] {
        &self.tokens
    }

    /// Recoverable diagnostics found while tokenizing dialogue text.
    pub fn diagnostics(&self) -> &[DialogueTextDiagnostic] {
        &self.diagnostics
    }

    /// Consumes the parse result and returns only tokens.
    pub fn into_tokens(self) -> Vec<DialogueToken> {
        self.tokens
    }

    /// Consumes the parse result without discarding recoverable diagnostics.
    pub fn into_parts(self) -> (Vec<DialogueToken>, Vec<DialogueTextDiagnostic>) {
        (self.tokens, self.diagnostics)
    }
}

impl DialogueTextDiagnostic {
    fn new(range: TextRange, message: impl Into<String>, recovery: impl Into<String>) -> Self {
        Self::with_code(
            DialogueTextDiagnosticCode::DialogueText,
            range,
            message,
            recovery,
        )
    }

    fn with_code(
        code: DialogueTextDiagnosticCode,
        range: TextRange,
        message: impl Into<String>,
        recovery: impl Into<String>,
    ) -> Self {
        Self {
            code,
            range,
            message: message.into(),
            recovery: recovery.into(),
        }
    }

    /// Stable structured diagnostic identity.
    pub const fn code(&self) -> DialogueTextDiagnosticCode {
        self.code
    }

    /// Byte range relative to the dialogue source passed to the tokenizer.
    pub const fn range(&self) -> &TextRange {
        &self.range
    }

    /// Human-readable diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Suggested local recovery.
    pub fn recovery(&self) -> &str {
        &self.recovery
    }
}

fn flush_text(text: &mut String, tokens: &mut Vec<DialogueToken>) {
    if !text.is_empty() {
        tokens.push(DialogueToken::Text(core::mem::take(text)));
    }
}

fn skip_to(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>, consumed_to: usize) {
    while chars
        .peek()
        .is_some_and(|(offset, _)| *offset < consumed_to)
    {
        let _ = chars.next();
    }
}

fn parse_natural_ruby(source: &str, start: usize) -> Option<(DialogueToken, usize)> {
    let after_marker = start + '｜'.len_utf8();
    let tail = source.get(after_marker..)?;
    let open_relative = tail.find('《')?;
    let base = &tail[..open_relative];
    if base.is_empty() {
        return None;
    }
    let ruby_start = after_marker + open_relative + '《'.len_utf8();
    let ruby_tail = source.get(ruby_start..)?;
    let close_relative = ruby_tail.find('》')?;
    let ruby = &ruby_tail[..close_relative];
    if ruby.is_empty() {
        return None;
    }
    let consumed_to = ruby_start + close_relative + '》'.len_utf8();
    Some((
        DialogueToken::Ruby {
            base: base.to_owned(),
            ruby: ruby.to_owned(),
        },
        consumed_to,
    ))
}

fn parse_ascii_explicit_ruby(source: &str, start: usize) -> Option<(DialogueToken, usize)> {
    let after_marker = start + '|'.len_utf8();
    let tail = source.get(after_marker..)?;
    let base_tail = tail.strip_prefix('[')?;
    let base_end_relative = base_tail.find(']')?;
    let base = &base_tail[..base_end_relative];
    if base.is_empty() {
        return None;
    }
    let after_base = after_marker + '['.len_utf8() + base_end_relative + ']'.len_utf8();
    let ruby_tail = source.get(after_base..)?.strip_prefix('(')?;
    let ruby_end_relative = ruby_tail.find(')')?;
    let ruby = &ruby_tail[..ruby_end_relative];
    if ruby.is_empty() {
        return None;
    }
    let consumed_to = after_base + '('.len_utf8() + ruby_end_relative + ')'.len_utf8();
    Some((
        DialogueToken::Ruby {
            base: base.to_owned(),
            ruby: ruby.to_owned(),
        },
        consumed_to,
    ))
}

fn parse_ascii_compact_ruby(source: &str, start: usize) -> Option<(DialogueToken, usize)> {
    let after_marker = start + '|'.len_utf8();
    let tail = source.get(after_marker..)?;
    if tail.starts_with('[') {
        return None;
    }
    let open_relative = tail.find('{')?;
    let base = &tail[..open_relative];
    if !is_valid_compact_ruby_base(base) {
        return None;
    }
    let ruby_start = after_marker + open_relative + '{'.len_utf8();
    let ruby_tail = source.get(ruby_start..)?;
    let close_relative = ruby_tail.find('}')?;
    let ruby = &ruby_tail[..close_relative];
    if ruby.is_empty() {
        return None;
    }
    let consumed_to = ruby_start + close_relative + '}'.len_utf8();
    Some((
        DialogueToken::Ruby {
            base: base.to_owned(),
            ruby: ruby.to_owned(),
        },
        consumed_to,
    ))
}

fn is_valid_compact_ruby_base(base: &str) -> bool {
    !base.is_empty()
        && base
            .chars()
            .all(|ch| !ch.is_whitespace() && !matches!(ch, '[' | ']' | '{' | '}' | '#' | '|'))
}

fn compact_ruby_diagnostic(source: &str, start: usize) -> Option<DialogueTextDiagnostic> {
    let after_marker = start + '|'.len_utf8();
    let tail = source.get(after_marker..)?;
    if tail.starts_with('[') {
        return None;
    }
    let open_relative = tail.find('{')?;
    let close_relative = tail.get(open_relative + '{'.len_utf8()..)?.find('}')?;
    let end = after_marker + open_relative + '{'.len_utf8() + close_relative + '}'.len_utf8();
    let candidate = source.get(start..end)?;
    let base = &tail[..open_relative];
    (!is_valid_compact_ruby_base(base)).then(|| {
        DialogueTextDiagnostic::new(
            TextRange::new(start, end),
            format!("invalid compact ruby `{candidate}`"),
            "use `|[base](ruby)` when ruby base contains whitespace or reserved markup characters",
        )
    })
}

fn parse_bracket_ruby(source: &str, start: usize) -> Option<(DialogueToken, usize)> {
    let boundary = find_dialogue_tag_boundary(source, start)?;
    if boundary.unterminated_quote_start().is_some() {
        return None;
    }
    let after_open = boundary.end();
    let inside = trim_rich_text_whitespace(source.get(start + 1..boundary.close())?);
    let (tag_name, attrs) = split_tag_name_attrs(inside);
    if !matches!(tag_name, "ruby" | "rb") {
        return None;
    }
    let parsed_arguments = parse_tag_arguments(
        attrs,
        slice_offset(source, attrs),
        MAX_RICH_TEXT_CONTENT_ARGUMENTS,
    );
    if !parsed_arguments.diagnostics.is_empty() {
        return None;
    }
    let ruby = parsed_arguments
        .entries
        .iter()
        .find(|argument| argument.name() == Some("rt"))?
        .value()?
        .value()
        .to_owned();
    let tail = source.get(after_open..)?;
    let close_tag = format!("[/{tag_name}]");
    let close_relative = tail.find(&close_tag)?;
    let base_end = after_open + close_relative;
    let base = trim_rich_text_whitespace(source.get(after_open..base_end)?);
    if base.is_empty() {
        return None;
    }
    Some((
        DialogueToken::Ruby {
            base: base.to_owned(),
            ruby,
        },
        base_end + close_tag.len(),
    ))
}

fn parse_raw_span(source: &str, start: usize) -> Option<(String, usize)> {
    let raw_body_start = start + "[raw]".len();
    if !source.get(start..)?.starts_with("[raw]") {
        return None;
    }
    let tail = source.get(raw_body_start..)?;
    let close_relative = tail.find("[/raw]")?;
    let raw_body_end = raw_body_start + close_relative;
    let consumed_to = raw_body_end + "[/raw]".len();
    Some((source[raw_body_start..raw_body_end].to_owned(), consumed_to))
}

fn parse_inline_raw_span(source: &str, start: usize) -> Option<(String, usize)> {
    let tail = source.get(start..)?;
    tail.strip_prefix("[raw:")?;
    let (raw, consumed_to) = take_balanced_bracket(source, start + "[raw:".len())?;
    Some((raw.trim_start().to_owned(), consumed_to))
}

fn parse_inline_style_span(source: &str, start: usize) -> Option<(Vec<DialogueToken>, usize)> {
    let boundary = find_dialogue_tag_boundary(source, start)?;
    if boundary.unterminated_quote_start().is_some() {
        return None;
    }
    let consumed_to = boundary.end();
    let inside = trim_rich_text_whitespace(source.get(start + 1..boundary.close())?);
    let (tag_source, body) = split_once_top_level(inside, ':')?;
    if body.is_empty() {
        return None;
    }
    let tag_source = trim_rich_text_whitespace(tag_source);
    let (name, attrs, authored_value) = parse_inline_style_head(tag_source)?;
    let range = TextRange::new(start, consumed_to);
    let (arguments, attrs_range) = if let Some(value) = authored_value {
        let value_start = slice_offset(source, value);
        let value_range = TextRange::new(value_start, value_start + value.len());
        let value_surface = tag_arg_value(value, value_start).ok()?;
        (
            vec![DialogueTagArg::Positional {
                value: DialogueTagArgValueSurface::Present(value_surface),
                range: value_range,
            }],
            value_range,
        )
    } else {
        let end = start + 1 + tag_source.len();
        (Vec::new(), TextRange::new(end, end))
    };
    Some((
        vec![
            DialogueToken::Tag(DialogueTag::new(
                name.clone(),
                name.clone(),
                attrs,
                if arguments.is_empty() {
                    DialogueTagPayload::None
                } else {
                    DialogueTagPayload::Arguments
                },
                arguments,
                DialogueTagRanges::new(
                    TextRange::new(
                        slice_offset(source, tag_source),
                        slice_offset(source, tag_source) + name.len(),
                    ),
                    range,
                    attrs_range,
                ),
            )),
            DialogueToken::Text(body.to_owned()),
            DialogueToken::EndTag(DialogueEndTag::synthetic(name, boundary.close())),
        ],
        consumed_to,
    ))
}

fn parse_inline_style_head(source: &str) -> Option<(String, String, Option<&str>)> {
    if matches!(source, "em" | "strong") {
        return Some((source.to_owned(), String::new(), None));
    }
    let value = trim_rich_text_whitespace(source.strip_prefix("color ")?);
    (!value.is_empty()).then(|| {
        let attrs = if (value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\''))
        {
            format!("value={value}")
        } else {
            format!("value=\"{value}\"")
        };
        ("color".to_owned(), attrs, Some(value))
    })
}

fn take_balanced_bracket(source: &str, start: usize) -> Option<(String, usize)> {
    let mut depth = 1_u32;
    for (relative, ch) in source.get(start..)?.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    let cursor = start + relative;
                    return Some((source[start..cursor].to_owned(), cursor + 1));
                }
            }
            _ => {}
        }
    }
    None
}

fn take_balanced_paren(source: &str, start: usize) -> Option<(String, usize)> {
    let mut depth = 1_u32;
    for (relative, ch) in source.get(start..)?.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    let cursor = start + relative;
                    return Some((source[start..cursor].to_owned(), cursor + 1));
                }
            }
            _ => {}
        }
    }
    None
}

fn slice_offset(source: &str, slice: &str) -> usize {
    (slice.as_ptr() as usize).saturating_sub(source.as_ptr() as usize)
}

fn split_once_top_level(source: &str, needle: char) -> Option<(&str, &str)> {
    let mut bracket = 0_u32;
    let mut paren = 0_u32;
    let mut brace = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    for (index, ch) in source.char_indices() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '[' => bracket += 1,
            ']' => bracket = bracket.saturating_sub(1),
            '(' => paren += 1,
            ')' => paren = paren.saturating_sub(1),
            '{' => brace += 1,
            '}' => brace = brace.saturating_sub(1),
            _ if ch == needle && bracket == 0 && paren == 0 && brace == 0 => {
                return Some((&source[..index], &source[index + ch.len_utf8()..]));
            }
            _ => {}
        }
    }
    None
}

fn parse_dialogue_expr_lossy(source: &str) -> Expr {
    parse_expr(source).unwrap_or_else(|_| Expr::Raw(source.to_owned()))
}

fn parse_dialogue_expr_token(source: &str, absolute_start: usize) -> DialogueToken {
    let (expr_source, range) = trimmed_expr_source(source, absolute_start);
    let expr = parse_dialogue_expr_lossy(expr_source);
    function_ruby_token(&expr).unwrap_or_else(|| {
        DialogueToken::Expr(DialogueExpr::new(expr, expr_source.to_owned(), range))
    })
}

fn trimmed_expr_source(source: &str, absolute_start: usize) -> (&str, TextRange) {
    let leading = source.len() - source.trim_start().len();
    let trimmed = source.trim();
    let start = absolute_start + leading;
    let end = start + trimmed.len();
    (trimmed, TextRange::new(start, end))
}

fn function_ruby_token(expr: &Expr) -> Option<DialogueToken> {
    let Expr::Call(call) = expr else {
        return None;
    };
    if !matches!(call.callee(), Expr::Path(path) if path == "ruby") {
        return None;
    }
    let [
        CallArg::Positional(Expr::Literal(Literal::String(base))),
        CallArg::Positional(Expr::Literal(Literal::String(ruby))),
    ] = call.args()
    else {
        return None;
    };
    Some(DialogueToken::Ruby {
        base: base.to_owned(),
        ruby: ruby.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use arcweft_presentation::rich_text::{
        BuiltinRichTextFx, RichTextTagFamily, inferred_tag_family,
    };

    use super::{find_dialogue_tag_boundary, parse_dialogue_text};
    use crate::ast::{
        common::TextRange,
        dialogue::{DialogueTagKind, DialogueToken},
    };

    #[test]
    fn every_builtin_selector_is_an_effect_with_or_without_attributes() {
        for effect in BuiltinRichTextFx::ALL {
            for attrs in ["", "phase=glyph_transform"] {
                assert_eq!(
                    inferred_tag_family(effect.selector(), attrs),
                    Some(RichTextTagFamily::Effect),
                    "{} with `{attrs}`",
                    effect.selector()
                );
            }
        }
        assert_eq!(inferred_tag_family("unknown", ""), None);
    }

    #[test]
    fn dialogue_tags_expose_language_owned_semantic_kinds() {
        let parsed = parse_dialogue_text("[fx notice()]x[/fx][reset][p]");
        let kinds = parsed
            .tokens()
            .iter()
            .filter_map(|token| match token {
                DialogueToken::Tag(tag) => Some(tag.kind()),
                DialogueToken::EndTag(tag) => Some(tag.kind()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                DialogueTagKind::Fx,
                DialogueTagKind::Fx,
                DialogueTagKind::Reset,
                DialogueTagKind::Point,
            ]
        );
    }

    #[test]
    fn parses_dialogue_tag_arguments_with_absolute_ranges() {
        let source = "A[effect .warning amplitude=4px mood=\"very urgent\"]text[/effect]";
        let parsed = parse_dialogue_text(source);
        assert_eq!(parsed.diagnostics(), &[]);

        let tag = parsed
            .tokens()
            .iter()
            .find_map(|token| match token {
                DialogueToken::Tag(tag) if tag.name() == "effect" => Some(tag),
                _ => None,
            })
            .expect("effect tag");
        assert_eq!(
            &source[tag.range().as_range()],
            "[effect .warning amplitude=4px mood=\"very urgent\"]"
        );
        assert_eq!(
            &source[tag.attrs_range().as_range()],
            ".warning amplitude=4px mood=\"very urgent\""
        );
        assert_eq!(tag.arguments().len(), 3);
        assert_eq!(tag.arguments()[0].name(), None);
        assert_eq!(
            tag.arguments()[0].value().expect("selector value").value(),
            ".warning"
        );
        assert_eq!(tag.arguments()[1].name(), Some("amplitude"));
        assert_eq!(
            tag.arguments()[1].value().expect("amplitude value").value(),
            "4px"
        );
        assert_eq!(tag.arguments()[2].name(), Some("mood"));
        assert_eq!(
            tag.arguments()[2].value().expect("mood value").source(),
            "\"very urgent\""
        );
        assert_eq!(
            tag.arguments()[2].value().expect("mood value").value(),
            "very urgent"
        );
        let value_range = tag.arguments()[2].value().expect("mood value").range();
        assert_eq!(&source[value_range.as_range()], "\"very urgent\"");

        let end = parsed
            .tokens()
            .iter()
            .find_map(|token| match token {
                DialogueToken::EndTag(end) => Some(end),
                _ => None,
            })
            .expect("effect end tag");
        assert_eq!(end.name(), "effect");
        assert!(!end.is_synthetic());
        assert_eq!(&source[end.range().as_range()], "[/effect]");
    }

    #[test]
    fn named_argument_value_range_starts_after_the_assignment() {
        let source = "[effect .warning accent=accent]text[/effect]";
        let parsed = parse_dialogue_text(source);

        assert_eq!(parsed.diagnostics(), &[]);
        let Some(DialogueToken::Tag(tag)) = parsed.tokens().first() else {
            panic!("effect tag");
        };
        let argument = &tag.arguments()[1];
        let name_range = argument.name_range().expect("authored name range");
        let value_range = argument.value().expect("named value").range();
        assert_eq!(&source[name_range.as_range()], "accent");
        assert_eq!(&source[value_range.as_range()], "accent");
        assert!(name_range.end() < value_range.start());
    }

    #[test]
    fn reports_unterminated_dialogue_tag_quotes_and_recovers_the_tag() {
        let source = "[effect .warning mood=\"very urgent]text";
        let parsed = parse_dialogue_text(source);

        assert_eq!(parsed.diagnostics().len(), 1);
        assert!(
            parsed.diagnostics()[0]
                .message()
                .contains("unterminated quote")
        );
        let diagnostic_range = parsed.diagnostics()[0].range();
        assert_eq!(&source[diagnostic_range.as_range()], "\"very urgent]");
        assert!(matches!(
            parsed.tokens().first(),
            Some(DialogueToken::Tag(tag)) if tag.name() == "effect"
        ));
    }

    #[test]
    fn quoted_closing_brackets_do_not_end_dialogue_tags() {
        let source = "[effect .warning note=\"contains ] safely\"]text[/effect]";
        let boundary = find_dialogue_tag_boundary(source, 0).expect("tag boundary");
        assert_eq!(&source[boundary.close()..boundary.end()], "]");
        assert_eq!(
            &source[..boundary.end()],
            "[effect .warning note=\"contains ] safely\"]"
        );
        assert_eq!(boundary.unterminated_quote_start(), None);

        let parsed = parse_dialogue_text(source);

        assert_eq!(parsed.diagnostics(), &[]);
        let Some(DialogueToken::Tag(tag)) = parsed.tokens().first() else {
            panic!("effect tag");
        };
        assert_eq!(&source[tag.name_range().as_range()], "effect");
        assert_eq!(
            tag.arguments()[1].value().expect("note value").value(),
            "contains ] safely"
        );
    }

    #[test]
    fn bracket_ruby_allows_a_closing_bracket_in_the_quoted_reading() {
        let source = "[ruby rt=\"a]b\"]base[/ruby]";
        let parsed = parse_dialogue_text(source);

        assert_eq!(parsed.diagnostics(), &[]);
        assert_eq!(
            parsed.tokens(),
            &[DialogueToken::Ruby {
                base: "base".to_owned(),
                ruby: "a]b".to_owned(),
            }]
        );
    }

    #[test]
    fn short_color_span_ignores_delimiters_inside_a_quoted_value() {
        let source = "[color \"a]b:c\":text]";
        let parsed = parse_dialogue_text(source);

        assert_eq!(parsed.diagnostics(), &[]);
        let Some(DialogueToken::Tag(tag)) = parsed.tokens().first() else {
            panic!("color tag");
        };
        assert_eq!(tag.attrs(), "value=\"a]b:c\"");
        assert_eq!(
            tag.arguments()[0].value().expect("color value").source(),
            "\"a]b:c\""
        );
        assert_eq!(
            tag.arguments()[0].value().expect("color value").value(),
            "a]b:c"
        );
        assert!(matches!(
            parsed.tokens().get(1),
            Some(DialogueToken::Text(text)) if text == "text"
        ));
        assert!(matches!(
            parsed.tokens().get(2),
            Some(DialogueToken::EndTag(end)) if end.is_synthetic() && end.name() == "color"
        ));
    }

    #[test]
    fn unterminated_later_quote_recovers_at_its_own_closing_bracket() {
        let source = "[effect .warning note=\"safe ] here\" mood=\"unterminated]text";
        let parsed = parse_dialogue_text(source);

        assert_eq!(parsed.diagnostics().len(), 1);
        let diagnostic = &parsed.diagnostics()[0];
        assert_eq!(
            &source[diagnostic.range().start()..diagnostic.range().end()],
            "\"unterminated]"
        );
        let Some(DialogueToken::Tag(tag)) = parsed.tokens().first() else {
            panic!("recovered effect tag");
        };
        assert_eq!(
            tag.arguments()[1].value().expect("safe note value").value(),
            "safe ] here"
        );
        assert!(tag.arguments()[2].value().is_none());
        assert!(
            matches!(parsed.tokens().get(1), Some(DialogueToken::Text(text)) if text == "text")
        );
    }

    #[test]
    fn short_color_span_argument_range_uses_authored_color_value() {
        let source = "[color #a8b5ff:night]";
        let parsed = parse_dialogue_text(source);

        assert_eq!(parsed.diagnostics(), &[]);
        let Some(DialogueToken::Tag(tag)) = parsed.tokens().first() else {
            panic!("color tag");
        };
        assert_eq!(tag.attrs(), "value=\"#a8b5ff\"");
        assert_eq!(tag.arguments()[0].name(), None);
        assert_eq!(tag.arguments()[0].name_range(), None);
        let value = tag.arguments()[0].value().expect("short color value");
        assert_eq!(value.source(), "#a8b5ff");
        assert_eq!(&source[value.range().as_range()], "#a8b5ff");
        let Some(DialogueToken::EndTag(end)) = parsed.tokens().get(2) else {
            panic!("synthetic color end tag");
        };
        assert!(end.is_synthetic());
        assert_eq!(
            end.range(),
            TextRange::new(source.len() - 1, source.len() - 1)
        );
    }
}
