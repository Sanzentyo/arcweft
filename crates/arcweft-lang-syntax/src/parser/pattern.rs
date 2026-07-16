//! Private nested pattern-family events over the shared cursor.

use super::document::ShadowDocumentParser;
use super::path::emit_path;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_open_delimiter, find_matching_close,
    find_top_level_boundary, first_significant, token_text, trimmed_end,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_pattern(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) {
    let end = trimmed_end(parser, parser.cursor(), end);
    if parser.cursor() >= end {
        parser.start(SyntaxKind::MissingPattern, role);
        parser.finish();
        return;
    }

    if boundary(parser, parser.cursor(), end, &["|"]).is_some() {
        emit_or_pattern(parser, end, role);
        return;
    }
    if let Some(rest) = whole_binding_rest(parser, parser.cursor(), end) {
        emit_whole_binding_pattern(parser, rest, end, role);
        return;
    }
    if is_variant_pattern(parser, parser.cursor(), end) {
        emit_variant_pattern(parser, end, role);
        return;
    }
    if boundary(parser, parser.cursor(), end, &["{"]).is_some() {
        emit_record_pattern(parser, end, role);
        return;
    }

    match parser.current_text() {
        Some("_") => emit_flat_pattern(parser, end, SyntaxKind::WildcardPattern, role),
        Some("mut") => emit_mutable_binding_pattern(parser, end, role),
        Some("(") => emit_tuple_pattern(parser, end, role),
        Some("[") => emit_sequence_pattern(parser, end, role),
        Some("..") => emit_rest_pattern(parser, end, role),
        _ if parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) => {
            emit_flat_pattern(parser, end, SyntaxKind::EntityReferencePattern, role);
        }
        _ if parser.current_kind().is_some_and(is_literal) => {
            emit_flat_pattern(parser, end, SyntaxKind::LiteralPattern, role);
        }
        _ if matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        ) =>
        {
            emit_binding_pattern(parser, end, role);
        }
        _ => emit_flat_pattern(parser, end, SyntaxKind::ErrorPattern, role),
    }
}

fn emit_or_pattern(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    parser.start(SyntaxKind::OrPattern, role);
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= end {
            break;
        }
        let alternative_end = find_top_level_boundary(parser, parser.cursor(), &["|"]).min(end);
        emit_pattern(parser, alternative_end, SyntaxRole::Element(ordinal));
        bump_until(parser, alternative_end);
        ordinal = ordinal.saturating_add(1);
        if parser.at("|") {
            parser.bump();
        } else {
            break;
        }
    }
    parser.finish();
}

fn emit_whole_binding_pattern(
    parser: &mut ShadowDocumentParser<'_, '_>,
    rest: usize,
    end: usize,
    role: SyntaxRole,
) {
    parser.start(SyntaxKind::WholeBindingPattern, role);
    emit_binding_pattern(parser, parser.cursor() + 1, SyntaxRole::Name);
    bump_until(parser, rest);
    emit_pattern(parser, end, SyntaxRole::Pattern);
    parser.finish();
}

fn emit_variant_pattern(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    let payload = boundary(parser, parser.cursor(), end, &["(", "{"]);
    let head_end = payload.unwrap_or(end);
    parser.start(SyntaxKind::VariantPattern, role);
    if parser.at(".") {
        parser.bump();
        parser.bump_trivia();
    }
    emit_path(parser, head_end, SyntaxRole::Target);
    bump_until(parser, head_end);
    match payload.and_then(|_| parser.current_text()) {
        Some("(") => emit_pattern_group(parser, end),
        Some("{") => emit_record_fields(parser, end),
        _ => {}
    }
    parser.finish();
}

fn emit_record_pattern(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    let open = boundary(parser, parser.cursor(), end, &["{"]).expect("classified record pattern");
    parser.start(SyntaxKind::RecordPattern, role);
    if parser.cursor() < open {
        emit_path(parser, open, SyntaxRole::Target);
        bump_until(parser, open);
    }
    emit_record_fields(parser, end);
    parser.finish();
}

fn emit_pattern_group(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    let close = find_matching_close(parser, parser.cursor(), "(")
        .unwrap_or(end)
        .min(end);
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    emit_pattern_list(parser, close, ")");
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.pattern.missing_variant_close",
    );
}

fn emit_tuple_pattern(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    parser.start(SyntaxKind::TuplePattern, role);
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    let close = find_matching_close(parser, parser.cursor(), "(")
        .unwrap_or(end)
        .min(end);
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    emit_pattern_list(parser, close, ")");
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.pattern.missing_tuple_close",
    );
    parser.finish();
}

fn emit_sequence_pattern(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    parser.start(SyntaxKind::SequencePattern, role);
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    let close = find_matching_close(parser, parser.cursor(), "[")
        .unwrap_or(end)
        .min(end);
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at("]") {
            break;
        }
        let element_end = find_top_level_boundary(parser, parser.cursor(), &[",", "]"]).min(close);
        if parser.at("..") {
            emit_rest_pattern(parser, element_end, SyntaxRole::Element(ordinal));
        } else {
            emit_pattern(parser, element_end, SyntaxRole::Element(ordinal));
        }
        bump_until(parser, element_end);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.pattern.missing_sequence_close",
    );
    parser.finish();
}

fn emit_record_fields(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{")
        .unwrap_or(end)
        .min(end);
    parser.start(SyntaxKind::FieldList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at("}") {
            break;
        }
        let field_end = find_top_level_boundary(parser, parser.cursor(), &[",", "}"]).min(close);
        if parser.at("..") {
            emit_rest_pattern(parser, field_end, SyntaxRole::Field(ordinal));
        } else {
            emit_record_field(parser, field_end, ordinal);
        }
        bump_until(parser, field_end);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.pattern.missing_record_close",
    );
}

fn emit_record_field(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, ordinal: u16) {
    parser.start(SyntaxKind::RecordPatternField, SyntaxRole::Field(ordinal));
    let colon = boundary(parser, parser.cursor(), end, &[":"]);
    if let Some(colon) = colon {
        parser.start(SyntaxKind::NameReference, SyntaxRole::Name);
        bump_until(parser, trimmed_end(parser, parser.cursor(), colon));
        parser.finish();
        bump_until(parser, colon);
        parser.bump();
        parser.bump_trivia();
        emit_pattern(parser, end, SyntaxRole::Pattern);
    } else {
        emit_binding_pattern(parser, end, SyntaxRole::Pattern);
    }
    parser.finish();
}

fn emit_pattern_list(parser: &mut ShadowDocumentParser<'_, '_>, close: usize, delimiter: &str) {
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at(delimiter) {
            break;
        }
        let element_end =
            find_top_level_boundary(parser, parser.cursor(), &[",", delimiter]).min(close);
        emit_pattern(parser, element_end, SyntaxRole::Element(ordinal));
        bump_until(parser, element_end);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
}

fn emit_binding_pattern(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    parser.start(SyntaxKind::BindingPattern, role);
    parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
    bump_until(parser, end);
    parser.finish();
    parser.finish();
}

fn emit_mutable_binding_pattern(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) {
    parser.start(SyntaxKind::MutableBindingPattern, role);
    parser.bump();
    parser.bump_trivia();
    parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
    bump_until(parser, end);
    parser.finish();
    parser.finish();
}

fn emit_rest_pattern(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    parser.start(SyntaxKind::RestPattern, role);
    parser.bump();
    parser.bump_trivia();
    if parser.cursor() < end {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        bump_until(parser, end);
        parser.finish();
    }
    parser.finish();
}

fn emit_flat_pattern(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    kind: SyntaxKind,
    role: SyntaxRole,
) {
    parser.start(kind, role);
    bump_until(parser, end);
    parser.finish();
}

fn whole_binding_rest(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<usize> {
    let first = first_significant(parser, start, end)?;
    let token = parser.token_at(first)?;
    if token.kind() != SyntaxKind::IdentifierToken {
        return None;
    }
    let rest = first_significant(parser, first + 1, end)?;
    let rest_token = parser.token_at(rest)?;
    let rest_text = parser.text_of(rest_token);
    (matches!(rest_text, "." | "(" | "[")
        || rest_token.kind() == SyntaxKind::EntityReferenceToken
        || is_literal(rest_token.kind()))
    .then_some(rest)
}

fn is_variant_pattern(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> bool {
    if token_text(parser, start) == Some(".") {
        return true;
    }
    let first_text =
        first_significant(parser, start, end).and_then(|index| token_text(parser, index));
    if matches!(first_text, Some("Some" | "None" | "Ok" | "Err")) {
        return true;
    }
    boundary(parser, start, end, &["."]).is_some()
}

fn boundary(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
    spellings: &[&str],
) -> Option<usize> {
    let found = find_top_level_boundary(parser, start, spellings);
    (found < end).then_some(found)
}

const fn is_literal(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::NumberToken
            | SyntaxKind::StringToken
            | SyntaxKind::RawStringToken
            | SyntaxKind::CharacterToken
    )
}
