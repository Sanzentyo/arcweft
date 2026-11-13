//! Consuming agreement of correlated original roots, before value-use Fits.

#[cfg(test)]
mod tests;

use super::super::{ConstraintInferenceBranch, DeferredCycleWitness};
use super::*;
use crate::effect_row::{EffectConstraintEnvironment, EffectConstraintVariable};
use crate::types::{
    compatibility::binding_plan::{unify_original_binding, unify_original_length},
    constraints::{
        application::ConstraintApplicationInventory,
        lexical::ConstraintLexicalScope,
        normalization::{ConstraintClosedEffects, project_closed_type, seal_path},
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
    Completion,
}

#[derive(Debug)]
pub(super) struct OriginalProductRejection {
    pub(super) check: OriginalProductCheck,
    pub(super) reason: TypeConstraintRejection,
}

type JointBranch<D> = ConstraintInferenceBranch<D, Arc<[OriginalProductMember]>>;

pub(super) struct OriginalProductSolution<D: ConstraintDomain> {
    pub(super) branch: JointBranch<D>,
    pub(super) effects: Arc<ConstraintClosedEffects<D>>,
}

pub(super) struct OriginalProductInference<D: ConstraintDomain> {
    pub(super) members: Arc<[OriginalProductMember]>,
    pub(super) scope: ConstraintLexicalScope,
    pub(super) result: Result<Box<[OriginalProductSolution<D>]>, OriginalProductRejection>,
}

/// Owns the unchanged original producer/source forest and its separate inferred
/// products. No later use is an inference input, selector or applicability fact.
pub(crate) struct OriginalConstraintForest<D: ConstraintDomain, P> {
    pub(super) original: OpenConstraintComponent<D, P>,
    pub(super) products: Box<[OriginalProductInference<D>]>,
}

impl<D: ConstraintDomain, P> OpenConstraintComponent<D, P> {
    /// Enumerates only complete products of real original candidate paths.
    /// Rejected/ambiguous producer states are derived diagnostics, never extra
    /// witnesses with an empty effect row. Original rejections retain addresses.
    pub(crate) fn infer_original_products<A: TypeConstraintAccounting>(
        self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<OriginalConstraintForest<D, P>, PreparedValueFailure> {
        context.check_cancelled()?;
        if self.has_live_uses() {
            return Err(PreparedValueFailure::UseState);
        }
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
                for member in members.iter() {
                    let path = self.original_path(*member)?;
                    scope.merge(&path.lineage.scope, context)?;
                    path.applications
                        .merge_lexical_scopes(&mut scope, context)?;
                    for equation in &path.equations {
                        scope.merge(equation.scope(), context)?;
                    }
                }
                for _ in scope.shape().binders() {
                    context.enter_node()?;
                }
                let result = match context.with_original_scope(scope.clone(), |context| {
                    self.infer_original_product(&members, &scope, context)
                }) {
                    Ok(solutions) => Ok(solutions),
                    Err(OriginalInferenceFailure::Rejected(rejection)) => Err(rejection),
                    Err(OriginalInferenceFailure::Constraint(error)) => return Err(error.into()),
                    Err(OriginalInferenceFailure::Instantiation(error)) => return Err(error.into()),
                };
                context.enter_node()?;
                products.push(OriginalProductInference {
                    members,
                    scope,
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
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Box<[OriginalProductSolution<D>]>, OriginalInferenceFailure> {
        let mut applications = ConstraintApplicationInventory::empty();
        for member in members.iter() {
            applications.merge_original(&self.original_path(*member)?.applications, context)?;
        }
        let mut variables = BTreeMap::new();
        for member in members.iter() {
            for variable in self.original_path(*member)?.effects.original_variables() {
                context.enter_node()?;
                let eligibility = applications.effect_eligibility(variable).ok_or_else(|| crate::types::constraints::effect_invariant(crate::types::constraints::TypeConstraintEffectInvariantKind::ForeignVariable, Some(variable.clone())))?;
                variables.insert(variable.clone(), eligibility);
            }
        }
        let mut rows = Vec::new();
        for (variable, eligibility) in variables {
            context.enter_node()?;
            rows.push(EffectConstraintVariable::new(variable, eligibility));
        }
        let mut effects =
            EffectConstraintEnvironment::new(&rows).map_err(TypeConstraintError::from)?;
        for member in members.iter() {
            let path = self.original_path(*member)?;
            let predicate = scope
                .transport_predicate(
                    &path.lineage.scope,
                    path.effects.original_predicate(),
                    context,
                )
                .map_err(projection_failure)?;
            effects
                .restore_predicate(&predicate, context)
                .map_err(|error| reject(OriginalProductCheck::EffectRelation(*member), error))?;
        }
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
                    .transport(&operand_scope, context)
                    .map_err(projection_failure)?;
                let check = OriginalProductCheck::TypeBinding {
                    member: *member,
                    parameter: parameter.clone(),
                };
                let mut next = Vec::new();
                for branch in branches {
                    match unify_original_binding(&reference.view(), &value.view(), branch, context)
                    {
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
            let branch = seal_path(branch, context)
                .map_err(|error| reject(OriginalProductCheck::Completion, error))?;
            if let Some(parameter) = cyclic {
                return Err(reject(
                    OriginalProductCheck::Completion,
                    TypeConstraintRejection::CyclicInstantiation { parameter }.into(),
                ));
            }
            let effects = ConstraintClosedEffects::for_branch(&branch, context)
                .map_err(|error| reject(OriginalProductCheck::Completion, error))?;
            self.validate_original_bindings(&branch, members, scope, &effects, context)?;
            context
                .validate_type_and_const_completion(&branch)
                .map_err(|error| reject(OriginalProductCheck::Completion, error))?;
            context.enter_node()?;
            solutions.push(OriginalProductSolution { branch, effects });
        }
        Ok(solutions.into_boxed_slice())
    }

    fn validate_original_bindings<A: TypeConstraintAccounting>(
        &self,
        branch: &JointBranch<D>,
        members: &[OriginalProductMember],
        scope: &ConstraintLexicalScope,
        effects: &Arc<ConstraintClosedEffects<D>>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), OriginalInferenceFailure> {
        let project = |ty: &TypeKind, context: &mut TypeConstraintContext<'_, A, D>| {
            project_closed_type(
                ty,
                branch,
                crate::types::constraints::ConstraintClosurePolicy::Hint,
                effects,
                context,
            )
        };
        for member in members {
            let original = self.original_path(*member)?;
            for parameter in branch.bindings.keys() {
                if original
                    .applications
                    .parameter_eligibility(parameter)
                    .is_none()
                {
                    continue;
                }
                context.enter_node()?;
                let parameter: crate::types::constraints::ConstraintGenericParameterId =
                    parameter.clone().into();
                let check = OriginalProductCheck::BindingScope {
                    member: *member,
                    parameter: parameter.clone(),
                };
                let owner = context.binding_scope(original, &parameter)?;
                let crate::types::constraints::ConstraintGenericParameterId::Type(reference) =
                    &parameter
                else {
                    unreachable!("type key")
                };
                let value = project(&TypeKind::GenericParam(reference.clone()), context)?;
                let value = owner
                    .transport_type(scope, &value, context)
                    .map_err(|error| {
                        scope_projection_failure(check.clone(), parameter.clone(), error)
                    })?;
                for _ in owner.shape().binders() {
                    context.enter_node()?;
                }
                context
                    .with_original_scope(owner, |context| {
                        crate::types::constraints::validate_type(
                            &value,
                            original.projection_view(),
                            context,
                        )
                    })
                    .map_err(|error| scope_constraint_failure(check, parameter, error))?;
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
                let Some(value) = effects.completion().substitution.get(parameter) else {
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
                    .transport_row(scope, value, context)
                    .map_err(|error| {
                        scope_projection_failure(check.clone(), kind.clone(), error)
                    })?;
                original
                    .effects
                    .validate_row(&value)
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
                let value = scope
                    .transport_type(&owner, value.value(), context)
                    .map_err(projection_failure)?;
                let left = project(&TypeKind::GenericParam(parameter.clone()), context)?;
                let right = project(&value, context)?;
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
                let equation = equation
                    .transport(scope, context)
                    .map_err(projection_failure)?;
                equation
                    .validate_completed(branch, effects, context)
                    .map_err(|error| reject(check, error))?;
            }
        }
        Ok(())
    }
}

enum OriginalInferenceFailure {
    Rejected(OriginalProductRejection),
    Constraint(TypeConstraintError),
    Instantiation(TypeInstantiationError),
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
