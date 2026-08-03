//! Exact grammar transactions for keyword-owned control statements.

use arcweft_source::SourceRange;

use super::{emit_item_expression, top_level_operator};
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::name::{SyntaxName, SyntaxNameIssue};
use crate::parser::cursor::ShadowDocumentParser;
use crate::parser::shadow_recovery::bump_until;

pub(in crate::parser) fn emit_keyword_statement(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    kind: SyntaxKind,
) -> PendingKeywordStatementProjection {
    parser.bump();
    parser.bump_trivia();

    match kind {
        SyntaxKind::OutStatement => {
            let label = emit_optional_label(parser, end);
            emit_item_expression(parser, end, SyntaxRole::Initializer, item_kind);
            PendingKeywordStatementProjection::Out { label }
        }
        SyntaxKind::GotoStatement => {
            emit_item_expression(parser, end, SyntaxRole::Target, item_kind);
            PendingKeywordStatementProjection::Goto
        }
        SyntaxKind::DeferStatement => {
            emit_item_expression(parser, end, SyntaxRole::Initializer, item_kind);
            PendingKeywordStatementProjection::Defer
        }
        SyntaxKind::SignalStatement => {
            emit_signal(parser, end, item_kind);
            PendingKeywordStatementProjection::Signal
        }
        SyntaxKind::BreakStatement => {
            let label = emit_optional_label(parser, end);
            if parser.cursor() < end {
                emit_item_expression(parser, end, SyntaxRole::Initializer, item_kind);
            }
            PendingKeywordStatementProjection::Break { label }
        }
        SyntaxKind::ContinueStatement => {
            let label = emit_optional_label(parser, end);
            if parser.cursor() < end {
                emit_unexpected_continue_suffix(parser, end);
            }
            PendingKeywordStatementProjection::Continue { label }
        }
        _ => unreachable!("keyword statement dispatcher is closed over six families"),
    }
}

fn emit_optional_label(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
) -> Option<Result<SyntaxName, SyntaxNameIssue>> {
    if parser.cursor() >= end || parser.current_kind() != Some(SyntaxKind::LifetimeToken) {
        return None;
    }

    let token = parser.current().expect("preflighted control label token");
    let spelling = parser.text_of(token);
    let label = SyntaxName::try_new(spelling.strip_prefix('\'').unwrap_or(""));
    parser.start(SyntaxKind::NameReference, SyntaxRole::Label(0));
    parser.bump();
    parser.finish();
    parser.bump_trivia();
    Some(label)
}

fn emit_signal(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, item_kind: SyntaxKind) {
    let arrow = top_level_operator(parser, parser.cursor(), end, "<-");
    let target_end = arrow.unwrap_or(end);
    emit_item_expression(parser, target_end, SyntaxRole::Target, item_kind);
    bump_until(parser, target_end);

    if arrow.is_some() && parser.at("<-") {
        parser.bump();
        parser.bump_trivia();
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        parser.push(SyntaxEvent::MissingToken {
            expected: crate::grammar::event::ExpectedToken::try_with_spelling(
                SyntaxKind::PunctuationToken,
                "<-",
            )
            .expect("real grammar punctuation token"),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.statement.missing_signal_arrow",
            SourceRange::new(at, at),
            "signal requires `<-` between its target and value",
        )));
    }

    emit_item_expression(parser, end, SyntaxRole::Initializer, item_kind);
}

fn emit_unexpected_continue_suffix(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_until(parser, end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.statement.unexpected_continue_value",
        SourceRange::new(start, parser.current_offset()),
        "continue accepts an optional label but no value",
    )));
}
