//! Semantic assertion policy shared by checking and runtime-plan lowering.

use arcweft_lang_syntax::assertion::AssertionMode;

/// Compiler-selected assertion profile for one accepted project transaction.
///
/// This is distinct from a launch profile: it controls whether debug-only
/// assertion work is admitted into the executable semantic generation.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AssertionBuildProfile {
    /// Retain authored Debug assertions as runtime-only guards.
    #[default]
    Debug,
    /// Omit every Debug assertion runtime effect while retaining source HIR.
    Release,
}

/// Source context in which an assertion was authored.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssertionContext {
    OrdinaryBody,
    ProofBody,
    PredicateBody,
    ConstOrType,
}

/// Runtime guard emitted after semantic assertion checks.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AssertionRuntimePolicy {
    AlwaysGuard,
    DebugGuard,
}

impl AssertionContext {
    /// Returns whether a typed assertion mode is legal in this semantic context.
    pub const fn allows(self, mode: AssertionMode) -> bool {
        match self {
            Self::OrdinaryBody => true,
            Self::ProofBody => matches!(mode, AssertionMode::Prove),
            Self::PredicateBody | Self::ConstOrType => false,
        }
    }
}

impl AssertionBuildProfile {
    /// Stable spelling used by accepted build and artifact identities.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }

    /// Returns whether Debug assertions enter executable semantic facts.
    pub const fn retains_debug_assertions(self) -> bool {
        matches!(self, Self::Debug)
    }
}

#[cfg(test)]
mod tests {
    use super::{AssertionBuildProfile, AssertionContext};
    use arcweft_lang_syntax::assertion::AssertionMode;

    #[test]
    fn context_and_runtime_policy_are_owned_by_typed_assertion_boundaries() {
        assert!(AssertionContext::OrdinaryBody.allows(AssertionMode::Debug));
        assert!(AssertionContext::ProofBody.allows(AssertionMode::Prove));
        assert!(!AssertionContext::ProofBody.allows(AssertionMode::Check));
        assert!(!AssertionContext::PredicateBody.allows(AssertionMode::Prove));
        assert!(!AssertionContext::ConstOrType.allows(AssertionMode::Debug));

        assert!(AssertionBuildProfile::Debug.retains_debug_assertions());
        assert!(!AssertionBuildProfile::Release.retains_debug_assertions());
        assert_eq!(AssertionBuildProfile::Debug.as_str(), "debug");
        assert_eq!(AssertionBuildProfile::Release.as_str(), "release");
    }
}
