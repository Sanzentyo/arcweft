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

/// Parser-owned local-name state for one View `.part(...)` modifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingViewPartLocalName {
    Present(SourceRange),
    Missing(SourceRange),
    Invalid(SourceRange),
}

impl PendingViewPartLocalName {
    pub(crate) const fn source(self) -> SourceRange {
        match self {
            Self::Present(source) | Self::Missing(source) | Self::Invalid(source) => source,
        }
    }

    pub(crate) const fn has_recovery(self) -> bool {
        !matches!(self, Self::Present(_))
    }

    fn rebased(self, offset: usize) -> Option<Self> {
        let source = rebase_range(self.source(), offset)?;
        Some(match self {
            Self::Present(_) => Self::Present(source),
            Self::Missing(_) => Self::Missing(source),
            Self::Invalid(_) => Self::Invalid(source),
        })
    }
}

/// Exact parser-selected source roles for one View `.part(...)` modifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PendingViewPartModifierProjection {
    whole: SourceRange,
    dot: SourceRange,
    name: SourceRange,
    open: SourceRange,
    local_name: PendingViewPartLocalName,
    close: Option<SourceRange>,
}

impl PendingViewPartModifierProjection {
    pub(crate) const fn new(
        whole: SourceRange,
        dot: SourceRange,
        name: SourceRange,
        open: SourceRange,
        local_name: PendingViewPartLocalName,
        close: Option<SourceRange>,
    ) -> Self {
        Self {
            whole,
            dot,
            name,
            open,
            local_name,
            close,
        }
    }

    pub(crate) const fn whole(self) -> SourceRange {
        self.whole
    }

    pub(crate) const fn dot(self) -> SourceRange {
        self.dot
    }

    pub(crate) const fn name(self) -> SourceRange {
        self.name
    }

    pub(crate) const fn open(self) -> SourceRange {
        self.open
    }

    pub(crate) const fn local_name(self) -> PendingViewPartLocalName {
        self.local_name
    }

    pub(crate) const fn close(self) -> Option<SourceRange> {
        self.close
    }

    pub(crate) const fn has_recovery(self) -> bool {
        self.local_name.has_recovery() || self.close.is_none()
    }

    fn ranges_are_valid_for(self, owner: SourceRange) -> bool {
        range_contains(owner, self.whole)
            && range_contains(self.whole, self.dot)
            && range_contains(self.whole, self.name)
            && range_contains(self.whole, self.open)
            && range_contains(self.whole, self.local_name.source())
            && self
                .close
                .is_none_or(|close| range_contains(self.whole, close))
            && self.whole.start() == self.dot.start()
            && self.dot.end() <= self.name.start()
            && self.name.end() <= self.open.start()
            && self.open.end() <= self.local_name.source().start()
            && self.close.is_none_or(|close| {
                self.local_name.source().end() <= close.start() && close.end() == self.whole.end()
            })
    }

    fn rebased(self, offset: usize) -> Option<Self> {
        Some(Self::new(
            rebase_range(self.whole, offset)?,
            rebase_range(self.dot, offset)?,
            rebase_range(self.name, offset)?,
            rebase_range(self.open, offset)?,
            self.local_name.rebased(offset)?,
            match self.close {
                Some(close) => Some(rebase_range(close, offset)?),
                None => None,
            },
        ))
    }
}

/// Parser-owned modifier inventory for one attached View fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingViewFragmentProjection {
    part_modifiers: Box<[PendingViewPartModifierProjection]>,
}

impl PendingViewFragmentProjection {
    pub(crate) fn new(part_modifiers: Box<[PendingViewPartModifierProjection]>) -> Self {
        Self { part_modifiers }
    }

    pub(crate) const fn part_modifiers(&self) -> &[PendingViewPartModifierProjection] {
        &self.part_modifiers
    }

    pub(crate) fn has_recovery(&self) -> bool {
        self.part_modifiers
            .iter()
            .any(|modifier| modifier.has_recovery())
    }

    pub(crate) fn ranges_are_valid_for(&self, owner: SourceRange) -> bool {
        self.part_modifiers
            .iter()
            .all(|modifier| modifier.ranges_are_valid_for(owner))
            && self
                .part_modifiers
                .windows(2)
                .all(|pair| pair[0].whole().end() <= pair[1].whole().start())
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        self.part_modifiers
            .iter()
            .copied()
            .map(|modifier| modifier.rebased(offset))
            .collect::<Option<Vec<_>>>()
            .map(Vec::into_boxed_slice)
            .map(Self::new)
    }
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

const fn range_contains(owner: SourceRange, child: SourceRange) -> bool {
    owner.start() <= child.start() && child.end() <= owner.end()
}
