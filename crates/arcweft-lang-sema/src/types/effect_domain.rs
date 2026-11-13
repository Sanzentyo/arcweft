//! Effect evidence admitted by one phase of the shared semantic type grammar.

use std::{fmt::Debug, hash::Hash};

use crate::effect_row::{EffectPredicate, EffectRow};

use super::{
    ArrayLength, GenericScope, GenericTypeReference, TypeKind, TypeProjectionControl,
    TypeProjectionError, TypeProjectionNodeKind, structural_projection::ScopedTypeProjection,
};

#[cfg(test)]
mod tests;

mod prepared;
pub(crate) use prepared::{
    PreparedEffects, PreparedTypeEffectProjectionFailure, PreparedTypeRetentionFailure,
};

mod sealed {
    pub trait Sealed {}
}

/// Selects the row and predicate evidence carried by a semantic type tree.
///
/// The grammar and every recursive child retain the same domain. Only the
/// canonical domain can issue checked semantic identities and runtime inputs.
pub trait TypeEffectDomain: sealed::Sealed + Clone + Copy + Debug + Eq + Hash {
    type Row: Clone + Debug + Eq + Hash;
    type Predicate: Clone + Debug + Eq + Hash;
}

/// The ordinary effect algebra used by declarations and completed type facts.
///
/// An omitted authored annotation remains an unknown `EffectRow`; membership
/// in this domain alone does not certify that inference has completed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CanonicalEffects;

impl sealed::Sealed for CanonicalEffects {}

impl TypeEffectDomain for CanonicalEffects {
    type Row = EffectRow;
    type Predicate = EffectPredicate;
}

/// The phase owner supplies both algebra transitions under the exact incoming
/// binders. This interface deliberately supplies no unchecked type conversion.
pub(in crate::types) trait TypeEffectProjection<S: TypeEffectDomain, T: TypeEffectDomain>:
    TypeProjectionControl
{
    fn project_row(
        &mut self,
        row: &S::Row,
        scope: &GenericScope,
    ) -> Result<T::Row, TypeProjectionError<Self::Error>>;

    fn project_predicate(
        &mut self,
        predicate: &S::Predicate,
        scope: &GenericScope,
    ) -> Result<T::Predicate, TypeProjectionError<Self::Error>>;
}

struct EffectDomainProjection<'a, C>(&'a mut C);

impl<C: TypeProjectionControl> TypeProjectionControl for EffectDomainProjection<'_, C> {
    type Error = C::Error;
    fn check(&mut self) -> Result<(), Self::Error> {
        self.0.check()
    }
    fn visit_node(&mut self, kind: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error> {
        self.0.visit_node(kind, depth)
    }
    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.0.visit_binding()
    }
}

impl<S, T, C> ScopedTypeProjection<S, T> for EffectDomainProjection<'_, C>
where
    S: TypeEffectDomain,
    T: TypeEffectDomain,
    C: TypeEffectProjection<S, T>,
{
    fn project_type_reference(
        &mut self,
        reference: &GenericTypeReference,
        _: &GenericScope,
        _: &GenericScope,
        _: u64,
    ) -> Result<TypeKind<T>, TypeProjectionError<Self::Error>> {
        Ok(TypeKind::GenericParam(reference.clone()))
    }
    fn project_length(
        &mut self,
        length: &ArrayLength,
        _: &GenericScope,
        _: &GenericScope,
        depth: u64,
    ) -> Result<ArrayLength, TypeProjectionError<Self::Error>> {
        self.0
            .visit_node(TypeProjectionNodeKind::Const, depth)
            .map_err(TypeProjectionError::Control)?;
        Ok(length.clone())
    }
    fn project_row(
        &mut self,
        row: &S::Row,
        source: &GenericScope,
        _: &GenericScope,
        depth: u64,
    ) -> Result<T::Row, TypeProjectionError<Self::Error>> {
        self.0
            .visit_node(TypeProjectionNodeKind::Effect, depth)
            .map_err(TypeProjectionError::Control)?;
        self.0.project_row(row, source)
    }
    fn project_predicate(
        &mut self,
        predicate: &S::Predicate,
        source: &GenericScope,
        _: &GenericScope,
        depth: u64,
    ) -> Result<T::Predicate, TypeProjectionError<Self::Error>> {
        self.0
            .visit_node(TypeProjectionNodeKind::Effect, depth)
            .map_err(TypeProjectionError::Control)?;
        self.0.project_predicate(predicate, source)
    }
}

impl<S: TypeEffectDomain> TypeKind<S> {
    /// The phase interprets effect evidence while the common scoped fold owns
    /// every recursive type child and binder. Other scalar ports retain their
    /// original type/const references.
    pub(in crate::types) fn project_effect_domain<T, C>(
        &self,
        scope: &GenericScope,
        control: &mut C,
    ) -> Result<TypeKind<T>, TypeProjectionError<C::Error>>
    where
        T: TypeEffectDomain,
        C: TypeEffectProjection<S, T>,
    {
        self.project_structure(scope, scope, 1, &mut EffectDomainProjection(control))
    }
}
