//! Shared path events over the private document cursor.

use super::cursor::ShadowDocumentParser;
use super::shadow_recovery::{bump_until, expected, first_significant, token_text};
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::grammar::source_projection::{
    PendingPathProjection, PendingPathRoot, PendingPathSegment, PendingPathSegmentKind,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PathSeparatorGrammar {
    DottedIdentifiers,
    DottedOrQualified,
    QualifiedOnly,
}

pub(super) fn emit_path(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    separators: PathSeparatorGrammar,
) {
    let owner = parser.start_projected_owner(SyntaxKind::Path, role);
    let mut component_ordinal = 0_u32;
    let mut root = PendingPathRoot::ImplicitCrate;
    let mut root_open = true;
    let mut segments = Vec::new();
    loop {
        if parser.cursor() >= end || !accepts_component(separators, parser.current_kind()) {
            break;
        }

        let token = parser.current().expect("path component token");
        let spelling = parser.current_text().expect("path component spelling");
        let initial_component_uses_module_root = component_ordinal == 0
            && match separators {
                PathSeparatorGrammar::DottedIdentifiers => false,
                PathSeparatorGrammar::DottedOrQualified => true,
                PathSeparatorGrammar::QualifiedOnly => {
                    // In expression grammar, `self.value` is value selection.
                    // Only an authored `::` opens explicit module-root syntax.
                    first_significant(parser, parser.cursor().saturating_add(1), end)
                        .and_then(|separator| token_text(parser, separator))
                        == Some("::")
                }
            };
        if initial_component_uses_module_root && spelling == "crate" {
            root = PendingPathRoot::Crate(token.range());
            root_open = false;
        } else if initial_component_uses_module_root && spelling == "self" {
            root = PendingPathRoot::SelfModule(token.range());
            root_open = false;
        } else if initial_component_uses_module_root && spelling == "parent" {
            root = PendingPathRoot::Super(vec![token.range()].into_boxed_slice());
            root_open = false;
        } else if root_open
            && spelling == "super"
            && (component_ordinal > 0 || initial_component_uses_module_root)
        {
            let mut levels = match root {
                PendingPathRoot::Super(levels) => levels.into_vec(),
                PendingPathRoot::ImplicitCrate => Vec::new(),
                PendingPathRoot::Crate(_) | PendingPathRoot::SelfModule(_) => {
                    unreachable!("explicit roots close root recognition")
                }
            };
            levels.push(token.range());
            root = PendingPathRoot::Super(levels.into_boxed_slice());
        } else {
            root_open = false;
            let kind = match token.kind() {
                SyntaxKind::IdentifierToken => PendingPathSegmentKind::Identifier,
                SyntaxKind::KeywordToken => PendingPathSegmentKind::Keyword,
                SyntaxKind::LifetimeToken => PendingPathSegmentKind::Lifetime,
                _ => unreachable!("path components are preflighted by token kind"),
            };
            segments.push(PendingPathSegment::new(kind, token.range()));
        }

        parser.start(
            SyntaxKind::PathSegment,
            SyntaxRole::Element(component_ordinal),
        );
        parser.bump();
        parser.finish();
        component_ordinal = component_ordinal.saturating_add(1);

        let Some((separator, _, spelling)) = parser.next_significant() else {
            break;
        };
        let admitted = match separators {
            PathSeparatorGrammar::DottedIdentifiers => spelling == ".",
            PathSeparatorGrammar::DottedOrQualified => matches!(spelling, "." | "::"),
            PathSeparatorGrammar::QualifiedOnly => spelling == "::",
        };
        if separator >= end || !admitted {
            break;
        }
        let Some(next) = next_segment(parser, separator + 1, end, separators) else {
            break;
        };
        bump_until(parser, separator);
        parser.bump();
        bump_until(parser, next);
    }
    if segments.is_empty() {
        let missing_at = match &root {
            PendingPathRoot::ImplicitCrate
                if separators == PathSeparatorGrammar::DottedIdentifiers =>
            {
                Some(parser.current_offset())
            }
            PendingPathRoot::ImplicitCrate => None,
            PendingPathRoot::Crate(source) | PendingPathRoot::SelfModule(source) => {
                Some(source.end())
            }
            PendingPathRoot::Super(levels) => levels.last().map(|source| source.end()),
        };
        if let Some(at) = missing_at {
            parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
            parser.push(SyntaxEvent::MissingToken {
                expected: expected(SyntaxKind::IdentifierToken),
                at,
            });
            parser.finish();
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.path.missing_segment",
                arcweft_source::SourceRange::new(at, at),
                "a path requires an identifier segment",
            )));
        }
    }
    parser.set_path_projection(owner, PendingPathProjection::new(root, segments));
    parser.finish();
}

fn next_segment(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
    grammar: PathSeparatorGrammar,
) -> Option<usize> {
    let index = (start..end).find(|index| {
        parser.token_at(*index).is_some_and(|token| {
            !matches!(
                token.kind(),
                SyntaxKind::WhitespaceToken
                    | SyntaxKind::NewlineToken
                    | SyntaxKind::CommentToken
                    | SyntaxKind::DocCommentToken
            )
        })
    })?;
    accepts_component(grammar, parser.token_at(index).map(|token| token.kind())).then_some(index)
}

fn accepts_component(grammar: PathSeparatorGrammar, kind: Option<SyntaxKind>) -> bool {
    match grammar {
        PathSeparatorGrammar::DottedIdentifiers => kind == Some(SyntaxKind::IdentifierToken),
        PathSeparatorGrammar::DottedOrQualified | PathSeparatorGrammar::QualifiedOnly => matches!(
            kind,
            Some(
                SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken | SyntaxKind::LifetimeToken
            )
        ),
    }
}
