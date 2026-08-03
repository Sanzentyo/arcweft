//! Parser-owned structural projection for View part exports.

use arcweft_source::SourceRange;

/// Authored required keyword or its exact zero-width insertion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingViewRequiredKeyword {
    Authored(SourceRange),
    Missing(SourceRange),
}

impl PendingViewRequiredKeyword {
    pub(crate) const fn source(self) -> SourceRange {
        match self {
            Self::Authored(source) | Self::Missing(source) => source,
        }
    }

    pub(crate) const fn is_missing(self) -> bool {
        matches!(self, Self::Missing(_))
    }

    fn rebased(self, offset: usize) -> Option<Self> {
        let source = rebase_range(self.source(), offset)?;
        Some(match self {
            Self::Authored(_) => Self::Authored(source),
            Self::Missing(_) => Self::Missing(source),
        })
    }
}

/// Structural state fixed while the View export grammar owns the token cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingViewExportProjection {
    part: PendingViewRequiredKeyword,
    alias: PendingViewRequiredKeyword,
    misplaced: bool,
}

impl PendingViewExportProjection {
    pub(crate) const fn new(
        part: PendingViewRequiredKeyword,
        alias: PendingViewRequiredKeyword,
        misplaced: bool,
    ) -> Self {
        Self {
            part,
            alias,
            misplaced,
        }
    }

    pub(crate) const fn part(&self) -> PendingViewRequiredKeyword {
        self.part
    }

    pub(crate) const fn alias(&self) -> PendingViewRequiredKeyword {
        self.alias
    }

    pub(crate) const fn is_misplaced(&self) -> bool {
        self.misplaced
    }

    pub(crate) const fn has_recovery(&self) -> bool {
        self.part.is_missing() || self.alias.is_missing() || self.misplaced
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self::new(
            self.part.rebased(offset)?,
            self.alias.rebased(offset)?,
            self.misplaced,
        ))
    }
}

fn rebase_range(range: SourceRange, offset: usize) -> Option<SourceRange> {
    Some(SourceRange::new(
        range.start().checked_add(offset)?,
        range.end().checked_add(offset)?,
    ))
}
