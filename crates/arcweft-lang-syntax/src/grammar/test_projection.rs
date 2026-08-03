//! Parser-owned adapter-kind projection for `test` declarations.

use arcweft_source::SourceRange;

use crate::name::SyntaxName;

/// Closed built-in test adapter vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum KnownTestKind {
    Scenario,
    Visual,
    Audio,
    Fixture,
}

/// Parser-selected adapter kind for one `test` declaration.
///
/// The projection retains validated names so attachment and HIR lowering never
/// reopen source text to rediscover built-in or custom adapter semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingTestKindProjection {
    Known {
        value: KnownTestKind,
        source: SourceRange,
    },
    Custom {
        value: SyntaxName,
        source: SourceRange,
    },
    Missing {
        insertion: SourceRange,
    },
}

impl PendingTestKindProjection {
    pub(crate) const fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing { .. })
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Known { value, source } => Self::Known {
                value: *value,
                source: rebase_range(*source, offset)?,
            },
            Self::Custom { value, source } => Self::Custom {
                value: value.clone(),
                source: rebase_range(*source, offset)?,
            },
            Self::Missing { insertion } => Self::Missing {
                insertion: rebase_range(*insertion, offset)?,
            },
        })
    }
}

fn rebase_range(range: SourceRange, offset: usize) -> Option<SourceRange> {
    Some(SourceRange::new(
        range.start().checked_add(offset)?,
        range.end().checked_add(offset)?,
    ))
}
