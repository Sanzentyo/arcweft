//! Private pattern-family event classification over the shared cursor.

use super::document::ShadowDocumentParser;
use super::shadow_recovery::{bump_until, trimmed_end};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_pattern(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) {
    let end = trimmed_end(parser, parser.cursor(), end);
    let Some(first) = parser.current() else {
        parser.start(SyntaxKind::MissingPattern, role);
        parser.finish();
        return;
    };
    if parser.cursor() >= end {
        parser.start(SyntaxKind::MissingPattern, role);
        parser.finish();
        return;
    }
    let kind = match parser.text_of(first) {
        "_" => SyntaxKind::WildcardPattern,
        "mut" => SyntaxKind::MutableBindingPattern,
        "(" => SyntaxKind::TuplePattern,
        "[" => SyntaxKind::SequencePattern,
        "{" => SyntaxKind::RecordPattern,
        _ if first.kind() == SyntaxKind::EntityReferenceToken => SyntaxKind::EntityReferencePattern,
        _ if matches!(
            first.kind(),
            SyntaxKind::NumberToken
                | SyntaxKind::StringToken
                | SyntaxKind::RawStringToken
                | SyntaxKind::CharacterToken
        ) =>
        {
            SyntaxKind::LiteralPattern
        }
        _ => SyntaxKind::BindingPattern,
    };
    parser.start(kind, role);
    bump_until(parser, end);
    parser.finish();
}
