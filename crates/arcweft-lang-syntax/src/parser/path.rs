//! Shared path events over the private document cursor.

use super::document::ShadowDocumentParser;
use super::shadow_recovery::bump_until;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_path(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    parser.start(SyntaxKind::Path, role);
    let mut segment = 0_u32;
    loop {
        if parser.cursor() >= end
            || !matches!(
                parser.current_kind(),
                Some(
                    SyntaxKind::IdentifierToken
                        | SyntaxKind::KeywordToken
                        | SyntaxKind::LifetimeToken
                )
            )
        {
            break;
        }

        parser.start(SyntaxKind::PathSegment, SyntaxRole::Element(segment));
        parser.bump();
        parser.finish();
        segment = segment.saturating_add(1);

        let Some((separator, _, spelling)) = parser.next_significant() else {
            break;
        };
        if separator >= end || !matches!(spelling, "." | "::") {
            break;
        }
        let Some(next) = next_segment(parser, separator + 1, end) else {
            break;
        };
        bump_until(parser, separator);
        parser.bump();
        bump_until(parser, next);
    }
    parser.finish();
}

fn next_segment(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> Option<usize> {
    (start..end).find(|index| {
        parser.token_at(*index).is_some_and(|token| {
            matches!(
                token.kind(),
                SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken | SyntaxKind::LifetimeToken
            )
        })
    })
}
