//! Consuming agreement of correlated original roots, before value-use Fits.

use crate::types::PreparedEffects;

mod closing;
mod effects;
#[cfg(test)]
mod tests;

use super::super::{ConstraintInferenceBranch, DeferredCycleWitness};
use super::*;
use crate::effect_row::{EffectConstraintEnvironment, EffectConstraintVariable};
use crate::types::{
    compatibility::binding_plan::{unify_original_binding, unify_original_length},
    constraints::{
        TypeConstraintInvariant,
        application::ConstraintApplicationInventory,
        lexical::ConstraintLexicalScope,
        normalization::{ConstraintClosedEffects, ConstraintEffectProjection, seal_path},
    },
};

/// Addresses the actual retained path. Diagnostic branches cannot inhabit it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OriginalProductMember {
    pub(super) producer: u64,
    pub(super) frontier: usize,
    pub(super) path: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum OriginalProductCheck {
    BindingScope {
        member: OriginalProductMember,
        parameter: crate::types::constraints::ConstraintGenericParameterId,
    },
    Equation {
        member: OriginalProductMember,
        equation: super::super::ConstraintEquationId,
    },
    TypeBinding {
        member: OriginalProductMember,
        parameter: GenericTypeReference,
    },
    ConstBinding {
        member: OriginalProductMember,
        parameter: GenericConstReference,
    },
    EffectRelation(OriginalProductMember),
    MemberCompletion(OriginalProductMember),
    Completion,
}

#[derive(Debug)]
pub(super) struct OriginalProductRejection {
    pub(super) check: OriginalProductCheck,
    pub(super) reason: TypeConstraintRejection,
}

type JointBranch<D> = ConstraintInferenceBranch<D, Arc<[OriginalProductMember]>>;

pub(super) struct PreparedOriginalProduct<D: ConstraintDomain> {
    pub(super) branch: JointBranch<D>,
    pub(super) effects: crate::types::constraints::normalization::BoundConstraintEffects,
}

pub(super) struct OriginalProductInference<S> {
    pub(super) members: Arc<[OriginalProductMember]>,
    pub(super) scope: ConstraintLexicalScope,
    effect_reads: Option<crate::effect_row::prepared::PreparedEffectReadScope>,
    pub(super) result: Result<Box<[S]>, OriginalProductRejection>,
}

/// Owns the unchanged original producer/source forest and its separate inferred
/// products. No later use is an inference input, selector or applicability fact.
pub(crate) struct OriginalConstraintForest<D: ConstraintDomain, P> {
    pub(super) original: OpenConstraintComponent<D, P>,
    pub(super) products: Box<[OriginalProductInference<PreparedOriginalProduct<D>>]>,
}

pub(super) struct ClosedOriginalProduct<D: ConstraintDomain> {
    pub(super) branch: JointBranch<D>,
    pub(super) effects: Arc<ConstraintClosedEffects<D>>,
    pub(super) completed: Box<[CompletedOriginalMember<D>]>,
}

/// Canonical materialization input remains correlated with the exact original
/// producer/frontier/path. Its joint branch retains proofs, not a second value.
pub(super) struct CompletedOriginalMember<D: ConstraintDomain> {
    pub(super) member: OriginalProductMember,
    pub(super) input: super::super::MaterializationInput<D>,
}

#[cfg(test)]
impl<D: ConstraintDomain> CompletedOriginalMember<D> {
    fn fixture_materialization<A: TypeConstraintAccounting>(
        self,
        graph: &PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<super::super::ConstraintMaterialization<D>, TypeConstraintError> {
        let owner = graph.owner(context)?;
        super::super::ConstraintMaterialization::from_component(self.input, &owner, context)
    }
}

/// Only this phase has checked original scope, binding and equation conditions
/// against actual inputs. A prepared forest cannot impersonate this result.
pub(crate) struct ClosedOriginalConstraintForest<D: ConstraintDomain, P> {
    pub(super) original: OpenConstraintComponent<D, P>,
    pub(super) products: Box<[OriginalProductInference<ClosedOriginalProduct<D>>]>,
}

pub(crate) struct OriginalForestCloseFailure<D: ConstraintDomain, P> {
    error: PreparedValueFailure,
    forest: OriginalConstraintForest<D, P>,
}

impl<D: ConstraintDomain, P> std::fmt::Debug for OriginalForestCloseFailure<D, P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OriginalForestCloseFailure")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl<D: ConstraintDomain, P> OriginalForestCloseFailure<D, P> {
    pub(crate) fn into_parts(self) -> (PreparedValueFailure, OriginalConstraintForest<D, P>) {
        (self.error, self.forest)
    }
}

impl<D: ConstraintDomain, P> OpenConstraintComponent<D, P> {
    /// Enumerates only complete products of real original candidate paths.
    /// Rejected/ambiguous producer states are derived diagnostics, never extra
    /// witnesses with an empty effect row. Original rejections retain addresses.
    pub(crate) fn infer_original_products<A: TypeConstraintAccounting>(
        self,
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut PreparedEffectGraph,
    ) -> Result<OriginalConstraintForest<D, P>, PreparedValueFailure> {
        context.check_cancelled()?;
        graph
            .read()
            .validate_owner(&self.effect_owner)
            .map_err(|error| PreparedValueFailure::Effect(error.into()))?;
        if self.has_live_uses() {
            return Err(PreparedValueFailure::UseState);
        }
        context.with_effect_transaction(graph, |context, graph| {
            self.infer_original_products_in(context, graph)
        })
    }

    fn infer_original_products_in<A: TypeConstraintAccounting>(
        self,
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut PreparedEffectGraph,
    ) -> Result<OriginalConstraintForest<D, P>, PreparedValueFailure> {
        let mut axes = Vec::new();
        for (producer, value) in &self.values {
            context.enter_node()?;
            let ValueDefinition::Constraint { frontiers, .. } = &value.definition else {
                continue;
            };
            let mut members = Vec::new();
            for (frontier, root) in frontiers.iter().enumerate() {
                for path in 0..root.transaction.frontier.len() {
                    context.enter_node()?;
                    members.push(OriginalProductMember {
                        producer: *producer,
                        frontier,
                        path,
                    });
                }
            }
            context.enter_node()?;
            axes.push(members);
        }
        let mut products = Vec::new();
        if axes.iter().all(|axis| !axis.is_empty()) {
            for _ in &axes {
                context.enter_node()?;
            }
            let mut positions = vec![0; axes.len()];
            loop {
                let mut members = Vec::new();
                for (axis, position) in axes.iter().zip(&positions) {
                    context.enter_node()?;
                    members.push(axis[*position]);
                }
                context.enter_node()?;
                let members: Arc<[OriginalProductMember]> = members.into();
                let mut scope = ConstraintLexicalScope::default();
                let mut source_reads = Vec::new();
                for member in members.iter() {
                    let path = self.original_path(*member)?;
                    scope.merge(&path.lineage.scope, context)?;
                    path.applications
                        .merge_lexical_scopes(&mut scope, context)?;
                    for equation in &path.equations {
                        scope.merge(equation.scope(), context)?;
                    }
                    for reads in path.lineage.probe_effect_reads() {
                        context.enter_node()?;
                        source_reads.push(reads);
                    }
                }
                for _ in scope.shape().binders() {
                    context.enter_node()?;
                }
                let inferred = context.with_original_scope(scope.clone(), |context| {
                    context.with_effect_reads(graph, source_reads, |context, graph| {
                        self.infer_original_product(&members, &scope, graph, context)
                    })
                });
                let (result, effect_reads) = match inferred {
                    Ok((solutions, reads)) => (Ok(solutions), reads),
                    Err(OriginalInferenceFailure::Rejected(rejection)) => (Err(rejection), None),
                    Err(OriginalInferenceFailure::Constraint(error)) => return Err(error.into()),
                    Err(OriginalInferenceFailure::Instantiation(error)) => return Err(error.into()),
                    Err(OriginalInferenceFailure::Effect(error)) => return Err(error.into()),
                };
                context.enter_node()?;
                products.push(OriginalProductInference {
                    members,
                    scope,
                    effect_reads,
                    result,
                });
                let mut depth = positions.len();
                while depth > 0 {
                    context.enter_node()?;
                    depth -= 1;
                    positions[depth] += 1;
                    if positions[depth] < axes[depth].len() {
                        break;
                    }
                    positions[depth] = 0;
                }
                if positions.iter().all(|position| *position == 0) {
                    break;
                }
            }
        }
        context.check_cancelled()?;
        Ok(OriginalConstraintForest {
            original: self,
            products: products.into_boxed_slice(),
        })
    }

    fn original_path(
        &self,
        member: OriginalProductMember,
    ) -> Result<&ConstraintPath<D>, TypeConstraintError> {
        let value = self
            .values
            .get(&member.producer)
            .expect("product member is issued from this frozen inventory");
        let ValueDefinition::Constraint { frontiers, .. } = &value.definition else {
            unreachable!("product axes contain only original constraints")
        };
        Ok(&frontiers[member.frontier].transaction.frontier[member.path])
    }

    fn infer_original_product<A: TypeConstraintAccounting>(
        &self,
        members: &Arc<[OriginalProductMember]>,
        scope: &ConstraintLexicalScope,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Box<[PreparedOriginalProduct<D>]>, OriginalInferenceFailure> {
        let mut applications = ConstraintApplicationInventory::empty();
        for member in members.iter() {
            applications.merge_original(&self.original_path(*member)?.applications, context)?;
        }
        let mut variables = BTreeMap::new();
        for member in members.iter() {
            let original_effects = &self.original_path(*member)?.effects;
            for variable in original_effects.original_variables() {
                context.enter_node()?;
                // Captured rigid Γ is owned by the original environment. Its
                // inputs need not be declaration parameters of an application.
                // Inference still follows the merged real application owners.
                let eligibility = if matches!(variable, crate::types::GenericEffectReference::Free(_)) {
                    original_effects.eligibility(variable).filter(|role| *role == crate::effect_row::EffectConstraintEligibility::Rigid)
                } else {
                    applications.effect_eligibility(variable)
                }.ok_or_else(|| crate::types::constraints::effect_invariant(crate::types::constraints::TypeConstraintEffectInvariantKind::ForeignVariable, Some(variable.clone())))?;
                variables.insert(variable.clone(), eligibility);
            }
        }
        let mut rows = Vec::new();
        for (variable, eligibility) in variables {
            context.enter_node()?;
            rows.push(EffectConstraintVariable::new(variable, eligibility));
        }
        let effects = EffectConstraintEnvironment::new_prepared(
            &rows,
            &crate::effect_row::prepared::PreparedEffectPredicate::Canonical(
                crate::effect_row::EffectPredicate::unconstrained(),
            ),
            graph.read(),
            context,
        )?;
        let original_effects = self.prepare_original_effects(members, scope, graph, context)?;
        context.enter_node()?;
        let seed = ConstraintInferenceBranch {
            lineage: Arc::clone(members),
            applications: Arc::new(applications),
            bindings: BTreeMap::new(),
            const_bindings: BTreeMap::new(),
            effects,
            equations: Vec::new(),
            choice_key: Vec::new(),
            deferred_cycles: DeferredCycleWitness::default(),
        };
        let operand_scope = Arc::new(scope.clone());
        let mut branches = vec![seed];
        for member in members.iter() {
            let original = self.original_path(*member)?;
            for (parameter, value) in &original.bindings {
                context.enter_node()?;
                let owner = original
                    .applications
                    .owner_for_parameter(&parameter.clone().into())
                    .ok_or_else(|| {
                        TypeConstraintError::from(TypeConstraintRejection::EscapingInstantiation {
                            parameter: parameter.clone().into(),
                        })
                    })?;
                let reference = super::super::ConstraintOperand::introduced_type(
                    parameter,
                    Arc::clone(&operand_scope),
                    owner,
                    context,
                )?;
                let value = value
                    .transport(&operand_scope, graph, context)
                    .map_err(projection_failure)?;
                let check = OriginalProductCheck::TypeBinding {
                    member: *member,
                    parameter: parameter.clone(),
                };
                let mut next = Vec::new();
                for branch in branches {
                    match unify_original_binding(
                        graph,
                        &reference.view(),
                        &value.view(),
                        branch,
                        context,
                    ) {
                        Ok(paths) => next.extend(paths),
                        Err(error) => return Err(reject(check, error)),
                    }
                }
                if next.is_empty() {
                    return Err(reject(check, TypeConstraintRejection::Mismatch.into()));
                }
                branches = next;
            }
            for (parameter, value) in &original.const_bindings {
                context.enter_node()?;
                let owner = original
                    .applications
                    .owner_for_parameter(&parameter.clone().into())
                    .ok_or_else(|| {
                        TypeConstraintError::from(TypeConstraintRejection::EscapingInstantiation {
                            parameter: parameter.clone().into(),
                        })
                    })?;
                let reference = super::super::ConstraintOperand::introduced_const(
                    parameter,
                    Arc::clone(&operand_scope),
                    owner,
                    context,
                )?;
                let value = value
                    .transport(&operand_scope, context)
                    .map_err(projection_failure)?;
                let check = OriginalProductCheck::ConstBinding {
                    member: *member,
                    parameter: parameter.clone(),
                };
                let mut next = Vec::new();
                for branch in branches {
                    match unify_original_length(&reference.view(), &value.view(), branch, context) {
                        Ok(Some(path)) => next.push(path),
                        Ok(None) => {}
                        Err(error) => return Err(reject(check, error)),
                    }
                }
                if next.is_empty() {
                    return Err(reject(check, TypeConstraintRejection::Mismatch.into()));
                }
                branches = next;
            }
        }
        let mut solutions = Vec::new();
        for branch in branches {
            let cyclic = branch.deferred_cycles.parameters.iter().next().cloned();
            let branch = seal_path(branch, graph, context)
                .map_err(|error| reject(OriginalProductCheck::Completion, error))?;
            if let Some(parameter) = cyclic {
                return Err(reject(
                    OriginalProductCheck::Completion,
                    TypeConstraintRejection::CyclicInstantiation { parameter }.into(),
                ));
            }
            let (relation, constraint) = self.freeze_original_effects(
                &branch,
                members,
                scope,
                &original_effects,
                graph,
                context,
            )?;
            let effects = crate::types::constraints::normalization::PreparedConstraintEffects::new(
                relation, constraint, context,
            )?
            .bind(graph, context)?;
            context
                .validate_type_and_const_completion(&branch)
                .map_err(|error| reject(OriginalProductCheck::Completion, error))?;
            context.enter_node()?;
            solutions.push(PreparedOriginalProduct { branch, effects });
        }
        Ok(solutions.into_boxed_slice())
    }

    fn validate_original_bindings<A: TypeConstraintAccounting>(
        &self,
        branch: &JointBranch<D>,
        members: &[OriginalProductMember],
        scope: &ConstraintLexicalScope,
        projection: &mut ConstraintEffectProjection<D>,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), OriginalInferenceFailure> {
        let project = |projection: &mut ConstraintEffectProjection<D>,
                       ty: &TypeKind<PreparedEffects>,
                       context: &mut TypeConstraintContext<'_, A, D>| {
            projection.project_type(
                ty,
                branch,
                crate::types::constraints::ConstraintClosurePolicy::Hint,
                graph.read(),
                context,
            )
        };
        for member in members {
            let original = self.original_path(*member)?;
            let mut projected_bindings = BTreeMap::new();
            for reference in branch.bindings.keys() {
                if original
                    .applications
                    .parameter_eligibility(reference)
                    .is_none()
                {
                    continue;
                }
                context.enter_node()?;
                let parameter: crate::types::constraints::ConstraintGenericParameterId =
                    reference.clone().into();
                let check = OriginalProductCheck::BindingScope {
                    member: *member,
                    parameter: parameter.clone(),
                };
                let owner = context.binding_scope(original, &parameter)?;
                let projected = project(
                    projection,
                    &TypeKind::GenericParam(reference.clone()),
                    context,
                )?;
                let value = owner
                    .transport_canonical_type(scope, &projected, context)
                    .map_err(|error| {
                        scope_projection_failure(check.clone(), parameter.clone(), error)
                    })?;
                for _ in owner.shape().binders() {
                    context.enter_node()?;
                }
                context
                    .with_original_scope(owner.clone(), |context| {
                        let prepared =
                            value
                                .prepare_effects(owner.shape(), context)
                                .map_err(|error| match error {
                                    crate::types::TypeProjectionError::Control(error) => error,
                                    crate::types::TypeProjectionError::Instantiation(error) => {
                                        TypeConstraintInvariant::Instantiation(error).into()
                                    }
                                })?;
                        crate::types::constraints::validate_type(
                            &prepared,
                            original.projection_view(),
                            graph.read(),
                            context,
                        )
                    })
                    .map_err(|error| scope_constraint_failure(check, parameter, error))?;
                if original.bindings.contains_key(reference) {
                    context.enter_node()?;
                    projected_bindings.insert(reference, projected);
                }
            }
            for parameter in branch.const_bindings.keys() {
                if original
                    .applications
                    .const_parameter_eligibility(parameter)
                    .is_none()
                {
                    continue;
                }
                context.enter_node()?;
                let kind: crate::types::constraints::ConstraintGenericParameterId =
                    parameter.clone().into();
                let check = OriginalProductCheck::BindingScope {
                    member: *member,
                    parameter: kind.clone(),
                };
                let owner = context.binding_scope(original, &kind)?;
                let value = crate::types::constraints::normalization::seal_const(
                    &ArrayLength::Generic(parameter.clone()),
                    branch.projection_view(),
                    &mut std::collections::BTreeSet::new(),
                    context,
                )?;
                let value = owner
                    .transport_length(scope, &value, context)
                    .map_err(|error| {
                        scope_projection_failure(check.clone(), kind.clone(), error)
                    })?;
                if let ArrayLength::Generic(reference) = value
                    && !matches!(reference, GenericConstReference::Bound(_))
                    && original
                        .applications
                        .const_parameter_eligibility(&reference)
                        .is_none()
                {
                    return Err(reject(
                        check,
                        TypeConstraintRejection::EscapingInstantiation { parameter: kind }.into(),
                    ));
                }
            }
            for parameter in original.effects.original_variables() {
                let Some(value) = projection.owner().row(parameter) else {
                    continue;
                };
                context.enter_node()?;
                let kind: crate::types::constraints::ConstraintGenericParameterId =
                    parameter.clone().into();
                let check = OriginalProductCheck::BindingScope {
                    member: *member,
                    parameter: kind.clone(),
                };
                let value = original
                    .lineage
                    .scope
                    .transport_row(scope, value.value(), context)
                    .map_err(|error| {
                        scope_projection_failure(check.clone(), kind.clone(), error)
                    })?;
                original
                    .effects
                    .validate_row(
                        &crate::effect_row::prepared::PreparedEffectRow::Canonical(value),
                        graph.read(),
                        context,
                    )
                    .map_err(TypeConstraintError::from)
                    .map_err(|error| scope_constraint_failure(check, kind, error))?;
            }
            for (parameter, value) in &original.bindings {
                context.enter_node()?;
                let check = OriginalProductCheck::TypeBinding {
                    member: *member,
                    parameter: parameter.clone(),
                };
                let owner = context.binding_scope(original, &parameter.clone().into())?;
                // The scope check already projected bindings retained by
                // this member in this exact joint context.
                let left = match projected_bindings.remove(parameter) {
                    Some(value) => value,
                    None => project(
                        projection,
                        &TypeKind::GenericParam(parameter.clone()),
                        context,
                    )?,
                };
                let right = context.with_original_scope(owner, |context| {
                    let canonical = projection.inspect_operand(
                        value.view(),
                        branch,
                        crate::types::constraints::ConstraintClosurePolicy::Hint,
                        graph.read(),
                        context,
                    )?;
                    scope
                        .transport_canonical_type(value.scope(), &canonical, context)
                        .map_err(projection_failure)
                })?;
                if !crate::types::constraints::completed_types_equal(&left, &right, context)? {
                    return Err(reject(check, TypeConstraintRejection::Mismatch.into()));
                }
            }
            for (parameter, value) in &original.const_bindings {
                context.enter_node()?;
                let check = OriginalProductCheck::ConstBinding {
                    member: *member,
                    parameter: parameter.clone(),
                };
                let owner = context.binding_scope(original, &parameter.clone().into())?;
                let value = scope
                    .transport_length(&owner, value.value(), context)
                    .map_err(projection_failure)?;
                let left = crate::types::constraints::normalization::seal_const(
                    &ArrayLength::Generic(parameter.clone()),
                    branch.projection_view(),
                    &mut std::collections::BTreeSet::new(),
                    context,
                )?;
                let right = crate::types::constraints::normalization::seal_const(
                    &value,
                    branch.projection_view(),
                    &mut std::collections::BTreeSet::new(),
                    context,
                )?;
                if left != right {
                    return Err(reject(check, TypeConstraintRejection::Mismatch.into()));
                }
            }
            for equation in &original.equations {
                context.enter_node()?;
                let check = OriginalProductCheck::Equation {
                    member: *member,
                    equation: equation.ordinal,
                };
                equation
                    .validate_completed(branch, projection, graph.read(), context)
                    .map_err(|error| reject(check, error))?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
enum OriginalInferenceFailure {
    Rejected(OriginalProductRejection),
    Constraint(TypeConstraintError),
    Instantiation(TypeInstantiationError),
    Effect(PreparedEffectFailure<TypeConstraintError>),
}

impl From<PreparedEffectFailure<TypeConstraintError>> for OriginalInferenceFailure {
    fn from(error: PreparedEffectFailure<TypeConstraintError>) -> Self {
        Self::Effect(error)
    }
}

impl From<TypeConstraintError> for OriginalInferenceFailure {
    fn from(error: TypeConstraintError) -> Self {
        Self::Constraint(error)
    }
}

fn reject(check: OriginalProductCheck, error: TypeConstraintError) -> OriginalInferenceFailure {
    match error {
        TypeConstraintError::Rejected(reason) => {
            OriginalInferenceFailure::Rejected(OriginalProductRejection { check, reason })
        }
        error => OriginalInferenceFailure::Constraint(error),
    }
}

fn projection_failure(error: TypeProjectionError<TypeConstraintError>) -> OriginalInferenceFailure {
    match error {
        TypeProjectionError::Control(error) => OriginalInferenceFailure::Constraint(error),
        TypeProjectionError::Instantiation(error) => OriginalInferenceFailure::Instantiation(error),
    }
}

fn scope_projection_failure(
    check: OriginalProductCheck,
    parameter: crate::types::constraints::ConstraintGenericParameterId,
    error: TypeProjectionError<TypeConstraintError>,
) -> OriginalInferenceFailure {
    match error {
        TypeProjectionError::Instantiation(TypeInstantiationError::SpecializationScopeMismatch) => {
            reject(
                check,
                TypeConstraintRejection::EscapingInstantiation { parameter }.into(),
            )
        }
        error => projection_failure(error),
    }
}

fn scope_constraint_failure(
    check: OriginalProductCheck,
    parameter: crate::types::constraints::ConstraintGenericParameterId,
    error: TypeConstraintError,
) -> OriginalInferenceFailure {
    use crate::types::constraints::{
        TypeConstraintEffectInvariant, TypeConstraintEffectInvariantKind, TypeConstraintInvariant,
        TypeConstraintParameterScopeInvariant,
    };
    match error {
        TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(
            TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope { .. }
            | TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope { .. },
        ))
        | TypeConstraintError::Invariant(TypeConstraintInvariant::Effect(
            TypeConstraintEffectInvariant {
                kind: TypeConstraintEffectInvariantKind::ForeignVariable,
                ..
            },
        )) => reject(
            check,
            TypeConstraintRejection::EscapingInstantiation { parameter }.into(),
        ),
        error => reject(check, error),
    }
}
