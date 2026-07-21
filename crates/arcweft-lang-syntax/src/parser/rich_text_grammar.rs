//! Private `RichText` descendants emitted inside the shared dialogue grammar.
//!
//! This module consumes the document lexer's existing cursor and the neutral
//! argument scan owned by `text::rich_text_tag`. It never invokes the public
//! dialogue parser, reparses a source substring, or wraps detached AST nodes.

use arcweft_source::SourceRange;

use super::document::ShadowDocumentParser;
use super::expression::emit_expression;
use super::shadow_recovery::{emit_close_delimiter, emit_open_delimiter};
use crate::ast::common::TextRange;
use crate::ast::dialogue::DialogueTagArgSyntaxIssue;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::text::{
    DialogueTextDiagnosticCode, MAX_RICH_TEXT_CONTENT_ARGUMENTS, MAX_RICH_TEXT_CONTENT_TAGS,
    MAX_RICH_TEXT_TAG_BODY_BYTES, ScannedTagArgValue, ScannedTagArgValueSurface,
    ScannedTagArgument, ScannedTagArguments, find_dialogue_tag_boundary_before,
    is_rich_text_whitespace, scan_dialogue_opaque_surface, scan_tag_arguments,
    trim_rich_text_whitespace, utf8_boundary_at_or_before,
};

pub(super) fn emit_dialogue_rich_text(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    let content_end = parser
        .offset_at_token_boundary(end)
        .expect("dialogue content end is a lexer boundary");
    let mut content_tag_count = 0_usize;
    let mut attached_tag_ordinal = 0_usize;
    let mut argument_count = 0_usize;
    let mut tag_limit_exhausted = false;
    let mut argument_limit_exhausted = false;

    while parser.cursor() < end {
        let start = parser.current_offset();
        if consume_tag_after_content_limit(
            parser,
            start,
            content_end,
            content_tag_count,
            &mut tag_limit_exhausted,
        ) || consume_opaque_dialogue_surface(
            parser,
            start,
            content_end,
            &mut content_tag_count,
            &mut argument_count,
            &mut tag_limit_exhausted,
            &mut argument_limit_exhausted,
        ) || consume_overlong_tag(parser, start, content_end)
        {
            continue;
        }

        let Some(surface) = RichTextTagSurface::scan(parser, content_end) else {
            let _ = parser.bump();
            continue;
        };
        if parser.token_boundary_index(surface.end).is_none() {
            let _ = parser.bump();
            continue;
        }

        match surface.body {
            RichTextTagBody::Open(open) => {
                emit_open_tag(
                    parser,
                    surface,
                    open,
                    u32::try_from(attached_tag_ordinal).expect("RichText tag limit fits u32"),
                    &mut argument_count,
                    &mut argument_limit_exhausted,
                );
            }
            RichTextTagBody::End { name_range } => {
                emit_end_tag(
                    parser,
                    surface,
                    name_range,
                    u32::try_from(attached_tag_ordinal).expect("RichText tag limit fits u32"),
                );
            }
        }
        content_tag_count += 1;
        attached_tag_ordinal += 1;
    }
}

#[derive(Clone, Copy)]
enum RichTextContentLimit {
    Tags,
    Arguments,
}

fn consume_tag_after_content_limit(
    parser: &mut ShadowDocumentParser<'_, '_>,
    start: usize,
    content_end: usize,
    content_tag_count: usize,
    tag_limit_exhausted: &mut bool,
) -> bool {
    if !parser.at("[") || (!*tag_limit_exhausted && content_tag_count < MAX_RICH_TEXT_CONTENT_TAGS)
    {
        return false;
    }
    if let Some(boundary) = find_dialogue_tag_boundary_before(parser.source(), start, content_end) {
        if !core::mem::replace(tag_limit_exhausted, true) {
            emit_content_limit_diagnostic(
                parser,
                RichTextContentLimit::Tags,
                SourceRange::new(start, boundary.end()),
            );
        }
        consume_opaque_to(parser, boundary.end());
    } else {
        let _ = parser.bump();
    }
    true
}

fn consume_opaque_dialogue_surface(
    parser: &mut ShadowDocumentParser<'_, '_>,
    start: usize,
    content_end: usize,
    content_tag_count: &mut usize,
    argument_count: &mut usize,
    tag_limit_exhausted: &mut bool,
    argument_limit_exhausted: &mut bool,
) -> bool {
    let Some(opaque) = scan_dialogue_opaque_surface(parser.source(), start, content_end) else {
        return false;
    };
    let tag_overflow =
        opaque.rich_text_tags() > MAX_RICH_TEXT_CONTENT_TAGS.saturating_sub(*content_tag_count);
    let argument_overflow = opaque.rich_text_arguments()
        > MAX_RICH_TEXT_CONTENT_ARGUMENTS.saturating_sub(*argument_count);
    if tag_overflow {
        if !core::mem::replace(tag_limit_exhausted, true) {
            emit_content_limit_diagnostic(
                parser,
                RichTextContentLimit::Tags,
                SourceRange::new(start, opaque.end()),
            );
        }
    } else if argument_overflow {
        if !core::mem::replace(argument_limit_exhausted, true) {
            emit_content_limit_diagnostic(
                parser,
                RichTextContentLimit::Arguments,
                SourceRange::new(start, opaque.end()),
            );
        }
    } else {
        *content_tag_count += opaque.rich_text_tags();
        *argument_count += opaque.rich_text_arguments();
    }
    consume_opaque_to(parser, opaque.end());
    true
}

fn consume_overlong_tag(
    parser: &mut ShadowDocumentParser<'_, '_>,
    start: usize,
    content_end: usize,
) -> bool {
    if !parser.at("[") {
        return false;
    }
    let Some(boundary) = find_dialogue_tag_boundary_before(parser.source(), start, content_end)
    else {
        return false;
    };
    let inside = &parser.source()[start + '['.len_utf8()..boundary.close()];
    if inside.len() <= MAX_RICH_TEXT_TAG_BODY_BYTES {
        return false;
    }
    let limit = utf8_boundary_at_or_before(inside, MAX_RICH_TEXT_TAG_BODY_BYTES);
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        DialogueTextDiagnosticCode::RichTextTagBodyTooLong.as_str(),
        SourceRange::new(start + '['.len_utf8() + limit, boundary.close()),
        format!("dialogue RichText tag body exceeds {MAX_RICH_TEXT_TAG_BODY_BYTES} bytes"),
    )));
    consume_opaque_to(parser, boundary.end());
    true
}

fn emit_content_limit_diagnostic(
    parser: &mut ShadowDocumentParser<'_, '_>,
    limit: RichTextContentLimit,
    range: SourceRange,
) {
    let (code, message) = match limit {
        RichTextContentLimit::Tags => (
            DialogueTextDiagnosticCode::RichTextContentTagLimit.as_str(),
            format!("dialogue content has more than {MAX_RICH_TEXT_CONTENT_TAGS} RichText tags"),
        ),
        RichTextContentLimit::Arguments => (
            DialogueTextDiagnosticCode::RichTextContentArgumentLimit.as_str(),
            format!(
                "dialogue content has more than {MAX_RICH_TEXT_CONTENT_ARGUMENTS} RichText arguments"
            ),
        ),
    };
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code, range, message,
    )));
}

fn consume_opaque_to(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    while parser.current_offset() < end {
        let _ = parser
            .bump()
            .expect("opaque dialogue surface remains inside the accepted content");
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RichTextTagSurface<'source> {
    end: usize,
    unterminated_quote: Option<TextRange>,
    body: RichTextTagBody<'source>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RichTextTagBody<'source> {
    Open(OpenTagSurface<'source>),
    End { name_range: TextRange },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpenTagSurface<'source> {
    source_name: &'source str,
    name_range: TextRange,
    attrs: &'source str,
    attrs_range: TextRange,
}

impl<'source> RichTextTagSurface<'source> {
    fn scan(parser: &ShadowDocumentParser<'source, '_>, content_end: usize) -> Option<Self> {
        parser.at("[").then_some(())?;
        let open = parser.current_offset();
        let boundary = find_dialogue_tag_boundary_before(parser.source(), open, content_end)?;
        let close = boundary.close();
        let end = boundary.end();
        let unterminated_quote = boundary
            .unterminated_quote_start()
            .map(|start| TextRange::new(start, end));
        let inside_source = parser.source().get(open + 1..close)?;
        let inside = trim_rich_text_whitespace(inside_source);
        if inside.is_empty() {
            return None;
        }
        let inside_start = open + 1 + subslice_offset(inside_source, inside);

        if let Some(name) = inside.strip_prefix('/') {
            let name = trim_rich_text_whitespace(name);
            let name_start = inside_start + 1 + subslice_offset(&inside[1..], name);
            return Some(Self {
                end,
                unterminated_quote,
                body: RichTextTagBody::End {
                    name_range: TextRange::new(name_start, name_start + name.len()),
                },
            });
        }

        let (source_name, attrs, name_start) = if let Some(attrs) = inside.strip_prefix('!') {
            ("!", trim_rich_text_whitespace(attrs), inside_start)
        } else {
            let (source_name, attrs) = split_tag_head(inside);
            (
                source_name,
                attrs,
                inside_start + subslice_offset(inside, source_name),
            )
        };
        if source_name.is_empty() {
            return None;
        }
        let attrs_start = inside_start + subslice_offset(inside, attrs);
        Some(Self {
            end,
            unterminated_quote,
            body: RichTextTagBody::Open(OpenTagSurface {
                source_name,
                name_range: TextRange::new(name_start, name_start + source_name.len()),
                attrs,
                attrs_range: TextRange::new(attrs_start, attrs_start + attrs.len()),
            }),
        })
    }
}

fn emit_open_tag(
    parser: &mut ShadowDocumentParser<'_, '_>,
    surface: RichTextTagSurface<'_>,
    open: OpenTagSurface<'_>,
    ordinal: u32,
    content_argument_count: &mut usize,
    argument_limit_exhausted: &mut bool,
) {
    parser.start(SyntaxKind::RichTextTag, SyntaxRole::RichTextTag(ordinal));
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    bump_to_range_start(parser, open.name_range);
    emit_range_node(
        parser,
        SyntaxKind::RichTextTagName,
        SyntaxRole::Name,
        open.name_range,
    );

    if !open.attrs.is_empty() {
        bump_to_range_start(parser, open.attrs_range);
        match open.source_name {
            "mark" => bump_until_offset(parser, open.attrs_range.end()),
            "fx" => emit_expression_payload(
                parser,
                open.attrs_range,
                SyntaxKind::RichTextFxCallPayload,
                SyntaxRole::Operand,
            ),
            "call" | "!" => emit_expression_payload(
                parser,
                open.attrs_range,
                SyntaxKind::RichTextDialogueCallPayload,
                SyntaxRole::Operand,
            ),
            "if" => emit_expression_payload(
                parser,
                open.attrs_range,
                SyntaxKind::RichTextConditionPayload,
                SyntaxRole::Condition,
            ),
            _ => {
                let remaining =
                    MAX_RICH_TEXT_CONTENT_ARGUMENTS.saturating_sub(*content_argument_count);
                let scanned = scan_tag_arguments(open.attrs, open.attrs_range.start(), remaining);
                *content_argument_count =
                    (*content_argument_count).saturating_add(scanned.entries().len());
                emit_argument_payload(
                    parser,
                    open.attrs_range,
                    &scanned,
                    surface.unterminated_quote.is_some(),
                    argument_limit_exhausted,
                );
            }
        }
    }

    let close = surface.end - 1;
    bump_until_offset(parser, close);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.rich_text.tag.missing_close",
    );
    emit_unterminated_quote_diagnostic(parser, surface.unterminated_quote);
    parser.finish();
}

fn emit_end_tag(
    parser: &mut ShadowDocumentParser<'_, '_>,
    surface: RichTextTagSurface<'_>,
    name_range: TextRange,
    ordinal: u32,
) {
    parser.start(SyntaxKind::RichTextEndTag, SyntaxRole::RichTextTag(ordinal));
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    if parser.at("/") {
        let _ = parser.bump();
    }
    bump_to_range_start(parser, name_range);
    if name_range.start() != name_range.end() {
        emit_range_node(
            parser,
            SyntaxKind::RichTextTagName,
            SyntaxRole::Name,
            name_range,
        );
    }
    let close = surface.end - 1;
    bump_until_offset(parser, close);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.rich_text.tag.missing_close",
    );
    emit_unterminated_quote_diagnostic(parser, surface.unterminated_quote);
    parser.finish();
}

fn emit_expression_payload(
    parser: &mut ShadowDocumentParser<'_, '_>,
    range: TextRange,
    kind: SyntaxKind,
    expression_role: SyntaxRole,
) {
    parser.start(kind, SyntaxRole::Payload);
    let end = parser
        .token_boundary_index(range.end())
        .expect("dedicated RichText payload ends at a lexer boundary");
    emit_expression(parser, end, expression_role);
    parser.finish();
}

fn emit_argument_payload(
    parser: &mut ShadowDocumentParser<'_, '_>,
    range: TextRange,
    scanned: &ScannedTagArguments,
    tag_reports_unterminated_quote: bool,
    argument_limit_exhausted: &mut bool,
) {
    parser.start(SyntaxKind::RichTextArgumentPayload, SyntaxRole::Payload);
    parser.start(SyntaxKind::RichTextArgumentList, SyntaxRole::Element(0));
    for (ordinal, argument) in scanned.entries().iter().enumerate() {
        bump_to_range_start(parser, argument.range());
        emit_argument(
            parser,
            argument,
            u16::try_from(ordinal).expect("RichText tag argument limit fits u16"),
        );
    }
    bump_until_offset(parser, range.end());
    parser.finish();
    for diagnostic in scanned.diagnostics() {
        if tag_reports_unterminated_quote
            && diagnostic.code() == DialogueTextDiagnosticCode::RichTextAttributeUnterminatedQuote
        {
            continue;
        }
        if diagnostic.code() == DialogueTextDiagnosticCode::RichTextContentArgumentLimit
            && core::mem::replace(argument_limit_exhausted, true)
        {
            continue;
        }
        let range = diagnostic.range();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            diagnostic.code().as_str(),
            SourceRange::new(range.start(), range.end()),
            diagnostic.message(),
        )));
    }
    parser.finish();
}

fn emit_argument(
    parser: &mut ShadowDocumentParser<'_, '_>,
    argument: &ScannedTagArgument,
    ordinal: u16,
) {
    match argument {
        ScannedTagArgument::Positional { value, range } => {
            parser.start(
                SyntaxKind::RichTextPositionalArgument,
                SyntaxRole::Argument(ordinal),
            );
            let mut cursor = PartitionedEventCursor::new(parser, range.start());
            emit_value_surface(&mut cursor, value);
            cursor.finish_at(range.end());
            parser.finish();
        }
        ScannedTagArgument::Named {
            name_range,
            equals_range,
            value,
            range,
        } => {
            parser.start(
                SyntaxKind::RichTextNamedArgument,
                SyntaxRole::Argument(ordinal),
            );
            let mut cursor = PartitionedEventCursor::new(parser, range.start());
            cursor.start(SyntaxKind::RichTextArgumentKey, SyntaxRole::Key);
            cursor.emit_to(name_range.end());
            cursor.finish();
            cursor.start(SyntaxKind::RichTextArgumentEquals, SyntaxRole::Equals);
            cursor.emit_to_as(equals_range.end(), SyntaxKind::PunctuationToken);
            cursor.finish();
            emit_value_surface(&mut cursor, value);
            cursor.finish_at(range.end());
            parser.finish();
        }
        ScannedTagArgument::Invalid { range, issue } => {
            parser.start(
                SyntaxKind::RichTextInvalidArgument,
                SyntaxRole::Argument(ordinal),
            );
            let mut cursor = PartitionedEventCursor::new(parser, range.start());
            let issue_range = issue_range(issue);
            cursor.emit_to(issue_range.start());
            cursor.start(SyntaxKind::RichTextInvalidArgumentIssue, SyntaxRole::Issue);
            cursor.emit_to(issue_range.end());
            cursor.finish();
            cursor.emit_to(range.end());
            cursor.finish_at(range.end());
            parser.finish();
        }
    }
}

fn emit_value_surface(
    cursor: &mut PartitionedEventCursor<'_, '_, '_>,
    value: &ScannedTagArgValueSurface,
) {
    match value {
        ScannedTagArgValueSurface::Present(value) => emit_present_value(cursor, value),
        ScannedTagArgValueSurface::Missing { range } => {
            cursor.start(SyntaxKind::RichTextMissingArgumentValue, SyntaxRole::Value);
            cursor.emit_to(range.end());
            cursor.finish();
        }
    }
}

fn emit_present_value(cursor: &mut PartitionedEventCursor<'_, '_, '_>, value: &ScannedTagArgValue) {
    cursor.start(SyntaxKind::RichTextArgumentValue, SyntaxRole::Value);
    cursor.start(SyntaxKind::RichTextArgumentToken, SyntaxRole::Token);
    if let Some(opening) = value.opening_quote_range() {
        cursor.start(SyntaxKind::RichTextArgumentQuote, SyntaxRole::OpeningQuote);
        cursor.emit_to_as(opening.end(), SyntaxKind::PunctuationToken);
        cursor.finish();
    }
    cursor.start(SyntaxKind::RichTextArgumentContent, SyntaxRole::Content);
    cursor.emit_to(value.content_range().end());
    cursor.finish();
    if let Some(closing) = value.closing_quote_range() {
        cursor.start(SyntaxKind::RichTextArgumentQuote, SyntaxRole::ClosingQuote);
        cursor.emit_to_as(closing.end(), SyntaxKind::PunctuationToken);
        cursor.finish();
    }
    cursor.finish_at(value.token_range().end());
    cursor.finish();
    cursor.finish();
}

struct PartitionedEventCursor<'parser, 'source, 'events> {
    parser: &'parser mut ShadowDocumentParser<'source, 'events>,
    offset: usize,
}

impl<'parser, 'source, 'events> PartitionedEventCursor<'parser, 'source, 'events> {
    fn new(parser: &'parser mut ShadowDocumentParser<'source, 'events>, offset: usize) -> Self {
        assert_eq!(
            parser.current().map(|token| token.range().start()),
            Some(offset),
            "partitioned RichText range begins at the current lexer boundary"
        );
        Self { parser, offset }
    }

    fn start(&mut self, kind: SyntaxKind, role: SyntaxRole) {
        self.parser.start(kind, role);
    }

    fn finish(&mut self) {
        self.parser.finish();
    }

    fn emit_to(&mut self, end: usize) {
        self.emit_to_with_kind(end, None);
    }

    fn emit_to_as(&mut self, end: usize, split_kind: SyntaxKind) {
        self.emit_to_with_kind(end, Some(split_kind));
    }

    fn emit_to_with_kind(&mut self, end: usize, split_kind: Option<SyntaxKind>) {
        assert!(self.offset <= end, "RichText ranges remain ordered");
        while self.offset < end {
            let token = self
                .parser
                .current()
                .expect("RichText range stays inside the lexed dialogue payload");
            assert!(
                token.range().start() <= self.offset && self.offset < token.range().end(),
                "partition cursor remains inside the current lexer token"
            );
            let segment_end = end.min(token.range().end());
            let whole = self.offset == token.range().start() && segment_end == token.range().end();
            let kind = if whole {
                token.kind()
            } else {
                split_kind.unwrap_or(SyntaxKind::TextToken)
            };
            self.parser.push(SyntaxEvent::token(
                kind,
                SourceRange::new(self.offset, segment_end),
            ));
            self.offset = segment_end;
            if self.offset == token.range().end() {
                let consumed = self
                    .parser
                    .take_for_partition()
                    .expect("partitioned token remains current");
                assert_eq!(consumed, token);
            }
        }
    }

    fn finish_at(&self, expected: usize) {
        assert_eq!(
            self.offset, expected,
            "RichText node retains its exact range"
        );
    }
}

fn emit_range_node(
    parser: &mut ShadowDocumentParser<'_, '_>,
    kind: SyntaxKind,
    role: SyntaxRole,
    range: TextRange,
) {
    parser.start(kind, role);
    bump_until_offset(parser, range.end());
    parser.finish();
}

fn bump_to_range_start(parser: &mut ShadowDocumentParser<'_, '_>, range: TextRange) {
    bump_until_offset(parser, range.start());
}

fn bump_until_offset(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    while parser.current_offset() < end {
        let token = parser
            .current()
            .expect("RichText range stays inside the dialogue payload");
        assert!(
            token.range().end() <= end,
            "non-value RichText range ends at a lexer boundary"
        );
        let _ = parser.bump();
    }
    assert_eq!(parser.current_offset(), end);
}

fn issue_range(issue: &DialogueTagArgSyntaxIssue) -> TextRange {
    match issue {
        DialogueTagArgSyntaxIssue::EmptyKey { range }
        | DialogueTagArgSyntaxIssue::InvalidKey { range }
        | DialogueTagArgSyntaxIssue::InvalidEscape { range }
        | DialogueTagArgSyntaxIssue::UnterminatedQuote { range }
        | DialogueTagArgSyntaxIssue::KeyTooLong { range }
        | DialogueTagArgSyntaxIssue::ValueTooLong { range } => *range,
    }
}

fn emit_unterminated_quote_diagnostic(
    parser: &mut ShadowDocumentParser<'_, '_>,
    range: Option<TextRange>,
) {
    let Some(range) = range else {
        return;
    };
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        DialogueTextDiagnosticCode::RichTextAttributeUnterminatedQuote.as_str(),
        SourceRange::new(range.start(), range.end()),
        "unterminated quote in dialogue tag arguments",
    )));
}

fn split_tag_head(source: &str) -> (&str, &str) {
    source
        .char_indices()
        .find_map(|(index, character)| is_rich_text_whitespace(character).then_some(index))
        .map_or((source, &source[source.len()..]), |index| {
            (
                &source[..index],
                trim_rich_text_whitespace(&source[index..]),
            )
        })
}

fn subslice_offset(source: &str, subslice: &str) -> usize {
    let source_start = source.as_ptr() as usize;
    let source_end = source_start
        .checked_add(source.len())
        .expect("source address range does not overflow");
    let subslice_start = subslice.as_ptr() as usize;
    let subslice_end = subslice_start
        .checked_add(subslice.len())
        .expect("subslice address range does not overflow");
    assert!(
        source_start <= subslice_start && subslice_end <= source_end,
        "RichText range source must be an authored subslice"
    );
    subslice_start - source_start
}
