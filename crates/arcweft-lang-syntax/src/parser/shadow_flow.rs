//! Private Flow body grammar over the shared full-source cursor.

use arcweft_source::SourceRange;

use super::declaration::{
    emit_contract_clauses, emit_extra_parameter_group_recovery, emit_fixed_parameters,
    emit_generic_parameters, emit_name, emit_outer_prefixes, emit_return_type, emit_visibility,
    emit_where_clause,
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
    parser.start(SyntaxKind::FlowItem, role);
    emit_outer_prefixes(&mut parser);
    parser.bump_trivia();
    emit_visibility(&mut parser);
    parser.bump_trivia();
    if parser.at("flow") {
        parser.bump();
    }
    parser.bump_trivia();
    emit_flow_identity(&mut parser);
    parser.bump_trivia();

    if parser.at("<") {
        emit_generic_parameters(&mut parser);
        parser.bump_trivia();
    }
    if parser.at("(") {
        emit_fixed_parameters(&mut parser, "Flow parameters require an authored type");
        parser.bump_trivia();
    }
    emit_extra_parameter_group_recovery(&mut parser, "flow");
    if parser.at("->") {
        emit_return_type(&mut parser, SyntaxKind::FlowItem);
        parser.bump_trivia();
    }
    if parser.at("where") {
        emit_where_clause(&mut parser);
        parser.bump_trivia();
    }
    loop {
        emit_contract_clauses(&mut parser);
        if !retain_auxiliary_contract_clause(&mut parser) {
            break;
        }
        parser.bump_trivia();
    }

    parser.start(SyntaxKind::FlowBody, SyntaxRole::Body);
    if parser.at("{") {
        emit_braced_block(
            &mut parser,
            SyntaxKind::FlowItem,
            SyntaxKind::Block,
            SyntaxRole::Body,
            "syntax.flow.missing_block_close",
        );
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.flow.missing_body",
            SourceRange::new(at, at),
            "missing Flow body",
        )));
    }
    parser.finish();

    while parser.bump().is_some() {}
    parser.finish();
}

fn emit_flow_identity(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.current_kind() != Some(SyntaxKind::EntityReferenceToken) {
        emit_name(parser, "flow");
        return;
    }

    parser.bump();
    parser.bump_trivia();
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        emit_name(parser, "flow identity");
    }
}

fn retain_auxiliary_contract_clause(parser: &mut ShadowDocumentParser<'_, '_>) -> bool {
    if !matches!(
        parser.current_text(),
        Some("invariant" | "assume" | "reads" | "effects" | "modifies" | "decreases")
    ) {
        return false;
    }

    let mut depth = 0_usize;
    while let Some(token) = parser.current() {
        let text = parser.text_of(token);
        let line_end = token.kind() == SyntaxKind::NewlineToken && depth == 0;
        if token.kind() == SyntaxKind::PunctuationToken {
            match text {
                "(" | "[" | "{" | "<" => depth += 1,
                ")" | "]" | "}" | ">" => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        parser.bump();
        if line_end {
            break;
        }
    }
    true
}
