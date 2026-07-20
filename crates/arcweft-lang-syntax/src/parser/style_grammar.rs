//! Private native Style grammar over the shared lossless document cursor.
//!
//! This deliberately does not reuse the public Style AST parser.  Stage 1
//! needs a typed, lossless event tree that can be validated independently
//! before the atomic syntax switch chooses its public representation.

use arcweft_source::SourceRange;

use super::declaration::{emit_outer_prefixes, emit_visibility};
use super::document::ShadowDocumentParser;
use super::expression::emit_expression;
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

pub(super) fn emit_declaration(
    source: &str,
    tokens: &[LexToken],
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) {
    let mut parser = ShadowDocumentParser::new(source, tokens, events, budget);
    parser.start(SyntaxKind::StyleItem, role);
    emit_outer_prefixes(&mut parser);
    parser.bump_trivia();
    emit_visibility(&mut parser);
    parser.bump_trivia();
    if parser.at("style") {
        parser.bump();
    }
    parser.bump_trivia();
    emit_style_name(&mut parser);
    parser.bump_trivia();
    emit_style_body(&mut parser);
    while parser.bump().is_some() {}
    parser.finish();
}

fn emit_style_name(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
        return;
    }

    let end = find_top_level_boundary(parser, parser.cursor(), &["{"]);
    let end = trimmed_end(parser, parser.cursor(), end);
    if parser.cursor() >= end {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
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
        return;
    }

    parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
    let mut saw_name = false;
    while parser.cursor() < end {
        if parser.current_kind().is_some_and(|kind| {
            matches!(kind, SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        }) {
            saw_name = true;
        }
        parser.bump();
    }
    parser.finish();
    if !saw_name {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.invalid_name",
            token_range(parser, 0, end),
            "style declaration name must be a dotted identifier path",
        )));
    }
}

fn emit_style_body(parser: &mut ShadowDocumentParser<'_, '_>) {
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
        return;
    }

    parser.start(SyntaxKind::StyleBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let end = token_count(parser);
    let close = find_matching_close(parser, parser.cursor(), "{").unwrap_or(end);
    parser.start(SyntaxKind::ItemList, SyntaxRole::Element(0));
    emit_style_members(parser, close, true);
    bump_until(parser, close);
    parser.finish();
    if parser.at("}") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.style.missing_body_close",
        );
    } else {
        let at = parser.current_offset();
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.missing_body_close",
            SourceRange::new(at, at),
            "style declaration body requires a closing `}`",
        )));
    }
    parser.finish();
}

fn emit_style_members(parser: &mut ShadowDocumentParser<'_, '_>, close: usize, allow_tokens: bool) {
    let mut ordinal = 0_u32;
    while parser.cursor() < close {
        bump_member_separators(parser, close);
        if parser.cursor() >= close {
            break;
        }
        let start = parser.cursor();
        if parser.at("token") {
            emit_token_declaration(
                parser,
                member_boundary(parser, start, close),
                ordinal,
                allow_tokens,
            );
        } else if parser.at("when")
            && next_significant_text(parser, start + 1, close) == Some("environment")
        {
            emit_environment_block(parser, close, ordinal);
        } else if find_top_level_boundary(parser, start, &["{"]) < close {
            emit_rule(parser, close, ordinal);
        } else {
            let end = member_boundary(parser, start, close);
            emit_invalid_member(parser, end, ordinal);
        }
        if parser.cursor() == start {
            parser.bump();
        }
        ordinal = ordinal.saturating_add(1);
    }
}

fn emit_token_declaration(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    ordinal: u32,
    allow_tokens: bool,
) {
    let start = parser.cursor();
    parser.start(
        SyntaxKind::StyleTokenDeclaration,
        SyntaxRole::Element(ordinal),
    );
    parser.bump();
    bump_trivia_before(parser, end);
    emit_member_name(parser, end);
    bump_trivia_before(parser, end);

    if parser.at(":") {
        parser.bump();
        bump_trivia_before(parser, end);
        let type_end = find_top_level_boundary(parser, parser.cursor(), &["="]).min(end);
        if parser.cursor() < type_end {
            emit_type(parser, type_end, SyntaxRole::Type);
            bump_until(parser, type_end);
        } else {
            emit_missing_type(parser);
        }
    }
    bump_trivia_before(parser, end);
    emit_assignment_and_expression(
        parser,
        end,
        SyntaxRole::Initializer,
        "syntax.style.token_initializer",
    );
    bump_until(parser, end);
    if !allow_tokens {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.environment_token",
            token_range(parser, start, end),
            "style tokens are only allowed at the sheet level",
        )));
    }
    parser.finish();
}

fn emit_rule(parser: &mut ShadowDocumentParser<'_, '_>, close: usize, ordinal: u32) {
    let start = parser.cursor();
    let open = find_top_level_boundary(parser, start, &["{"]).min(close);
    parser.start(SyntaxKind::StyleRule, SyntaxRole::Element(ordinal));
    emit_selector(parser, open);
    bump_until(parser, open);
    if !parser.at("{") {
        emit_invalid_member(parser, member_boundary(parser, start, close), ordinal);
        parser.finish();
        return;
    }
    emit_style_rule_body(parser, close);
    parser.finish();
}

fn emit_selector(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    parser.start(SyntaxKind::StyleSelector, SyntaxRole::Target);
    let mut ordinal = 0_u32;
    while parser.cursor() < end {
        if parser.current_kind().is_some_and(is_trivia) {
            parser.bump();
            continue;
        }
        let sequence_end = selector_sequence_end(parser, end);
        parser.start(
            SyntaxKind::StyleSelectorSequence,
            SyntaxRole::Element(ordinal),
        );
        bump_until(parser, sequence_end);
        parser.finish();
        ordinal = ordinal.saturating_add(1);
    }
    if ordinal == 0 {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingName, SyntaxRole::Target);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::IdentifierToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.missing_selector",
            SourceRange::new(at, at),
            "style rule requires a selector before `{`",
        )));
    }
    parser.finish();
}

fn emit_style_rule_body(parser: &mut ShadowDocumentParser<'_, '_>, enclosing_close: usize) {
    parser.start(SyntaxKind::StyleBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let end = find_matching_close(parser, parser.cursor(), "{")
        .unwrap_or(enclosing_close)
        .min(enclosing_close);
    parser.start(SyntaxKind::FieldList, SyntaxRole::Element(0));
    let mut ordinal = 0_u32;
    while parser.cursor() < end {
        bump_member_separators(parser, end);
        if parser.cursor() >= end {
            break;
        }
        let member_end = member_boundary(parser, parser.cursor(), end);
        emit_property_declaration(parser, member_end, ordinal);
        bump_until(parser, member_end);
        ordinal = ordinal.saturating_add(1);
    }
    parser.finish();
    bump_until(parser, end);
    if parser.at("}") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.style.missing_rule_close",
        );
    } else {
        let at = parser.current_offset();
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.missing_rule_close",
            SourceRange::new(at, at),
            "style rule requires a closing `}`",
        )));
    }
    parser.finish();
}

fn emit_property_declaration(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u32) {
    let start = parser.cursor();
    parser.start(
        SyntaxKind::StylePropertyDeclaration,
        SyntaxRole::Element(ordinal),
    );
    emit_member_name(parser, end);
    bump_trivia_before(parser, end);
    emit_assignment_and_expression(
        parser,
        end,
        SyntaxRole::Initializer,
        "syntax.style.property_initializer",
    );
    bump_until(parser, end);
    if start == parser.cursor() {
        parser.bump();
    }
    parser.finish();
}

fn emit_environment_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    enclosing_close: usize,
    ordinal: u32,
) {
    parser.start(
        SyntaxKind::StyleEnvironmentBlock,
        SyntaxRole::Element(ordinal),
    );
    parser.bump();
    bump_trivia_before(parser, enclosing_close);
    if parser.at("environment") {
        parser.start(SyntaxKind::NameReference, SyntaxRole::Target);
        parser.bump();
        parser.finish();
    } else {
        emit_missing_name(
            parser,
            SyntaxRole::Target,
            "syntax.style.environment_name",
            "style environment block requires `environment`",
        );
    }
    bump_trivia_before(parser, enclosing_close);
    emit_environment_condition(parser, enclosing_close);
    bump_trivia_before(parser, enclosing_close);
    if parser.at("{") {
        emit_environment_body(parser, enclosing_close);
    } else {
        let at = parser.current_offset();
        emit_missing_delimiter(parser, SyntaxKind::OpenBraceNode, SyntaxRole::OpenDelimiter);
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.environment_body",
            SourceRange::new(at, at),
            "style environment block requires a braced body",
        )));
    }
    parser.finish();
}

fn emit_environment_condition(parser: &mut ShadowDocumentParser<'_, '_>, enclosing_close: usize) {
    parser.start(SyntaxKind::StyleEnvironmentCondition, SyntaxRole::Condition);
    if !parser.at("(") {
        let at = parser.current_offset();
        emit_missing_delimiter(parser, SyntaxKind::OpenParenNode, SyntaxRole::OpenDelimiter);
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.environment_condition",
            SourceRange::new(at, at),
            "style environment condition requires `(`",
        )));
        parser.finish();
        return;
    }
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    let end = find_matching_close(parser, parser.cursor(), "(")
        .unwrap_or_else(|| {
            find_top_level_boundary(parser, parser.cursor(), &["{"]).min(enclosing_close)
        })
        .min(enclosing_close);
    parser.start(SyntaxKind::FieldList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    while parser.cursor() < end {
        bump_member_separators(parser, end);
        if parser.cursor() >= end {
            break;
        }
        let clause_end = find_top_level_boundary(parser, parser.cursor(), &[","]).min(end);
        parser.start(
            SyntaxKind::StyleEnvironmentClause,
            SyntaxRole::Field(ordinal),
        );
        emit_expression(parser, clause_end, SyntaxRole::Condition);
        bump_until(parser, clause_end);
        parser.finish();
        if parser.at(",") {
            parser.bump();
        }
        ordinal = ordinal.saturating_add(1);
    }
    parser.finish();
    bump_until(parser, end);
    if parser.at(")") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseParenNode,
            ")",
            "syntax.style.environment_condition_close",
        );
    } else {
        let at = parser.current_offset();
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseParenNode,
            SyntaxRole::CloseDelimiter,
        );
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.environment_condition_close",
            SourceRange::new(at, at),
            "style environment condition requires a closing `)`",
        )));
    }
    parser.finish();
}

fn emit_environment_body(parser: &mut ShadowDocumentParser<'_, '_>, enclosing_close: usize) {
    parser.start(SyntaxKind::StyleBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let end = find_matching_close(parser, parser.cursor(), "{")
        .unwrap_or(enclosing_close)
        .min(enclosing_close);
    parser.start(SyntaxKind::ItemList, SyntaxRole::Element(0));
    emit_style_members(parser, end, false);
    bump_until(parser, end);
    parser.finish();
    if parser.at("}") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.style.environment_body_close",
        );
    } else {
        let at = parser.current_offset();
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.environment_body_close",
            SourceRange::new(at, at),
            "style environment body requires a closing `}`",
        )));
    }
    parser.finish();
}

fn emit_member_name(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    if parser.cursor() < end
        && parser.current_kind().is_some_and(|kind| {
            matches!(kind, SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        })
    {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        while parser.at(".") {
            parser.bump();
            if parser.current_kind().is_some_and(|kind| {
                matches!(kind, SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
            }) {
                parser.bump();
            } else {
                break;
            }
        }
        parser.finish();
        return;
    }
    emit_missing_name(
        parser,
        SyntaxRole::Name,
        "syntax.style.member_name",
        "style member requires a name",
    );
}

fn emit_assignment_and_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    diagnostic: &'static str,
) {
    if matches!(parser.current_text(), Some("=" | "+=" | "-=")) {
        parser.bump();
    } else {
        let at = parser.current_offset();
        emit_missing_delimiter(
            parser,
            SyntaxKind::MissingTokenNode,
            SyntaxRole::Recovery(0),
        );
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            diagnostic,
            SourceRange::new(at, at),
            "style member requires an assignment operator",
        )));
        return;
    }
    bump_trivia_before(parser, end);
    if parser.cursor() < end {
        emit_expression(parser, end, role);
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingExpression, role);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::TextToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            diagnostic,
            SourceRange::new(at, at),
            "style member requires an initializer expression",
        )));
    }
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
) {
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
    if index == parser.cursor() {
        index.saturating_add(1).min(end)
    } else {
        index
    }
}

fn next_significant_text<'a>(
    parser: &'a ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<&'a str> {
    first_significant(parser, start, end).and_then(|index| token_text(parser, index))
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
