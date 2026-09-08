//! Shared compiler limits for `#[fx] fn ... -> Fx` graph factories.

/// Deterministic limits for validation and compile-time graph expansion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FxExpansionLimits {
    pub max_depth: usize,
    pub max_visits: usize,
    pub max_nodes: usize,
}

/// Compiler-wide Fx graph resource limits.
pub const FX_EXPANSION_LIMITS: FxExpansionLimits = FxExpansionLimits {
    max_depth: 64,
    max_visits: 16_384,
    max_nodes: 4_096,
};
