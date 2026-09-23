//! Consumer-owned work admission for semantic type projection.

use std::convert::Infallible;

use thiserror::Error;

use super::{GenericEffectReference, GenericScopeError, TypeInstantiationError};
use crate::{
    effect_row::{
        DecisionControl, DecisionEncoding, DecisionWork, EffectRow, EffectRowError,
        MembershipEncoding,
    },
    effects::EffectId,
};

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
    row.encode(&mut EffectProjectionVisitor { control, depth })
}

pub(in crate::types) fn visit_effect_predicate<C: TypeProjectionControl>(
    control: &mut C,
    predicate: &crate::effect_row::EffectPredicate,
    depth: u64,
) -> Result<(), TypeProjectionError<C::Error>> {
    predicate.encode(&mut EffectProjectionVisitor { control, depth })
}

/// Decision construction uses the same consumer-owned control as type folds.
pub(in crate::types) struct EffectProjectionControl<'a, C> {
    pub(in crate::types) control: &'a mut C,
    pub(in crate::types) depth: u64,
}

impl<C: TypeProjectionControl> DecisionControl for EffectProjectionControl<'_, C> {
    type Error = TypeProjectionError<C::Error>;

    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        self.control.check().map_err(TypeProjectionError::Control)?;
        self.control
            .visit_node(TypeProjectionNodeKind::Effect, self.depth)
            .map_err(TypeProjectionError::Control)
    }
}

/// The row owner supplies its complete canonical graph grammar. This visitor
/// admits every structural token and atom without reconstructing a row or
/// allocating an encoded copy of it.
struct EffectProjectionVisitor<'a, C> {
    control: &'a mut C,
    depth: u64,
}

impl<C: TypeProjectionControl> EffectProjectionVisitor<'_, C> {
    fn visit(&mut self, depth: u64) -> Result<(), TypeProjectionError<C::Error>> {
        self.control.check().map_err(TypeProjectionError::Control)?;
        self.control
            .visit_node(TypeProjectionNodeKind::Effect, depth)
            .map_err(TypeProjectionError::Control)
    }

    fn visit_atom(&mut self) -> Result<(), TypeProjectionError<C::Error>> {
        let depth = self
            .depth
            .checked_add(1)
            .ok_or(TypeInstantiationError::DepthOverflow)?;
        self.visit(depth)
    }
}

impl<C: TypeProjectionControl> DecisionEncoding<GenericEffectReference>
    for EffectProjectionVisitor<'_, C>
{
    type Error = TypeProjectionError<C::Error>;

    fn tag(&mut self, _: u8) -> Result<(), Self::Error> {
        self.visit(self.depth)
    }

    fn count(&mut self, _: usize) -> Result<(), Self::Error> {
        self.visit(self.depth)
    }

    fn variable(&mut self, _: &GenericEffectReference) -> Result<(), Self::Error> {
        self.visit_atom()
    }
}

impl<C: TypeProjectionControl> MembershipEncoding<GenericEffectReference>
    for EffectProjectionVisitor<'_, C>
{
    fn effect(&mut self, _: &EffectId) -> Result<(), Self::Error> {
        self.visit_atom()
    }
}
