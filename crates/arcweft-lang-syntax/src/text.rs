use crate::ast::{
    common::TextRange,
    dialogue::{
        DialogueEndTag, DialogueExpr, DialogueTag, DialogueTagArg, DialogueTagArgValue,
        DialogueToken, LineMark,
    },
};
use crate::expr::{CallArg, Expr, Literal, parse_expr};

pub use arcweft_dialogue::rich_text::canonical_tag_name as canonical_rich_text_tag_name;

/// Parsed dialogue-text tokens plus recoverable text-mode diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueTextParse {
    tokens: Vec<DialogueToken>,
    diagnostics: Vec<DialogueTextDiagnostic>,
}

/// Quote-aware closing boundary of one authored dialogue tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialogueTagBoundary {
    close: usize,
    unterminated_quote_start: Option<usize>,
}

/// Rich-text family inferred from a dot-selector dialogue tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichTextTagFamily {
    /// Presentation style such as italic or opacity.
    Style,
    /// Writing-mode or ruby layout.
    Layout,
    /// Post-layout visual transform.
    Transform,
    /// Registry-extensible visual effect.
    Effect,
    /// Zero-width line marker.
    Marker,
}

/// Resolves the canonical family of an inferred dot-selector tag.
pub fn inferred_rich_text_tag_family(selector: &str, attrs: &str) -> Option<RichTextTagFamily> {
    match selector {
        "italic" | "oblique" | "opacity" | "alpha" | "layer" | "object_layer" | "meta"
        | "metadata" | "data" | "z" | "z_index" => Some(RichTextTagFamily::Style),
        "horizontal_tb"
        | "vertical_rl"
        | "vertical_lr"
        | "dir"
        | "ruby_over"
        | "ruby_under"
        | "ruby_inter_character" => Some(RichTextTagFamily::Layout),
        "offset" | "pos" | "rotate" | "scale" | "skew" => Some(RichTextTagFamily::Transform),
        "wave" | "shake" | "arc" | "spin" | "pulse" | "motion" | "typewriter" | "jitter"
        | "shader" | "host" => Some(RichTextTagFamily::Effect),
        "mark" => Some(RichTextTagFamily::Marker),
        _ if !attrs.trim().is_empty() => Some(RichTextTagFamily::Effect),
        _ => None,
    }
}

impl DialogueTagBoundary {
    /// Byte offset of the closing `]`.
    pub const fn close(&self) -> usize {
        self.close
    }

    /// Exclusive byte offset immediately after the closing `]`.
    pub const fn end(&self) -> usize {
        self.close + ']'.len_utf8()
    }

    /// Opening quote offset when recovery selected a `]` inside an
    /// unterminated quoted argument.
    pub const fn unterminated_quote_start(&self) -> Option<usize> {
        self.unterminated_quote_start
    }
}

/// A recoverable diagnostic produced while tokenizing dialogue text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueTextDiagnostic {
    range: TextRange,
    message: String,
    recovery: String,
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
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut text = String::new();
    let mut chars = source.char_indices().peekable();

    while let Some((index, ch)) = chars.next() {
        match ch {
            '\\' => {
                if let Some((_, escaped)) = chars.next() {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(DialogueToken::Escape(escaped));
                } else {
                    text.push(ch);
                }
            }
            '|' => {
                if let Some((ruby_token, consumed_to)) = parse_ascii_explicit_ruby(source, index)
                    .or_else(|| parse_ascii_compact_ruby(source, index))
                {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(ruby_token);
                    skip_to(&mut chars, consumed_to);
                } else {
                    if let Some(diagnostic) = compact_ruby_diagnostic(source, index) {
                        diagnostics.push(diagnostic);
                    }
                    text.push(ch);
                }
            }
            '｜' => {
                if let Some((ruby_token, consumed_to)) = parse_natural_ruby(source, index) {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(ruby_token);
                    skip_to(&mut chars, consumed_to);
                } else {
                    text.push(ch);
                }
            }
            '#' if chars.peek().is_some_and(|(_, next)| *next == '[') => {
                let _ = chars.next();
                if let Some((expr, consumed_to)) = take_balanced_bracket(source, index + 2) {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(parse_dialogue_expr_token(&expr, index + 2));
                    skip_to(&mut chars, consumed_to);
                } else {
                    text.push_str("#[");
                }
            }
            '$' if chars.peek().is_some_and(|(_, next)| *next == '(') => {
                let _ = chars.next();
                if let Some((expr, consumed_to)) = take_balanced_paren(source, index + 2) {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(parse_dialogue_expr_token(&expr, index + 2));
                    skip_to(&mut chars, consumed_to);
                } else {
                    text.push_str("$(");
                }
            }
            '[' => {
                if let Some((ruby, consumed_to)) = parse_bracket_ruby(source, index) {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(ruby);
                    skip_to(&mut chars, consumed_to);
                    continue;
                }
                if let Some((raw, consumed_to)) = parse_raw_span(source, index) {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(DialogueToken::Raw(raw));
                    skip_to(&mut chars, consumed_to);
                    continue;
                }
                if let Some((raw, consumed_to)) = parse_inline_raw_span(source, index) {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(DialogueToken::Raw(raw));
                    skip_to(&mut chars, consumed_to);
                    continue;
                }
                if let Some((span_tokens, consumed_to)) = parse_inline_style_span(source, index) {
                    flush_text(&mut text, &mut tokens);
                    tokens.extend(span_tokens);
                    skip_to(&mut chars, consumed_to);
                    continue;
                }
                if let Some(parsed_tag) = parse_tag(source, index) {
                    flush_text(&mut text, &mut tokens);
                    if let Some(diagnostic) = parsed_tag.diagnostic {
                        diagnostics.push(diagnostic);
                    }
                    tokens.push(parsed_tag.token);
                    skip_to(&mut chars, parsed_tag.consumed_to);
                } else {
                    text.push(ch);
                }
            }
            _ => text.push(ch),
        }
    }

    flush_text(&mut text, &mut tokens);
    DialogueTextParse::new(tokens, diagnostics)
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
        Self {
            range,
            message: message.into(),
            recovery: recovery.into(),
        }
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
    let inside = source.get(start + 1..boundary.close())?.trim();
    let (tag_name, attrs) = split_tag_name_attrs(inside);
    if !matches!(tag_name, "ruby" | "rb") {
        return None;
    }
    let (arguments, unterminated_quote) = parse_tag_arguments(attrs, slice_offset(source, attrs));
    if unterminated_quote.is_some() {
        return None;
    }
    let ruby = arguments
        .iter()
        .find(|argument| argument.name() == Some("rt"))?
        .value()
        .value()
        .to_owned();
    let tail = source.get(after_open..)?;
    let close_tag = format!("[/{tag_name}]");
    let close_relative = tail.find(&close_tag)?;
    let base_end = after_open + close_relative;
    let base = source.get(after_open..base_end)?.trim();
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
    let inside = source.get(start + 1..boundary.close())?.trim();
    let (tag_source, body) = split_once_top_level(inside, ':')?;
    if body.is_empty() {
        return None;
    }
    let (name, attrs, authored_value) = parse_inline_style_head(tag_source.trim())?;
    let range = TextRange::new(start, consumed_to);
    let (arguments, attrs_range) = authored_value.map_or_else(
        || {
            let end = start + 1 + tag_source.len();
            (Vec::new(), TextRange::new(end, end))
        },
        |value| {
            let value_start = slice_offset(source, value);
            let value_range = TextRange::new(value_start, value_start + value.len());
            (
                vec![DialogueTagArg::Named {
                    name: "value".to_owned(),
                    name_range: None,
                    value: DialogueTagArgValue::new(
                        value.to_owned(),
                        unquote_tag_arg(value),
                        value_range,
                    ),
                }],
                value_range,
            )
        },
    );
    Some((
        vec![
            DialogueToken::Tag(DialogueTag::new(
                name.clone(),
                TextRange::new(
                    slice_offset(source, tag_source.trim()),
                    slice_offset(source, tag_source.trim()) + name.len(),
                ),
                attrs,
                arguments,
                range,
                attrs_range,
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
    let value = source.strip_prefix("color ")?.trim();
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

struct ParsedTag {
    token: DialogueToken,
    consumed_to: usize,
    diagnostic: Option<DialogueTextDiagnostic>,
}

struct OpenTagContext {
    inside_start: usize,
    consumed_to: usize,
    range: TextRange,
    diagnostic: Option<DialogueTextDiagnostic>,
}

fn parse_tag(source: &str, open: usize) -> Option<ParsedTag> {
    let boundary = find_dialogue_tag_boundary(source, open)?;
    let close = boundary.close();
    let unterminated_quote = boundary.unterminated_quote_start();
    let inside_source = &source[open + '['.len_utf8()..close];
    let inside = inside_source.trim();
    let inside_leading = inside_source.len() - inside_source.trim_start().len();
    let inside_start = open + '['.len_utf8() + inside_leading;
    let consumed_to = close + ']'.len_utf8();
    let range = TextRange::new(open, consumed_to);
    let diagnostic = unterminated_quote.map(|quote_start| {
        DialogueTextDiagnostic::new(
            TextRange::new(quote_start, consumed_to),
            "unterminated quote in dialogue tag arguments",
            "close the quoted tag argument before `]`",
        )
    });
    if let Some(name) = inside.strip_prefix('/') {
        let name = name.trim();
        return Some(ParsedTag {
            token: if name.is_empty() {
                DialogueToken::InferredEndTag
            } else {
                DialogueToken::EndTag(DialogueEndTag::new(name.to_owned(), range))
            },
            consumed_to,
            diagnostic,
        });
    }

    (!inside.is_empty()).then_some(())?;
    Some(parse_open_tag(
        inside,
        OpenTagContext {
            inside_start,
            consumed_to,
            range,
            diagnostic,
        },
    ))
}

fn parse_open_tag(inside: &str, context: OpenTagContext) -> ParsedTag {
    if inside.starts_with('.') {
        let (selector, attrs) = split_tag_name_attrs(inside);
        return parsed_dialogue_tag(selector, attrs, attrs.to_owned(), true, inside, context);
    }
    if let Some(attrs) = inside.strip_prefix('!') {
        let attrs = attrs.trim();
        return parsed_dialogue_tag("call", attrs, attrs.to_owned(), false, inside, context);
    }
    let (name, attrs) = split_tag_name_attrs(inside);
    if name == "mark" && !attrs.is_empty() {
        return ParsedTag {
            token: DialogueToken::Mark(LineMark::new(attrs.to_owned())),
            consumed_to: context.consumed_to,
            diagnostic: context.diagnostic,
        };
    }
    if name == "w" && !attrs.is_empty() && !attrs.contains('=') {
        return parsed_dialogue_tag(name, attrs, format!("time={attrs}"), false, inside, context);
    }
    let (name, attrs) = normalize_tag_alias(name, attrs);
    parsed_dialogue_tag(name, attrs, attrs.to_owned(), false, inside, context)
}

fn parsed_dialogue_tag(
    name: &str,
    authored_attrs: &str,
    stored_attrs: String,
    inferred: bool,
    inside: &str,
    context: OpenTagContext,
) -> ParsedTag {
    let attrs_start = context.inside_start + tag_attrs_offset(inside, authored_attrs);
    let attrs_range = TextRange::new(attrs_start, attrs_start + authored_attrs.len());
    let (arguments, argument_quote) = parse_tag_arguments(authored_attrs, attrs_start);
    let tag = DialogueTag::new(
        name.to_owned(),
        TextRange::new(
            context.inside_start,
            context.inside_start + split_tag_name_attrs(inside).0.len(),
        ),
        stored_attrs,
        arguments,
        context.range,
        attrs_range,
    );
    ParsedTag {
        token: if inferred {
            DialogueToken::InferredTag(tag)
        } else {
            DialogueToken::Tag(tag)
        },
        consumed_to: context.consumed_to,
        diagnostic: context
            .diagnostic
            .or_else(|| argument_quote.map(tag_quote_diagnostic)),
    }
}

/// Finds the quote-aware closing boundary for the tag beginning at `open`.
///
/// A `]` inside a matching single- or double-quoted argument does not close
/// the tag. If a quote remains unterminated, the first `]` inside that quote is
/// returned as a recovery boundary together with the quote's byte offset.
#[must_use]
pub fn find_dialogue_tag_boundary(source: &str, open: usize) -> Option<DialogueTagBoundary> {
    source.get(open..)?.starts_with('[').then_some(())?;
    let start = open + '['.len_utf8();
    let mut quote = None;
    let mut quote_start = None;
    let mut escaped = false;
    let mut quoted_close = None;
    for (relative, ch) in source.get(start..)?.char_indices() {
        let index = start + relative;
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active {
                quote = None;
                quote_start = None;
                quoted_close = None;
            } else if ch == ']' && quoted_close.is_none() {
                quoted_close = Some(index);
            }
            continue;
        }
        if matches!(ch, '"' | '\'') {
            quote = Some(ch);
            quote_start = Some(index);
            quoted_close = None;
        } else if ch == ']' {
            return Some(DialogueTagBoundary {
                close: index,
                unterminated_quote_start: None,
            });
        }
    }
    quoted_close.map(|close| DialogueTagBoundary {
        close,
        unterminated_quote_start: quote_start,
    })
}

fn parse_tag_arguments(source: &str, base: usize) -> (Vec<DialogueTagArg>, Option<TextRange>) {
    let mut arguments = Vec::new();
    let mut cursor = 0;
    let mut unterminated_quote = None;
    while cursor < source.len() {
        cursor += source[cursor..]
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .map(char::len_utf8)
            .sum::<usize>();
        if cursor >= source.len() {
            break;
        }
        let start = cursor;
        let mut quote = None;
        let mut quote_start = None;
        let mut escaped = false;
        for (relative, ch) in source[start..].char_indices() {
            let index = start + relative;
            if let Some(active) = quote {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == active {
                    quote = None;
                    quote_start = None;
                }
            } else if matches!(ch, '"' | '\'') {
                quote = Some(ch);
                quote_start = Some(index);
            } else if ch.is_whitespace() {
                cursor = index;
                break;
            }
            cursor = index + ch.len_utf8();
        }
        if let Some(quote_start) = quote_start {
            unterminated_quote = Some(TextRange::new(base + quote_start, base + source.len()));
        }
        let argument_source = &source[start..cursor];
        if argument_source.is_empty() {
            continue;
        }
        let assignment = unquoted_assignment(argument_source);
        let argument = if let Some(equal) = assignment {
            let name_head = &argument_source[..equal];
            let name_source = name_head.trim();
            let name_offset = name_head.len() - name_head.trim_start().len();
            let value_start = equal + '='.len_utf8();
            let value_tail = &argument_source[value_start..];
            let value_source = value_tail.trim();
            let value_offset = value_start + value_tail.len() - value_tail.trim_start().len();
            DialogueTagArg::Named {
                name: name_source.to_owned(),
                name_range: Some(TextRange::new(
                    base + start + name_offset,
                    base + start + name_offset + name_source.len(),
                )),
                value: tag_arg_value(value_source, base + start + value_offset),
            }
        } else {
            DialogueTagArg::Positional {
                value: tag_arg_value(argument_source, base + start),
            }
        };
        arguments.push(argument);
    }
    (arguments, unterminated_quote)
}

fn unquoted_assignment(source: &str) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    source.char_indices().find_map(|(index, ch)| {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active {
                quote = None;
            }
            return None;
        }
        if matches!(ch, '"' | '\'') {
            quote = Some(ch);
            None
        } else {
            (ch == '=').then_some(index)
        }
    })
}

fn tag_arg_value(source: &str, start: usize) -> DialogueTagArgValue {
    DialogueTagArgValue::new(
        source.to_owned(),
        unquote_tag_arg(source),
        TextRange::new(start, start + source.len()),
    )
}

fn unquote_tag_arg(source: &str) -> String {
    source
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            source
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(source)
        .to_owned()
}

fn tag_quote_diagnostic(range: TextRange) -> DialogueTextDiagnostic {
    DialogueTextDiagnostic::new(
        range,
        "unterminated quote in dialogue tag arguments",
        "close the quoted tag argument before `]`",
    )
}

fn slice_offset(source: &str, slice: &str) -> usize {
    (slice.as_ptr() as usize).saturating_sub(source.as_ptr() as usize)
}

fn tag_attrs_offset(source: &str, attrs: &str) -> usize {
    if attrs.is_empty() {
        source.len()
    } else {
        slice_offset(source, attrs)
    }
}

fn split_tag_name_attrs(source: &str) -> (&str, &str) {
    let mut parts = source.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or_default();
    let attrs = parts.next().unwrap_or_default().trim();
    (name, attrs)
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

fn normalize_tag_alias<'a>(name: &'a str, attrs: &'a str) -> (&'a str, &'a str) {
    match name {
        "page" => ("p", attrs),
        "wait" => ("l", attrs),
        "nl" => ("r", attrs),
        _ => (name, attrs),
    }
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
    let Expr::Call { callee, args } = expr else {
        return None;
    };
    if !matches!(callee.as_ref(), Expr::Path(path) if path == "ruby") {
        return None;
    }
    let [
        CallArg::Positional(Expr::Literal(Literal::String(base))),
        CallArg::Positional(Expr::Literal(Literal::String(ruby))),
    ] = args.as_slice()
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
    use super::{find_dialogue_tag_boundary, parse_dialogue_text};
    use crate::ast::{common::TextRange, dialogue::DialogueToken};

    #[test]
    fn parses_dialogue_tag_arguments_with_absolute_ranges() {
        let source = "A[decorate .warning amplitude=4px mood=\"very urgent\"]text[/decorate]";
        let parsed = parse_dialogue_text(source);
        assert_eq!(parsed.diagnostics(), &[]);

        let tag = parsed
            .tokens()
            .iter()
            .find_map(|token| match token {
                DialogueToken::Tag(tag) if tag.name() == "decorate" => Some(tag),
                _ => None,
            })
            .expect("decorate tag");
        assert_eq!(
            &source[tag.range().as_range()],
            "[decorate .warning amplitude=4px mood=\"very urgent\"]"
        );
        assert_eq!(
            &source[tag.attrs_range().as_range()],
            ".warning amplitude=4px mood=\"very urgent\""
        );
        assert_eq!(tag.arguments().len(), 3);
        assert_eq!(tag.arguments()[0].name(), None);
        assert_eq!(tag.arguments()[0].value().value(), ".warning");
        assert_eq!(tag.arguments()[1].name(), Some("amplitude"));
        assert_eq!(tag.arguments()[1].value().value(), "4px");
        assert_eq!(tag.arguments()[2].name(), Some("mood"));
        assert_eq!(tag.arguments()[2].value().source(), "\"very urgent\"");
        assert_eq!(tag.arguments()[2].value().value(), "very urgent");
        let value_range = tag.arguments()[2].value().range();
        assert_eq!(&source[value_range.as_range()], "\"very urgent\"");

        let end = parsed
            .tokens()
            .iter()
            .find_map(|token| match token {
                DialogueToken::EndTag(end) => Some(end),
                _ => None,
            })
            .expect("decorate end tag");
        assert_eq!(end.name(), "decorate");
        assert!(!end.is_synthetic());
        assert_eq!(&source[end.range().as_range()], "[/decorate]");
    }

    #[test]
    fn named_argument_value_range_starts_after_the_assignment() {
        let source = "[decorate .warning accent=accent]text[/decorate]";
        let parsed = parse_dialogue_text(source);

        assert_eq!(parsed.diagnostics(), &[]);
        let Some(DialogueToken::Tag(tag)) = parsed.tokens().first() else {
            panic!("decorate tag");
        };
        let argument = &tag.arguments()[1];
        let name_range = argument.name_range().expect("authored name range");
        let value_range = argument.value().range();
        assert_eq!(&source[name_range.as_range()], "accent");
        assert_eq!(&source[value_range.as_range()], "accent");
        assert!(name_range.end() < value_range.start());
    }

    #[test]
    fn reports_unterminated_dialogue_tag_quotes_and_recovers_the_tag() {
        let source = "[decorate .warning mood=\"very urgent]text";
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
            Some(DialogueToken::Tag(tag)) if tag.name() == "decorate"
        ));
    }

    #[test]
    fn quoted_closing_brackets_do_not_end_dialogue_tags() {
        let source = "[decorate .warning note=\"contains ] safely\"]text[/decorate]";
        let boundary = find_dialogue_tag_boundary(source, 0).expect("tag boundary");
        assert_eq!(&source[boundary.close()..boundary.end()], "]");
        assert_eq!(
            &source[..boundary.end()],
            "[decorate .warning note=\"contains ] safely\"]"
        );
        assert_eq!(boundary.unterminated_quote_start(), None);

        let parsed = parse_dialogue_text(source);

        assert_eq!(parsed.diagnostics(), &[]);
        let Some(DialogueToken::Tag(tag)) = parsed.tokens().first() else {
            panic!("decorate tag");
        };
        assert_eq!(&source[tag.name_range().as_range()], "decorate");
        assert_eq!(tag.arguments()[1].value().value(), "contains ] safely");
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
        assert_eq!(tag.arguments()[0].value().source(), "\"a]b:c\"");
        assert_eq!(tag.arguments()[0].value().value(), "a]b:c");
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
        let source = "[decorate .warning note=\"safe ] here\" mood=\"unterminated]text";
        let parsed = parse_dialogue_text(source);

        assert_eq!(parsed.diagnostics().len(), 1);
        let diagnostic = &parsed.diagnostics()[0];
        assert_eq!(
            &source[diagnostic.range().start()..diagnostic.range().end()],
            "\"unterminated]"
        );
        let Some(DialogueToken::Tag(tag)) = parsed.tokens().first() else {
            panic!("recovered decorate tag");
        };
        assert_eq!(tag.arguments()[1].value().value(), "safe ] here");
        assert_eq!(tag.arguments()[2].value().value(), "\"unterminated");
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
        assert_eq!(tag.arguments()[0].name(), Some("value"));
        assert_eq!(tag.arguments()[0].name_range(), None);
        let value = tag.arguments()[0].value();
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
