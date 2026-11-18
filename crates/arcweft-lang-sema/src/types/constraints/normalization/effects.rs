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
use crate::effect_row::prepared::{
    BoundScopedEffectCompletion, PreparedEffectActualInputs, PreparedEffectGraph,
    PreparedEffectPredicate, PreparedEffectRow, PreparedEffectScopedConstraint,
    PreparedEffectUnknownRef, PrincipalEffectClosure,
};
use crate::effect_row::{EffectConstraintEnvironment, EffectPredicate, EffectRow};
use crate::types::{GenericEffectReference, PreparedEffects, TypeProjectionControl};

mod scoped;

pub(in crate::types) struct ConstraintClosedEffects<D: ConstraintDomain> {
    substitution: BTreeMap<GenericEffectReference, ConstraintEffectBinding<D>>,
    predicate: ConstraintOperand<D, EffectPredicate>,
    context: Arc<ConstraintEffectCompletionContext<D>>,
}

struct ConstraintEffectBinding<D: ConstraintDomain> {
    function: PreparedEffectUnknownRef,
    value: ConstraintOperand<D, EffectRow>,
}

/// The real calculation scope and domain survive independently of any one
/// row. The context contains no operands, so primitive origins cannot cycle.
pub(in crate::types::constraints) struct ConstraintEffectCompletionContext<D: ConstraintDomain> {
    constraints: EffectConstraintEnvironment<PreparedEffects>,
    applications: Arc<ConstraintApplicationInventory<D>>,
    scope: Arc<ConstraintLexicalScope>,
    origin: ConstraintEffectCompletionOrigin,
    domain: Arc<EffectPredicate>,
    principal: Arc<PrincipalEffectClosure>,
    source_constraint: PreparedEffectScopedConstraint,
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
            ConstraintEffectCompletionOrigin::Joint(_) => {
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
    Joint(Arc<[OriginalProducerContext]>),
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

pub(in crate::types::constraints) enum ConstraintProjectedEffectHeader<
    P = (),
    E: crate::types::TypeEffectDomain = crate::types::CanonicalEffects,
> {
    Original,
    Closed {
        row: Option<E::Row>,
        predicate: Option<E::Predicate>,
        proof: P,
    },
}

impl<P, E: crate::types::TypeEffectDomain> ConstraintProjectedEffectHeader<P, E> {
    pub(in crate::types::constraints) fn take_row(&mut self, original: &E::Row) -> E::Row {
        match self {
            Self::Original => original.clone(),
            Self::Closed { row, .. } => row
                .take()
                .expect("the closed function row is consumed once"),
        }
    }

    pub(in crate::types::constraints) fn take_predicate(
        &mut self,
        original: &E::Predicate,
    ) -> E::Predicate {
        match self {
            Self::Original => original.clone(),
            Self::Closed { predicate, .. } => predicate
                .take()
                .expect("the closed function predicate is consumed once"),
        }
    }
}

impl<D: ConstraintDomain, E: crate::types::TypeEffectDomain>
    ConstraintProjectedEffectHeader<Option<ConstraintClosedEffectHeader<D>>, E>
{
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
        original: &BoundScopedEffectCompletion,
        inputs: &[(&PreparedEffectUnknownRef, &EffectRow)],
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Arc<Self>, TypeConstraintError> {
        for _ in path.lineage.scope.shape().binders() {
            context.enter_node()?;
        }
        let inputs = graph.read().actual_inputs(inputs, context)?;
        Self::complete_branch(
            path,
            Arc::new(path.lineage.scope.clone()),
            ConstraintEffectCompletionOrigin::Source(path.lineage.original.clone()),
            original,
            &inputs,
            graph,
            context,
        )
    }

    /// C is interpreted in the real relation context, including a joint
    /// original product. No synthetic source lineage is introduced.
    pub(in crate::types::constraints) fn for_branch<A: TypeConstraintAccounting, L>(
        branch: &ConstraintInferenceBranch<D, L>,
        scope: &ConstraintLexicalScope,
        sources: Arc<[OriginalProducerContext]>,
        original: &BoundScopedEffectCompletion,
        inputs: &PreparedEffectActualInputs<'_>,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Arc<Self>, TypeConstraintError> {
        for _ in scope.shape().binders() {
            context.enter_node()?;
        }
        let scope = Arc::new(scope.clone());
        Self::complete_branch(
            branch,
            scope,
            ConstraintEffectCompletionOrigin::Joint(sources),
            original,
            inputs,
            graph,
            context,
        )
    }

    fn complete_branch<A: TypeConstraintAccounting, L>(
        branch: &ConstraintInferenceBranch<D, L>,
        scope: Arc<ConstraintLexicalScope>,
        origin: ConstraintEffectCompletionOrigin,
        original: &BoundScopedEffectCompletion,
        inputs: &PreparedEffectActualInputs<'_>,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Arc<Self>, TypeConstraintError> {
        context.with_effect_transaction(graph, |context, graph| {
            let sources = match &origin {
                ConstraintEffectCompletionOrigin::Source(source) => std::slice::from_ref(source),
                ConstraintEffectCompletionOrigin::Joint(sources) => sources.as_ref(),
            };
            let mut domains = Vec::new();
            for source in sources {
                context.enter_node()?;
                let domain = scope
                    .transport_predicate(source.gamma(), &source.domain(), context)
                    .map_err(|error| match error {
                        crate::types::TypeProjectionError::Control(error) => error,
                        crate::types::TypeProjectionError::Instantiation(error) => {
                            TypeConstraintInvariant::Instantiation(error).into()
                        }
                    })?;
                context.enter_node()?;
                domains.push(PreparedEffectPredicate::Canonical(domain));
            }
            let domain = graph.and(domains, context)?;
            let source_constraint =
                branch.prepare_effect_constraint_in(&scope, &domain, graph, context)?;
            let mut queries = Vec::new();
            for key in branch.effects.original_variables() {
                context.enter_node()?;
                if !matches!(key, GenericEffectReference::Inference(_)) {
                    continue;
                }
                let function = branch
                    .applications
                    .effect_function(key)
                    .ok_or(TypeConstraintInvariant::OriginalProducer)?
                    .clone();
                original.as_scoped().unknown(&function)?;
                let application = branch
                    .applications
                    .effect_application(key)
                    .ok_or(TypeConstraintInvariant::OriginalProducer)?;
                let mut edit = graph.edit_terms();
                let row = application.transport(&scope, &mut edit, context)?;
                edit.commit()?;
                context.enter_node()?;
                queries.push((key.clone(), function, row));
            }
            let mut ports = Vec::new();
            for (_, _, row) in &queries {
                context.enter_node()?;
                ports.push(row);
            }
            let principal = original.close_original_with_inputs(
                graph.read(),
                &ports,
                &[source_constraint.domain(), source_constraint.predicate()],
                inputs,
                context,
            )?;
            let domain = principal.confirm_scoped_constraint(&source_constraint, context)?;
            let owner = Arc::new(ConstraintEffectCompletionContext {
                constraints: branch.effects.clone(),
                applications: Arc::clone(&branch.applications),
                scope,
                origin,
                domain: Arc::new(domain),
                principal: Arc::new(principal),
                source_constraint,
            });
            let predicate =
                owner.seal(Arc::clone(&owner.domain), owner.domain.variables(), context)?;
            let mut rows = BTreeMap::new();
            for (key, function, query) in queries {
                let row = Arc::new(owner.principal.resolve_row(&query, context)?);
                let value = owner.seal(Arc::clone(&row), row.variables()?, context)?;
                context.enter_node()?;
                rows.insert(key, ConstraintEffectBinding { function, value });
            }
            Ok(Arc::new(Self {
                substitution: rows,
                predicate,
                context: owner,
            }))
        })
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

    pub(in crate::types::constraints) fn calculation_scope(&self) -> &ConstraintLexicalScope {
        &self.context.scope
    }

    pub(in crate::types::constraints) fn row(
        &self,
        key: &GenericEffectReference,
    ) -> Option<&ConstraintOperand<D, EffectRow>> {
        self.substitution.get(key).map(|binding| &binding.value)
    }

    pub(in crate::types::constraints) fn canonicalize<
        C: TypeProjectionControl<Error = TypeConstraintError>,
    >(
        &self,
        ty: &crate::types::TypeKind<PreparedEffects>,
        scope: &crate::types::GenericScope,
        context: &mut C,
    ) -> Result<crate::types::TypeKind, TypeConstraintError> {
        use crate::types::{PreparedTypeEffectProjectionFailure, TypeProjectionError};
        ty.project_canonical_effects(scope, &self.context.principal, context)
            .map_err(|error| match error {
                TypeProjectionError::Instantiation(error) => {
                    TypeConstraintInvariant::Instantiation(error).into()
                }
                TypeProjectionError::Control(PreparedTypeEffectProjectionFailure::Control(
                    error,
                )) => error,
                TypeProjectionError::Control(PreparedTypeEffectProjectionFailure::Effects(
                    error,
                )) => error
                    .map_control(|error| match error {
                        TypeProjectionError::Control(error) => error,
                        TypeProjectionError::Instantiation(error) => {
                            TypeConstraintInvariant::Instantiation(error).into()
                        }
                    })
                    .into(),
            })
    }

    pub(in crate::types::constraints) fn project_header<A: TypeConstraintAccounting>(
        self: &Arc<Self>,
        header: &ConstraintFunctionHeaderView<'_, D>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<
        ConstraintProjectedEffectHeader<Option<ConstraintClosedEffectHeader<D>>, PreparedEffects>,
        TypeConstraintError,
    > {
        header.admit(graph, context)?;
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
        row: &PreparedEffectRow,
        predicate: &PreparedEffectPredicate,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ConstraintProjectedEffectHeader<(), PreparedEffects>, TypeConstraintError> {
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
                    if row.value.admits_effect_parameter(&parameter, context)? {
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
        row: &PreparedEffectRow,
        predicate: &PreparedEffectPredicate,
        context: &mut TypeConstraintContext<'_, A, D>,
        proof: impl FnOnce(
            &EffectRow,
            &EffectPredicate,
            BTreeSet<GenericEffectReference>,
            BTreeSet<GenericEffectReference>,
            &mut TypeConstraintContext<'_, A, D>,
        ) -> Result<P, TypeConstraintError>,
    ) -> Result<ConstraintProjectedEffectHeader<P, PreparedEffects>, TypeConstraintError> {
        let mut row_keys = BTreeSet::new();
        let row = match row {
            PreparedEffectRow::Canonical(row) => row
                .try_substitute_variables(context, &mut |key, context| {
                    self.replacement(key, scope, &mut row_keys, context)
                })?,
            PreparedEffectRow::Term(_) => {
                let uses = self.context.principal.row_function_uses(row, context)?;
                for (key, binding) in &self.substitution {
                    context.enter_node()?;
                    if uses.contains_unknown(&binding.function)? {
                        row_keys.insert(key.clone());
                    }
                }
                self.context.principal.resolve_row(row, context)?
            }
        };
        let mut predicate_keys = BTreeSet::new();
        let predicate = match predicate {
            PreparedEffectPredicate::Canonical(predicate) => predicate
                .try_substitute_variables(context, &mut |key, context| {
                    self.replacement(key, scope, &mut predicate_keys, context)
                })?,
            PreparedEffectPredicate::Term(_) => {
                let uses = self
                    .context
                    .principal
                    .predicate_function_uses(predicate, context)?;
                for (key, binding) in &self.substitution {
                    context.enter_node()?;
                    if uses.contains_unknown(&binding.function)? {
                        predicate_keys.insert(key.clone());
                    }
                }
                self.context
                    .principal
                    .resolve_predicate(predicate, context)?
            }
        };
        let proof = proof(&row, &predicate, row_keys, predicate_keys, context)?;
        Ok(ConstraintProjectedEffectHeader::Closed {
            row: Some(PreparedEffectRow::Canonical(row)),
            predicate: Some(PreparedEffectPredicate::Canonical(predicate)),
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
        if row.value.shared_scope() == *target {
            return Ok(row.value.value().clone());
        }
        target
            .transport_row(row.value.scope(), row.value.value(), context)
            .map_err(|error| {
                TypeConstraintContext::<A, D>::binding_transport_error(error, key.clone().into())
            })
    }
}
