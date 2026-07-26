//! Private nested type-family events over the shared cursor.

use super::document::ShadowDocumentParser;
use super::path::emit_path;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_open_delimiter, find_matching_close,
    find_top_level_boundary, trimmed_end,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_type(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    let end = trimmed_end(parser, parser.cursor(), end);
    if parser.cursor() >= end {
        parser.start(SyntaxKind::MissingType, role);
        parser.finish();
        return;
    }

    if let Some(arrow) = boundary(parser, parser.cursor(), end, &["->"]) {
        emit_function_type(parser, arrow, end, role);
        return;
    }
    if boundary(parser, parser.cursor(), end, &["|"]).is_some() {
        emit_sum_type(parser, end, role);
        return;
    }

    match parser.current_text() {
        Some("&") => emit_reference_type(parser, end, role),
        Some("(") => emit_tuple_type(parser, end, role),
        Some("[") => emit_bracket_type(parser, end, role),
        Some("_") => emit_flat_type(parser, end, SyntaxKind::InferType, role),
        Some(spelling) if is_primitive_type(spelling) => {
            emit_flat_type(parser, end, SyntaxKind::PrimitiveType, role);
        }
        _ if parser.current_kind() == Some(SyntaxKind::LifetimeToken) => {
            emit_flat_type(parser, end, SyntaxKind::LifetimeType, role);
        }
        _ if generic_open(parser, parser.cursor(), end).is_some() => {
            emit_generic_type(parser, end, role);
        }
        _ if matches!(parser.current_kind(), Some(SyntaxKind::NumberToken)) => {
            emit_flat_type(parser, end, SyntaxKind::PrimitiveType, role);
        }
        _ => emit_path_type(parser, end, role),
    }
}

fn emit_function_type(
    parser: &mut ShadowDocumentParser<'_, '_>,
    arrow: usize,
    end: usize,
    role: SyntaxRole,
) {
    parser.start(SyntaxKind::FunctionType, role);
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    let parameter_end = trimmed_end(parser, parser.cursor(), arrow);
    if parser.at("(")
        && find_matching_close(parser, parser.cursor() + 1, "(")
            .is_some_and(|close| close + 1 == parameter_end)
    {
        emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
        emit_type_list(parser, parameter_end.saturating_sub(1), ")");
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseParenNode,
            ")",
            "syntax.type.missing_function_parameter_close",
        );
    } else {
        emit_type(parser, parameter_end, SyntaxRole::Element(0));
    }
    parser.finish();
    bump_until(parser, arrow);
    parser.bump();
    parser.bump_trivia();
    emit_type(parser, end, SyntaxRole::RightOperand);
    parser.finish();
}

fn emit_sum_type(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    parser.start(SyntaxKind::SumType, role);
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= end {
            break;
        }
        let alternative_end = find_top_level_boundary(parser, parser.cursor(), &["|"]).min(end);
        emit_type(parser, alternative_end, SyntaxRole::Element(ordinal));
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

fn emit_reference_type(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    parser.start(SyntaxKind::ReferenceType, role);
    parser.bump();
    parser.bump_trivia();
    if parser.current_kind() == Some(SyntaxKind::LifetimeToken) {
        parser.start(SyntaxKind::LifetimeType, SyntaxRole::Element(0));
        parser.bump();
        parser.finish();
        parser.bump_trivia();
    }
    if parser.at("mut") {
        parser.bump();
        parser.bump_trivia();
    }
    emit_type(parser, end, SyntaxRole::Operand);
    parser.finish();
}

fn emit_tuple_type(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    parser.start(SyntaxKind::TupleType, role);
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    let close = find_matching_close(parser, parser.cursor(), "(")
        .unwrap_or(end)
        .min(end);
    emit_type_list(parser, close, ")");
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.type.missing_tuple_close",
    );
    parser.finish();
}

fn emit_bracket_type(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    let close = find_matching_close(parser, parser.cursor() + 1, "[")
        .unwrap_or(end)
        .min(end);
    let semicolon = boundary(parser, parser.cursor() + 1, close, &[";"]);
    parser.start(
        if semicolon.is_some() {
            SyntaxKind::ArrayType
        } else {
            SyntaxKind::SliceType
        },
        role,
    );
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    let element_end = semicolon.unwrap_or(close);
    emit_type(parser, element_end, SyntaxRole::Element(0));
    bump_until(parser, element_end);
    if parser.at(";") {
        parser.bump();
        parser.bump_trivia();
        parser.start(SyntaxKind::TypeArgument, SyntaxRole::Element(1));
        emit_type(parser, close, SyntaxRole::Type);
        parser.finish();
        bump_until(parser, close);
    }
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.type.missing_bracket_close",
    );
    parser.finish();
}

fn emit_generic_type(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    let open = generic_open(parser, parser.cursor(), end).expect("classified generic type");
    parser.start(SyntaxKind::GenericApplicationType, role);
    emit_path(parser, open, SyntaxRole::Target);
    bump_until(parser, open);
    emit_open_delimiter(parser, SyntaxKind::OpenAngleNode, "<");
    let close = find_matching_close(parser, parser.cursor(), "<")
        .unwrap_or(end)
        .min(end);
    parser.start(SyntaxKind::ArgumentList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }
        let argument_end = find_top_level_boundary(parser, parser.cursor(), &[",", ">"]).min(close);
        parser.start(SyntaxKind::TypeArgument, SyntaxRole::Argument(ordinal));
        let equals = boundary(parser, parser.cursor(), argument_end, &["="]);
        if let Some(equals) = equals {
            parser.start(SyntaxKind::NameReference, SyntaxRole::Name);
            bump_until(parser, trimmed_end(parser, parser.cursor(), equals));
            parser.finish();
            bump_until(parser, equals);
            parser.bump();
            parser.bump_trivia();
            emit_type(parser, argument_end, SyntaxRole::Type);
        } else {
            emit_type(parser, argument_end, SyntaxRole::Type);
        }
        bump_until(parser, argument_end);
        parser.finish();
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
        SyntaxKind::CloseAngleNode,
        ">",
        "syntax.type.missing_generic_close",
    );
    parser.finish();
}

fn emit_path_type(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    parser.start(SyntaxKind::PathType, role);
    emit_path(parser, end, SyntaxRole::Target);
    bump_until(parser, end);
    parser.finish();
}

fn emit_type_list(parser: &mut ShadowDocumentParser<'_, '_>, close: usize, delimiter: &str) {
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at(delimiter) {
            break;
        }
        let element_end =
            find_top_level_boundary(parser, parser.cursor(), &[",", delimiter]).min(close);
        emit_type(parser, element_end, SyntaxRole::Element(ordinal));
        bump_until(parser, element_end);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
}

fn emit_flat_type(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    kind: SyntaxKind,
    role: SyntaxRole,
) {
    parser.start(kind, role);
    bump_until(parser, end);
    parser.finish();
}

fn generic_open(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> Option<usize> {
    boundary(parser, start, end, &["<"])
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

fn is_primitive_type(spelling: &str) -> bool {
    matches!(
        spelling,
        "Bool"
            | "Int"
            | "Float"
            | "String"
            | "Char"
            | "Unit"
            | "Never"
            | "!"
            | "U8"
            | "U16"
            | "U32"
            | "U64"
            | "I8"
            | "I16"
            | "I32"
            | "I64"
            | "F32"
            | "F64"
    )
}
