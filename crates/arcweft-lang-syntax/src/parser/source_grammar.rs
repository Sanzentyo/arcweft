//! Attached `source` declaration grammar over the shared document cursor.

use arcweft_id::PublicId;
use arcweft_source::SourceRange;

use super::declaration::{emit_contract_clause_until, emit_outer_prefixes, emit_visibility};
use super::document::ShadowDocumentParser;
use super::expression::emit_expression;
use super::lexer::LexToken;
use super::pattern::emit_pattern;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter, expected,
    find_matching_close, find_statement_terminator, find_top_level_boundary, first_significant,
    token_count, token_text, trimmed_end,
};
use super::statement::{emit_braced_block_until, emit_statement_fragment};
use super::type_ref::emit_type;
use crate::grammar::budget::GrammarBudget;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceIdProblem {
    WrongFamily,
    Malformed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceIdEmission {
    requires_name: bool,
    consumed_type_colon: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SourceContractLedger {
    requires: u16,
    ensures: u16,
    saw_ensures: bool,
}

pub(super) fn emit_declaration(
    source: &str,
    tokens: &[LexToken],
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) {
    let mut parser = ShadowDocumentParser::new(source, tokens, events, budget);
    parser.start(SyntaxKind::SourceItem, role);
    parser.start(SyntaxKind::DeclarationHeader, SyntaxRole::Element(0));
    emit_outer_prefixes(&mut parser);
    parser.bump_trivia();
    emit_visibility(&mut parser);
    parser.bump_trivia();

    if parser.at("source") {
        parser.bump();
    }
    parser.bump_trivia();
    let public_id = emit_public_id(&mut parser);
    parser.bump_trivia();
    match public_id {
        None => emit_required_name(&mut parser),
        Some(emission) if emission.requires_name && emission.consumed_type_colon => {
            emit_missing_name(&mut parser);
        }
        Some(emission) if emission.requires_name => emit_required_name(&mut parser),
        Some(emission) if !emission.consumed_type_colon => emit_optional_name(&mut parser),
        Some(_) => {}
    }
    parser.bump_trivia();
    emit_source_type(
        &mut parser,
        public_id.is_some_and(|emission| emission.consumed_type_colon),
    );
    parser.finish();
    emit_source_body(&mut parser);
    while parser.bump().is_some() {}
    parser.finish();
}

fn emit_public_id(parser: &mut ShadowDocumentParser<'_, '_>) -> Option<SourceIdEmission> {
    if parser.current_kind() != Some(SyntaxKind::EntityReferenceToken) {
        return None;
    }

    let token = parser
        .current()
        .expect("checked source declaration ID token");
    let spelling = parser.text_of(token);
    let trailing_type_colon = spelling.ends_with(':');
    let id_range = if trailing_type_colon {
        SourceRange::new(token.range().start(), token.range().end().saturating_sub(1))
    } else {
        token.range()
    };
    let id_spelling = if trailing_type_colon {
        &spelling[..spelling.len().saturating_sub(1)]
    } else {
        spelling
    };
    let (requires_name, problem) = classify_source_id(id_spelling);

    parser.start(SyntaxKind::DeclarationPublicId, SyntaxRole::PublicId);
    if let Some(problem) = problem {
        parser.start(
            match problem {
                SourceIdProblem::WrongFamily => SyntaxKind::WrongFamilyReference,
                SourceIdProblem::Malformed => SyntaxKind::ErrorNode,
            },
            match problem {
                SourceIdProblem::WrongFamily => SyntaxRole::Reference(0),
                SourceIdProblem::Malformed => SyntaxRole::Recovery(0),
            },
        );
    }
    if trailing_type_colon {
        parser.take_for_partition();
        parser.push(SyntaxEvent::token(
            SyntaxKind::EntityReferenceToken,
            id_range,
        ));
    } else {
        parser.bump();
    }
    if problem.is_some() {
        parser.finish();
    }
    parser.finish();
    if let Some(problem) = problem {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            match problem {
                SourceIdProblem::WrongFamily => "syntax.source.wrong_family_id",
                SourceIdProblem::Malformed => "syntax.source.malformed_id",
            },
            id_range,
            match problem {
                SourceIdProblem::WrongFamily => {
                    "source declaration ID must belong to the `source` family"
                }
                SourceIdProblem::Malformed => "source declaration ID is malformed",
            },
        )));
    }
    if trailing_type_colon {
        parser.push(SyntaxEvent::token(
            SyntaxKind::PunctuationToken,
            SourceRange::new(id_range.end(), token.range().end()),
        ));
    }
    Some(SourceIdEmission {
        requires_name,
        consumed_type_colon: trailing_type_colon,
    })
}

fn classify_source_id(spelling: &str) -> (bool, Option<SourceIdProblem>) {
    let Some(body) = spelling.strip_prefix('@') else {
        return (false, Some(SourceIdProblem::Malformed));
    };

    if let Some(delimited) = body.strip_prefix('<') {
        let Some(delimited) = delimited.strip_suffix('>') else {
            return (false, Some(SourceIdProblem::Malformed));
        };
        if PublicId::try_new(delimited).is_err() {
            return (false, Some(SourceIdProblem::Malformed));
        }
        return (
            false,
            delimited
                .strip_prefix("source.")
                .is_none_or(str::is_empty)
                .then_some(SourceIdProblem::WrongFamily),
        );
    }

    if let Some((family, relative)) = body.split_once(":.") {
        let requires_name = relative.is_empty();
        if family.is_empty() || !valid_id_tail(relative.trim_start_matches('.'), requires_name) {
            return (requires_name, Some(SourceIdProblem::Malformed));
        }
        return (
            requires_name,
            (family != "source").then_some(SourceIdProblem::WrongFamily),
        );
    }

    if body.starts_with('.') {
        let suffix = body.trim_start_matches('.');
        let requires_name = suffix.is_empty();
        let valid_marker = body == ".";
        return (
            requires_name,
            (!valid_id_tail(suffix, requires_name) || (requires_name && !valid_marker))
                .then_some(SourceIdProblem::Malformed),
        );
    }

    if body.starts_with("super.") {
        let mut suffix = body;
        while let Some(rest) = suffix.strip_prefix("super.") {
            suffix = rest;
        }
        let requires_name = suffix.is_empty();
        return (
            requires_name,
            (!valid_id_tail(suffix, false)).then_some(SourceIdProblem::Malformed),
        );
    }

    if !valid_absolute_id(body) {
        return (false, Some(SourceIdProblem::Malformed));
    }
    (
        false,
        body.strip_prefix("source.")
            .is_none_or(str::is_empty)
            .then_some(SourceIdProblem::WrongFamily),
    )
}

fn valid_absolute_id(body: &str) -> bool {
    PublicId::try_new(body).is_ok()
        && !body.contains([':', '/', '{', '}'])
        && body.split('.').all(|component| !component.is_empty())
}

fn valid_id_tail(tail: &str, allow_empty: bool) -> bool {
    if tail.is_empty() {
        return allow_empty;
    }
    !tail.contains([':', '/', '{', '}'])
        && tail.split('.').all(|component| !component.is_empty())
        && PublicId::try_new(format!("source.{tail}")).is_ok()
}

fn emit_optional_name(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
    }
}

fn emit_required_name(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        emit_optional_name(parser);
    } else {
        emit_missing_name(parser);
    }
}

fn emit_missing_name(parser: &mut ShadowDocumentParser<'_, '_>) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.source.missing_name",
        SourceRange::new(at, at),
        "source declaration requires a public ID or local name",
    )));
}

fn emit_source_type(parser: &mut ShadowDocumentParser<'_, '_>, consumed_colon: bool) {
    if !consumed_colon {
        if parser.at(":") {
            parser.bump();
        } else {
            let at = parser.current_offset();
            parser.push(SyntaxEvent::MissingToken {
                expected: expected(SyntaxKind::PunctuationToken),
                at,
            });
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.source.missing_colon",
                SourceRange::new(at, at),
                "source declaration requires `:` before its source type",
            )));
        }
    }
    parser.bump_trivia();

    let body = (parser.cursor()..token_count(parser))
        .find(|index| token_text(parser, *index) == Some("{"))
        .unwrap_or_else(|| token_count(parser));
    let end = trimmed_end(parser, parser.cursor(), body);
    if parser.cursor() == end {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingType, SyntaxRole::Type);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::IdentifierToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.source.missing_type",
            SourceRange::new(at, at),
            "source declaration requires a source type",
        )));
        return;
    }
    emit_type(parser, end, SyntaxRole::Type);
    bump_until(parser, end);
}

fn emit_source_body(parser: &mut ShadowDocumentParser<'_, '_>) {
    parser.bump_trivia();
    if !parser.at("{") {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.source.missing_body",
            SourceRange::new(at, at),
            "source declaration requires a body",
        )));
        return;
    }

    parser.start(SyntaxKind::Block, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{").unwrap_or(token_count(parser));
    parser.start(SyntaxKind::StatementList, SyntaxRole::Element(0));
    let mut ordinal = 0_u32;
    let mut contracts = SourceContractLedger::default();
    while parser.cursor() < close {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }

        let start = parser.cursor();
        let terminator = find_source_entry_terminator(parser, start, close);
        let segment_end = terminator.map_or(close, |(index, _)| index);
        let significant_end = trimmed_end(parser, start, segment_end);
        if significant_end == start {
            bump_until(parser, segment_end.saturating_add(1));
            continue;
        }
        let end = if terminator.is_some_and(|(_, semicolon)| semicolon) {
            segment_end.saturating_add(1)
        } else {
            significant_end
        };
        let is_statement = emit_source_body_entry(parser, end, ordinal, &mut contracts);
        bump_until(parser, end);
        if is_statement {
            ordinal = ordinal.saturating_add(1);
        }
    }
    parser.finish();
    parser.start(SyntaxKind::OmittedBlockTail, SyntaxRole::Tail);
    parser.finish();

    if parser.cursor() == close && parser.at("}") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.source.missing_block_close",
        );
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.source.missing_block_close",
            SourceRange::new(at, at),
            "missing closing `}` for source body",
        )));
    }
    parser.finish();
}

fn find_source_entry_terminator(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    close: usize,
) -> Option<(usize, bool)> {
    let ordinary = find_statement_terminator(parser, start, close);
    if token_text(parser, start) != Some("on") {
        return ordinary;
    }

    let arrow = find_top_level_boundary(parser, start, &["=>"]).min(close);
    if arrow >= close || token_text(parser, arrow) != Some("=>") {
        return ordinary;
    }
    let Some(body_open) = first_significant(parser, arrow.saturating_add(1), close) else {
        return ordinary;
    };
    if token_text(parser, body_open) != Some("{") {
        return ordinary;
    }

    let mut brace_depth = 1_usize;
    for index in body_open.saturating_add(1)..close {
        match token_text(parser, index) {
            Some("{") => brace_depth = brace_depth.saturating_add(1),
            Some("}") => brace_depth = brace_depth.saturating_sub(1),
            _ => {}
        }
        if parser
            .token_at(index)
            .is_some_and(|token| token.kind() == SyntaxKind::NewlineToken)
            && brace_depth == 1
            && first_significant(parser, index.saturating_add(1), close)
                .and_then(|next| token_text(parser, next))
                .is_some_and(is_source_body_head)
        {
            let recovered = (index, false);
            return ordinary
                .filter(|(ordinary_index, _)| *ordinary_index <= index)
                .or(Some(recovered));
        }
    }
    ordinary
}

fn is_source_body_head(spelling: &str) -> bool {
    matches!(
        spelling,
        "from" | "backpressure" | "replay" | "privacy" | "on" | "requires" | "ensures"
    )
}

fn emit_source_body_entry(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    ordinal: u32,
    contracts: &mut SourceContractLedger,
) -> bool {
    match parser.current_text() {
        Some("from") => emit_source_from(parser, end, ordinal),
        Some("on") => emit_source_handler(parser, end, ordinal),
        Some("requires") => {
            let clause_start = parser.current_offset();
            emit_contract_clause_until(
                parser,
                end,
                SyntaxKind::RequiresClause,
                SyntaxRole::RequiresClause(contracts.requires),
            );
            if contracts.saw_ensures {
                parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                    "syntax.contract.invalid_clause_order",
                    SourceRange::new(clause_start, parser.current_offset()),
                    "`requires` clauses must precede every `ensures` clause",
                )));
            }
            contracts.requires = contracts.requires.saturating_add(1);
            return false;
        }
        Some("ensures") => {
            emit_contract_clause_until(
                parser,
                end,
                SyntaxKind::EnsuresClause,
                SyntaxRole::EnsuresClause(contracts.ensures),
            );
            contracts.ensures = contracts.ensures.saturating_add(1);
            contracts.saw_ensures = true;
            return false;
        }
        _ => emit_statement_fragment(parser, end, SyntaxRole::Statement(ordinal)),
    }
    true
}

fn emit_source_from(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u32) {
    parser.start(
        SyntaxKind::ExpressionStatement,
        SyntaxRole::Statement(ordinal),
    );
    parser.bump();
    parser.bump_trivia();
    emit_expression(parser, end, SyntaxRole::Initializer);
    bump_until(parser, end);
    parser.finish();
}

fn emit_source_handler(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u32) {
    parser.start(SyntaxKind::OnStatement, SyntaxRole::Statement(ordinal));
    parser.bump();
    parser.bump_trivia();
    let arrow = find_top_level_boundary(parser, parser.cursor(), &["=>"]).min(end);
    let has_arrow = arrow < end && token_text(parser, arrow) == Some("=>");
    let event_start = first_significant(parser, parser.cursor(), arrow);
    match event_start.and_then(|index| token_text(parser, index)) {
        Some("item" | "error" | "progress") => {
            parser.start(SyntaxKind::NameReference, SyntaxRole::Name);
            parser.bump();
            parser.finish();
            parser.bump_trivia();
            emit_pattern(parser, arrow, SyntaxRole::Pattern);
        }
        _ => emit_expression(parser, arrow, SyntaxRole::Condition),
    }
    bump_until(parser, arrow);

    if !has_arrow {
        let at = parser.current_offset();
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::PunctuationToken),
            at,
        });
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.source.missing_handler_arrow",
            SourceRange::new(at, at),
            "source handler requires `=>` before its body",
        )));
        parser.finish();
        return;
    }

    parser.bump();
    parser.bump_trivia();
    let body_end = trimmed_end(parser, parser.cursor(), end);
    if parser.cursor() >= body_end {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.source.missing_handler_body",
            SourceRange::new(at, at),
            "source handler requires a statement or block body",
        )));
    } else if parser.at("{") {
        emit_braced_block_until(
            parser,
            end,
            SyntaxKind::SourceItem,
            SyntaxKind::Block,
            SyntaxRole::Body,
            "syntax.source.missing_handler_close",
        );
    } else {
        emit_statement_fragment(parser, end, SyntaxRole::Body);
    }
    bump_until(parser, end);
    parser.finish();
}
