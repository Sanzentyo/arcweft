//! An exact path's immutable effect completion and the rows a header uses.

use super::super::{
    OriginalProducerContext, lexical::ConstraintLexicalScope, transaction::ConstraintPath,
};
use super::*;
use crate::effect_row::{
    EffectConstraintCompletion, EffectConstraintEnvironment, EffectPredicate, EffectRow,
};
use crate::types::GenericEffectReference;

pub(in crate::types::constraints) struct ConstraintClosedEffects<D: ConstraintDomain> {
    completion: EffectConstraintCompletion,
    premises: EffectConstraintEnvironment,
    applications: Arc<ConstraintApplicationInventory<D>>,
    scope: Arc<ConstraintLexicalScope>,
    original: OriginalProducerContext,
}

/// A header refers only to the completion rows it actually substituted.
/// This retains algebraic derivation, not graph-wide Free authorization.
pub(in crate::types::constraints) struct ConstraintClosedEffectHeader<D: ConstraintDomain> {
    pub(in crate::types::constraints) owner: Arc<ConstraintClosedEffects<D>>,
    pub(in crate::types::constraints) row_keys: Box<[GenericEffectReference]>,
    pub(in crate::types::constraints) predicate_keys: Box<[GenericEffectReference]>,
}

impl<D: ConstraintDomain> ConstraintClosedEffects<D> {
    pub(in crate::types::constraints) fn for_path<A: TypeConstraintAccounting>(
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Arc<Self>, TypeConstraintError> {
        let completion = path.effects.complete(context)?;
        for _ in path.effects.original_variables() {
            context.enter_node()?;
        }
        Ok(Arc::new(Self {
            completion,
            premises: path.effects.clone(),
            applications: Arc::clone(&path.applications),
            scope: Arc::new(path.lineage.scope.clone()),
            original: path.lineage.original.clone(),
        }))
    }

    pub(in crate::types::constraints) fn require_path(
        &self,
        path: &ConstraintPath<D>,
    ) -> Result<(), TypeConstraintError> {
        if !Arc::ptr_eq(&self.applications, &path.applications)
            || self.premises != path.effects
            || self.scope.as_ref() != &path.lineage.scope
            || self.original != path.lineage.original
        {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        Ok(())
    }

    pub(in crate::types::constraints) fn completion(&self) -> &EffectConstraintCompletion {
        &self.completion
    }

    pub(in crate::types::constraints) fn project_header<A: TypeConstraintAccounting>(
        self: &Arc<Self>,
        scope: &Arc<ConstraintLexicalScope>,
        row: &EffectRow,
        predicate: &EffectPredicate,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<
        (
            EffectRow,
            EffectPredicate,
            Option<ConstraintClosedEffectHeader<D>>,
        ),
        TypeConstraintError,
    > {
        let mut row_keys = BTreeSet::new();
        let row = row.try_substitute_variables(context, &mut |key, context| {
            self.replacement(key, scope, &mut row_keys, context)
        })?;
        let mut predicate_keys = BTreeSet::new();
        let predicate = if predicate.variables().next().is_none() {
            predicate.clone()
        } else {
            predicate.try_substitute_variables(context, &mut |key, context| {
                self.replacement(key, scope, &mut predicate_keys, context)
            })?
        };
        let proof = (!row_keys.is_empty() || !predicate_keys.is_empty()).then(|| {
            ConstraintClosedEffectHeader {
                owner: Arc::clone(self),
                row_keys: row_keys.into_iter().collect(),
                predicate_keys: predicate_keys.into_iter().collect(),
            }
        });
        Ok((row, predicate, proof))
    }

    fn replacement<A: TypeConstraintAccounting>(
        &self,
        key: &GenericEffectReference,
        target: &Arc<ConstraintLexicalScope>,
        used: &mut BTreeSet<GenericEffectReference>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectRow, TypeConstraintError> {
        let Some(row) = self.completion.substitution.get(key) else {
            return Ok(EffectRow::open(
                crate::effects::EffectSet::new(),
                key.clone(),
            ));
        };
        used.insert(key.clone());
        if self.scope == *target {
            return Ok(row.clone());
        }
        target
            .transport_row(&self.scope, row, context)
            .map_err(|error| {
                TypeConstraintContext::<A, D>::binding_transport_error(error, key.clone().into())
            })
    }
}
