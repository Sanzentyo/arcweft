//! Canonical algebra projection without a selected-call or checked-type seal.

use super::*;
use crate::effect_row::prepared::{PreparedEffectInvariant, PrincipalEffectClosure};
use crate::effect_row::prepared::{
    PreparedEffectReadView, PreparedEffectUnknownRef, ScopedEffectCompletion,
};
use std::sync::Arc;

#[cfg(test)]
mod tests;

impl OriginalTypeEffectProjection {
    pub(crate) fn value(&self) -> &TypeKind {
        &self.value
    }

    pub(crate) fn principal(&self) -> &Arc<PrincipalEffectClosure> {
        &self.principal
    }

    pub(in crate::types) fn into_parts(self) -> (TypeKind, Arc<PrincipalEffectClosure>) {
        (self.value, self.principal)
    }
}

struct Canonicalization<'a, C> {
    principal: Option<&'a PrincipalEffectClosure>,
    control: &'a mut C,
    depth: u64,
}

impl<'a, C: TypeProjectionControl> Canonicalization<'a, C> {
    fn principal(
        &self,
    ) -> Result<
        &'a PrincipalEffectClosure,
        TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>,
    > {
        self.principal.ok_or_else(|| {
            PreparedTypeEffectProjectionFailure::from_effect(
                PreparedEffectInvariant::UnclosedCell.into(),
            )
        })
    }
}

impl<C: TypeProjectionControl> TypeProjectionControl for Canonicalization<'_, C> {
    type Error = PreparedTypeEffectProjectionFailure<C::Error>;

    fn check(&mut self) -> Result<(), Self::Error> {
        self.control
            .check()
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }

    fn visit_node(&mut self, kind: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error> {
        self.control
            .visit_node(kind, depth)
            .map_err(PreparedTypeEffectProjectionFailure::Control)?;
        self.depth = depth;
        Ok(())
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.control
            .visit_binding()
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }
}

impl<C: TypeProjectionControl> TypeEffectProjection<PreparedEffects, CanonicalEffects>
    for Canonicalization<'_, C>
{
    fn project_row(
        &mut self,
        row: &PreparedEffectRow,
        _: &GenericScope,
    ) -> Result<EffectRow, TypeProjectionError<Self::Error>> {
        match row {
            PreparedEffectRow::Canonical(row) => {
                visit_effect_row(self.control, row, self.depth)
                    .map_err(PreparedTypeEffectProjectionFailure::from_projection)?;
                Ok(row.clone())
            }
            PreparedEffectRow::Term(_) => self
                .principal()?
                .resolve_row(
                    row,
                    &mut EffectProjectionControl {
                        control: self.control,
                        depth: self.depth,
                    },
                )
                .map_err(PreparedTypeEffectProjectionFailure::from_effect),
        }
    }

    fn project_predicate(
        &mut self,
        predicate: &PreparedEffectPredicate,
        _: &GenericScope,
    ) -> Result<EffectPredicate, TypeProjectionError<Self::Error>> {
        match predicate {
            PreparedEffectPredicate::Canonical(predicate) => {
                visit_effect_predicate(self.control, predicate, self.depth)
                    .map_err(PreparedTypeEffectProjectionFailure::from_projection)?;
                Ok(predicate.clone())
            }
            PreparedEffectPredicate::Term(_) => self
                .principal()?
                .resolve_predicate(
                    predicate,
                    &mut EffectProjectionControl {
                        control: self.control,
                        depth: self.depth,
                    },
                )
                .map_err(PreparedTypeEffectProjectionFailure::from_effect),
        }
    }
}

impl TypeKind<PreparedEffects> {
    /// Observe this type against the current original relation. Consumers of
    /// the observation must check it again at publication, after C is closed.
    pub(crate) fn observe_canonical_effects<C: TypeProjectionControl>(
        &self,
        scope: &GenericScope,
        graph: PreparedEffectReadView<'_>,
        control: &mut C,
    ) -> Result<TypeKind, TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>> {
        let roots = PreparedTypeEffectRoots::collect([self], std::iter::empty(), control)
            .map_err(PreparedTypeEffectProjectionFailure::from_projection)?;
        let principal = if roots.has_terms() {
            Some(
                graph
                    .observe_principal(
                        roots.rows(),
                        roots.predicates(),
                        &mut EffectProjectionControl { control, depth: 1 },
                    )
                    .map_err(PreparedTypeEffectProjectionFailure::from_effect)?,
            )
        } else {
            None
        };
        self.project_effect_domain(
            scope,
            &mut Canonicalization {
                principal: principal.as_ref(),
                control,
                depth: 1,
            },
        )
    }

    /// Close the immutable original C against actual inputs, then project each
    /// complete field through the common scoped type fold. Query fields extend
    /// reachability only; they never constrain inference to make a use succeed.
    pub(crate) fn close_original_effects<C: TypeProjectionControl>(
        &self,
        scope: &GenericScope,
        original: &ScopedEffectCompletion,
        graph: PreparedEffectReadView<'_>,
        bindings: &[(&PreparedEffectUnknownRef, &EffectRow)],
        control: &mut C,
    ) -> Result<
        OriginalTypeEffectProjection,
        TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>,
    > {
        control.check().map_err(|error| {
            TypeProjectionError::Control(PreparedTypeEffectProjectionFailure::Control(error))
        })?;
        let roots = PreparedTypeEffectRoots::collect([self], std::iter::empty(), control)
            .map_err(PreparedTypeEffectProjectionFailure::from_projection)?;
        let principal = original
            .close_original(
                graph,
                roots.rows(),
                roots.predicates(),
                bindings,
                &mut EffectProjectionControl { control, depth: 1 },
            )
            .map_err(PreparedTypeEffectProjectionFailure::from_effect)?;
        let value = self.project_canonical_effects(scope, &principal, control)?;
        control.visit_binding().map_err(|error| {
            TypeProjectionError::Control(PreparedTypeEffectProjectionFailure::Control(error))
        })?;
        Ok(OriginalTypeEffectProjection {
            value,
            principal: Arc::new(principal),
        })
    }

    /// Project every live term only when it is unconditionally invariant and
    /// representable. Canonical authored annotations, including omitted rows,
    /// retain their original meaning. This algebra transition does not certify
    /// annotation completion, producer selection, scope admission or a checked
    /// semantic identity.
    pub(crate) fn project_canonical_effects<C: TypeProjectionControl>(
        &self,
        scope: &GenericScope,
        principal: &PrincipalEffectClosure,
        control: &mut C,
    ) -> Result<TypeKind, TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>> {
        self.project_effect_domain(
            scope,
            &mut Canonicalization {
                principal: Some(principal),
                control,
                depth: 1,
            },
        )
    }
}
