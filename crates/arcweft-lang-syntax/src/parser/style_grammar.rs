//! Private native Style grammar over the shared lossless document cursor.

use arcweft_source::SourceRange;

use super::cursor::ShadowDocumentParser;
use super::declaration::{emit_outer_prefixes, emit_visibility};
use super::expression::{emit_entity_reference, emit_expression};
use super::lexer::LexToken;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter, expected,
    find_matching_close, find_top_level_boundary, first_significant, token_count, token_text,
    trimmed_end,
};
use super::type_ref::emit_type;
use crate::grammar::budget::GrammarBudget;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::grammar::style_projection::{
    PendingStyleBodyProjection, PendingStyleDeclarationProjection, PendingStyleEnvironmentClause,
    PendingStyleEnvironmentComparison, PendingStyleEnvironmentCondition,
    PendingStyleEnvironmentConditionRecovery, PendingStyleEnvironmentField,
    PendingStyleEnvironmentProjection, PendingStyleId, PendingStyleMemberProjection,
    PendingStyleName, PendingStylePredicate, PendingStylePropertyProjection,
    PendingStylePunctuation, PendingStyleRuleProjection, PendingStyleSelectorPart,
    PendingStyleSelectorProjection, PendingStyleSelectorRelation, PendingStyleSelectorSequence,
    PendingStyleTokenProjection, PendingStyleTypeAnnotation, StyleEnvironmentComparison,
    StyleEnvironmentConditionIssue, StyleEnvironmentField, StyleIdForm, StylePropertyOperation,
    StyleSelectorRelation, StyleSyntaxName,
};
use crate::id_ref::{
    AuthoredIdRef, AuthoredIdRoot, AuthoredIdSegment, SyntaxIdRefIssue, SyntaxIdRefShape,
    SyntaxIdRefSyntax,
};
use crate::name::SyntaxName;

mod environment;
mod selector;

use self::environment::emit_environment_block;
use self::selector::emit_rule;

pub(super) fn emit_declaration(
    source: &str,
    tokens: &[LexToken],
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) {
    let mut parser = ShadowDocumentParser::new(source, tokens, events, budget);
    let owner = parser.start_projected_owner(SyntaxKind::StyleItem, role);
    emit_outer_prefixes(&mut parser);
    parser.bump_trivia();
    emit_visibility(&mut parser);
    parser.bump_trivia();
    if parser.at("style") {
        parser.bump();
    }
    parser.bump_trivia();
    let id = emit_style_id(&mut parser);
    parser.bump_trivia();
    let trailing_header_recovery = recover_header_tail(&mut parser);
    let body = emit_style_body(&mut parser);
    parser.set_style_projection(
        owner,
        PendingStyleDeclarationProjection {
            id,
            trailing_header_recovery,
            body,
        },
    );
    while parser.bump().is_some() {}
    parser.finish();
}

fn emit_style_id(parser: &mut ShadowDocumentParser<'_, '_>) -> PendingStyleId {
    if parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) {
        let source = parser.current().expect("Style ID token").range();
        let (_, authored) = emit_entity_reference(parser, SyntaxRole::Reference(0));
        let (value, canonical_style_family) = authored.normalized_for_family(
            &SyntaxName::try_new("style").expect("fixed Style family is an identifier"),
        );
        if !canonical_style_family {
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.style.id_family",
                source,
                "style declaration IDs must resolve through the `style` family",
            )));
        }
        return PendingStyleId::Authored {
            value,
            source,
            form: StyleIdForm::Explicit,
            canonical_style_family,
        };
    }

    if matches!(
        parser.current_kind(),
        Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
    ) {
        return emit_bare_style_id(parser);
    }

    if parser.at("{") || parser.is_at_end() {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingName, SyntaxRole::Reference(0));
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::IdentifierToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.missing_name",
            SourceRange::new(at, at),
            "style declaration requires a name or canonical style ID",
        )));
        return PendingStyleId::Missing {
            value: recovered_style_id(SyntaxIdRefIssue::MissingSuffix, 0),
            insertion: SourceRange::new(at, at),
        };
    }

    let start = parser.current_offset();
    let end = trimmed_end(
        parser,
        parser.cursor(),
        find_top_level_boundary(parser, parser.cursor(), &["{"]),
    );
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Reference(0));
    bump_until(parser, end);
    parser.finish();
    let source = SourceRange::new(start, parser.current_offset());
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.style.invalid_name",
        source,
        "style declaration name must be a dotted identifier path",
    )));
    PendingStyleId::Invalid {
        value: recovered_style_id(SyntaxIdRefIssue::InvalidSegment { ordinal: 0 }, 0),
        source,
        authored_name: false,
    }
}

fn emit_bare_style_id(parser: &mut ShadowDocumentParser<'_, '_>) -> PendingStyleId {
    let start = parser.current_offset();
    parser.start(SyntaxKind::NameDefinition, SyntaxRole::Reference(0));
    let mut segments = Vec::new();
    let mut valid = true;
    while let Some(token) = parser.current().filter(|token| {
        matches!(
            token.kind(),
            SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
        )
    }) {
        segments.push(
            AuthoredIdSegment::try_new(parser.text_of(token))
                .expect("identifier tokens are non-empty ID segments"),
        );
        parser.bump();
        if !parser.at(".") {
            break;
        }
        parser.bump();
        if !matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        ) {
            valid = false;
            break;
        }
    }
    parser.finish();
    let source = SourceRange::new(start, parser.current_offset());
    if valid {
        let segment_count = u32::try_from(segments.len()).unwrap_or(u32::MAX);
        return PendingStyleId::Authored {
            value: SyntaxIdRefSyntax::new(
                Ok(AuthoredIdRef::new(style_family_root(0), segments)),
                SyntaxIdRefShape::new(false, false, 0, segment_count),
            ),
            source,
            form: StyleIdForm::Bare,
            canonical_style_family: true,
        };
    }
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.style.invalid_name",
        source,
        "style declaration name must be a dotted identifier path",
    )));
    PendingStyleId::Invalid {
        value: recovered_style_id(
            SyntaxIdRefIssue::InvalidSegment {
                ordinal: u32::try_from(segments.len()).unwrap_or(u32::MAX),
            },
            u32::try_from(segments.len().saturating_add(1)).unwrap_or(u32::MAX),
        ),
        source,
        authored_name: true,
    }
}

fn recovered_style_id(issue: SyntaxIdRefIssue, segment_count: u32) -> SyntaxIdRefSyntax {
    SyntaxIdRefSyntax::new(
        Err(issue),
        SyntaxIdRefShape::new(false, false, 0, segment_count),
    )
}

fn style_family_root(parent_depth: usize) -> AuthoredIdRoot {
    AuthoredIdRoot::FamilyRelative {
        family: SyntaxName::try_new("style").expect("fixed Style family is an identifier"),
        parent_depth,
    }
}

fn recover_header_tail(parser: &mut ShadowDocumentParser<'_, '_>) -> bool {
    if parser.at("{") || parser.is_at_end() {
        return false;
    }
    let end = trimmed_end(
        parser,
        parser.cursor(),
        find_top_level_boundary(parser, parser.cursor(), &["{"]),
    );
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_until(parser, end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.style.trailing_head",
        SourceRange::new(start, parser.current_offset()),
        "unexpected text after the Style declaration ID",
    )));
    parser.bump_trivia();
    true
}

fn emit_style_body(parser: &mut ShadowDocumentParser<'_, '_>) -> PendingStyleBodyProjection {
    if !parser.at("{") {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::PunctuationToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.missing_body",
            SourceRange::new(at, at),
            "style declaration requires a braced body",
        )));
        return PendingStyleBodyProjection::Missing;
    }

    parser.start(SyntaxKind::StyleBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let end = token_count(parser);
    let matched_close = find_matching_close(parser, parser.cursor(), "{");
    let close = matched_close.unwrap_or(end);
    parser.start(SyntaxKind::ItemList, SyntaxRole::Element(0));
    let members = emit_style_members(parser, close, true);
    bump_until(parser, close);
    parser.finish();
    let closed = parser.at("}");
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.style.missing_body_close",
    );
    parser.finish();
    PendingStyleBodyProjection::Braced {
        members: members.into_boxed_slice(),
        closed,
    }
}

fn emit_style_members(
    parser: &mut ShadowDocumentParser<'_, '_>,
    close: usize,
    allow_tokens: bool,
) -> Vec<PendingStyleMemberProjection> {
    let mut members = Vec::new();
    while parser.cursor() < close {
        bump_member_separators(parser, close);
        if parser.cursor() >= close {
            break;
        }
        let source_ordinal = u32::try_from(members.len()).unwrap_or(u32::MAX);
        let start = parser.cursor();
        let member = if parser.at("token") {
            PendingStyleMemberProjection::Token(emit_token_declaration(
                parser,
                member_boundary(parser, start, close),
                source_ordinal,
                allow_tokens,
            ))
        } else if parser.at("when")
            && next_significant_text(parser, start + 1, close) == Some("environment")
        {
            PendingStyleMemberProjection::Environment(emit_environment_block(
                parser,
                close,
                source_ordinal,
            ))
        } else if find_top_level_boundary(parser, start, &["{"]) < close {
            PendingStyleMemberProjection::Rule(emit_rule(parser, close, source_ordinal))
        } else {
            let end = member_boundary(parser, start, close);
            emit_invalid_member(parser, end, source_ordinal);
            PendingStyleMemberProjection::Recovery { source_ordinal }
        };
        if parser.cursor() == start {
            parser.bump();
        }
        members.push(member);
    }
    members
}

fn emit_token_declaration(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    source_ordinal: u32,
    allowed_at_this_depth: bool,
) -> PendingStyleTokenProjection {
    let start = parser.cursor();
    parser.start(
        SyntaxKind::StyleTokenDeclaration,
        SyntaxRole::Element(source_ordinal),
    );
    parser.bump();
    bump_trivia_before(parser, end);
    let name = emit_style_name(
        parser,
        end,
        SyntaxKind::NameDefinition,
        SyntaxRole::Name,
        true,
        "syntax.style.member_name",
        "style token requires a name",
    );
    let id = name.token_id();
    bump_trivia_before(parser, end);

    let type_annotation = if parser.at(":") {
        let colon = parser.current().expect("Style type colon").range();
        parser.start(SyntaxKind::ColonNode, SyntaxRole::Colon);
        parser.bump();
        parser.finish();
        bump_trivia_before(parser, end);
        let type_end =
            find_top_level_boundary(parser, parser.cursor(), &["=", "+=", "-="]).min(end);
        if parser.cursor() < type_end {
            emit_type(parser, type_end, SyntaxRole::Type);
            bump_until(parser, type_end);
        } else {
            emit_missing_type(parser);
        }
        PendingStyleTypeAnnotation::Present { colon }
    } else {
        PendingStyleTypeAnnotation::Absent
    };
    bump_trivia_before(parser, end);
    let assignment = emit_assignment(parser, "syntax.style.token_initializer");
    bump_trivia_before(parser, end);
    emit_expression(parser, end, SyntaxRole::Initializer);
    bump_until(parser, end);
    if !allowed_at_this_depth {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.environment_token",
            token_range(parser, start, end),
            "style tokens are only allowed at the sheet level",
        )));
    }
    parser.finish();
    PendingStyleTokenProjection {
        source_ordinal,
        name,
        id,
        type_annotation,
        assignment,
        allowed_at_this_depth,
    }
}

fn emit_style_name(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    kind: SyntaxKind,
    role: SyntaxRole,
    allow_dot: bool,
    code: &'static str,
    message: &'static str,
) -> PendingStyleName {
    if parser.cursor() >= end
        || !matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        )
    {
        return emit_missing_name(parser, role, code, message);
    }

    let start = parser.current_offset();
    let mut spelling = String::new();
    let mut dotted_component_count = 1_usize;
    parser.start(kind, role);
    while let Some(token) = parser.current().filter(|token| {
        matches!(
            token.kind(),
            SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken
        )
    }) {
        spelling.push_str(parser.text_of(token));
        parser.bump();

        if parser.cursor() >= end {
            break;
        }
        let separator = match parser.current_text() {
            Some("-") => Some('-'),
            Some(".") if allow_dot => Some('.'),
            _ => None,
        };
        let Some(separator) = separator else {
            break;
        };
        if separator == '.' {
            dotted_component_count = dotted_component_count.saturating_add(1);
        }
        spelling.push(separator);
        parser.bump();
        if !matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        ) {
            break;
        }
    }
    parser.finish();
    let source = SourceRange::new(start, parser.current_offset());
    let value = StyleSyntaxName::try_new(&spelling);
    if value.is_err() {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            code, source, message,
        )));
    }
    PendingStyleName::Authored {
        value,
        dotted_component_count: u32::try_from(dotted_component_count).unwrap_or(u32::MAX),
        source,
    }
}

fn emit_assignment(
    parser: &mut ShadowDocumentParser<'_, '_>,
    diagnostic: &'static str,
) -> PendingStylePunctuation {
    if parser.at("=") {
        let source = parser.current().expect("Style assignment").range();
        parser.start(SyntaxKind::EqualsNode, SyntaxRole::Equals);
        parser.bump();
        parser.finish();
        return PendingStylePunctuation::Authored(source);
    }

    let at = parser.current_offset();
    parser.start(SyntaxKind::EqualsNode, SyntaxRole::Equals);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::PunctuationToken),
        at,
    });
    parser.finish();
    if matches!(parser.current_text(), Some("+=" | "-=")) {
        let source = parser
            .current()
            .expect("unsupported Style assignment")
            .range();
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        parser.bump();
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            diagnostic,
            source,
            "native Style assignment uses `=` or the `append` keyword",
        )));
        return PendingStylePunctuation::Unsupported(source);
    }
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        diagnostic,
        SourceRange::new(at, at),
        "style member requires `=`",
    )));
    PendingStylePunctuation::Missing(SourceRange::new(at, at))
}

fn emit_invalid_member(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u32) {
    let start = parser.cursor();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Element(ordinal));
    bump_until(parser, end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.style.invalid_member",
        token_range(parser, start, end),
        "style body accepts tokens, selector rules, and environment blocks",
    )));
}

fn emit_missing_type(parser: &mut ShadowDocumentParser<'_, '_>) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingType, SyntaxRole::Type);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.style.token_type",
        SourceRange::new(at, at),
        "style token type is missing after `:`",
    )));
}

fn emit_missing_name(
    parser: &mut ShadowDocumentParser<'_, '_>,
    role: SyntaxRole,
    code: &'static str,
    message: &'static str,
) -> PendingStyleName {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingName, role);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code,
        SourceRange::new(at, at),
        message,
    )));
    PendingStyleName::Missing {
        insertion: SourceRange::new(at, at),
    }
}

fn bump_member_separators(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    while parser.cursor() < end
        && parser
            .current()
            .is_some_and(|token| is_trivia(token.kind()) || parser.text_of(token) == ";")
    {
        parser.bump();
    }
}

fn bump_trivia_before(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    while parser.cursor() < end && parser.current_kind().is_some_and(is_trivia) {
        parser.bump();
    }
}

fn bump_selector_trivia(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
) -> Option<SourceRange> {
    let start = parser
        .current()
        .filter(|token| parser.cursor() < end && is_trivia(token.kind()))
        .map(LexToken::range)?;
    let mut finish = start.end();
    while parser.cursor() < end && parser.current_kind().is_some_and(is_trivia) {
        finish = parser.current().expect("selector trivia").range().end();
        parser.bump();
    }
    Some(SourceRange::new(start.start(), finish))
}

fn member_boundary(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> usize {
    let mut depth = 0_usize;
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            return end;
        };
        let text = parser.text_of(token);
        if depth == 0 && (text == ";" || token.kind() == SyntaxKind::NewlineToken) {
            return index;
        }
        match text {
            "(" | "[" | "{" | "<" => depth += 1,
            ")" | "]" | "}" | ">" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    end
}

fn environment_clause_end(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> usize {
    let mut delimiters = Vec::new();
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            return end;
        };
        let text = parser.text_of(token);
        if delimiters.is_empty() && text == "," {
            return index;
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
    }
    end
}

fn selector_sequence_end(parser: &ShadowDocumentParser<'_, '_>, end: usize) -> usize {
    let mut index = parser.cursor();
    while index < end {
        let Some(token) = parser.token_at(index) else {
            break;
        };
        if is_trivia(token.kind()) || parser.text_of(token) == ">" {
            break;
        }
        index += 1;
    }
    index.max(parser.cursor().saturating_add(1)).min(end)
}

fn next_nontrivia(
    parser: &ShadowDocumentParser<'_, '_>,
    mut index: usize,
    end: usize,
) -> Option<usize> {
    while index < end {
        let token = parser.token_at(index)?;
        if !is_trivia(token.kind()) {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn next_significant_text<'a>(
    parser: &'a ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<&'a str> {
    first_significant(parser, start, end).and_then(|index| token_text(parser, index))
}

fn pending_name_range(name: &PendingStyleName) -> SourceRange {
    match name {
        PendingStyleName::Authored { source, .. } => *source,
        PendingStyleName::Missing { insertion } => *insertion,
    }
}

const fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::WhitespaceToken
            | SyntaxKind::NewlineToken
            | SyntaxKind::CommentToken
            | SyntaxKind::DocCommentToken
    )
}

fn token_range(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> SourceRange {
    let start = first_significant(parser, start, end).unwrap_or(start);
    let end = trimmed_end(parser, start, end);
    let range_start = parser
        .token_at(start)
        .map_or_else(|| parser.current_offset(), |token| token.range().start());
    let range_end = end
        .checked_sub(1)
        .and_then(|index| parser.token_at(index))
        .map_or(range_start, |token| token.range().end());
    SourceRange::new(range_start, range_end)
}
