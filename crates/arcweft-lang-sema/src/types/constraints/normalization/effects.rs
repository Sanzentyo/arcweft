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
    PreparedEffectUnknownRef, PrincipalEffectClosure, ScopedEffectCompletion,
};
use crate::effect_row::{EffectConstraintEnvironment, EffectPredicate, EffectRow};
use crate::types::{GenericEffectReference, PreparedEffects, TypeProjectionControl};

mod scoped;

/// The original relation and its exact source obligation travel together
/// through the bound phase, before the final canonical closure is available.
pub(in crate::types::constraints) struct PreparedConstraintEffects {
    bound: BoundScopedEffectCompletion,
    constraint: PreparedEffectScopedConstraint,
}

impl PreparedConstraintEffects {
    pub(in crate::types::constraints) fn bind<A: TypeConstraintAccounting, D: ConstraintDomain>(
        original: &Arc<ScopedEffectCompletion>,
        constraint: PreparedEffectScopedConstraint,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        context.with_effect_transaction(graph, |context, graph| {
            original.require_retained_constraint(&constraint, context)?;
            let binding = graph.bind_original_scoped(original, context)?;
            let bound = binding.freeze(graph.read(), context)?;
            Ok(Self { bound, constraint })
        })
    }
}

pub(in crate::types) struct ConstraintClosedEffects<D: ConstraintDomain> {
    substitution: BTreeMap<GenericEffectReference, ConstraintEffectBinding<D>>,
    predicate: ConstraintOperand<D, EffectPredicate>,
    context: Arc<ConstraintEffectCompletionContext<D>>,
}

struct ConstraintEffectBinding<D: ConstraintDomain> {
    function: PreparedEffectUnknownRef,
    value: ConstraintOperand<D, EffectRow>,
}

#[derive(Default)]
struct HeaderReplacementUses {
    row: BTreeSet<GenericEffectReference>,
    predicate: BTreeSet<GenericEffectReference>,
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

impl<P> ConstraintProjectedEffectHeader<P, PreparedEffects> {
    fn closed(row: EffectRow, predicate: EffectPredicate, proof: P) -> Self {
        Self::Closed {
            row: Some(PreparedEffectRow::Canonical(row)),
            predicate: Some(PreparedEffectPredicate::Canonical(predicate)),
            proof,
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
    pub(in crate::types::constraints) fn for_original_path<A: TypeConstraintAccounting>(
        path: &ConstraintPath<D>,
        original: &PreparedConstraintEffects,
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
            &original.bound,
            &original.constraint,
            &inputs,
            graph,
            context,
        )
    }

    /// Some fixtures supply a separately bound relation and an additional
    /// path fact to validate against it.
    #[cfg(test)]
    pub(in crate::types::constraints) fn for_path<A: TypeConstraintAccounting>(
        path: &ConstraintPath<D>,
        original: &BoundScopedEffectCompletion,
        inputs: &[(&PreparedEffectUnknownRef, &EffectRow)],
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Arc<Self>, TypeConstraintError> {
        context.with_effect_transaction(graph, |context, graph| {
            let constraint = path.prepare_effect_constraint(graph, context)?;
            for _ in path.lineage.scope.shape().binders() {
                context.enter_node()?;
            }
            let inputs = graph.read().actual_inputs(inputs, context)?;
            Self::complete_branch(
                path,
                Arc::new(path.lineage.scope.clone()),
                ConstraintEffectCompletionOrigin::Source(path.lineage.original.clone()),
                original,
                &constraint,
                &inputs,
                graph,
                context,
            )
        })
    }

    /// C is interpreted in the real relation context, including a joint
    /// original product. No synthetic source lineage is introduced.
    pub(in crate::types::constraints) fn for_branch<A: TypeConstraintAccounting, L>(
        branch: &ConstraintInferenceBranch<D, L>,
        scope: &ConstraintLexicalScope,
        sources: Arc<[OriginalProducerContext]>,
        original: &PreparedConstraintEffects,
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
            &original.bound,
            &original.constraint,
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
        source_constraint: &PreparedEffectScopedConstraint,
        inputs: &PreparedEffectActualInputs<'_>,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Arc<Self>, TypeConstraintError> {
        context.with_effect_transaction(graph, |context, graph| {
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
            let domain = principal.confirm_scoped_constraint(source_constraint, context)?;
            let owner = Arc::new(ConstraintEffectCompletionContext {
                constraints: branch.effects.clone(),
                applications: Arc::clone(&branch.applications),
                scope,
                origin,
                domain: Arc::new(domain),
                principal: Arc::new(principal),
                source_constraint: source_constraint.clone(),
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
        ty.project_canonical_effects(scope, &self.context.principal, context)
            .map_err(TypeConstraintError::from)
    }

    /// Complete an annotation using the principal already proved for this
    /// equation. Compare its value in the same lexical scope as the annotation
    /// and keep the operation charged to the original transaction.
    pub(in crate::types::constraints) fn complete_binding_annotation<
        A: TypeConstraintAccounting,
    >(
        &self,
        annotation: &crate::types::TypeKind<PreparedEffects>,
        (value, value_scope): (&crate::types::TypeKind, &ConstraintLexicalScope),
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<crate::types::TypeKind, TypeConstraintError> {
        let annotation_scope = context.lexical_frames().clone();
        let annotation = self.canonicalize(annotation, annotation_scope.shape(), context)?;
        let value = annotation_scope.transport_canonical_type(value_scope, value, context)?;
        annotation
            .binding_type_with_inferred_effects_with_control(&value, context)?
            .ok_or_else(|| TypeConstraintRejection::Mismatch.into())
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
        let mut uses = HeaderReplacementUses::default();
        let (row, predicate) = self.project_header_values(
            header.lexical_scope(),
            header.row(),
            header.predicate(),
            Some(&mut uses),
            context,
        )?;
        let row_support = if row.is_known() {
            Some(self.header_support(
                header,
                ConstraintEffectField::Row,
                row.variables()?,
                &uses.row,
                context,
            )?)
        } else {
            None
        };
        let predicate_support = self.header_support(
            header,
            ConstraintEffectField::Predicate,
            predicate.variables(),
            &uses.predicate,
            context,
        )?;
        let proof = ConstraintClosedEffectHeader {
            input: header.derivation(),
            owner: Arc::clone(self),
            row_keys: uses.row.into_iter().collect(),
            predicate_keys: uses.predicate.into_iter().collect(),
            row_support,
            predicate_support,
        };
        Ok(ConstraintProjectedEffectHeader::closed(
            row,
            predicate,
            Some(proof),
        ))
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
        let (row, predicate) = self.project_header_values(scope, row, predicate, None, context)?;
        Ok(ConstraintProjectedEffectHeader::closed(row, predicate, ()))
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

    fn project_header_values<A: TypeConstraintAccounting>(
        &self,
        scope: &Arc<ConstraintLexicalScope>,
        row: &PreparedEffectRow,
        predicate: &PreparedEffectPredicate,
        uses: Option<&mut HeaderReplacementUses>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(EffectRow, EffectPredicate), TypeConstraintError> {
        let (mut row_keys, mut predicate_keys) = uses.map_or((None, None), |uses| {
            (Some(&mut uses.row), Some(&mut uses.predicate))
        });
        let row = match row {
            PreparedEffectRow::Canonical(row) => row
                .try_substitute_variables(context, &mut |key, context| {
                    self.replacement(key, scope, row_keys.as_deref_mut(), context)
                })?,
            PreparedEffectRow::Term(_) => {
                if let Some(row_keys) = row_keys {
                    let uses = self.context.principal.row_function_uses(row, context)?;
                    for (key, binding) in &self.substitution {
                        context.enter_node()?;
                        if uses.contains_unknown(&binding.function)? {
                            row_keys.insert(key.clone());
                        }
                    }
                }
                self.context.principal.resolve_row(row, context)?
            }
        };
        let predicate = match predicate {
            PreparedEffectPredicate::Canonical(predicate) => {
                predicate.try_substitute_variables(context, &mut |key, context| {
                    self.replacement(key, scope, predicate_keys.as_deref_mut(), context)
                })?
            }
            PreparedEffectPredicate::Term(_) => {
                if let Some(predicate_keys) = predicate_keys {
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
                }
                self.context
                    .principal
                    .resolve_predicate(predicate, context)?
            }
        };
        Ok((row, predicate))
    }

    fn replacement<A: TypeConstraintAccounting>(
        &self,
        key: &GenericEffectReference,
        target: &Arc<ConstraintLexicalScope>,
        used: Option<&mut BTreeSet<GenericEffectReference>>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectRow, TypeConstraintError> {
        let Some(row) = self.substitution.get(key) else {
            return Ok(EffectRow::open(
                crate::effects::EffectSet::new(),
                key.clone(),
            ));
        };
        if let Some(used) = used {
            used.insert(key.clone());
        }
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
