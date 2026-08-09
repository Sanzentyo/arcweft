//! Final semantic type records owned by the qualified HIR arena.
//!
//! Type payloads retain root-preserving paths, typed semantic components, and
//! qualified child IDs only. The lowering transaction supplies the resolver
//! used here so construction proves child liveness without retaining syntax or
//! reopening source text.

use std::sync::Arc;

use thiserror::Error;

use crate::expr::{HirBorrowKind, HirPoisonState, HirRecoveryIssue};
use crate::identity::{HirModuleId, ScopeId, TypeId};
use crate::leaf::{HirName, HirPath, HirTypeRegion, HirTypeRegionIssue};

/// Validated semantic spelling of one declared function-type effect.
///
/// This payload belongs to the function-type owner. Project callable source
/// coordinates are retained independently by the final HIR source index.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirEffectName(Arc<str>);

/// Invalid semantic effect spelling in a function type.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirEffectNameError {
    #[error("HIR effect name cannot be empty")]
    Empty,
    #[error("HIR effect name contains a control character at byte {byte}")]
    Control { byte: usize },
}

impl HirEffectName {
    pub fn try_new(value: impl Into<Arc<str>>) -> Result<Self, HirEffectNameError> {
        let value = value.into();
        if value.is_empty() {
            return Err(HirEffectNameError::Empty);
        }
        if let Some((byte, _)) = value
            .char_indices()
            .find(|(_, character)| character.is_control())
        {
            return Err(HirEffectNameError::Control { byte });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Transaction-owned typed lookup required to construct one type record.
///
/// Implementations include both previously committed IDs and IDs reserved by
/// the current all-or-nothing lowering transaction. A successful lookup proves
/// that a child type is live and visible from `scope`.
pub(crate) trait HirTypeResolver {
    fn scope_is_live(&self, scope: ScopeId) -> bool;

    fn resolve_type(&self, scope: ScopeId, ty: TypeId) -> Option<&HirType>;
}

/// One immutable type-arena record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirType {
    kind: HirTypeKind,
    scope: ScopeId,
    state: HirPoisonState,
}

impl HirType {
    pub(crate) fn try_new<R: HirTypeResolver + ?Sized>(
        owner: TypeId,
        kind: HirTypeKind,
        scope: ScopeId,
        state: HirPoisonState,
        resolver: &R,
    ) -> Result<Self, HirTypeInvariantError> {
        let expected = owner.module();
        let actual = scope.module();
        if actual != expected {
            return Err(HirTypeInvariantError::ForeignScope { expected, actual });
        }
        if !resolver.scope_is_live(scope) {
            return Err(HirTypeInvariantError::ScopeNotLive { scope });
        }
        kind.validate(owner, scope, resolver)?;
        if matches!(state, HirPoisonState::Clean) && kind.contains_recovery_payload() {
            return Err(HirTypeInvariantError::CleanRecoveryPayload);
        }
        match (&kind, &state) {
            (
                HirTypeKind::Recovery(payload),
                HirPoisonState::Poisoned(crate::expr::HirRecoveryIssue::InvalidType(issue)),
            ) if payload.issue() == *issue => {}
            (HirTypeKind::Recovery(_), HirPoisonState::Poisoned(_)) => {
                return Err(HirTypeInvariantError::RecoveryIssueMismatch);
            }
            (_, HirPoisonState::Poisoned(crate::expr::HirRecoveryIssue::InvalidType(_))) => {
                return Err(HirTypeInvariantError::UnexpectedGenericRecoveryIssue);
            }
            _ => {}
        }
        let invalid_named_region =
            HirRecoveryIssue::InvalidTypeRegion(HirTypeRegionIssue::InvalidNamedRegion);
        match (&kind, &state) {
            (HirTypeKind::Reference(reference), HirPoisonState::Poisoned(issue))
                if reference.region().is_none() && *issue == invalid_named_region => {}
            (HirTypeKind::Reference(reference), _) if reference.region().is_none() => {
                return Err(
                    HirTypeInvariantError::MissingReferenceRegionRequiresInvalidNamedRegionPoison,
                );
            }
            (_, HirPoisonState::Poisoned(issue)) if *issue == invalid_named_region => {
                return Err(
                    HirTypeInvariantError::InvalidNamedRegionPoisonRequiresMissingReferenceRegion,
                );
            }
            _ => {}
        }
        Ok(Self { kind, scope, state })
    }

    /// Returns the exact final semantic type payload.
    pub const fn kind(&self) -> &HirTypeKind {
        &self.kind
    }

    /// Returns the lexical scope inherited by this type node.
    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    /// Returns the semantic recovery state retained with this type node.
    pub const fn state(&self) -> &HirPoisonState {
        &self.state
    }

    pub const fn is_poisoned(&self) -> bool {
        self.state.is_poisoned()
    }
}

impl crate::arena::HirArenaPayload for HirType {
    fn is_poisoned(&self) -> bool {
        self.is_poisoned()
    }
}

/// The exact final semantic projection of the attached `TypeRef` inventory.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirTypeKind {
    Never,
    ConstInt(usize),
    Path(HirPath),
    Tuple(Box<[TypeId]>),
    Function(HirFunctionType),
    Choice(Box<[TypeId]>),
    Generic(HirGenericType),
    TraitBound(HirTraitBoundType),
    Projection(HirProjectionType),
    Reference(HirReferenceType),
    Slice(TypeId),
    Recovery(HirTypeError),
}

impl HirTypeKind {
    /// Returns whether this is the canonical zero-element tuple (`Unit`).
    pub const fn is_unit(&self) -> bool {
        matches!(self, Self::Tuple(elements) if elements.is_empty())
    }

    /// Returns the same-arena type nodes owned directly by this payload.
    ///
    /// Nominal resolution uses this structural ownership to identify authored
    /// type roots. Contextual children such as the `Flow` in `Ref<Flow>` are
    /// arena nodes with source identity, but they are deliberately not
    /// standalone runtime types.
    pub fn direct_type_children(&self) -> Vec<TypeId> {
        match self {
            Self::Tuple(elements) | Self::Choice(elements) => elements.to_vec(),
            Self::Function(function) => function
                .parameters()
                .iter()
                .copied()
                .chain(std::iter::once(function.return_type()))
                .collect(),
            Self::Generic(generic) => generic.arguments().to_vec(),
            Self::TraitBound(bound) => bound
                .arguments()
                .iter()
                .copied()
                .chain(
                    bound
                        .associated()
                        .iter()
                        .map(HirAssociatedTypeBinding::value),
                )
                .collect(),
            Self::Projection(projection) => vec![projection.subject()],
            Self::Reference(reference) => vec![reference.referent()],
            Self::Slice(item) => vec![*item],
            Self::Never | Self::ConstInt(_) | Self::Path(_) | Self::Recovery(_) => Vec::new(),
        }
    }

    fn validate<R: HirTypeResolver + ?Sized>(
        &self,
        owner: TypeId,
        scope: ScopeId,
        resolver: &R,
    ) -> Result<(), HirTypeInvariantError> {
        match self {
            Self::Never | Self::ConstInt(_) | Self::Path(_) | Self::Recovery(_) => Ok(()),
            Self::Tuple(elements) | Self::Choice(elements) => {
                validate_types(owner.module(), scope, elements, resolver)
            }
            Self::Function(function) => function.validate(owner.module(), scope, resolver),
            Self::Generic(generic) => {
                validate_types(owner.module(), scope, generic.arguments(), resolver)
            }
            Self::TraitBound(bound) => bound.validate(owner.module(), scope, resolver),
            Self::Projection(projection) => {
                validate_type(owner.module(), scope, projection.subject(), resolver).map(|_| ())
            }
            Self::Reference(reference) => reference.validate(owner, scope, resolver),
            Self::Slice(element) => {
                validate_type(owner.module(), scope, *element, resolver).map(|_| ())
            }
        }
    }

    fn contains_recovery_payload(&self) -> bool {
        matches!(self, Self::Recovery(_))
    }
}

/// Function-type payload with ordered parameters and a closed optional effect row.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirFunctionType {
    parameters: Box<[TypeId]>,
    return_type: TypeId,
    effects: Option<HirTypeEffectRow>,
}

impl HirFunctionType {
    pub(crate) const fn new(
        parameters: Box<[TypeId]>,
        return_type: TypeId,
        effects: Option<HirTypeEffectRow>,
    ) -> Self {
        Self {
            parameters,
            return_type,
            effects,
        }
    }

    /// Returns parameter types in authored order.
    pub fn parameters(&self) -> &[TypeId] {
        &self.parameters
    }

    /// Returns the required function result type.
    pub const fn return_type(&self) -> TypeId {
        self.return_type
    }

    /// Returns the authored closed effect row, preserving absent versus empty.
    pub const fn effects(&self) -> Option<&HirTypeEffectRow> {
        self.effects.as_ref()
    }

    fn validate<R: HirTypeResolver + ?Sized>(
        &self,
        expected: HirModuleId,
        scope: ScopeId,
        resolver: &R,
    ) -> Result<(), HirTypeInvariantError> {
        validate_types(expected, scope, &self.parameters, resolver)?;
        validate_type(expected, scope, self.return_type, resolver).map(|_| ())
    }
}

/// Closed semantic effect row attached to a function type.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirTypeEffectRow {
    effects: Box<[HirEffectName]>,
}

impl HirTypeEffectRow {
    pub(crate) fn new(effects: Vec<HirEffectName>) -> Self {
        Self {
            effects: effects.into_boxed_slice(),
        }
    }

    /// Returns validated effects in authored order.
    pub fn effects(&self) -> &[HirEffectName] {
        &self.effects
    }
}

/// Generic type application with a root-preserving base path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirGenericType {
    base: HirPath,
    arguments: Box<[TypeId]>,
}

impl HirGenericType {
    pub(crate) const fn new(base: HirPath, arguments: Box<[TypeId]>) -> Self {
        Self { base, arguments }
    }

    /// Returns the root-preserving generic base.
    pub const fn base(&self) -> &HirPath {
        &self.base
    }

    /// Returns generic arguments in authored order.
    pub fn arguments(&self) -> &[TypeId] {
        &self.arguments
    }
}

/// Trait-bound type with ordered arguments and associated-type equalities.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirTraitBoundType {
    base: HirPath,
    arguments: Box<[TypeId]>,
    associated: Box<[HirAssociatedTypeBinding]>,
}

impl HirTraitBoundType {
    pub(crate) const fn new(
        base: HirPath,
        arguments: Box<[TypeId]>,
        associated: Box<[HirAssociatedTypeBinding]>,
    ) -> Self {
        Self {
            base,
            arguments,
            associated,
        }
    }

    /// Returns the root-preserving trait path.
    pub const fn base(&self) -> &HirPath {
        &self.base
    }

    /// Returns positional trait arguments in authored order.
    pub fn arguments(&self) -> &[TypeId] {
        &self.arguments
    }

    /// Returns associated-type equalities in authored order.
    pub fn associated(&self) -> &[HirAssociatedTypeBinding] {
        &self.associated
    }

    fn validate<R: HirTypeResolver + ?Sized>(
        &self,
        expected: HirModuleId,
        scope: ScopeId,
        resolver: &R,
    ) -> Result<(), HirTypeInvariantError> {
        validate_types(expected, scope, &self.arguments, resolver)?;
        self.associated.iter().try_for_each(|binding| {
            validate_type(expected, scope, binding.value(), resolver).map(|_| ())
        })
    }
}

/// One typed associated-type equality inside a trait bound.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirAssociatedTypeBinding {
    name: HirName,
    value: TypeId,
}

impl HirAssociatedTypeBinding {
    pub(crate) const fn new(name: HirName, value: TypeId) -> Self {
        Self { name, value }
    }

    /// Returns the associated-type name.
    pub const fn name(&self) -> &HirName {
        &self.name
    }

    /// Returns the same-arena equality value.
    pub const fn value(&self) -> TypeId {
        self.value
    }
}

/// Associated-type projection over a same-arena subject type.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirProjectionType {
    subject: TypeId,
    associated: HirName,
}

impl HirProjectionType {
    pub(crate) const fn new(subject: TypeId, associated: HirName) -> Self {
        Self {
            subject,
            associated,
        }
    }

    /// Returns the projected subject type.
    pub const fn subject(&self) -> TypeId {
        self.subject
    }

    /// Returns the associated-type name.
    pub const fn associated(&self) -> &HirName {
        &self.associated
    }
}

/// Reference type whose region identity is disjoint from runtime registry paths.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirReferenceType {
    kind: HirBorrowKind,
    region: Option<HirTypeRegion>,
    referent: TypeId,
}

impl HirReferenceType {
    pub(crate) const fn new(
        kind: HirBorrowKind,
        region: Option<HirTypeRegion>,
        referent: TypeId,
    ) -> Self {
        Self {
            kind,
            region,
            referent,
        }
    }

    /// Returns shared versus mutable reference semantics.
    pub const fn kind(&self) -> HirBorrowKind {
        self.kind
    }

    /// Returns the HIR type-region identity when semantic construction succeeded.
    pub const fn region(&self) -> Option<&HirTypeRegion> {
        self.region.as_ref()
    }

    /// Returns the same-arena referent type.
    pub const fn referent(&self) -> TypeId {
        self.referent
    }

    fn validate<R: HirTypeResolver + ?Sized>(
        &self,
        owner: TypeId,
        scope: ScopeId,
        resolver: &R,
    ) -> Result<(), HirTypeInvariantError> {
        if let Some(HirTypeRegion::Elided(region)) = &self.region {
            let actual = region.owner_type();
            if actual != owner {
                return Err(HirTypeInvariantError::ElidedRegionOwnerMismatch {
                    expected: owner,
                    actual,
                });
            }
        }
        validate_type(owner.module(), scope, self.referent, resolver).map(|_| ())
    }
}

/// Generic type-family recovery retained only for unclassifiable syntax.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirTypeError {
    issue: HirGenericTypeIssue,
}

impl HirTypeError {
    pub(crate) const fn new(issue: HirGenericTypeIssue) -> Self {
        Self { issue }
    }

    /// Returns the generic recovery cause.
    pub const fn issue(&self) -> HirGenericTypeIssue {
        self.issue
    }
}

/// Recovery causes reserved for syntax outside every known type family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirGenericTypeIssue {
    UnclassifiedSyntax,
    TransactionalChildFailure,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum HirTypeInvariantError {
    #[error("type owner belongs to module {expected:?}, but its scope belongs to {actual:?}")]
    ForeignScope {
        expected: HirModuleId,
        actual: HirModuleId,
    },
    #[error("type scope {scope:?} is not live in the lowering transaction")]
    ScopeNotLive { scope: ScopeId },
    #[error("nested type belongs to module {actual:?}, expected {expected:?}")]
    ForeignType {
        expected: HirModuleId,
        actual: HirModuleId,
    },
    #[error("nested type {ty:?} is not live and visible from scope {scope:?}")]
    TypeNotVisible { scope: ScopeId, ty: TypeId },
    #[error("elided region belongs to type {actual:?}, expected {expected:?}")]
    ElidedRegionOwnerMismatch { expected: TypeId, actual: TypeId },
    #[error("a clean type cannot contain a recovery payload")]
    CleanRecoveryPayload,
    #[error("generic type recovery payload and poison issue disagree")]
    RecoveryIssueMismatch,
    #[error("a non-recovery type cannot carry a generic type recovery issue")]
    UnexpectedGenericRecoveryIssue,
    #[error("a missing reference region requires exact invalid-named-region poison")]
    MissingReferenceRegionRequiresInvalidNamedRegionPoison,
    #[error("invalid-named-region poison requires a missing reference region payload")]
    InvalidNamedRegionPoisonRequiresMissingReferenceRegion,
}

fn validate_types<R: HirTypeResolver + ?Sized>(
    expected: HirModuleId,
    scope: ScopeId,
    types: &[TypeId],
    resolver: &R,
) -> Result<(), HirTypeInvariantError> {
    types
        .iter()
        .try_for_each(|ty| validate_type(expected, scope, *ty, resolver).map(|_| ()))
}

fn validate_type<R: HirTypeResolver + ?Sized>(
    expected: HirModuleId,
    scope: ScopeId,
    ty: TypeId,
    resolver: &R,
) -> Result<&HirType, HirTypeInvariantError> {
    let actual = ty.module();
    if actual != expected {
        return Err(HirTypeInvariantError::ForeignType { expected, actual });
    }
    resolver
        .resolve_type(scope, ty)
        .ok_or(HirTypeInvariantError::TypeNotVisible { scope, ty })
}

#[cfg(test)]
#[path = "type_ref/tests.rs"]
mod tests;
