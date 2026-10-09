//! Typed lexical and effect roles used by the private executable row encoder.
//! The admitted canonical membership graph supplies its own ordered grammar;
//! no generic serialization, source spelling, or new quota is introduced.

use super::{RuntimeArrayLength, RuntimeFunctionTypeContract, RuntimeTypeBinder, RuntimeTypeScope};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticEncodingError};

impl RuntimeTypeBinder {
    pub(crate) fn encode_semantic_roles(self, encoder: &mut TaskSemanticEncoder<'_>) {
        encoder.ordinal(u32::from(self.types()));
        encoder.ordinal(u32::from(self.const_lengths()));
        encoder.ordinal(self.effects());
    }
}

impl RuntimeTypeScope {
    pub(crate) fn try_visit_semantic_child_counts<E>(
        &self,
        visitor: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(self.binders().len())
    }

    pub(crate) fn encode_semantic_scope(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
    ) -> Result<(), TaskSemanticEncodingError> {
        encoder.status()?;
        encoder.count(self.binders().len());
        for binder in self.binders() {
            encoder.enter_element();
            encoder.status()?;
            binder.encode_semantic_roles(encoder);
            encoder.status()?;
        }
        encoder.status()
    }
}

impl RuntimeArrayLength {
    pub(crate) fn encode_semantic_length(&self, encoder: &mut TaskSemanticEncoder<'_>) {
        match self {
            Self::Constant(length) => {
                encoder.tag(0);
                encoder.scalar_u64(*length);
            }
            Self::Bound(reference) => {
                encoder.tag(1);
                encoder.ordinal(reference.depth());
                encoder.ordinal(u32::from(reference.slot()));
            }
        }
    }
}

impl RuntimeFunctionTypeContract {
    pub(crate) fn try_visit_semantic_child_counts<E>(
        &self,
        visitor: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        self.predicate().try_visit_semantic_child_counts(visitor)?;
        self.invocation().try_visit_semantic_child_counts(visitor)
    }

    pub(crate) fn encode_semantic_contract(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
    ) -> Result<(), TaskSemanticEncodingError> {
        encoder.status()?;
        self.binder().encode_semantic_roles(encoder);
        self.predicate().encode(encoder)?;
        self.invocation().encode(encoder)?;
        encoder.status()
    }
}

#[cfg(test)]
mod tests;
