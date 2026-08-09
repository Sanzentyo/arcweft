//! Parser-owned source projection for one Flow contract clause.
//!
//! Contract keywords and closed mode words are tokens rather than semantic
//! name nodes.  The parser records their exact ranges here so attached syntax
//! can retain revision-bound source evidence without scanning source text.

use arcweft_source::SourceRange;

use super::kinds::SyntaxKind;

/// Closed mode vocabulary admitted by Flow condition clauses.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum PendingFlowContractMode {
    Default,
    Prove(SourceRange),
    Check(SourceRange),
    Debug(SourceRange),
}

impl PendingFlowContractMode {
    pub(crate) const fn source(self) -> Option<SourceRange> {
        match self {
            Self::Default => None,
            Self::Prove(source) | Self::Check(source) | Self::Debug(source) => Some(source),
        }
    }

    fn rebased(self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Default => Self::Default,
            Self::Prove(source) => Self::Prove(rebase_range(source, offset)?),
            Self::Check(source) => Self::Check(rebase_range(source, offset)?),
            Self::Debug(source) => Self::Debug(rebase_range(source, offset)?),
        })
    }
}

/// Exact keyword and mode ranges selected for one typed Flow contract node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingFlowContractClauseProjection {
    clause_keyword: SourceRange,
    mode: PendingFlowContractMode,
    no_effect_keyword: Option<SourceRange>,
}

impl PendingFlowContractClauseProjection {
    pub(crate) const fn new(
        clause_keyword: SourceRange,
        mode: PendingFlowContractMode,
        no_effect_keyword: Option<SourceRange>,
    ) -> Self {
        Self {
            clause_keyword,
            mode,
            no_effect_keyword,
        }
    }

    pub(crate) const fn clause_keyword(&self) -> SourceRange {
        self.clause_keyword
    }

    pub(crate) const fn mode(&self) -> PendingFlowContractMode {
        self.mode
    }

    pub(crate) const fn no_effect_keyword(&self) -> Option<SourceRange> {
        self.no_effect_keyword
    }

    pub(crate) const fn accepts_kind(&self, kind: SyntaxKind) -> bool {
        match kind {
            SyntaxKind::RequiresClause
            | SyntaxKind::EnsuresClause
            | SyntaxKind::InvariantClause => self.no_effect_keyword.is_none(),
            SyntaxKind::NoEffectClause => {
                matches!(self.mode, PendingFlowContractMode::Default)
                    && self.no_effect_keyword.is_some()
            }
            SyntaxKind::AssumeClause
            | SyntaxKind::ReadsClause
            | SyntaxKind::EffectsClause
            | SyntaxKind::ModifiesClause
            | SyntaxKind::DecreasesClause => {
                matches!(self.mode, PendingFlowContractMode::Default)
                    && self.no_effect_keyword.is_none()
            }
            _ => false,
        }
    }

    pub(crate) fn ranges_are_valid_for(&self, kind: SyntaxKind, owner: SourceRange) -> bool {
        if !self.accepts_kind(kind) || !token_belongs_to(owner, self.clause_keyword) {
            return false;
        }
        let mut previous_end = self.clause_keyword.end();
        if let Some(mode) = self.mode.source() {
            if !token_belongs_to(owner, mode) || mode.start() < previous_end {
                return false;
            }
            previous_end = mode.end();
        }
        if let Some(no_effect) = self.no_effect_keyword
            && (!token_belongs_to(owner, no_effect) || no_effect.start() < previous_end)
        {
            return false;
        }
        true
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            clause_keyword: rebase_range(self.clause_keyword, offset)?,
            mode: self.mode.rebased(offset)?,
            no_effect_keyword: match self.no_effect_keyword {
                Some(source) => Some(rebase_range(source, offset)?),
                None => None,
            },
        })
    }
}

fn token_belongs_to(owner: SourceRange, token: SourceRange) -> bool {
    token.start() < token.end() && owner.start() <= token.start() && token.end() <= owner.end()
}

fn rebase_range(range: SourceRange, offset: usize) -> Option<SourceRange> {
    Some(SourceRange::new(
        range.start().checked_add(offset)?,
        range.end().checked_add(offset)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::{PendingFlowContractClauseProjection, PendingFlowContractMode};
    use crate::grammar::kinds::SyntaxKind;
    use arcweft_source::SourceRange;

    #[test]
    fn projection_keeps_no_effect_keywords_distinct_from_modes() {
        let projection = PendingFlowContractClauseProjection::new(
            SourceRange::new(10, 17),
            PendingFlowContractMode::Default,
            Some(SourceRange::new(18, 27)),
        );
        assert!(
            projection.ranges_are_valid_for(SyntaxKind::NoEffectClause, SourceRange::new(10, 42))
        );
        assert!(!projection.accepts_kind(SyntaxKind::EnsuresClause));
    }
}
