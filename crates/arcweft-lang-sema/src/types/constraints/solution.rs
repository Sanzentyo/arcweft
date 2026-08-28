//! Opaque completed type-constraint solution authority.
//!
//! Active paths enter through one completion seal. The resulting carrier owns
//! its exact parameter/effect scope, completeness, and canonical rows; a later
//! continuation validates only the monotone scope transition and exact
//! inherited-key join before restoring those rows.

use std::{
    collections::{BTreeMap, BTreeSet},
    slice,
};

use crate::effect_row::{
    EffectConstraintEligibility, EffectIssuerRebindError, EffectRow, EffectSubstitution, EffectVar,
    EffectVarIssuer,
};

use super::super::{ArrayLength, GenericConstParameterId, GenericTypeParameterId, TypeKind};
use super::context::{
    TypeConstraintAccounting, TypeConstraintContext, TypeConstraintEffectScope,
    TypeConstraintParameterScope,
};
use super::normalization::{
    ConstraintBindingLookup, ConstraintConstBindingLookup, project_const_argument, project_type,
    validate_selected_call_self,
};
use super::{
    ConstraintClosurePolicy, ConstraintDomain, ConstraintPath, TypeConstraintError,
    TypeConstraintInvariant, TypeConstraintParameterEligibility, TypeConstraintRejection,
    TypeConstraintShape,
};

#[derive(Debug, Eq, PartialEq)]
struct CheckedTypeArgumentBinding {
    parameter: GenericTypeParameterId,
    value: TypeKind,
}

#[derive(Debug, Eq, PartialEq)]
struct CheckedConstArgumentBinding {
    parameter: GenericConstParameterId,
    value: ArrayLength,
}

impl CheckedConstArgumentBinding {
    fn new(parameter: GenericConstParameterId, value: ArrayLength) -> Self {
        Self { parameter, value }
    }
}

impl CheckedTypeArgumentBinding {
    fn new(parameter: GenericTypeParameterId, value: TypeKind) -> Self {
        Self { parameter, value }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct CheckedEffectArgumentBinding {
    variable: EffectVar,
    value: EffectRow,
}

impl CheckedEffectArgumentBinding {
    fn new(variable: EffectVar, value: EffectRow) -> Self {
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
    parameter_scope: TypeConstraintParameterScope,
    effect_scope: TypeConstraintEffectScope,
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
    bindings: Box<[CheckedTypeArgumentBinding]>,
    const_bindings: Box<[CheckedConstArgumentBinding]>,
    effect_bindings: Box<[CheckedEffectArgumentBinding]>,
}

impl TypeConstraintSolution {
    pub(crate) fn bindings(&self) -> TypeConstraintBindingIter<'_> {
        TypeConstraintBindingIter(self.bindings.iter())
    }

    pub(crate) fn effect_bindings(&self) -> TypeConstraintEffectBindingIter<'_> {
        TypeConstraintEffectBindingIter(self.effect_bindings.iter())
    }

    pub(crate) fn const_bindings(&self) -> TypeConstraintConstBindingIter<'_> {
        TypeConstraintConstBindingIter(self.const_bindings.iter())
    }

    /// Complete and seal one active lower path. No caller may publish or
    /// inherit its rows until this owner has projected the whole path and
    /// checked its scope and completeness.
    pub(super) fn complete_path<A, D>(
        bindings: BTreeMap<GenericTypeParameterId, TypeKind>,
        const_bindings: BTreeMap<GenericConstParameterId, ArrayLength>,
        effect_bindings: BTreeMap<EffectVar, EffectRow>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    {
        Self::seal_rows(
            bindings,
            const_bindings,
            effect_bindings,
            CompletedSolutionInput::ActivePath,
            context,
        )
    }

    #[cfg(test)]
    pub(crate) fn test_seal_completed<A, D, B, E>(
        bindings: B,
        effect_bindings: E,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
        B: IntoIterator<Item = (GenericTypeParameterId, TypeKind)>,
        E: IntoIterator<Item = (EffectVar, EffectRow)>,
    {
        Self::seal_rows(
            bindings,
            std::iter::empty(),
            effect_bindings,
            CompletedSolutionInput::ClaimedCompleted,
            context,
        )
    }

    #[cfg(test)]
    pub(crate) fn test_seal_completed_with_consts<A, D, B, C, E>(
        bindings: B,
        const_bindings: C,
        effect_bindings: E,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
        B: IntoIterator<Item = (GenericTypeParameterId, TypeKind)>,
        C: IntoIterator<Item = (GenericConstParameterId, ArrayLength)>,
        E: IntoIterator<Item = (EffectVar, EffectRow)>,
    {
        Self::seal_rows(
            bindings,
            const_bindings,
            effect_bindings,
            CompletedSolutionInput::ClaimedCompleted,
            context,
        )
    }

    fn seal_rows<A, D, B, C, E>(
        bindings: B,
        const_bindings: C,
        effect_bindings: E,
        input: CompletedSolutionInput,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
        B: IntoIterator<Item = (GenericTypeParameterId, TypeKind)>,
        C: IntoIterator<Item = (GenericConstParameterId, ArrayLength)>,
        E: IntoIterator<Item = (EffectVar, EffectRow)>,
    {
        let source_bindings = bindings.into_iter().collect::<Vec<_>>();
        let source_const_bindings = const_bindings.into_iter().collect::<Vec<_>>();
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
        let lookup = source_bindings.iter().cloned().collect::<BTreeMap<_, _>>();
        let const_lookup = source_const_bindings
            .iter()
            .cloned()
            .collect::<BTreeMap<_, _>>();
        let mut bindings = Vec::with_capacity(source_bindings.len());
        for (parameter, value) in source_bindings {
            match context.parameter_eligibility(&parameter) {
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
                &lookup,
                &const_lookup,
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
            bindings.push((parameter, projected.value));
        }
        for (parameter, eligibility) in context.parameter_scope.iter() {
            if matches!(eligibility, TypeConstraintParameterEligibility::Bindable)
                && !lookup.contains_key(parameter)
            {
                return Err(TypeConstraintRejection::IncompleteInstantiation {
                    parameter: parameter.clone().into(),
                }
                .into());
            }
        }

        let mut const_bindings = Vec::with_capacity(source_const_bindings.len());
        for (parameter, value) in source_const_bindings {
            match context.const_parameter_eligibility(&parameter) {
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
                &const_lookup,
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
            const_bindings.push((parameter, projected));
        }
        for (parameter, eligibility) in context.parameter_scope.const_iter() {
            if matches!(eligibility, super::TypeConstraintConstEligibility::Bindable)
                && !const_lookup.contains_key(parameter)
            {
                return Err(TypeConstraintRejection::IncompleteInstantiation {
                    parameter: parameter.clone().into(),
                }
                .into());
            }
        }

        let effect_bindings = effect_bindings.into_iter().collect::<Vec<_>>();
        if let Some(rows) = effect_bindings
            .windows(2)
            .find(|rows| rows[0].0 >= rows[1].0)
        {
            return Err(super::effect_invariant(
                super::TypeConstraintEffectInvariantKind::DuplicateOrUnorderedInherited,
                Some(rows[1].0),
            ));
        }
        for (variable, value) in &effect_bindings {
            if context.effect_eligibility(*variable).is_none() {
                return Err(super::effect_invariant(
                    super::TypeConstraintEffectInvariantKind::ForeignVariable,
                    Some(*variable),
                ));
            }
            if !matches!(value.tail(), crate::effect_row::EffectRowTail::Closed) {
                return Err(super::effect_invariant(
                    super::TypeConstraintEffectInvariantKind::NonCanonicalInherited,
                    Some(*variable),
                ));
            }
        }
        for row in context.effect_scope.variables() {
            if matches!(row.eligibility(), EffectConstraintEligibility::Bindable)
                && effect_bindings
                    .binary_search_by_key(&row.variable(), |(variable, _)| *variable)
                    .is_err()
            {
                return Err(super::effect_invariant(
                    super::TypeConstraintEffectInvariantKind::MissingInherited,
                    Some(row.variable()),
                ));
            }
        }

        Ok(Self {
            authority: CompletedTypeConstraintAuthority {
                parameter_scope: context.parameter_scope.clone(),
                effect_scope: context.effect_scope.clone(),
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
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ConstraintPath<D>, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    {
        if !self
            .authority
            .parameter_scope
            .accepts_continuation_scope(&context.parameter_scope)
        {
            if let Some((parameter, _)) = self.bindings().find(|(parameter, _)| {
                matches!(
                    context.parameter_eligibility(parameter),
                    Some(TypeConstraintParameterEligibility::Rigid)
                )
            }) {
                return Err(completed_solution_invariant(
                    super::InheritedSolutionInvariantKind::RigidBinding,
                    Some(parameter.clone().into()),
                ));
            }
            if let Some((parameter, _)) = self.const_bindings().find(|(parameter, _)| {
                matches!(
                    context.const_parameter_eligibility(parameter),
                    Some(super::TypeConstraintConstEligibility::Rigid)
                )
            }) {
                return Err(completed_solution_invariant(
                    super::InheritedSolutionInvariantKind::RigidBinding,
                    Some(parameter.clone().into()),
                ));
            }
            let parameter = self
                .bindings()
                .find_map(|(parameter, _)| {
                    context
                        .parameter_eligibility(parameter)
                        .is_none()
                        .then(|| parameter.clone().into())
                })
                .or_else(|| {
                    self.const_bindings().find_map(|(parameter, _)| {
                        context
                            .const_parameter_eligibility(parameter)
                            .is_none()
                            .then(|| parameter.clone().into())
                    })
                })
                .or_else(|| {
                    context
                        .required_inherited_keys()
                        .first()
                        .cloned()
                        .map(Into::into)
                })
                .or_else(|| {
                    context
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
        if !self
            .authority
            .effect_scope
            .accepts_continuation_scope(&context.effect_scope)
        {
            return Err(super::effect_invariant(
                super::TypeConstraintEffectInvariantKind::ForeignVariable,
                self.effect_bindings().next().map(|(variable, _)| *variable),
            ));
        }
        require_exact_type_keys(self, context.required_inherited_keys())?;
        require_exact_const_keys(self, context.required_inherited_const_keys())?;
        require_exact_effect_keys(self, context.required_inherited_effects())?;

        let mut path = context.start_path()?;
        for (parameter, value) in self.bindings() {
            context.restore_completed_binding(&mut path, parameter.clone(), value.clone())?;
        }
        for (parameter, value) in self.const_bindings() {
            context.restore_completed_const_binding(&mut path, parameter.clone(), value.clone())?;
        }
        for (variable, value) in self.effect_bindings() {
            path.effects
                .restore_completed_inherited(*variable, value.concrete());
        }
        Ok(path)
    }

    /// Apply the sealed lower solution without exposing a caller-owned
    /// substitution table. Callers only receive the normalized type result.
    pub(crate) fn apply(&self, ty: &TypeKind) -> TypeKind {
        let bindings = self
            .bindings()
            .map(|(parameter, value)| (parameter.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        let effects = EffectSubstitution::from_rows(
            self.effect_bindings()
                .map(|(variable, value)| (*variable, value.clone())),
        );
        let const_bindings = self
            .const_bindings()
            .map(|(parameter, value)| (parameter.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        ty.substitute_type_parameters(&bindings)
            .substitute_const_parameters(&const_bindings)
            .substitute_effect_rows(&effects)
            .expect("sealed constraint solutions contain only canonical effect rows")
    }

    pub(crate) fn checked_rebind_effect_issuer(
        &self,
        prepared: EffectVarIssuer,
        checked: EffectVarIssuer,
        authorized_ordinals: &BTreeSet<u32>,
    ) -> Result<Self, EffectIssuerRebindError> {
        let bindings = self
            .bindings()
            .map(|(parameter, value)| {
                Ok(CheckedTypeArgumentBinding::new(
                    parameter.clone(),
                    value.checked_rebind_effect_rows(prepared, checked, authorized_ordinals)?,
                ))
            })
            .collect::<Result<Box<[_]>, _>>()?;
        let effect_bindings = self
            .effect_bindings()
            .map(|(variable, value)| {
                if variable.issuer() != prepared {
                    return Err(EffectIssuerRebindError::ForeignVariable {
                        variable: *variable,
                    });
                }
                if !authorized_ordinals.contains(&variable.index()) {
                    return Err(EffectIssuerRebindError::UnauthorizedVariable {
                        variable: *variable,
                    });
                }
                Ok(CheckedEffectArgumentBinding::new(
                    variable.rebind_issuer(prepared, checked),
                    value.checked_rebind_issuer(prepared, checked, authorized_ordinals)?,
                ))
            })
            .collect::<Result<Box<[_]>, _>>()?;
        let const_bindings = self
            .const_bindings()
            .map(|(parameter, value)| {
                CheckedConstArgumentBinding::new(parameter.clone(), value.clone())
            })
            .collect();
        Ok(Self {
            authority: CompletedTypeConstraintAuthority {
                parameter_scope: self.authority.parameter_scope.clone(),
                effect_scope: self.authority.effect_scope.checked_rebind_issuer(
                    prepared,
                    checked,
                    authorized_ordinals,
                )?,
            },
            bindings,
            const_bindings,
            effect_bindings,
        })
    }
}

fn require_exact_type_keys(
    solution: &TypeConstraintSolution,
    required: &[GenericTypeParameterId],
) -> Result<(), TypeConstraintError> {
    let rows = solution.bindings().collect::<Vec<_>>();
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
    required: &[GenericConstParameterId],
) -> Result<(), TypeConstraintError> {
    let rows = solution.const_bindings().collect::<Vec<_>>();
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
    required: &[EffectVar],
) -> Result<(), TypeConstraintError> {
    let rows = solution.effect_bindings().collect::<Vec<_>>();
    let mut row_index = 0;
    for required_key in required {
        if row_index < rows.len() && rows[row_index].0 < required_key {
            return Err(super::effect_invariant(
                super::TypeConstraintEffectInvariantKind::UnexpectedInherited,
                Some(*rows[row_index].0),
            ));
        }
        if row_index == rows.len() || rows[row_index].0 > required_key {
            return Err(super::effect_invariant(
                super::TypeConstraintEffectInvariantKind::MissingInherited,
                Some(*required_key),
            ));
        }
        row_index += 1;
    }
    if let Some((variable, _)) = rows.get(row_index) {
        return Err(super::effect_invariant(
            super::TypeConstraintEffectInvariantKind::UnexpectedInherited,
            Some(**variable),
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

impl ConstraintBindingLookup for TypeConstraintSolution {
    fn binding(&self, parameter: &GenericTypeParameterId) -> Option<&TypeKind> {
        self.bindings
            .binary_search_by(|binding| binding.parameter.cmp(parameter))
            .ok()
            .map(|index| &self.bindings[index].value)
    }
}

impl ConstraintConstBindingLookup for TypeConstraintSolution {
    fn const_binding(&self, parameter: &GenericConstParameterId) -> Option<&ArrayLength> {
        self.const_bindings
            .binary_search_by(|binding| binding.parameter.cmp(parameter))
            .ok()
            .map(|index| &self.const_bindings[index].value)
    }
}

pub(crate) struct TypeConstraintBindingIter<'a>(slice::Iter<'a, CheckedTypeArgumentBinding>);

pub(crate) struct TypeConstraintConstBindingIter<'a>(slice::Iter<'a, CheckedConstArgumentBinding>);

pub(crate) struct TypeConstraintEffectBindingIter<'a>(
    slice::Iter<'a, CheckedEffectArgumentBinding>,
);

impl<'a> Iterator for TypeConstraintBindingIter<'a> {
    type Item = (&'a GenericTypeParameterId, &'a TypeKind);

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|binding| (&binding.parameter, &binding.value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for TypeConstraintBindingIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0
            .next_back()
            .map(|binding| (&binding.parameter, &binding.value))
    }
}

impl ExactSizeIterator for TypeConstraintBindingIter<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'a> Iterator for TypeConstraintConstBindingIter<'a> {
    type Item = (&'a GenericConstParameterId, &'a ArrayLength);

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|binding| (&binding.parameter, &binding.value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for TypeConstraintConstBindingIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0
            .next_back()
            .map(|binding| (&binding.parameter, &binding.value))
    }
}

impl ExactSizeIterator for TypeConstraintConstBindingIter<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'a> Iterator for TypeConstraintEffectBindingIter<'a> {
    type Item = (&'a EffectVar, &'a EffectRow);

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|binding| (&binding.variable, &binding.value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for TypeConstraintEffectBindingIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0
            .next_back()
            .map(|binding| (&binding.variable, &binding.value))
    }
}

impl ExactSizeIterator for TypeConstraintEffectBindingIter<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

#[cfg(test)]
mod malformed_completed_tests {
    use super::*;
    use crate::types::constraints::context::{
        LocalConstraintAccounting, TypeConstraintContext, TypeConstraintLimits,
    };
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
        let mut context =
            TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
                TypeConstraintLimits::new(256, 128, 32, 16),
                &cancellation,
                scope,
            );
        TypeConstraintSolution::test_seal_completed(rows, std::iter::empty(), &mut context)
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
