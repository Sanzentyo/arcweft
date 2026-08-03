//! Private trait and implementation grammar over the shared document cursor.

use arcweft_source::SourceRange;

use super::cursor::ShadowDocumentParser;
use super::declaration::{
    FixedParameterGrammar, emit_fixed_parameters, emit_generic_parameters,
    emit_missing_parameter_group, emit_name, emit_outer_prefixes, emit_visibility,
    emit_where_clause,
};
use super::lexer::LexToken;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter, expected,
    find_matching_close, find_top_level_boundary, first_significant, token_count, token_text,
    trimmed_end,
};
use super::statement::emit_braced_block;
use super::type_ref::emit_type;
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
    debug_assert!(matches!(kind, SyntaxKind::TraitItem | SyntaxKind::ImplItem));
    let mut parser = ShadowDocumentParser::new(source, tokens, events, budget);
    parser.start(kind, role);
    emit_outer_prefixes(&mut parser);
    parser.bump_trivia();
    emit_visibility(&mut parser);
    parser.bump_trivia();
    if kind == SyntaxKind::TraitItem {
        emit_trait_header(&mut parser);
    } else {
        emit_impl_header(&mut parser);
    }
    parser.bump_trivia();
    if parser.at("where") {
        emit_where_clause(&mut parser);
        parser.bump_trivia();
    }
    emit_member_body(&mut parser, kind);
    emit_trailing_recovery(&mut parser);
    parser.finish();
}

fn emit_trait_header(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.at("trait") {
        parser.bump();
    }
    parser.bump_trivia();
    emit_name(parser, "trait");
    parser.bump_trivia();
    if parser.at("<") {
        emit_generic_parameters(parser);
        parser.bump_trivia();
    }
    if !parser.at(":") {
        return;
    }

    parser.bump();
    let end = find_top_level_boundary(parser, parser.cursor(), &["where", "{"]);
    let mut ordinal = 0_u32;
    while parser.cursor() < end {
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

fn emit_impl_header(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.at("impl") {
        parser.bump();
    }
    parser.bump_trivia();
    if parser.at("<") {
        emit_generic_parameters(parser);
        parser.bump_trivia();
    }

    let end = find_top_level_boundary(parser, parser.cursor(), &["where", "{"]);
    let for_token = find_top_level_boundary(parser, parser.cursor(), &["for"]);
    if for_token < end {
        emit_type(parser, for_token, SyntaxRole::Target);
        bump_until(parser, for_token);
        parser.bump();
        parser.bump_trivia();
        emit_type(parser, end, SyntaxRole::Type);
    } else {
        emit_type(parser, end, SyntaxRole::Type);
    }
    bump_until(parser, end);
}

fn emit_member_body(parser: &mut ShadowDocumentParser<'_, '_>, item_kind: SyntaxKind) {
    if !parser.at("{") {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.trait_impl.missing_body",
            SourceRange::new(at, at),
            "trait or impl declaration requires a braced body",
        )));
        return;
    }

    parser.start(SyntaxKind::DelimitedGroup, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let end = token_count(parser);
    let close = find_matching_close(parser, parser.cursor(), "{").unwrap_or(end);
    parser.start(SyntaxKind::ItemList, SyntaxRole::Element(0));
    emit_members(parser, close, item_kind);
    bump_until(parser, close);
    parser.finish();
    if parser.at("}") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.trait_impl.missing_body_close",
        );
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.trait_impl.missing_body_close",
            SourceRange::new(at, at),
            "missing closing `}` for trait or impl declaration",
        )));
    }
    parser.finish();
}

fn emit_members(parser: &mut ShadowDocumentParser<'_, '_>, close: usize, item_kind: SyntaxKind) {
    let mut ordinal = 0_u32;
    while parser.cursor() < close {
        bump_member_separators(parser);
        if parser.cursor() >= close {
            break;
        }
        let end = member_boundary(parser, parser.cursor(), close);
        match member_head(parser, parser.cursor(), end) {
            Some("type") => {
                emit_associated_type(parser, end, ordinal, item_kind == SyntaxKind::ImplItem);
            }
            Some("fn") => emit_function_member(parser, end, ordinal),
            _ => emit_error_member(parser, end, ordinal),
        }
        bump_until(parser, end);
        ordinal = ordinal.saturating_add(1);
    }
}

fn bump_member_separators(parser: &mut ShadowDocumentParser<'_, '_>) {
    while parser.current_kind().is_some_and(|kind| {
        matches!(
            kind,
            SyntaxKind::WhitespaceToken | SyntaxKind::NewlineToken | SyntaxKind::CommentToken
        )
    }) {
        parser.bump();
    }
}

fn emit_associated_type(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    ordinal: u32,
    target_required: bool,
) {
    let content_end = member_content_end(parser, parser.cursor(), end);
    parser.start(SyntaxKind::TypeAliasItem, SyntaxRole::Element(ordinal));
    emit_outer_prefixes(parser);
    parser.bump_trivia();
    emit_visibility(parser);
    parser.bump_trivia();
    if parser.at("type") {
        parser.bump();
    }
    parser.bump_trivia();
    emit_name(parser, "type");
    parser.bump_trivia();
    if parser.at("<") {
        emit_generic_parameters(parser);
        parser.bump_trivia();
    }

    if parser.at("=") {
        parser.bump();
        parser.bump_trivia();
        emit_type(
            parser,
            trimmed_end(parser, parser.cursor(), content_end),
            SyntaxRole::Type,
        );
    } else if target_required {
        emit_missing_associated_type_target(parser);
    }
    parser.bump_trivia();
    if parser.cursor() < content_end {
        emit_member_tail_error(parser, content_end, "associated type");
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_missing_associated_type_target(parser: &mut ShadowDocumentParser<'_, '_>) {
    let at = parser.current_offset();
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::PunctuationToken),
        at,
    });
    emit_type(parser, parser.cursor(), SyntaxRole::Type);
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.impl.missing_associated_type_target",
        SourceRange::new(at, at),
        "impl associated type requires `= Type`",
    )));
}

fn emit_function_member(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u32) {
    let content_end = member_content_end(parser, parser.cursor(), end);
    parser.start(SyntaxKind::FunctionItem, SyntaxRole::Element(ordinal));
    emit_outer_prefixes(parser);
    parser.bump_trivia();
    emit_visibility(parser);
    parser.bump_trivia();
    if parser.at("fn") {
        parser.bump();
    }
    parser.bump_trivia();
    emit_name(parser, "fn");
    parser.bump_trivia();
    if parser.at("<") {
        emit_generic_parameters(parser);
        parser.bump_trivia();
    }

    let mut groups = 0_u16;
    while parser.at("(") && parser.cursor() < end {
        emit_fixed_parameters(
            parser,
            FixedParameterGrammar::MethodReceiver,
            "trait and impl function parameters require a type",
            "syntax.decl.unclosed_parameters",
        );
        groups = groups.saturating_add(1);
        parser.bump_trivia();
    }
    if groups == 0 {
        emit_missing_parameter_group(parser, "fn", "at least one parameter group");
        parser.bump_trivia();
    }

    if parser.at("->") {
        emit_member_return_type(parser, content_end);
        parser.bump_trivia();
    }
    if parser.at("where") {
        emit_member_where_clause(parser, content_end);
        parser.bump_trivia();
    }

    if parser.at("{") {
        parser.start(SyntaxKind::FunctionBody, SyntaxRole::Body);
        emit_braced_block(
            parser,
            SyntaxKind::FunctionItem,
            SyntaxKind::Block,
            SyntaxRole::Body,
            "syntax.impl.function_missing_block_close",
        );
        parser.finish();
    }
    parser.bump_trivia();
    if parser.cursor() < content_end {
        emit_member_tail_error(parser, content_end, "method");
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_member_tail_error(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    owner: &'static str,
) {
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_until(parser, end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.trait_impl.invalid_member_tail",
        SourceRange::new(start, parser.current_offset()),
        format!("unexpected token in Trait/Impl {owner} declaration"),
    )));
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
        "unexpected syntax after Trait/Impl declaration body",
    )));
}

fn emit_member_return_type(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    parser.start(SyntaxKind::ReturnType, SyntaxRole::ReturnType);
    parser.bump();
    parser.bump_trivia();
    let type_end = find_top_level_boundary(parser, parser.cursor(), &["where", "{"]).min(end);
    emit_type(parser, type_end, SyntaxRole::Type);
    bump_until(parser, type_end);
    parser.finish();
}

fn emit_member_where_clause(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    parser.start(SyntaxKind::WhereClause, SyntaxRole::WhereClause);
    parser.bump();
    parser.bump_trivia();
    let clause_end = find_top_level_boundary(parser, parser.cursor(), &["{"]).min(end);
    parser.start(SyntaxKind::WherePredicateList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    while parser.cursor() < clause_end {
        parser.bump_trivia();
        if parser.cursor() >= clause_end {
            break;
        }
        let predicate_end =
            find_top_level_boundary(parser, parser.cursor(), &[","]).min(clause_end);
        parser.start(
            SyntaxKind::WherePredicate,
            SyntaxRole::WherePredicate(ordinal),
        );
        emit_bound_predicate(parser, predicate_end);
        parser.finish();
        bump_until(parser, predicate_end);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        }
    }
    parser.finish();
    parser.finish();
}

fn emit_bound_predicate(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    let colon = find_top_level_boundary(parser, parser.cursor(), &[":"]).min(end);
    if colon == end {
        emit_type(parser, end, SyntaxRole::Type);
        return;
    }
    emit_type(parser, colon, SyntaxRole::LeftOperand);
    bump_until(parser, colon);
    parser.bump();
    let mut ordinal = 0_u32;
    while parser.cursor() < end {
        parser.bump_trivia();
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

fn emit_error_member(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u32) {
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorItem, SyntaxRole::Element(ordinal));
    bump_until(parser, end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.trait_impl.invalid_member",
        SourceRange::new(start, parser.current_offset()),
        "trait and impl bodies accept associated types and functions",
    )));
}

fn member_head<'a>(
    parser: &'a ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<&'a str> {
    let index = member_head_index(parser, start, end)?;
    token_text(parser, index)
}

fn member_head_index(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<usize> {
    let index = member_payload_index(parser, start, end)?;
    token_text(parser, index)
        .is_some_and(|text| matches!(text, "type" | "fn"))
        .then_some(index)
}

fn member_payload_index(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<usize> {
    let mut index = next_non_trivia(parser, start, end)?;
    while token_text(parser, index) == Some("#") {
        index = next_non_trivia(parser, index + 1, end)?;
        if token_text(parser, index) != Some("[") {
            return None;
        }
        index = skip_balanced_group(parser, index, end, "[", "]")?;
        index = next_non_trivia(parser, index, end)?;
    }
    if token_text(parser, index) == Some("pub") {
        index = next_non_trivia(parser, index + 1, end)?;
        if token_text(parser, index) == Some("(") {
            index = skip_balanced_group(parser, index, end, "(", ")")?;
            index = next_non_trivia(parser, index, end)?;
        }
    }
    Some(index)
}

fn member_boundary(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> usize {
    let mut depth = 0_usize;
    let payload = member_payload_index(parser, start, end);
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            return index;
        };
        let text = parser.text_of(token);
        let saw_payload = payload.is_some_and(|payload| index >= payload);
        if depth == 0 && text == ";" && saw_payload {
            return index + 1;
        }
        if depth == 0 && token.kind() == SyntaxKind::NewlineToken && saw_payload {
            let next =
                first_significant(parser, index + 1, end).and_then(|next| token_text(parser, next));
            if next.is_some_and(|next| matches!(next, "where" | "{" | "(" | "->" | "=")) {
                continue;
            }
            return index;
        }
        match text {
            "(" | "[" | "<" => depth += 1,
            ")" | "]" | ">" => depth = depth.saturating_sub(1),
            "{" => depth += 1,
            "}" if depth != 0 => depth -= 1,
            _ => {}
        }
    }
    end
}

fn member_content_end(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> usize {
    let end = trimmed_end(parser, start, end);
    if end > start && token_text(parser, end - 1) == Some(";") {
        end - 1
    } else {
        end
    }
}

fn next_non_trivia(
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

fn skip_balanced_group(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
    open: &str,
    close: &str,
) -> Option<usize> {
    let mut depth = 0_usize;
    for index in start..end {
        match token_text(parser, index)? {
            text if text == open => depth += 1,
            text if text == close => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
    }
    None
}
