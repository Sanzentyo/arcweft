//! One iterative structural fold for scoped type and effect projections.

use std::sync::Arc;

use super::{
    ArrayLength, GenericBinder, GenericScope, GenericTypeReference, TypeEffectDomain,
    TypeInstantiationError, TypeKind, TypeProjectionControl, TypeProjectionError,
    TypeProjectionNodeKind,
    constraints::{TypeConstraintChildren, TypeConstraintShape},
};

#[cfg(test)]
mod tests;

/// Phase owners interpret scalar ports; the shared grammar owns recursive
/// structure and the exact source/target scopes under every function binder.
pub(in crate::types) trait ScopedTypeProjection<S: TypeEffectDomain, T: TypeEffectDomain>:
    TypeProjectionControl
{
    fn project_type_reference(
        &mut self,
        reference: &GenericTypeReference,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<TypeKind<T>, TypeProjectionError<Self::Error>>;

    fn project_length(
        &mut self,
        length: &ArrayLength,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<ArrayLength, TypeProjectionError<Self::Error>>;

    fn project_row(
        &mut self,
        row: &S::Row,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<T::Row, TypeProjectionError<Self::Error>>;

    fn project_predicate(
        &mut self,
        predicate: &S::Predicate,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
    ) -> Result<T::Predicate, TypeProjectionError<Self::Error>>;
}

#[derive(Clone)]
enum Scope<'a> {
    Borrowed(&'a GenericScope),
    Extended(Arc<GenericScope>),
}

impl Scope<'_> {
    fn get(&self) -> &GenericScope {
        match self {
            Self::Borrowed(scope) => scope,
            Self::Extended(scope) => scope,
        }
    }

    fn with_binder<C: TypeProjectionControl>(
        &self,
        binder: GenericBinder,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<C::Error>> {
        // Admit every copied binding and the new binding before allocating.
        for _ in self.get().binders() {
            control
                .visit_binding()
                .map_err(TypeProjectionError::Control)?;
        }
        control
            .visit_binding()
            .map_err(TypeProjectionError::Control)?;
        Ok(Self::Extended(Arc::new(self.get().with_binder(binder))))
    }
}

#[derive(Clone)]
struct Scopes<'a> {
    source: Scope<'a>,
    target: Scope<'a>,
}

impl Scopes<'_> {
    fn with_binder<C: TypeProjectionControl>(
        self,
        binder: GenericBinder,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<C::Error>> {
        if binder.is_empty() {
            return Ok(self);
        }
        // Sharing is only a storage optimization for the same borrowed scope;
        // pointer identity never decides a semantic type/effect comparison.
        let same = std::ptr::eq(self.source.get(), self.target.get());
        let source = self.source.with_binder(binder, control)?;
        let target = if same {
            source.clone()
        } else {
            self.target.with_binder(binder, control)?
        };
        Ok(Self { source, target })
    }
}

struct Frame<'ty, 'scope, S: TypeEffectDomain, T: TypeEffectDomain> {
    shape: TypeConstraintShape<'ty, S>,
    children: TypeConstraintChildren<'ty, S>,
    projected: Vec<TypeKind<T>>,
    scopes: Scopes<'scope>,
    depth: u64,
}

impl<S: TypeEffectDomain, T: TypeEffectDomain> Frame<'_, '_, S, T> {
    fn finish<C: ScopedTypeProjection<S, T>>(
        self,
        control: &mut C,
    ) -> Result<TypeKind<T>, TypeProjectionError<C::Error>> {
        let source = self.scopes.source.get();
        let target = self.scopes.target.get();
        self.shape.rebuild_with(
            self.projected,
            control,
            |control, length| control.project_length(length, source, target, self.depth),
            |control, row| control.project_row(row, source, target, self.depth),
            |control, predicate| control.project_predicate(predicate, source, target, self.depth),
        )
    }
}

pub(in crate::types) fn child_depth<E: std::error::Error + 'static>(
    depth: u64,
) -> Result<u64, TypeProjectionError<E>> {
    depth
        .checked_add(1)
        .ok_or_else(|| TypeInstantiationError::DepthOverflow.into())
}

impl<S: TypeEffectDomain> TypeKind<S> {
    /// Replacement values are already interpreted by their owning scalar port.
    /// The fold never descends into them again in the source environment.
    pub(in crate::types) fn project_structure<T, C>(
        &self,
        source: &GenericScope,
        target: &GenericScope,
        depth: u64,
        control: &mut C,
    ) -> Result<TypeKind<T>, TypeProjectionError<C::Error>>
    where
        T: TypeEffectDomain,
        C: ScopedTypeProjection<S, T>,
    {
        control.check().map_err(TypeProjectionError::Control)?;
        let mut frames = Vec::<Frame<'_, '_, S, T>>::new();
        let mut current = (
            self,
            Scopes {
                source: Scope::Borrowed(source),
                target: Scope::Borrowed(target),
            },
            depth,
        );
        loop {
            let (ty, scopes, depth) = current;
            control.check().map_err(TypeProjectionError::Control)?;
            control
                .visit_node(TypeProjectionNodeKind::Type, depth)
                .map_err(TypeProjectionError::Control)?;
            let mut value = if let TypeKind::GenericParam(reference) = ty {
                control.project_type_reference(
                    reference,
                    scopes.source.get(),
                    scopes.target.get(),
                    depth,
                )?
            } else {
                let shape = ty.constraint_shape();
                let mut frame = Frame {
                    shape,
                    children: shape.children(),
                    projected: Vec::new(),
                    scopes: scopes.with_binder(shape.binder(), control)?,
                    depth,
                };
                if let Some(child) = frame.children.next() {
                    current = (child, frame.scopes.clone(), child_depth(depth)?);
                    frames.push(frame);
                    continue;
                }
                frame.finish(control)?
            };
            loop {
                let Some(mut parent) = frames.pop() else {
                    return Ok(value);
                };
                parent.projected.push(value);
                if let Some(child) = parent.children.next() {
                    current = (child, parent.scopes.clone(), child_depth(parent.depth)?);
                    frames.push(parent);
                    break;
                }
                value = parent.finish(control)?;
            }
        }
    }
}
