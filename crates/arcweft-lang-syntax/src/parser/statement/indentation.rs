//! Token-linear indentation geometry for statement-owned suites.
//!
//! This module measures exact lexer-token coordinates. It never rebuilds or
//! reparses source strings; grammar-family modules remain responsible for
//! deciding which typed child kind each logical line owns.

use super::super::cursor::{DocumentParser, is_trivia_kind};
use super::super::expression::is_expression_continuation_token;
use super::super::shadow_recovery::token_text;
use crate::grammar::kinds::SyntaxKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum IndentedSuiteIssue {
    MissingNewline,
    MissingIndentedItem,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::parser) struct IndentedSuiteInterval {
    colon: usize,
    payload_start: usize,
    first_item: usize,
    end: usize,
    item_indent: Option<usize>,
    issue: Option<IndentedSuiteIssue>,
}

/// Monotonic indentation cache for one already-measured suite.
///
/// Same-line semicolon siblings reuse the first item's indentation. When an
/// emitted item crossed a physical line, `observe` scans only that newly
/// consumed interval before measuring the next line. This keeps the suite
/// loops linear without adding parser-global cursor state.
pub(super) struct SuiteLineIndentCursor {
    previous_item_start: usize,
    current_indent: usize,
}

impl SuiteLineIndentCursor {
    pub(super) const fn new(first_item: usize, first_indent: usize) -> Self {
        Self {
            previous_item_start: first_item,
            current_indent: first_indent,
        }
    }

    pub(super) fn observe(&mut self, parser: &DocumentParser<'_, '_>, item_start: usize) -> usize {
        if item_start != self.previous_item_start
            && has_newline_between(parser, self.previous_item_start, item_start)
        {
            self.current_indent =
                token_indent(parser, physical_line_owner_start(parser, item_start));
        }
        self.previous_item_start = item_start;
        self.current_indent
    }
}

impl IndentedSuiteInterval {
    pub(super) const fn colon(self) -> usize {
        self.colon
    }

    pub(super) const fn payload_start(self) -> usize {
        self.payload_start
    }

    pub(super) const fn first_item(self) -> usize {
        self.first_item
    }

    pub(super) const fn end(self) -> usize {
        self.end
    }

    pub(super) const fn item_indent(self) -> Option<usize> {
        self.item_indent
    }

    pub(super) const fn issue(self) -> Option<IndentedSuiteIssue> {
        self.issue
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ContentLine {
    content: usize,
    indent: usize,
    next_line: usize,
}

/// Finds the first top-level braced or indentation body introducer on the
/// current physical head line.
pub(super) fn head_body_introducer(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<usize> {
    let mut delimiters = Vec::<&str>::new();
    for index in start..end {
        let token = parser.token_at(index)?;
        if token.kind() == SyntaxKind::NewlineToken {
            return None;
        }
        let text = parser.text_of(token);
        if delimiters.is_empty() && matches!(text, "{" | ":") {
            return Some(index);
        }
        match text {
            "(" | "[" => delimiters.push(text),
            ")" if delimiters.last() == Some(&"(") => {
                delimiters.pop();
            }
            "]" if delimiters.last() == Some(&"[") => {
                delimiters.pop();
            }
            _ => {}
        }
    }
    None
}

/// Finds the final owner-level braced or indented body introducer in one
/// bounded head. Earlier braces remain available to the shared expression or
/// pattern parser as record/block payloads.
pub(super) fn trailing_owner_body_token(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    end: usize,
    allow_colon: bool,
) -> Option<usize> {
    scan_owner_body_suffix(parser, start, end, allow_colon).0
}

/// Finds the final owner-level braced body and its exclusive close boundary
/// with one forward token scan. An unclosed final body extends to `end`.
pub(super) fn trailing_braced_body_interval(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<(usize, usize)> {
    scan_owner_body_suffix(parser, start, end, false).1
}

fn scan_owner_body_suffix(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    end: usize,
    allow_colon: bool,
) -> (Option<usize>, Option<(usize, usize)>) {
    let mut delimiters = Vec::<&str>::new();
    let mut outer_brace = None;
    let mut body_token = None;
    let mut body_token_is_colon = false;
    let mut braced_body = None;

    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            break;
        };
        let text = parser.text_of(token);
        if body_token_is_colon && !is_trivia_kind(token.kind()) {
            body_token = None;
            body_token_is_colon = false;
        }
        if delimiters.is_empty()
            && outer_brace.is_none()
            && braced_body.is_some()
            && is_expression_continuation_token(text)
        {
            body_token = None;
            braced_body = None;
        }
        match text {
            "(" | "[" | "{" => {
                if delimiters.is_empty() && text == "{" {
                    outer_brace = Some(index);
                    body_token = Some(index);
                    body_token_is_colon = false;
                    braced_body = Some((index, end));
                }
                delimiters.push(text);
            }
            ")" | "]" | "}" => {
                let expected = match text {
                    ")" => "(",
                    "]" => "[",
                    "}" => "{",
                    _ => unreachable!(),
                };
                if delimiters.last().copied() == Some(expected) {
                    let closes_outer = delimiters.len() == 1 && expected == "{";
                    delimiters.pop();
                    if closes_outer && let Some(open) = outer_brace.take() {
                        braced_body = Some((open, index.saturating_add(1)));
                    }
                }
            }
            ":" if allow_colon && delimiters.is_empty() => {
                body_token = Some(index);
                body_token_is_colon = true;
            }
            _ => {}
        }
    }

    (body_token, braced_body)
}

pub(super) fn physical_line_end(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    limit: usize,
) -> usize {
    (start..limit)
        .find(|index| {
            parser
                .token_at(*index)
                .is_some_and(|token| token.kind() == SyntaxKind::NewlineToken)
        })
        .unwrap_or(limit)
}

pub(super) fn has_newline_between(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> bool {
    (start..end).any(|index| {
        parser
            .token_at(index)
            .is_some_and(|token| token.kind() == SyntaxKind::NewlineToken)
    })
}

/// Whether `index` is the first token boundary of a physical source line.
///
/// Suite intervals end immediately after the terminating newline of their
/// final accepted item.  Treating that boundary as "same line" would let the
/// following dedented statement leak back into the suite owner.
pub(super) fn starts_physical_line(parser: &DocumentParser<'_, '_>, index: usize) -> bool {
    index == 0
        || parser
            .token_at(index.saturating_sub(1))
            .is_some_and(|token| token.kind() == SyntaxKind::NewlineToken)
}

/// Consumes trivia only after the owning suite has committed that exact token
/// interval. Pending trivia before a dedented sibling remains with the outer
/// owner.
pub(super) fn bump_trivia_before(parser: &mut DocumentParser<'_, '_>, end: usize) {
    while parser.cursor() < end && parser.current_kind().is_some_and(is_trivia_kind) {
        parser.bump();
    }
}

/// Measures one `:`-introduced suite through the first owner-level dedent.
pub(super) fn indented_suite_interval(
    parser: &DocumentParser<'_, '_>,
    owner_start: usize,
    colon: usize,
    limit: usize,
) -> IndentedSuiteInterval {
    debug_assert_eq!(token_text(parser, colon), Some(":"));
    let owner_indent = token_indent(parser, owner_start);
    let mut index = colon.saturating_add(1);
    let mut newline = None;
    let mut inline_payload = false;
    while index < limit {
        let Some(token) = parser.token_at(index) else {
            break;
        };
        if token.kind() == SyntaxKind::NewlineToken {
            newline = Some(index);
            break;
        }
        if !is_horizontal_trivia(token.kind()) {
            inline_payload = true;
        }
        index += 1;
    }

    let Some(newline) = newline else {
        return IndentedSuiteInterval {
            colon,
            payload_start: colon.saturating_add(1),
            first_item: limit,
            end: limit,
            item_indent: None,
            issue: Some(IndentedSuiteIssue::MissingNewline),
        };
    };
    let payload_start = newline.saturating_add(1);
    if inline_payload {
        return IndentedSuiteInterval {
            colon,
            payload_start: colon.saturating_add(1),
            first_item: limit,
            end: newline,
            item_indent: None,
            issue: Some(IndentedSuiteIssue::MissingNewline),
        };
    }

    let Some(first) = next_content_line(parser, payload_start, limit) else {
        return IndentedSuiteInterval {
            colon,
            payload_start,
            first_item: limit,
            end: limit,
            item_indent: None,
            issue: Some(IndentedSuiteIssue::MissingIndentedItem),
        };
    };
    if first.indent <= owner_indent {
        return IndentedSuiteInterval {
            colon,
            payload_start,
            first_item: first.content,
            // Buffered blank/comment-only rows belong to the outer owner when
            // the first real content line is already dedented.
            end: payload_start,
            item_indent: None,
            issue: Some(IndentedSuiteIssue::MissingIndentedItem),
        };
    }

    let mut accepted_end = first.next_line;
    let mut scan = first.next_line;
    while let Some(line) = next_content_line(parser, scan, limit) {
        if line.indent <= owner_indent {
            return IndentedSuiteInterval {
                colon,
                payload_start,
                first_item: first.content,
                end: accepted_end,
                item_indent: Some(first.indent),
                issue: None,
            };
        }
        accepted_end = line.next_line;
        scan = line.next_line;
    }

    IndentedSuiteInterval {
        colon,
        payload_start,
        first_item: first.content,
        end: limit,
        item_indent: Some(first.indent),
        issue: None,
    }
}

/// Finds the exclusive token boundary of one source-ordered suite item.
pub(super) fn indented_item_end(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    suite_end: usize,
    suite_indent: usize,
    is_sibling_head: impl Fn(Option<SyntaxKind>, Option<&str>) -> bool,
    is_continuation_head: impl Fn(Option<SyntaxKind>, Option<&str>) -> bool,
) -> usize {
    let mut delimiters = Vec::<&str>::new();
    let mut index = start;
    while index < suite_end {
        let Some(token) = parser.token_at(index) else {
            return suite_end;
        };
        let text = parser.text_of(token);
        if delimiters.is_empty() && text == ";" {
            return index;
        }
        if token.kind() == SyntaxKind::NewlineToken
            && let Some(next) = next_content_line(parser, index.saturating_add(1), suite_end)
        {
            let next_text = token_text(parser, next.content);
            let closes_current = matches!(
                (delimiters.last().copied(), next_text),
                (Some("("), Some(")")) | (Some("["), Some("]")) | (Some("{"), Some("}"))
            );
            let continues_current = next.indent == suite_indent
                && is_continuation_head(
                    parser
                        .token_at(next.content)
                        .map(super::super::lexer::LexToken::kind),
                    next_text,
                );
            if !closes_current
                && !continues_current
                && next.indent <= suite_indent
                && (delimiters.is_empty()
                    || next.indent < suite_indent
                    || is_sibling_head(
                        parser
                            .token_at(next.content)
                            .map(super::super::lexer::LexToken::kind),
                        next_text,
                    ))
            {
                // Preserve the terminating newline for a nested `:` owner,
                // while leaving all later buffered trivia with the outer
                // suite. Callers trim it for non-suite items.
                return index.saturating_add(1).min(suite_end);
            }

            // `next_content_line` has already classified every intervening
            // blank/comment-only row.  Jump to its content instead of
            // rescanning the same trivia after every newline; each token is
            // therefore visited only a constant number of times.
            index = next.content;
            continue;
        } else if token.kind() == SyntaxKind::NewlineToken {
            return suite_end;
        }
        match text {
            "(" | "[" | "{" => delimiters.push(text),
            ")" if delimiters.last() == Some(&"(") => {
                delimiters.pop();
            }
            "]" if delimiters.last() == Some(&"[") => {
                delimiters.pop();
            }
            "}" if delimiters.last() == Some(&"{") => {
                delimiters.pop();
            }
            _ => {}
        }
        index += 1;
    }
    suite_end
}

fn next_content_line(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    limit: usize,
) -> Option<ContentLine> {
    let mut line_start_offset = token_boundary_offset(parser, start);
    let mut first_content = None;
    let mut index = start;
    while index < limit {
        let token = parser.token_at(index)?;
        match token.kind() {
            SyntaxKind::NewlineToken => {
                if let Some(content) = first_content {
                    let indent = parser
                        .token_at(content)
                        .expect("line content token exists")
                        .range()
                        .start()
                        .saturating_sub(line_start_offset);
                    return Some(ContentLine {
                        content,
                        indent,
                        next_line: index.saturating_add(1),
                    });
                }
                line_start_offset = token.range().end();
                first_content = None;
            }
            SyntaxKind::WhitespaceToken
            | SyntaxKind::CommentToken
            | SyntaxKind::DocCommentToken => {}
            _ => {
                first_content.get_or_insert(index);
            }
        }
        index += 1;
    }
    first_content.map(|content| {
        let indent = parser
            .token_at(content)
            .expect("line content token exists")
            .range()
            .start()
            .saturating_sub(line_start_offset);
        ContentLine {
            content,
            indent,
            next_line: limit,
        }
    })
}

pub(super) fn token_indent(parser: &DocumentParser<'_, '_>, index: usize) -> usize {
    let offset = parser
        .token_at(index)
        .expect("indentation anchor token exists")
        .range()
        .start();
    let line_start = (0..index)
        .rev()
        .find_map(|candidate| {
            parser.token_at(candidate).and_then(|token| {
                (token.kind() == SyntaxKind::NewlineToken).then_some(token.range().end())
            })
        })
        .unwrap_or(0);
    offset.saturating_sub(line_start)
}

/// Returns the first non-trivia token on the physical line containing
/// `index`. Indentation-owned expression suites use the line owner rather
/// than the inline expression token as their dedent baseline.
pub(super) fn physical_line_owner_start(parser: &DocumentParser<'_, '_>, index: usize) -> usize {
    let line_start = (0..index)
        .rev()
        .find(|candidate| {
            parser
                .token_at(*candidate)
                .is_some_and(|token| token.kind() == SyntaxKind::NewlineToken)
        })
        .map_or(0, |newline| newline.saturating_add(1));
    (line_start..=index)
        .find(|candidate| {
            parser
                .token_at(*candidate)
                .is_some_and(|token| !is_trivia_kind(token.kind()))
        })
        .unwrap_or(index)
}

fn token_boundary_offset(parser: &DocumentParser<'_, '_>, index: usize) -> usize {
    parser
        .token_at(index)
        .map_or_else(|| parser.current_offset(), |token| token.range().start())
}

const fn is_horizontal_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::WhitespaceToken | SyntaxKind::CommentToken | SyntaxKind::DocCommentToken
    )
}
