//! Private ordinary-function grammar over the shared document cursor.

use arcweft_source::SourceRange;

use super::cursor::ShadowDocumentParser;
use super::declaration::{
    FixedParameterGrammar, emit_callable_contract_clauses, emit_fixed_parameters,
    emit_generic_parameters, emit_missing_parameter_group, emit_name, emit_outer_prefixes,
    emit_return_type, emit_visibility, emit_where_clause,
};
use super::lexer::LexToken;
use super::statement::emit_braced_block;
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
    parser.start(SyntaxKind::FunctionItem, role);
    emit_outer_prefixes(&mut parser);
    parser.bump_trivia();
    emit_visibility(&mut parser);
    parser.bump_trivia();

    if parser.at("fn") {
        parser.bump();
    }
    parser.bump_trivia();
    emit_name(&mut parser, "fn");
    parser.bump_trivia();

    if parser.at("<") {
        emit_generic_parameters(&mut parser);
        parser.bump_trivia();
    }

    let mut saw_group = false;
    while parser.at("(") {
        emit_fixed_parameters(
            &mut parser,
            FixedParameterGrammar::TypedPattern,
            "ordinary function parameters require an authored type",
            "syntax.decl.unclosed_parameters",
        );
        saw_group = true;
        parser.bump_trivia();
    }
    if !saw_group {
        emit_missing_parameter_group(&mut parser, "fn", "at least one parameter group");
        parser.bump_trivia();
    }

    if parser.at("->") {
        emit_return_type(&mut parser, SyntaxKind::FunctionItem);
        parser.bump_trivia();
    }
    if parser.at("where") {
        emit_where_clause(&mut parser);
        parser.bump_trivia();
    }

    emit_callable_contract_clauses(&mut parser);
    emit_body(&mut parser);
    emit_trailing_recovery(&mut parser);
    parser.finish();
}

fn emit_body(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.at("{") {
        parser.start(SyntaxKind::FunctionBody, SyntaxRole::Body);
        emit_braced_block(
            parser,
            SyntaxKind::FunctionItem,
            SyntaxKind::Block,
            SyntaxRole::Body,
            "syntax.function.missing_block_close",
        );
        parser.finish();
        return;
    }

    let at = parser.current_offset();
    parser.start(SyntaxKind::FunctionBody, SyntaxRole::Body);
    parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.decl.missing_body",
        SourceRange::new(at, at),
        "missing ordinary function block body",
    )));
    parser.finish();
}

fn emit_trailing_recovery(parser: &mut ShadowDocumentParser<'_, '_>) {
    parser.bump_trivia();
    if parser.is_at_end() {
        return;
    }

    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    while parser.bump().is_some() {}
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.declaration.trailing_syntax",
        SourceRange::new(start, parser.current_offset()),
        "unexpected syntax after ordinary function body",
    )));
}
