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

/// Accepted structural anchor and generation evidence for a lexical frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeScopeOrigin {
    Expression(arcweft_lang_sema::semantic_coordinate::CheckedExpressionOrigin),
    Statement(arcweft_lang_sema::semantic_coordinate::CheckedStatementOrigin),
}

impl RuntimeScopeOrigin {
    pub fn coordinate(&self) -> &arcweft_lang_sema::semantic_coordinate::CheckedSemanticPath {
        match self {
            Self::Expression(origin) => origin.coordinate(),
            Self::Statement(origin) => origin.coordinate().path(),
        }
    }

    pub(super) fn validate_owner(
        &self,
        project: arcweft_lang_hir::project::HirAnalysisProjectView<'_>,
        owner: RuntimeScopeOwner,
    ) -> bool {
        match (self, owner) {
            (Self::Expression(origin), RuntimeScopeOwner::Expression(owner)) => {
                origin.validate_owner(project, owner)
            }
            (Self::Statement(origin), RuntimeScopeOwner::Statement(owner)) => {
                origin.validate_owner(project, owner)
            }
            _ => false,
        }
    }

    pub(super) fn validate_authority(
        &self,
        authority: &arcweft_lang_sema::final_analysis::CheckedLocalUseAuthority,
        owner: RuntimeScopeOwner,
    ) -> bool {
        match (self, owner) {
            (Self::Expression(origin), RuntimeScopeOwner::Expression(owner)) => {
                origin.validate_authority(authority, owner)
            }
            (Self::Statement(origin), RuntimeScopeOwner::Statement(owner)) => {
                origin.validate_authority(authority, owner)
            }
            _ => false,
        }
    }
}

/// One lexical frame, its local carrier when present, and an optional
/// continuation for a Try residual targeting an enclosing owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeScopeFact {
    origin: RuntimeScopeOrigin,
    identity: RuntimeScopeIdentity,
    continuation: Option<RuntimeScopeContinuation>,
    semantic_unit: Option<Box<RuntimeNormalizedType>>,
    carrier_block: Option<Box<RuntimeCarrierBlock>>,
}

/// The normalized local carrier owned by one immediate computation block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCarrierBlock {
    carrier_type: RuntimeNormalizedType,
    delivers_value: bool,
}

impl RuntimeCarrierBlock {
    pub const fn carrier_type(&self) -> &RuntimeNormalizedType {
        &self.carrier_type
    }

    pub const fn delivers_value(&self) -> bool {
        self.delivers_value
    }
}

impl RuntimeScopeFact {
    pub const fn new(
        origin: RuntimeScopeOrigin,
        identity: RuntimeScopeIdentity,
        continuation: Option<RuntimeScopeContinuation>,
    ) -> Self {
        Self {
            origin,
            identity,
            continuation,
            semantic_unit: None,
            carrier_block: None,
        }
    }

    /// Supplies the physical result of a scope whose tail is consumed by the
    /// compiler. The caller continues with the checked semantic value.
    pub fn with_semantic_unit(mut self, unit: RuntimeNormalizedType) -> Self {
        self.semantic_unit = Some(Box::new(unit));
        self
    }

    pub fn semantic_unit(&self) -> Option<&RuntimeNormalizedType> {
        self.semantic_unit.as_deref()
    }

    pub fn with_carrier_block(
        mut self,
        carrier_type: RuntimeNormalizedType,
        delivers_value: bool,
    ) -> Self {
        self.carrier_block = Some(Box::new(RuntimeCarrierBlock {
            carrier_type,
            delivers_value,
        }));
        self
    }

    pub fn carrier_block(&self) -> Option<&RuntimeCarrierBlock> {
        self.carrier_block.as_deref()
    }

    pub const fn origin(&self) -> &RuntimeScopeOrigin {
        &self.origin
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
        roots.extend(self.semantic_unit());
        roots.extend(self.carrier_block().map(RuntimeCarrierBlock::carrier_type));
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
