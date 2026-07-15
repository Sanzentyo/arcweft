//! Semantic assertion policy shared by checking and runtime-plan lowering.

use arcweft_lang_hir::identity::{ExprId, StmtId};
use arcweft_lang_syntax::assertion::{AssertionFactClass, AssertionMode};

/// Source context in which an assertion was authored.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssertionContext {
    OrdinaryBody,
    ProofBody,
    PredicateBody,
    ConstOrType,
}

/// Runtime guard emitted after semantic assertion checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssertionRuntimePolicy {
    None,
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

impl AssertionRuntimePolicy {
    /// Derives runtime intent from the syntax-owned mode.
    pub const fn for_mode(mode: AssertionMode) -> Self {
        match mode {
            AssertionMode::Prove => Self::None,
            AssertionMode::Check => Self::AlwaysGuard,
            AssertionMode::Debug => Self::DebugGuard,
        }
    }
}

/// Fully typed assertion ready for fact and runtime-plan lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedAssertion {
    stmt: StmtId,
    mode: AssertionMode,
    conditions: Box<[ExprId]>,
    runtime: AssertionRuntimePolicy,
    fact_class: AssertionFactClass,
}

impl CheckedAssertion {
    /// Returns the statement identity used by faults and diagnostics.
    pub const fn stmt(&self) -> StmtId {
        self.stmt
    }

    /// Returns the selected source mode.
    pub const fn mode(&self) -> AssertionMode {
        self.mode
    }

    /// Returns condition identities in short-circuit evaluation order.
    pub const fn conditions(&self) -> &[ExprId] {
        &self.conditions
    }

    /// Returns the runtime guard policy.
    pub const fn runtime(&self) -> AssertionRuntimePolicy {
        self.runtime
    }

    /// Returns the release or debug-only fact domain.
    pub const fn fact_class(&self) -> AssertionFactClass {
        self.fact_class
    }
}

#[cfg(test)]
mod tests {
    use super::{AssertionContext, AssertionRuntimePolicy};
    use arcweft_lang_syntax::assertion::AssertionMode;

    #[test]
    fn context_and_runtime_policy_are_owned_by_typed_assertion_boundaries() {
        assert!(AssertionContext::OrdinaryBody.allows(AssertionMode::Debug));
        assert!(AssertionContext::ProofBody.allows(AssertionMode::Prove));
        assert!(!AssertionContext::ProofBody.allows(AssertionMode::Check));
        assert!(!AssertionContext::PredicateBody.allows(AssertionMode::Prove));
        assert!(!AssertionContext::ConstOrType.allows(AssertionMode::Debug));

        assert_eq!(
            AssertionRuntimePolicy::for_mode(AssertionMode::Prove),
            AssertionRuntimePolicy::None
        );
        assert_eq!(
            AssertionRuntimePolicy::for_mode(AssertionMode::Check),
            AssertionRuntimePolicy::AlwaysGuard
        );
        assert_eq!(
            AssertionRuntimePolicy::for_mode(AssertionMode::Debug),
            AssertionRuntimePolicy::DebugGuard
        );
    }
}
