//! Final typed plan-local declaration table.

use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};

mod placement;
mod source;
pub use placement::{
    RuntimeLineLocalBody, RuntimeLocalInitialization, RuntimeLocalOwner, RuntimeLocalPlacement,
    RuntimeLocalPlacementError, RuntimeLocalStorage,
};
pub use source::{
    RuntimeLocalBindingDeclaration, RuntimeLocalBindingKind, RuntimeLocalBindingStorage,
    RuntimeLocalDeclarationSource,
};

/// Stable origin issued by the semantic owner, separate from the plan-local ordinal.
/// Transporting this identity does not prove executable body admission.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimeLocalOrigin {
    Binding([u8; 32]),
    Parameter(super::RuntimeFunctionParameterIdentity),
    EvaluatedResult(super::RuntimeFunctionDefinitionIdentity),
    Generated(RuntimeGeneratedLocalOrigin),
}

impl From<super::RuntimeFunctionInputOrigin> for RuntimeLocalOrigin {
    fn from(origin: super::RuntimeFunctionInputOrigin) -> Self {
        match origin {
            super::RuntimeFunctionInputOrigin::Binding(binding) => Self::Binding(binding),
            super::RuntimeFunctionInputOrigin::Parameter(parameter) => Self::Parameter(parameter),
            super::RuntimeFunctionInputOrigin::EvaluatedResult(definition) => {
                Self::EvaluatedResult(definition)
            }
        }
    }
}

/// One accepted structural owner plus its generated-local semantic role.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeGeneratedLocalOrigin([u8; 32]);

impl RuntimeGeneratedLocalOrigin {
    #[must_use]
    pub const fn from_accepted_identity(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// One sealed plan-local declaration row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeLocalDeclaration {
    source: RuntimeLocalDeclarationSource,
    ty: RuntimePlanTypeId,
    context: Option<RuntimePlanTypeId>,
    placement: RuntimeLocalPlacement,
}

impl RuntimeLocalDeclaration {
    #[must_use]
    pub const fn placement(self) -> RuntimeLocalPlacement {
        self.placement
    }
    #[must_use]
    pub const fn source(self) -> RuntimeLocalDeclarationSource {
        self.source
    }

    #[must_use]
    pub const fn origin(self) -> RuntimeLocalOrigin {
        self.source.origin()
    }

    #[must_use]
    pub const fn context(self) -> Option<RuntimePlanTypeId> {
        self.context
    }
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
    declarations: Vec<PendingRuntimeLocalDeclaration>,
    maximum: u32,
    reserved: u32,
    sealed: bool,
}

/// Placement is incomplete only inside the construction authority.
#[derive(Debug)]
struct PendingRuntimeLocalDeclaration {
    source: RuntimeLocalDeclarationSource,
    ty: RuntimePlanTypeId,
    context: Option<RuntimePlanTypeId>,
    placement: Option<RuntimeLocalPlacement>,
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeLocalDeclarationTableError {
    #[error("runtime local-declaration identity space is exhausted")]
    IdentityExhausted,
}

impl RuntimeLocalDeclarationTableBuilder {
    pub(crate) fn len(&self) -> usize {
        self.declarations.len()
    }

    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            declarations: Vec::new(),
            maximum: u32::MAX,
            reserved: 0,
            sealed: false,
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
        source: RuntimeLocalDeclarationSource,
        ty: RuntimePlanTypeId,
    ) -> Result<RuntimeLocalDeclarationId, RuntimeLocalDeclarationTableError> {
        let prepared = self.prepare_request_count(1)?;
        self.commit_request_count(prepared);
        self.materialize(source, ty, None)
            .ok_or(RuntimeLocalDeclarationTableError::IdentityExhausted)
    }

    /// Reserves capacity atomically without minting unused slot coordinates.
    pub(crate) fn prepare_request_count(
        &self,
        count: usize,
    ) -> Result<u32, RuntimeLocalDeclarationTableError> {
        let count = u32::try_from(count)
            .map_err(|_| RuntimeLocalDeclarationTableError::IdentityExhausted)?;
        self.reserved
            .checked_add(count)
            .filter(|value| *value <= self.maximum && !self.sealed)
            .ok_or(RuntimeLocalDeclarationTableError::IdentityExhausted)
    }

    pub(crate) fn commit_request_count(&mut self, prepared: u32) {
        self.reserved = prepared;
    }

    /// Only an admitted, not-yet-materialized request may consume one reservation.
    pub(crate) fn materialize(
        &mut self,
        source: RuntimeLocalDeclarationSource,
        ty: RuntimePlanTypeId,
        context: Option<RuntimePlanTypeId>,
    ) -> Option<RuntimeLocalDeclarationId> {
        if self.sealed || self.declarations.len() >= self.reserved as usize {
            return None;
        }
        let ordinal = u32::try_from(self.declarations.len().checked_add(1)?).ok()?;
        let ordinal = NonZeroU32::new(ordinal)?;
        self.declarations.push(PendingRuntimeLocalDeclaration {
            source,
            ty,
            context,
            placement: None,
        });
        Some(RuntimeLocalDeclarationId::from_accepted_ordinal(ordinal))
    }

    pub(crate) fn take_finish(
        &mut self,
    ) -> Result<RuntimeLocalDeclarationTable, RuntimeLocalPlacementError> {
        // Check every row before taking any of them. Rejection publishes none.
        for (index, declaration) in self.declarations.iter().enumerate() {
            if declaration.placement.is_none() {
                return Err(RuntimeLocalPlacementError::Unowned {
                    local: RuntimeLocalDeclarationId::from_accepted_ordinal(
                        NonZeroU32::new(
                            u32::try_from(index)
                                .map_err(|_| RuntimeLocalPlacementError::CoordinateOverflow)?
                                .checked_add(1)
                                .ok_or(RuntimeLocalPlacementError::CoordinateOverflow)?,
                        )
                        .expect("materialized ordinal is positive"),
                    ),
                });
            }
        }
        self.sealed = true;
        Ok(RuntimeLocalDeclarationTable {
            declarations: std::mem::take(&mut self.declarations)
                .into_iter()
                .map(|row| RuntimeLocalDeclaration {
                    source: row.source,
                    ty: row.ty,
                    context: row.context,
                    placement: row
                        .placement
                        .expect("all placements checked before publication"),
                })
                .collect(),
        })
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) fn finish(mut self) -> RuntimeLocalDeclarationTable {
        for row in &mut self.declarations {
            row.placement = Some(RuntimeLocalPlacement::test_fixture());
        }
        self.take_finish()
            .expect("test table supplies explicit fixture placements")
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
        let declaration = RuntimeLocalBindingDeclaration::new(
            RuntimeLocalBindingKind::LetBinding,
            true,
            RuntimeLocalBindingStorage::RetainedState,
        );
        let first_source = RuntimeLocalDeclarationSource::Binding {
            identity: [0x31; 32],
            declaration,
        };
        let first_origin = first_source.origin();
        let second_source = RuntimeLocalDeclarationSource::Parameter(
            super::super::RuntimeFunctionParameterIdentity::from_accepted_identity([0x71; 32]),
        );
        let second_origin = second_source.origin();
        let first = builder.push(first_source, ty(3)).expect("first local");
        let second = builder.push(second_source, ty(7)).expect("second local");
        let table = builder.finish();
        assert_eq!(
            table.get(first).map(RuntimeLocalDeclaration::source),
            Some(first_source)
        );
        assert_eq!(
            table.get(first).and_then(|row| row.source().binding()),
            Some(declaration)
        );
        assert_eq!(
            table.get(second).map(RuntimeLocalDeclaration::source),
            Some(second_source)
        );

        assert_eq!(first.get(), NonZeroU32::MIN);
        assert_eq!(second.get(), NonZeroU32::new(2).unwrap());
        assert_eq!(
            table.get(first).map(RuntimeLocalDeclaration::origin),
            Some(first_origin)
        );
        assert_eq!(
            table.get(second).map(RuntimeLocalDeclaration::origin),
            Some(second_origin)
        );
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
        let source = RuntimeLocalDeclarationSource::Binding {
            identity: [0x11; 32],
            declaration: RuntimeLocalBindingDeclaration::new(
                RuntimeLocalBindingKind::PatternBinding,
                false,
                RuntimeLocalBindingStorage::Derived,
            ),
        };
        let origin = source.origin();
        let first = builder.push(source, ty(1)).expect("bounded first local");
        assert_eq!(
            builder.push(
                RuntimeLocalDeclarationSource::Binding {
                    identity: [0x22; 32],
                    declaration: RuntimeLocalBindingDeclaration::new(
                        RuntimeLocalBindingKind::PatternBinding,
                        false,
                        RuntimeLocalBindingStorage::Derived
                    )
                },
                ty(2)
            ),
            Err(RuntimeLocalDeclarationTableError::IdentityExhausted)
        );
        let table = builder.finish();
        assert_eq!(table.len(), 1);
        assert_eq!(
            table.get(first).map(RuntimeLocalDeclaration::origin),
            Some(origin)
        );
        assert_eq!(
            table.get(first).map(RuntimeLocalDeclaration::ty),
            Some(ty(1))
        );
    }
}
