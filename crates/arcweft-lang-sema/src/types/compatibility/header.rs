//! Scope-independent type headers shared by preparation and final checking.

use crate::types::{EntityType, TypeEffectDomain};

impl<E: TypeEffectDomain> EntityType<E> {
    /// A family expectation can admit a payload specialization. Structural
    /// comparison requires the same payload presence as well as entity kind.
    pub(in crate::types) fn accepts_header(&self, actual: &Self, structural: bool) -> bool {
        self.kind() == actual.kind()
            && (self.value().is_some() == actual.value().is_some()
                || (!structural && self.value().is_none()))
    }
}
