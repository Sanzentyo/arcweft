//! Closed runtime-plan type families.

use crate::pattern::RuntimeCheckedType;

/// Top-level execution family for a semantic type outside the closed checked
/// value algebra.
///
/// The family deliberately carries no reconstructed generic arguments. Exact
/// semantic identity and normalized descendants remain higher-layer facts;
/// this core owner describes only the runtime representation selected for one
/// plan type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeOperationalType {
    Range,
    Iterator,
    Sequence,
    Tuple,
    Choice,
    Result,
    Option,
    Map,
    Need,
    Stream,
    Source,
    ThreadHandle,
    Shared,
    Reference,
    Function,
}

/// Final runtime representation selected for one normalized semantic type.
///
/// Checked types retain their complete structural predicate. Operational
/// types retain their closed top-level execution family without pretending
/// that the checked value algebra can represent their descendants.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimePlanTypeKind {
    Checked(RuntimeCheckedType),
    Operational(RuntimeOperationalType),
}
