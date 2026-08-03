//! Inclusive syntax transaction limits.

/// Syntax allocation family whose inclusive hard limit was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxLimit {
    PrefixDepth,
    StyleNestingDepth,
    AssertionConditions,
    PredicateParameters,
    ProofParameters,
    ContractClauses,
    GenericParameters,
    WherePredicates,
    FixedParameters,
    DeclarationMembers,
    ActivityPorts,
    MetricLabels,
    MetricBuckets,
    ViewExports,
    LayerMembers,
    TopLevelItems,
    Statements,
    Expressions,
    TypeNodes,
    PatternNodes,
    IdentityBearingNodes,
    Diagnostics,
}

impl SyntaxLimit {
    /// Inclusive hard maximum for the allocation family.
    pub const fn maximum(self) -> usize {
        match self {
            Self::PrefixDepth
            | Self::StyleNestingDepth
            | Self::AssertionConditions
            | Self::PredicateParameters
            | Self::ProofParameters
            | Self::ContractClauses
            | Self::MetricLabels
            | Self::LayerMembers => 64,
            Self::GenericParameters
            | Self::WherePredicates
            | Self::FixedParameters
            | Self::ActivityPorts
            | Self::ViewExports => 256,
            Self::DeclarationMembers | Self::MetricBuckets | Self::Diagnostics => 1_024,
            Self::TopLevelItems => 16_384,
            Self::Statements => 65_536,
            Self::Expressions => 262_144,
            Self::TypeNodes | Self::PatternNodes => 131_072,
            Self::IdentityBearingNodes => 1_048_576,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SyntaxLimit;

    #[test]
    fn syntax_hard_limits_match_the_language_contract() {
        assert_eq!(SyntaxLimit::PrefixDepth.maximum(), 64);
        assert_eq!(SyntaxLimit::StyleNestingDepth.maximum(), 64);
        assert_eq!(SyntaxLimit::AssertionConditions.maximum(), 64);
        assert_eq!(SyntaxLimit::PredicateParameters.maximum(), 64);
        assert_eq!(SyntaxLimit::ProofParameters.maximum(), 64);
        assert_eq!(SyntaxLimit::ContractClauses.maximum(), 64);
        assert_eq!(SyntaxLimit::GenericParameters.maximum(), 256);
        assert_eq!(SyntaxLimit::WherePredicates.maximum(), 256);
        assert_eq!(SyntaxLimit::FixedParameters.maximum(), 256);
        assert_eq!(SyntaxLimit::DeclarationMembers.maximum(), 1_024);
        assert_eq!(SyntaxLimit::ActivityPorts.maximum(), 256);
        assert_eq!(SyntaxLimit::MetricLabels.maximum(), 64);
        assert_eq!(SyntaxLimit::MetricBuckets.maximum(), 1_024);
        assert_eq!(SyntaxLimit::ViewExports.maximum(), 256);
        assert_eq!(SyntaxLimit::LayerMembers.maximum(), 64);
        assert_eq!(SyntaxLimit::TopLevelItems.maximum(), 16_384);
        assert_eq!(SyntaxLimit::Statements.maximum(), 65_536);
        assert_eq!(SyntaxLimit::Expressions.maximum(), 262_144);
        assert_eq!(SyntaxLimit::TypeNodes.maximum(), 131_072);
        assert_eq!(SyntaxLimit::PatternNodes.maximum(), 131_072);
        assert_eq!(SyntaxLimit::IdentityBearingNodes.maximum(), 1_048_576);
        assert_eq!(SyntaxLimit::Diagnostics.maximum(), 1_024);
    }
}
