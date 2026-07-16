//! Private type-family event classification over the shared cursor.

use super::document::ShadowDocumentParser;
use super::shadow_recovery::{bump_until, range_contains, trimmed_end};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_type(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    let end = trimmed_end(parser, parser.cursor(), end);
    let Some(first) = parser.current() else {
        parser.start(SyntaxKind::MissingType, role);
        parser.finish();
        return;
    };
    if parser.cursor() >= end {
        parser.start(SyntaxKind::MissingType, role);
        parser.finish();
        return;
    }
    let kind = match parser.text_of(first) {
        "&" => SyntaxKind::ReferenceType,
        "(" => SyntaxKind::TupleType,
        "[" if range_contains(parser, parser.cursor(), end, ";") => SyntaxKind::ArrayType,
        "[" => SyntaxKind::SliceType,
        "_" => SyntaxKind::InferType,
        spelling if is_primitive_type(spelling) => SyntaxKind::PrimitiveType,
        _ if first.kind() == SyntaxKind::LifetimeToken => SyntaxKind::LifetimeType,
        _ if range_contains(parser, parser.cursor(), end, "<") => {
            SyntaxKind::GenericApplicationType
        }
        _ => SyntaxKind::PathType,
    };
    parser.start(kind, role);
    bump_until(parser, end);
    parser.finish();
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
