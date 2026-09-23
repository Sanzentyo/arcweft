//! Opaque completed type-constraint solution authority.
//!
//! Active paths enter through one completion seal. The resulting carrier owns
//! its exact parameter/effect scope, completeness, and canonical rows; a later
//! continuation validates only the monotone scope transition and exact
//! inherited-key join before restoring those rows.

use std::{slice, sync::Arc};

use crate::effect_row::{EffectConstraintEligibility, EffectRow};

use super::super::{
    ArrayLength, GenericConstReference, GenericEffectReference, GenericScope, GenericTypeReference,
    ScopedArrayLengthView, ScopedConstReferenceView, ScopedEffectReferenceView,
    ScopedEffectRowView, ScopedTypeReferenceView, ScopedTypeView, TypeKind,
};
#[cfg(test)]
use super::super::{GenericConstParameterId, GenericTypeParameterId};
use super::context::{CompletedParameterScope, TypeConstraintAccounting, TypeConstraintContext};
use super::normalization::{project_const_argument, project_type, validate_selected_call_self};
use super::{
    ConstraintClosurePolicy, ConstraintDomain, ConstraintPath, TypeConstraintError,
    TypeConstraintInvariant, TypeConstraintParameterEligibility, TypeConstraintRejection,
    TypeConstraintShape,
};

mod residual;
use residual::ResidualGenericBinder;
mod instantiation;
mod template;
#[cfg(test)]
mod tests;
pub(crate) use instantiation::ClosedTypeInstantiation;
pub use instantiation::TypeInstantiationError;

#[derive(Debug, Eq, PartialEq)]
struct CheckedTypeArgumentBinding {
    parameter: GenericTypeReference,
    value: TypeKind,
}

#[derive(Debug, Eq, PartialEq)]
struct CheckedConstArgumentBinding {
    parameter: GenericConstReference,
    value: ArrayLength,
}

impl CheckedConstArgumentBinding {
    fn new(parameter: GenericConstReference, value: ArrayLength) -> Self {
        Self { parameter, value }
    }
}

impl CheckedTypeArgumentBinding {
    fn new(parameter: GenericTypeReference, value: TypeKind) -> Self {
        Self { parameter, value }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct CheckedEffectArgumentBinding {
    variable: GenericEffectReference,
    value: EffectRow,
}

impl CheckedEffectArgumentBinding {
    fn new(variable: GenericEffectReference, value: EffectRow) -> Self {
        Self { variable, value }
    }
}

/// Exact scope authority under which one solution was completed.
///
/// Eligibility is retained because it is part of the completeness proof. A
/// later continuation may change eligibility, but must retain the exact type,
/// constant, and effect-variable inventories before it can restore the rows.
#[derive(Debug, Eq, PartialEq)]
struct CompletedTypeConstraintAuthority {
    parameter_scope: CompletedParameterScope,
}

#[derive(Clone, Copy)]
enum CompletedSolutionInput {
    ActivePath,
    #[cfg(test)]
    ClaimedCompleted,
}

impl CompletedSolutionInput {
    const fn requires_canonical_claim(self) -> bool {
        match self {
            Self::ActivePath => false,
            #[cfg(test)]
            Self::ClaimedCompleted => true,
        }
    }
}

/// Sorted, opaque, completed binding solution. It intentionally does not
/// implement `Clone`; sharing is represented by `Arc<TypeConstraintSolution>`
/// only. Production construction is confined to the completion seal below,
/// which proves scope, completeness, and canonicality exactly once.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct TypeConstraintSolution {
    authority: CompletedTypeConstraintAuthority,
    residual: ResidualGenericBinder,
    bindings: Box<[CheckedTypeArgumentBinding]>,
    const_bindings: Box<[CheckedConstArgumentBinding]>,
    effect_bindings: Box<[CheckedEffectArgumentBinding]>,
    effect_predicate: crate::effect_row::EffectPredicate,
}

impl TypeConstraintSolution {
    pub(super) fn equal_with<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        other: &Self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<bool, TypeConstraintError> {
        context.check_cancelled()?;
        // These inventories contain reference identities and eligibility only;
        // their type/effect values are compared through the controlled folds below.
        for solution in [self, other] {
            let scope = &solution.authority.parameter_scope;
            for _ in scope.iter() {
                context.enter_node()?;
            }
            for _ in scope.const_iter() {
                context.enter_node()?;
            }
            for _ in scope.effect_contract().variables() {
                context.enter_node()?;
            }
        }
        if self.authority != other.authority
            || self.residual != other.residual
            || self.bindings.len() != other.bindings.len()
            || self.const_bindings.len() != other.const_bindings.len()
            || self.effect_bindings.len() != other.effect_bindings.len()
        {
            return Ok(false);
        }
        for (left, right) in self.bindings.iter().zip(&other.bindings) {
            context.enter_node()?;
            if left.parameter != right.parameter
                || !super::normalization::completed_types_equal(&left.value, &right.value, context)?
            {
                return Ok(false);
            }
        }
        for (left, right) in self.const_bindings.iter().zip(&other.const_bindings) {
            context.enter_node()?;
            if left != right {
                return Ok(false);
            }
        }
        for (left, right) in self.effect_bindings.iter().zip(&other.effect_bindings) {
            context.enter_node()?;
            if left.variable != right.variable || !left.value.equal_with(&right.value, context)? {
                return Ok(false);
            }
        }
        self.effect_predicate
            .equal_with(&other.effect_predicate, context)
    }

    pub(crate) fn effect_predicate(&self) -> crate::types::ScopedEffectPredicateView<'_> {
        crate::types::ScopedEffectPredicateView::sealed(
            &self.effect_predicate,
            self.residual.scope(),
        )
    }
    pub(crate) fn type_parameter_view<'a>(
        &'a self,
        parameter: &'a GenericTypeReference,
    ) -> ScopedTypeReferenceView<'a> {
        ScopedTypeReferenceView::sealed(parameter, self.authority.parameter_scope.template_scope())
    }

    pub(crate) fn const_parameter_view<'a>(
        &'a self,
        parameter: &'a GenericConstReference,
    ) -> ScopedConstReferenceView<'a> {
        ScopedConstReferenceView::sealed(parameter, self.authority.parameter_scope.template_scope())
    }
    pub(crate) fn has_residual_type(&self, parameter: &GenericTypeReference) -> bool {
        self.residual.contains_type(parameter)
    }

    pub(crate) fn has_residual_const(&self, parameter: &GenericConstReference) -> bool {
        self.residual.contains_const(parameter)
    }

    pub(crate) fn has_residual_effects(&self) -> bool {
        self.residual.binder().effects() != 0
    }

    pub(crate) fn bindings(&self) -> TypeConstraintBindingIter<'_> {
        TypeConstraintBindingIter {
            rows: self.bindings.iter(),
            scope: self.residual.scope(),
            template_scope: self.authority.parameter_scope.template_scope(),
        }
    }

    pub(crate) fn effect_bindings(&self) -> TypeConstraintEffectBindingIter<'_> {
        TypeConstraintEffectBindingIter {
            rows: self.effect_bindings.iter(),
            scope: self.residual.scope(),
            template_scope: self.authority.parameter_scope.template_scope(),
        }
    }

    pub(crate) fn const_bindings(&self) -> TypeConstraintConstBindingIter<'_> {
        TypeConstraintConstBindingIter {
            rows: self.const_bindings.iter(),
            scope: self.residual.scope(),
            template_scope: self.authority.parameter_scope.template_scope(),
        }
    }

    pub(super) fn reify_projection<A: TypeConstraintAccounting, D: ConstraintDomain, P>(
        &self,
        key: Arc<P>,
        ty: &TypeKind,
        path: &ConstraintPath<D>,
        application: super::application::ConstraintApplicationId,
        closure: ConstraintClosurePolicy,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<super::KeyedConstraintProjection<P>, TypeConstraintError> {
        let application =
            path.applications
                .application(application)
                .ok_or(TypeConstraintError::Invariant(
                    super::TypeConstraintInvariant::ParameterScope(
                        super::TypeConstraintParameterScopeInvariant::ApplicationOutOfScope,
                    ),
                ))?;
        let value = self.residual.reify_type(ty, path, application, context)?;
        if closure == ConstraintClosurePolicy::ProjectionFuture {
            self.residual
                .validate_reified_future_type(&value, path, context)?;
        }
        Ok(super::KeyedConstraintProjection::new(
            key,
            value,
            self.residual.scope().clone(),
        ))
    }

    /// Complete and seal one active lower path. No caller may publish or
    /// inherit its rows until this owner has projected the whole path and
    /// checked its scope and completeness.
    pub(super) fn complete_application<A, D>(
        path: &ConstraintPath<D>,
        application: super::application::ConstraintApplicationId,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    {
        let application =
            path.applications
                .application(application)
                .ok_or(TypeConstraintError::Invariant(
                    super::TypeConstraintInvariant::ParameterScope(
                        super::TypeConstraintParameterScopeInvariant::ApplicationOutOfScope,
                    ),
                ))?;
        Self::seal_application(
            path,
            application,
            CompletedSolutionInput::ActivePath,
            context,
        )
    }

    #[cfg(test)]
    pub(crate) fn test_seal_completed<A, D, B>(
        bindings: B,
        path: &mut ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
        B: IntoIterator<Item = (GenericTypeParameterId, TypeKind)>,
    {
        Self::test_seal_completed_with_consts(bindings, std::iter::empty(), path, context)
    }

    #[cfg(test)]
    pub(crate) fn test_seal_completed_with_consts<A, D, B, C>(
        bindings: B,
        const_bindings: C,
        path: &mut ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
        B: IntoIterator<Item = (GenericTypeParameterId, TypeKind)>,
        C: IntoIterator<Item = (GenericConstParameterId, ArrayLength)>,
    {
        let applications = Arc::clone(&path.applications);
        let application = applications.root_scope();
        let bindings = bindings
            .into_iter()
            .map(|(parameter, value)| {
                let reference = application
                    .parameters()
                    .type_reference(&parameter.clone().into())
                    .unwrap_or(GenericTypeReference::Free(parameter));
                let value = context
                    .open_template_type(&value, path, application.id())
                    .map_err(|error| {
                        map_completed_self_error(
                            error,
                            reference.clone().into(),
                            CompletedSolutionInput::ClaimedCompleted,
                        )
                    })?;
                Ok((reference, value))
            })
            .collect::<Result<Vec<_>, TypeConstraintError>>()?;
        let const_bindings = const_bindings
            .into_iter()
            .map(|(parameter, value)| {
                let reference = application
                    .parameters()
                    .const_reference(&parameter.clone().into())
                    .unwrap_or(GenericConstReference::Free(parameter));
                let value = context.open_template_length(&value, path, application.id())?;
                Ok((reference, value))
            })
            .collect::<Result<Vec<_>, TypeConstraintError>>()?;
        for rows in [
            bindings
                .iter()
                .map(|(key, _)| super::ConstraintGenericParameterId::from(key.clone()))
                .collect::<Vec<_>>(),
            const_bindings
                .iter()
                .map(|(key, _)| super::ConstraintGenericParameterId::from(key.clone()))
                .collect::<Vec<_>>(),
        ] {
            if let Some(pair) = rows.windows(2).find(|pair| pair[0] >= pair[1]) {
                return Err(completed_solution_invariant(
                    super::InheritedSolutionInvariantKind::DuplicateOrUnordered,
                    Some(pair[1].clone()),
                ));
            }
        }
        path.bindings = bindings.into_iter().collect();
        path.const_bindings = const_bindings.into_iter().collect();
        Self::seal_application(
            path,
            application,
            CompletedSolutionInput::ClaimedCompleted,
            context,
        )
    }

    fn seal_application<A, D>(
        path: &ConstraintPath<D>,
        application: &super::application::ConstraintApplicationScope<D>,
        input: CompletedSolutionInput,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    {
        let source_bindings = path
            .bindings
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        let source_const_bindings = path
            .const_bindings
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        if let Some(rows) = source_bindings
            .windows(2)
            .find(|rows| rows[0].0 >= rows[1].0)
        {
            return Err(completed_solution_invariant(
                super::InheritedSolutionInvariantKind::DuplicateOrUnordered,
                Some(rows[1].0.clone().into()),
            ));
        }
        if let Some(rows) = source_const_bindings
            .windows(2)
            .find(|rows| rows[0].0 >= rows[1].0)
        {
            return Err(completed_solution_invariant(
                super::InheritedSolutionInvariantKind::DuplicateOrUnordered,
                Some(rows[1].0.clone().into()),
            ));
        }
        let residual = ResidualGenericBinder::for_application(path, application)?;
        let mut bindings = Vec::with_capacity(source_bindings.len());
        for (parameter, value) in source_bindings {
            match context.parameter_eligibility(&parameter, path.projection_view()) {
                None => {
                    return Err(completed_solution_invariant(
                        super::InheritedSolutionInvariantKind::OutOfScope,
                        Some(parameter.clone().into()),
                    ));
                }
                Some(TypeConstraintParameterEligibility::Rigid) => {
                    return Err(completed_solution_invariant(
                        super::InheritedSolutionInvariantKind::RigidBinding,
                        Some(parameter.clone().into()),
                    ));
                }
                Some(
                    TypeConstraintParameterEligibility::Bindable
                    | TypeConstraintParameterEligibility::FutureEligible,
                ) => {}
            }
            let Some(declaration) = application
                .parameters()
                .type_declaration(&parameter)
                .cloned()
            else {
                continue;
            };
            if matches!(
                value.constraint_shape(),
                TypeConstraintShape::Generic(bound) if bound == &parameter
            ) {
                if !input.requires_canonical_claim() {
                    return Err(TypeConstraintRejection::CyclicInstantiation {
                        parameter: parameter.into(),
                    }
                    .into());
                }
                return Err(completed_solution_invariant(
                    super::InheritedSolutionInvariantKind::SelfBinding,
                    Some(parameter.clone().into()),
                ));
            }
            let projected = project_type(
                &value,
                path.projection_view(),
                ConstraintClosurePolicy::SolutionCompletion,
                context,
            )
            .map_err(|error| {
                map_completed_canonical_error(error, parameter.clone().into(), input)
            })?;
            if input.requires_canonical_claim() && projected.value != value {
                return Err(completed_solution_invariant(
                    super::InheritedSolutionInvariantKind::NonCanonical,
                    Some(parameter.clone().into()),
                ));
            }
            validate_selected_call_self(&projected.value, context).map_err(|error| {
                map_completed_self_error(error, parameter.clone().into(), input)
            })?;
            bindings.push((
                declaration,
                residual.reify_type(&projected.value, path, application, context)?,
            ));
        }
        let mut const_bindings = Vec::with_capacity(source_const_bindings.len());
        for (parameter, value) in source_const_bindings {
            match context.const_parameter_eligibility(&parameter, path.projection_view()) {
                None => {
                    return Err(completed_solution_invariant(
                        super::InheritedSolutionInvariantKind::OutOfScope,
                        Some(parameter.clone().into()),
                    ));
                }
                Some(super::TypeConstraintConstEligibility::Rigid) => {
                    return Err(completed_solution_invariant(
                        super::InheritedSolutionInvariantKind::RigidBinding,
                        Some(parameter.clone().into()),
                    ));
                }
                Some(
                    super::TypeConstraintConstEligibility::Bindable
                    | super::TypeConstraintConstEligibility::FutureEligible,
                ) => {}
            }
            let Some(declaration) = application
                .parameters()
                .const_declaration(&parameter)
                .cloned()
            else {
                continue;
            };
            if matches!(&value, ArrayLength::Generic(bound) if bound == &parameter) {
                if !input.requires_canonical_claim() {
                    return Err(TypeConstraintRejection::CyclicInstantiation {
                        parameter: parameter.into(),
                    }
                    .into());
                }
                return Err(completed_solution_invariant(
                    super::InheritedSolutionInvariantKind::SelfBinding,
                    Some(parameter.clone().into()),
                ));
            }
            let projected = project_const_argument(
                &value,
                path.projection_view(),
                ConstraintClosurePolicy::SolutionCompletion,
                context,
            )
            .map_err(|error| {
                map_completed_canonical_error(error, parameter.clone().into(), input)
            })?;
            if input.requires_canonical_claim() && projected != value {
                return Err(completed_solution_invariant(
                    super::InheritedSolutionInvariantKind::NonCanonical,
                    Some(parameter.clone().into()),
                ));
            }
            if matches!(projected, ArrayLength::Error(_) | ArrayLength::Inferred) {
                return Err(completed_solution_invariant(
                    super::InheritedSolutionInvariantKind::Forbidden,
                    Some(parameter.clone().into()),
                ));
            }
            const_bindings.push((
                declaration,
                residual.reify_length(&projected, path, application, context)?,
            ));
        }
        context.validate_type_and_const_completion(path)?;

        let completed_effects = path.effects.complete(context)?;
        let opened_effect_bindings = completed_effects.bindings;
        let effect_predicate = residual.reify_effect_predicate(
            &completed_effects.predicate,
            path,
            application,
            context,
        )?;
        let mut effect_bindings = Vec::new();
        for row in application.effects().variables() {
            if row.eligibility() != EffectConstraintEligibility::Bindable {
                continue;
            }
            let reference = application
                .parameters()
                .effect_reference(row.variable())
                .expect("sealed effect parameter opening");
            let value = opened_effect_bindings
                .iter()
                .find(|(key, _)| key == &reference)
                .map(|(_, value)| value)
                .ok_or_else(|| {
                    super::effect_invariant(
                        super::TypeConstraintEffectInvariantKind::MissingInherited,
                        Some(row.variable().clone()),
                    )
                })?;
            context.validate_effect_row(value, path.projection_view())?;
            let value = residual.reify_effect_row(value, path, application, context)?;
            effect_bindings.push((row.variable().clone(), value));
        }
        Ok(Self {
            residual,
            effect_predicate,
            authority: CompletedTypeConstraintAuthority {
                parameter_scope: application.parameters().completed_contract().clone(),
            },
            bindings: bindings
                .into_iter()
                .map(|(parameter, value)| CheckedTypeArgumentBinding::new(parameter, value))
                .collect(),
            const_bindings: const_bindings
                .into_iter()
                .map(|(parameter, value)| CheckedConstArgumentBinding::new(parameter, value))
                .collect(),
            effect_bindings: effect_bindings
                .into_iter()
                .map(|(variable, value)| CheckedEffectArgumentBinding::new(variable, value))
                .collect(),
        })
    }

    /// Restore a completed solution into the exact next continuation scope.
    /// Internal row validity is not rechecked here: the opaque carrier owns
    /// that proof. This boundary checks only the phase transition (same
    /// inventory and exact required keys) before transferring the rows.
    pub(super) fn restore_inherited_path<A, D>(
        &self,
        application: super::application::ConstraintApplicationId,
        mut path: ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ConstraintPath<D>, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    {
        let applications = Arc::clone(&path.applications);
        let application = applications.require_application(application)?;
        let parameters = application.parameters();
        let effects = application.effects();
        if !self
            .authority
            .parameter_scope
            .accepts_continuation_scope(parameters.completed_contract())
        {
            if let Some((parameter, _)) = self.bindings().find(|(parameter, _)| {
                matches!(
                    context.parameter_eligibility(parameter.value(), path.projection_view()),
                    Some(TypeConstraintParameterEligibility::Rigid)
                )
            }) {
                return Err(completed_solution_invariant(
                    super::InheritedSolutionInvariantKind::RigidBinding,
                    Some(parameter.value().clone().into()),
                ));
            }
            if let Some((parameter, _)) = self.const_bindings().find(|(parameter, _)| {
                matches!(
                    context.const_parameter_eligibility(parameter.value(), path.projection_view()),
                    Some(super::TypeConstraintConstEligibility::Rigid)
                )
            }) {
                return Err(completed_solution_invariant(
                    super::InheritedSolutionInvariantKind::RigidBinding,
                    Some(parameter.value().clone().into()),
                ));
            }
            let parameter = self
                .bindings()
                .find_map(|(parameter, _)| {
                    parameters
                        .type_reference(parameter.value())
                        .is_none()
                        .then(|| parameter.value().clone().into())
                })
                .or_else(|| {
                    self.const_bindings().find_map(|(parameter, _)| {
                        parameters
                            .const_reference(parameter.value())
                            .is_none()
                            .then(|| parameter.value().clone().into())
                    })
                })
                .or_else(|| {
                    parameters
                        .required_inherited_keys()
                        .first()
                        .cloned()
                        .map(Into::into)
                })
                .or_else(|| {
                    parameters
                        .required_inherited_const_keys()
                        .first()
                        .cloned()
                        .map(Into::into)
                });
            return Err(completed_solution_invariant(
                super::InheritedSolutionInvariantKind::OutOfScope,
                parameter,
            ));
        }
        require_exact_type_keys(self, parameters.required_inherited_keys())?;
        require_exact_const_keys(self, parameters.required_inherited_const_keys())?;
        require_exact_effect_keys(self, effects.required_inherited())?;
        let predicate = self.residual.reopen_effect_predicate(
            &self.effect_predicate,
            &path,
            application,
            context,
        )?;
        path.effects.restore_predicate(&predicate, context)?;

        for (parameter, value) in self.bindings() {
            let value = self
                .residual
                .reopen_type(value.value(), &path, application, context)?;
            context.restore_completed_binding(
                &mut path,
                application.id(),
                parameter.value().clone(),
                value,
            )?;
        }
        for (parameter, value) in self.const_bindings() {
            let value = self
                .residual
                .reopen_length(value.value(), &path, application, context)?;
            context.restore_completed_const_binding(
                &mut path,
                application.id(),
                parameter.value().clone(),
                value,
            )?;
        }
        for (variable, value) in self.effect_bindings() {
            let reference = parameters
                .effect_reference(variable.value())
                .expect("the inherited contract retains each effect parameter");
            let value =
                self.residual
                    .reopen_effect_row(value.value(), &path, application, context)?;
            path.effects
                .restore_completed_inherited(reference, &value, context)?;
        }
        Ok(path)
    }
}

fn require_exact_type_keys(
    solution: &TypeConstraintSolution,
    required: &[GenericTypeReference],
) -> Result<(), TypeConstraintError> {
    let rows = solution
        .bindings()
        .map(|(key, value)| (key.value(), value))
        .collect::<Vec<_>>();
    let mut row_index = 0;
    for required_key in required {
        if row_index < rows.len() && rows[row_index].0 < required_key {
            return Err(completed_solution_invariant(
                super::InheritedSolutionInvariantKind::UnexpectedKey,
                Some(rows[row_index].0.clone().into()),
            ));
        }
        if row_index == rows.len() || rows[row_index].0 > required_key {
            return Err(completed_solution_invariant(
                super::InheritedSolutionInvariantKind::Unclosed,
                Some(required_key.clone().into()),
            ));
        }
        row_index += 1;
    }
    if let Some((parameter, _)) = rows.get(row_index) {
        return Err(completed_solution_invariant(
            super::InheritedSolutionInvariantKind::UnexpectedKey,
            Some((*parameter).clone().into()),
        ));
    }
    Ok(())
}

fn require_exact_const_keys(
    solution: &TypeConstraintSolution,
    required: &[GenericConstReference],
) -> Result<(), TypeConstraintError> {
    let rows = solution
        .const_bindings()
        .map(|(key, value)| (key.value(), value))
        .collect::<Vec<_>>();
    let mut row_index = 0;
    for required_key in required {
        if row_index < rows.len() && rows[row_index].0 < required_key {
            return Err(completed_solution_invariant(
                super::InheritedSolutionInvariantKind::UnexpectedKey,
                Some(rows[row_index].0.clone().into()),
            ));
        }
        if row_index == rows.len() || rows[row_index].0 > required_key {
            return Err(completed_solution_invariant(
                super::InheritedSolutionInvariantKind::Unclosed,
                Some(required_key.clone().into()),
            ));
        }
        row_index += 1;
    }
    if let Some((parameter, _)) = rows.get(row_index) {
        return Err(completed_solution_invariant(
            super::InheritedSolutionInvariantKind::UnexpectedKey,
            Some((*parameter).clone().into()),
        ));
    }
    Ok(())
}

fn require_exact_effect_keys(
    solution: &TypeConstraintSolution,
    required: &[GenericEffectReference],
) -> Result<(), TypeConstraintError> {
    let rows = solution
        .effect_bindings()
        .map(|(parameter, value)| (parameter.value(), value.value()))
        .collect::<Vec<_>>();
    let mut row_index = 0;
    for required_key in required {
        if row_index < rows.len() && rows[row_index].0 < required_key {
            return Err(super::effect_invariant(
                super::TypeConstraintEffectInvariantKind::UnexpectedInherited,
                Some(rows[row_index].0.clone()),
            ));
        }
        if row_index == rows.len() || rows[row_index].0 > required_key {
            return Err(super::effect_invariant(
                super::TypeConstraintEffectInvariantKind::MissingInherited,
                Some(required_key.clone()),
            ));
        }
        row_index += 1;
    }
    if let Some((variable, _)) = rows.get(row_index) {
        return Err(super::effect_invariant(
            super::TypeConstraintEffectInvariantKind::UnexpectedInherited,
            Some((*variable).clone()),
        ));
    }
    Ok(())
}

fn completed_solution_invariant(
    kind: super::InheritedSolutionInvariantKind,
    parameter: Option<super::ConstraintGenericParameterId>,
) -> TypeConstraintError {
    TypeConstraintError::Invariant(TypeConstraintInvariant::InheritedSolution(
        super::InheritedSolutionInvariant { kind, parameter },
    ))
}

fn map_completed_canonical_error(
    error: TypeConstraintError,
    binding_parameter: super::ConstraintGenericParameterId,
    input: CompletedSolutionInput,
) -> TypeConstraintError {
    if !input.requires_canonical_claim() {
        return error;
    }
    match error {
        TypeConstraintError::Abort(error) => TypeConstraintError::Abort(error),
        TypeConstraintError::Rejected(TypeConstraintRejection::CyclicInstantiation {
            parameter,
        }) => completed_solution_invariant(
            super::InheritedSolutionInvariantKind::OccursOrCycle,
            Some(parameter),
        ),
        TypeConstraintError::Rejected(TypeConstraintRejection::IncompleteInstantiation {
            parameter,
        }) => completed_solution_invariant(
            super::InheritedSolutionInvariantKind::Unclosed,
            Some(parameter),
        ),
        TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(
            super::TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope { parameter },
        )) => completed_solution_invariant(
            super::InheritedSolutionInvariantKind::OutOfScope,
            Some(parameter.into()),
        ),
        TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(
            super::TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope { parameter },
        )) => completed_solution_invariant(
            super::InheritedSolutionInvariantKind::OutOfScope,
            Some(parameter.into()),
        ),
        TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(
            super::TypeConstraintParameterScopeInvariant::RigidBinding { parameter },
        )) => completed_solution_invariant(
            super::InheritedSolutionInvariantKind::RigidBinding,
            Some(parameter.into()),
        ),
        TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(
            super::TypeConstraintParameterScopeInvariant::RigidConstBinding { parameter },
        )) => completed_solution_invariant(
            super::InheritedSolutionInvariantKind::RigidBinding,
            Some(parameter.into()),
        ),
        TypeConstraintError::Invariant(TypeConstraintInvariant::InheritedSolution(error)) => {
            TypeConstraintError::Invariant(TypeConstraintInvariant::InheritedSolution(error))
        }
        TypeConstraintError::Invariant(TypeConstraintInvariant::Effect(error)) => {
            TypeConstraintError::Invariant(TypeConstraintInvariant::Effect(error))
        }
        TypeConstraintError::Invariant(_) | TypeConstraintError::Rejected(_) => {
            completed_solution_invariant(
                super::InheritedSolutionInvariantKind::Forbidden,
                Some(binding_parameter),
            )
        }
    }
}

fn map_completed_self_error(
    error: TypeConstraintError,
    binding_parameter: super::ConstraintGenericParameterId,
    input: CompletedSolutionInput,
) -> TypeConstraintError {
    if !input.requires_canonical_claim() {
        return error;
    }
    match error {
        TypeConstraintError::Abort(error) => TypeConstraintError::Abort(error),
        TypeConstraintError::Invariant(error) => TypeConstraintError::Invariant(error),
        TypeConstraintError::Rejected(TypeConstraintRejection::UnresolvedType)
        | TypeConstraintError::Rejected(TypeConstraintRejection::Mismatch)
        | TypeConstraintError::Rejected(TypeConstraintRejection::AmbiguousSolution { .. })
        | TypeConstraintError::Rejected(TypeConstraintRejection::CyclicInstantiation { .. })
        | TypeConstraintError::Rejected(TypeConstraintRejection::IncompleteInstantiation {
            ..
        })
        | TypeConstraintError::Rejected(TypeConstraintRejection::EffectSubset { .. }) => {
            completed_solution_invariant(
                super::InheritedSolutionInvariantKind::Forbidden,
                Some(binding_parameter),
            )
        }
    }
}

pub(crate) struct TypeConstraintBindingIter<'a> {
    rows: slice::Iter<'a, CheckedTypeArgumentBinding>,
    scope: &'a GenericScope,
    template_scope: &'a GenericScope,
}

pub(crate) struct TypeConstraintConstBindingIter<'a> {
    rows: slice::Iter<'a, CheckedConstArgumentBinding>,
    scope: &'a GenericScope,
    template_scope: &'a GenericScope,
}

pub(crate) struct TypeConstraintEffectBindingIter<'a> {
    rows: slice::Iter<'a, CheckedEffectArgumentBinding>,
    scope: &'a GenericScope,
    template_scope: &'a GenericScope,
}

impl<'a> Iterator for TypeConstraintBindingIter<'a> {
    type Item = (ScopedTypeReferenceView<'a>, ScopedTypeView<'a>);
    fn next(&mut self) -> Option<Self::Item> {
        self.rows.next().map(|binding| {
            (
                ScopedTypeReferenceView::sealed(&binding.parameter, self.template_scope),
                ScopedTypeView::sealed(&binding.value, self.scope),
            )
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.rows.size_hint()
    }
}
impl DoubleEndedIterator for TypeConstraintBindingIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.rows.next_back().map(|binding| {
            (
                ScopedTypeReferenceView::sealed(&binding.parameter, self.template_scope),
                ScopedTypeView::sealed(&binding.value, self.scope),
            )
        })
    }
}
impl ExactSizeIterator for TypeConstraintBindingIter<'_> {
    fn len(&self) -> usize {
        self.rows.len()
    }
}

impl<'a> Iterator for TypeConstraintConstBindingIter<'a> {
    type Item = (ScopedConstReferenceView<'a>, ScopedArrayLengthView<'a>);
    fn next(&mut self) -> Option<Self::Item> {
        self.rows.next().map(|binding| {
            (
                ScopedConstReferenceView::sealed(&binding.parameter, self.template_scope),
                ScopedArrayLengthView::sealed(&binding.value, self.scope),
            )
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.rows.size_hint()
    }
}
impl DoubleEndedIterator for TypeConstraintConstBindingIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.rows.next_back().map(|binding| {
            (
                ScopedConstReferenceView::sealed(&binding.parameter, self.template_scope),
                ScopedArrayLengthView::sealed(&binding.value, self.scope),
            )
        })
    }
}
impl ExactSizeIterator for TypeConstraintConstBindingIter<'_> {
    fn len(&self) -> usize {
        self.rows.len()
    }
}
impl<'a> Iterator for TypeConstraintEffectBindingIter<'a> {
    type Item = (ScopedEffectReferenceView<'a>, ScopedEffectRowView<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        self.rows.next().map(|binding| {
            (
                ScopedEffectReferenceView::sealed(&binding.variable, self.template_scope),
                ScopedEffectRowView::sealed(&binding.value, self.scope),
            )
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.rows.size_hint()
    }
}

impl DoubleEndedIterator for TypeConstraintEffectBindingIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.rows.next_back().map(|binding| {
            (
                ScopedEffectReferenceView::sealed(&binding.variable, self.template_scope),
                ScopedEffectRowView::sealed(&binding.value, self.scope),
            )
        })
    }
}

impl ExactSizeIterator for TypeConstraintEffectBindingIter<'_> {
    fn len(&self) -> usize {
        self.rows.len()
    }
}

#[cfg(test)]
mod malformed_completed_tests {
    use super::*;
    use crate::types::constraints::context::{LocalConstraintAccounting, TypeConstraintLimits};
    use crate::types::constraints::{
        InheritedSolutionInvariant, InheritedSolutionInvariantKind, NoConstraintClient,
        TypeConstraintInvariant, TypeConstraintParameterEligibility, TypeConstraintParameterScope,
    };
    use crate::types::{DetachedGenericOwnerId, GenericParameterOwnerId, GenericTypeParameterId};
    use std::{collections::BTreeSet, sync::atomic::AtomicBool};

    fn parameter(ordinal: u16) -> GenericTypeParameterId {
        GenericTypeParameterId::new(
            GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(190)),
            ordinal,
        )
    }

    fn seal_completed(
        rows: Vec<(GenericTypeParameterId, TypeKind)>,
    ) -> Result<TypeConstraintSolution, TypeConstraintError> {
        let parameters = rows
            .iter()
            .map(|(parameter, _)| parameter.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|parameter| (parameter, TypeConstraintParameterEligibility::Bindable))
            .collect::<Vec<_>>();
        let scope = TypeConstraintParameterScope::new(parameters).expect("unique test scope");
        let cancellation = AtomicBool::new(false);
        let (mut context, mut path) = super::super::test_support::ConstraintTestSetup::<
            LocalConstraintAccounting<'_>,
            NoConstraintClient,
        >::with_scope(
            TypeConstraintLimits::new(256, 128, 32, 16),
            &cancellation,
            scope,
        )
        .into_path();
        TypeConstraintSolution::test_seal_completed(rows, &mut path, &mut context)
    }

    #[test]
    fn malformed_completed_rows_are_typed_duplicate_or_unordered_invariants() {
        let first = parameter(0);
        let second = parameter(1);
        for (rows, expected) in [
            (
                vec![(first.clone(), TypeKind::I32), (first, TypeKind::String)],
                InheritedSolutionInvariantKind::DuplicateOrUnordered,
            ),
            (
                vec![(second, TypeKind::I32), (parameter(0), TypeKind::String)],
                InheritedSolutionInvariantKind::DuplicateOrUnordered,
            ),
        ] {
            assert!(matches!(
                seal_completed(rows),
                Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                        kind,
                        ..
                    }),
                )) if kind == expected
            ));
        }
    }
}
