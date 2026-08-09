//! Private retained Signal declaration grammar.

use arcweft_id::DeclarationIdentityFamily;
use arcweft_source::SourceRange;

use super::cursor::DocumentParser;
use super::declaration::emit_retained_declaration_header;
use super::expression::emit_expression;
use super::lexer::LexToken;
use super::shadow_recovery::{
    bump_until, emit_required_punctuation, find_statement_terminator, token_count,
};
use super::type_ref::{emit_type, nominal_type_prefix_end};
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
    let mut parser = DocumentParser::new(source, tokens, events, budget);
    parser.start(SyntaxKind::SignalDeclarationItem, role);
    emit_retained_declaration_header(
        &mut parser,
        DeclarationIdentityFamily::Signal,
        emit_observable_type,
    );
    emit_logical_end_and_recovery(&mut parser);
    parser.finish();
}

fn emit_observable_type(parser: &mut DocumentParser<'_, '_>) {
    emit_required_punctuation(
        parser,
        SyntaxKind::ColonNode,
        SyntaxRole::Colon,
        ":",
        "syntax.signal.missing_colon",
        "Signal declaration requires `: ObservableType`",
    );
    parser.bump_trivia();

    let line_end = statement_end(parser);
    let type_start = parser.cursor();
    let type_end = nominal_type_prefix_end(parser, type_start, line_end);
    parser.start(SyntaxKind::SignalObservableType, SyntaxRole::Type);
    if type_start == type_end {
        let at = parser.current_offset();
        emit_type(parser, type_end, SyntaxRole::Type);
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

fn statement_end(parser: &DocumentParser<'_, '_>) -> usize {
    find_statement_terminator(parser, parser.cursor(), token_count(parser))
        .map_or_else(|| token_count(parser), |(end, _)| end)
}

fn emit_logical_end_and_recovery(parser: &mut DocumentParser<'_, '_>) {
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
    if initializer {
        parser.bump();
        parser.bump_trivia();
        let expression_end = statement_end(parser);
        emit_expression(parser, expression_end, SyntaxRole::Initializer);
        bump_until(parser, expression_end);
    }
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
