//! Inclusive syntax transaction limits.

/// Syntax allocation family whose inclusive hard limit was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxLimit {
    PrefixDepth,
    AssertionConditions,
    TopLevelItems,
    Statements,
    Expressions,
    TypeNodes,
    PatternNodes,
    Diagnostics,
}

impl SyntaxLimit {
    /// Inclusive hard maximum for the allocation family.
    pub const fn maximum(self) -> usize {
        match self {
            Self::PrefixDepth | Self::AssertionConditions => 64,
            Self::TopLevelItems => 16_384,
            Self::Statements => 65_536,
            Self::Expressions => 262_144,
            Self::TypeNodes | Self::PatternNodes => 131_072,
            Self::Diagnostics => 1_024,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SyntaxLimit;

    #[test]
    fn syntax_hard_limits_match_the_language_contract() {
        assert_eq!(SyntaxLimit::PrefixDepth.maximum(), 64);
        assert_eq!(SyntaxLimit::AssertionConditions.maximum(), 64);
        assert_eq!(SyntaxLimit::TopLevelItems.maximum(), 16_384);
        assert_eq!(SyntaxLimit::Statements.maximum(), 65_536);
        assert_eq!(SyntaxLimit::Expressions.maximum(), 262_144);
        assert_eq!(SyntaxLimit::TypeNodes.maximum(), 131_072);
        assert_eq!(SyntaxLimit::PatternNodes.maximum(), 131_072);
        assert_eq!(SyntaxLimit::Diagnostics.maximum(), 1_024);
    }
}
