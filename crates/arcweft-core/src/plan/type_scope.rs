//! Lexical scopes retained by executable type declarations.
//!
//! References describe a binder depth and a kind-specific slot. They carry no
//! semantic-analysis issuer or inference identity. A declaration validates
//! every occurrence against its incoming scope before it can be used by a
//! program; executable value and ABI roots additionally require an empty scope.

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use crate::effect_row::{EffectFormula, EffectPredicate, EffectSet};

/// Independent namespaces introduced by one nonempty function binder.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(deny_unknown_fields)]
pub struct RuntimeTypeBinder {
    types: u16,
    const_lengths: u16,
    effects: u32,
}

impl RuntimeTypeBinder {
    pub const EMPTY: Self = Self::new(0, 0, 0);

    #[must_use]
    pub const fn new(types: u16, const_lengths: u16, effects: u32) -> Self {
        Self {
            types,
            const_lengths,
            effects,
        }
    }

    #[must_use]
    pub const fn types(self) -> u16 {
        self.types
    }

    #[must_use]
    pub const fn const_lengths(self) -> u16 {
        self.const_lengths
    }

    #[must_use]
    pub const fn effects(self) -> u32 {
        self.effects
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.types == 0 && self.const_lengths == 0 && self.effects == 0
    }
}

/// Incoming binders in outermost-to-innermost order. Empty binders do not
/// introduce a lexical level and cannot occur in this canonical inventory.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeTypeScope(Box<[RuntimeTypeBinder]>);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeBoundTypeReference {
    depth: u32,
    slot: u16,
}

impl RuntimeBoundTypeReference {
    #[must_use]
    pub const fn depth(self) -> u32 {
        self.depth
    }
    #[must_use]
    pub const fn slot(self) -> u16 {
        self.slot
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeBoundConstReference {
    depth: u32,
    slot: u16,
}

impl RuntimeBoundConstReference {
    #[must_use]
    pub const fn depth(self) -> u32 {
        self.depth
    }
    #[must_use]
    pub const fn slot(self) -> u16 {
        self.slot
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeBoundEffectReference {
    depth: u32,
    slot: u32,
}

impl RuntimeBoundEffectReference {
    #[must_use]
    pub const fn depth(self) -> u32 {
        self.depth
    }
    #[must_use]
    pub const fn slot(self) -> u32 {
        self.slot
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RuntimeArrayLength {
    Constant(u64),
    Bound(RuntimeBoundConstReference),
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeTypeScopeError {
    #[error("runtime type scope contains an empty binder")]
    EmptyBinder,
    #[error("runtime type scope exceeds the maximum type depth")]
    DepthLimit,
    #[error("runtime type reference has no binder at depth {depth}")]
    UnknownDepth { depth: u32 },
    #[error("runtime bound type slot {slot} exceeds arity {arity}")]
    TypeSlot { slot: u16, arity: u16 },
    #[error("runtime bound constant slot {slot} exceeds arity {arity}")]
    ConstSlot { slot: u16, arity: u16 },
    #[error("runtime bound effect slot {slot} exceeds arity {arity}")]
    EffectSlot { slot: u32, arity: u32 },
    #[error("runtime function type has an impossible effect predicate")]
    ImpossiblePredicate,
    #[error("a scoped runtime type descendant cannot be used as an executable root")]
    ScopedRoot,
}

impl RuntimeTypeScope {
    #[must_use]
    pub fn root() -> Self {
        Self::default()
    }

    pub fn try_from_binders(
        binders: impl Into<Box<[RuntimeTypeBinder]>>,
    ) -> Result<Self, RuntimeTypeScopeError> {
        let binders = binders.into();
        if binders.len() > super::MAX_RUNTIME_PLAN_TYPE_DEPTH {
            return Err(RuntimeTypeScopeError::DepthLimit);
        }
        if binders.iter().any(|binder| binder.is_empty()) {
            return Err(RuntimeTypeScopeError::EmptyBinder);
        }
        Ok(Self(binders))
    }

    #[must_use]
    pub fn binders(&self) -> &[RuntimeTypeBinder] {
        &self.0
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn require_root(&self) -> Result<(), RuntimeTypeScopeError> {
        self.is_root()
            .then_some(())
            .ok_or(RuntimeTypeScopeError::ScopedRoot)
    }

    /// Empty binders preserve the incoming depth. Every other binder adds one
    /// checked lexical level, regardless of the arity of each namespace.
    pub fn enter(&self, binder: RuntimeTypeBinder) -> Result<Self, RuntimeTypeScopeError> {
        if binder.is_empty() {
            return Ok(self.clone());
        }
        if self.0.len() >= super::MAX_RUNTIME_PLAN_TYPE_DEPTH {
            return Err(RuntimeTypeScopeError::DepthLimit);
        }
        let mut binders = self.0.to_vec();
        binders.push(binder);
        Ok(Self(binders.into_boxed_slice()))
    }

    fn binder(&self, depth: u32) -> Result<RuntimeTypeBinder, RuntimeTypeScopeError> {
        usize::try_from(depth)
            .ok()
            .and_then(|depth| self.0.len().checked_sub(depth.checked_add(1)?))
            .and_then(|index| self.0.get(index))
            .copied()
            .ok_or(RuntimeTypeScopeError::UnknownDepth { depth })
    }

    pub fn bound_type(
        &self,
        depth: u32,
        slot: u16,
    ) -> Result<RuntimeBoundTypeReference, RuntimeTypeScopeError> {
        let arity = self.binder(depth)?.types;
        if slot >= arity {
            return Err(RuntimeTypeScopeError::TypeSlot { slot, arity });
        }
        Ok(RuntimeBoundTypeReference { depth, slot })
    }

    pub fn bound_const(
        &self,
        depth: u32,
        slot: u16,
    ) -> Result<RuntimeBoundConstReference, RuntimeTypeScopeError> {
        let arity = self.binder(depth)?.const_lengths;
        if slot >= arity {
            return Err(RuntimeTypeScopeError::ConstSlot { slot, arity });
        }
        Ok(RuntimeBoundConstReference { depth, slot })
    }

    pub fn bound_effect(
        &self,
        depth: u32,
        slot: u32,
    ) -> Result<RuntimeBoundEffectReference, RuntimeTypeScopeError> {
        let arity = self.binder(depth)?.effects;
        if slot >= arity {
            return Err(RuntimeTypeScopeError::EffectSlot { slot, arity });
        }
        Ok(RuntimeBoundEffectReference { depth, slot })
    }

    pub fn validate_type(
        &self,
        reference: RuntimeBoundTypeReference,
    ) -> Result<(), RuntimeTypeScopeError> {
        self.bound_type(reference.depth, reference.slot).map(|_| ())
    }

    pub fn validate_length(&self, length: RuntimeArrayLength) -> Result<(), RuntimeTypeScopeError> {
        match length {
            RuntimeArrayLength::Constant(_) => Ok(()),
            RuntimeArrayLength::Bound(reference) => self
                .bound_const(reference.depth, reference.slot)
                .map(|_| ()),
        }
    }

    pub fn validate_effect(
        &self,
        reference: RuntimeBoundEffectReference,
    ) -> Result<(), RuntimeTypeScopeError> {
        self.bound_effect(reference.depth, reference.slot)
            .map(|_| ())
    }
}

impl<'de> Deserialize<'de> for RuntimeTypeScope {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from_binders(Box::<[RuntimeTypeBinder]>::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)
    }
}

/// Binder and membership terms owned by one function type. Parameter and result
/// type edges enter this binder; the predicate and invocation row use that same
/// resulting scope. This contract is never inferred from an executable body.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeFunctionTypeContract {
    binder: RuntimeTypeBinder,
    predicate: EffectPredicate<RuntimeBoundEffectReference>,
    invocation: EffectFormula<RuntimeBoundEffectReference>,
}

impl RuntimeFunctionTypeContract {
    #[must_use]
    pub fn new(
        binder: RuntimeTypeBinder,
        predicate: EffectPredicate<RuntimeBoundEffectReference>,
        invocation: EffectFormula<RuntimeBoundEffectReference>,
    ) -> Self {
        Self {
            binder,
            predicate,
            invocation,
        }
    }

    #[must_use]
    pub fn monomorphic(effects: EffectSet) -> Self {
        Self::new(
            RuntimeTypeBinder::EMPTY,
            EffectPredicate::unconstrained(),
            EffectFormula::literal(effects, None),
        )
    }

    #[must_use]
    pub const fn binder(&self) -> RuntimeTypeBinder {
        self.binder
    }
    #[must_use]
    pub const fn predicate(&self) -> &EffectPredicate<RuntimeBoundEffectReference> {
        &self.predicate
    }
    #[must_use]
    pub const fn invocation(&self) -> &EffectFormula<RuntimeBoundEffectReference> {
        &self.invocation
    }

    /// Validates both effect terms and returns the exact child-edge scope.
    pub fn child_scope(
        &self,
        incoming: &RuntimeTypeScope,
    ) -> Result<RuntimeTypeScope, RuntimeTypeScopeError> {
        let scope = incoming.enter(self.binder)?;
        for reference in self
            .predicate
            .variables()
            .chain(self.invocation.variables())
        {
            scope.validate_effect(*reference)?;
        }
        if self.predicate.is_impossible() {
            return Err(RuntimeTypeScopeError::ImpossiblePredicate);
        }
        Ok(scope)
    }
}

#[cfg(test)]
mod tests;
