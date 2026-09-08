//! Generic binders and kind-separated references in semantic type terms.
//!
//! Declaration identity is independent of an application. Opening a binder
//! creates inference references in a private application namespace; a caller's
//! free reference never becomes the callee's inference variable.

use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

use thiserror::Error;

use crate::effect_row::EffectVar;

use super::{
    ArrayLength, GenericConstParameterId, GenericTypeParameterId, SemanticTypeDigest, TypeKind,
};

/// Owned lexical context for a projected type term. Consumers must explicitly
/// close it at the root or transfer its incoming binders to a function scheme.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScopedType {
    value: TypeKind,
    scope: GenericScope,
}

impl ScopedType {
    pub(in crate::types) const fn new(value: TypeKind, scope: GenericScope) -> Self {
        Self { value, scope }
    }

    pub(crate) const fn view(&self) -> ScopedTypeView<'_> {
        ScopedTypeView::sealed(&self.value, &self.scope)
    }
}

/// A borrowed semantic term together with its incoming lexical scope. This
/// carrier supplies context, not a certificate that the term is well scoped.
#[derive(Debug, Eq, PartialEq)]
pub struct ScopedView<'a, T> {
    value: &'a T,
    scope: &'a GenericScope,
}

impl<T> Copy for ScopedView<'_, T> {}
impl<T> Clone for ScopedView<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

/// A contextual semantic type term.
pub type ScopedTypeView<'a> = ScopedView<'a, TypeKind>;
/// A contextual array-length term.
pub type ScopedArrayLengthView<'a> = ScopedView<'a, ArrayLength>;
/// A template type-parameter key with its declaration/scheme scope.
pub type ScopedTypeReferenceView<'a> = ScopedView<'a, GenericTypeReference>;
/// A template constant-parameter key with its declaration/scheme scope.
pub type ScopedConstReferenceView<'a> = ScopedView<'a, GenericConstReference>;

impl<'a, T> ScopedView<'a, T> {
    pub(in crate::types) const fn sealed(value: &'a T, scope: &'a GenericScope) -> Self {
        Self { value, scope }
    }

    pub(crate) const fn at_root(value: &'a T) -> Self {
        Self {
            value,
            scope: &ROOT_GENERIC_SCOPE,
        }
    }

    pub const fn value(self) -> &'a T {
        self.value
    }
    pub const fn scope(self) -> &'a GenericScope {
        self.scope
    }
}

impl ScopedTypeView<'_> {
    pub fn semantic_identity_digest(self) -> Result<SemanticTypeDigest, GenericScopeError> {
        self.value.semantic_identity_digest_in_scope(self.scope)
    }

    pub fn semantic_identity_digest_with_control<C: super::TypeProjectionControl>(
        self,
        control: &mut C,
    ) -> Result<SemanticTypeDigest, super::TypeProjectionError<C::Error>> {
        self.value
            .semantic_identity_digest_in_scope_with_control(self.scope, control)
    }
}

impl ScopedArrayLengthView<'_> {
    pub(crate) fn canonical_checked_bytes(self) -> Result<Vec<u8>, super::TypeInstantiationError> {
        self.value.canonical_checked_bytes_in_scope(self.scope)
    }

    pub(crate) fn canonical_checked_bytes_with_control<C: super::TypeProjectionControl>(
        self,
        control: &mut C,
    ) -> Result<Vec<u8>, super::TypeProjectionError<C::Error>> {
        self.value
            .canonical_checked_bytes_in_scope_with_control(self.scope, control)
    }
}

impl ScopedTypeReferenceView<'_> {
    pub fn semantic_identity_digest(self) -> Result<SemanticTypeDigest, GenericScopeError> {
        TypeKind::GenericParam(self.value.clone()).semantic_identity_digest_in_scope(self.scope)
    }

    pub fn semantic_identity_digest_with_control<C: super::TypeProjectionControl>(
        self,
        control: &mut C,
    ) -> Result<SemanticTypeDigest, super::TypeProjectionError<C::Error>> {
        control
            .check()
            .map_err(super::TypeProjectionError::Control)?;
        TypeKind::GenericParam(self.value.clone())
            .semantic_identity_digest_in_scope_with_control(self.scope, control)
    }
}

impl ScopedConstReferenceView<'_> {
    pub(crate) fn canonical_checked_bytes(self) -> Result<Vec<u8>, super::TypeInstantiationError> {
        ArrayLength::Generic(self.value.clone()).canonical_checked_bytes_in_scope(self.scope)
    }

    pub(crate) fn canonical_checked_bytes_with_control<C: super::TypeProjectionControl>(
        self,
        control: &mut C,
    ) -> Result<Vec<u8>, super::TypeProjectionError<C::Error>> {
        control
            .check()
            .map_err(super::TypeProjectionError::Control)?;
        ArrayLength::Generic(self.value.clone())
            .canonical_checked_bytes_in_scope_with_control(self.scope, control)
    }
}
/// Arity of the three namespaces bound by a function scheme.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenericBinder {
    types: u16,
    const_lengths: u16,
    effects: u32,
}

impl GenericBinder {
    pub const EMPTY: Self = Self {
        types: 0,
        const_lengths: 0,
        effects: 0,
    };

    pub(crate) const fn new(types: u16, const_lengths: u16, effects: u32) -> Self {
        Self {
            types,
            const_lengths,
            effects,
        }
    }

    pub const fn types(self) -> u16 {
        self.types
    }
    pub const fn const_lengths(self) -> u16 {
        self.const_lengths
    }
    pub const fn effects(self) -> u32 {
        self.effects
    }
    pub const fn is_empty(self) -> bool {
        self.types == 0 && self.const_lengths == 0 && self.effects == 0
    }

    /// Combines adjacent quantifier inventories, preserving the left-hand
    /// slots as a prefix of each namespace in the resulting binder.
    pub(crate) fn checked_append(self, inner: Self) -> Result<Self, GenericScopeError> {
        let types =
            self.types
                .checked_add(inner.types)
                .ok_or(GenericScopeError::BinderArityOverflow {
                    kind: GenericParameterKind::Type,
                    count: usize::from(self.types) + usize::from(inner.types),
                })?;
        let const_lengths = self.const_lengths.checked_add(inner.const_lengths).ok_or(
            GenericScopeError::BinderArityOverflow {
                kind: GenericParameterKind::Const,
                count: usize::from(self.const_lengths) + usize::from(inner.const_lengths),
            },
        )?;
        let effects = self.effects.checked_add(inner.effects).ok_or_else(|| {
            GenericScopeError::BinderArityOverflow {
                kind: GenericParameterKind::Effect,
                count: usize::try_from(u64::from(self.effects) + u64::from(inner.effects))
                    .unwrap_or(usize::MAX),
            }
        })?;
        Ok(Self {
            types,
            const_lengths,
            effects,
        })
    }
}

/// A process-local opening capability. It has no serialized representation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in crate::types) struct GenericApplicationIssuer(NonZeroU64);

impl GenericApplicationIssuer {
    fn fresh() -> Result<Self, GenericScopeError> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let value = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .map_err(|_| GenericScopeError::IssuerExhausted)?;
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(GenericScopeError::IssuerExhausted)
    }
}

macro_rules! bound_parameter {
    ($name:ident, $slot:ty) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            depth: u32,
            slot: $slot,
        }

        impl $name {
            pub const fn depth(self) -> u32 {
                self.depth
            }
            pub const fn slot(self) -> $slot {
                self.slot
            }
        }
    };
}

bound_parameter!(BoundTypeParameter, u16);
bound_parameter!(BoundConstParameter, u16);
bound_parameter!(BoundEffectParameter, u32);

macro_rules! inference_parameter {
    ($name:ident, $slot:ty) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            issuer: GenericApplicationIssuer,
            slot: $slot,
        }

        impl $name {
            pub(in crate::types) const fn issuer(self) -> GenericApplicationIssuer {
                self.issuer
            }
            pub(in crate::types) const fn slot(self) -> $slot {
                self.slot
            }
        }
    };
}

inference_parameter!(InferenceTypeParameter, u16);
inference_parameter!(InferenceConstParameter, u16);
inference_parameter!(InferenceEffectParameter, u32);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GenericTypeReference {
    Free(GenericTypeParameterId),
    Bound(BoundTypeParameter),
    Inference(InferenceTypeParameter),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GenericConstReference {
    Free(GenericConstParameterId),
    Bound(BoundConstParameter),
    Inference(InferenceConstParameter),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GenericEffectReference {
    Free(EffectVar),
    Bound(BoundEffectParameter),
    Inference(InferenceEffectParameter),
}

impl From<GenericTypeParameterId> for GenericTypeReference {
    fn from(parameter: GenericTypeParameterId) -> Self {
        Self::Free(parameter)
    }
}

impl From<GenericConstParameterId> for GenericConstReference {
    fn from(parameter: GenericConstParameterId) -> Self {
        Self::Free(parameter)
    }
}

impl From<EffectVar> for GenericEffectReference {
    fn from(parameter: EffectVar) -> Self {
        Self::Free(parameter)
    }
}

impl GenericTypeReference {
    pub const fn free_parameter(&self) -> Option<&GenericTypeParameterId> {
        match self {
            Self::Free(parameter) => Some(parameter),
            _ => None,
        }
    }

    pub(crate) fn source_label(&self) -> String {
        match self {
            Self::Free(parameter) => parameter.source_label(),
            Self::Bound(parameter) => {
                format!("$bound-type<{}>#{}", parameter.depth, parameter.slot)
            }
            Self::Inference(parameter) => format!("$inference-type#{}", parameter.slot),
        }
    }
}

impl GenericConstReference {
    pub const fn free_parameter(&self) -> Option<&GenericConstParameterId> {
        match self {
            Self::Free(parameter) => Some(parameter),
            _ => None,
        }
    }

    pub(crate) fn source_label(&self) -> String {
        match self {
            Self::Free(parameter) => parameter.source_label(),
            Self::Bound(parameter) => {
                format!("$bound-const<{}>#{}", parameter.depth, parameter.slot)
            }
            Self::Inference(parameter) => format!("$inference-const#{}", parameter.slot),
        }
    }
}

/// Incoming lexical binders, from outermost to innermost.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GenericScope {
    binders: Vec<GenericBinder>,
}

static ROOT_GENERIC_SCOPE: GenericScope = GenericScope {
    binders: Vec::new(),
};

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum GenericScopeError {
    #[error("generic application issuer space is exhausted")]
    IssuerExhausted,
    #[error("generic binder depth {depth} is outside the lexical scope")]
    UnknownDepth { depth: u32 },
    #[error("generic {kind:?} slot {slot} is outside binder arity {arity}")]
    UnknownSlot {
        kind: GenericParameterKind,
        slot: u32,
        arity: u32,
    },
    #[error("an active {kind:?} inference reference escaped its application scope")]
    EscapedInference { kind: GenericParameterKind },
    #[error("generic {kind:?} binder arity {count} exceeds the slot representation")]
    BinderArityOverflow {
        kind: GenericParameterKind,
        count: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GenericParameterKind {
    Type,
    Const,
    Effect,
}

impl GenericScope {
    pub(crate) fn without_inner(&self, count: usize) -> Result<Self, GenericScopeError> {
        let length =
            self.binders
                .len()
                .checked_sub(count)
                .ok_or(GenericScopeError::UnknownDepth {
                    depth: u32::try_from(count).unwrap_or(u32::MAX),
                })?;
        Ok(Self {
            binders: self.binders[..length].to_vec(),
        })
    }

    pub(crate) fn with_binder(&self, binder: GenericBinder) -> Self {
        if binder.is_empty() {
            return self.clone();
        }
        let mut binders = self.binders.to_vec();
        binders.push(binder);
        Self { binders }
    }

    pub fn binders(&self) -> &[GenericBinder] {
        &self.binders
    }

    fn binder(&self, depth: u32) -> Result<GenericBinder, GenericScopeError> {
        usize::try_from(depth)
            .ok()
            .and_then(|depth| depth.checked_add(1))
            .and_then(|distance| self.binders.len().checked_sub(distance))
            .and_then(|index| self.binders.get(index).copied())
            .ok_or(GenericScopeError::UnknownDepth { depth })
    }

    pub(crate) fn bound_type(
        &self,
        depth: u32,
        slot: u16,
    ) -> Result<GenericTypeReference, GenericScopeError> {
        check_slot(
            GenericParameterKind::Type,
            u32::from(slot),
            u32::from(self.binder(depth)?.types),
        )?;
        Ok(GenericTypeReference::Bound(BoundTypeParameter {
            depth,
            slot,
        }))
    }

    pub(crate) fn bound_const(
        &self,
        depth: u32,
        slot: u16,
    ) -> Result<GenericConstReference, GenericScopeError> {
        check_slot(
            GenericParameterKind::Const,
            u32::from(slot),
            u32::from(self.binder(depth)?.const_lengths),
        )?;
        Ok(GenericConstReference::Bound(BoundConstParameter {
            depth,
            slot,
        }))
    }

    pub(crate) fn bound_effect(
        &self,
        depth: u32,
        slot: u32,
    ) -> Result<GenericEffectReference, GenericScopeError> {
        check_slot(
            GenericParameterKind::Effect,
            slot,
            self.binder(depth)?.effects,
        )?;
        Ok(GenericEffectReference::Bound(BoundEffectParameter {
            depth,
            slot,
        }))
    }
}

impl GenericTypeReference {
    /// Resolves a type occurrence to the incoming template's coordinate.
    /// References owned by a nested function have no key in that template.
    pub(crate) fn template_key(
        &self,
        incoming: &GenericScope,
        occurrence: &GenericScope,
    ) -> Result<Option<Self>, GenericScopeError> {
        match self {
            Self::Free(_) => Ok(Some(self.clone())),
            Self::Inference(_) => Err(GenericScopeError::EscapedInference {
                kind: GenericParameterKind::Type,
            }),
            Self::Bound(parameter) => {
                occurrence.bound_type(parameter.depth(), parameter.slot())?;
                let local = occurrence
                    .binders
                    .len()
                    .checked_sub(incoming.binders.len())
                    .ok_or(GenericScopeError::UnknownDepth {
                        depth: parameter.depth(),
                    })?;
                let Some(depth) = usize::try_from(parameter.depth())
                    .ok()
                    .and_then(|depth| depth.checked_sub(local))
                    .and_then(|depth| u32::try_from(depth).ok())
                else {
                    return Ok(None);
                };
                incoming.bound_type(depth, parameter.slot()).map(Some)
            }
        }
    }
}

impl GenericConstReference {
    /// Resolves a length occurrence to the incoming template's coordinate.
    pub(crate) fn template_key(
        &self,
        incoming: &GenericScope,
        occurrence: &GenericScope,
    ) -> Result<Option<Self>, GenericScopeError> {
        match self {
            Self::Free(_) => Ok(Some(self.clone())),
            Self::Inference(_) => Err(GenericScopeError::EscapedInference {
                kind: GenericParameterKind::Const,
            }),
            Self::Bound(parameter) => {
                occurrence.bound_const(parameter.depth(), parameter.slot())?;
                let local = occurrence
                    .binders
                    .len()
                    .checked_sub(incoming.binders.len())
                    .ok_or(GenericScopeError::UnknownDepth {
                        depth: parameter.depth(),
                    })?;
                let Some(depth) = usize::try_from(parameter.depth())
                    .ok()
                    .and_then(|depth| depth.checked_sub(local))
                    .and_then(|depth| u32::try_from(depth).ok())
                else {
                    return Ok(None);
                };
                incoming.bound_const(depth, parameter.slot()).map(Some)
            }
        }
    }
}

/// The types-owned opening of one callable binder. Only the constraint
/// initialization owner can mint it and project its inference slots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::types) struct OpenedGenericScope {
    issuer: GenericApplicationIssuer,
    binder: GenericBinder,
}

impl OpenedGenericScope {
    pub(in crate::types) const fn issuer(&self) -> GenericApplicationIssuer {
        self.issuer
    }

    pub(in crate::types) fn new(binder: GenericBinder) -> Result<Self, GenericScopeError> {
        Ok(Self {
            issuer: GenericApplicationIssuer::fresh()?,
            binder,
        })
    }

    pub(in crate::types) fn type_reference(
        &self,
        slot: u16,
    ) -> Result<GenericTypeReference, GenericScopeError> {
        check_slot(
            GenericParameterKind::Type,
            u32::from(slot),
            u32::from(self.binder.types),
        )?;
        Ok(GenericTypeReference::Inference(InferenceTypeParameter {
            issuer: self.issuer,
            slot,
        }))
    }

    pub(in crate::types) fn const_reference(
        &self,
        slot: u16,
    ) -> Result<GenericConstReference, GenericScopeError> {
        check_slot(
            GenericParameterKind::Const,
            u32::from(slot),
            u32::from(self.binder.const_lengths),
        )?;
        Ok(GenericConstReference::Inference(InferenceConstParameter {
            issuer: self.issuer,
            slot,
        }))
    }

    pub(in crate::types) fn effect_reference(
        &self,
        slot: u32,
    ) -> Result<GenericEffectReference, GenericScopeError> {
        check_slot(GenericParameterKind::Effect, slot, self.binder.effects)?;
        Ok(GenericEffectReference::Inference(
            InferenceEffectParameter {
                issuer: self.issuer,
                slot,
            },
        ))
    }
}

fn check_slot(kind: GenericParameterKind, slot: u32, arity: u32) -> Result<(), GenericScopeError> {
    if slot < arity {
        Ok(())
    } else {
        Err(GenericScopeError::UnknownSlot { kind, slot, arity })
    }
}
