//! Private test and bench plan grammar over the shared document cursor.

use arcweft_source::SourceRange;

use super::declaration::emit_outer_prefixes;
use super::document::ShadowDocumentParser;
use super::expression::{emit_expression, emit_named_plan_block};
use super::lexer::LexToken;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter, expected,
    find_matching_close, find_statement_terminator, find_top_level_boundary, first_significant,
    token_count, trimmed_end,
};
use crate::grammar::budget::GrammarBudget;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_declaration(
    source: &str,
    tokens: &[LexToken],
    kind: SyntaxKind,
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) {
    debug_assert!(matches!(kind, SyntaxKind::TestItem | SyntaxKind::BenchItem));
    let keyword = if kind == SyntaxKind::TestItem {
        "test"
    } else {
        "bench"
    };
    let mut parser = ShadowDocumentParser::new(source, tokens, events, budget);
    parser.start(kind, role);
    emit_outer_prefixes(&mut parser);
    parser.bump_trivia();

    if parser.at(keyword) {
        parser.bump();
    }
    parser.bump_trivia();
    emit_plan_id(&mut parser, keyword);
    parser.bump_trivia();
    if kind == SyntaxKind::TestItem {
        emit_test_kind(&mut parser);
        parser.bump_trivia();
    }
    recover_trailing_header(&mut parser);
    emit_plan_body(&mut parser, keyword);

    while parser.bump().is_some() {}
    parser.finish();
}

fn emit_plan_id(parser: &mut ShadowDocumentParser<'_, '_>, keyword: &'static str) {
    if parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) {
        parser.bump();
        return;
    }

    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::EntityReferenceToken),
        at,
    });
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        if keyword == "test" {
            "syntax.test.missing_id"
        } else {
            "syntax.bench.missing_id"
        },
        SourceRange::new(at, at),
        "plan declaration requires an entity ID",
    )));
}

fn emit_test_kind(parser: &mut ShadowDocumentParser<'_, '_>) {
    if matches!(
        parser.current_kind(),
        Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
    ) {
        parser.start(SyntaxKind::NameReference, SyntaxRole::Type);
        parser.bump();
        parser.finish();
        return;
    }

    let at = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.test.missing_kind",
        SourceRange::new(at, at),
        "test declaration requires an adapter kind",
    )));
}

fn recover_trailing_header(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.at("{") {
        return;
    }
    let open = find_top_level_boundary(parser, parser.cursor(), &["{"]);
    if open == token_count(parser) {
        return;
    }

    let start = parser.cursor();
    let recovery_end = trimmed_end(parser, start, open);
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(1));
    bump_until(parser, recovery_end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.item.unexpected_token",
        token_range(parser, start, recovery_end),
        "unexpected plan declaration header tokens",
    )));
    bump_until(parser, open);
}

fn emit_plan_body(parser: &mut ShadowDocumentParser<'_, '_>, keyword: &'static str) {
    if !parser.at("{") {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::PunctuationToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            missing_body_code(keyword),
            SourceRange::new(at, at),
            "plan declaration requires a braced body",
        )));
        return;
    }

    parser.start(SyntaxKind::Block, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let end = token_count(parser);
    let close = find_matching_close(parser, parser.cursor(), "{").unwrap_or(end);
    parser.start(SyntaxKind::StatementList, SyntaxRole::Element(0));
    emit_plan_statements(parser, close);
    bump_until(parser, close);
    parser.finish();

    if parser.at("}") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            missing_body_code(keyword),
        );
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            missing_body_code(keyword),
            SourceRange::new(at, at),
            "plan body requires a closing `}`",
        )));
    }
    parser.finish();
}

fn emit_plan_statements(parser: &mut ShadowDocumentParser<'_, '_>, close: usize) {
    let mut ordinal = 0_u32;
    while parser.cursor() < close {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }

        let start = parser.cursor();
        let terminator = find_statement_terminator(parser, start, close);
        let boundary = terminator.map_or(close, |(index, _)| index);
        let significant_end = trimmed_end(parser, start, boundary);
        if first_significant(parser, start, significant_end).is_none() {
            bump_until(parser, boundary.saturating_add(1).min(close));
            continue;
        }
        let statement_end = if terminator.is_some_and(|(_, semicolon)| semicolon) {
            boundary + 1
        } else {
            significant_end
        };
        emit_plan_statement(parser, statement_end, ordinal);
        bump_until(parser, boundary);
        ordinal = ordinal.saturating_add(1);
    }
}

fn emit_plan_statement(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u32) {
    let child_end = if end > parser.cursor()
        && parser
            .token_at(end - 1)
            .is_some_and(|token| parser.text_of(token) == ";")
    {
        end - 1
    } else {
        end
    };
    if matches!(parser.current_text(), Some("setup" | "measure" | "report"))
        && find_top_level_boundary(parser, parser.cursor(), &["{"]) < child_end
    {
        parser.start(
            SyntaxKind::ExpressionStatement,
            SyntaxRole::Statement(ordinal),
        );
        emit_named_plan_block(parser, child_end, SyntaxRole::Operand);
    } else if parser.at("goto") {
        parser.start(SyntaxKind::GotoStatement, SyntaxRole::Statement(ordinal));
        parser.bump();
        parser.bump_trivia();
        emit_expression(parser, child_end, SyntaxRole::Operand);
    } else {
        parser.start(
            SyntaxKind::ExpressionStatement,
            SyntaxRole::Statement(ordinal),
        );
        emit_expression(parser, child_end, SyntaxRole::Operand);
    }
    bump_until(parser, end);
    parser.finish();
}

fn missing_body_code(keyword: &str) -> &'static str {
    match keyword {
        "test" => "syntax.test.missing_body",
        "bench" => "syntax.bench.missing_body",
        _ => unreachable!("plan grammar is only used by test and bench"),
    }
}

fn token_range(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> SourceRange {
    let start_offset = parser
        .token_at(start)
        .map_or_else(|| parser.current_offset(), |token| token.range().start());
    let end_offset = (start..end)
        .rev()
        .find_map(|index| parser.token_at(index).map(|token| token.range().end()))
        .unwrap_or(start_offset);
    SourceRange::new(start_offset, end_offset)
}
