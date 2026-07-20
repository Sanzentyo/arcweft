//! Private retained Signal declaration grammar.

use arcweft_id::RetainedIdentityFamily;
use arcweft_source::SourceRange;

use super::declaration::emit_retained_declaration_header;
use super::document::ShadowDocumentParser;
use super::lexer::LexToken;
use super::shadow_recovery::{
    bump_until, expected, find_matching_close, find_statement_terminator, token_count, trimmed_end,
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
    parser.start(SyntaxKind::SignalDeclarationItem, role);
    emit_retained_declaration_header(
        &mut parser,
        RetainedIdentityFamily::Signal,
        emit_observable_type,
    );
    emit_logical_end_and_recovery(&mut parser);
    parser.finish();
}

fn emit_observable_type(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.at(":") {
        parser.bump();
    } else {
        let at = parser.current_offset();
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::PunctuationToken),
            at,
        });
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.signal.missing_colon",
            SourceRange::new(at, at),
            "Signal declaration requires `: ObservableType`",
        )));
    }
    parser.bump_trivia();

    let line_end = statement_end(parser);
    let type_start = parser.cursor();
    let type_end = observable_type_end(parser, type_start, line_end);
    parser.start(SyntaxKind::SignalObservableType, SyntaxRole::Type);
    if type_start == type_end {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingType, SyntaxRole::Type);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.signal.missing_type",
            SourceRange::new(at, at),
            "Signal declaration requires an observable type",
        )));
    } else {
        emit_type(parser, type_end, SyntaxRole::Type);
    }
    bump_until(parser, type_end);
    parser.finish();
}

fn statement_end(parser: &ShadowDocumentParser<'_, '_>) -> usize {
    find_statement_terminator(parser, parser.cursor(), token_count(parser))
        .map_or_else(|| token_count(parser), |(end, _)| end)
}

fn observable_type_end(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    line_end: usize,
) -> usize {
    let Some(open) = next_significant_index(parser, start, line_end)
        .and_then(|head| next_significant_index(parser, head + 1, line_end))
        .filter(|index| {
            parser
                .token_at(*index)
                .is_some_and(|token| parser.text_of(token) == "<")
        })
    else {
        return trimmed_end(parser, start, line_end);
    };
    find_matching_close(parser, open + 1, "<").map_or(line_end, |close| (close + 1).min(line_end))
}

fn emit_logical_end_and_recovery(parser: &mut ShadowDocumentParser<'_, '_>) {
    parser.bump_trivia();
    if parser.at(";") {
        parser.bump();
        parser.bump_trivia();
    }
    if parser.is_at_end() {
        return;
    }

    let start = parser.current_offset();
    let initializer = parser.at("=");
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    while parser.bump().is_some() {}
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        if initializer {
            "syntax.signal.initializer_not_allowed"
        } else {
            "syntax.declaration.trailing_syntax"
        },
        SourceRange::new(start, parser.current_offset()),
        "Signal declaration accepts no initializer, body, policy, or adapter binding",
    )));
}

fn next_significant_index(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<usize> {
    (start..end).find(|index| {
        parser.token_at(*index).is_some_and(|token| {
            !matches!(
                token.kind(),
                SyntaxKind::WhitespaceToken
                    | SyntaxKind::NewlineToken
                    | SyntaxKind::CommentToken
                    | SyntaxKind::DocCommentToken
            )
        })
    })
}
