//! Private predicate/proof grammar over the shared document cursor.

use arcweft_source::SourceRange;

use super::cursor::DocumentParser;
use super::declaration::{
    FixedParameterGrammar, emit_callable_contract_clauses, emit_declaration_identity,
    emit_extra_parameter_group_recovery, emit_fixed_parameters, emit_generic_parameters,
    emit_missing_parameter_group, emit_outer_prefixes, emit_proof_outer_prefixes, emit_return_type,
    emit_visibility, emit_where_clause,
};
use super::expression::emit_expression;
use super::lexer::LexToken;
use super::shadow_recovery::{bump_until, token_count, trimmed_end};
use super::statement::emit_block_body;
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
    debug_assert!(matches!(
        kind,
        SyntaxKind::PredicateItem | SyntaxKind::ProofItem
    ));
    let mut parser = DocumentParser::new(source, tokens, events, budget);
    let owner = parser.start_projected_owner(kind, role);
    if kind == SyntaxKind::ProofItem {
        emit_proof_outer_prefixes(&mut parser);
    } else {
        emit_outer_prefixes(&mut parser);
    }
    parser.bump_trivia();
    emit_visibility(&mut parser);
    parser.bump_trivia();

    let keyword = if kind == SyntaxKind::PredicateItem {
        "predicate"
    } else {
        "proof"
    };
    let keyword_range = parser.current().map_or_else(
        || SourceRange::new(parser.current_offset(), parser.current_offset()),
        super::lexer::LexToken::range,
    );
    if parser.at(keyword) {
        parser.bump();
    }
    parser.bump_trivia();
    if kind == SyntaxKind::ProofItem {
        let identity = emit_declaration_identity(
            &mut parser,
            arcweft_id::DeclarationIdentityFamily::Proof,
            keyword_range,
        );
        parser.set_declaration_header_projection(owner, identity);
        parser.bump_trivia();
    } else {
        super::declaration::emit_name(&mut parser, keyword);
    }
    parser.bump_trivia();

    if parser.at("<") {
        emit_generic_parameters(&mut parser);
        parser.bump_trivia();
    }
    if parser.at("(") {
        emit_fixed_parameters(
            &mut parser,
            FixedParameterGrammar::TypedPattern,
            "predicate and proof parameters require an authored type",
            if kind == SyntaxKind::PredicateItem {
                "syntax.predicate.missing_parameter_close"
            } else {
                "syntax.proof.missing_parameter_close"
            },
        );
    } else {
        emit_missing_parameter_group(&mut parser, keyword, "exactly one fixed parameter group");
    }
    parser.bump_trivia();
    emit_extra_parameter_group_recovery(
        &mut parser,
        keyword,
        FixedParameterGrammar::TypedPattern,
        "predicate and proof recovery parameters require an authored type",
    );

    if parser.at("->") {
        emit_return_type(&mut parser, kind);
        parser.bump_trivia();
    }
    if parser.at("where") {
        emit_where_clause(&mut parser);
        parser.bump_trivia();
    }

    emit_callable_contract_clauses(&mut parser);
    emit_body(&mut parser, kind, keyword);
    while parser.bump().is_some() {}
    parser.finish();
}

fn emit_body(parser: &mut DocumentParser<'_, '_>, item_kind: SyntaxKind, keyword: &str) {
    let body_kind = if item_kind == SyntaxKind::PredicateItem {
        SyntaxKind::PredicateBody
    } else {
        SyntaxKind::ProofBody
    };
    if parser.at("=") {
        parser.start(body_kind, SyntaxRole::Body);
        parser.start(SyntaxKind::ExpressionBody, SyntaxRole::Body);
        parser.bump();
        parser.bump_trivia();
        let end = trimmed_end(parser, parser.cursor(), token_count(parser));
        emit_expression(parser, end, SyntaxRole::Body);
        bump_until(parser, end);
        parser.finish();
        parser.finish();
        return;
    }
    if parser.at("{") {
        emit_block_body(parser, item_kind, body_kind, keyword);
        return;
    }

    let at = parser.current_offset();
    parser.start(body_kind, SyntaxRole::Body);
    parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        if item_kind == SyntaxKind::PredicateItem {
            "syntax.predicate.missing_body"
        } else {
            "syntax.proof.missing_body"
        },
        SourceRange::new(at, at),
        format!("missing `{keyword}` expression or block body"),
    )));
    parser.finish();
}
