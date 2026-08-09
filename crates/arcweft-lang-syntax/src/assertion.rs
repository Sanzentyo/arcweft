//! Shared syntax vocabulary for proof and runtime assertion statements.

/// Source assertion mode selected after `assert.`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AssertionMode {
    Prove,
    Check,
    Debug,
}

impl AssertionMode {
    /// Resolves one canonical mode keyword without duplicating string matches.
    pub fn from_keyword(keyword: &str) -> Option<Self> {
        match keyword {
            "prove" => Some(Self::Prove),
            "check" => Some(Self::Check),
            "debug" => Some(Self::Debug),
            _ => None,
        }
    }

    /// Canonical source keyword for this mode.
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Prove => "prove",
            Self::Check => "check",
            Self::Debug => "debug",
        }
    }

    /// Whether this mode always emits a runtime guard in release output.
    pub const fn has_release_runtime_instruction(self) -> bool {
        matches!(self, Self::Check)
    }

    /// Whether this mode can produce a runtime assertion guard.
    pub const fn is_runtime_capable(self) -> bool {
        matches!(self, Self::Check | Self::Debug)
    }

    /// Fact class produced by a successfully established assertion.
    pub const fn facts(self) -> AssertionFactClass {
        match self {
            Self::Prove | Self::Check => AssertionFactClass::Release,
            Self::Debug => AssertionFactClass::DebugOnly,
        }
    }
}

/// Safety domain in which assertion-derived facts are valid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssertionFactClass {
    Release,
    DebugOnly,
}

#[cfg(test)]
mod tests {
    use super::{AssertionFactClass, AssertionMode};

    #[test]
    fn assertion_modes_own_runtime_and_fact_policy() {
        assert_eq!(AssertionMode::Prove.keyword(), "prove");
        assert_eq!(
            AssertionMode::from_keyword("prove"),
            Some(AssertionMode::Prove)
        );
        assert_eq!(AssertionMode::from_keyword("other"), None);
        assert!(!AssertionMode::Prove.has_release_runtime_instruction());
        assert!(!AssertionMode::Prove.is_runtime_capable());
        assert_eq!(AssertionMode::Prove.facts(), AssertionFactClass::Release);

        assert_eq!(AssertionMode::Check.keyword(), "check");
        assert!(AssertionMode::Check.has_release_runtime_instruction());
        assert!(AssertionMode::Check.is_runtime_capable());
        assert_eq!(AssertionMode::Check.facts(), AssertionFactClass::Release);

        assert_eq!(AssertionMode::Debug.keyword(), "debug");
        assert!(!AssertionMode::Debug.has_release_runtime_instruction());
        assert!(AssertionMode::Debug.is_runtime_capable());
        assert_eq!(AssertionMode::Debug.facts(), AssertionFactClass::DebugOnly);
    }
}
