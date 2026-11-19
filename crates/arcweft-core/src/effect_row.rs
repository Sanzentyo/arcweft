//! Canonical finite effect formulas and predicates.
//!
//! Semantic checking and executable type verification use the same immutable
//! membership algebra. The caller owns reference scopes and charges every
//! algebraic operation to its existing work budget; this module owns neither
//! inference variables nor application state.

mod decision;
mod declaration;
mod membership;
mod set;

pub use decision::{
    ConditionalDecisionCompletion, DecisionControl, DecisionEncoding, DecisionLowerEnvelope,
    DecisionWork, EffectDecision, EffectDecisionReplacement,
};
pub use declaration::{
    EffectDecisionDeclaration, EffectDecisionNodeDeclaration, EffectDecisionTarget,
    EffectDeclarationError, EffectMembershipDeclaration,
};
pub use membership::{
    EffectCompletion, EffectFormula, EffectFormulaBuilder, EffectMembershipView, EffectPredicate,
    MembershipEncoding,
};
pub use set::{EffectSet, EffectSetParseError};
