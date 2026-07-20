use super::{DialogueTextDiagnostic, DialogueTextDiagnosticCode, parse_dialogue_expr_lossy};
use crate::ast::{
    common::TextRange,
    dialogue::{
        DialogueCallSurface, DialogueEndTag, DialogueExprSurface, DialogueTag, DialogueTagArg,
        DialogueTagArgSyntaxIssue, DialogueTagArgValue, DialogueTagArgValueSurface,
        DialogueTagPayload, DialogueTagRanges, DialogueToken, LineMark, QuoteStyle,
    },
};

/// Maximum number of argument entries retained for one `RichText` tag.
pub const MAX_RICH_TEXT_TAG_ARGUMENTS: usize = 32;
/// Maximum UTF-8 byte length of one `RichText` argument key.
pub const MAX_RICH_TEXT_TAG_KEY_BYTES: usize = 64;
/// Maximum encoded and decoded UTF-8 byte length of one `RichText` value.
pub const MAX_RICH_TEXT_TAG_VALUE_BYTES: usize = 4_096;
/// Maximum UTF-8 byte length of one `RichText` tag body.
pub const MAX_RICH_TEXT_TAG_BODY_BYTES: usize = 16_384;
/// Maximum number of `RichText` tag nodes retained in one dialogue content.
pub const MAX_RICH_TEXT_CONTENT_TAGS: usize = 4_096;
/// Maximum total `RichText` argument entries retained in one dialogue content.
pub const MAX_RICH_TEXT_CONTENT_ARGUMENTS: usize = 32_768;

/// Quote-aware closing boundary of one authored dialogue tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialogueTagBoundary {
    close: usize,
    unterminated_quote_start: Option<usize>,
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

pub(super) struct ParsedTag {
    pub(super) token: DialogueToken,
    pub(super) consumed_to: usize,
    pub(super) diagnostics: Vec<DialogueTextDiagnostic>,
}

struct OpenTagContext {
    inside_start: usize,
    consumed_to: usize,
    range: TextRange,
    diagnostics: Vec<DialogueTextDiagnostic>,
    unterminated_quote: Option<TextRange>,
    content_arguments_remaining: usize,
}

struct OpenTagHead<'a> {
    name: &'a str,
    source_name: &'a str,
    name_range: TextRange,
    authored_attrs: &'a str,
    stored_attrs: String,
    inferred: bool,
}

pub(super) fn parse_tag(
    source: &str,
    open: usize,
    content_arguments_remaining: usize,
) -> Option<ParsedTag> {
    let boundary = find_dialogue_tag_boundary(source, open)?;
    let close = boundary.close();
    let unterminated_quote = boundary.unterminated_quote_start();
    let inside_source = &source[open + '['.len_utf8()..close];
    let inside = trim_rich_text_whitespace(inside_source);
    let inside_leading = inside_source.len() - trim_rich_text_whitespace_start(inside_source).len();
    let inside_start = open + '['.len_utf8() + inside_leading;
    let consumed_to = close + ']'.len_utf8();
    let range = TextRange::new(open, consumed_to);
    if inside_source.len() > MAX_RICH_TEXT_TAG_BODY_BYTES {
        let limit_start = open
            + '['.len_utf8()
            + utf8_boundary_at_or_before(inside_source, MAX_RICH_TEXT_TAG_BODY_BYTES);
        return Some(ParsedTag {
            token: DialogueToken::Text(source.get(open..consumed_to)?.to_owned()),
            consumed_to,
            diagnostics: vec![DialogueTextDiagnostic::with_code(
                DialogueTextDiagnosticCode::RichTextTagBodyTooLong,
                TextRange::new(limit_start, close),
                format!("dialogue RichText tag body exceeds {MAX_RICH_TEXT_TAG_BODY_BYTES} bytes"),
                "shorten the tag body",
            )],
        });
    }
    let unterminated_quote_range =
        unterminated_quote.map(|quote_start| TextRange::new(quote_start, consumed_to));
    let diagnostics = unterminated_quote_range
        .map(|quote_range| {
            vec![DialogueTextDiagnostic::with_code(
                DialogueTextDiagnosticCode::RichTextAttributeUnterminatedQuote,
                quote_range,
                "unterminated quote in dialogue tag arguments",
                "close the quoted tag argument before `]`",
            )]
        })
        .unwrap_or_default();
    if let Some(name) = inside.strip_prefix('/') {
        let name = trim_rich_text_whitespace(name);
        return Some(ParsedTag {
            token: if name.is_empty() {
                DialogueToken::InferredEndTag
            } else {
                DialogueToken::EndTag(DialogueEndTag::new(name.to_owned(), range))
            },
            consumed_to,
            diagnostics,
        });
    }

    (!inside.is_empty()).then_some(())?;
    Some(parse_open_tag(
        inside,
        OpenTagContext {
            inside_start,
            consumed_to,
            range,
            diagnostics,
            unterminated_quote: unterminated_quote_range,
            content_arguments_remaining,
        },
    ))
}

fn parse_open_tag(inside: &str, context: OpenTagContext) -> ParsedTag {
    if inside.starts_with('.') {
        let (selector, attrs) = split_tag_name_attrs(inside);
        let name_range =
            TextRange::new(context.inside_start, context.inside_start + selector.len());
        return parsed_dialogue_tag(
            OpenTagHead {
                name: selector,
                source_name: selector,
                name_range,
                authored_attrs: attrs,
                stored_attrs: attrs.to_owned(),
                inferred: true,
            },
            inside,
            context,
        );
    }
    if let Some(attrs) = inside.strip_prefix('!') {
        let attrs = trim_rich_text_whitespace(attrs);
        let name_range = TextRange::new(context.inside_start, context.inside_start + 1);
        return parsed_dialogue_tag(
            OpenTagHead {
                name: "call",
                source_name: "!",
                name_range,
                authored_attrs: attrs,
                stored_attrs: attrs.to_owned(),
                inferred: false,
            },
            inside,
            context,
        );
    }
    let (source_name, attrs) = split_tag_name_attrs(inside);
    let name_range = TextRange::new(
        context.inside_start,
        context.inside_start + source_name.len(),
    );
    if source_name == "mark" && !attrs.is_empty() {
        return ParsedTag {
            token: DialogueToken::Mark(LineMark::new(attrs.to_owned())),
            consumed_to: context.consumed_to,
            diagnostics: context.diagnostics,
        };
    }
    if source_name == "w" && !attrs.is_empty() && !attrs.contains('=') {
        return parsed_dialogue_tag(
            OpenTagHead {
                name: source_name,
                source_name,
                name_range,
                authored_attrs: attrs,
                stored_attrs: format!("time={attrs}"),
                inferred: false,
            },
            inside,
            context,
        );
    }
    let (name, attrs) = normalize_tag_alias(source_name, attrs);
    parsed_dialogue_tag(
        OpenTagHead {
            name,
            source_name,
            name_range,
            authored_attrs: attrs,
            stored_attrs: attrs.to_owned(),
            inferred: false,
        },
        inside,
        context,
    )
}

fn parsed_dialogue_tag(
    head: OpenTagHead<'_>,
    inside: &str,
    mut context: OpenTagContext,
) -> ParsedTag {
    let attrs_start = context.inside_start + tag_attrs_offset(inside, head.authored_attrs);
    let attrs_range = TextRange::new(attrs_start, attrs_start + head.authored_attrs.len());
    let mut parsed_arguments = if matches!(head.name, "fx" | "call" | "if") {
        ParsedTagArguments::default()
    } else {
        parse_tag_arguments(
            head.authored_attrs,
            attrs_start,
            context.content_arguments_remaining,
        )
    };
    if context.unterminated_quote.is_some() {
        parsed_arguments.diagnostics.retain(|diagnostic| {
            diagnostic.code() != DialogueTextDiagnosticCode::RichTextAttributeUnterminatedQuote
        });
    }
    let payload = match (head.name, head.authored_attrs.is_empty()) {
        (_, true) => DialogueTagPayload::None,
        ("fx", false) => DialogueTagPayload::FxCall(DialogueCallSurface::new(
            parse_dialogue_expr_lossy(head.authored_attrs),
            head.authored_attrs.to_owned(),
            attrs_range,
        )),
        ("call", false) => DialogueTagPayload::DialogueCall(DialogueCallSurface::new(
            parse_dialogue_expr_lossy(head.authored_attrs),
            head.authored_attrs.to_owned(),
            attrs_range,
        )),
        ("if", false) => DialogueTagPayload::Condition(DialogueExprSurface::new(
            parse_dialogue_expr_lossy(head.authored_attrs),
            head.authored_attrs.to_owned(),
            attrs_range,
        )),
        _ => DialogueTagPayload::Arguments,
    };
    let tag = DialogueTag::new(
        head.name.to_owned(),
        head.source_name.to_owned(),
        head.stored_attrs,
        payload,
        parsed_arguments.entries,
        DialogueTagRanges::new(head.name_range, context.range, attrs_range),
    );
    context.diagnostics.extend(parsed_arguments.diagnostics);
    ParsedTag {
        token: if head.inferred {
            DialogueToken::InferredTag(tag)
        } else {
            DialogueToken::Tag(tag)
        },
        consumed_to: context.consumed_to,
        diagnostics: context.diagnostics,
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
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if ch == active {
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

#[derive(Default)]
pub(super) struct ParsedTagArguments {
    pub(super) entries: Vec<DialogueTagArg>,
    pub(super) diagnostics: Vec<DialogueTextDiagnostic>,
}

struct ArgumentBoundary {
    end: usize,
    unterminated_quote_start: Option<usize>,
}

pub(super) fn parse_tag_arguments(
    source: &str,
    base: usize,
    content_arguments_remaining: usize,
) -> ParsedTagArguments {
    let mut parsed = ParsedTagArguments::default();
    let mut cursor = 0;
    while cursor < source.len() {
        cursor += source[cursor..]
            .chars()
            .take_while(|ch| is_rich_text_whitespace(*ch))
            .map(char::len_utf8)
            .sum::<usize>();
        if cursor >= source.len() {
            break;
        }
        if parsed.entries.len() >= MAX_RICH_TEXT_TAG_ARGUMENTS {
            parsed.diagnostics.push(DialogueTextDiagnostic::with_code(
                DialogueTextDiagnosticCode::RichTextAttributeTooMany,
                TextRange::new(base + cursor, base + source.len()),
                format!(
                    "dialogue RichText tag has more than {MAX_RICH_TEXT_TAG_ARGUMENTS} arguments"
                ),
                "remove excess arguments",
            ));
            break;
        }
        if parsed.entries.len() >= content_arguments_remaining {
            parsed.diagnostics.push(DialogueTextDiagnostic::with_code(
                DialogueTextDiagnosticCode::RichTextContentArgumentLimit,
                TextRange::new(base + cursor, base + source.len()),
                format!(
                    "dialogue content has more than {MAX_RICH_TEXT_CONTENT_ARGUMENTS} RichText arguments"
                ),
                "split the dialogue content or remove excess arguments",
            ));
            break;
        }
        let start = cursor;
        let boundary = find_argument_boundary(source, start);
        cursor = boundary.end;
        let argument_source = &source[start..cursor];
        if argument_source.is_empty() {
            continue;
        }
        let range = TextRange::new(base + start, base + cursor);
        let argument = if let Some(quote_start) = boundary.unterminated_quote_start {
            let issue = DialogueTagArgSyntaxIssue::UnterminatedQuote {
                range: TextRange::new(base + quote_start, base + cursor),
            };
            parsed.diagnostics.push(argument_issue_diagnostic(&issue));
            DialogueTagArg::Invalid {
                source: argument_source.to_owned(),
                range,
                issue,
            }
        } else {
            parse_tag_argument(
                argument_source,
                base + start,
                range,
                &mut parsed.diagnostics,
            )
        };
        parsed.entries.push(argument);
    }
    parsed
}

fn find_argument_boundary(source: &str, start: usize) -> ArgumentBoundary {
    let mut quote = None;
    let mut quote_start = None;
    let mut escaped = false;
    let mut end = source.len();
    for (relative, ch) in source[start..].char_indices() {
        let index = start + relative;
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if ch == active {
                quote = None;
                quote_start = None;
            }
            continue;
        }
        if matches!(ch, '"' | '\'') {
            quote = Some(ch);
            quote_start = Some(index);
        } else if is_rich_text_whitespace(ch) {
            end = index;
            break;
        }
    }
    ArgumentBoundary {
        end,
        unterminated_quote_start: quote_start,
    }
}

fn parse_tag_argument(
    source: &str,
    start: usize,
    range: TextRange,
    diagnostics: &mut Vec<DialogueTextDiagnostic>,
) -> DialogueTagArg {
    let Some(equal) = unquoted_assignment(source) else {
        return match tag_arg_value(source, start) {
            Ok(value) => DialogueTagArg::Positional {
                value: DialogueTagArgValueSurface::Present(value),
                range,
            },
            Err(issue) => {
                diagnostics.push(argument_issue_diagnostic(&issue));
                DialogueTagArg::Invalid {
                    source: source.to_owned(),
                    range,
                    issue,
                }
            }
        };
    };

    let key = &source[..equal];
    let key_range = TextRange::new(start, start + key.len());
    let equals_range = TextRange::new(start + equal, start + equal + '='.len_utf8());
    if key.is_empty() {
        let issue = DialogueTagArgSyntaxIssue::EmptyKey { range: key_range };
        diagnostics.push(argument_issue_diagnostic(&issue));
        return DialogueTagArg::Invalid {
            source: source.to_owned(),
            range,
            issue,
        };
    }
    if key.len() > MAX_RICH_TEXT_TAG_KEY_BYTES {
        let limit = utf8_boundary_at_or_before(key, MAX_RICH_TEXT_TAG_KEY_BYTES);
        let issue = DialogueTagArgSyntaxIssue::KeyTooLong {
            range: TextRange::new(start + limit, start + key.len()),
        };
        diagnostics.push(argument_issue_diagnostic(&issue));
        return DialogueTagArg::Invalid {
            source: source.to_owned(),
            range,
            issue,
        };
    }
    if !valid_rich_text_key(key) {
        let issue = DialogueTagArgSyntaxIssue::InvalidKey { range: key_range };
        diagnostics.push(argument_issue_diagnostic(&issue));
        return DialogueTagArg::Invalid {
            source: source.to_owned(),
            range,
            issue,
        };
    }

    let value_start = equal + '='.len_utf8();
    let authored_value = &source[value_start..];
    let value_absolute_start = start + value_start;
    let value = if authored_value.is_empty() {
        let missing_range = TextRange::new(value_absolute_start, value_absolute_start);
        diagnostics.push(DialogueTextDiagnostic::with_code(
            DialogueTextDiagnosticCode::RichTextAttributeMissingValue,
            missing_range,
            format!("dialogue RichText attribute `{key}` is missing a value"),
            "insert an authored value after `=`",
        ));
        DialogueTagArgValueSurface::Missing {
            range: missing_range,
        }
    } else {
        match tag_arg_value(authored_value, value_absolute_start) {
            Ok(value) => DialogueTagArgValueSurface::Present(value),
            Err(issue) => {
                diagnostics.push(argument_issue_diagnostic(&issue));
                return DialogueTagArg::Invalid {
                    source: source.to_owned(),
                    range,
                    issue,
                };
            }
        }
    };
    DialogueTagArg::Named {
        name: key.to_owned(),
        name_range: key_range,
        equals_range,
        value,
        range,
    }
}

fn unquoted_assignment(source: &str) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    source.char_indices().find_map(|(index, ch)| {
        if escaped {
            escaped = false;
            return None;
        }
        if ch == '\\' {
            escaped = true;
            return None;
        }
        if let Some(active) = quote {
            if ch == active {
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

pub(super) fn tag_arg_value(
    source: &str,
    start: usize,
) -> Result<DialogueTagArgValue, DialogueTagArgSyntaxIssue> {
    let token_range = TextRange::new(start, start + source.len());
    if source.len() > MAX_RICH_TEXT_TAG_VALUE_BYTES {
        let limit = utf8_boundary_at_or_before(source, MAX_RICH_TEXT_TAG_VALUE_BYTES);
        return Err(DialogueTagArgSyntaxIssue::ValueTooLong {
            range: TextRange::new(start + limit, start + source.len()),
        });
    }
    let (quote, content, content_start, opening_quote_range, closing_quote_range) =
        quoted_value_parts(source, start);
    let value = decode_tag_arg_value(content, content_start)?;
    if value.len() > MAX_RICH_TEXT_TAG_VALUE_BYTES {
        return Err(DialogueTagArgSyntaxIssue::ValueTooLong {
            range: TextRange::new(content_start, content_start + content.len()),
        });
    }
    Ok(DialogueTagArgValue::new(
        source.to_owned(),
        value,
        token_range,
        TextRange::new(content_start, content_start + content.len()),
        quote,
        opening_quote_range,
        closing_quote_range,
    ))
}

fn quoted_value_parts(
    source: &str,
    start: usize,
) -> (
    QuoteStyle,
    &str,
    usize,
    Option<TextRange>,
    Option<TextRange>,
) {
    let quoted = source
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(|value| (QuoteStyle::Double, value))
        .or_else(|| {
            source
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
                .map(|value| (QuoteStyle::Single, value))
        });
    let Some((quote, content)) = quoted else {
        return (QuoteStyle::Unquoted, source, start, None, None);
    };
    let content_start = start + 1;
    (
        quote,
        content,
        content_start,
        Some(TextRange::new(start, start + 1)),
        Some(TextRange::new(
            start + source.len() - 1,
            start + source.len(),
        )),
    )
}

fn decode_tag_arg_value(
    source: &str,
    absolute_start: usize,
) -> Result<String, DialogueTagArgSyntaxIssue> {
    let mut decoded = String::with_capacity(source.len());
    let mut chars = source.char_indices();
    while let Some((offset, ch)) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }
        let Some((next_offset, escaped)) = chars.next() else {
            return Err(DialogueTagArgSyntaxIssue::InvalidEscape {
                range: TextRange::new(absolute_start + offset, absolute_start + offset + 1),
            });
        };
        let decoded_escape = match escaped {
            '\\' => '\\',
            '"' => '"',
            '\'' => '\'',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            ' ' => ' ',
            '=' => '=',
            '[' => '[',
            ']' => ']',
            _ => {
                return Err(DialogueTagArgSyntaxIssue::InvalidEscape {
                    range: TextRange::new(
                        absolute_start + offset,
                        absolute_start + next_offset + escaped.len_utf8(),
                    ),
                });
            }
        };
        decoded.push(decoded_escape);
    }
    Ok(decoded)
}

fn argument_issue_diagnostic(issue: &DialogueTagArgSyntaxIssue) -> DialogueTextDiagnostic {
    match issue {
        DialogueTagArgSyntaxIssue::EmptyKey { range } => DialogueTextDiagnostic::with_code(
            DialogueTextDiagnosticCode::RichTextAttributeEmptyKey,
            *range,
            "dialogue RichText attribute key is empty",
            "insert a key matching `[a-z][a-z0-9_]*` before `=`",
        ),
        DialogueTagArgSyntaxIssue::InvalidKey { range } => DialogueTextDiagnostic::with_code(
            DialogueTextDiagnosticCode::RichTextAttributeInvalidKey,
            *range,
            "dialogue RichText attribute key is not canonical",
            "use an ASCII lowercase key matching `[a-z][a-z0-9_]*`",
        ),
        DialogueTagArgSyntaxIssue::InvalidEscape { range } => DialogueTextDiagnostic::with_code(
            DialogueTextDiagnosticCode::RichTextAttributeInvalidEscape,
            *range,
            "dialogue RichText attribute contains an unsupported escape",
            "use one of `\\\\`, `\\\"`, `\\'`, `\\n`, `\\r`, `\\t`, `\\ `, `\\=`, `\\[`, or `\\]`",
        ),
        DialogueTagArgSyntaxIssue::UnterminatedQuote { range } => {
            DialogueTextDiagnostic::with_code(
                DialogueTextDiagnosticCode::RichTextAttributeUnterminatedQuote,
                *range,
                "unterminated quote in dialogue tag arguments",
                "close the quoted tag argument before `]`",
            )
        }
        DialogueTagArgSyntaxIssue::KeyTooLong { range } => DialogueTextDiagnostic::with_code(
            DialogueTextDiagnosticCode::RichTextAttributeKeyTooLong,
            *range,
            format!("dialogue RichText attribute key exceeds {MAX_RICH_TEXT_TAG_KEY_BYTES} bytes"),
            "shorten the attribute key",
        ),
        DialogueTagArgSyntaxIssue::ValueTooLong { range } => DialogueTextDiagnostic::with_code(
            DialogueTextDiagnosticCode::RichTextAttributeValueTooLong,
            *range,
            format!(
                "dialogue RichText attribute value exceeds {MAX_RICH_TEXT_TAG_VALUE_BYTES} bytes"
            ),
            "shorten the attribute value",
        ),
    }
}

fn is_rich_text_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\u{0009}'..='\u{000D}'
            | '\u{0020}'
            | '\u{0085}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
    )
}

pub(super) fn trim_rich_text_whitespace(source: &str) -> &str {
    trim_rich_text_whitespace_start(source).trim_end_matches(is_rich_text_whitespace)
}

fn trim_rich_text_whitespace_start(source: &str) -> &str {
    source.trim_start_matches(is_rich_text_whitespace)
}

fn valid_rich_text_key(key: &str) -> bool {
    let mut bytes = key.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn utf8_boundary_at_or_before(source: &str, limit: usize) -> usize {
    let mut boundary = limit.min(source.len());
    while boundary > 0 && !source.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

fn tag_attrs_offset(source: &str, attrs: &str) -> usize {
    if attrs.is_empty() {
        source.len()
    } else {
        (attrs.as_ptr() as usize).saturating_sub(source.as_ptr() as usize)
    }
}

pub(super) fn split_tag_name_attrs(source: &str) -> (&str, &str) {
    let split = source
        .char_indices()
        .find_map(|(index, ch)| is_rich_text_whitespace(ch).then_some(index));
    split.map_or((source, ""), |index| {
        (
            &source[..index],
            trim_rich_text_whitespace(&source[index..]),
        )
    })
}

fn normalize_tag_alias<'a>(name: &'a str, attrs: &'a str) -> (&'a str, &'a str) {
    match name {
        "page" => ("p", attrs),
        "wait" => ("l", attrs),
        "nl" => ("r", attrs),
        _ => (name, attrs),
    }
}
