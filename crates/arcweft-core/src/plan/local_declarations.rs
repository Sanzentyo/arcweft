//! Final typed plan-local declaration table.

use std::num::NonZeroU32;

use thiserror::Error;

use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};

/// One sealed plan-local declaration row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeLocalDeclaration {
    ty: RuntimePlanTypeId,
}

impl RuntimeLocalDeclaration {
    #[must_use]
    pub const fn ty(self) -> RuntimePlanTypeId {
        self.ty
    }
}

/// The complete typed local-declaration identity domain of one plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeLocalDeclarationTable {
    declarations: Box<[RuntimeLocalDeclaration]>,
}

impl RuntimeLocalDeclarationTable {
    #[must_use]
    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    #[must_use]
    pub fn contains(&self, local: RuntimeLocalDeclarationId) -> bool {
        self.get(local).is_some()
    }

    #[must_use]
    pub fn get(&self, local: RuntimeLocalDeclarationId) -> Option<RuntimeLocalDeclaration> {
        usize::try_from(local.get().get() - 1)
            .ok()
            .and_then(|index| self.declarations.get(index))
            .copied()
    }

    pub fn declarations(&self) -> impl ExactSizeIterator<Item = RuntimeLocalDeclaration> + '_ {
        self.declarations.iter().copied()
    }
}

/// Sole internal issuer for final typed local identities.
#[derive(Debug)]
pub(crate) struct RuntimeLocalDeclarationTableBuilder {
    declarations: Vec<RuntimeLocalDeclaration>,
    maximum: u32,
}

pub(crate) struct PreparedRuntimeLocalDeclarationBatch {
    ids: Box<[RuntimeLocalDeclarationId]>,
    declarations: Vec<RuntimeLocalDeclaration>,
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeLocalDeclarationTableError {
    #[error("runtime local-declaration identity space is exhausted")]
    IdentityExhausted,
}

impl RuntimeLocalDeclarationTableBuilder {
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            declarations: Vec::new(),
            maximum: u32::MAX,
        }
    }

    pub(crate) fn contains(&self, local: RuntimeLocalDeclarationId) -> bool {
        usize::try_from(local.get().get() - 1)
            .ok()
            .is_some_and(|index| index < self.declarations.len())
    }

    #[cfg(test)]
    pub(crate) fn push(
        &mut self,
        ty: RuntimePlanTypeId,
    ) -> Result<RuntimeLocalDeclarationId, RuntimeLocalDeclarationTableError> {
        let prepared = self.prepare_batch([ty])?;
        self.commit_batch(prepared)
            .first()
            .copied()
            .ok_or(RuntimeLocalDeclarationTableError::IdentityExhausted)
    }

    pub(crate) fn prepare_batch(
        &self,
        types: impl IntoIterator<Item = RuntimePlanTypeId>,
    ) -> Result<PreparedRuntimeLocalDeclarationBatch, RuntimeLocalDeclarationTableError> {
        let types = types.into_iter().collect::<Box<[_]>>();
        let final_len = self
            .declarations
            .len()
            .checked_add(types.len())
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value <= self.maximum)
            .ok_or(RuntimeLocalDeclarationTableError::IdentityExhausted)?;
        let _ = final_len;
        let mut declarations = self.declarations.clone();
        let mut ids = Vec::with_capacity(types.len());
        for ty in types {
            let ordinal = declarations
                .len()
                .checked_add(1)
                .and_then(|value| u32::try_from(value).ok())
                .and_then(NonZeroU32::new)
                .ok_or(RuntimeLocalDeclarationTableError::IdentityExhausted)?;
            declarations.push(RuntimeLocalDeclaration { ty });
            ids.push(RuntimeLocalDeclarationId::from_accepted_ordinal(ordinal));
        }
        Ok(PreparedRuntimeLocalDeclarationBatch {
            ids: ids.into_boxed_slice(),
            declarations,
        })
    }

    pub(crate) fn commit_batch(
        &mut self,
        prepared: PreparedRuntimeLocalDeclarationBatch,
    ) -> Box<[RuntimeLocalDeclarationId]> {
        self.declarations = prepared.declarations;
        prepared.ids
    }

    #[must_use]
    pub(crate) fn finish(self) -> RuntimeLocalDeclarationTable {
        RuntimeLocalDeclarationTable {
            declarations: self.declarations.into_boxed_slice(),
        }
    }

    #[cfg(test)]
    fn with_maximum_for_test(maximum: u32) -> Self {
        Self {
            maximum,
            ..Self::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_id::RuntimePlanTypeId;

    fn ty(ordinal: u32) -> RuntimePlanTypeId {
        RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::new(ordinal).unwrap())
    }

    #[test]
    fn builder_seals_typed_rows_in_contiguous_order() {
        let mut builder = RuntimeLocalDeclarationTableBuilder::new();
        let first = builder.push(ty(3)).expect("first local");
        let second = builder.push(ty(7)).expect("second local");
        let table = builder.finish();

        assert_eq!(first.get(), NonZeroU32::MIN);
        assert_eq!(second.get(), NonZeroU32::new(2).unwrap());
        assert_eq!(
            table.get(first).map(RuntimeLocalDeclaration::ty),
            Some(ty(3))
        );
        assert_eq!(
            table.get(second).map(RuntimeLocalDeclaration::ty),
            Some(ty(7))
        );
    }

    #[test]
    fn exhaustion_does_not_append_an_untyped_row() {
        let mut builder = RuntimeLocalDeclarationTableBuilder::with_maximum_for_test(1);
        let first = builder.push(ty(1)).expect("bounded first local");
        assert_eq!(
            builder.push(ty(2)),
            Err(RuntimeLocalDeclarationTableError::IdentityExhausted)
        );
        let table = builder.finish();
        assert_eq!(table.len(), 1);
        assert_eq!(
            table.get(first).map(RuntimeLocalDeclaration::ty),
            Some(ty(1))
        );
    }
}
