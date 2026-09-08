//! Typed lexical surfaces consumed by the attached dialogue-content grammar.
//!
//! This scanner runs inside the document parser over its already accepted
//! source interval.  It retains semantic text plus exact authored ranges and
//! never constructs the legacy dialogue AST or reparses an expression.

use crate::ast::common::TextRange;
use crate::name::{is_identifier_continue, is_identifier_start};

/// One grammar-recognized non-plain-text dialogue surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScannedDialogueSurface {
    range: TextRange,
    kind: ScannedDialogueSurfaceKind,
    point_actions: usize,
    action_arguments: usize,
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

    pub(crate) const fn point_actions(&self) -> usize {
        self.point_actions
    }

    pub(crate) const fn action_arguments(&self) -> usize {
        self.action_arguments
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
    /// A `#` content escape whose head and optional attached body are parsed
    /// by their owning grammars.  The body is deliberately kept out of the
    /// ordinary Pratt head range so its brackets cannot become an index
    /// postfix.
    ContentApplication {
        hash: TextRange,
        head: TextRange,
        callee: ScannedContentApplicationCallee,
        body: Option<ScannedContentApplicationBody>,
    },
}

/// Typed lexical shape of a content-application callee.  The ordinary
/// expression projection remains authoritative for call arity and recovery;
/// this shape only distinguishes an implicit-root path from a qualified path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ScannedContentApplicationCallee {
    ImplicitRoot(crate::name::SyntaxName),
    Qualified,
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
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ScannedInterpolationForm {
    HashBracket,
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

    pub(crate) fn value(&self) -> &str {
        &self.decoded
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ScannedContentApplicationBody {
    open: TextRange,
    content: TextRange,
    close: Option<TextRange>,
}

impl ScannedContentApplicationBody {
    pub(crate) const fn open(self) -> TextRange {
        self.open
    }

    pub(crate) const fn content(self) -> TextRange {
        self.content
    }

    pub(crate) const fn close(self) -> Option<TextRange> {
        self.close
    }

    pub(crate) const fn end(self) -> usize {
        match self.close {
            Some(close) => close.end(),
            None => self.content.end(),
        }
    }
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
        '|' => scan_ascii_explicit_ruby(bounded, start),
        '｜' => scan_natural_ruby(bounded, start),
        '#' if tail.starts_with("#[") => scan_interpolation(
            bounded,
            start,
            "#[",
            '[',
            ']',
            ScannedInterpolationForm::HashBracket,
        ),
        '#' => scan_content_application(bounded, start),
        _ => None,
    }
}

fn scanned(
    range: TextRange,
    kind: ScannedDialogueSurfaceKind,
    point_actions: usize,
    action_arguments: usize,
) -> ScannedDialogueSurface {
    ScannedDialogueSurface {
        range,
        kind,
        point_actions,
        action_arguments,
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

fn scan_content_application(source: &str, start: usize) -> Option<ScannedDialogueSurface> {
    let hash_end = start.checked_add('#'.len_utf8())?;
    let (head_end, has_call, callee) = scan_content_expression_head(source, hash_end)?;
    let body = has_call
        .then(|| scan_content_application_body(source, head_end))
        .flatten();
    let end = body.map_or(head_end, ScannedContentApplicationBody::end);
    (hash_end < end).then_some(scanned(
        TextRange::new(start, end),
        ScannedDialogueSurfaceKind::ContentApplication {
            hash: TextRange::new(start, hash_end),
            head: TextRange::new(hash_end, head_end),
            callee,
            body,
        },
        0,
        0,
    ))
}

fn scan_content_application_body(
    source: &str,
    start: usize,
) -> Option<ScannedContentApplicationBody> {
    let open_end = start.checked_add('['.len_utf8())?;
    source.get(start..)?.starts_with('[').then_some(())?;
    let close = balanced_close(source, open_end, '[', ']').and_then(|start| {
        let end = start.checked_add(']'.len_utf8())?;
        Some(TextRange::new(start, end))
    });
    let content_end = close.map_or(source.len(), |close| close.start());
    Some(ScannedContentApplicationBody {
        open: TextRange::new(start, open_end),
        content: TextRange::new(open_end, content_end),
        close,
    })
}

/// Finds the deterministic lexical boundary handed to the ordinary Pratt
/// parser for a `#` escape. This deliberately does not classify callee names:
/// every identifier/path and every parenthesized call uses the same rule.
fn scan_content_expression_head(
    source: &str,
    start: usize,
) -> Option<(usize, bool, ScannedContentApplicationCallee)> {
    let mut cursor = start;
    let first = source.get(cursor..)?.chars().next()?;
    if !is_identifier_start(first) {
        return None;
    }
    cursor = cursor.checked_add(first.len_utf8())?;
    while let Some(character) = source.get(cursor..).and_then(|tail| tail.chars().next()) {
        if !is_identifier_continue(character) {
            break;
        }
        cursor = cursor.checked_add(character.len_utf8())?;
    }
    let first_end = cursor;

    let mut qualified = false;
    loop {
        let separator_len = if source.get(cursor..)?.starts_with('.') {
            '.'.len_utf8()
        } else if source.get(cursor..)?.starts_with("::") {
            "::".len()
        } else {
            break;
        };
        qualified = true;
        let segment_start = cursor.checked_add(separator_len)?;
        let segment = source.get(segment_start..)?.chars().next()?;
        if !is_identifier_start(segment) {
            break;
        }
        cursor = segment_start.checked_add(segment.len_utf8())?;
        while let Some(character) = source.get(cursor..).and_then(|tail| tail.chars().next()) {
            if !is_identifier_continue(character) {
                break;
            }
            cursor = cursor.checked_add(character.len_utf8())?;
        }
    }

    while source
        .get(cursor..)
        .and_then(|tail| tail.chars().next())
        .is_some_and(char::is_whitespace)
    {
        cursor = cursor.checked_add(source.get(cursor..)?.chars().next()?.len_utf8())?;
    }
    let has_call = source.get(cursor..)?.starts_with('(');
    if has_call {
        cursor = balanced_close(source, cursor + '('.len_utf8(), '(', ')')
            .and_then(|close| close.checked_add(')'.len_utf8()))
            .unwrap_or(source.len());
    }
    let callee = if qualified {
        ScannedContentApplicationCallee::Qualified
    } else {
        ScannedContentApplicationCallee::ImplicitRoot(
            crate::name::SyntaxName::try_new(source.get(start..first_end)?).ok()?,
        )
    };
    Some((cursor, has_call, callee))
}

fn balanced_close(source: &str, start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 1_u32;
    let mut quote = None;
    let mut escaped = false;
    for (relative, character) in source.get(start..)?.char_indices() {
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
        if matches!(character, '"' | '\'') {
            quote = Some(character);
            continue;
        }
        if character == open {
            depth = depth.checked_add(1)?;
        } else if character == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(start + relative);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{ScannedDialogueSurfaceKind, ScannedInterpolationForm, scan_dialogue_surface};
    use crate::ast::common::TextRange;

    #[test]
    fn scans_retained_surfaces_and_canonical_content_calls() {
        for (source, expected) in [
            ("\\[", "escape"),
            ("｜漢字《かんじ》", "ruby"),
            ("|[base](ruby)", "ruby"),
            ("#[actor.name]", "interpolation"),
            ("#name", "content_application"),
            ("#call(args)[#nested()[text]]", "content_application"),
        ] {
            let surface = scan_dialogue_surface(source, 0, source.len()).expect(source);
            assert_eq!(surface.range(), TextRange::new(0, source.len()), "{source}");
            match expected {
                "escape" => assert!(matches!(
                    surface.kind(),
                    ScannedDialogueSurfaceKind::Escape { value: '[', .. }
                )),
                "ruby" => assert!(matches!(
                    surface.kind(),
                    ScannedDialogueSurfaceKind::Ruby(_)
                )),
                "interpolation" => assert!(matches!(
                    surface.kind(),
                    ScannedDialogueSurfaceKind::Interpolation {
                        form: ScannedInterpolationForm::HashBracket,
                        ..
                    }
                )),
                "content_application" => assert!(matches!(
                    surface.kind(),
                    ScannedDialogueSurfaceKind::ContentApplication { .. }
                )),
                _ => panic!("unexpected expected surface `{expected}`"),
            }
        }
    }

    #[test]
    fn raw_is_retained_only_as_the_canonical_hash_call_body() {
        let source = "#raw()[a[b]c]";
        let surface = scan_dialogue_surface(source, 0, source.len()).expect("raw call");
        let ScannedDialogueSurfaceKind::ContentApplication { head, body, .. } = surface.kind()
        else {
            panic!("raw content application surface");
        };
        assert_eq!(*head, TextRange::new(1, 6));
        let body = body.expect("raw body");
        assert_eq!(body.content(), TextRange::new(7, 12));
        assert_eq!(body.close(), Some(TextRange::new(12, 13)));
    }

    #[test]
    fn spaced_raw_and_non_raw_heads_have_distinct_typed_shapes() {
        let raw_source = "#raw ()[a[b]]";
        let raw = scan_dialogue_surface(raw_source, 0, raw_source.len()).expect("spaced raw");
        let ordinary_source = "#object ()[#[value]]";
        let ordinary =
            scan_dialogue_surface(ordinary_source, 0, ordinary_source.len()).expect("ordinary");
        let ScannedDialogueSurfaceKind::ContentApplication { head, .. } = raw.kind() else {
            panic!("raw surface");
        };
        assert_eq!(*head, TextRange::new(1, 7));
        let ScannedDialogueSurfaceKind::ContentApplication { callee, .. } = ordinary.kind() else {
            panic!("ordinary surface");
        };
        assert!(matches!(
            callee,
            super::ScannedContentApplicationCallee::ImplicitRoot(_)
        ));
    }
}
