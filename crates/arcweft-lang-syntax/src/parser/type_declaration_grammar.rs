//! Private nominal-type declaration grammar over the shared document cursor.

use arcweft_source::SourceRange;

use super::cursor::DocumentParser;
use super::declaration::{
    emit_generic_parameters, emit_name, emit_outer_prefixes, emit_visibility, emit_where_clause,
};
use super::lexer::LexToken;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter,
    emit_required_punctuation, expected, find_matching_close, find_top_level_boundary,
    first_significant, token_count, token_text, trimmed_end,
};
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
    debug_assert!(matches!(
        kind,
        SyntaxKind::EnumItem | SyntaxKind::StructItem | SyntaxKind::TypeAliasItem
    ));
    let mut parser = DocumentParser::new(source, tokens, events, budget);
    parser.start(kind, role);
    emit_outer_prefixes(&mut parser);
    parser.bump_trivia();
    emit_visibility(&mut parser);
    parser.bump_trivia();

    let keyword = match kind {
        SyntaxKind::EnumItem => "enum",
        SyntaxKind::StructItem => "struct",
        SyntaxKind::TypeAliasItem => "type",
        _ => unreachable!("validated nominal declaration kind"),
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

    if kind == SyntaxKind::TypeAliasItem {
        emit_type_alias_tail(&mut parser);
    } else {
        if parser.at("where") {
            emit_where_clause(&mut parser);
            parser.bump_trivia();
        }
        emit_nominal_body(&mut parser, kind);
    }

    while parser.bump().is_some() {}
    parser.finish();
}

fn emit_type_alias_tail(parser: &mut DocumentParser<'_, '_>) {
    let has_equals = emit_required_punctuation(
        parser,
        SyntaxKind::EqualsNode,
        SyntaxRole::Equals,
        "=",
        "syntax.type_alias.missing_equals",
        "type alias requires `=` before its target type",
    );
    if has_equals {
        parser.bump_trivia();
    }

    let target_end =
        find_top_level_boundary(parser, parser.cursor(), token_count(parser), &["where"]);
    emit_type(parser, target_end, SyntaxRole::Type);
    bump_until(parser, target_end);
    parser.bump_trivia();
    emit_alias_where_clauses(parser);
}

fn emit_alias_where_clauses(parser: &mut DocumentParser<'_, '_>) {
    while parser.at("where") {
        emit_where_clause(parser);
        parser.bump_trivia();
    }
}

fn emit_nominal_body(parser: &mut DocumentParser<'_, '_>, item_kind: SyntaxKind) {
    if !parser.at("{") {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.nominal.missing_body",
            SourceRange::new(at, at),
            "nominal type declaration requires a braced body",
        )));
        return;
    }

    parser.start(SyntaxKind::DelimitedGroup, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let end = token_count(parser);
    let close = find_matching_close(parser, parser.cursor(), "{").unwrap_or(end);
    parser.start(SyntaxKind::FieldList, SyntaxRole::Element(0));
    emit_fields(parser, close, item_kind);
    bump_until(parser, close);
    parser.finish();
    if parser.at("}") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.nominal.missing_body_close",
        );
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.nominal.missing_body_close",
            SourceRange::new(at, at),
            "missing closing `}` for nominal type declaration",
        )));
    }
    parser.finish();
}

fn emit_fields(parser: &mut DocumentParser<'_, '_>, close: usize, item_kind: SyntaxKind) {
    let mut ordinal = 0_u16;
    while parser.cursor() < close {
        bump_nominal_member_leading_trivia(parser);
        if parser.cursor() >= close {
            break;
        }
        if parser.at(",") {
            parser.bump();
            continue;
        }
        parser.start(SyntaxKind::RecordField, SyntaxRole::Field(ordinal));
        emit_outer_prefixes(parser);
        parser.bump_trivia();
        if parser.cursor() >= close {
            emit_missing_field_name(parser, "field requires an ordinary name");
            if item_kind == SyntaxKind::StructItem {
                emit_missing_field_type(parser, "field requires `: Type`");
            }
            parser.finish();
            break;
        }
        let end = field_boundary(parser, parser.cursor(), close);
        if item_kind == SyntaxKind::StructItem {
            emit_named_field(parser, end, ordinal);
        } else {
            emit_enum_variant(parser, end, ordinal);
        }
        bump_until(parser, end);
        if parser.at(",") {
            parser.bump();
        }
        parser.finish();
        ordinal = ordinal.saturating_add(1);
    }
}

fn bump_nominal_member_leading_trivia(parser: &mut DocumentParser<'_, '_>) {
    while matches!(
        parser.current_kind(),
        Some(SyntaxKind::WhitespaceToken | SyntaxKind::NewlineToken | SyntaxKind::CommentToken)
    ) {
        parser.bump();
    }
}

fn emit_enum_variant(parser: &mut DocumentParser<'_, '_>, end: usize, _ordinal: u16) {
    let significant_end = trimmed_end(parser, parser.cursor(), end);
    let Some(name) = first_significant(parser, parser.cursor(), significant_end) else {
        emit_missing_field_name(parser, "enum variant requires an ordinary name");
        return;
    };
    bump_until(parser, name);
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
    } else {
        emit_missing_field_name(parser, "enum variant requires an ordinary name");
    }
    parser.bump_trivia();

    if parser.cursor() < significant_end {
        emit_type(parser, significant_end, SyntaxRole::Type);
    }
    bump_until(parser, significant_end);
}

fn emit_named_field(parser: &mut DocumentParser<'_, '_>, end: usize, _ordinal: u16) {
    let significant_end = trimmed_end(parser, parser.cursor(), end);
    let Some(name) = first_significant(parser, parser.cursor(), significant_end) else {
        emit_missing_field_name(parser, "field requires an ordinary name");
        emit_missing_field_type(parser, "field requires `: Type`");
        return;
    };
    bump_until(parser, name);
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
    } else {
        emit_missing_field_name(parser, "field requires an ordinary name");
    }
    parser.bump_trivia();

    let colon = find_top_level_boundary(parser, parser.cursor(), end, &[":"]);
    if colon < significant_end && token_text(parser, colon) == Some(":") {
        bump_until(parser, colon);
        parser.start(SyntaxKind::ColonNode, SyntaxRole::Colon);
        parser.bump();
        parser.finish();
        parser.bump_trivia();
        emit_type(parser, significant_end, SyntaxRole::Type);
    } else {
        emit_missing_delimiter(parser, SyntaxKind::ColonNode, SyntaxRole::Colon);
        emit_missing_field_type(parser, "field requires `: Type`");
        if parser.cursor() < significant_end {
            parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
            bump_until(parser, significant_end);
            parser.finish();
        }
    }
    bump_until(parser, significant_end);
}

fn emit_missing_field_name(parser: &mut DocumentParser<'_, '_>, message: &'static str) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.nominal.missing_field_name",
        SourceRange::new(at, at),
        message,
    )));
}

fn emit_missing_field_type(parser: &mut DocumentParser<'_, '_>, message: &'static str) {
    let at = parser.current_offset();
    emit_type(parser, parser.cursor(), SyntaxRole::Type);
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.nominal.missing_field_type",
        SourceRange::new(at, at),
        message,
    )));
}

fn field_boundary(parser: &DocumentParser<'_, '_>, start: usize, end: usize) -> usize {
    let mut depth = 0_usize;
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            return index;
        };
        let text = parser.text_of(token);
        if depth == 0 && (text == "," || token.kind() == SyntaxKind::NewlineToken) {
            return index;
        }
        match text {
            "(" | "[" | "{" | "<" => depth += 1,
            ")" | "]" | "}" | ">" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    end
}
