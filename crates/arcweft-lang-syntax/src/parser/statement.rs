//! Private predicate/proof block and statement grammar over the shared cursor.

use arcweft_source::SourceRange;

use super::document::ShadowDocumentParser;
use super::expression::{emit_expression, expression_is_call};
use super::pattern::emit_pattern;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter,
    find_matching_close, find_statement_terminator, find_top_level_boundary, first_significant,
    token_count, token_text, trimmed_end,
};
use super::type_ref::emit_type;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_block_body(
    parser: &mut ShadowDocumentParser<'_, '_>,
    item_kind: SyntaxKind,
    body_kind: SyntaxKind,
    keyword: &str,
) {
    let block_kind = if item_kind == SyntaxKind::PredicateItem {
        SyntaxKind::PredicateBlock
    } else {
        SyntaxKind::ProofBlock
    };
    parser.start(body_kind, SyntaxRole::Body);
    parser.start(block_kind, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{").unwrap_or(token_count(parser));
    parser.start(SyntaxKind::StatementList, SyntaxRole::Element(0));
    let mut statement = 0_u32;
    let mut has_tail = false;

    while parser.cursor() < close {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }
        let start = parser.cursor();
        let terminator = find_statement_terminator(parser, start, close);
        let segment_end = terminator.map_or(close, |(index, _)| index);
        let significant_end = trimmed_end(parser, start, segment_end);
        let first = first_significant(parser, start, significant_end)
            .and_then(|index| token_text(parser, index));
        let semicolon = terminator.is_some_and(|(_, semicolon)| semicolon);
        let later = terminator
            .is_some_and(|(index, _)| first_significant(parser, index + 1, close).is_some());
        let statement_shaped = matches!(first, Some("let" | "assert"));
        if semicolon || later || statement_shaped {
            let end = if semicolon {
                terminator.map_or(segment_end, |(index, _)| index + 1)
            } else {
                significant_end
            };
            emit_statement(parser, end, item_kind, statement);
            statement = statement.saturating_add(1);
            bump_until(parser, segment_end);
            continue;
        }

        parser.finish();
        emit_expression(parser, significant_end, SyntaxRole::Tail);
        bump_until(parser, close);
        has_tail = true;
        break;
    }

    if !has_tail {
        parser.finish();
        parser.start(SyntaxKind::OmittedBlockTail, SyntaxRole::Tail);
        parser.finish();
    }

    if parser.cursor() == close && parser.at("}") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.block.missing_close",
        );
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            if keyword == "predicate" {
                "syntax.predicate.missing_block_close"
            } else {
                "syntax.proof.missing_block_close"
            },
            SourceRange::new(at, at),
            "missing closing `}` for declaration block",
        )));
    }
    parser.finish();
    parser.finish();
}

fn emit_statement(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    let child_end = if end > parser.cursor() && token_text(parser, end - 1) == Some(";") {
        end - 1
    } else {
        end
    };
    let first = parser.current_text();
    let kind = match first {
        Some("let") => SyntaxKind::LetStatement,
        Some("assert") => SyntaxKind::AssertionStatement,
        _ if item_kind == SyntaxKind::ProofItem
            && expression_is_call(parser, parser.cursor(), child_end) =>
        {
            SyntaxKind::ProofCallStatement
        }
        _ => SyntaxKind::ErrorStatement,
    };
    parser.start(kind, SyntaxRole::Statement(ordinal));
    if kind == SyntaxKind::LetStatement {
        emit_let_children(parser, child_end);
    } else if kind == SyntaxKind::AssertionStatement {
        emit_assertion_children(parser, child_end);
    } else if kind == SyntaxKind::ProofCallStatement {
        emit_expression(parser, child_end, SyntaxRole::Callee);
    } else {
        bump_until(parser, child_end);
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_let_children(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    parser.bump();
    parser.bump_trivia();
    let equals = find_top_level_boundary(parser, parser.cursor(), &["="]).min(end);
    let colon = find_top_level_boundary(parser, parser.cursor(), &[":"]).min(equals);
    let pattern_end = colon.min(equals);
    emit_pattern(parser, pattern_end, SyntaxRole::Pattern);
    bump_until(parser, pattern_end);
    if colon < equals && parser.cursor() == colon {
        parser.bump();
        parser.bump_trivia();
        emit_type(parser, equals, SyntaxRole::Type);
        bump_until(parser, equals);
    }
    if equals < end && parser.cursor() == equals {
        parser.bump();
        parser.bump_trivia();
        emit_expression(parser, end, SyntaxRole::Initializer);
    }
}

fn emit_assertion_children(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    let open = find_top_level_boundary(parser, parser.cursor(), &["("]).min(end);
    bump_until(parser, open.saturating_add(1).min(end));
    let close = find_matching_close(parser, parser.cursor(), "(")
        .unwrap_or(end)
        .min(end);
    if parser.cursor() < close {
        emit_expression(parser, close, SyntaxRole::Condition);
    }
    bump_until(parser, end);
}
