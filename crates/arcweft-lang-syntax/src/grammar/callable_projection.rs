//! Parser-owned semantic projections for callable-only source forms.

use arcweft_source::SourceRange;

/// Closed semantic receiver mode selected by the method-parameter grammar.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MethodReceiverSyntaxKind {
    Owned,
    SharedReference,
    MutableReference,
}

/// Exact parser-owned source projection for one method receiver.
///
/// The variants make invalid marker combinations unrepresentable. In
/// particular, `mut self` remains an owned receiver whose binding Pattern owns
/// the `mut`, while `&mut self` owns a mutable-reference marker here and an
/// immutable binding Pattern for `self`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingMethodReceiverProjection {
    Owned {
        whole: SourceRange,
        mut_keyword: Option<SourceRange>,
        self_keyword: SourceRange,
    },
    SharedReference {
        whole: SourceRange,
        ampersand: SourceRange,
        self_keyword: SourceRange,
    },
    MutableReference {
        whole: SourceRange,
        ampersand: SourceRange,
        mut_keyword: SourceRange,
        self_keyword: SourceRange,
    },
}

impl PendingMethodReceiverProjection {
    pub(crate) const fn kind(&self) -> MethodReceiverSyntaxKind {
        match self {
            Self::Owned { .. } => MethodReceiverSyntaxKind::Owned,
            Self::SharedReference { .. } => MethodReceiverSyntaxKind::SharedReference,
            Self::MutableReference { .. } => MethodReceiverSyntaxKind::MutableReference,
        }
    }

    pub(crate) const fn whole(&self) -> SourceRange {
        match self {
            Self::Owned { whole, .. }
            | Self::SharedReference { whole, .. }
            | Self::MutableReference { whole, .. } => *whole,
        }
    }

    pub(crate) const fn ampersand(&self) -> Option<SourceRange> {
        match self {
            Self::Owned { .. } => None,
            Self::SharedReference { ampersand, .. } | Self::MutableReference { ampersand, .. } => {
                Some(*ampersand)
            }
        }
    }

    pub(crate) const fn mut_keyword(&self) -> Option<SourceRange> {
        match self {
            Self::Owned { mut_keyword, .. } => *mut_keyword,
            Self::SharedReference { .. } => None,
            Self::MutableReference { mut_keyword, .. } => Some(*mut_keyword),
        }
    }

    pub(crate) const fn self_keyword(&self) -> SourceRange {
        match self {
            Self::Owned { self_keyword, .. }
            | Self::SharedReference { self_keyword, .. }
            | Self::MutableReference { self_keyword, .. } => *self_keyword,
        }
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Owned {
                whole,
                mut_keyword,
                self_keyword,
            } => Self::Owned {
                whole: rebase_range(*whole, offset)?,
                mut_keyword: match mut_keyword {
                    Some(range) => Some(rebase_range(*range, offset)?),
                    None => None,
                },
                self_keyword: rebase_range(*self_keyword, offset)?,
            },
            Self::SharedReference {
                whole,
                ampersand,
                self_keyword,
            } => Self::SharedReference {
                whole: rebase_range(*whole, offset)?,
                ampersand: rebase_range(*ampersand, offset)?,
                self_keyword: rebase_range(*self_keyword, offset)?,
            },
            Self::MutableReference {
                whole,
                ampersand,
                mut_keyword,
                self_keyword,
            } => Self::MutableReference {
                whole: rebase_range(*whole, offset)?,
                ampersand: rebase_range(*ampersand, offset)?,
                mut_keyword: rebase_range(*mut_keyword, offset)?,
                self_keyword: rebase_range(*self_keyword, offset)?,
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
