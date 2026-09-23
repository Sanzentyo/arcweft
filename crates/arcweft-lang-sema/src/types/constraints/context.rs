//! Constraint work accounting, limits, and lexical traversal state.
//!
//! This module owns the persistent cancellation/work context. The transaction
//! lives in the sibling `transaction` module and only borrows it per phase.

#[cfg(test)]
use std::collections::BTreeMap;
use std::{
    marker::PhantomData,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::effect_row::{
    EffectConstraintEligibility, EffectConstraintEnvironment, EffectConstraintVariable, EffectRow,
};

use super::super::generics::OpenedGenericScope;
use super::super::{
    GenericBinder, GenericConstReference, GenericEffectReference, GenericParameterKind,
    GenericScope, GenericScopeError, GenericTypeReference, TypeCompatibilityControl, TypeKind,
};
#[cfg(test)]
use super::super::{GenericConstParameterId, GenericTypeParameterId};
use super::application::{ConstraintApplicationId, ConstraintApplicationScope};
use super::normalization::ConstraintProjectionView;
use super::{
    ConstraintDomain, ConstraintPath, TypeConstraintAbort, TypeConstraintError,
    TypeConstraintInvariant, TypeConstraintParameterScopeInvariant, TypeConstraintShape,
    effect_invariant, occurs_in_shape,
};

/// Inclusive bounds for one candidate's type-constraint relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TypeConstraintLimits {
    pub(crate) max_work: u64,
    pub(crate) max_nodes: u64,
    pub(crate) max_branches: u64,
    pub(crate) max_bindings: u64,
    pub(crate) max_source_probes: u64,
    pub(crate) max_materializations: u64,
}

impl TypeConstraintLimits {
    pub(crate) const fn new(
        max_work: u64,
        max_nodes: u64,
        max_branches: u64,
        max_bindings: u64,
    ) -> Self {
        Self {
            max_work,
            max_nodes,
            max_branches,
            max_bindings,
            max_source_probes: u64::MAX,
            max_materializations: u64::MAX,
        }
    }

    pub(crate) const fn max_work(self) -> u64 {
        self.max_work
    }

    pub(crate) const fn with_source_limits(
        mut self,
        max_source_probes: u64,
        max_materializations: u64,
    ) -> Self {
        self.max_source_probes = max_source_probes;
        self.max_materializations = max_materializations;
        self
    }

    pub(crate) const fn max_source_probes(self) -> u64 {
        self.max_source_probes
    }

    pub(crate) const fn max_materializations(self) -> u64 {
        self.max_materializations
    }
}

/// Checked lower work counters accumulated by one constraint context.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TypeConstraintWorkReport {
    pub(crate) work: u64,
    pub(crate) nodes: u64,
    pub(crate) branches: u64,
    pub(crate) bindings: u64,
    pub(crate) source_probes: u64,
    pub(crate) materializations: u64,
}

impl TypeConstraintWorkReport {
    pub(crate) const ZERO: Self = Self {
        work: 0,
        nodes: 0,
        branches: 0,
        bindings: 0,
        source_probes: 0,
        materializations: 0,
    };

    pub(crate) const fn work(&self) -> u64 {
        self.work
    }

    #[cfg(test)]
    pub(crate) const fn nodes(&self) -> u64 {
        self.nodes
    }

    pub(crate) const fn source_probes(&self) -> u64 {
        self.source_probes
    }

    pub(crate) const fn materializations(&self) -> u64 {
        self.materializations
    }

    pub(crate) fn checked_add(&self, other: &Self) -> Result<Self, super::TypeConstraintError> {
        Ok(Self {
            work: self
                .work
                .checked_add(other.work)
                .ok_or(super::TypeConstraintError::Abort(
                    super::TypeConstraintAbort::ArithmeticOverflow,
                ))?,
            nodes: self
                .nodes
                .checked_add(other.nodes)
                .ok_or(super::TypeConstraintError::Abort(
                    super::TypeConstraintAbort::ArithmeticOverflow,
                ))?,
            branches: self.branches.checked_add(other.branches).ok_or(
                super::TypeConstraintError::Abort(super::TypeConstraintAbort::ArithmeticOverflow),
            )?,
            bindings: self.bindings.checked_add(other.bindings).ok_or(
                super::TypeConstraintError::Abort(super::TypeConstraintAbort::ArithmeticOverflow),
            )?,
            source_probes: self.source_probes.checked_add(other.source_probes).ok_or(
                super::TypeConstraintError::Abort(super::TypeConstraintAbort::ArithmeticOverflow),
            )?,
            materializations: self
                .materializations
                .checked_add(other.materializations)
                .ok_or(super::TypeConstraintError::Abort(
                    super::TypeConstraintAbort::ArithmeticOverflow,
                ))?,
        })
    }
}

/// Eligibility of an exact declaration-owned generic parameter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum TypeConstraintParameterEligibility {
    Rigid,
    Bindable,
    FutureEligible,
}

/// Whether a keyed final projection must be fully closed now or may carry
/// declaration-owned future-eligible parameters into the next continuation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TypeConstraintProjectionClosure {
    Closed,
    AllowFutureEligible,
}

/// Declaration or scheme effect-parameter contract. It contains no application
/// issuer; TypeConstraintParameterScope opens its slots alongside type and
/// constant slots in the same OpenedGenericScope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeConstraintEffectScope {
    variables: Box<[EffectConstraintVariable]>,
    free_variables: Box<[EffectConstraintVariable]>,
    required_inherited: Box<[GenericEffectReference]>,
}

impl TypeConstraintEffectScope {
    pub(crate) fn seal_call_scope<V, R>(
        variables: V,
        required_inherited: R,
    ) -> Result<Self, TypeConstraintInvariant>
    where
        V: IntoIterator<Item = EffectConstraintVariable>,
        R: IntoIterator<Item = GenericEffectReference>,
    {
        let (free_variables, variables): (Vec<_>, Vec<_>) = variables
            .into_iter()
            .partition(|row| row.eligibility() == EffectConstraintEligibility::Rigid);
        if [&free_variables, &variables].into_iter().any(|rows| {
            rows.windows(2)
                .any(|rows| rows[0].variable() >= rows[1].variable())
        }) || free_variables.iter().any(|row| {
            !matches!(
                row.variable(),
                GenericEffectReference::Free(_) | GenericEffectReference::Inference(_)
            )
        }) || variables
            .iter()
            .any(|row| matches!(row.variable(), GenericEffectReference::Inference(_)))
        {
            return Err(effect_scope_invariant(
                super::TypeConstraintEffectInvariantKind::DuplicateOrUnorderedScope,
                None,
            ));
        }
        let required_inherited = required_inherited.into_iter().collect::<Vec<_>>();
        if required_inherited.windows(2).any(|rows| rows[0] >= rows[1]) {
            return Err(effect_scope_invariant(
                super::TypeConstraintEffectInvariantKind::DuplicateOrUnorderedInherited,
                None,
            ));
        }
        for variable in &required_inherited {
            let Some(row) = variables.iter().find(|row| row.variable() == variable) else {
                return Err(effect_scope_invariant(
                    super::TypeConstraintEffectInvariantKind::RequiredInheritedOutOfScope,
                    Some(variable.clone()),
                ));
            };
            if row.eligibility() != EffectConstraintEligibility::Bindable {
                return Err(effect_scope_invariant(
                    super::TypeConstraintEffectInvariantKind::RequiredInheritedNotBindable,
                    Some(variable.clone()),
                ));
            }
        }
        Ok(Self {
            variables: variables.into_boxed_slice(),
            free_variables: free_variables.into_boxed_slice(),
            required_inherited: required_inherited.into_boxed_slice(),
        })
    }

    pub(crate) fn variables(&self) -> impl Iterator<Item = &EffectConstraintVariable> {
        self.free_variables.iter().chain(self.variables.iter())
    }

    fn candidate_rows(&self) -> impl Iterator<Item = &EffectConstraintVariable> {
        self.variables.iter()
    }

    fn template_eligibility(
        &self,
        reference: &GenericEffectReference,
    ) -> Option<EffectConstraintEligibility> {
        self.variables
            .binary_search_by(|row| row.variable().cmp(reference))
            .ok()
            .map(|index| self.variables[index].eligibility())
    }

    pub(crate) fn required_inherited(&self) -> &[GenericEffectReference] {
        &self.required_inherited
    }

    pub(super) fn accepts_continuation_scope(&self, other: &Self) -> bool {
        self.free_variables == other.free_variables
            && self.variables.len() == other.variables.len()
            && self
                .variables
                .iter()
                .zip(&other.variables)
                .all(|(completed, next)| {
                    completed.variable() == next.variable()
                        && (completed.eligibility() == next.eligibility()
                            || (completed.eligibility()
                                == EffectConstraintEligibility::FutureEligible
                                && next.eligibility() == EffectConstraintEligibility::Bindable))
                })
    }
}
fn effect_scope_invariant(
    kind: super::TypeConstraintEffectInvariantKind,
    variable: Option<GenericEffectReference>,
) -> super::TypeConstraintInvariant {
    match effect_invariant(kind, variable) {
        TypeConstraintError::Invariant(invariant) => invariant,
        TypeConstraintError::Rejected(_) | TypeConstraintError::Abort(_) => {
            unreachable!("effect scope errors are invariants")
        }
    }
}

/// Semantic inventory of one generic constant parameter. Constant and type
/// namespaces remain distinct, but share the same continuation eligibility
/// transitions and completed-solution authority.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum TypeConstraintConstEligibility {
    Rigid,
    Bindable,
    FutureEligible,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeConstraintTypeParameterScopeRow {
    parameter: GenericTypeReference,
    eligibility: TypeConstraintParameterEligibility,
}

impl TypeConstraintTypeParameterScopeRow {
    pub(crate) fn new(
        parameter: impl Into<GenericTypeReference>,
        eligibility: TypeConstraintParameterEligibility,
    ) -> Self {
        Self {
            parameter: parameter.into(),
            eligibility,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeConstraintConstParameterScopeRow {
    parameter: GenericConstReference,
    eligibility: TypeConstraintConstEligibility,
}

impl TypeConstraintConstParameterScopeRow {
    pub(crate) fn new(
        parameter: impl Into<GenericConstReference>,
        eligibility: TypeConstraintConstEligibility,
    ) -> Self {
        Self {
            parameter: parameter.into(),
            eligibility,
        }
    }
}

/// Types-owned sorted contract for inherited binding keys required by the
/// sealed continuation prefix. The namespaces are retained separately so a
/// type and constant at the same owner/ordinal cannot alias.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RequiredInheritedBindingKeys {
    type_keys: Box<[GenericTypeReference]>,
    const_keys: Box<[GenericConstReference]>,
}

/// The complete lower-visible parameter inventory for one candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeConstraintParameterScope {
    opening: OpenedGenericScope,
    contract: CompletedParameterScope,
}

/// Declaration and rigid-reference inventory retained after an application
/// closes. Rigid references may name a parent-owned inference variable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CompletedParameterScope {
    template_scope: GenericScope,
    type_parameters: Box<[TypeConstraintTypeParameterScopeRow]>,
    const_parameters: Box<[TypeConstraintConstParameterScopeRow]>,
    effects: TypeConstraintEffectScope,
    rigid_types: Box<[GenericTypeReference]>,
    rigid_consts: Box<[GenericConstReference]>,
    required_inherited: RequiredInheritedBindingKeys,
}

impl TypeConstraintParameterScope {
    /// Seal the exact kind-separated inventories and the inherited-key
    /// contract.  This is the only production constructor; callers must
    /// provide rows in canonical order and cannot ask the lower layer to sort
    /// or repair them.
    pub(crate) fn seal_call_scope<T, C, R, Q>(
        template_binder: GenericBinder,
        type_parameters: T,
        const_parameters: C,
        effects: TypeConstraintEffectScope,
        required_inherited_keys: R,
        required_inherited_const_keys: Q,
    ) -> Result<Self, super::TypeConstraintInvariant>
    where
        T: IntoIterator<Item = TypeConstraintTypeParameterScopeRow>,
        C: IntoIterator<Item = TypeConstraintConstParameterScopeRow>,
        R: IntoIterator<Item = GenericTypeReference>,
        Q: IntoIterator<Item = GenericConstReference>,
    {
        let template_scope = GenericScope::default().with_binder(template_binder);
        for row in effects.variables() {
            match row.variable() {
                GenericEffectReference::Free(_) => {}
                GenericEffectReference::Inference(_)
                    if row.eligibility() == EffectConstraintEligibility::Rigid => {}
                GenericEffectReference::Bound(parameter)
                    if parameter.depth() == 0
                        && row.eligibility() != EffectConstraintEligibility::Rigid =>
                {
                    template_scope
                        .bound_effect(0, parameter.slot())
                        .map_err(TypeConstraintInvariant::GenericScope)?;
                }
                _ => {
                    return Err(effect_scope_invariant(
                        super::TypeConstraintEffectInvariantKind::ForeignVariable,
                        Some(row.variable().clone()),
                    ));
                }
            }
        }
        for slot in 0..template_binder.effects() {
            let reference = template_scope
                .bound_effect(0, slot)
                .map_err(TypeConstraintInvariant::GenericScope)?;
            if effects.template_eligibility(&reference).is_none() {
                return Err(effect_scope_invariant(
                    super::TypeConstraintEffectInvariantKind::ForeignVariable,
                    Some(reference),
                ));
            }
        }
        let (free_type_rows, type_parameters): (Vec<_>, Vec<_>) = type_parameters
            .into_iter()
            .partition(|row| row.eligibility == TypeConstraintParameterEligibility::Rigid);
        let free_types = free_type_rows
            .into_iter()
            .map(|row| row.parameter)
            .collect::<Vec<_>>();
        validate_scope_rows(free_types.iter(), false)?;
        validate_scope_rows(type_parameters.iter().map(|row| &row.parameter), false)?;
        let (free_const_rows, const_parameters): (Vec<_>, Vec<_>) = const_parameters
            .into_iter()
            .partition(|row| row.eligibility == TypeConstraintConstEligibility::Rigid);
        let free_consts = free_const_rows
            .into_iter()
            .map(|row| row.parameter)
            .collect::<Vec<_>>();
        validate_scope_rows(free_consts.iter(), true)?;
        validate_scope_rows(const_parameters.iter().map(|row| &row.parameter), true)?;

        for parameter in &free_types {
            if !matches!(
                parameter,
                GenericTypeReference::Free(_) | GenericTypeReference::Inference(_)
            ) {
                return Err(scope_invariant(
                    TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope {
                        parameter: parameter.clone(),
                    },
                ));
            }
        }
        for parameter in &free_consts {
            if !matches!(
                parameter,
                GenericConstReference::Free(_) | GenericConstReference::Inference(_)
            ) {
                return Err(scope_invariant(
                    TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope {
                        parameter: parameter.clone(),
                    },
                ));
            }
        }
        for row in &type_parameters {
            match &row.parameter {
                GenericTypeReference::Free(_) => {}
                GenericTypeReference::Bound(parameter) if parameter.depth() == 0 => {
                    template_scope
                        .bound_type(0, parameter.slot())
                        .map_err(TypeConstraintInvariant::GenericScope)?;
                }
                _ => {
                    return Err(scope_invariant(
                        TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope {
                            parameter: row.parameter.clone(),
                        },
                    ));
                }
            }
        }
        for row in &const_parameters {
            match &row.parameter {
                GenericConstReference::Free(_) => {}
                GenericConstReference::Bound(parameter) if parameter.depth() == 0 => {
                    template_scope
                        .bound_const(0, parameter.slot())
                        .map_err(TypeConstraintInvariant::GenericScope)?;
                }
                _ => {
                    return Err(scope_invariant(
                        TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope {
                            parameter: row.parameter.clone(),
                        },
                    ));
                }
            }
        }

        for slot in 0..template_binder.types() {
            let parameter = template_scope
                .bound_type(0, slot)
                .map_err(TypeConstraintInvariant::GenericScope)?;
            if type_parameters
                .binary_search_by(|row| row.parameter.cmp(&parameter))
                .is_err()
            {
                return Err(scope_invariant(
                    TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope { parameter },
                ));
            }
        }
        for slot in 0..template_binder.const_lengths() {
            let parameter = template_scope
                .bound_const(0, slot)
                .map_err(TypeConstraintInvariant::GenericScope)?;
            if const_parameters
                .binary_search_by(|row| row.parameter.cmp(&parameter))
                .is_err()
            {
                return Err(scope_invariant(
                    TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope { parameter },
                ));
            }
        }

        let required_inherited_keys = required_inherited_keys.into_iter().collect::<Vec<_>>();
        validate_scope_rows(required_inherited_keys.iter(), false)?;
        for key in &required_inherited_keys {
            let Some(row) = type_parameters.iter().find(|row| &row.parameter == key) else {
                return Err(scope_invariant(
                    super::TypeConstraintParameterScopeInvariant::RequiredInheritedKeyOutOfScope {
                        parameter: key.clone(),
                    },
                ));
            };
            if !matches!(
                row.eligibility,
                TypeConstraintParameterEligibility::Bindable
            ) {
                return Err(scope_invariant(
                    super::TypeConstraintParameterScopeInvariant::RequiredInheritedKeyNotBindable {
                        parameter: key.clone(),
                    },
                ));
            }
        }

        let required_inherited_const_keys = required_inherited_const_keys
            .into_iter()
            .collect::<Vec<_>>();
        validate_scope_rows(required_inherited_const_keys.iter(), true)?;
        for key in &required_inherited_const_keys {
            let Some(row) = const_parameters.iter().find(|row| &row.parameter == key) else {
                return Err(scope_invariant(
                    super::TypeConstraintParameterScopeInvariant::RequiredInheritedConstKeyOutOfScope {
                        parameter: key.clone(),
                    },
                ));
            };
            if !matches!(row.eligibility, TypeConstraintConstEligibility::Bindable) {
                return Err(scope_invariant(
                    super::TypeConstraintParameterScopeInvariant::RequiredInheritedConstKeyNotBindable {
                        parameter: key.clone(),
                    },
                ));
            }
        }

        let type_count = u16::try_from(type_parameters.len()).map_err(|_| {
            TypeConstraintInvariant::GenericScope(GenericScopeError::BinderArityOverflow {
                kind: GenericParameterKind::Type,
                count: type_parameters.len(),
            })
        })?;
        let const_count = u16::try_from(const_parameters.len()).map_err(|_| {
            TypeConstraintInvariant::GenericScope(GenericScopeError::BinderArityOverflow {
                kind: GenericParameterKind::Const,
                count: const_parameters.len(),
            })
        })?;
        let effect_count = effects.candidate_rows().count();
        let effect_count = u32::try_from(effect_count).map_err(|_| {
            TypeConstraintInvariant::GenericScope(GenericScopeError::BinderArityOverflow {
                kind: GenericParameterKind::Effect,
                count: effect_count,
            })
        })?;
        let opening =
            OpenedGenericScope::new(GenericBinder::new(type_count, const_count, effect_count))
                .map_err(TypeConstraintInvariant::GenericScope)?;
        let contract = CompletedParameterScope {
            template_scope,
            type_parameters: type_parameters.into_boxed_slice(),
            const_parameters: const_parameters.into_boxed_slice(),
            effects,
            rigid_types: free_types.into_boxed_slice(),
            rigid_consts: free_consts.into_boxed_slice(),
            required_inherited: RequiredInheritedBindingKeys {
                type_keys: required_inherited_keys.into_boxed_slice(),
                const_keys: required_inherited_const_keys.into_boxed_slice(),
            },
        };
        Ok(Self { opening, contract })
    }

    #[cfg(test)]
    pub(crate) fn new<I>(parameters: I) -> Result<Self, super::TypeConstraintError>
    where
        I: IntoIterator<Item = (GenericTypeParameterId, TypeConstraintParameterEligibility)>,
    {
        let mut inventory = BTreeMap::new();
        for (parameter, eligibility) in parameters {
            if inventory.insert(parameter, eligibility).is_some() {
                return Err(scope_error(
                    super::TypeConstraintParameterScopeInvariant::DuplicateParameter,
                ));
            }
        }
        Self::seal_call_scope(
            GenericBinder::EMPTY,
            inventory.into_iter().map(|(parameter, eligibility)| {
                TypeConstraintTypeParameterScopeRow::new(parameter, eligibility)
            }),
            std::iter::empty(),
            crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], [])
                .expect("empty effect scope"),
            std::iter::empty(),
            std::iter::empty(),
        )
        .map_err(super::TypeConstraintError::Invariant)
    }

    #[cfg(test)]
    pub(crate) fn new_with_constants<I, J>(
        type_parameters: I,
        const_parameters: J,
    ) -> Result<Self, super::TypeConstraintError>
    where
        I: IntoIterator<Item = (GenericTypeParameterId, TypeConstraintParameterEligibility)>,
        J: IntoIterator<Item = (GenericConstParameterId, TypeConstraintConstEligibility)>,
    {
        let mut types = BTreeMap::new();
        for (parameter, eligibility) in type_parameters {
            if types.insert(parameter, eligibility).is_some() {
                return Err(scope_error(
                    super::TypeConstraintParameterScopeInvariant::DuplicateParameter,
                ));
            }
        }
        let mut constants = BTreeMap::new();
        for (parameter, eligibility) in const_parameters {
            if constants.insert(parameter, eligibility).is_some() {
                return Err(scope_error(
                    super::TypeConstraintParameterScopeInvariant::DuplicateParameter,
                ));
            }
        }
        Self::seal_call_scope(
            GenericBinder::EMPTY,
            types.into_iter().map(|(parameter, eligibility)| {
                TypeConstraintTypeParameterScopeRow::new(parameter, eligibility)
            }),
            constants.into_iter().map(|(parameter, eligibility)| {
                TypeConstraintConstParameterScopeRow::new(parameter, eligibility)
            }),
            crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], [])
                .expect("empty effect scope"),
            std::iter::empty(),
            std::iter::empty(),
        )
        .map_err(super::TypeConstraintError::Invariant)
    }

    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self::seal_call_scope(
            GenericBinder::EMPTY,
            std::iter::empty(),
            std::iter::empty(),
            crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], [])
                .expect("empty effect scope"),
            std::iter::empty(),
            std::iter::empty(),
        )
        .expect("empty test scope is valid")
    }

    pub(super) const fn completed_contract(&self) -> &CompletedParameterScope {
        &self.contract
    }

    pub(super) fn application_issuer(&self) -> super::super::generics::GenericApplicationIssuer {
        self.opening.issuer()
    }

    pub(crate) fn type_reference(
        &self,
        parameter: &GenericTypeReference,
    ) -> Option<GenericTypeReference> {
        let slot = self
            .contract
            .type_parameters
            .binary_search_by(|row| row.parameter.cmp(parameter))
            .ok()?;
        self.opening.type_reference(u16::try_from(slot).ok()?).ok()
    }

    pub(crate) fn const_reference(
        &self,
        parameter: &GenericConstReference,
    ) -> Option<GenericConstReference> {
        let slot = self
            .contract
            .const_parameters
            .binary_search_by(|row| row.parameter.cmp(parameter))
            .ok()?;
        self.opening.const_reference(u16::try_from(slot).ok()?).ok()
    }

    pub(crate) fn effect_reference(
        &self,
        parameter: &GenericEffectReference,
    ) -> Option<GenericEffectReference> {
        let slot = self
            .contract
            .effects
            .candidate_rows()
            .position(|row| row.variable() == parameter)?;
        self.opening
            .effect_reference(u32::try_from(slot).ok()?)
            .ok()
    }

    pub(super) fn effect_declaration(
        &self,
        reference: &GenericEffectReference,
    ) -> Option<&GenericEffectReference> {
        let GenericEffectReference::Inference(variable) = reference else {
            return None;
        };
        (variable.issuer() == self.opening.issuer()).then_some(())?;
        self.contract
            .effects
            .candidate_rows()
            .nth(usize::try_from(variable.slot()).ok()?)
            .map(EffectConstraintVariable::variable)
    }

    pub(crate) fn effect_eligibility(
        &self,
        reference: &GenericEffectReference,
    ) -> Option<EffectConstraintEligibility> {
        match reference {
            GenericEffectReference::Free(_) | GenericEffectReference::Inference(_) => self
                .contract
                .effects
                .free_variables
                .binary_search_by(|row| row.variable().cmp(reference))
                .ok()
                .map(|_| EffectConstraintEligibility::Rigid)
                .or_else(|| {
                    matches!(reference, GenericEffectReference::Inference(_))
                        .then(|| {
                            self.effect_declaration(reference).and_then(|parameter| {
                                self.contract.effects.template_eligibility(parameter)
                            })
                        })
                        .flatten()
                }),
            GenericEffectReference::Bound(_) => None,
        }
    }

    pub(crate) const fn effect_contract(&self) -> &TypeConstraintEffectScope {
        &self.contract.effects
    }

    pub(super) fn opened_effect_variables(&self) -> Vec<EffectConstraintVariable> {
        let mut rows = self
            .contract
            .effects
            .variables()
            .map(|row| {
                let reference = if row.eligibility() == EffectConstraintEligibility::Rigid {
                    row.variable().clone()
                } else {
                    self.effect_reference(row.variable())
                        .expect("sealed effect slot")
                };
                EffectConstraintVariable::new(reference, row.eligibility())
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| left.variable().cmp(right.variable()));
        rows
    }

    pub(super) fn type_declaration(
        &self,
        reference: &GenericTypeReference,
    ) -> Option<&GenericTypeReference> {
        let GenericTypeReference::Inference(variable) = reference else {
            return None;
        };
        (variable.issuer() == self.opening.issuer()).then_some(())?;
        self.contract
            .type_parameters
            .get(usize::from(variable.slot()))
            .map(|row| &row.parameter)
    }

    pub(super) fn const_declaration(
        &self,
        reference: &GenericConstReference,
    ) -> Option<&GenericConstReference> {
        let GenericConstReference::Inference(variable) = reference else {
            return None;
        };
        (variable.issuer() == self.opening.issuer()).then_some(())?;
        self.contract
            .const_parameters
            .get(usize::from(variable.slot()))
            .map(|row| &row.parameter)
    }

    pub(crate) fn eligibility(
        &self,
        reference: &GenericTypeReference,
    ) -> Option<TypeConstraintParameterEligibility> {
        match reference {
            GenericTypeReference::Free(_) | GenericTypeReference::Inference(_) => self
                .contract
                .rigid_types
                .binary_search(reference)
                .ok()
                .map(|_| TypeConstraintParameterEligibility::Rigid)
                .or_else(|| {
                    matches!(reference, GenericTypeReference::Inference(_))
                        .then(|| {
                            self.type_declaration(reference)
                                .and_then(|parameter| self.contract.eligibility(parameter))
                        })
                        .flatten()
                }),
            GenericTypeReference::Bound(_) => None,
        }
    }

    pub(crate) fn const_eligibility(
        &self,
        reference: &GenericConstReference,
    ) -> Option<TypeConstraintConstEligibility> {
        match reference {
            GenericConstReference::Free(_) | GenericConstReference::Inference(_) => self
                .contract
                .rigid_consts
                .binary_search(reference)
                .ok()
                .map(|_| TypeConstraintConstEligibility::Rigid)
                .or_else(|| {
                    matches!(reference, GenericConstReference::Inference(_))
                        .then(|| {
                            self.const_declaration(reference)
                                .and_then(|parameter| self.contract.const_eligibility(parameter))
                        })
                        .flatten()
                }),
            GenericConstReference::Bound(_) => None,
        }
    }

    pub(crate) fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&GenericTypeReference, &TypeConstraintParameterEligibility)>
    {
        self.contract.iter()
    }

    pub(crate) fn const_iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&GenericConstReference, &TypeConstraintConstEligibility)>
    {
        self.contract.const_iter()
    }

    pub(super) fn required_inherited_keys(&self) -> &[GenericTypeReference] {
        self.contract.required_inherited_keys()
    }

    pub(super) fn required_inherited_const_keys(&self) -> &[GenericConstReference] {
        self.contract.required_inherited_const_keys()
    }
}

impl CompletedParameterScope {
    pub(super) const fn effect_contract(&self) -> &TypeConstraintEffectScope {
        &self.effects
    }
    pub(super) const fn template_scope(&self) -> &GenericScope {
        &self.template_scope
    }
    pub(crate) fn eligibility(
        &self,
        parameter: &GenericTypeReference,
    ) -> Option<TypeConstraintParameterEligibility> {
        self.type_parameters
            .binary_search_by(|row| row.parameter.cmp(parameter))
            .ok()
            .map(|index| self.type_parameters[index].eligibility)
    }

    pub(crate) fn const_eligibility(
        &self,
        parameter: &GenericConstReference,
    ) -> Option<TypeConstraintConstEligibility> {
        self.const_parameters
            .binary_search_by(|row| row.parameter.cmp(parameter))
            .ok()
            .map(|index| self.const_parameters[index].eligibility)
    }

    pub(crate) fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&GenericTypeReference, &TypeConstraintParameterEligibility)>
    {
        self.type_parameters
            .iter()
            .map(|row| (&row.parameter, &row.eligibility))
    }

    /// Accepts only the exact declaration inventory and monotone continuation
    /// transitions. Rigid and completed bindable roles are stable;
    /// `FutureEligible` alone may become `Bindable` in a later group.
    pub(super) fn accepts_continuation_scope(&self, other: &Self) -> bool {
        self.template_scope == other.template_scope
            && self.effects.accepts_continuation_scope(&other.effects)
            && self.rigid_types == other.rigid_types
            && self.rigid_consts == other.rigid_consts
            && self.type_parameters.len() == other.type_parameters.len()
            && self
                .type_parameters
                .iter()
                .zip(&other.type_parameters)
                .all(|(completed, next)| {
                    completed.parameter == next.parameter
                        && matches!(
                            (completed.eligibility, next.eligibility),
                            (
                                TypeConstraintParameterEligibility::Rigid,
                                TypeConstraintParameterEligibility::Rigid,
                            ) | (
                                TypeConstraintParameterEligibility::Bindable,
                                TypeConstraintParameterEligibility::Bindable,
                            ) | (
                                TypeConstraintParameterEligibility::FutureEligible,
                                TypeConstraintParameterEligibility::FutureEligible
                                    | TypeConstraintParameterEligibility::Bindable,
                            )
                        )
                })
            && self.const_parameters.len() == other.const_parameters.len()
            && self
                .const_parameters
                .iter()
                .zip(&other.const_parameters)
                .all(|(completed, next)| {
                    completed.parameter == next.parameter
                        && matches!(
                            (completed.eligibility, next.eligibility),
                            (
                                TypeConstraintConstEligibility::Rigid,
                                TypeConstraintConstEligibility::Rigid,
                            ) | (
                                TypeConstraintConstEligibility::Bindable,
                                TypeConstraintConstEligibility::Bindable,
                            ) | (
                                TypeConstraintConstEligibility::FutureEligible,
                                TypeConstraintConstEligibility::FutureEligible
                                    | TypeConstraintConstEligibility::Bindable,
                            )
                        )
                })
    }

    pub(super) fn required_inherited_keys(&self) -> &[GenericTypeReference] {
        &self.required_inherited.type_keys
    }

    pub(super) fn required_inherited_const_keys(&self) -> &[GenericConstReference] {
        &self.required_inherited.const_keys
    }

    pub(crate) fn const_iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&GenericConstReference, &TypeConstraintConstEligibility)>
    {
        self.const_parameters
            .iter()
            .map(|row| (&row.parameter, &row.eligibility))
    }
}

fn scope_invariant(
    invariant: super::TypeConstraintParameterScopeInvariant,
) -> super::TypeConstraintInvariant {
    super::TypeConstraintInvariant::ParameterScope(invariant)
}

#[cfg(test)]
fn scope_error(
    invariant: super::TypeConstraintParameterScopeInvariant,
) -> super::TypeConstraintError {
    super::TypeConstraintError::Invariant(scope_invariant(invariant))
}

fn validate_scope_rows<'a, I, P>(
    rows: I,
    const_namespace: bool,
) -> Result<(), super::TypeConstraintInvariant>
where
    I: IntoIterator<Item = &'a P>,
    P: Ord + 'a,
{
    let mut previous = None;
    for parameter in rows {
        if let Some(previous) = previous {
            if previous == parameter {
                return Err(scope_invariant(
                    super::TypeConstraintParameterScopeInvariant::DuplicateParameter,
                ));
            }
            if previous > parameter {
                return Err(scope_invariant(if const_namespace {
                    super::TypeConstraintParameterScopeInvariant::ConstParameterUnordered
                } else {
                    super::TypeConstraintParameterScopeInvariant::ParameterUnordered
                }));
            }
        }
        previous = Some(parameter);
    }
    Ok(())
}

/// Lower-owned accounting hook. Callable work sessions implement this trait;
/// the types layer only knows that an accepted delta is charged into the
/// session's pending full report before descent or allocation.
pub(crate) trait TypeConstraintAccounting {
    fn charge_constraint(
        &mut self,
        delta: &TypeConstraintWorkReport,
        limits: TypeConstraintLimits,
    ) -> Result<(), TypeConstraintError>;

    /// Commits the already checked proposal. This operation is infallible and
    /// idempotent so releasing the context and its accounting reservation
    /// cannot publish the same proposal twice.
    fn commit(&mut self);
}

/// Only a reserved lower-accounting issuer may construct a production
/// constraint context. The issuer owns the exact projected limits and
/// cancellation token, preventing a caller from pairing a session with a
/// detached budget or token.
pub(crate) trait TypeConstraintContextIssuer<'c>: TypeConstraintAccounting {
    fn context_limits(&self) -> TypeConstraintLimits;
    fn context_cancellation(&self) -> &'c AtomicBool;
}

#[cfg(test)]
pub(crate) struct LocalConstraintAccounting<'c> {
    report: TypeConstraintWorkReport,
    limits: TypeConstraintLimits,
    cancellation: &'c AtomicBool,
}

#[cfg(test)]
impl<'c> LocalConstraintAccounting<'c> {
    pub(crate) fn new(limits: TypeConstraintLimits, cancellation: &'c AtomicBool) -> Self {
        Self {
            report: TypeConstraintWorkReport::default(),
            limits,
            cancellation,
        }
    }
}

#[cfg(test)]
impl TypeConstraintAccounting for LocalConstraintAccounting<'_> {
    fn charge_constraint(
        &mut self,
        delta: &TypeConstraintWorkReport,
        _limits: TypeConstraintLimits,
    ) -> Result<(), TypeConstraintError> {
        self.report = self.report.checked_add(delta)?;
        Ok(())
    }

    fn commit(&mut self) {}
}

#[cfg(test)]
impl<'c> TypeConstraintContextIssuer<'c> for LocalConstraintAccounting<'c> {
    fn context_limits(&self) -> TypeConstraintLimits {
        self.limits
    }

    fn context_cancellation(&self) -> &'c AtomicBool {
        self.cancellation
    }
}

/// Cancellation and checked-accounting context for a constraint component.
/// `A` is the one accounting authority; `D` carries the source/branch
/// inventory used by the correlated transaction without importing callable
/// declarations into the types layer.
pub(crate) struct TypeConstraintContext<'c, A: TypeConstraintAccounting, D: ConstraintDomain> {
    limits: TypeConstraintLimits,
    cancellation: &'c AtomicBool,
    accounting: A,
    lexical_scope: GenericScope,
    work: TypeConstraintWorkReport,
    domain: PhantomData<fn() -> D>,
}

impl<'c, A, D> TypeConstraintContext<'c, A, D>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    pub(crate) fn with_accounting(accounting: A) -> Self
    where
        A: TypeConstraintContextIssuer<'c>,
    {
        let limits = accounting.context_limits();
        let cancellation = accounting.context_cancellation();
        Self {
            limits,
            cancellation,
            accounting,
            lexical_scope: GenericScope::default(),
            work: TypeConstraintWorkReport::default(),
            domain: PhantomData,
        }
    }

    pub(crate) fn check_cancelled(&self) -> Result<(), TypeConstraintError> {
        if self.cancellation.load(Ordering::Acquire) {
            Err(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
        } else {
            Ok(())
        }
    }

    pub(crate) fn parameter_eligibility(
        &self,
        parameter: &GenericTypeReference,
        view: ConstraintProjectionView<'_, D>,
    ) -> Option<TypeConstraintParameterEligibility> {
        if let GenericTypeReference::Bound(parameter) = parameter {
            return self
                .lexical_scope
                .bound_type(parameter.depth(), parameter.slot())
                .ok()
                .map(|_| TypeConstraintParameterEligibility::Rigid);
        }
        view.applications().parameter_eligibility(parameter)
    }

    pub(crate) fn const_parameter_eligibility(
        &self,
        parameter: &GenericConstReference,
        view: ConstraintProjectionView<'_, D>,
    ) -> Option<TypeConstraintConstEligibility> {
        if let GenericConstReference::Bound(parameter) = parameter {
            return self
                .lexical_scope
                .bound_const(parameter.depth(), parameter.slot())
                .ok()
                .map(|_| TypeConstraintConstEligibility::Rigid);
        }
        view.applications().const_parameter_eligibility(parameter)
    }

    pub(crate) fn with_binder<T>(
        &mut self,
        binder: GenericBinder,
        action: impl FnOnce(&mut Self) -> Result<T, TypeConstraintError>,
    ) -> Result<T, TypeConstraintError> {
        let enclosing = self.enter_binder_scope(binder);
        let result = action(self);
        self.restore_binder_scope(enclosing);
        result
    }

    /// The iterative type projector retains the returned enclosing scope on
    /// its frame and restores it when that frame finishes. Its outer scoped
    /// call also restores the original scope on every error return.
    pub(super) fn enter_binder_scope(&mut self, binder: GenericBinder) -> GenericScope {
        let nested = self.lexical_scope.with_binder(binder);
        std::mem::replace(&mut self.lexical_scope, nested)
    }

    pub(super) fn restore_binder_scope(&mut self, enclosing: GenericScope) {
        self.lexical_scope = enclosing;
    }

    /// Required type/constant keys must be bound before final projection and
    /// publication. Future groups and rigid references remain open.
    pub(super) fn validate_type_and_const_completion(
        &self,
        path: &ConstraintPath<D>,
    ) -> Result<(), TypeConstraintError> {
        for application in path.applications.applications() {
            let scope = application.parameters();
            for (parameter, eligibility) in scope.iter() {
                if matches!(eligibility, TypeConstraintParameterEligibility::Bindable)
                    && !scope
                        .type_reference(parameter)
                        .is_some_and(|reference| path.bindings.contains_key(&reference))
                {
                    return Err(super::TypeConstraintRejection::IncompleteInstantiation {
                        parameter: parameter.clone().into(),
                    }
                    .into());
                }
            }
            for (parameter, eligibility) in scope.const_iter() {
                if matches!(eligibility, TypeConstraintConstEligibility::Bindable)
                    && !scope
                        .const_reference(parameter)
                        .is_some_and(|reference| path.const_bindings.contains_key(&reference))
                {
                    return Err(super::TypeConstraintRejection::IncompleteInstantiation {
                        parameter: parameter.clone().into(),
                    }
                    .into());
                }
            }
        }
        Ok(())
    }

    /// Validates references carried by a type node's header in the current
    /// lexical scope. Child traversal enters the node's binder separately.
    pub(crate) fn validate_type_header(
        &self,
        shape: TypeConstraintShape<'_>,
        view: ConstraintProjectionView<'_, D>,
    ) -> Result<(), TypeConstraintError> {
        match shape {
            TypeConstraintShape::Generic(parameter)
                if self.parameter_eligibility(parameter, view).is_none() =>
            {
                Err(super::references::type_out_of_scope(parameter))
            }
            TypeConstraintShape::Array {
                len: super::super::ArrayLength::Generic(parameter),
                ..
            } if self.const_parameter_eligibility(parameter, view).is_none() => {
                Err(super::references::const_out_of_scope(parameter))
            }
            TypeConstraintShape::Array {
                len: super::super::ArrayLength::Error(_) | super::super::ArrayLength::Inferred,
                ..
            } => Err(super::TypeConstraintRejection::UnresolvedType.into()),
            _ => Ok(()),
        }
    }

    /// Opens formal references in the declaration or function-scheme template.
    /// Actual operand values enter unchanged and do not use this boundary.
    pub(crate) fn open_template_type(
        &mut self,
        ty: &TypeKind,
        path: &ConstraintPath<D>,
        application: ConstraintApplicationId,
    ) -> Result<TypeKind, TypeConstraintError> {
        let application = path.applications.require_application(application)?;
        self.with_template_scope(application, |context| {
            super::references::map_type(
                ty,
                &super::references::OpenTemplateReferences,
                application,
                path,
                context,
            )
        })
    }

    #[cfg(test)]
    pub(crate) fn open_template_length(
        &mut self,
        length: &super::super::ArrayLength,
        path: &ConstraintPath<D>,
        application: ConstraintApplicationId,
    ) -> Result<super::super::ArrayLength, TypeConstraintError> {
        let application = path.applications.require_application(application)?;
        self.with_template_scope(application, |context| {
            super::references::map_length(
                length,
                &super::references::OpenTemplateReferences,
                application,
                path,
                context,
            )
        })
    }

    fn with_template_scope<R>(
        &mut self,
        application: &ConstraintApplicationScope<D>,
        operation: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let template = application
            .parameters()
            .completed_contract()
            .template_scope()
            .clone();
        let enclosing = std::mem::replace(&mut self.lexical_scope, template);
        let result = operation(self);
        self.lexical_scope = enclosing;
        result
    }

    pub(super) fn lexical_scope(&self) -> &GenericScope {
        &self.lexical_scope
    }
    pub(crate) fn effect_eligibility(
        &self,
        variable: &GenericEffectReference,
        view: ConstraintProjectionView<'_, D>,
    ) -> Option<EffectConstraintEligibility> {
        match variable {
            GenericEffectReference::Bound(parameter) => self
                .lexical_scope
                .bound_effect(parameter.depth(), parameter.slot())
                .ok()
                .map(|_| EffectConstraintEligibility::Rigid),
            _ => view.applications().effect_eligibility(variable),
        }
    }

    pub(crate) fn validate_effect_row(
        &self,
        row: &EffectRow,
        view: ConstraintProjectionView<'_, D>,
    ) -> Result<(), TypeConstraintError> {
        let variables = row.variables().map_err(|_| {
            effect_invariant(super::TypeConstraintEffectInvariantKind::UnknownRow, None)
        })?;
        for variable in variables {
            if self.effect_eligibility(variable, view).is_none() {
                return Err(effect_invariant(
                    super::TypeConstraintEffectInvariantKind::ForeignVariable,
                    Some(variable.clone()),
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn enter_node(&mut self) -> Result<(), TypeConstraintError> {
        self.charge_counter(1, Counter::Nodes)
    }

    pub(crate) fn charge_source_probe(&mut self) -> Result<(), TypeConstraintError> {
        self.charge_counter(1, Counter::SourceProbes)
    }

    pub(crate) fn charge_materialization(&mut self) -> Result<(), TypeConstraintError> {
        self.charge_counter(1, Counter::Materializations)
    }

    pub(super) fn start_path(
        &mut self,
        application: ConstraintApplicationScope<D>,
    ) -> Result<ConstraintPath<D>, TypeConstraintError> {
        self.start_path_with_imported(application, None)
    }

    pub(super) fn start_path_with_imported(
        &mut self,
        application: ConstraintApplicationScope<D>,
        imported: Option<super::ImportedGenericParameterScopeLease>,
    ) -> Result<ConstraintPath<D>, TypeConstraintError> {
        self.charge_counter(1, Counter::Branches)?;
        let effects =
            EffectConstraintEnvironment::new(&application.parameters().opened_effect_variables())
                .map_err(TypeConstraintError::from)?;
        Ok(ConstraintPath::empty_with_imported(
            application,
            effects,
            imported,
        ))
    }

    pub(crate) fn fork_path(
        &mut self,
        path: &ConstraintPath<D>,
    ) -> Result<ConstraintPath<D>, TypeConstraintError> {
        self.charge_counter(1, Counter::Branches)?;
        Ok(path.clone())
    }

    /// Extend only this frontier row. Other rows keep their original scope
    /// inventory, so an unchosen application's variables never become eligible.
    pub(super) fn admit_application(
        &mut self,
        mut path: ConstraintPath<D>,
        application: ConstraintApplicationScope<D>,
    ) -> Result<ConstraintPath<D>, TypeConstraintError> {
        self.check_cancelled()?;
        path.applications
            .validate_admission(&application)
            .map_err(|error| {
                TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(error))
            })?;
        // Copy-on-write scope insertion visits the existing inventory. Admit
        // that work and the new effect rows before allocating or mutating it.
        let nodes = path
            .applications
            .len()
            .checked_add(1)
            .and_then(|count| count.checked_add(application.effects().variables().count()))
            .and_then(|count| u64::try_from(count).ok())
            .ok_or(TypeConstraintError::Abort(
                TypeConstraintAbort::ArithmeticOverflow,
            ))?;
        self.charge_counter(nodes, Counter::Nodes)?;
        path.effects
            .admit_variables(&application.parameters().opened_effect_variables())
            .map_err(TypeConstraintError::from)?;
        std::sync::Arc::make_mut(&mut path.applications)
            .admit(application)
            .map_err(|error| {
                TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(error))
            })?;
        Ok(path)
    }

    pub(crate) fn add_binding(
        &mut self,
        path: ConstraintPath<D>,
        parameter: GenericTypeReference,
        value: &TypeKind,
        value_shape: TypeConstraintShape<'_>,
    ) -> Result<Option<ConstraintPath<D>>, TypeConstraintError> {
        self.check_cancelled()?;
        if path.bindings.contains_key(&parameter) {
            return Ok(Some(path));
        }
        match self.parameter_eligibility(&parameter, path.projection_view()) {
            None => {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::ParameterScope(
                        TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope {
                            parameter,
                        },
                    ),
                ));
            }
            Some(TypeConstraintParameterEligibility::Rigid) => {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::ParameterScope(
                        TypeConstraintParameterScopeInvariant::RigidBinding { parameter },
                    ),
                ));
            }
            Some(_) => {}
        }
        let binding_count = path
            .bindings
            .len()
            .checked_add(path.const_bindings.len())
            .and_then(|count| count.checked_add(1))
            .and_then(|count| u64::try_from(count).ok())
            .ok_or(TypeConstraintError::Abort(
                TypeConstraintAbort::ArithmeticOverflow,
            ))?;
        self.charge_binding(binding_count)?;
        let mut path = path;
        if occurs_in_shape(value_shape, &parameter, path.projection_view(), self)? {
            // A back-edge is evidence, not an immediate candidate failure.
            // The close phase can discard this row while retaining a valid
            // sibling and gives later source failures precedence.
            path.deferred_cycles
                .parameters
                .insert(parameter.clone().into());
        }
        path.bindings.insert(parameter, value.clone());
        Ok(Some(path))
    }

    pub(crate) fn add_const_binding(
        &mut self,
        path: ConstraintPath<D>,
        parameter: GenericConstReference,
        value: &super::super::ArrayLength,
    ) -> Result<Option<ConstraintPath<D>>, TypeConstraintError> {
        self.check_cancelled()?;
        if path.const_bindings.contains_key(&parameter) {
            return Ok(Some(path));
        }
        match self.const_parameter_eligibility(&parameter, path.projection_view()) {
            None => {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::ParameterScope(
                        TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope {
                            parameter,
                        },
                    ),
                ));
            }
            Some(TypeConstraintConstEligibility::Rigid) => {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::ParameterScope(
                        TypeConstraintParameterScopeInvariant::RigidConstBinding { parameter },
                    ),
                ));
            }
            Some(
                TypeConstraintConstEligibility::Bindable
                | TypeConstraintConstEligibility::FutureEligible,
            ) => {}
        }
        let binding_count = path
            .bindings
            .len()
            .checked_add(path.const_bindings.len())
            .and_then(|count| count.checked_add(1))
            .and_then(|count| u64::try_from(count).ok())
            .ok_or(TypeConstraintError::Abort(
                TypeConstraintAbort::ArithmeticOverflow,
            ))?;
        self.charge_binding(binding_count)?;
        let mut path = path;
        if super::normalization::const_occurs_in(value, &parameter, path.projection_view(), self)? {
            path.deferred_cycles
                .parameters
                .insert(parameter.clone().into());
        }
        path.const_bindings.insert(parameter, value.clone());
        Ok(Some(path))
    }

    pub(crate) fn add_sealed_binding(
        &mut self,
        path: &mut ConstraintPath<D>,
        parameter: GenericTypeReference,
        value: TypeKind,
    ) -> Result<(), TypeConstraintError> {
        self.check_cancelled()?;
        let binding_count = path
            .bindings
            .len()
            .checked_add(path.const_bindings.len())
            .and_then(|count| count.checked_add(1))
            .and_then(|count| u64::try_from(count).ok())
            .ok_or(TypeConstraintError::Abort(
                TypeConstraintAbort::ArithmeticOverflow,
            ))?;
        self.charge_binding(binding_count)?;
        let shape = value.constraint_shape();
        if occurs_in_shape(shape, &parameter, path.projection_view(), self)? {
            path.deferred_cycles
                .parameters
                .insert(parameter.clone().into());
        }
        path.bindings.insert(parameter, value);
        Ok(())
    }

    pub(crate) fn add_sealed_const_binding(
        &mut self,
        path: &mut ConstraintPath<D>,
        parameter: GenericConstReference,
        value: super::super::ArrayLength,
    ) -> Result<(), TypeConstraintError> {
        self.check_cancelled()?;
        let binding_count = path
            .bindings
            .len()
            .checked_add(path.const_bindings.len())
            .and_then(|count| count.checked_add(1))
            .and_then(|count| u64::try_from(count).ok())
            .ok_or(TypeConstraintError::Abort(
                TypeConstraintAbort::ArithmeticOverflow,
            ))?;
        self.charge_binding(binding_count)?;
        if super::normalization::const_occurs_in(&value, &parameter, path.projection_view(), self)?
        {
            path.deferred_cycles
                .parameters
                .insert(parameter.clone().into());
        }
        path.const_bindings.insert(parameter, value);
        Ok(())
    }

    /// Restores a row from the opaque completed-solution owner. Canonicality,
    /// scope, and occurs checks have already been sealed by that owner; this
    /// operation only charges the new path and transfers the row.
    pub(super) fn restore_completed_binding(
        &mut self,
        path: &mut ConstraintPath<D>,
        application: ConstraintApplicationId,
        parameter: GenericTypeReference,
        value: TypeKind,
    ) -> Result<(), TypeConstraintError> {
        let reference = path
            .applications
            .require_application(application)?
            .parameters()
            .type_reference(&parameter)
            .ok_or_else(|| {
                TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(
                    TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope {
                        parameter: parameter.into(),
                    },
                ))
            })?;
        let binding_count = path
            .bindings
            .len()
            .checked_add(path.const_bindings.len())
            .and_then(|count| count.checked_add(1))
            .and_then(|count| u64::try_from(count).ok())
            .ok_or(TypeConstraintError::Abort(
                TypeConstraintAbort::ArithmeticOverflow,
            ))?;
        self.charge_binding(binding_count)?;
        assert!(
            path.bindings.insert(reference, value).is_none(),
            "completed solution rows are uniquely sealed before restoration"
        );
        Ok(())
    }

    pub(super) fn restore_completed_const_binding(
        &mut self,
        path: &mut ConstraintPath<D>,
        application: ConstraintApplicationId,
        parameter: GenericConstReference,
        value: super::super::ArrayLength,
    ) -> Result<(), TypeConstraintError> {
        let reference = path
            .applications
            .require_application(application)?
            .parameters()
            .const_reference(&parameter)
            .ok_or_else(|| {
                TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(
                    TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope {
                        parameter: parameter.into(),
                    },
                ))
            })?;
        let binding_count = path
            .bindings
            .len()
            .checked_add(path.const_bindings.len())
            .and_then(|count| count.checked_add(1))
            .and_then(|count| u64::try_from(count).ok())
            .ok_or(TypeConstraintError::Abort(
                TypeConstraintAbort::ArithmeticOverflow,
            ))?;
        self.charge_binding(binding_count)?;
        assert!(
            path.const_bindings.insert(reference, value).is_none(),
            "completed const solution rows are uniquely sealed before restoration"
        );
        Ok(())
    }

    fn charge_binding(&mut self, actual: u64) -> Result<(), TypeConstraintError> {
        self.check_cancelled()?;
        let next_work = self
            .work
            .work
            .checked_add(1)
            .ok_or(TypeConstraintError::Abort(
                TypeConstraintAbort::ArithmeticOverflow,
            ))?;
        let next_bindings = self
            .work
            .bindings
            .checked_add(1)
            .ok_or(TypeConstraintError::Abort(
                TypeConstraintAbort::ArithmeticOverflow,
            ))?;
        if next_work > self.limits.max_work {
            return Err(TypeConstraintError::Abort(TypeConstraintAbort::WorkLimit {
                requested: 1,
                consumed: self.work.work,
                limit: self.limits.max_work,
            }));
        }
        if actual > self.limits.max_bindings {
            return Err(TypeConstraintError::Abort(
                TypeConstraintAbort::BindingLimit {
                    actual,
                    limit: self.limits.max_bindings,
                },
            ));
        }
        let mut delta = TypeConstraintWorkReport::ZERO;
        delta.work = 1;
        delta.bindings = 1;
        self.charge_accounting(&delta)?;
        self.work.work = next_work;
        self.work.bindings = next_bindings;
        Ok(())
    }

    fn charge_counter(&mut self, units: u64, counter: Counter) -> Result<(), TypeConstraintError> {
        self.check_cancelled()?;
        let next_work = self
            .work
            .work
            .checked_add(units)
            .ok_or(TypeConstraintError::Abort(
                TypeConstraintAbort::ArithmeticOverflow,
            ))?;
        let next_counter = counter.checked_add(&self.work, units)?;
        if next_work > self.limits.max_work {
            return Err(TypeConstraintError::Abort(TypeConstraintAbort::WorkLimit {
                requested: units,
                consumed: self.work.work,
                limit: self.limits.max_work,
            }));
        }
        counter.check_limit(next_counter, self.limits)?;
        let mut delta = TypeConstraintWorkReport::ZERO;
        delta.work = units;
        match counter {
            Counter::Nodes => delta.nodes = units,
            Counter::Branches => delta.branches = units,
            Counter::SourceProbes => delta.source_probes = units,
            Counter::Materializations => delta.materializations = units,
        }
        self.charge_accounting(&delta)?;
        self.work.work = next_work;
        counter.assign(&mut self.work, next_counter);
        Ok(())
    }

    fn charge_accounting(
        &mut self,
        delta: &TypeConstraintWorkReport,
    ) -> Result<(), TypeConstraintError> {
        self.accounting.charge_constraint(delta, self.limits)
    }

    pub(crate) fn accounting_mut(&mut self) -> &mut A {
        &mut self.accounting
    }
}

impl<A: TypeConstraintAccounting, D: ConstraintDomain> Drop for TypeConstraintContext<'_, A, D> {
    fn drop(&mut self) {
        // The context owns the reservation. A driver may finish or be dropped
        // while this context is still loaned to a surrounding component.
        self.accounting.commit();
    }
}

impl<A, D> TypeCompatibilityControl for TypeConstraintContext<'_, A, D>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    type Error = TypeConstraintError;

    fn enter(&mut self, _expected: &TypeKind, _actual: &TypeKind) -> Result<(), Self::Error> {
        self.enter_node()
    }
}

impl<A: TypeConstraintAccounting, D: ConstraintDomain> crate::effect_row::DecisionControl
    for TypeConstraintContext<'_, A, D>
{
    type Error = TypeConstraintError;

    fn charge(&mut self, _work: crate::effect_row::DecisionWork) -> Result<(), Self::Error> {
        // Semantic decision visits and emitted nodes consume the same existing
        // structural/work counters, including cancellation and external accounting.
        self.enter_node()
    }
}

#[derive(Clone, Copy)]
enum Counter {
    Nodes,
    Branches,
    SourceProbes,
    Materializations,
}

impl Counter {
    fn checked_add(
        self,
        report: &TypeConstraintWorkReport,
        units: u64,
    ) -> Result<u64, TypeConstraintError> {
        match self {
            Self::Nodes => report.nodes.checked_add(units),
            Self::Branches => report.branches.checked_add(units),
            Self::SourceProbes => report.source_probes.checked_add(units),
            Self::Materializations => report.materializations.checked_add(units),
        }
        .ok_or(TypeConstraintError::Abort(
            TypeConstraintAbort::ArithmeticOverflow,
        ))
    }

    fn check_limit(
        self,
        actual: u64,
        limits: TypeConstraintLimits,
    ) -> Result<(), TypeConstraintError> {
        let limit = match self {
            Self::Nodes => limits.max_nodes,
            Self::Branches => limits.max_branches,
            Self::SourceProbes => limits.max_source_probes,
            Self::Materializations => limits.max_materializations,
        };
        if actual <= limit {
            return Ok(());
        }
        Err(match self {
            Self::Nodes => {
                TypeConstraintError::Abort(TypeConstraintAbort::NodeLimit { actual, limit })
            }
            Self::Branches => {
                TypeConstraintError::Abort(TypeConstraintAbort::BranchLimit { actual, limit })
            }
            Self::SourceProbes => {
                TypeConstraintError::Abort(TypeConstraintAbort::SourceProbeLimit { actual, limit })
            }
            Self::Materializations => {
                TypeConstraintError::Abort(TypeConstraintAbort::MaterializationLimit {
                    actual,
                    limit,
                })
            }
        })
    }

    fn assign(self, report: &mut TypeConstraintWorkReport, actual: u64) {
        match self {
            Self::Nodes => report.nodes = actual,
            Self::Branches => report.branches = actual,
            Self::SourceProbes => report.source_probes = actual,
            Self::Materializations => report.materializations = actual,
        }
    }
}
