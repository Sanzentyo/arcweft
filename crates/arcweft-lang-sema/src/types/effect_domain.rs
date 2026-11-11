//! Effect evidence admitted by one phase of the shared semantic type grammar.

use std::{fmt::Debug, hash::Hash, sync::Arc};

use crate::effect_row::{EffectPredicate, EffectRow};

use super::{
    GenericScope, TypeInstantiationError, TypeKind, TypeProjectionControl, TypeProjectionError,
    TypeProjectionNodeKind,
    constraints::{TypeConstraintChildren, TypeConstraintShape},
};

#[cfg(test)]
mod tests;

mod prepared;
pub(crate) use prepared::{PreparedEffects, PreparedTypeEffectProjectionFailure};

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

struct ProjectionFrame<'a, S: TypeEffectDomain, T: TypeEffectDomain> {
    shape: TypeConstraintShape<'a, S>,
    children: TypeConstraintChildren<'a, S>,
    projected: Vec<TypeKind<T>>,
    scope: Arc<GenericScope>,
}

fn copy_scope<C: TypeProjectionControl>(
    scope: &GenericScope,
    binder: Option<super::GenericBinder>,
    control: &mut C,
) -> Result<Arc<GenericScope>, TypeProjectionError<C::Error>> {
    for _ in scope.binders() {
        control
            .visit_binding()
            .map_err(TypeProjectionError::Control)?;
    }
    let scope = if let Some(binder) = binder {
        control
            .visit_binding()
            .map_err(TypeProjectionError::Control)?;
        scope.with_binder(binder)
    } else {
        scope.clone()
    };
    Ok(Arc::new(scope))
}

fn rebuild<S, T, C>(
    shape: TypeConstraintShape<'_, S>,
    children: Vec<TypeKind<T>>,
    scope: &GenericScope,
    depth: u64,
    control: &mut C,
) -> Result<TypeKind<T>, TypeProjectionError<C::Error>>
where
    S: TypeEffectDomain,
    T: TypeEffectDomain,
    C: TypeEffectProjection<S, T>,
{
    shape.rebuild_with(
        children,
        control,
        |control, length| {
            control
                .visit_node(TypeProjectionNodeKind::Const, depth)
                .map_err(TypeProjectionError::Control)?;
            Ok(length.clone())
        },
        |control, row| {
            control
                .visit_node(TypeProjectionNodeKind::Effect, depth)
                .map_err(TypeProjectionError::Control)?;
            control.project_row(row, scope)
        },
        |control, predicate| {
            control
                .visit_node(TypeProjectionNodeKind::Effect, depth)
                .map_err(TypeProjectionError::Control)?;
            control.project_predicate(predicate, scope)
        },
    )
}

impl<S: TypeEffectDomain> TypeKind<S> {
    /// Changes only effect evidence through the one exhaustive structural fold.
    /// Scope copies, nodes and algebra projections are admitted by the owner
    /// before construction. No recursive Rust call stack follows the type.
    pub(in crate::types) fn project_effect_domain<T, C>(
        &self,
        scope: &GenericScope,
        control: &mut C,
    ) -> Result<TypeKind<T>, TypeProjectionError<C::Error>>
    where
        T: TypeEffectDomain,
        C: TypeEffectProjection<S, T>,
    {
        control.check().map_err(TypeProjectionError::Control)?;
        let mut scope = copy_scope(scope, None, control)?;
        let mut frames = Vec::<ProjectionFrame<'_, S, T>>::new();
        let mut current = self;
        loop {
            control.check().map_err(TypeProjectionError::Control)?;
            let depth = u64::try_from(frames.len())
                .map_err(|_| TypeInstantiationError::DepthOverflow)?
                .checked_add(1)
                .ok_or(TypeInstantiationError::DepthOverflow)?;
            control
                .visit_node(TypeProjectionNodeKind::Type, depth)
                .map_err(TypeProjectionError::Control)?;
            let shape = current.constraint_shape();
            let binder = shape.binder();
            if !binder.is_empty() {
                scope = copy_scope(&scope, Some(binder), control)?;
            }
            let mut children = shape.children();
            if let Some(child) = children.next() {
                frames.push(ProjectionFrame {
                    shape,
                    children,
                    projected: Vec::new(),
                    scope: Arc::clone(&scope),
                });
                current = child;
                continue;
            }
            let mut value = rebuild(shape, Vec::new(), &scope, depth, control)?;
            loop {
                let Some(mut frame) = frames.pop() else {
                    return Ok(value);
                };
                frame.projected.push(value);
                if let Some(child) = frame.children.next() {
                    current = child;
                    scope = Arc::clone(&frame.scope);
                    frames.push(frame);
                    break;
                }
                let depth = u64::try_from(frames.len())
                    .map_err(|_| TypeInstantiationError::DepthOverflow)?
                    .checked_add(1)
                    .ok_or(TypeInstantiationError::DepthOverflow)?;
                value = rebuild(frame.shape, frame.projected, &frame.scope, depth, control)?;
            }
        }
    }
}
