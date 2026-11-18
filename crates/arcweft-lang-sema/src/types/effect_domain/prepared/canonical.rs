//! Canonical algebra projection without a selected-call or checked-type seal.

use super::*;
use crate::effect_row::prepared::{PreparedEffectInvariant, PrincipalEffectClosure};
use crate::effect_row::prepared::{
    PreparedEffectReadView, PreparedEffectUnknownRef, ScopedEffectCompletion,
};
use crate::effect_row::{DecisionControl, DecisionWork};
use crate::types::TypeInstantiationError;
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

/// Copy admission accounts for canonical decisions and labels before their
/// allocation. A decision root stays at header depth; stored atoms are one
/// level below it.
struct EffectCopyControl<'a, C> {
    control: &'a mut C,
    depth: u64,
}

impl<C: TypeProjectionControl> DecisionControl for EffectCopyControl<'_, C> {
    type Error = TypeProjectionError<C::Error>;

    fn charge(&mut self, work: DecisionWork) -> Result<(), Self::Error> {
        self.control.check().map_err(TypeProjectionError::Control)?;
        let depth = match work {
            DecisionWork::Visit => self.depth,
            DecisionWork::Node => self
                .depth
                .checked_add(1)
                .ok_or(TypeInstantiationError::DepthOverflow)?,
        };
        self.control
            .visit_node(TypeProjectionNodeKind::Effect, depth)
            .map_err(TypeProjectionError::Control)
    }
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
            PreparedEffectRow::Canonical(row) => row
                .copy_with_control(&mut EffectCopyControl {
                    control: self.control,
                    depth: self.depth,
                })
                .map_err(PreparedTypeEffectProjectionFailure::from_projection),
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
            PreparedEffectPredicate::Canonical(predicate) => predicate
                .copy_with_control(&mut EffectCopyControl {
                    control: self.control,
                    depth: self.depth,
                })
                .map_err(PreparedTypeEffectProjectionFailure::from_projection),
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

fn observe_type_roots<'a, C: TypeProjectionControl>(
    types: impl IntoIterator<Item = &'a TypeKind<PreparedEffects>>,
    graph: PreparedEffectReadView<'_>,
    control: &mut C,
) -> Result<
    Option<PrincipalEffectClosure>,
    TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>,
> {
    let roots = PreparedTypeEffectRoots::collect(types, std::iter::empty(), control)
        .map_err(PreparedTypeEffectProjectionFailure::from_projection)?;
    if !roots.has_terms() {
        return Ok(None);
    }
    graph
        .observe_principal(
            roots.rows(),
            roots.predicates(),
            &mut EffectProjectionControl { control, depth: 1 },
        )
        .map(Some)
        .map_err(|error| PreparedTypeEffectProjectionFailure::from_effect(error).into())
}

impl TypeKind<PreparedEffects> {
    /// Observe authored annotations without interpreting any live graph term.
    /// A caller must retain the original type when the comparison needs C.
    pub(crate) fn project_effect_annotations<C: TypeProjectionControl>(
        &self,
        scope: &GenericScope,
        control: &mut C,
    ) -> Result<Option<TypeKind>, TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>>
    {
        let roots = PreparedTypeEffectRoots::collect([self], std::iter::empty(), control)
            .map_err(PreparedTypeEffectProjectionFailure::from_projection)?;
        if roots.has_terms() {
            return Ok(None);
        }
        self.project_effects_with_principal(scope, None, control)
            .map(Some)
    }

    /// Observe this type against the current original relation. Consumers of
    /// the observation must check it again at publication, after C is closed.
    pub(crate) fn observe_canonical_effects<C: TypeProjectionControl>(
        &self,
        scope: &GenericScope,
        graph: PreparedEffectReadView<'_>,
        control: &mut C,
    ) -> Result<TypeKind, TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>> {
        let principal = observe_type_roots([self], graph, control)?;
        self.project_effects_with_principal(scope, principal.as_ref(), control)
    }

    /// Observe both sides in one principal. Preparing a dependency needed by
    /// either side requires retrying the pair from the same graph revision.
    pub(crate) fn observe_compatibility_inputs<C: TypeProjectionControl>(
        &self,
        actual: &Self,
        scope: &GenericScope,
        graph: PreparedEffectReadView<'_>,
        control: &mut C,
    ) -> Result<
        (TypeKind, TypeKind),
        TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>,
    > {
        let principal = observe_type_roots([self, actual], graph, control)?;
        let expected = self.project_effects_with_principal(scope, principal.as_ref(), control)?;
        let actual = actual.project_effects_with_principal(scope, principal.as_ref(), control)?;
        Ok((expected, actual))
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
        self.project_effects_with_principal(scope, Some(principal), control)
    }

    fn project_effects_with_principal<C: TypeProjectionControl>(
        &self,
        scope: &GenericScope,
        principal: Option<&PrincipalEffectClosure>,
        control: &mut C,
    ) -> Result<TypeKind, TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>> {
        self.project_effect_domain(
            scope,
            &mut Canonicalization {
                principal,
                control,
                depth: 1,
            },
        )
    }
}
