//! Typed lexical surfaces consumed by the attached dialogue-content grammar.
//!
//! This scanner runs inside the document parser over its already accepted
//! source interval.  It retains semantic text plus exact authored ranges and
//! never constructs the legacy dialogue AST or reparses an expression.

use crate::ast::common::TextRange;

use super::rich_text_tag::{
    MAX_RICH_TEXT_CONTENT_ARGUMENTS, ScannedTagArgValue, ScannedTagArgument,
    find_dialogue_tag_boundary_before, scan_tag_arg_value, scan_tag_arguments,
    trim_rich_text_whitespace,
};

/// One grammar-recognized non-plain-text dialogue surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScannedDialogueSurface {
    range: TextRange,
    kind: ScannedDialogueSurfaceKind,
    rich_text_tags: usize,
    rich_text_arguments: usize,
}

impl ScannedDialogueSurface {
    pub(crate) const fn range(&self) -> TextRange {
        self.range
    }

    pub(crate) const fn end(&self) -> usize {
        self.range.end()
    }

    pub(crate) const fn kind(&self) -> &ScannedDialogueSurfaceKind {
        &self.kind
    }

    pub(crate) const fn rich_text_tags(&self) -> usize {
        self.rich_text_tags
    }

    pub(crate) const fn rich_text_arguments(&self) -> usize {
        self.rich_text_arguments
    }
}

/// Exact semantic family and authored parts of one recognized surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ScannedDialogueSurfaceKind {
    Escape {
        marker: TextRange,
        escaped: TextRange,
        value: char,
    },
    Ruby(ScannedDialogueRuby),
    Interpolation {
        form: ScannedInterpolationForm,
        open: TextRange,
        payload: TextRange,
        close: TextRange,
    },
    Raw {
        form: ScannedRawForm,
        open: TextRange,
        body: ScannedDialogueText,
        close: TextRange,
    },
    InlineStyle(ScannedInlineStyle),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScannedDialogueRuby {
    form: ScannedRubyForm,
    base: ScannedDialogueText,
    ruby: ScannedDialogueText,
}

impl ScannedDialogueRuby {
    pub(crate) const fn base(&self) -> &ScannedDialogueText {
        &self.base
    }

    pub(crate) const fn ruby(&self) -> &ScannedDialogueText {
        &self.ruby
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ScannedRubyForm {
    Natural,
    AsciiExplicit,
    AsciiCompact,
    Bracket,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ScannedInterpolationForm {
    HashBracket,
    DollarParen,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ScannedRawForm {
    Paired,
    Inline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScannedDialogueText {
    decoded: Box<str>,
    range: TextRange,
}

impl ScannedDialogueText {
    fn from_source(source: &str, range: TextRange) -> Option<Self> {
        Some(Self {
            decoded: source.get(range.as_range())?.into(),
            range,
        })
    }

    fn decoded(value: impl Into<Box<str>>, range: TextRange) -> Self {
        Self {
            decoded: value.into(),
            range,
        }
    }

    pub(crate) fn value(&self) -> &str {
        &self.decoded
    }

    pub(crate) const fn range(&self) -> TextRange {
        self.range
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScannedInlineStyle {
    style: ScannedInlineStyleKind,
    name: TextRange,
    value: Option<ScannedTagArgValue>,
    body: ScannedDialogueText,
    separator: TextRange,
    inferred_end: usize,
}

impl ScannedInlineStyle {
    pub(crate) const fn style(&self) -> ScannedInlineStyleKind {
        self.style
    }

    pub(crate) const fn name(&self) -> TextRange {
        self.name
    }

    pub(crate) const fn value(&self) -> Option<&ScannedTagArgValue> {
        self.value.as_ref()
    }

    pub(crate) const fn body(&self) -> &ScannedDialogueText {
        &self.body
    }

    pub(crate) const fn separator(&self) -> TextRange {
        self.separator
    }

    pub(crate) const fn inferred_end(&self) -> usize {
        self.inferred_end
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ScannedInlineStyleKind {
    Emphasis,
    Strong,
    Color,
}

/// Scans one typed dialogue surface beginning at `start` without reading past
/// the accepted content boundary `end`.
pub(crate) fn scan_dialogue_surface(
    source: &str,
    start: usize,
    end: usize,
) -> Option<ScannedDialogueSurface> {
    let bounded = source.get(..end)?;
    let tail = bounded.get(start..)?;
    let first = tail.chars().next()?;

    match first {
        '\\' => scan_escape(tail, start),
        '|' => scan_ascii_explicit_ruby(bounded, start)
            .or_else(|| scan_ascii_compact_ruby(bounded, start)),
        '｜' => scan_natural_ruby(bounded, start),
        '#' if tail.starts_with("#[") => scan_interpolation(
            bounded,
            start,
            "#[",
            '[',
            ']',
            ScannedInterpolationForm::HashBracket,
        ),
        '$' if tail.starts_with("$(") => scan_interpolation(
            bounded,
            start,
            "$(",
            '(',
            ')',
            ScannedInterpolationForm::DollarParen,
        ),
        '[' => scan_bracket_ruby(bounded, start)
            .or_else(|| scan_paired_raw(bounded, start))
            .or_else(|| scan_inline_raw(bounded, start))
            .or_else(|| scan_inline_style(bounded, start)),
        _ => None,
    }
}

fn scanned(
    range: TextRange,
    kind: ScannedDialogueSurfaceKind,
    rich_text_tags: usize,
    rich_text_arguments: usize,
) -> ScannedDialogueSurface {
    ScannedDialogueSurface {
        range,
        kind,
        rich_text_tags,
        rich_text_arguments,
    }
}

fn scan_escape(tail: &str, start: usize) -> Option<ScannedDialogueSurface> {
    let value = tail['\\'.len_utf8()..].chars().next()?;
    let marker = TextRange::new(start, start + '\\'.len_utf8());
    let escaped = TextRange::new(marker.end(), marker.end() + value.len_utf8());
    Some(scanned(
        TextRange::new(start, escaped.end()),
        ScannedDialogueSurfaceKind::Escape {
            marker,
            escaped,
            value,
        },
        0,
        0,
    ))
}

fn scan_natural_ruby(source: &str, start: usize) -> Option<ScannedDialogueSurface> {
    let after_marker = start + '｜'.len_utf8();
    let tail = source.get(after_marker..)?;
    let open_relative = tail.find('《')?;
    let base_range = TextRange::new(after_marker, after_marker + open_relative);
    (base_range.start() < base_range.end()).then_some(())?;
    let ruby_start = base_range.end() + '《'.len_utf8();
    let ruby_tail = source.get(ruby_start..)?;
    let close_relative = ruby_tail.find('》')?;
    let ruby_range = TextRange::new(ruby_start, ruby_start + close_relative);
    (ruby_range.start() < ruby_range.end()).then_some(())?;
    let end = ruby_range.end() + '》'.len_utf8();
    Some(scanned(
        TextRange::new(start, end),
        ScannedDialogueSurfaceKind::Ruby(ScannedDialogueRuby {
            form: ScannedRubyForm::Natural,
            base: ScannedDialogueText::from_source(source, base_range)?,
            ruby: ScannedDialogueText::from_source(source, ruby_range)?,
        }),
        0,
        0,
    ))
}

fn scan_ascii_explicit_ruby(source: &str, start: usize) -> Option<ScannedDialogueSurface> {
    let base_start = start + "|[".len();
    source.get(start..)?.starts_with("|[").then_some(())?;
    let base_tail = source.get(base_start..)?;
    let base_end = base_start + base_tail.find(']')?;
    (base_start < base_end).then_some(())?;
    let ruby_start = base_end + "](".len();
    source.get(base_end..)?.starts_with("](").then_some(())?;
    let ruby_tail = source.get(ruby_start..)?;
    let ruby_end = ruby_start + ruby_tail.find(')')?;
    (ruby_start < ruby_end).then_some(())?;
    let end = ruby_end + ')'.len_utf8();
    Some(scanned(
        TextRange::new(start, end),
        ScannedDialogueSurfaceKind::Ruby(ScannedDialogueRuby {
            form: ScannedRubyForm::AsciiExplicit,
            base: ScannedDialogueText::from_source(source, TextRange::new(base_start, base_end))?,
            ruby: ScannedDialogueText::from_source(source, TextRange::new(ruby_start, ruby_end))?,
        }),
        0,
        0,
    ))
}

fn scan_ascii_compact_ruby(source: &str, start: usize) -> Option<ScannedDialogueSurface> {
    let base_start = start + '|'.len_utf8();
    let tail = source.get(base_start..)?;
    (!tail.starts_with('[')).then_some(())?;
    let open_relative = tail.find('{')?;
    let base_range = TextRange::new(base_start, base_start + open_relative);
    valid_compact_ruby_base(source.get(base_range.as_range())?).then_some(())?;
    let ruby_start = base_range.end() + '{'.len_utf8();
    let ruby_tail = source.get(ruby_start..)?;
    let ruby_end = ruby_start + ruby_tail.find('}')?;
    (ruby_start < ruby_end).then_some(())?;
    let end = ruby_end + '}'.len_utf8();
    Some(scanned(
        TextRange::new(start, end),
        ScannedDialogueSurfaceKind::Ruby(ScannedDialogueRuby {
            form: ScannedRubyForm::AsciiCompact,
            base: ScannedDialogueText::from_source(source, base_range)?,
            ruby: ScannedDialogueText::from_source(source, TextRange::new(ruby_start, ruby_end))?,
        }),
        0,
        0,
    ))
}

fn scan_bracket_ruby(source: &str, start: usize) -> Option<ScannedDialogueSurface> {
    let boundary = find_dialogue_tag_boundary_before(source, start, source.len())?;
    boundary
        .unterminated_quote_start()
        .is_none()
        .then_some(())?;
    let inside_source = source.get(start + '['.len_utf8()..boundary.close())?;
    let inside = trim_rich_text_whitespace(inside_source);
    let (tag_name, attrs) = split_tag_head(inside);
    matches!(tag_name, "ruby" | "rb").then_some(())?;
    let attrs_start = slice_offset(source, attrs)?;
    let arguments = scan_tag_arguments(attrs, attrs_start, MAX_RICH_TEXT_CONTENT_ARGUMENTS);
    arguments.diagnostics().is_empty().then_some(())?;
    let ruby = arguments.entries().iter().find_map(|argument| {
        let ScannedTagArgument::Named {
            name_range, value, ..
        } = argument
        else {
            return None;
        };
        (source.get(name_range.as_range())? == "rt").then_some(value)
    })?;
    let base_start = boundary.end();
    let close_spelling = format!("[/{tag_name}]");
    let close_relative = source.get(base_start..)?.find(&close_spelling)?;
    let close_start = base_start + close_relative;
    let untrimmed_base = source.get(base_start..close_start)?;
    let base = trim_rich_text_whitespace(untrimmed_base);
    (!base.is_empty()).then_some(())?;
    let base_offset = slice_offset(source, base)?;
    let close_end = close_start + close_spelling.len();
    Some(scanned(
        TextRange::new(start, close_end),
        ScannedDialogueSurfaceKind::Ruby(ScannedDialogueRuby {
            form: ScannedRubyForm::Bracket,
            base: ScannedDialogueText::from_source(
                source,
                TextRange::new(base_offset, base_offset + base.len()),
            )?,
            ruby: ScannedDialogueText::decoded(ruby.decoded(), ruby.content_range()),
        }),
        1,
        0,
    ))
}

fn scan_interpolation(
    source: &str,
    start: usize,
    spelling: &str,
    open_character: char,
    close_character: char,
    form: ScannedInterpolationForm,
) -> Option<ScannedDialogueSurface> {
    source.get(start..)?.starts_with(spelling).then_some(())?;
    let payload_start = start + spelling.len();
    let close = balanced_close(source, payload_start, open_character, close_character)?;
    let end = close + close_character.len_utf8();
    Some(scanned(
        TextRange::new(start, end),
        ScannedDialogueSurfaceKind::Interpolation {
            form,
            open: TextRange::new(start, payload_start),
            payload: TextRange::new(payload_start, close),
            close: TextRange::new(close, end),
        },
        0,
        0,
    ))
}

fn scan_paired_raw(source: &str, start: usize) -> Option<ScannedDialogueSurface> {
    source.get(start..)?.starts_with("[raw]").then_some(())?;
    let body_start = start + "[raw]".len();
    let close_relative = source.get(body_start..)?.find("[/raw]")?;
    let close_start = body_start + close_relative;
    let end = close_start + "[/raw]".len();
    Some(scanned(
        TextRange::new(start, end),
        ScannedDialogueSurfaceKind::Raw {
            form: ScannedRawForm::Paired,
            open: TextRange::new(start, body_start),
            body: ScannedDialogueText::from_source(
                source,
                TextRange::new(body_start, close_start),
            )?,
            close: TextRange::new(close_start, end),
        },
        1,
        0,
    ))
}

fn scan_inline_raw(source: &str, start: usize) -> Option<ScannedDialogueSurface> {
    source.get(start..)?.starts_with("[raw:").then_some(())?;
    let payload_start = start + "[raw:".len();
    let close = balanced_close(source, payload_start, '[', ']')?;
    let untrimmed = source.get(payload_start..close)?;
    let body = untrimmed.trim_start_matches(char::is_whitespace);
    let body_start = slice_offset(source, body)?;
    let end = close + ']'.len_utf8();
    Some(scanned(
        TextRange::new(start, end),
        ScannedDialogueSurfaceKind::Raw {
            form: ScannedRawForm::Inline,
            open: TextRange::new(start, payload_start),
            body: ScannedDialogueText::from_source(
                source,
                TextRange::new(body_start, body_start + body.len()),
            )?,
            close: TextRange::new(close, end),
        },
        1,
        0,
    ))
}

fn scan_inline_style(source: &str, start: usize) -> Option<ScannedDialogueSurface> {
    let boundary = find_dialogue_tag_boundary_before(source, start, source.len())?;
    boundary
        .unterminated_quote_start()
        .is_none()
        .then_some(())?;
    let inside_source = source.get(start + '['.len_utf8()..boundary.close())?;
    let inside = trim_rich_text_whitespace(inside_source);
    let (head, body) = split_once_top_level(inside, ':')?;
    (!body.is_empty()).then_some(())?;
    let head = trim_rich_text_whitespace(head);
    let head_start = slice_offset(source, head)?;
    let (style, name_len, value_source) = if head == "em" {
        (ScannedInlineStyleKind::Emphasis, head.len(), None)
    } else if head == "strong" {
        (ScannedInlineStyleKind::Strong, head.len(), None)
    } else {
        let value = trim_rich_text_whitespace(head.strip_prefix("color")?);
        (!value.is_empty()).then_some(())?;
        (ScannedInlineStyleKind::Color, "color".len(), Some(value))
    };
    let value = match value_source {
        Some(value) => Some(scan_tag_arg_value(value, slice_offset(source, value)?).ok()?),
        None => None,
    };
    let body_start = slice_offset(source, body)?;
    let separator = body_start.checked_sub(':'.len_utf8())?;
    let end = boundary.end();
    Some(scanned(
        TextRange::new(start, end),
        ScannedDialogueSurfaceKind::InlineStyle(ScannedInlineStyle {
            style,
            name: TextRange::new(head_start, head_start + name_len),
            value,
            body: ScannedDialogueText::from_source(
                source,
                TextRange::new(body_start, body_start + body.len()),
            )?,
            separator: TextRange::new(separator, body_start),
            inferred_end: boundary.close(),
        }),
        2,
        usize::from(value_source.is_some()),
    ))
}

fn balanced_close(source: &str, start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 1_u32;
    for (relative, character) in source.get(start..)?.char_indices() {
        if character == open {
            depth = depth.checked_add(1)?;
        } else if character == close {
            depth -= 1;
            if depth == 0 {
                return Some(start + relative);
            }
        }
    }
    None
}

fn split_tag_head(source: &str) -> (&str, &str) {
    source
        .char_indices()
        .find_map(|(index, character)| character.is_whitespace().then_some(index))
        .map_or((source, &source[source.len()..]), |index| {
            (
                &source[..index],
                trim_rich_text_whitespace(&source[index..]),
            )
        })
}

fn split_once_top_level(source: &str, needle: char) -> Option<(&str, &str)> {
    let mut bracket = 0_u32;
    let mut paren = 0_u32;
    let mut brace = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in source.char_indices() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active {
                quote = None;
            }
            continue;
        }
        match character {
            '"' | '\'' => quote = Some(character),
            '[' => bracket += 1,
            ']' => bracket = bracket.saturating_sub(1),
            '(' => paren += 1,
            ')' => paren = paren.saturating_sub(1),
            '{' => brace += 1,
            '}' => brace = brace.saturating_sub(1),
            _ if character == needle && bracket == 0 && paren == 0 && brace == 0 => {
                return Some((&source[..index], &source[index + character.len_utf8()..]));
            }
            _ => {}
        }
    }
    None
}

fn valid_compact_ruby_base(base: &str) -> bool {
    !base.is_empty()
        && base.chars().all(|character| {
            !character.is_whitespace() && !matches!(character, '[' | ']' | '{' | '}' | '#' | '|')
        })
}

fn slice_offset(source: &str, slice: &str) -> Option<usize> {
    let source_start = source.as_ptr() as usize;
    let source_end = source_start.checked_add(source.len())?;
    let slice_start = slice.as_ptr() as usize;
    let slice_end = slice_start.checked_add(slice.len())?;
    (source_start <= slice_start && slice_end <= source_end).then_some(slice_start - source_start)
}

#[cfg(test)]
mod tests {
    use super::{
        ScannedDialogueSurfaceKind, ScannedInlineStyleKind, ScannedInterpolationForm,
        ScannedRawForm, ScannedRubyForm, scan_dialogue_surface,
    };
    use crate::ast::common::TextRange;

    #[test]
    fn scans_semantic_text_and_exact_ranges_for_every_non_tag_surface() {
        for (source, expected) in [
            ("\\[", "escape"),
            ("｜漢字《かんじ》", "ruby"),
            ("|[base](ruby)", "ruby"),
            ("|base{ruby}", "ruby"),
            ("#[actor.name]", "interpolation"),
            ("$(actor.name)", "interpolation"),
            ("[raw]a[b]c[/raw]", "raw"),
            ("[raw: a[b]c]", "raw"),
        ] {
            let surface = scan_dialogue_surface(source, 0, source.len()).expect(source);
            assert_eq!(surface.range(), TextRange::new(0, source.len()), "{source}");
            match expected {
                "escape" => assert!(matches!(
                    surface.kind(),
                    ScannedDialogueSurfaceKind::Escape { value: '[', .. }
                )),
                "ruby" => {
                    let ScannedDialogueSurfaceKind::Ruby(ruby) = surface.kind() else {
                        panic!("unexpected surface for {source}");
                    };
                    assert!(matches!(
                        ruby.form,
                        ScannedRubyForm::Natural
                            | ScannedRubyForm::AsciiExplicit
                            | ScannedRubyForm::AsciiCompact
                    ));
                    assert!(!ruby.base().value().is_empty());
                    assert!(matches!(ruby.ruby().value(), "ruby" | "かんじ"));
                }
                "interpolation" => assert!(matches!(
                    surface.kind(),
                    ScannedDialogueSurfaceKind::Interpolation {
                        form: ScannedInterpolationForm::HashBracket
                            | ScannedInterpolationForm::DollarParen,
                        ..
                    }
                )),
                "raw" => {
                    let ScannedDialogueSurfaceKind::Raw { form, body, .. } = surface.kind() else {
                        panic!("unexpected surface for {source}");
                    };
                    assert!(matches!(
                        form,
                        ScannedRawForm::Paired | ScannedRawForm::Inline
                    ));
                    assert_eq!(body.value(), "a[b]c");
                }
                _ => panic!("unexpected expected surface `{expected}`"),
            }
        }
    }

    #[test]
    fn bracket_ruby_retains_decoded_reading_without_tag_identity() {
        let source = "[ruby rt=\"ka\\nji\"]base[/ruby]";
        let surface = scan_dialogue_surface(source, 0, source.len()).expect("bracket ruby");
        let ScannedDialogueSurfaceKind::Ruby(ruby) = surface.kind() else {
            panic!("ruby surface");
        };
        assert_eq!(ruby.form, ScannedRubyForm::Bracket);
        assert_eq!(ruby.base().value(), "base");
        assert_eq!(ruby.ruby().value(), "ka\nji");
        assert_eq!(surface.rich_text_tags(), 1);
        assert_eq!(surface.rich_text_arguments(), 0);
    }

    #[test]
    fn inline_style_retains_typed_style_value_body_and_inferred_end() {
        let source = "[color \"a]b\":night]";
        let surface = scan_dialogue_surface(source, 0, source.len()).expect("inline style");
        let ScannedDialogueSurfaceKind::InlineStyle(style) = surface.kind() else {
            panic!("inline style surface");
        };
        assert_eq!(style.style(), ScannedInlineStyleKind::Color);
        assert_eq!(style.value().expect("color value").decoded(), "a]b");
        assert_eq!(style.body().value(), "night");
        assert_eq!(style.inferred_end(), source.len() - 1);
        assert_eq!(surface.rich_text_tags(), 2);
        assert_eq!(surface.rich_text_arguments(), 1);
    }

    #[test]
    fn malformed_or_out_of_interval_surfaces_are_not_guessed() {
        for source in ["\\", "#[missing", "|bad base{ruby}", "[raw]open"] {
            assert_eq!(
                scan_dialogue_surface(source, 0, source.len()),
                None,
                "{source}"
            );
        }
        let source = "#[ok]tail";
        assert_eq!(scan_dialogue_surface(source, 0, 4), None);
    }
}
