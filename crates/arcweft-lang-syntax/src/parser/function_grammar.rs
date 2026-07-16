//! Private ordinary-function grammar over the shared document cursor.

use arcweft_source::SourceRange;

use super::declaration::{
    emit_contract_clauses, emit_fixed_parameters, emit_generic_parameters,
    emit_missing_parameter_group, emit_name, emit_outer_prefixes, emit_return_type,
    emit_visibility, emit_where_clause,
};
use super::document::ShadowDocumentParser;
use super::lexer::LexToken;
use super::statement::emit_braced_block;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_declaration(
    source: &str,
    tokens: &[LexToken],
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
) {
    let mut parser = ShadowDocumentParser::new(source, tokens, events);
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

    let mut groups = 0_u16;
    while parser.at("(") {
        emit_fixed_parameters(
            &mut parser,
            "ordinary function parameters require an authored type",
        );
        groups = groups.saturating_add(1);
        parser.bump_trivia();
    }
    if groups == 0 {
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

    emit_contract_clauses(&mut parser);
    emit_body(&mut parser);
    while parser.bump().is_some() {}
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
