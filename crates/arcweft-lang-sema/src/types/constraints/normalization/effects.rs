//! One branch's scoped effect completion and the rows a header uses.

use super::super::{
    OriginalProducerContext,
    lexical::ConstraintLexicalScope,
    transaction::{
        ConstraintEffectHeaderDerivation, ConstraintFunctionHeaderView, ConstraintInferenceBranch,
        ConstraintPath,
    },
};
use super::*;
use crate::effect_row::{
    EffectConstraintCompletion, EffectConstraintEnvironment, EffectPredicate, EffectRow,
};
use crate::types::GenericEffectReference;

pub(in crate::types) struct ConstraintClosedEffects<D: ConstraintDomain> {
    completion: EffectConstraintCompletion,
    constraints: EffectConstraintEnvironment,
    applications: Arc<ConstraintApplicationInventory<D>>,
    scope: Arc<ConstraintLexicalScope>,
    origin: ConstraintEffectCompletionOrigin,
}

/// A relation's mathematical completion cannot impersonate a source closure.
/// Neither case authorizes Free references or supplies a declaration's D.
enum ConstraintEffectCompletionOrigin {
    Source(OriginalProducerContext),
    ScopedRelation,
}

/// A header refers only to the completion rows it actually substituted.
/// This retains algebraic derivation, not graph-wide Free authorization.
pub(in crate::types::constraints) struct ConstraintClosedEffectHeader<D: ConstraintDomain> {
    pub(in crate::types::constraints) input: ConstraintEffectHeaderDerivation<D>,
    pub(in crate::types::constraints) owner: Arc<ConstraintClosedEffects<D>>,
    pub(in crate::types::constraints) row_keys: Box<[GenericEffectReference]>,
    pub(in crate::types::constraints) predicate_keys: Box<[GenericEffectReference]>,
}

pub(in crate::types::constraints) enum ConstraintProjectedEffectHeader<P = ()> {
    Original,
    Closed {
        row: Option<EffectRow>,
        predicate: Option<EffectPredicate>,
        proof: P,
    },
}

impl<P> ConstraintProjectedEffectHeader<P> {
    pub(in crate::types::constraints) fn take_row(&mut self, original: &EffectRow) -> EffectRow {
        match self {
            Self::Original => original.clone(),
            Self::Closed { row, .. } => row
                .take()
                .expect("the closed function row is consumed once"),
        }
    }

    pub(in crate::types::constraints) fn take_predicate(
        &mut self,
        original: &EffectPredicate,
    ) -> EffectPredicate {
        match self {
            Self::Original => original.clone(),
            Self::Closed { predicate, .. } => predicate
                .take()
                .expect("the closed function predicate is consumed once"),
        }
    }
}

impl<D: ConstraintDomain> ConstraintProjectedEffectHeader<Option<ConstraintClosedEffectHeader<D>>> {
    /// Publishing the derivation requires consuming both fields through the
    /// corresponding function header rebuild.
    pub(in crate::types::constraints) fn finish(
        self,
    ) -> Result<Option<ConstraintClosedEffectHeader<D>>, TypeConstraintError> {
        match self {
            Self::Original => Ok(None),
            Self::Closed {
                row: None,
                predicate: None,
                proof,
            } => Ok(proof),
            Self::Closed { .. } => Err(TypeConstraintInvariant::OriginalProducer.into()),
        }
    }
}

impl<D: ConstraintDomain> ConstraintClosedEffects<D> {
    pub(in crate::types::constraints) fn for_path<A: TypeConstraintAccounting>(
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Arc<Self>, TypeConstraintError> {
        for _ in path.lineage.scope.shape().binders() {
            context.enter_node()?;
        }
        Self::complete_branch(
            path,
            Arc::new(path.lineage.scope.clone()),
            ConstraintEffectCompletionOrigin::Source(path.lineage.original.clone()),
            context,
        )
    }

    /// C is interpreted in the real relation context, including a joint
    /// original product. No synthetic source lineage is introduced.
    pub(in crate::types) fn for_branch<A: TypeConstraintAccounting, L>(
        branch: &ConstraintInferenceBranch<D, L>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Arc<Self>, TypeConstraintError> {
        let scope = Arc::new(context.capture_lexical_scope()?);
        Self::complete_branch(
            branch,
            scope,
            ConstraintEffectCompletionOrigin::ScopedRelation,
            context,
        )
    }

    fn complete_branch<A: TypeConstraintAccounting, L>(
        branch: &ConstraintInferenceBranch<D, L>,
        scope: Arc<ConstraintLexicalScope>,
        origin: ConstraintEffectCompletionOrigin,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Arc<Self>, TypeConstraintError> {
        let completion = branch.effects.complete(context)?;
        for _ in branch.effects.original_variables() {
            context.enter_node()?;
        }
        Ok(Arc::new(Self {
            completion,
            constraints: branch.effects.clone(),
            applications: Arc::clone(&branch.applications),
            scope,
            origin,
        }))
    }

    pub(in crate::types::constraints) fn require_branch<L>(
        &self,
        branch: &ConstraintInferenceBranch<D, L>,
    ) -> Result<(), TypeConstraintError> {
        if !Arc::ptr_eq(&self.applications, &branch.applications)
            || self.constraints != branch.effects
        {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        Ok(())
    }

    pub(in crate::types::constraints) fn require_path(
        &self,
        path: &ConstraintPath<D>,
    ) -> Result<(), TypeConstraintError> {
        self.require_branch(path)?;
        if self.scope.as_ref() != &path.lineage.scope
            || !matches!(&self.origin,
                ConstraintEffectCompletionOrigin::Source(original)
                    if original == &path.lineage.original)
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
        header: &ConstraintFunctionHeaderView<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<
        ConstraintProjectedEffectHeader<Option<ConstraintClosedEffectHeader<D>>>,
        TypeConstraintError,
    > {
        self.project_header_values(
            header.lexical_scope(),
            header.row(),
            header.predicate(),
            context,
            |row_keys, predicate_keys| {
                (!row_keys.is_empty() || !predicate_keys.is_empty()).then(|| {
                    ConstraintClosedEffectHeader {
                        input: header.derivation(),
                        owner: Arc::clone(self),
                        row_keys: row_keys.into_iter().collect(),
                        predicate_keys: predicate_keys.into_iter().collect(),
                    }
                })
            },
        )
    }

    /// Inspection returns values only; it cannot issue a retained derivation.
    pub(in crate::types::constraints::normalization) fn project_inspected_header<
        A: TypeConstraintAccounting,
    >(
        &self,
        scope: &Arc<ConstraintLexicalScope>,
        row: &EffectRow,
        predicate: &EffectPredicate,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ConstraintProjectedEffectHeader, TypeConstraintError> {
        self.project_header_values(scope, row, predicate, context, |_, _| ())
    }

    fn project_header_values<A: TypeConstraintAccounting, P>(
        &self,
        scope: &Arc<ConstraintLexicalScope>,
        row: &EffectRow,
        predicate: &EffectPredicate,
        context: &mut TypeConstraintContext<'_, A, D>,
        proof: impl FnOnce(BTreeSet<GenericEffectReference>, BTreeSet<GenericEffectReference>) -> P,
    ) -> Result<ConstraintProjectedEffectHeader<P>, TypeConstraintError> {
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
        Ok(ConstraintProjectedEffectHeader::Closed {
            row: Some(row),
            predicate: Some(predicate),
            proof: proof(row_keys, predicate_keys),
        })
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
