//! Shared path events over the private document cursor.

use crate::ast::symbol_path::ProjectSymbolSegment;

use super::cursor::DocumentParser;
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
    ExternalProjectSymbols,
    QualifiedOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PathComponent {
    end: usize,
    kind: PendingPathSegmentKind,
    source: arcweft_source::SourceRange,
}

impl PathComponent {
    pub(super) const fn end(self) -> usize {
        self.end
    }

    pub(super) const fn kind(self) -> PendingPathSegmentKind {
        self.kind
    }
}

pub(super) fn emit_path(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    separators: PathSeparatorGrammar,
) -> PendingPathProjection {
    let owner = parser.start_projected_owner(SyntaxKind::Path, role);
    let mut component_ordinal = 0_u32;
    let mut root = PendingPathRoot::ImplicitCrate;
    let mut root_open = true;
    let mut segments = Vec::new();
    while let Some(component) = path_component(parser, parser.cursor(), end, separators) {
        let token = parser.current().expect("path component token");
        let spelling = parser.current_text().expect("path component spelling");
        let initial_component_uses_module_root = component_ordinal == 0
            && match separators {
                PathSeparatorGrammar::DottedIdentifiers => false,
                PathSeparatorGrammar::DottedOrQualified
                | PathSeparatorGrammar::ExternalProjectSymbols => true,
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
            segments.push(PendingPathSegment::new(component.kind, component.source));
        }

        parser.start(
            SyntaxKind::PathSegment,
            SyntaxRole::Element(component_ordinal),
        );
        bump_until(parser, component.end);
        parser.finish();
        component_ordinal = component_ordinal.saturating_add(1);

        let Some((separator, _, spelling)) = parser.next_significant() else {
            break;
        };
        let admitted = match separators {
            PathSeparatorGrammar::DottedIdentifiers => spelling == ".",
            PathSeparatorGrammar::DottedOrQualified
            | PathSeparatorGrammar::ExternalProjectSymbols => {
                matches!(spelling, "." | "::")
            }
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
        emit_missing_path_segment(parser, &root, separators);
    }
    let projection = PendingPathProjection::new(root, segments);
    parser.set_path_projection(owner, projection.clone());
    parser.finish();
    projection
}

fn emit_missing_path_segment(
    parser: &mut DocumentParser<'_, '_>,
    root: &PendingPathRoot,
    separators: PathSeparatorGrammar,
) {
    let missing_at = match root {
        PendingPathRoot::ImplicitCrate if separators == PathSeparatorGrammar::DottedIdentifiers => {
            Some(parser.current_offset())
        }
        PendingPathRoot::ImplicitCrate => None,
        PendingPathRoot::Crate(source) | PendingPathRoot::SelfModule(source) => Some(source.end()),
        PendingPathRoot::Super(levels) => levels.last().map(|source| source.end()),
    };
    let Some(at) = missing_at else {
        return;
    };
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

fn next_segment(
    parser: &DocumentParser<'_, '_>,
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
    path_component(parser, index, end, grammar).map(|_| index)
}

pub(super) fn path_component(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    end: usize,
    grammar: PathSeparatorGrammar,
) -> Option<PathComponent> {
    let first = parser.token_at(start)?;
    let ordinary_kind = match grammar {
        PathSeparatorGrammar::DottedIdentifiers => (first.kind() == SyntaxKind::IdentifierToken)
            .then_some(PendingPathSegmentKind::Identifier)?,
        PathSeparatorGrammar::DottedOrQualified | PathSeparatorGrammar::QualifiedOnly => {
            match first.kind() {
                SyntaxKind::IdentifierToken => PendingPathSegmentKind::Identifier,
                SyntaxKind::KeywordToken => PendingPathSegmentKind::Keyword,
                SyntaxKind::LifetimeToken => PendingPathSegmentKind::Lifetime,
                _ => return None,
            }
        }
        PathSeparatorGrammar::ExternalProjectSymbols => match first.kind() {
            SyntaxKind::IdentifierToken => PendingPathSegmentKind::Identifier,
            SyntaxKind::KeywordToken => PendingPathSegmentKind::Keyword,
            SyntaxKind::LifetimeToken => PendingPathSegmentKind::Lifetime,
            SyntaxKind::NumberToken => PendingPathSegmentKind::ProjectSymbol,
            _ => return None,
        },
    };

    let mut component_end = start + 1;
    if grammar == PathSeparatorGrammar::ExternalProjectSymbols
        && matches!(
            first.kind(),
            SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken | SyntaxKind::NumberToken
        )
    {
        while component_end + 1 < end {
            let separator = parser.token_at(component_end)?;
            let next = parser.token_at(component_end + 1)?;
            if token_text(parser, component_end) != Some("-")
                || !matches!(
                    next.kind(),
                    SyntaxKind::IdentifierToken
                        | SyntaxKind::KeywordToken
                        | SyntaxKind::NumberToken
                )
                || parser
                    .token_at(component_end - 1)
                    .is_none_or(|previous| previous.range().end() != separator.range().start())
                || separator.range().end() != next.range().start()
            {
                break;
            }
            component_end += 2;
        }
    }

    let last = parser.token_at(component_end - 1)?;
    let source = arcweft_source::SourceRange::new(first.range().start(), last.range().end());
    let kind = if grammar == PathSeparatorGrammar::ExternalProjectSymbols
        && (component_end > start + 1 || first.kind() == SyntaxKind::NumberToken)
    {
        ProjectSymbolSegment::try_new(parser.source()[source.as_range()].to_owned()).ok()?;
        PendingPathSegmentKind::ProjectSymbol
    } else {
        ordinary_kind
    };
    Some(PathComponent {
        end: component_end,
        kind,
        source,
    })
}
