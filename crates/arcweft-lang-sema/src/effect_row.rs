use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{effect_model::CallableId, effects::EffectSet, types::GenericEffectReference};

pub(crate) use arcweft_core::effect_row::{
    DecisionControl, DecisionEncoding, DecisionWork, EffectFormula, MembershipEncoding,
};

/// The canonical shared predicate instantiated with semantic references.
pub type EffectPredicate<V = GenericEffectReference> = arcweft_core::effect_row::EffectPredicate<V>;

#[cfg(test)]
mod scope_tests;

/// Equality visits the canonical graph grammar before inspecting its immutable
/// rows. It consumes the same surrounding decision budget as construction.
struct EffectEqualityControl<'a, C>(&'a mut C);

impl<V, C: DecisionControl> DecisionEncoding<V> for EffectEqualityControl<'_, C> {
    type Error = C::Error;

    fn tag(&mut self, _: u8) -> Result<(), Self::Error> {
        self.0.charge(DecisionWork::Visit)
    }

    fn count(&mut self, _: usize) -> Result<(), Self::Error> {
        self.0.charge(DecisionWork::Visit)
    }

    fn variable(&mut self, _: &V) -> Result<(), Self::Error> {
        self.0.charge(DecisionWork::Visit)
    }
}

impl<V, C: DecisionControl> MembershipEncoding<V> for EffectEqualityControl<'_, C> {
    fn effect(&mut self, _: &crate::effects::EffectId) -> Result<(), Self::Error> {
        self.0.charge(DecisionWork::Visit)
    }
}

/// An inferred finite effect-set formula, or an annotation still awaiting its
/// owning inference phase. Unknown rows never supply an empty-set substitute.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct EffectRow {
    formula: Option<EffectFormula<GenericEffectReference>>,
}

/// Exact substitutions produced when a polymorphic callable is instantiated.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectSubstitution(BTreeMap<GenericEffectReference, EffectRow>);

/// Eligibility of one issuer-backed effect variable in a lower constraint
/// run. Bindable variables close in this run; future-eligible variables remain
/// absent until a constraint touches them or their declaration position enters
/// the active callable group.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum EffectConstraintEligibility {
    Rigid,
    Bindable,
    FutureEligible,
}

/// One authorized variable row used to initialize a path-local effect
/// environment. Rows are sealed and ordered by the types layer before they
/// reach this lower algebra.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EffectConstraintVariable {
    variable: GenericEffectReference,
    eligibility: EffectConstraintEligibility,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EffectConstraintParameter {
    eligibility: EffectConstraintEligibility,
    restored: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EffectConstraintCompletion {
    pub(crate) bindings: Vec<(GenericEffectReference, EffectRow)>,
    pub(crate) predicate: EffectPredicate,
}

/// Branch-local higher-order effect constraints. This is deliberately not an
/// exact substitution table: directional function relations need lower and
/// upper bounds plus residual-aware tail edges before a minimal fixed point
/// can be sealed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EffectConstraintEnvironment {
    parameters: BTreeMap<GenericEffectReference, EffectConstraintParameter>,
    predicate: EffectPredicate<GenericEffectReference>,
}

/// Closed failures produced by the path-local effect algebra. The types layer
/// maps only `MissingEffects` to candidate rejection; every other variant is
/// malformed authority or a non-canonical sealed seed.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum EffectConstraintEnvironmentError {
    #[error("unknown effect row reached issuer-backed lower")]
    UnknownRow,
    #[error("effect variable is outside the authorized lower scope")]
    ForeignVariable { variable: GenericEffectReference },
    #[error("effect constraint scope is duplicated or not canonically ordered")]
    NonCanonicalScope,
    #[error("effect rows are not in the subset relation")]
    MissingEffects { missing: EffectSet },
    #[error("effect constraints do not have a pointwise least solution")]
    AmbiguousCompletion,
}

/// Closed or bounded effect-row evidence for one callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectRowSummary {
    callable: CallableId,
    inferred: EffectRow,
    upper_bound: Option<EffectRow>,
    forbidden: EffectRow,
}

/// Stable report projection of callable effect rows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectRowReport {
    summaries: BTreeMap<CallableId, EffectRowSummary>,
}

/// Closed effect-row evidence for one callable at a crate boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosedEffectRowSummary {
    callable: CallableId,
    inferred: EffectSet,
    upper_bound: Option<EffectSet>,
    forbidden: EffectSet,
}

/// Boundary-safe projection that contains only resolved effect sets.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClosedEffectRowReport {
    summaries: BTreeMap<CallableId, ClosedEffectRowSummary>,
}

/// Failure while binding or resolving an effect row.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EffectRowError {
    #[error("effect row is unknown and must be annotated before a dynamic call")]
    UnknownRow,
    #[error("effect reference {variable:?} is unbound")]
    UnboundVariable { variable: GenericEffectReference },
    #[error(
        "effect reference {variable:?} was already bound to {existing:?}, cannot rebind it to {requested:?}"
    )]
    ConflictingBinding {
        variable: GenericEffectReference,
        existing: Box<EffectRow>,
        requested: Box<EffectRow>,
    },
    #[error("effect reference {variable:?} participates in a cyclic row substitution")]
    CyclicBinding { variable: GenericEffectReference },
}

/// Failure while checking that one actual effect row is admitted by another.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EffectSubsetError {
    #[error("effect inference has no pointwise least solution")]
    AmbiguousInference,
    #[error("an unknown effect row cannot participate in subset checking")]
    UnknownRow,
    #[error("the permitted closed row is missing effects {missing:?}")]
    MissingEffects { missing: EffectSet },
    #[error("actual effect reference {variable:?} is unresolved against a closed permitted row")]
    UnresolvedActualTail { variable: GenericEffectReference },
    #[error("effect-row substitution is cyclic at {variable:?}")]
    CyclicSubstitution { variable: GenericEffectReference },
    #[error("effect reference {variable:?} has incompatible row bindings")]
    ConflictingBinding {
        variable: GenericEffectReference,
        existing: Box<EffectRow>,
        requested: Box<EffectRow>,
    },
}

/// Failure while resolving a report into closed boundary evidence.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EffectRowCloseError {
    #[error("effect row report could not resolve `{callable}`: {source}")]
    Unresolved {
        callable: CallableId,
        #[source]
        source: Box<EffectRowError>,
    },
}

impl EffectConstraintVariable {
    pub(crate) const fn new(
        variable: GenericEffectReference,
        eligibility: EffectConstraintEligibility,
    ) -> Self {
        Self {
            variable,
            eligibility,
        }
    }

    pub(crate) const fn variable(&self) -> &GenericEffectReference {
        &self.variable
    }

    pub(crate) const fn eligibility(&self) -> EffectConstraintEligibility {
        self.eligibility
    }
}

impl EffectRow {
    pub(crate) fn equal_with<C: DecisionControl>(
        &self,
        other: &Self,
        control: &mut C,
    ) -> Result<bool, C::Error> {
        self.encode(&mut EffectEqualityControl(control))?;
        other.encode(&mut EffectEqualityControl(control))?;
        Ok(self == other)
    }

    /// Encodes the complete formula through its owning transcript's primitives.
    /// Unknown annotation state is distinct from a known empty effect set.
    pub(crate) fn encode<E: MembershipEncoding<GenericEffectReference>>(
        &self,
        encoder: &mut E,
    ) -> Result<(), E::Error> {
        match &self.formula {
            None => encoder.tag(0),
            Some(formula) => {
                encoder.tag(1)?;
                formula.encode(encoder)
            }
        }
    }
    pub fn unknown() -> Self {
        Self::default()
    }

    pub fn closed(concrete: EffectSet) -> Self {
        Self {
            formula: Some(EffectFormula::literal(concrete, None)),
        }
    }

    pub fn open(concrete: EffectSet, variable: GenericEffectReference) -> Self {
        Self {
            formula: Some(EffectFormula::literal(concrete, Some(variable))),
        }
    }

    fn known(&self) -> Result<&EffectFormula<GenericEffectReference>, EffectRowError> {
        self.formula.as_ref().ok_or(EffectRowError::UnknownRow)
    }

    pub const fn is_known(&self) -> bool {
        self.formula.is_some()
    }

    /// Canonical shared effect grammar. An annotation awaiting inference has
    /// no formula; runtime projection must reject that unresolved state.
    pub const fn formula(
        &self,
    ) -> Option<&arcweft_core::effect_row::EffectFormula<GenericEffectReference>> {
        self.formula.as_ref()
    }

    pub fn is_closed(&self) -> bool {
        self.formula.as_ref().is_some_and(EffectFormula::is_closed)
    }

    pub fn closed_value(&self) -> Option<EffectSet> {
        self.formula
            .as_ref()
            .filter(|formula| formula.is_closed())
            .map(EffectFormula::constant_effects)
    }

    pub fn is_empty(&self) -> bool {
        self.closed_value()
            .is_some_and(|effects| effects.is_empty())
    }

    pub(crate) fn semantic_cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (&self.formula, &other.formula) {
            (Some(left), Some(right)) => left.cmp(right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    }

    /// Unconditional named effects. This projection is not a proof that the
    /// complete row is closed; consumers requiring a concrete row use resolve.
    pub fn constant_effects(&self) -> Result<EffectSet, EffectRowError> {
        Ok(self.known()?.constant_effects())
    }

    pub(crate) fn variables(
        &self,
    ) -> Result<impl Iterator<Item = &GenericEffectReference>, EffectRowError> {
        Ok(self.known()?.variables())
    }

    pub(crate) fn union<C: DecisionControl>(
        &self,
        other: &Self,
        control: &mut C,
    ) -> Result<Self, C::Error>
    where
        C::Error: From<EffectRowError>,
    {
        let left = self.known().map_err(C::Error::from)?;
        let right = other.known().map_err(C::Error::from)?;
        Ok(Self {
            formula: Some(left.union(right, control)?),
        })
    }

    /// Proves a row relation without inferring assignments. Closed capability
    /// sets retain EffectId's coverage relation; symbolic membership must be
    /// valid for every valuation of its still-rigid references.
    pub(crate) fn is_covered_by<C: DecisionControl>(
        &self,
        permitted: &Self,
        control: &mut C,
    ) -> Result<bool, C::Error> {
        let (Some(actual), Some(permitted)) = (&self.formula, &permitted.formula) else {
            return Ok(false);
        };
        if actual.is_closed() && permitted.is_closed() {
            control.charge(DecisionWork::Visit)?;
            return Ok(actual
                .constant_effects()
                .effects_not_covered_by(&permitted.constant_effects())
                .is_empty());
        }
        Ok(actual.subset(permitted, control)?.is_unconstrained())
    }

    pub(crate) fn try_map_variables<C: DecisionControl>(
        &self,
        control: &mut C,
        mapping: &mut impl FnMut(
            &GenericEffectReference,
            &mut C,
        ) -> Result<GenericEffectReference, C::Error>,
    ) -> Result<Self, C::Error> {
        let Some(formula) = &self.formula else {
            return Ok(Self::unknown());
        };
        Ok(Self {
            formula: Some(formula.map_references(control, mapping)?),
        })
    }

    /// Simultaneous substitution: replacement rows belong to the caller and
    /// are never looked up again in the callee's parameter namespace.
    pub(crate) fn try_substitute_variables<C: DecisionControl>(
        &self,
        control: &mut C,
        mapping: &mut impl FnMut(&GenericEffectReference, &mut C) -> Result<Self, C::Error>,
    ) -> Result<Self, C::Error>
    where
        C::Error: From<EffectRowError>,
    {
        let Some(formula) = &self.formula else {
            return Ok(Self::unknown());
        };
        if formula.is_closed() {
            return Ok(self.clone());
        }
        let Some(replacements) = effect_replacements(formula.variables(), control, mapping)? else {
            return Ok(Self::unknown());
        };
        Ok(Self {
            formula: Some(formula.substitute(&replacements, control)?),
        })
    }

    pub fn display_label(&self) -> String {
        let Some(formula) = &self.formula else {
            return "unknown".to_owned();
        };
        let concrete = formula.constant_effects();
        if formula.is_closed() {
            return format_effect_set(&concrete);
        }
        if let Some(variable) = formula.single_reference() {
            return if concrete.is_empty() {
                format!("{{ | {} }}", variable.source_label())
            } else {
                format!(
                    "{{ {} | {} }}",
                    effect_labels(&concrete),
                    variable.source_label()
                )
            };
        }
        format!("{formula:?}")
    }

    pub fn resolve(&self, substitutions: &EffectSubstitution) -> Result<EffectSet, EffectRowError> {
        self.resolve_with(|variable| substitutions.get(variable), |_| Ok(()))
    }

    pub(crate) fn resolve_with<'rows, E: From<EffectRowError>>(
        &'rows self,
        lookup: impl Fn(&GenericEffectReference) -> Option<&'rows EffectRow>,
        visit: impl FnMut(&EffectRow) -> Result<(), E>,
    ) -> Result<EffectSet, E> {
        let row = self.resolve_partial_with(lookup, visit)?;
        let formula = row.known().map_err(E::from)?;
        if let Some(variable) = formula.variables().next() {
            return Err(E::from(EffectRowError::UnboundVariable {
                variable: variable.clone(),
            }));
        }
        Ok(formula.constant_effects())
    }

    pub fn check_subset(
        actual: &EffectRow,
        permitted: &EffectRow,
        substitution: &mut EffectSubstitution,
    ) -> Result<(), EffectSubsetError> {
        let actual = actual
            .resolve_partial(substitution)
            .map_err(EffectSubsetError::from_row_error)?;
        let permitted = permitted
            .resolve_partial(substitution)
            .map_err(EffectSubsetError::from_row_error)?;
        let mut visit = |_: &EffectRow| Ok::<(), EffectSubsetError>(());
        let mut control = RowVisitControl {
            row: &actual,
            visit: &mut visit,
        };
        let inferred = permitted
            .known()
            .map_err(EffectSubsetError::from_row_error)?
            .variables()
            .cloned()
            .collect::<BTreeSet<_>>();
        let predicate = actual
            .known()
            .map_err(EffectSubsetError::from_row_error)?
            .subset(
                permitted
                    .known()
                    .map_err(EffectSubsetError::from_row_error)?,
                &mut control,
            )?;
        let completed = predicate.complete(&inferred, &mut control)?;
        if completed.admissibility.is_impossible() {
            return Err(EffectSubsetError::MissingEffects {
                missing: completed.admissibility.rejected_labels(&mut control)?,
            });
        }
        if !completed.admissibility.is_unconstrained() {
            let variable = actual
                .variables()
                .map_err(EffectSubsetError::from_row_error)?
                .next()
                .expect("a residual admission predicate retains an actual variable");
            return Err(EffectSubsetError::UnresolvedActualTail {
                variable: variable.clone(),
            });
        }
        let least = completed
            .least
            .ok_or(EffectSubsetError::AmbiguousInference)?;
        let mut next = substitution.clone();
        for (variable, formula) in least {
            next.bind_row(
                variable,
                &Self {
                    formula: Some(formula),
                },
            )
            .map_err(EffectSubsetError::from_row_error)?;
        }
        *substitution = next;
        Ok(())
    }

    pub(crate) fn resolve_partial(
        &self,
        substitutions: &EffectSubstitution,
    ) -> Result<Self, EffectRowError> {
        self.resolve_partial_with(|variable| substitutions.get(variable), |_| Ok(()))
    }

    pub(crate) fn resolve_partial_with<'rows, E: From<EffectRowError>>(
        &'rows self,
        lookup: impl Fn(&GenericEffectReference) -> Option<&'rows EffectRow>,
        mut visit: impl FnMut(&EffectRow) -> Result<(), E>,
    ) -> Result<Self, E> {
        enum Task<'a> {
            Enter(GenericEffectReference),
            Finish(GenericEffectReference, &'a EffectRow),
        }
        visit(self)?;
        let formula = self.known().map_err(E::from)?;
        if formula.is_closed() {
            return Ok(self.clone());
        }
        let mut pending = formula
            .variables()
            .cloned()
            .map(Task::Enter)
            .collect::<Vec<_>>();
        let mut visiting = BTreeSet::new();
        let mut completed = BTreeMap::new();
        let mut absent = BTreeSet::new();
        while let Some(task) = pending.pop() {
            match task {
                Task::Enter(variable) => {
                    if completed.contains_key(&variable) || absent.contains(&variable) {
                        continue;
                    }
                    let Some(row) = lookup(&variable) else {
                        absent.insert(variable);
                        continue;
                    };
                    visit(row)?;
                    if !visiting.insert(variable.clone()) {
                        return Err(E::from(EffectRowError::CyclicBinding {
                            variable: variable.clone(),
                        }));
                    }
                    let row_formula = row.known().map_err(E::from)?;
                    pending.push(Task::Finish(variable, row));
                    pending.extend(row_formula.variables().cloned().map(Task::Enter));
                }
                Task::Finish(variable, row) => {
                    let mut control = RowVisitControl {
                        row,
                        visit: &mut visit,
                    };
                    let formula = row
                        .known()
                        .map_err(E::from)?
                        .substitute(&completed, &mut control)?;
                    visiting.remove(&variable);
                    completed.insert(variable, formula);
                }
            }
        }
        let mut control = RowVisitControl {
            row: self,
            visit: &mut visit,
        };
        Ok(Self {
            formula: Some(formula.substitute(&completed, &mut control)?),
        })
    }
}

impl TryFrom<EffectRow> for EffectFormula<GenericEffectReference> {
    type Error = EffectRowError;

    fn try_from(row: EffectRow) -> Result<Self, Self::Error> {
        row.formula.ok_or(EffectRowError::UnknownRow)
    }
}

fn effect_replacements<'a, C: DecisionControl>(
    variables: impl Iterator<Item = &'a GenericEffectReference>,
    control: &mut C,
    mapping: &mut impl FnMut(&GenericEffectReference, &mut C) -> Result<EffectRow, C::Error>,
) -> Result<Option<BTreeMap<GenericEffectReference, EffectFormula<GenericEffectReference>>>, C::Error>
where
    C::Error: From<EffectRowError>,
{
    let mut replacements = BTreeMap::new();
    for variable in variables {
        control.charge(DecisionWork::Visit)?;
        if let std::collections::btree_map::Entry::Vacant(entry) =
            replacements.entry(variable.clone())
        {
            let replacement = mapping(variable, control)?;
            let Some(formula) = &replacement.formula else {
                return Ok(None);
            };
            entry.insert(formula.clone());
        }
    }
    Ok(Some(replacements))
}

/// Adapts the existing caller-owned row visitation boundary to decision work.
/// It does not own an accountant, counter, cancellation flag, or mutable row.
struct RowVisitControl<'a, F> {
    row: &'a EffectRow,
    visit: &'a mut F,
}

impl<E, F: FnMut(&EffectRow) -> Result<(), E>> DecisionControl for RowVisitControl<'_, F> {
    type Error = E;
    fn charge(&mut self, _: DecisionWork) -> Result<(), E> {
        (self.visit)(self.row)
    }
}
impl EffectSubsetError {
    fn from_row_error(error: EffectRowError) -> Self {
        match error {
            EffectRowError::UnknownRow => Self::UnknownRow,
            EffectRowError::UnboundVariable { variable } => Self::UnresolvedActualTail { variable },
            EffectRowError::ConflictingBinding {
                variable,
                existing,
                requested,
            } => Self::ConflictingBinding {
                variable,
                existing,
                requested,
            },
            EffectRowError::CyclicBinding { variable } => Self::CyclicSubstitution { variable },
        }
    }
}

impl EffectConstraintEnvironment {
    pub(crate) fn new(
        variables: &[EffectConstraintVariable],
    ) -> Result<Self, EffectConstraintEnvironmentError> {
        let mut environment = Self {
            parameters: BTreeMap::new(),
            predicate: EffectPredicate::unconstrained(),
        };
        environment.admit_variables(variables)?;
        Ok(environment)
    }

    /// Admit a disjoint application inventory while retaining the complete
    /// existing relation and inherited values.
    pub(crate) fn admit_variables(
        &mut self,
        variables: &[EffectConstraintVariable],
    ) -> Result<(), EffectConstraintEnvironmentError> {
        if variables
            .windows(2)
            .any(|rows| rows[0].variable >= rows[1].variable)
            || variables.iter().any(|row| {
                self.parameters.get(&row.variable).is_some_and(|existing| {
                    existing.eligibility != EffectConstraintEligibility::Rigid
                        || row.eligibility != EffectConstraintEligibility::Rigid
                })
            })
        {
            return Err(EffectConstraintEnvironmentError::NonCanonicalScope);
        }
        for row in variables {
            self.parameters
                .entry(row.variable.clone())
                .or_insert_with(|| EffectConstraintParameter {
                    eligibility: row.eligibility,
                    restored: false,
                });
        }
        Ok(())
    }

    pub(crate) fn validate_row(
        &self,
        row: &EffectRow,
    ) -> Result<(), EffectConstraintEnvironmentError> {
        let variables = row
            .variables()
            .map_err(|_| EffectConstraintEnvironmentError::UnknownRow)?;
        for variable in variables {
            if !self.parameters.contains_key(variable)
                && !matches!(variable, GenericEffectReference::Bound(_))
            {
                return Err(EffectConstraintEnvironmentError::ForeignVariable {
                    variable: variable.clone(),
                });
            }
        }
        Ok(())
    }

    fn quantified<C: DecisionControl>(
        &self,
        include_future: bool,
        control: &mut C,
    ) -> Result<BTreeSet<GenericEffectReference>, C::Error> {
        let mut variables = BTreeSet::new();
        for (variable, parameter) in &self.parameters {
            control.charge(DecisionWork::Visit)?;
            if parameter.eligibility == EffectConstraintEligibility::Bindable
                || (include_future
                    && parameter.eligibility == EffectConstraintEligibility::FutureEligible)
            {
                variables.insert(variable.clone());
            }
        }
        Ok(variables)
    }

    fn formula<C: DecisionControl>(
        &self,
        row: &EffectRow,
        control: &mut C,
    ) -> Result<EffectFormula<GenericEffectReference>, C::Error>
    where
        C::Error: From<EffectConstraintEnvironmentError>,
    {
        self.validate_row(row).map_err(C::Error::from)?;
        row.known()
            .expect("validated row")
            .map_references(control, &mut |variable, _| Ok(variable.clone()))
    }

    fn admit_predicate<C: DecisionControl>(
        &self,
        added: &EffectPredicate<GenericEffectReference>,
        control: &mut C,
    ) -> Result<EffectPredicate<GenericEffectReference>, C::Error>
    where
        C::Error: From<EffectConstraintEnvironmentError>,
    {
        let predicate = self.predicate.and(added, control)?;
        let quantified = self.quantified(true, control)?;
        let admitted = predicate.project(&quantified, control)?;
        if admitted.is_impossible() {
            return Err(EffectConstraintEnvironmentError::MissingEffects {
                missing: admitted.rejected_labels(control)?,
            }
            .into());
        }
        Ok(predicate)
    }

    /// Restore an exact completed binding. All validation and graph work
    /// precede mutation, and the caller retains the surrounding work ledger.
    pub(crate) fn restore_completed_inherited<C: DecisionControl>(
        &mut self,
        variable: GenericEffectReference,
        row: &EffectRow,
        control: &mut C,
    ) -> Result<(), C::Error>
    where
        C::Error: From<EffectConstraintEnvironmentError>,
    {
        let parameter = self
            .parameters
            .get(&variable)
            .expect("completed scope retains each sealed variable");
        assert!(
            !parameter.restored,
            "completed rows are uniquely sealed before restoration"
        );
        let actual = EffectFormula::variable(variable.clone(), control)?;
        let expected = self.formula(row, control)?;
        let equality = actual
            .subset(&expected, control)?
            .and(&expected.subset(&actual, control)?, control)?;
        let predicate = self.admit_predicate(&equality, control)?;
        control.charge(DecisionWork::Visit)?;
        let parameter = self
            .parameters
            .get_mut(&variable)
            .expect("variable validated above");
        parameter.restored = true;
        self.predicate = predicate;
        Ok(())
    }

    /// Add a directional relation without choosing any witness. Later source
    /// constraints may resolve a relation that currently has no least witness.
    pub(crate) fn constrain_subset<C: DecisionControl>(
        &mut self,
        actual: &EffectRow,
        permitted: &EffectRow,
        control: &mut C,
    ) -> Result<(), C::Error>
    where
        C::Error: From<EffectConstraintEnvironmentError>,
    {
        self.validate_row(actual).map_err(C::Error::from)?;
        self.validate_row(permitted).map_err(C::Error::from)?;
        let actual_formula = self.formula(actual, control)?;
        let permitted_formula = self.formula(permitted, control)?;
        let added = actual_formula.subset(&permitted_formula, control)?;
        let predicate = self.admit_predicate(&added, control)?;
        control.charge(DecisionWork::Visit)?;
        self.predicate = predicate;
        Ok(())
    }

    pub(crate) fn complete<C: DecisionControl>(
        &self,
        control: &mut C,
    ) -> Result<EffectConstraintCompletion, C::Error>
    where
        C::Error: From<EffectConstraintEnvironmentError>,
    {
        let quantified = self.quantified(false, control)?;
        let completed = self.predicate.complete(&quantified, control)?;
        if completed.admissibility.is_impossible() {
            return Err(EffectConstraintEnvironmentError::MissingEffects {
                missing: completed.admissibility.rejected_labels(control)?,
            }
            .into());
        }
        let least = completed
            .least
            .ok_or_else(|| C::Error::from(EffectConstraintEnvironmentError::AmbiguousCompletion))?;
        let mut bindings = Vec::new();
        for (variable, parameter) in &self.parameters {
            control.charge(DecisionWork::Visit)?;
            if parameter.eligibility == EffectConstraintEligibility::Bindable {
                let formula = least[variable]
                    .map_references(control, &mut |reference, _| Ok(reference.clone()))?;
                bindings.push((
                    variable.clone(),
                    EffectRow {
                        formula: Some(formula),
                    },
                ));
            }
        }
        Ok(EffectConstraintCompletion {
            bindings,
            predicate: completed.admissibility,
        })
    }

    pub(crate) fn restore_predicate<C: DecisionControl>(
        &mut self,
        predicate: &EffectPredicate,
        control: &mut C,
    ) -> Result<(), C::Error>
    where
        C::Error: From<EffectConstraintEnvironmentError>,
    {
        for reference in predicate.variables() {
            control.charge(DecisionWork::Visit)?;
            if !self.parameters.contains_key(reference)
                && !matches!(reference, GenericEffectReference::Bound(_))
            {
                return Err(EffectConstraintEnvironmentError::ForeignVariable {
                    variable: reference.clone(),
                }
                .into());
            }
        }
        let admitted = self.admit_predicate(predicate, control)?;
        control.charge(DecisionWork::Visit)?;
        self.predicate = admitted;
        Ok(())
    }

    pub(crate) fn bindings<C: DecisionControl>(
        &self,
        control: &mut C,
    ) -> Result<Vec<(GenericEffectReference, EffectRow)>, C::Error>
    where
        C::Error: From<EffectConstraintEnvironmentError>,
    {
        Ok(self.complete(control)?.bindings)
    }

    pub(crate) fn substitution<C: DecisionControl>(
        &self,
        control: &mut C,
    ) -> Result<EffectSubstitution, C::Error>
    where
        C::Error: From<EffectConstraintEnvironmentError>,
    {
        let mut rows = BTreeMap::new();
        for (variable, row) in self.bindings(control)? {
            control.charge(DecisionWork::Visit)?;
            rows.insert(variable, row);
        }
        Ok(EffectSubstitution(rows))
    }
}
impl EffectRowSummary {
    pub fn closed(
        callable: CallableId,
        inferred: EffectSet,
        upper_bound: Option<EffectSet>,
        forbidden: EffectSet,
    ) -> Self {
        Self {
            callable,
            inferred: EffectRow::closed(inferred),
            upper_bound: upper_bound.map(EffectRow::closed),
            forbidden: EffectRow::closed(forbidden),
        }
    }

    /// Records an inferred callable row whose residual tail is closed by the
    /// owning analysis substitution after fixed-point propagation.
    pub fn open_inferred(
        callable: CallableId,
        inferred: EffectSet,
        tail: GenericEffectReference,
        upper_bound: Option<EffectSet>,
        forbidden: EffectSet,
    ) -> Self {
        Self {
            callable,
            inferred: EffectRow::open(inferred, tail),
            upper_bound: upper_bound.map(EffectRow::closed),
            forbidden: EffectRow::closed(forbidden),
        }
    }

    pub const fn callable(&self) -> &CallableId {
        &self.callable
    }

    pub const fn inferred(&self) -> &EffectRow {
        &self.inferred
    }

    pub const fn upper_bound(&self) -> Option<&EffectRow> {
        self.upper_bound.as_ref()
    }

    pub const fn forbidden(&self) -> &EffectRow {
        &self.forbidden
    }

    pub fn resolve_closed(
        &self,
        substitutions: &EffectSubstitution,
    ) -> Result<ClosedEffectRowSummary, EffectRowError> {
        Ok(ClosedEffectRowSummary::new(
            self.callable.clone(),
            self.inferred.resolve(substitutions)?,
            self.upper_bound
                .as_ref()
                .map(|row| row.resolve(substitutions))
                .transpose()?,
            self.forbidden.resolve(substitutions)?,
        ))
    }
}

impl EffectRowReport {
    pub fn new(summaries: impl IntoIterator<Item = EffectRowSummary>) -> Self {
        let summaries = summaries
            .into_iter()
            .map(|summary| (summary.callable.clone(), summary))
            .collect();
        Self { summaries }
    }

    pub fn summary(&self, callable: &CallableId) -> Option<&EffectRowSummary> {
        self.summaries.get(callable)
    }

    pub fn summaries(&self) -> impl ExactSizeIterator<Item = (&CallableId, &EffectRowSummary)> {
        self.summaries.iter()
    }

    pub fn resolve_closed(
        &self,
        substitutions: &EffectSubstitution,
    ) -> Result<ClosedEffectRowReport, EffectRowCloseError> {
        let summaries = self
            .summaries
            .values()
            .map(|summary| {
                summary
                    .resolve_closed(substitutions)
                    .map(|closed| (closed.callable.clone(), closed))
                    .map_err(|source| EffectRowCloseError::Unresolved {
                        callable: summary.callable.clone(),
                        source: Box::new(source),
                    })
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(ClosedEffectRowReport { summaries })
    }
}

impl ClosedEffectRowSummary {
    pub fn new(
        callable: CallableId,
        inferred: EffectSet,
        upper_bound: Option<EffectSet>,
        forbidden: EffectSet,
    ) -> Self {
        Self {
            callable,
            inferred,
            upper_bound,
            forbidden,
        }
    }

    pub const fn callable(&self) -> &CallableId {
        &self.callable
    }

    pub const fn inferred(&self) -> &EffectSet {
        &self.inferred
    }

    pub const fn upper_bound(&self) -> Option<&EffectSet> {
        self.upper_bound.as_ref()
    }

    pub const fn forbidden(&self) -> &EffectSet {
        &self.forbidden
    }
}

impl ClosedEffectRowReport {
    pub fn new(summaries: impl IntoIterator<Item = ClosedEffectRowSummary>) -> Self {
        let summaries = summaries
            .into_iter()
            .map(|summary| (summary.callable.clone(), summary))
            .collect();
        Self { summaries }
    }

    pub fn summary(&self, callable: &CallableId) -> Option<&ClosedEffectRowSummary> {
        self.summaries.get(callable)
    }

    pub fn summaries(
        &self,
    ) -> impl ExactSizeIterator<Item = (&CallableId, &ClosedEffectRowSummary)> {
        self.summaries.iter()
    }
}

impl EffectSubstitution {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn from_rows(
        rows: impl IntoIterator<Item = (GenericEffectReference, EffectRow)>,
    ) -> Self {
        Self(rows.into_iter().collect())
    }

    pub fn bind_exact(
        &mut self,
        variable: GenericEffectReference,
        effects: EffectSet,
    ) -> Result<(), EffectRowError> {
        self.bind_row(variable, &EffectRow::closed(effects))
    }

    pub fn get(&self, variable: &GenericEffectReference) -> Option<&EffectRow> {
        self.0.get(variable)
    }

    /// The declaration substitution API has no work lease. It still uses the
    /// same transitive row resolution and simultaneous predicate substitution;
    /// bounded application/type folds supply their own DecisionControl instead.
    pub(crate) fn resolve_predicate(
        &self,
        predicate: &EffectPredicate,
    ) -> Result<EffectPredicate, EffectRowError> {
        struct Unmetered;
        impl DecisionControl for Unmetered {
            type Error = EffectRowError;
            fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
                Ok(())
            }
        }
        predicate.try_substitute_variables(&mut Unmetered, &mut |reference, _| {
            EffectRow::open(EffectSet::new(), reference.clone()).resolve_partial(self)
        })
    }

    pub(crate) fn bind_row(
        &mut self,
        variable: GenericEffectReference,
        requested: &EffectRow,
    ) -> Result<(), EffectRowError> {
        let requested = requested.resolve_partial(self)?;
        if requested
            .variables()?
            .any(|reference| *reference == variable)
        {
            if requested.known()?.single_reference() == Some(&variable)
                && requested.constant_effects()?.is_empty()
            {
                return Ok(());
            }
            return Err(EffectRowError::CyclicBinding {
                variable: variable.clone(),
            });
        }
        if let Some(existing) = self.0.get(&variable) {
            let existing = existing.resolve_partial(self)?;
            if existing == requested {
                return Ok(());
            }
            return Err(EffectRowError::ConflictingBinding {
                variable: variable.clone(),
                existing: Box::new(existing),
                requested: Box::new(requested),
            });
        }
        self.0.insert(variable, requested);
        Ok(())
    }
}

fn format_effect_set(effects: &EffectSet) -> String {
    let labels = effect_labels(effects);
    if labels.is_empty() {
        "{ }".to_owned()
    } else {
        format!("{{ {labels} }}")
    }
}

fn effect_labels(effects: &EffectSet) -> String {
    effects.to_labels().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::EffectSet;

    fn parameter(ordinal: u32) -> GenericEffectReference {
        parameter_in(801, ordinal)
    }

    fn parameter_in(owner: u64, ordinal: u32) -> GenericEffectReference {
        crate::types::GenericEffectParameterId::new(
            crate::types::GenericParameterOwnerId::Detached(
                crate::types::DetachedGenericOwnerId::new(owner),
            ),
            ordinal,
        )
        .into()
    }

    struct TestDecisionControl;

    impl DecisionControl for TestDecisionControl {
        type Error = EffectConstraintEnvironmentError;

        fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    struct TestSubstitutionControl;

    impl DecisionControl for TestSubstitutionControl {
        type Error = EffectRowError;

        fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[test]
    fn effect_row_display_label_covers_closed_open_and_unknown_rows() {
        let variable = parameter(3);
        assert_eq!(EffectRow::unknown().display_label(), "unknown");
        assert_eq!(EffectRow::closed(EffectSet::new()).display_label(), "{ }");
        assert_eq!(
            EffectRow::closed(EffectSet::from_labels(["fs.read"]).expect("valid row"))
                .display_label(),
            "{ fs.read }"
        );
        assert_eq!(
            EffectRow::open(EffectSet::new(), variable.clone()).display_label(),
            format!("{{ | {} }}", variable.source_label())
        );
        assert_eq!(
            EffectRow::open(
                EffectSet::from_labels(["log.write"]).expect("valid row"),
                variable.clone()
            )
            .display_label(),
            format!("{{ log.write | {} }}", variable.source_label())
        );
    }

    #[test]
    fn unknown_effect_rows_survive_reference_mapping_without_becoming_closed() {
        let mut control = TestDecisionControl;
        let mut mapper_called = false;
        let mapped = EffectRow::unknown()
            .try_map_variables(&mut control, &mut |reference, _| {
                mapper_called = true;
                Ok(reference.clone())
            })
            .expect("mapping a type does not resolve an unknown effect row");

        assert!(!mapper_called);
        assert!(!mapped.is_known());
        assert!(!mapped.is_empty());
        assert_eq!(mapped.display_label(), "unknown");
    }

    #[test]
    fn unknown_effect_rows_survive_substitution_without_becoming_closed() {
        let mut control = TestSubstitutionControl;
        let mut mapper_called = false;
        let substituted = EffectRow::unknown()
            .try_substitute_variables(&mut control, &mut |_, _| {
                mapper_called = true;
                Ok(EffectRow::closed(EffectSet::new()))
            })
            .expect("substitution does not resolve an unknown effect row");

        assert!(!mapper_called);
        assert!(!substituted.is_known());
        assert!(!substituted.is_empty());
        assert_eq!(substituted.display_label(), "unknown");

        let tail = parameter(9);
        let mut control = TestSubstitutionControl;
        let substituted = EffectRow::open(EffectSet::new(), tail)
            .try_substitute_variables(&mut control, &mut |_, _| Ok(EffectRow::unknown()))
            .expect("an unknown replacement remains unknown");
        assert!(!substituted.is_known());
        assert!(!substituted.is_empty());
    }

    #[test]
    fn resolves_a_polymorphic_effect_tail() {
        let variable = parameter(0);
        let row = EffectRow::open(
            EffectSet::from_labels(["log.write"]).expect("valid concrete row"),
            variable.clone(),
        );
        let mut substitutions = EffectSubstitution::new();
        substitutions
            .bind_exact(
                variable.clone(),
                EffectSet::from_labels(["fs.read"]).expect("valid tail row"),
            )
            .expect("fresh variable binds");
        assert_eq!(
            row.resolve(&substitutions)
                .expect("bound row resolves")
                .to_labels(),
            vec!["fs.read", "log.write"]
        );
    }

    #[test]
    fn closed_subset_reports_every_sorted_residual_effect() {
        let actual = EffectRow::closed(
            EffectSet::from_labels(["net.open", "control.suspend", "log.write"])
                .expect("valid actual row"),
        );
        let permitted =
            EffectRow::closed(EffectSet::from_labels(["log.write"]).expect("valid permitted row"));

        assert_eq!(
            EffectRow::check_subset(&actual, &permitted, &mut EffectSubstitution::new()),
            Err(EffectSubsetError::MissingEffects {
                missing: EffectSet::from_labels(["control.suspend", "net.open"])
                    .expect("valid missing row"),
            })
        );
    }

    #[test]
    fn open_permitted_tail_absorbs_the_complete_residual_row() {
        let permitted_tail = parameter(4);
        let actual = EffectRow::closed(
            EffectSet::from_labels(["control.suspend", "fs.read", "log.write"])
                .expect("valid actual row"),
        );
        let permitted = EffectRow::open(
            EffectSet::from_labels(["log.write"]).expect("valid permitted head"),
            permitted_tail.clone(),
        );
        let mut substitution = EffectSubstitution::new();

        EffectRow::check_subset(&actual, &permitted, &mut substitution)
            .expect("open tail accepts residual effects");

        assert_eq!(
            substitution.get(&permitted_tail),
            Some(&EffectRow::closed(
                EffectSet::from_labels(["control.suspend", "fs.read"]).expect("valid residual row")
            ))
        );
    }

    #[test]
    fn open_permitted_tail_retains_only_uncovered_actual_membership() {
        let actual_tail = parameter(2);
        let permitted_tail = parameter(3);
        let actual = EffectRow::open(
            EffectSet::from_labels(["fs.read", "log.write"]).expect("valid actual head"),
            actual_tail.clone(),
        );
        let permitted = EffectRow::open(
            EffectSet::from_labels(["log.write"]).expect("valid permitted head"),
            permitted_tail.clone(),
        );
        let mut substitution = EffectSubstitution::new();

        EffectRow::check_subset(&actual, &permitted, &mut substitution)
            .expect("permitted tail retains actual tail");

        let inferred = substitution.get(&permitted_tail).expect("inferred row");
        for labels in [
            vec![],
            vec!["log.write"],
            vec!["net.open"],
            vec!["log.write", "net.open"],
        ] {
            let value = EffectSet::from_labels(labels).unwrap();
            let expected = value
                .union(&EffectSet::from_labels(["fs.read"]).unwrap())
                .difference(&EffectSet::from_labels(["log.write"]).unwrap());
            let mut valuation = EffectSubstitution::new();
            valuation.bind_exact(actual_tail.clone(), value).unwrap();
            assert_eq!(inferred.resolve(&valuation).unwrap(), expected);
        }
    }

    #[test]
    fn prebound_open_tail_is_constrained_without_overwrite() {
        let residual_tail = parameter(8);
        let permitted_tail = parameter(9);
        let mut substitution = EffectSubstitution::new();
        substitution
            .bind_row(
                permitted_tail.clone(),
                &EffectRow::open(
                    EffectSet::from_labels(["fs.read"]).expect("valid existing head"),
                    residual_tail.clone(),
                ),
            )
            .expect("fresh permitted tail binds");
        let actual = EffectRow::closed(
            EffectSet::from_labels(["fs.read", "log.write"]).expect("valid actual row"),
        );
        let permitted = EffectRow::open(EffectSet::new(), permitted_tail.clone());

        EffectRow::check_subset(&actual, &permitted, &mut substitution)
            .expect("residual tail receives only the remaining effect");

        assert_eq!(
            substitution.get(&permitted_tail),
            Some(&EffectRow::open(
                EffectSet::from_labels(["fs.read"]).expect("valid retained head"),
                residual_tail.clone()
            ))
        );
        assert_eq!(
            substitution.get(&residual_tail),
            Some(&EffectRow::closed(
                EffectSet::from_labels(["log.write"]).expect("valid residual binding")
            ))
        );
    }

    #[test]
    fn unresolved_actual_tail_fails_against_a_closed_row() {
        let actual_tail = parameter(12);
        let actual = EffectRow::open(EffectSet::new(), actual_tail.clone());
        let permitted = EffectRow::closed(EffectSet::new());

        assert_eq!(
            EffectRow::check_subset(&actual, &permitted, &mut EffectSubstitution::new()),
            Err(EffectSubsetError::UnresolvedActualTail {
                variable: actual_tail
            })
        );
    }

    #[test]
    fn unknown_rows_fail_closed_during_subset_checking() {
        assert_eq!(
            EffectRow::check_subset(
                &EffectRow::unknown(),
                &EffectRow::closed(EffectSet::new()),
                &mut EffectSubstitution::new()
            ),
            Err(EffectSubsetError::UnknownRow)
        );
        assert_eq!(
            EffectRow::check_subset(
                &EffectRow::closed(EffectSet::new()),
                &EffectRow::unknown(),
                &mut EffectSubstitution::new()
            ),
            Err(EffectSubsetError::UnknownRow)
        );
    }

    #[test]
    fn unknown_row_fails_closed() {
        assert_eq!(
            EffectRow::unknown().resolve(&EffectSubstitution::new()),
            Err(EffectRowError::UnknownRow)
        );
    }

    #[test]
    fn report_resolves_to_closed_boundary_rows() {
        let variable = parameter(0);
        let callable = CallableId::new("fn.with_open_row");
        let row = EffectRowSummary {
            callable: callable.clone(),
            inferred: EffectRow::open(
                EffectSet::from_labels(["log.write"]).expect("valid concrete row"),
                variable.clone(),
            ),
            upper_bound: Some(EffectRow::closed(
                EffectSet::from_labels(["fs.read", "log.write"]).expect("valid bound row"),
            )),
            forbidden: EffectRow::closed(EffectSet::from_labels(["net.open"]).expect("valid row")),
        };
        let mut substitutions = EffectSubstitution::new();
        substitutions
            .bind_exact(
                variable.clone(),
                EffectSet::from_labels(["fs.read"]).expect("valid tail row"),
            )
            .expect("fresh variable binds");

        let report = EffectRowReport::new([row])
            .resolve_closed(&substitutions)
            .expect("report resolves");
        let summary = report.summary(&callable).expect("callable summary");
        assert_eq!(summary.inferred().to_labels(), vec!["fs.read", "log.write"]);
        assert_eq!(
            summary.upper_bound().expect("upper bound").to_labels(),
            vec!["fs.read", "log.write"]
        );
        assert_eq!(summary.forbidden().to_labels(), vec!["net.open"]);
    }

    #[test]
    fn report_close_error_names_unresolved_callable() {
        let variable = parameter(7);
        let callable = CallableId::new("fn.needs_row");
        let report = EffectRowReport::new([EffectRowSummary {
            callable: callable.clone(),
            inferred: EffectRow::open(EffectSet::new(), variable.clone()),
            upper_bound: None,
            forbidden: EffectRow::closed(EffectSet::new()),
        }]);

        assert_eq!(
            report.resolve_closed(&EffectSubstitution::new()),
            Err(EffectRowCloseError::Unresolved {
                callable,
                source: Box::new(EffectRowError::UnboundVariable { variable }),
            })
        );
    }

    #[test]
    fn constraint_environment_computes_residual_aware_minimal_fixed_point() {
        let source = parameter(0);
        let target = parameter(1);
        let mut environment = EffectConstraintEnvironment::new(&[
            EffectConstraintVariable::new(source.clone(), EffectConstraintEligibility::Bindable),
            EffectConstraintVariable::new(target.clone(), EffectConstraintEligibility::Bindable),
        ])
        .expect("canonical scope");
        let covered = EffectSet::from_labels(["fs.read"]).expect("effect");
        let residual = EffectSet::from_labels(["net.open"]).expect("effect");

        environment
            .constrain_subset(
                &EffectRow::open(covered.clone(), source.clone()),
                &EffectRow::open(covered.clone(), target.clone()),
                &mut TestDecisionControl,
            )
            .expect("tail edge");
        environment
            .constrain_subset(
                &EffectRow::closed(residual.clone()),
                &EffectRow::open(EffectSet::new(), source.clone()),
                &mut TestDecisionControl,
            )
            .expect("source lower bound");

        assert_eq!(
            environment
                .bindings(&mut TestDecisionControl)
                .expect("minimal solution"),
            vec![
                (source, EffectRow::closed(residual.clone())),
                (target, EffectRow::closed(residual)),
            ]
        );
    }

    #[test]
    fn constraint_environment_rejects_only_well_formed_subset_conflict_transactionally() {
        let variable = parameter(0);
        let mut environment = EffectConstraintEnvironment::new(&[EffectConstraintVariable::new(
            variable.clone(),
            EffectConstraintEligibility::Bindable,
        )])
        .expect("canonical scope");
        let permitted = EffectSet::from_labels(["fs.read"]).expect("effect");
        environment
            .constrain_subset(
                &EffectRow::open(EffectSet::new(), variable.clone()),
                &EffectRow::closed(permitted),
                &mut TestDecisionControl,
            )
            .expect("upper bound");
        let before = environment.clone();

        assert!(matches!(
            environment.constrain_subset(
                &EffectRow::closed(EffectSet::from_labels(["net.open"]).expect("effect")),
                &EffectRow::open(EffectSet::new(), variable.clone()),
                &mut TestDecisionControl,
            ),
            Err(EffectConstraintEnvironmentError::MissingEffects { .. })
        ));
        assert_eq!(environment, before);
    }

    #[test]
    fn constraint_environment_classifies_unknown_and_foreign_rows_as_invariants() {
        let variable = parameter(0);
        let environment = EffectConstraintEnvironment::new(&[EffectConstraintVariable::new(
            variable.clone(),
            EffectConstraintEligibility::Bindable,
        )])
        .expect("canonical scope");
        assert_eq!(
            environment.validate_row(&EffectRow::unknown()),
            Err(EffectConstraintEnvironmentError::UnknownRow)
        );
        let foreign = parameter_in(802, 0);
        assert_eq!(
            environment.validate_row(&EffectRow::open(EffectSet::new(), foreign.clone())),
            Err(EffectConstraintEnvironmentError::ForeignVariable { variable: foreign })
        );
    }
}
