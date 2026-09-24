//! Canonical finite effect formulas and predicates.
//!
//! Semantic checking and executable type verification use the same immutable
//! membership algebra. The caller owns reference scopes and charges every
//! algebraic operation to its existing work budget; this module owns neither
//! inference variables nor application state.

mod decision;
mod membership;
mod set;

pub use decision::{DecisionControl, DecisionEncoding, DecisionWork};
pub use membership::{EffectCompletion, EffectFormula, EffectPredicate, MembershipEncoding};
pub use set::{EffectSet, EffectSetParseError};

/// Equality charges the same canonical graph traversal as construction and
/// projection. Local allocation identities never participate in equality.
struct EffectEqualityControl<'a, C>(&'a mut C);

impl<V, C: DecisionControl> DecisionEncoding<V> for EffectEqualityControl<'_, C> {
    type Error = C::Error;

    fn tag(&mut self, _: u8) -> Result<(), Self::Error> {
        self.0.charge(DecisionWork::Visit)
    }

    fn count(&mut self, _: usize) -> Result<(), Self::Error> {
        self.0.charge(DecisionWork::Visit)
    }

    fn variable(&mut self, _: &V) -> Result<(), Self::Error> {
        self.0.charge(DecisionWork::Visit)
    }
}

impl<V, C: DecisionControl> MembershipEncoding<V> for EffectEqualityControl<'_, C> {
    fn effect(&mut self, _: &arcweft_id::EffectId) -> Result<(), Self::Error> {
        self.0.charge(DecisionWork::Visit)
    }
}
