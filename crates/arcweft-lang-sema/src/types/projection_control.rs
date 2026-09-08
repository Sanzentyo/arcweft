//! Consumer-owned work admission for semantic type projection.

use std::convert::Infallible;

use thiserror::Error;

use super::{GenericScopeError, TypeInstantiationError};
use crate::effect_row::{EffectRow, EffectRowError, EffectRowTail};

/// Structural occurrence visited before it is copied or projected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeProjectionNodeKind {
    Type,
    Const,
    Effect,
}

/// Accounting belongs to the consumer's transaction. Semantic projection
/// supplies occurrences and depth without owning a second budget or clock.
pub trait TypeProjectionControl {
    type Error: std::error::Error + 'static;

    fn check(&mut self) -> Result<(), Self::Error>;

    fn visit_node(&mut self, kind: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error>;

    fn visit_binding(&mut self) -> Result<(), Self::Error>;
}

/// Retains the consumer's exact abort separately from semantic invalidity.
#[derive(Debug, Error)]
pub enum TypeProjectionError<E: std::error::Error + 'static> {
    #[error(transparent)]
    Instantiation(#[from] TypeInstantiationError),
    #[error("semantic type projection aborted: {0}")]
    Control(#[source] E),
}

impl<E: std::error::Error + 'static> From<GenericScopeError> for TypeProjectionError<E> {
    fn from(error: GenericScopeError) -> Self {
        Self::Instantiation(error.into())
    }
}

impl<E: std::error::Error + 'static> From<EffectRowError> for TypeProjectionError<E> {
    fn from(error: EffectRowError) -> Self {
        Self::Instantiation(error.into())
    }
}

impl<E: std::error::Error + 'static> From<super::constraints::TypeConstraintError>
    for TypeProjectionError<E>
{
    fn from(_: super::constraints::TypeConstraintError) -> Self {
        Self::Instantiation(TypeInstantiationError::UnresolvedType)
    }
}

pub(crate) struct UnmeteredTypeProjection;

impl TypeProjectionControl for UnmeteredTypeProjection {
    type Error = Infallible;

    fn check(&mut self) -> Result<(), Infallible> {
        Ok(())
    }

    fn visit_node(&mut self, _: TypeProjectionNodeKind, _: u64) -> Result<(), Infallible> {
        Ok(())
    }

    fn visit_binding(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
}

impl TypeProjectionError<Infallible> {
    pub(crate) fn into_instantiation(self) -> TypeInstantiationError {
        match self {
            Self::Instantiation(error) => error,
            Self::Control(impossible) => match impossible {},
        }
    }
}

pub(in crate::types) fn visit_effect_row<C: TypeProjectionControl>(
    control: &mut C,
    row: &EffectRow,
    depth: u64,
) -> Result<(), TypeProjectionError<C::Error>> {
    control.check().map_err(TypeProjectionError::Control)?;
    control
        .visit_node(TypeProjectionNodeKind::Effect, depth)
        .map_err(TypeProjectionError::Control)?;
    if row.concrete().is_empty() && matches!(row.tail(), EffectRowTail::Closed) {
        return Ok(());
    }
    let child_depth = depth
        .checked_add(1)
        .ok_or(TypeInstantiationError::DepthOverflow)?;
    for _ in row.concrete().iter() {
        control
            .visit_node(TypeProjectionNodeKind::Effect, child_depth)
            .map_err(TypeProjectionError::Control)?;
    }
    if !matches!(row.tail(), EffectRowTail::Closed) {
        control
            .visit_node(TypeProjectionNodeKind::Effect, child_depth)
            .map_err(TypeProjectionError::Control)?;
    }
    Ok(())
}
