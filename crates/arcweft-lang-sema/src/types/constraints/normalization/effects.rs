//! One branch's scoped effect completion and the rows a header uses.

use super::super::{
    OriginalProducerContext,
    application::ImportedGenericParameterScopeLease,
    lexical::ConstraintLexicalScope,
    transaction::{
        ConstraintEffectField, ConstraintEffectHeaderDerivation, ConstraintFunctionHeaderView,
        ConstraintInferenceBranch, ConstraintOperand, ConstraintPath,
    },
};
use super::*;
use crate::effect_row::{
    EffectConstraintCompletion, EffectConstraintEnvironment, EffectPredicate, EffectRow,
};
use crate::types::GenericEffectReference;

pub(in crate::types) struct ConstraintClosedEffects<D: ConstraintDomain> {
    substitution: BTreeMap<GenericEffectReference, ConstraintOperand<D, EffectRow>>,
    predicate: ConstraintOperand<D, EffectPredicate>,
    context: Arc<ConstraintEffectCompletionContext<D>>,
}

/// The real calculation scope and domain survive independently of any one
/// row. The context contains no operands, so primitive origins cannot cycle.
pub(in crate::types::constraints) struct ConstraintEffectCompletionContext<D: ConstraintDomain> {
    constraints: EffectConstraintEnvironment,
    applications: Arc<ConstraintApplicationInventory<D>>,
    scope: Arc<ConstraintLexicalScope>,
    origin: ConstraintEffectCompletionOrigin,
    domain: Arc<EffectPredicate>,
}

/// Move-only issuance couples the computed value to its exact narrowed Γ.
pub(in crate::types::constraints) struct ConstraintEffectPrimitive<D: ConstraintDomain, T> {
    value: Arc<T>,
    context: Arc<ConstraintEffectCompletionContext<D>>,
    support: ImportedGenericParameterScopeLease,
}

impl<D: ConstraintDomain, T> ConstraintEffectPrimitive<D, T> {
    pub(in crate::types::constraints) fn into_parts(
        self,
    ) -> (
        Arc<T>,
        Arc<ConstraintEffectCompletionContext<D>>,
        ImportedGenericParameterScopeLease,
    ) {
        (self.value, self.context, self.support)
    }
}

impl<D: ConstraintDomain> ConstraintEffectCompletionContext<D> {
    pub(in crate::types::constraints) fn scope(&self) -> &Arc<ConstraintLexicalScope> {
        &self.scope
    }

    #[cfg(test)]
    pub(in crate::types::constraints) fn original_context(&self) -> &OriginalProducerContext {
        match &self.origin {
            ConstraintEffectCompletionOrigin::Source(original) => original,
            ConstraintEffectCompletionOrigin::ScopedRelation => {
                panic!("a relation has no source Γ")
            }
        }
    }

    fn support<'a, A: TypeConstraintAccounting>(
        &self,
        references: impl Iterator<Item = &'a GenericEffectReference>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ImportedGenericParameterScopeLease, TypeConstraintError> {
        let mut parameters = BTreeSet::new();
        for reference in references {
            context.enter_node()?;
            match reference {
                GenericEffectReference::Bound(bound) => {
                    self.scope
                        .shape()
                        .bound_effect(bound.depth(), bound.slot())?;
                }
                _ => {
                    parameters.insert(reference.clone().into());
                }
            }
        }
        Ok(self
            .applications
            .import_parameters(&parameters.into_iter().collect::<Vec<_>>())?)
    }

    fn seal<'a, T, A: TypeConstraintAccounting>(
        self: &Arc<Self>,
        value: Arc<T>,
        references: impl Iterator<Item = &'a GenericEffectReference>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ConstraintOperand<D, T>, TypeConstraintError> {
        let support = self.support(references, context)?;
        Ok(ConstraintOperand::from_effect_primitive(
            ConstraintEffectPrimitive {
                value,
                context: Arc::clone(self),
                support,
            },
        ))
    }
}

/// A relation's mathematical completion cannot impersonate a source closure.
/// A relation retains its mathematical domain without inventing a source D.
enum ConstraintEffectCompletionOrigin {
    Source(OriginalProducerContext),
    ScopedRelation,
}

/// A header refers only to the completion rows it actually substituted.
/// Each field retains only the Free references in its own canonical result.
pub(in crate::types::constraints) struct ConstraintClosedEffectHeader<D: ConstraintDomain> {
    pub(in crate::types::constraints) input: ConstraintEffectHeaderDerivation<D>,
    pub(in crate::types::constraints) owner: Arc<ConstraintClosedEffects<D>>,
    pub(in crate::types::constraints) row_keys: Box<[GenericEffectReference]>,
    pub(in crate::types::constraints) predicate_keys: Box<[GenericEffectReference]>,
    row_support: Option<ImportedGenericParameterScopeLease>,
    predicate_support: ImportedGenericParameterScopeLease,
}

impl<D: ConstraintDomain> ConstraintClosedEffectHeader<D> {
    pub(in crate::types::constraints) fn admits(
        &self,
        field: ConstraintEffectField,
        parameter: &super::super::ConstraintGenericParameterId,
    ) -> bool {
        match field {
            ConstraintEffectField::Row => self.row_support.as_ref(),
            ConstraintEffectField::Predicate => Some(&self.predicate_support),
        }
        .is_some_and(|scope| scope.admits_original_parameter(parameter))
    }
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
        let EffectConstraintCompletion {
            substitution,
            predicate,
        } = branch.effects.complete(context)?;
        for _ in branch.effects.original_variables() {
            context.enter_node()?;
        }
        let owner = Arc::new(ConstraintEffectCompletionContext {
            constraints: branch.effects.clone(),
            applications: Arc::clone(&branch.applications),
            scope,
            origin,
            domain: Arc::new(predicate),
        });
        let predicate = owner.seal(Arc::clone(&owner.domain), owner.domain.variables(), context)?;
        let mut rows = BTreeMap::new();
        for (key, row) in substitution.into_rows() {
            let row = Arc::new(row);
            let sealed = owner.seal(Arc::clone(&row), row.variables()?, context)?;
            rows.insert(key, sealed);
        }
        Ok(Arc::new(Self {
            substitution: rows,
            predicate,
            context: owner,
        }))
    }

    pub(in crate::types::constraints) fn require_branch<L>(
        &self,
        branch: &ConstraintInferenceBranch<D, L>,
    ) -> Result<(), TypeConstraintError> {
        if !Arc::ptr_eq(&self.context.applications, &branch.applications)
            || self.context.constraints != branch.effects
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
        if self.context.scope.as_ref() != &path.lineage.scope
            || !matches!(&self.context.origin,
                ConstraintEffectCompletionOrigin::Source(original)
                    if original == &path.lineage.original)
        {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        Ok(())
    }

    pub(in crate::types::constraints) fn predicate(
        &self,
    ) -> &ConstraintOperand<D, EffectPredicate> {
        &self.predicate
    }

    pub(in crate::types::constraints) fn row(
        &self,
        key: &GenericEffectReference,
    ) -> Option<&ConstraintOperand<D, EffectRow>> {
        self.substitution.get(key)
    }

    pub(in crate::types::constraints) fn project_header<A: TypeConstraintAccounting>(
        self: &Arc<Self>,
        header: &ConstraintFunctionHeaderView<'_, D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<
        ConstraintProjectedEffectHeader<Option<ConstraintClosedEffectHeader<D>>>,
        TypeConstraintError,
    > {
        header.admit(context)?;
        self.project_header_values(
            header.lexical_scope(),
            header.row(),
            header.predicate(),
            context,
            |row, predicate, row_keys, predicate_keys, context| {
                let row_support = if row.is_known() {
                    Some(self.header_support(
                        header,
                        ConstraintEffectField::Row,
                        row.variables()?,
                        &row_keys,
                        context,
                    )?)
                } else {
                    None
                };
                let predicate_support = self.header_support(
                    header,
                    ConstraintEffectField::Predicate,
                    predicate.variables(),
                    &predicate_keys,
                    context,
                )?;
                Ok(Some(ConstraintClosedEffectHeader {
                    input: header.derivation(),
                    owner: Arc::clone(self),
                    row_keys: row_keys.into_iter().collect(),
                    predicate_keys: predicate_keys.into_iter().collect(),
                    row_support,
                    predicate_support,
                }))
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
        self.project_header_values(scope, row, predicate, context, |_, _, _, _, _| Ok(()))
    }

    fn header_support<'a, A: TypeConstraintAccounting>(
        &self,
        header: &ConstraintFunctionHeaderView<'_, D>,
        field: ConstraintEffectField,
        references: impl Iterator<Item = &'a GenericEffectReference>,
        used: &BTreeSet<GenericEffectReference>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ImportedGenericParameterScopeLease, TypeConstraintError> {
        let mut parameters = BTreeSet::new();
        for reference in references {
            context.enter_node()?;
            if let GenericEffectReference::Bound(bound) = reference {
                header
                    .lexical_scope()
                    .shape()
                    .bound_effect(bound.depth(), bound.slot())?;
                continue;
            }
            let parameter = reference.clone().into();
            let mut admitted = header.admits_effect_parameter(field, &parameter, context)?;
            if !admitted {
                for key in used {
                    let row = self
                        .substitution
                        .get(key)
                        .ok_or(TypeConstraintInvariant::OriginalProducer)?;
                    if row.admits_effect_parameter(&parameter, context)? {
                        admitted = true;
                        break;
                    }
                }
            }
            // Inference occurrences retain their existing application-owner
            // admission. Free references require this field's derivation.
            if !admitted && matches!(reference, GenericEffectReference::Free(_)) {
                return Err(super::super::references::effect_out_of_scope(reference));
            }
            parameters.insert(parameter);
        }
        Ok(self
            .context
            .applications
            .import_parameters(&parameters.into_iter().collect::<Vec<_>>())?)
    }

    fn project_header_values<A: TypeConstraintAccounting, P>(
        &self,
        scope: &Arc<ConstraintLexicalScope>,
        row: &EffectRow,
        predicate: &EffectPredicate,
        context: &mut TypeConstraintContext<'_, A, D>,
        proof: impl FnOnce(
            &EffectRow,
            &EffectPredicate,
            BTreeSet<GenericEffectReference>,
            BTreeSet<GenericEffectReference>,
            &mut TypeConstraintContext<'_, A, D>,
        ) -> Result<P, TypeConstraintError>,
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
        let proof = proof(&row, &predicate, row_keys, predicate_keys, context)?;
        Ok(ConstraintProjectedEffectHeader::Closed {
            row: Some(row),
            predicate: Some(predicate),
            proof,
        })
    }

    fn replacement<A: TypeConstraintAccounting>(
        &self,
        key: &GenericEffectReference,
        target: &Arc<ConstraintLexicalScope>,
        used: &mut BTreeSet<GenericEffectReference>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectRow, TypeConstraintError> {
        let Some(row) = self.substitution.get(key) else {
            return Ok(EffectRow::open(
                crate::effects::EffectSet::new(),
                key.clone(),
            ));
        };
        used.insert(key.clone());
        if row.shared_scope() == *target {
            return Ok(row.value().clone());
        }
        target
            .transport_row(row.scope(), row.value(), context)
            .map_err(|error| {
                TypeConstraintContext::<A, D>::binding_transport_error(error, key.clone().into())
            })
    }
}
