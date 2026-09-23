//! Typed value/propagation join for one accepted lexical scope.

use arcweft_core::scope::RuntimeScopeIdentity;
use arcweft_lang_hir::identity::{ExprId, StmtId};
use thiserror::Error;

use super::{RuntimeNormalizedType, RuntimeTryBoundaryOwner, RuntimeTypeShape};

/// Exact owner of a lexical frame in one executable semantic view.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RuntimeScopeOwner {
    Expression(ExprId),
    Statement(StmtId),
}

/// One lexical frame and its optional outward propagation continuation.
/// The carrier belongs to this lowering boundary: its success is the scope's
/// own value, while its residual is exactly the checked enclosing carrier's.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeScopeFact {
    identity: RuntimeScopeIdentity,
    continuation: Option<RuntimeScopeContinuation>,
}

impl RuntimeScopeFact {
    pub const fn new(
        identity: RuntimeScopeIdentity,
        continuation: Option<RuntimeScopeContinuation>,
    ) -> Self {
        Self {
            identity,
            continuation,
        }
    }

    pub const fn identity(&self) -> &RuntimeScopeIdentity {
        &self.identity
    }

    pub const fn continuation(&self) -> Option<&RuntimeScopeContinuation> {
        self.continuation.as_ref()
    }

    pub(super) fn append_normalized_types<'a>(
        &'a self,
        roots: &mut Vec<&'a RuntimeNormalizedType>,
    ) {
        if let Some(continuation) = &self.continuation {
            roots.extend([continuation.carrier_type(), continuation.boundary_type()]);
        }
    }
}

/// A scope evaluates this carrier before its lexical frame is removed. Its
/// caller dispatches success and residual only after that removal completes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeScopeContinuation {
    carrier: RuntimeNormalizedType,
    boundary: RuntimeTryBoundaryOwner,
    boundary_type: RuntimeNormalizedType,
    exits: Box<[ExprId]>,
}

impl RuntimeScopeContinuation {
    pub fn try_new(
        carrier: RuntimeNormalizedType,
        boundary: RuntimeTryBoundaryOwner,
        boundary_type: RuntimeNormalizedType,
        exits: Box<[ExprId]>,
    ) -> Result<Self, RuntimeScopeContinuationError> {
        if boundary == RuntimeTryBoundaryOwner::Infallible {
            return Err(RuntimeScopeContinuationError::InfallibleBoundary);
        }
        if exits.is_empty() || exits.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(RuntimeScopeContinuationError::InvalidExitInventory);
        }
        match (carrier.shape(), boundary_type.shape()) {
            (
                RuntimeTypeShape::Result { error, .. },
                RuntimeTypeShape::Result { error: target, .. },
            ) => {
                if error != target {
                    return Err(RuntimeScopeContinuationError::ResidualTypeMismatch);
                }
            }
            (RuntimeTypeShape::Option { .. }, RuntimeTypeShape::Option { .. }) => {}
            _ => return Err(RuntimeScopeContinuationError::CarrierFamilyMismatch),
        }
        Ok(Self {
            carrier,
            boundary,
            boundary_type,
            exits,
        })
    }

    pub const fn carrier_type(&self) -> &RuntimeNormalizedType {
        &self.carrier
    }

    pub const fn boundary(&self) -> RuntimeTryBoundaryOwner {
        self.boundary
    }

    pub const fn boundary_type(&self) -> &RuntimeNormalizedType {
        &self.boundary_type
    }

    pub const fn exits(&self) -> &[ExprId] {
        &self.exits
    }

    pub fn value_type(&self) -> &RuntimeNormalizedType {
        match self.carrier.shape() {
            RuntimeTypeShape::Result { value, .. } => value,
            RuntimeTypeShape::Option { item, .. } => item,
            _ => unreachable!("scope continuation construction accepts only checked carriers"),
        }
    }

    pub fn residual_type(&self) -> Option<&RuntimeNormalizedType> {
        match self.carrier.shape() {
            RuntimeTypeShape::Result { error, .. } => Some(error),
            RuntimeTypeShape::Option { .. } => None,
            _ => unreachable!("scope continuation construction accepts only checked carriers"),
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeScopeContinuationError {
    #[error("scope continuation exits must be nonempty, unique, and source-owner ordered")]
    InvalidExitInventory,
    #[error("an infallible Try cannot own an outward scope continuation")]
    InfallibleBoundary,
    #[error("scope and enclosing propagation carriers have different families")]
    CarrierFamilyMismatch,
    #[error("scope and enclosing propagation carriers have different residual types")]
    ResidualTypeMismatch,
}
