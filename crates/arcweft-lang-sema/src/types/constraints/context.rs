//! Candidate-owned accounting context, limits, and scope.
//!
//! This module owns the persistent cancellation/work context. The transaction
//! lives in the sibling `transaction` module and only borrows it per phase.

#[cfg(test)]
use std::collections::BTreeMap;
use std::{
    collections::BTreeSet,
    marker::PhantomData,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::effect_row::{
    EffectConstraintEligibility, EffectConstraintEnvironment, EffectConstraintVariable,
    EffectIssuerRebindError, EffectRow, EffectVar, EffectVarIssuer,
};

use super::super::{
    GenericConstParameterId, GenericTypeParameterId, TypeCompatibilityControl, TypeKind,
};
#[cfg(test)]
use super::NoConstraintClient;
use super::{
    ConstraintDomain, ConstraintPath, TypeConstraintAbort, TypeConstraintError,
    TypeConstraintInvariant, TypeConstraintParameterScopeInvariant, TypeConstraintShape,
    effect_invariant, map_effect_environment_error, occurs_in_shape,
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

/// Checked lower work counters returned by every constraint run.
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

/// Complete issuer-backed effect-variable inventory for one lower run. The
/// callable preparation owner supplies canonical rows and the exact inherited
/// key contract; lower never derives scope from the types it happens to see.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeConstraintEffectScope {
    variables: Box<[EffectConstraintVariable]>,
    required_inherited: Box<[EffectVar]>,
}

impl TypeConstraintEffectScope {
    pub(crate) fn seal_call_scope<V, R>(
        variables: V,
        required_inherited: R,
    ) -> Result<Self, super::TypeConstraintInvariant>
    where
        V: IntoIterator<Item = EffectConstraintVariable>,
        R: IntoIterator<Item = EffectVar>,
    {
        let variables = variables.into_iter().collect::<Vec<_>>();
        let issuer = variables.first().map(|row| row.variable().issuer());
        if variables
            .windows(2)
            .any(|rows| rows[0].variable() >= rows[1].variable())
            || variables.iter().enumerate().any(|(index, row)| {
                issuer != Some(row.variable().issuer())
                    || u32::try_from(index).ok() != Some(row.variable().index())
            })
        {
            return Err(effect_scope_invariant(
                super::TypeConstraintEffectInvariantKind::DuplicateOrUnorderedScope,
                variables.windows(2).find_map(|rows| {
                    (rows[0].variable() >= rows[1].variable()).then_some(rows[1].variable())
                }),
            ));
        }
        let required_inherited = required_inherited.into_iter().collect::<Vec<_>>();
        if required_inherited.windows(2).any(|rows| rows[0] >= rows[1]) {
            return Err(effect_scope_invariant(
                super::TypeConstraintEffectInvariantKind::DuplicateOrUnorderedInherited,
                required_inherited
                    .windows(2)
                    .find_map(|rows| (rows[0] >= rows[1]).then_some(rows[1])),
            ));
        }
        for variable in &required_inherited {
            let Some(row) = variables.iter().find(|row| row.variable() == *variable) else {
                return Err(effect_scope_invariant(
                    super::TypeConstraintEffectInvariantKind::RequiredInheritedOutOfScope,
                    Some(*variable),
                ));
            };
            if !matches!(row.eligibility(), EffectConstraintEligibility::Bindable) {
                return Err(effect_scope_invariant(
                    super::TypeConstraintEffectInvariantKind::RequiredInheritedNotBindable,
                    Some(*variable),
                ));
            }
        }
        Ok(Self {
            variables: variables.into_boxed_slice(),
            required_inherited: required_inherited.into_boxed_slice(),
        })
    }

    pub(crate) fn variables(&self) -> &[EffectConstraintVariable] {
        &self.variables
    }

    pub(crate) fn eligibility(&self, variable: EffectVar) -> Option<EffectConstraintEligibility> {
        self.variables
            .binary_search_by_key(&variable, |row| row.variable())
            .ok()
            .map(|index| self.variables[index].eligibility())
    }

    pub(crate) fn required_inherited(&self) -> &[EffectVar] {
        &self.required_inherited
    }

    /// Accepts only the exact variable inventory and the monotone continuation
    /// transition `FutureEligible -> FutureEligible | Bindable`. Completed
    /// bindable rows cannot become future rows again.
    pub(super) fn accepts_continuation_scope(&self, other: &Self) -> bool {
        self.variables.len() == other.variables.len()
            && self
                .variables
                .iter()
                .zip(&other.variables)
                .all(|(completed, next)| {
                    completed.variable() == next.variable()
                        && matches!(
                            (completed.eligibility(), next.eligibility()),
                            (
                                EffectConstraintEligibility::Bindable,
                                EffectConstraintEligibility::Bindable,
                            ) | (
                                EffectConstraintEligibility::FutureEligible,
                                EffectConstraintEligibility::FutureEligible
                                    | EffectConstraintEligibility::Bindable,
                            )
                        )
                })
    }

    /// Rebind the complete prepared scope at the checked-call boundary. This
    /// is a bijective issuer transition owned by the sealed solution; it does
    /// not reconstruct or revalidate solution rows downstream.
    pub(super) fn checked_rebind_issuer(
        &self,
        prepared: EffectVarIssuer,
        checked: EffectVarIssuer,
        authorized_ordinals: &BTreeSet<u32>,
    ) -> Result<Self, EffectIssuerRebindError> {
        let rebind = |variable: EffectVar| {
            if variable.issuer() != prepared {
                return Err(EffectIssuerRebindError::ForeignVariable { variable });
            }
            if !authorized_ordinals.contains(&variable.index()) {
                return Err(EffectIssuerRebindError::UnauthorizedVariable { variable });
            }
            Ok(variable.rebind_issuer(prepared, checked))
        };
        let variables = self
            .variables
            .iter()
            .map(|row| {
                Ok(EffectConstraintVariable::new(
                    rebind(row.variable())?,
                    row.eligibility(),
                ))
            })
            .collect::<Result<Box<[_]>, _>>()?;
        let required_inherited = self
            .required_inherited
            .iter()
            .copied()
            .map(rebind)
            .collect::<Result<Box<[_]>, _>>()?;
        Ok(Self {
            variables,
            required_inherited,
        })
    }
}

fn effect_scope_invariant(
    kind: super::TypeConstraintEffectInvariantKind,
    variable: Option<EffectVar>,
) -> super::TypeConstraintInvariant {
    match effect_invariant(kind, variable) {
        TypeConstraintError::Invariant(invariant) => invariant,
        TypeConstraintError::Rejected(_) | TypeConstraintError::Abort(_) => {
            unreachable!("effect scope errors are invariants")
        }
    }
}

/// Semantic inventory of one generic constant parameter.  Constants are
/// deliberately tracked separately from type parameters; the lower solver has
/// no const-binding solution path, so only rigid constants may enter a checked
/// scope.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum TypeConstraintConstEligibility {
    Rigid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeConstraintTypeParameterScopeRow {
    parameter: GenericTypeParameterId,
    eligibility: TypeConstraintParameterEligibility,
}

impl TypeConstraintTypeParameterScopeRow {
    pub(crate) fn new(
        parameter: GenericTypeParameterId,
        eligibility: TypeConstraintParameterEligibility,
    ) -> Self {
        Self {
            parameter,
            eligibility,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeConstraintConstParameterScopeRow {
    parameter: GenericConstParameterId,
    eligibility: TypeConstraintConstEligibility,
}

impl TypeConstraintConstParameterScopeRow {
    pub(crate) fn new(
        parameter: GenericConstParameterId,
        eligibility: TypeConstraintConstEligibility,
    ) -> Self {
        Self {
            parameter,
            eligibility,
        }
    }
}

/// Types-owned sorted contract for the inherited type-binding keys required by
/// the sealed continuation prefix.  It intentionally has no const namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RequiredInheritedBindingKeys {
    keys: Box<[GenericTypeParameterId]>,
}

/// The complete lower-visible parameter inventory for one candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeConstraintParameterScope {
    type_parameters: Box<[TypeConstraintTypeParameterScopeRow]>,
    const_parameters: Box<[TypeConstraintConstParameterScopeRow]>,
    required_inherited: RequiredInheritedBindingKeys,
}

impl TypeConstraintParameterScope {
    /// Seal the exact kind-separated inventories and the inherited-key
    /// contract.  This is the only production constructor; callers must
    /// provide rows in canonical order and cannot ask the lower layer to sort
    /// or repair them.
    pub(crate) fn seal_call_scope<T, C, R>(
        type_parameters: T,
        const_parameters: C,
        required_inherited_keys: R,
    ) -> Result<Self, super::TypeConstraintInvariant>
    where
        T: IntoIterator<Item = TypeConstraintTypeParameterScopeRow>,
        C: IntoIterator<Item = TypeConstraintConstParameterScopeRow>,
        R: IntoIterator<Item = GenericTypeParameterId>,
    {
        let type_parameters = type_parameters.into_iter().collect::<Vec<_>>();
        validate_scope_rows(type_parameters.iter().map(|row| &row.parameter), false)?;
        let const_parameters = const_parameters.into_iter().collect::<Vec<_>>();
        validate_scope_rows(const_parameters.iter().map(|row| &row.parameter), true)?;
        if let Some(row) = const_parameters
            .iter()
            .find(|row| !matches!(row.eligibility, TypeConstraintConstEligibility::Rigid))
        {
            return Err(scope_invariant(
                super::TypeConstraintParameterScopeInvariant::UnsupportedConstParameter {
                    parameter: row.parameter.clone(),
                },
            ));
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

        Ok(Self {
            type_parameters: type_parameters.into_boxed_slice(),
            const_parameters: const_parameters.into_boxed_slice(),
            required_inherited: RequiredInheritedBindingKeys {
                keys: required_inherited_keys.into_boxed_slice(),
            },
        })
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
            inventory.into_iter().map(|(parameter, eligibility)| {
                TypeConstraintTypeParameterScopeRow::new(parameter, eligibility)
            }),
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
            if !matches!(eligibility, TypeConstraintConstEligibility::Rigid) {
                return Err(scope_error(
                    super::TypeConstraintParameterScopeInvariant::UnsupportedConstParameter {
                        parameter,
                    },
                ));
            }
            if constants.insert(parameter, eligibility).is_some() {
                return Err(scope_error(
                    super::TypeConstraintParameterScopeInvariant::DuplicateParameter,
                ));
            }
        }
        Self::seal_call_scope(
            types.into_iter().map(|(parameter, eligibility)| {
                TypeConstraintTypeParameterScopeRow::new(parameter, eligibility)
            }),
            constants.into_iter().map(|(parameter, eligibility)| {
                TypeConstraintConstParameterScopeRow::new(parameter, eligibility)
            }),
            std::iter::empty(),
        )
        .map_err(super::TypeConstraintError::Invariant)
    }

    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self::seal_call_scope(std::iter::empty(), std::iter::empty(), std::iter::empty())
            .expect("empty test scope is valid")
    }

    pub(crate) fn eligibility(
        &self,
        parameter: &GenericTypeParameterId,
    ) -> Option<TypeConstraintParameterEligibility> {
        self.type_parameters
            .binary_search_by(|row| row.parameter.cmp(parameter))
            .ok()
            .map(|index| self.type_parameters[index].eligibility)
    }

    pub(crate) fn const_eligibility(
        &self,
        parameter: &GenericConstParameterId,
    ) -> Option<TypeConstraintConstEligibility> {
        self.const_parameters
            .binary_search_by(|row| row.parameter.cmp(parameter))
            .ok()
            .map(|index| self.const_parameters[index].eligibility)
    }

    pub(crate) fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&GenericTypeParameterId, &TypeConstraintParameterEligibility)>
    {
        self.type_parameters
            .iter()
            .map(|row| (&row.parameter, &row.eligibility))
    }

    /// Accepts only the exact declaration inventory and monotone continuation
    /// transitions. Rigid and completed bindable roles are stable;
    /// `FutureEligible` alone may become `Bindable` in a later group.
    pub(super) fn accepts_continuation_scope(&self, other: &Self) -> bool {
        self.type_parameters.len() == other.type_parameters.len()
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
            && self
                .const_parameters
                .iter()
                .map(|row| &row.parameter)
                .eq(other.const_parameters.iter().map(|row| &row.parameter))
    }

    pub(super) fn required_inherited_keys(&self) -> &[GenericTypeParameterId] {
        &self.required_inherited.keys
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
    /// idempotent so a run may invoke it from either `complete` or `Drop`.
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

/// Cancellation and checked-accounting context for one candidate relation.
/// `A` is the one accounting authority; `C` carries the source/branch
/// inventory used by the correlated transaction without importing callable
/// declarations into the types layer.
pub(crate) struct TypeConstraintContext<'c, A: TypeConstraintAccounting, D: ConstraintDomain> {
    limits: TypeConstraintLimits,
    cancellation: &'c AtomicBool,
    accounting: A,
    pub(crate) parameter_scope: TypeConstraintParameterScope,
    pub(crate) effect_scope: TypeConstraintEffectScope,
    work: TypeConstraintWorkReport,
    domain: PhantomData<fn() -> D>,
}

#[cfg(test)]
impl<'c> TypeConstraintContext<'c, LocalConstraintAccounting<'c>, NoConstraintClient> {
    pub(crate) fn new(limits: TypeConstraintLimits, cancellation: &'c AtomicBool) -> Self {
        Self::with_accounting(
            LocalConstraintAccounting::new(limits, cancellation),
            TypeConstraintParameterScope::empty(),
            TypeConstraintEffectScope::seal_call_scope([], [])
                .expect("empty test effect scope is canonical"),
        )
    }
}

impl<'c, A, D> TypeConstraintContext<'c, A, D>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    pub(crate) fn with_accounting(
        accounting: A,
        parameter_scope: TypeConstraintParameterScope,
        effect_scope: TypeConstraintEffectScope,
    ) -> Self
    where
        A: TypeConstraintContextIssuer<'c>,
    {
        let limits = accounting.context_limits();
        let cancellation = accounting.context_cancellation();
        Self {
            limits,
            cancellation,
            accounting,
            parameter_scope,
            effect_scope,
            work: TypeConstraintWorkReport::default(),
            domain: PhantomData,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_scope(
        limits: TypeConstraintLimits,
        cancellation: &'c AtomicBool,
        parameter_scope: TypeConstraintParameterScope,
    ) -> TypeConstraintContext<'c, LocalConstraintAccounting<'c>, D> {
        TypeConstraintContext::with_accounting(
            LocalConstraintAccounting::new(limits, cancellation),
            parameter_scope,
            TypeConstraintEffectScope::seal_call_scope([], [])
                .expect("empty test effect scope is canonical"),
        )
    }

    #[cfg(test)]
    pub(crate) fn with_scopes(
        limits: TypeConstraintLimits,
        cancellation: &'c AtomicBool,
        parameter_scope: TypeConstraintParameterScope,
        effect_scope: TypeConstraintEffectScope,
    ) -> TypeConstraintContext<'c, LocalConstraintAccounting<'c>, D> {
        TypeConstraintContext::with_accounting(
            LocalConstraintAccounting::new(limits, cancellation),
            parameter_scope,
            effect_scope,
        )
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
        parameter: &GenericTypeParameterId,
    ) -> Option<TypeConstraintParameterEligibility> {
        self.parameter_scope.eligibility(parameter)
    }

    pub(crate) fn const_parameter_eligibility(
        &self,
        parameter: &GenericConstParameterId,
    ) -> Option<TypeConstraintConstEligibility> {
        self.parameter_scope.const_eligibility(parameter)
    }

    pub(super) fn required_inherited_keys(&self) -> &[GenericTypeParameterId] {
        self.parameter_scope.required_inherited_keys()
    }

    pub(super) fn required_inherited_effects(&self) -> &[EffectVar] {
        self.effect_scope.required_inherited()
    }

    pub(crate) fn effect_eligibility(
        &self,
        variable: EffectVar,
    ) -> Option<EffectConstraintEligibility> {
        self.effect_scope.eligibility(variable)
    }

    pub(crate) fn validate_effect_row(&self, row: &EffectRow) -> Result<(), TypeConstraintError> {
        match row.tail() {
            crate::effect_row::EffectRowTail::Closed => Ok(()),
            crate::effect_row::EffectRowTail::Variable(variable)
                if self.effect_scope.eligibility(variable).is_some() =>
            {
                Ok(())
            }
            crate::effect_row::EffectRowTail::Variable(variable) => Err(effect_invariant(
                super::TypeConstraintEffectInvariantKind::ForeignVariable,
                Some(variable),
            )),
            crate::effect_row::EffectRowTail::Unknown => Err(effect_invariant(
                super::TypeConstraintEffectInvariantKind::UnknownRow,
                None,
            )),
        }
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

    pub(crate) fn start_path(&mut self) -> Result<ConstraintPath<D>, TypeConstraintError> {
        self.charge_counter(1, Counter::Branches)?;
        let effects = EffectConstraintEnvironment::new(self.effect_scope.variables())
            .map_err(map_effect_environment_error)?;
        Ok(ConstraintPath::empty(effects))
    }

    pub(crate) fn fork_path(
        &mut self,
        path: &ConstraintPath<D>,
    ) -> Result<ConstraintPath<D>, TypeConstraintError> {
        self.charge_counter(1, Counter::Branches)?;
        Ok(path.clone())
    }

    pub(crate) fn add_binding(
        &mut self,
        path: ConstraintPath<D>,
        parameter: GenericTypeParameterId,
        value: &TypeKind,
        value_shape: TypeConstraintShape<'_>,
    ) -> Result<Option<ConstraintPath<D>>, TypeConstraintError> {
        self.check_cancelled()?;
        if path.bindings.contains_key(&parameter) {
            return Ok(Some(path));
        }
        match self.parameter_scope.eligibility(&parameter) {
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
            .checked_add(1)
            .and_then(|count| u64::try_from(count).ok())
            .ok_or(TypeConstraintError::Abort(
                TypeConstraintAbort::ArithmeticOverflow,
            ))?;
        self.charge_binding(binding_count)?;
        let mut path = path;
        if occurs_in_shape(value_shape, &parameter, &path.bindings, self)? {
            // A back-edge is evidence, not an immediate candidate failure.
            // The close phase can discard this row while retaining a valid
            // sibling and gives later source failures precedence.
            path.deferred_cycles.parameters.insert(parameter.clone());
        }
        path.bindings.insert(parameter, value.clone());
        Ok(Some(path))
    }

    pub(crate) fn add_sealed_binding(
        &mut self,
        path: &mut ConstraintPath<D>,
        parameter: GenericTypeParameterId,
        value: TypeKind,
    ) -> Result<(), TypeConstraintError> {
        self.check_cancelled()?;
        let binding_count = path
            .bindings
            .len()
            .checked_add(1)
            .and_then(|count| u64::try_from(count).ok())
            .ok_or(TypeConstraintError::Abort(
                TypeConstraintAbort::ArithmeticOverflow,
            ))?;
        self.charge_binding(binding_count)?;
        let shape = value.constraint_shape();
        if occurs_in_shape(shape, &parameter, &path.bindings, self)? {
            path.deferred_cycles.parameters.insert(parameter.clone());
        }
        path.bindings.insert(parameter, value);
        Ok(())
    }

    /// Restores a row from the opaque completed-solution owner. Canonicality,
    /// scope, and occurs checks have already been sealed by that owner; this
    /// operation only charges the new path and transfers the row.
    pub(super) fn restore_completed_binding(
        &mut self,
        path: &mut ConstraintPath<D>,
        parameter: GenericTypeParameterId,
        value: TypeKind,
    ) -> Result<(), TypeConstraintError> {
        let binding_count = path
            .bindings
            .len()
            .checked_add(1)
            .and_then(|count| u64::try_from(count).ok())
            .ok_or(TypeConstraintError::Abort(
                TypeConstraintAbort::ArithmeticOverflow,
            ))?;
        self.charge_binding(binding_count)?;
        assert!(
            path.bindings.insert(parameter, value).is_none(),
            "completed solution rows are uniquely sealed before restoration"
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

    pub(crate) fn commit_accounting(&mut self) {
        self.accounting.commit();
    }

    pub(crate) fn accounting_mut(&mut self) -> &mut A {
        &mut self.accounting
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
