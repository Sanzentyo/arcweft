use crate::ast::{
    common::TextRange,
    dialogue::{DialogueTag, DialogueToken, LineMark},
};
use crate::expr::{Expr, Literal, parse_expr};

/// Parsed dialogue-text tokens plus recoverable text-mode diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueTextParse {
    tokens: Vec<DialogueToken>,
    diagnostics: Vec<DialogueTextDiagnostic>,
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
                    tokens.push(parse_dialogue_expr_token(&expr));
                    skip_to(&mut chars, consumed_to);
                } else {
                    text.push_str("#[");
                }
            }
            '$' if chars.peek().is_some_and(|(_, next)| *next == '(') => {
                let _ = chars.next();
                if let Some((expr, consumed_to)) = take_balanced_paren(source, index + 2) {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(DialogueToken::Expr(parse_dialogue_expr_lossy(&expr)));
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
                if let Some((tag, consumed_to)) = parse_tag(source, index + 1) {
                    flush_text(&mut text, &mut tokens);
                    tokens.push(tag);
                    skip_to(&mut chars, consumed_to);
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
}

impl DialogueTextDiagnostic {
    fn new(range: TextRange, message: impl Into<String>, recovery: impl Into<String>) -> Self {
        Self {
            range,
            message: message.into(),
            recovery: recovery.into(),
        }
    }

    /// Byte range relative to the dialogue text source.
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
    let after_open = source
        .get(start..)?
        .find(']')
        .map(|close| start + close + 1)?;
    let inside = source.get(start + 1..after_open - 1)?.trim();
    let (tag_name, attrs) = split_tag_name_attrs(inside);
    if !matches!(tag_name, "ruby" | "rb") {
        return None;
    }
    let ruby = parse_ruby_attr(attrs)?;
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

fn parse_ruby_attr(attrs: &str) -> Option<String> {
    let value = attrs.trim().strip_prefix("rt")?.trim_start();
    let value = value.strip_prefix('=')?.trim_start();
    if let Some(quoted) = value.strip_prefix('"') {
        let end = quoted.find('"')?;
        return Some(quoted[..end].to_owned());
    }
    let end = value.find(char::is_whitespace).unwrap_or(value.len());
    (end > 0).then(|| value[..end].to_owned())
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
    let body = tail.strip_prefix("[raw:")?;
    let close_relative = body.rfind(']')?;
    let raw = body[..close_relative].trim_start().to_owned();
    Some((raw, start + "[raw:".len() + close_relative + ']'.len_utf8()))
}

fn parse_inline_style_span(source: &str, start: usize) -> Option<(Vec<DialogueToken>, usize)> {
    let close_relative = source.get(start + 1..)?.find(']')?;
    let consumed_to = start + 1 + close_relative + ']'.len_utf8();
    let inside = source.get(start + 1..start + 1 + close_relative)?.trim();
    let (tag_source, body) = split_once_top_level(inside, ':')?;
    if body.is_empty() {
        return None;
    }
    let (name, attrs) = parse_inline_style_head(tag_source.trim())?;
    Some((
        vec![
            DialogueToken::Tag(DialogueTag::new(name.clone(), attrs)),
            DialogueToken::Text(body.to_owned()),
            DialogueToken::EndTag(name),
        ],
        consumed_to,
    ))
}

fn parse_inline_style_head(source: &str) -> Option<(String, String)> {
    if matches!(source, "em" | "strong") {
        return Some((source.to_owned(), String::new()));
    }
    let value = source.strip_prefix("color ")?.trim();
    (!value.is_empty()).then(|| ("color".to_owned(), format!("value=\"{value}\"")))
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

fn parse_tag(source: &str, start: usize) -> Option<(DialogueToken, usize)> {
    let close_relative = source.get(start..)?.find(']')?;
    let inside = &source[start..start + close_relative];
    let consumed_to = start + close_relative + 1;
    if let Some(name) = inside.strip_prefix('/') {
        return Some((DialogueToken::EndTag(name.trim().to_owned()), consumed_to));
    }

    let trimmed = inside.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(name) = trimmed.strip_prefix('.') {
        let name = format!(".{name}");
        return Some((DialogueToken::Mark(LineMark::new(name)), consumed_to));
    }
    if let Some(attrs) = trimmed.strip_prefix('!') {
        return Some((
            DialogueToken::Tag(DialogueTag::new("call".to_owned(), attrs.trim().to_owned())),
            consumed_to,
        ));
    }
    let (name, attrs) = split_tag_name_attrs(trimmed);
    if name == "mark" && !attrs.is_empty() {
        return Some((
            DialogueToken::Mark(LineMark::new(attrs.to_owned())),
            consumed_to,
        ));
    }
    if name == "w" && !attrs.is_empty() && !attrs.contains('=') {
        return Some((
            DialogueToken::Tag(DialogueTag::new(name.to_owned(), format!("time={attrs}"))),
            consumed_to,
        ));
    }
    let (name, attrs) = normalize_tag_alias(name, attrs);
    Some((
        DialogueToken::Tag(DialogueTag::new(name.to_owned(), attrs.to_owned())),
        consumed_to,
    ))
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
    for (index, ch) in source.char_indices() {
        match ch {
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

fn parse_dialogue_expr_token(source: &str) -> DialogueToken {
    let expr = parse_dialogue_expr_lossy(source);
    function_ruby_token(&expr).unwrap_or(DialogueToken::Expr(expr))
}

fn function_ruby_token(expr: &Expr) -> Option<DialogueToken> {
    let Expr::Call { callee, args } = expr else {
        return None;
    };
    if !matches!(callee.as_ref(), Expr::Path(path) if path == "ruby") {
        return None;
    }
    let [
        Expr::Literal(Literal::String(base)),
        Expr::Literal(Literal::String(ruby)),
    ] = args.as_slice()
    else {
        return None;
    };
    Some(DialogueToken::Ruby {
        base: base.to_owned(),
        ruby: ruby.to_owned(),
    })
}
