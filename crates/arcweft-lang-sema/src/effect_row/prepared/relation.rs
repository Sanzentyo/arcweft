//! The mathematical relation contains no mutation or publication capability.
//! Live preparation and immutable completion read this same definition owner.

use super::*;

#[derive(Debug)]
pub(in crate::effect_row::prepared) struct PreparedEffectDefinitions {
    pub(in crate::effect_row::prepared) issuer: Arc<PreparedEffectIssuer>,
    pub(in crate::effect_row::prepared) choices: BTreeSet<u64>,
    pub(in crate::effect_row::prepared) functions: BTreeMap<u64, Arc<Function>>,
    pub(in crate::effect_row::prepared) obligations: Vec<Arc<ScopedObligation>>,
    pub(in crate::effect_row::prepared) rows: BTreeMap<u64, Arc<RowTerm>>,
    pub(in crate::effect_row::prepared) predicates: BTreeMap<u64, Arc<PredicateTerm>>,
}

impl PreparedEffectDefinitions {
    pub(super) fn reference(&self, ordinal: u64) -> PreparedEffectReference {
        PreparedEffectReference {
            issuer: Arc::clone(&self.issuer),
            ordinal,
        }
    }

    pub(super) fn validate_choice(
        &self,
        choice: &PreparedEffectChoice,
    ) -> Result<(), PreparedEffectInvariant> {
        self.read().validate_choice(choice)
    }

    pub(super) fn empty() -> Self {
        Self {
            issuer: Arc::new(PreparedEffectIssuer),
            choices: BTreeSet::new(),
            functions: BTreeMap::new(),
            obligations: Vec::new(),
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
        }
    }

    pub(super) fn validate(
        &self,
        reference: &PreparedEffectReference,
    ) -> Result<u64, PreparedEffectInvariant> {
        if !Arc::ptr_eq(&self.issuer, &reference.issuer) {
            return Err(PreparedEffectInvariant::ForeignIssuer);
        }
        Ok(reference.ordinal)
    }

    pub(super) fn function(
        &self,
        reference: &FunctionRef,
    ) -> Result<&Function, PreparedEffectInvariant> {
        self.read().function(reference)
    }

    pub(super) fn scheme(
        &self,
        reference: &PreparedEffectSchemeRef,
    ) -> Result<&Scheme, PreparedEffectInvariant> {
        self.read().scheme(reference)
    }

    pub(super) fn row(
        &self,
        reference: &PreparedEffectRowRef,
    ) -> Result<&RowTerm, PreparedEffectInvariant> {
        self.read().row(reference)
    }

    pub(super) fn predicate(
        &self,
        reference: &PreparedEffectPredicateRef,
    ) -> Result<&PredicateTerm, PreparedEffectInvariant> {
        self.read().predicate(reference)
    }
}
