//! Private predicate/proof grammar over the shared document cursor.

use arcweft_source::SourceRange;

use super::document::ShadowDocumentParser;
use super::expression::emit_expression;
use super::lexer::LexToken;
use super::pattern::emit_pattern;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter, expected,
    find_header_boundary, find_top_level_boundary, first_significant, token_count, token_text,
    trimmed_end,
};
use super::statement::emit_block_body;
use super::type_ref::emit_type;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_declaration(
    source: &str,
    tokens: &[LexToken],
    kind: SyntaxKind,
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
) {
    debug_assert!(matches!(
        kind,
        SyntaxKind::PredicateItem | SyntaxKind::ProofItem
    ));
    let mut parser = ShadowDocumentParser::new(source, tokens, events);
    parser.start(kind, role);
    parser.bump_trivia();
    emit_visibility(&mut parser);
    parser.bump_trivia();

    let keyword = if kind == SyntaxKind::PredicateItem {
        "predicate"
    } else {
        "proof"
    };
    if parser.at(keyword) {
        parser.bump();
    }
    parser.bump_trivia();
    emit_name(&mut parser, keyword);
    parser.bump_trivia();

    if parser.at("<") {
        emit_generic_parameters(&mut parser);
        parser.bump_trivia();
    }
    if parser.at("(") {
        emit_fixed_parameters(&mut parser, keyword);
    } else {
        emit_missing_parameter_group(&mut parser, keyword);
    }
    parser.bump_trivia();
    emit_extra_parameter_group_recovery(&mut parser, keyword);

    if parser.at("->") {
        emit_return_type(&mut parser, kind);
        parser.bump_trivia();
    }
    if parser.at("where") {
        emit_where_clause(&mut parser);
        parser.bump_trivia();
    }

    let mut requires = 0_u16;
    let mut ensures = 0_u16;
    let mut saw_ensures = false;
    while matches!(parser.current_text(), Some("requires" | "ensures")) {
        if parser.at("requires") {
            let clause_start = parser.current_offset();
            emit_contract_clause(
                &mut parser,
                SyntaxKind::RequiresClause,
                SyntaxRole::RequiresClause(requires),
            );
            if saw_ensures {
                parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                    "syntax.contract.invalid_clause_order",
                    SourceRange::new(clause_start, parser.current_offset()),
                    "`requires` clauses must precede every `ensures` clause",
                )));
            }
            requires = requires.saturating_add(1);
        } else {
            saw_ensures = true;
            emit_contract_clause(
                &mut parser,
                SyntaxKind::EnsuresClause,
                SyntaxRole::EnsuresClause(ensures),
            );
            ensures = ensures.saturating_add(1);
        }
        parser.bump_trivia();
    }

    emit_body(&mut parser, kind, keyword);
    while parser.bump().is_some() {}
    parser.finish();
}

fn emit_visibility(parser: &mut ShadowDocumentParser<'_, '_>) {
    if !parser.at("pub") {
        return;
    }
    parser.start(SyntaxKind::Visibility, SyntaxRole::Visibility);
    parser.bump();
    if parser.at("(") {
        let mut depth = 0_usize;
        while let Some(text) = parser.current_text() {
            match text {
                "(" => depth += 1,
                ")" if depth == 1 => {
                    parser.bump();
                    break;
                }
                ")" => depth = depth.saturating_sub(1),
                _ => {}
            }
            parser.bump();
        }
    }
    parser.finish();
}

fn emit_name(parser: &mut ShadowDocumentParser<'_, '_>, keyword: &str) {
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
        return;
    }

    parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
    let at = parser.current_offset();
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        if keyword == "predicate" {
            "syntax.predicate.missing_name"
        } else {
            "syntax.proof.missing_name"
        },
        SourceRange::new(at, at),
        format!("missing ordinary name after `{keyword}`"),
    )));
    parser.finish();
}

fn emit_generic_parameters(parser: &mut ShadowDocumentParser<'_, '_>) {
    parser.start(SyntaxKind::GenericParameterGroup, SyntaxRole::GenericGroup);
    emit_open_delimiter(parser, SyntaxKind::OpenAngleNode, "<");
    parser.start(SyntaxKind::GenericParameterList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    loop {
        parser.bump_trivia();
        if parser.is_at_end() || parser.at(">") {
            break;
        }
        let end = find_top_level_boundary(parser, parser.cursor(), &[",", ">"]);
        let first = first_significant(parser, parser.cursor(), end);
        let kind = first.and_then(|index| parser.token_at(index)).map_or(
            SyntaxKind::TypeParameter,
            |token| {
                if token.kind() == SyntaxKind::LifetimeToken {
                    SyntaxKind::LifetimeParameter
                } else {
                    SyntaxKind::TypeParameter
                }
            },
        );
        parser.start(kind, SyntaxRole::GenericParameter(ordinal));
        if let Some(name) = first {
            parser.bump_through(name.saturating_sub(1));
            parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
            parser.bump();
            parser.finish();
        }
        bump_until(parser, end);
        parser.finish();
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseAngleNode,
        ">",
        "syntax.generic.missing_close",
    );
    parser.finish();
}

fn emit_fixed_parameters(parser: &mut ShadowDocumentParser<'_, '_>, keyword: &str) {
    parser.start(SyntaxKind::FixedParameterGroup, SyntaxRole::ParameterGroup);
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    loop {
        parser.bump_trivia();
        if parser.is_at_end() || parser.at(")") {
            break;
        }
        let end = find_top_level_boundary(parser, parser.cursor(), &[",", ")"]);
        emit_parameter(parser, end, ordinal);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        if keyword == "predicate" {
            "syntax.predicate.missing_parameter_close"
        } else {
            "syntax.proof.missing_parameter_close"
        },
    );
    parser.finish();
}

fn emit_missing_parameter_group(parser: &mut ShadowDocumentParser<'_, '_>, keyword: &str) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::FixedParameterGroup, SyntaxRole::ParameterGroup);
    emit_missing_delimiter(parser, SyntaxKind::OpenParenNode, SyntaxRole::OpenDelimiter);
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    parser.finish();
    emit_missing_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        SyntaxRole::CloseDelimiter,
    );
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        if keyword == "predicate" {
            "syntax.predicate.missing_parameters"
        } else {
            "syntax.proof.missing_parameters"
        },
        SourceRange::new(at, at),
        format!("`{keyword}` requires exactly one fixed parameter group"),
    )));
    parser.finish();
}

fn emit_extra_parameter_group_recovery(parser: &mut ShadowDocumentParser<'_, '_>, keyword: &str) {
    let mut ordinal = 0_u32;
    while parser.at("(") {
        let start = parser.current_offset();
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(ordinal));
        parser.bump();
        let close = super::shadow_recovery::find_matching_close(parser, parser.cursor(), "(");
        if let Some(close) = close {
            bump_until(parser, close + 1);
        } else {
            bump_until(parser, token_count(parser));
        }
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            if keyword == "predicate" {
                "syntax.predicate.malformed_header"
            } else {
                "syntax.proof.malformed_header"
            },
            SourceRange::new(start, parser.current_offset()),
            format!("`{keyword}` accepts exactly one fixed parameter group"),
        )));
        ordinal = ordinal.saturating_add(1);
        parser.bump_trivia();
    }
}

fn emit_parameter(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u16) {
    parser.start(SyntaxKind::Parameter, SyntaxRole::Parameter(ordinal));
    let colon = find_top_level_boundary(parser, parser.cursor(), &[":"]);
    let colon = (colon < end && token_text(parser, colon) == Some(":")).then_some(colon);
    let pattern_end = colon.unwrap_or(end);
    emit_pattern(parser, pattern_end, SyntaxRole::ParameterPattern);
    bump_until(parser, pattern_end);
    if let Some(colon) = colon {
        debug_assert_eq!(parser.cursor(), colon);
        parser.bump();
        parser.bump_trivia();
        emit_type(parser, end, SyntaxRole::ParameterType);
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingType, SyntaxRole::ParameterType);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.parameter.missing_type",
            SourceRange::new(at, at),
            "predicate and proof parameters require an authored type",
        )));
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_return_type(parser: &mut ShadowDocumentParser<'_, '_>, item_kind: SyntaxKind) {
    let start = parser.current_offset();
    parser.start(SyntaxKind::ReturnType, SyntaxRole::ReturnType);
    parser.bump();
    parser.bump_trivia();
    let end = find_header_boundary(parser, parser.cursor());
    emit_type(parser, end, SyntaxRole::Type);
    bump_until(parser, end);
    parser.finish();
    if item_kind == SyntaxKind::PredicateItem {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.predicate.return_not_allowed",
            SourceRange::new(start, parser.current_offset()),
            "predicates have an implicit `Bool` return type",
        )));
    }
}

fn emit_where_clause(parser: &mut ShadowDocumentParser<'_, '_>) {
    parser.start(SyntaxKind::WhereClause, SyntaxRole::WhereClause);
    parser.bump();
    parser.bump_trivia();
    let clause_end = find_header_boundary(parser, parser.cursor());
    parser.start(SyntaxKind::WherePredicateList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    while parser.cursor() < clause_end {
        parser.bump_trivia();
        if parser.cursor() >= clause_end {
            break;
        }
        let end = find_top_level_boundary(parser, parser.cursor(), &[","]).min(clause_end);
        parser.start(
            SyntaxKind::WherePredicate,
            SyntaxRole::WherePredicate(ordinal),
        );
        emit_where_predicate_children(parser, trimmed_end(parser, parser.cursor(), end));
        parser.finish();
        bump_until(parser, end);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") && parser.cursor() < clause_end {
            parser.bump();
        }
    }
    parser.finish();
    bump_until(parser, clause_end);
    parser.finish();
}

fn emit_where_predicate_children(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    let colon = find_top_level_boundary(parser, parser.cursor(), &[":"]).min(end);
    if colon == end {
        emit_type(parser, end, SyntaxRole::Type);
        return;
    }

    emit_type(parser, colon, SyntaxRole::LeftOperand);
    bump_until(parser, colon);
    parser.bump();
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= end {
            break;
        }
        let bound_end = find_top_level_boundary(parser, parser.cursor(), &["+"]).min(end);
        emit_type(parser, bound_end, SyntaxRole::Element(ordinal));
        bump_until(parser, bound_end);
        ordinal = ordinal.saturating_add(1);
        if parser.at("+") {
            parser.bump();
        } else {
            break;
        }
    }
}

fn emit_contract_clause(
    parser: &mut ShadowDocumentParser<'_, '_>,
    kind: SyntaxKind,
    role: SyntaxRole,
) {
    parser.start(kind, role);
    parser.bump();
    parser.bump_trivia();
    let end = find_header_boundary(parser, parser.cursor());
    emit_expression(parser, end, SyntaxRole::Condition);
    bump_until(parser, end);
    parser.finish();
}

fn emit_body(parser: &mut ShadowDocumentParser<'_, '_>, item_kind: SyntaxKind, keyword: &str) {
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
        if keyword == "predicate" {
            "syntax.predicate.missing_body"
        } else {
            "syntax.proof.missing_body"
        },
        SourceRange::new(at, at),
        format!("missing `{keyword}` expression or block body"),
    )));
    parser.finish();
}
