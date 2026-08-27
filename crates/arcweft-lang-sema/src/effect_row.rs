use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicU64, Ordering},
};

use thiserror::Error;

use crate::{effect_model::CallableId, effects::EffectSet};

/// Typed issuer of one effect-variable namespace.
///
/// Candidate-owned higher-order effect variables use a schema/path digest;
/// checker-local inference uses the all-zero issuer inside its private
/// substitution.  Equality therefore never aliases equal ordinals minted by
/// distinct semantic owners.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectVarIssuer([u8; 32]);

/// Type-inference variable used as the open tail of an effect row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectVar {
    issuer: EffectVarIssuer,
    ordinal: u32,
}

/// Tail state of a set-like effect row.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum EffectRowTail {
    Closed,
    Variable(EffectVar),
    /// Legacy/untyped callable. Calling through it is rejected until resolved.
    #[default]
    Unknown,
}

/// Set-like effect row `{ concrete | tail }`.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct EffectRow {
    concrete: EffectSet,
    tail: EffectRowTail,
}

/// Exact substitutions produced when a polymorphic callable is instantiated.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectSubstitution(BTreeMap<EffectVar, EffectRow>);

/// Eligibility of one issuer-backed effect variable in a lower constraint
/// run. Bindable variables close in this run; future-eligible variables remain
/// absent until a constraint touches them or their declaration position enters
/// the active callable group.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum EffectConstraintEligibility {
    Bindable,
    FutureEligible,
}

/// One authorized variable row used to initialize a path-local effect
/// environment. Rows are sealed and ordered by the types layer before they
/// reach this lower algebra.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EffectConstraintVariable {
    variable: EffectVar,
    eligibility: EffectConstraintEligibility,
}

/// A directed open-tail relation. `source - covered` must flow into `target`;
/// `covered` is the permitted row's concrete prefix and prevents redundant
/// effects from being added to the minimal target binding.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EffectConstraintEdge {
    source: EffectVar,
    target: EffectVar,
    covered: EffectSet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EffectConstraintBounds {
    eligibility: EffectConstraintEligibility,
    lower: EffectSet,
    upper: Option<EffectSet>,
    touched: bool,
    inherited: Option<EffectSet>,
}

/// Branch-local higher-order effect constraints. This is deliberately not an
/// exact substitution table: directional function relations need lower and
/// upper bounds plus residual-aware tail edges before a minimal fixed point
/// can be sealed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EffectConstraintEnvironment {
    bounds: BTreeMap<EffectVar, EffectConstraintBounds>,
    edges: BTreeSet<EffectConstraintEdge>,
}

/// Closed failures produced by the path-local effect algebra. The types layer
/// maps only `MissingEffects` to candidate rejection; every other variant is
/// malformed authority or a non-canonical sealed seed.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum EffectConstraintEnvironmentError {
    #[error("unknown effect row reached issuer-backed lower")]
    UnknownRow,
    #[error("effect variable is outside the authorized lower scope")]
    ForeignVariable { variable: EffectVar },
    #[error("effect constraint scope is duplicated or not canonically ordered")]
    NonCanonicalScope,
    #[error("effect rows are not in the subset relation")]
    MissingEffects { missing: EffectSet },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum EffectIssuerRebindError {
    #[error("unknown effect row cannot cross the checked issuer boundary")]
    UnknownRow,
    #[error("effect variable belongs to a foreign prepared issuer")]
    ForeignVariable { variable: EffectVar },
    #[error("effect variable ordinal is outside the prepared overlay")]
    UnauthorizedVariable { variable: EffectVar },
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

/// Deterministic fresh effect-variable allocator scoped to one type check.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectVarSupply {
    next: u32,
}

/// Failure while binding or resolving an effect row.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EffectRowError {
    #[error("effect row is unknown and must be annotated before a dynamic call")]
    UnknownRow,
    #[error("effect variable e{variable} is unbound")]
    UnboundVariable { variable: u32 },
    #[error(
        "effect variable e{variable} was already bound to {existing:?}, cannot rebind it to {requested:?}"
    )]
    ConflictingBinding {
        variable: u32,
        existing: Box<EffectRow>,
        requested: Box<EffectRow>,
    },
    #[error("effect variable e{variable} participates in a cyclic row substitution")]
    CyclicBinding { variable: u32 },
}

/// Failure while checking that one actual effect row is admitted by another.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EffectSubsetError {
    #[error("an unknown effect row cannot participate in subset checking")]
    UnknownRow,
    #[error("the permitted closed row is missing effects {missing:?}")]
    MissingEffects { missing: EffectSet },
    #[error("actual effect tail e{variable} is unresolved against a closed permitted row")]
    UnresolvedActualTail { variable: u32 },
    #[error("effect-row substitution is cyclic at e{variable}")]
    CyclicSubstitution { variable: u32 },
    #[error("effect variable e{variable} has incompatible row bindings")]
    ConflictingBinding {
        variable: u32,
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

impl EffectVar {
    pub const fn from_index(index: u32) -> Self {
        Self {
            issuer: EffectVarIssuer::LOCAL,
            ordinal: index,
        }
    }

    pub(crate) const fn issued(issuer: EffectVarIssuer, ordinal: u32) -> Self {
        Self { issuer, ordinal }
    }

    pub const fn index(self) -> u32 {
        self.ordinal
    }

    pub const fn issuer(self) -> EffectVarIssuer {
        self.issuer
    }

    pub(crate) fn rebind_issuer(self, prepared: EffectVarIssuer, checked: EffectVarIssuer) -> Self {
        if self.issuer == prepared {
            Self {
                issuer: checked,
                ordinal: self.ordinal,
            }
        } else {
            self
        }
    }
}

impl EffectVarIssuer {
    const LOCAL: Self = Self([0; 32]);

    /// Mint one generation-local prepared namespace. It is never encoded into
    /// final facts; the checked-call sealer validates and rebinds it to the
    /// stable callable-owned namespace before canonical encoding.
    pub(crate) fn fresh_prepared() -> Option<Self> {
        static NEXT_PREPARED_EFFECT_ISSUER: AtomicU64 = AtomicU64::new(0);
        let ordinal = NEXT_PREPARED_EFFECT_ISSUER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .ok()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.prepared-callable-effect-issuer.v1\0");
        hasher.update(&ordinal.to_le_bytes());
        Some(Self(*hasher.finalize().as_bytes()))
    }

    /// Derive the stable checked namespace from the canonical callable owner.
    /// This constructor is crate-private and is called only by the checked-call
    /// authority after the callable digest has been minted.
    pub(crate) fn for_checked_callable(owner: &[u8; 32]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.checked-callable-effect-issuer.v1\0");
        hasher.update(owner);
        Self(*hasher.finalize().as_bytes())
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl EffectConstraintVariable {
    pub(crate) const fn new(variable: EffectVar, eligibility: EffectConstraintEligibility) -> Self {
        Self {
            variable,
            eligibility,
        }
    }

    pub(crate) const fn variable(self) -> EffectVar {
        self.variable
    }

    pub(crate) const fn eligibility(self) -> EffectConstraintEligibility {
        self.eligibility
    }
}

impl EffectRow {
    pub fn unknown() -> Self {
        Self::default()
    }

    pub fn closed(concrete: EffectSet) -> Self {
        Self {
            concrete,
            tail: EffectRowTail::Closed,
        }
    }

    pub fn open(concrete: EffectSet, tail: EffectVar) -> Self {
        Self {
            concrete,
            tail: EffectRowTail::Variable(tail),
        }
    }

    pub const fn concrete(&self) -> &EffectSet {
        &self.concrete
    }

    pub const fn tail(&self) -> EffectRowTail {
        self.tail
    }

    pub const fn is_known(&self) -> bool {
        !matches!(self.tail, EffectRowTail::Unknown)
    }

    pub(crate) fn checked_rebind_issuer(
        &self,
        prepared: EffectVarIssuer,
        checked: EffectVarIssuer,
        authorized_ordinals: &BTreeSet<u32>,
    ) -> Result<Self, EffectIssuerRebindError> {
        match self.tail {
            EffectRowTail::Closed => Ok(Self::closed(self.concrete.clone())),
            EffectRowTail::Unknown => Err(EffectIssuerRebindError::UnknownRow),
            EffectRowTail::Variable(variable) if variable.issuer() != prepared => {
                Err(EffectIssuerRebindError::ForeignVariable { variable })
            }
            EffectRowTail::Variable(variable)
                if !authorized_ordinals.contains(&variable.index()) =>
            {
                Err(EffectIssuerRebindError::UnauthorizedVariable { variable })
            }
            EffectRowTail::Variable(variable) => Ok(Self::open(
                self.concrete.clone(),
                variable.rebind_issuer(prepared, checked),
            )),
        }
    }

    pub fn display_label(&self) -> String {
        match self.tail {
            EffectRowTail::Unknown => "unknown".to_owned(),
            EffectRowTail::Closed => format_effect_set(&self.concrete),
            EffectRowTail::Variable(variable) if self.concrete.is_empty() => {
                format!("{{ | e{} }}", variable.index())
            }
            EffectRowTail::Variable(variable) => {
                format!(
                    "{{ {} | e{} }}",
                    effect_labels(&self.concrete),
                    variable.index()
                )
            }
        }
    }

    pub fn resolve(&self, substitutions: &EffectSubstitution) -> Result<EffectSet, EffectRowError> {
        let resolved = self.resolve_partial(substitutions)?;
        match resolved.tail {
            EffectRowTail::Closed => Ok(resolved.concrete),
            EffectRowTail::Variable(tail) => Err(EffectRowError::UnboundVariable {
                variable: tail.index(),
            }),
            EffectRowTail::Unknown => unreachable!("partial resolution rejects unknown rows"),
        }
    }

    /// Checks the complete actual row against the permitted row and extends
    /// the supplied typed substitution without discarding an existing tail.
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
        let residual = actual.concrete.difference(&permitted.concrete);
        match permitted.tail {
            EffectRowTail::Closed => {
                if !residual.is_empty() {
                    return Err(EffectSubsetError::MissingEffects { missing: residual });
                }
                match actual.tail {
                    EffectRowTail::Closed => Ok(()),
                    EffectRowTail::Variable(variable) => {
                        Err(EffectSubsetError::UnresolvedActualTail {
                            variable: variable.index(),
                        })
                    }
                    EffectRowTail::Unknown => {
                        unreachable!("partial resolution rejects unknown rows")
                    }
                }
            }
            EffectRowTail::Variable(permitted_tail) => {
                let requested = match actual.tail {
                    EffectRowTail::Closed => EffectRow::closed(residual),
                    EffectRowTail::Variable(actual_tail) if actual_tail == permitted_tail => {
                        if residual.is_empty() {
                            return Ok(());
                        }
                        EffectRow::closed(residual)
                    }
                    EffectRowTail::Variable(actual_tail) => EffectRow::open(residual, actual_tail),
                    EffectRowTail::Unknown => {
                        unreachable!("partial resolution rejects unknown rows")
                    }
                };
                substitution
                    .bind_row(permitted_tail, &requested)
                    .map_err(EffectSubsetError::from_row_error)
            }
            EffectRowTail::Unknown => unreachable!("partial resolution rejects unknown rows"),
        }
    }

    pub(crate) fn resolve_partial(
        &self,
        substitutions: &EffectSubstitution,
    ) -> Result<EffectRow, EffectRowError> {
        let mut concrete = self.concrete.clone();
        let mut tail = self.tail;
        let mut visited = std::collections::BTreeSet::new();
        loop {
            match tail {
                EffectRowTail::Closed => return Ok(EffectRow::closed(concrete)),
                EffectRowTail::Unknown => return Err(EffectRowError::UnknownRow),
                EffectRowTail::Variable(variable) => {
                    let Some(bound) = substitutions.0.get(&variable) else {
                        return Ok(EffectRow::open(concrete, variable));
                    };
                    if !visited.insert(variable) {
                        return Err(EffectRowError::CyclicBinding {
                            variable: variable.index(),
                        });
                    }
                    concrete.union_with(&bound.concrete);
                    tail = bound.tail;
                }
            }
        }
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
        if variables
            .windows(2)
            .any(|rows| rows[0].variable >= rows[1].variable)
        {
            return Err(EffectConstraintEnvironmentError::NonCanonicalScope);
        }
        let bounds = variables
            .iter()
            .map(|row| {
                (
                    row.variable,
                    EffectConstraintBounds {
                        eligibility: row.eligibility,
                        lower: EffectSet::new(),
                        upper: None,
                        touched: false,
                        inherited: None,
                    },
                )
            })
            .collect();
        Ok(Self {
            bounds,
            edges: BTreeSet::new(),
        })
    }

    pub(crate) fn validate_row(
        &self,
        row: &EffectRow,
    ) -> Result<(), EffectConstraintEnvironmentError> {
        match row.tail {
            EffectRowTail::Closed => Ok(()),
            EffectRowTail::Variable(variable) if self.bounds.contains_key(&variable) => Ok(()),
            EffectRowTail::Variable(variable) => {
                Err(EffectConstraintEnvironmentError::ForeignVariable { variable })
            }
            EffectRowTail::Unknown => Err(EffectConstraintEnvironmentError::UnknownRow),
        }
    }

    /// Restore one row from the opaque completed type-constraint solution.
    /// The solution owner has already proved exact scope membership and a
    /// canonical closed value; this method only enforces the target
    /// environment's unique-coordinate transition.
    pub(crate) fn restore_completed_inherited(
        &mut self,
        variable: EffectVar,
        concrete: &EffectSet,
    ) {
        let bounds = self
            .bounds
            .get_mut(&variable)
            .expect("completed effect scope transition retains every sealed variable");
        assert!(
            bounds.inherited.is_none(),
            "completed effect rows are uniquely sealed before restoration"
        );
        bounds.lower = concrete.clone();
        bounds.upper = Some(concrete.clone());
        bounds.touched = true;
        bounds.inherited = Some(concrete.clone());
    }

    /// Record `actual <= permitted` without prematurely closing either tail.
    /// The operation is transactional: an incompatible addition leaves this
    /// environment unchanged.
    pub(crate) fn constrain_subset(
        &mut self,
        actual: &EffectRow,
        permitted: &EffectRow,
    ) -> Result<(), EffectConstraintEnvironmentError> {
        self.validate_row(actual)?;
        self.validate_row(permitted)?;
        let mut next = self.clone();
        let residual = actual.concrete.difference(&permitted.concrete);

        match permitted.tail {
            EffectRowTail::Closed => {
                if !residual.is_empty() {
                    return Err(EffectConstraintEnvironmentError::MissingEffects {
                        missing: residual,
                    });
                }
                if let EffectRowTail::Variable(source) = actual.tail {
                    next.mark_touched(source)?;
                    next.intersect_upper(source, permitted.concrete.clone())?;
                }
            }
            EffectRowTail::Variable(target) => {
                next.mark_touched(target)?;
                next.extend_lower(target, &residual)?;
                if let EffectRowTail::Variable(source) = actual.tail {
                    next.mark_touched(source)?;
                    if source != target {
                        next.edges.insert(EffectConstraintEdge {
                            source,
                            target,
                            covered: permitted.concrete.clone(),
                        });
                    }
                }
            }
            EffectRowTail::Unknown => unreachable!("validated rows are known"),
        }
        next.solve_bounds()?;
        *self = next;
        Ok(())
    }

    pub(crate) fn bindings(
        &self,
    ) -> Result<Vec<(EffectVar, EffectRow)>, EffectConstraintEnvironmentError> {
        let bounds = self.solve_bounds()?;
        Ok(bounds
            .into_iter()
            .filter_map(|(variable, bounds)| {
                (matches!(bounds.eligibility, EffectConstraintEligibility::Bindable)
                    || bounds.touched
                    || bounds.inherited.is_some())
                .then(|| (variable, EffectRow::closed(bounds.lower)))
            })
            .collect())
    }

    pub(crate) fn bindings_equal(
        &self,
        other: &Self,
    ) -> Result<bool, EffectConstraintEnvironmentError> {
        Ok(self.bindings()? == other.bindings()?)
    }

    pub(crate) fn substitution(
        &self,
    ) -> Result<EffectSubstitution, EffectConstraintEnvironmentError> {
        Ok(EffectSubstitution(
            self.bindings()?.into_iter().collect::<BTreeMap<_, _>>(),
        ))
    }

    fn mark_touched(
        &mut self,
        variable: EffectVar,
    ) -> Result<(), EffectConstraintEnvironmentError> {
        let Some(bounds) = self.bounds.get_mut(&variable) else {
            return Err(EffectConstraintEnvironmentError::ForeignVariable { variable });
        };
        bounds.touched = true;
        Ok(())
    }

    fn extend_lower(
        &mut self,
        variable: EffectVar,
        effects: &EffectSet,
    ) -> Result<(), EffectConstraintEnvironmentError> {
        let Some(bounds) = self.bounds.get_mut(&variable) else {
            return Err(EffectConstraintEnvironmentError::ForeignVariable { variable });
        };
        bounds.lower.union_with(effects);
        Ok(())
    }

    fn intersect_upper(
        &mut self,
        variable: EffectVar,
        upper: EffectSet,
    ) -> Result<(), EffectConstraintEnvironmentError> {
        let Some(bounds) = self.bounds.get_mut(&variable) else {
            return Err(EffectConstraintEnvironmentError::ForeignVariable { variable });
        };
        bounds.upper = Some(match bounds.upper.take() {
            Some(existing) => existing.intersection(&upper),
            None => upper,
        });
        Ok(())
    }

    fn solve_bounds(
        &self,
    ) -> Result<BTreeMap<EffectVar, EffectConstraintBounds>, EffectConstraintEnvironmentError> {
        let mut bounds = self.bounds.clone();
        loop {
            let mut changed = false;
            for edge in &self.edges {
                let source = bounds.get(&edge.source).ok_or(
                    EffectConstraintEnvironmentError::ForeignVariable {
                        variable: edge.source,
                    },
                )?;
                let source_lower = source.lower.difference(&edge.covered);
                let target_upper = bounds
                    .get(&edge.target)
                    .ok_or(EffectConstraintEnvironmentError::ForeignVariable {
                        variable: edge.target,
                    })?
                    .upper
                    .clone();

                changed |= bounds
                    .get_mut(&edge.target)
                    .expect("edge target validated above")
                    .lower
                    .union_with(&source_lower);

                if let Some(target_upper) = target_upper {
                    let propagated = edge.covered.union(&target_upper);
                    let source = bounds
                        .get_mut(&edge.source)
                        .expect("edge source validated above");
                    let narrowed = match &source.upper {
                        Some(existing) => existing.intersection(&propagated),
                        None => propagated,
                    };
                    if source.upper.as_ref() != Some(&narrowed) {
                        source.upper = Some(narrowed);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        for row in bounds.values() {
            if let Some(upper) = &row.upper
                && !row.lower.is_subset(upper)
            {
                return Err(EffectConstraintEnvironmentError::MissingEffects {
                    missing: row.lower.difference(upper),
                });
            }
        }
        Ok(bounds)
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
        tail: EffectVar,
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

    pub(crate) fn from_rows(rows: impl IntoIterator<Item = (EffectVar, EffectRow)>) -> Self {
        Self(rows.into_iter().collect())
    }

    pub fn bind_exact(
        &mut self,
        variable: EffectVar,
        effects: EffectSet,
    ) -> Result<(), EffectRowError> {
        self.bind_row(variable, &EffectRow::closed(effects))
    }

    pub(crate) fn close_fresh_inferred_tail(&mut self, variable: EffectVar) {
        let previous = self.0.insert(variable, EffectRow::closed(EffectSet::new()));
        debug_assert!(previous.is_none(), "fresh effect-row tail was reused");
    }

    pub fn get(&self, variable: EffectVar) -> Option<&EffectRow> {
        self.0.get(&variable)
    }

    pub(crate) fn bind_row(
        &mut self,
        variable: EffectVar,
        requested: &EffectRow,
    ) -> Result<(), EffectRowError> {
        let requested = requested.resolve_partial(self)?;
        if requested.tail == EffectRowTail::Variable(variable) {
            if requested.concrete.is_empty() {
                return Ok(());
            }
            return Err(EffectRowError::CyclicBinding {
                variable: variable.index(),
            });
        }
        if let Some(existing) = self.0.get(&variable) {
            let existing = existing.resolve_partial(self)?;
            if existing == requested {
                return Ok(());
            }
            return Err(EffectRowError::ConflictingBinding {
                variable: variable.index(),
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

impl EffectVarSupply {
    /// Allocates a fresh effect-row variable.
    ///
    /// # Panics
    ///
    /// Panics if the process exhausts the `u32` effect-variable id space.
    pub fn fresh(&mut self) -> EffectVar {
        let variable = EffectVar::from_index(self.next);
        self.next = self
            .next
            .checked_add(1)
            .expect("effect variable space exhausted");
        variable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::EffectSet;

    #[test]
    fn effect_row_display_label_covers_closed_open_and_unknown_rows() {
        let variable = EffectVar::from_index(3);
        assert_eq!(EffectRow::unknown().display_label(), "unknown");
        assert_eq!(EffectRow::closed(EffectSet::new()).display_label(), "{ }");
        assert_eq!(
            EffectRow::closed(EffectSet::from_labels(["fs.read"]).expect("valid row"))
                .display_label(),
            "{ fs.read }"
        );
        assert_eq!(
            EffectRow::open(EffectSet::new(), variable).display_label(),
            "{ | e3 }"
        );
        assert_eq!(
            EffectRow::open(
                EffectSet::from_labels(["log.write"]).expect("valid row"),
                variable
            )
            .display_label(),
            "{ log.write | e3 }"
        );
    }

    #[test]
    fn resolves_a_polymorphic_effect_tail() {
        let mut supply = EffectVarSupply::default();
        let variable = supply.fresh();
        let row = EffectRow::open(
            EffectSet::from_labels(["log.write"]).expect("valid concrete row"),
            variable,
        );
        let mut substitutions = EffectSubstitution::new();
        substitutions
            .bind_exact(
                variable,
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
        let permitted_tail = EffectVar::from_index(4);
        let actual = EffectRow::closed(
            EffectSet::from_labels(["control.suspend", "fs.read", "log.write"])
                .expect("valid actual row"),
        );
        let permitted = EffectRow::open(
            EffectSet::from_labels(["log.write"]).expect("valid permitted head"),
            permitted_tail,
        );
        let mut substitution = EffectSubstitution::new();

        EffectRow::check_subset(&actual, &permitted, &mut substitution)
            .expect("open tail accepts residual effects");

        assert_eq!(
            substitution.get(permitted_tail),
            Some(&EffectRow::closed(
                EffectSet::from_labels(["control.suspend", "fs.read"]).expect("valid residual row")
            ))
        );
    }

    #[test]
    fn open_permitted_tail_retains_an_unresolved_actual_tail() {
        let actual_tail = EffectVar::from_index(2);
        let permitted_tail = EffectVar::from_index(3);
        let actual = EffectRow::open(
            EffectSet::from_labels(["fs.read", "log.write"]).expect("valid actual head"),
            actual_tail,
        );
        let permitted = EffectRow::open(
            EffectSet::from_labels(["log.write"]).expect("valid permitted head"),
            permitted_tail,
        );
        let mut substitution = EffectSubstitution::new();

        EffectRow::check_subset(&actual, &permitted, &mut substitution)
            .expect("permitted tail retains actual tail");

        assert_eq!(
            substitution.get(permitted_tail),
            Some(&EffectRow::open(
                EffectSet::from_labels(["fs.read"]).expect("valid residual head"),
                actual_tail
            ))
        );
    }

    #[test]
    fn prebound_open_tail_is_constrained_without_overwrite() {
        let residual_tail = EffectVar::from_index(8);
        let permitted_tail = EffectVar::from_index(9);
        let mut substitution = EffectSubstitution::new();
        substitution
            .bind_row(
                permitted_tail,
                &EffectRow::open(
                    EffectSet::from_labels(["fs.read"]).expect("valid existing head"),
                    residual_tail,
                ),
            )
            .expect("fresh permitted tail binds");
        let actual = EffectRow::closed(
            EffectSet::from_labels(["fs.read", "log.write"]).expect("valid actual row"),
        );
        let permitted = EffectRow::open(EffectSet::new(), permitted_tail);

        EffectRow::check_subset(&actual, &permitted, &mut substitution)
            .expect("residual tail receives only the remaining effect");

        assert_eq!(
            substitution.get(permitted_tail),
            Some(&EffectRow::open(
                EffectSet::from_labels(["fs.read"]).expect("valid retained head"),
                residual_tail
            ))
        );
        assert_eq!(
            substitution.get(residual_tail),
            Some(&EffectRow::closed(
                EffectSet::from_labels(["log.write"]).expect("valid residual binding")
            ))
        );
    }

    #[test]
    fn unresolved_actual_tail_fails_against_a_closed_row() {
        let actual_tail = EffectVar::from_index(12);
        let actual = EffectRow::open(EffectSet::new(), actual_tail);
        let permitted = EffectRow::closed(EffectSet::new());

        assert_eq!(
            EffectRow::check_subset(&actual, &permitted, &mut EffectSubstitution::new()),
            Err(EffectSubsetError::UnresolvedActualTail { variable: 12 })
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
        let mut supply = EffectVarSupply::default();
        let variable = supply.fresh();
        let callable = CallableId::new("fn.with_open_row");
        let row = EffectRowSummary {
            callable: callable.clone(),
            inferred: EffectRow::open(
                EffectSet::from_labels(["log.write"]).expect("valid concrete row"),
                variable,
            ),
            upper_bound: Some(EffectRow::closed(
                EffectSet::from_labels(["fs.read", "log.write"]).expect("valid bound row"),
            )),
            forbidden: EffectRow::closed(EffectSet::from_labels(["net.open"]).expect("valid row")),
        };
        let mut substitutions = EffectSubstitution::new();
        substitutions
            .bind_exact(
                variable,
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
        let variable = EffectVar::from_index(7);
        let callable = CallableId::new("fn.needs_row");
        let report = EffectRowReport::new([EffectRowSummary {
            callable: callable.clone(),
            inferred: EffectRow::open(EffectSet::new(), variable),
            upper_bound: None,
            forbidden: EffectRow::closed(EffectSet::new()),
        }]);

        assert_eq!(
            report.resolve_closed(&EffectSubstitution::new()),
            Err(EffectRowCloseError::Unresolved {
                callable,
                source: Box::new(EffectRowError::UnboundVariable { variable: 7 }),
            })
        );
    }

    #[test]
    fn constraint_environment_computes_residual_aware_minimal_fixed_point() {
        let issuer = EffectVarIssuer::fresh_prepared().expect("test issuer");
        let source = EffectVar::issued(issuer, 0);
        let target = EffectVar::issued(issuer, 1);
        let mut environment = EffectConstraintEnvironment::new(&[
            EffectConstraintVariable::new(source, EffectConstraintEligibility::Bindable),
            EffectConstraintVariable::new(target, EffectConstraintEligibility::Bindable),
        ])
        .expect("canonical scope");
        let covered = EffectSet::from_labels(["fs.read"]).expect("effect");
        let residual = EffectSet::from_labels(["net.open"]).expect("effect");

        environment
            .constrain_subset(
                &EffectRow::open(covered.clone(), source),
                &EffectRow::open(covered.clone(), target),
            )
            .expect("tail edge");
        environment
            .constrain_subset(
                &EffectRow::closed(residual.clone()),
                &EffectRow::open(EffectSet::new(), source),
            )
            .expect("source lower bound");

        assert_eq!(
            environment.bindings().expect("minimal solution"),
            vec![
                (source, EffectRow::closed(residual.clone())),
                (target, EffectRow::closed(residual)),
            ]
        );
    }

    #[test]
    fn constraint_environment_rejects_only_well_formed_subset_conflict_transactionally() {
        let issuer = EffectVarIssuer::fresh_prepared().expect("test issuer");
        let variable = EffectVar::issued(issuer, 0);
        let mut environment = EffectConstraintEnvironment::new(&[EffectConstraintVariable::new(
            variable,
            EffectConstraintEligibility::Bindable,
        )])
        .expect("canonical scope");
        let permitted = EffectSet::from_labels(["fs.read"]).expect("effect");
        environment
            .constrain_subset(
                &EffectRow::open(EffectSet::new(), variable),
                &EffectRow::closed(permitted),
            )
            .expect("upper bound");
        let before = environment.clone();

        assert!(matches!(
            environment.constrain_subset(
                &EffectRow::closed(EffectSet::from_labels(["net.open"]).expect("effect")),
                &EffectRow::open(EffectSet::new(), variable),
            ),
            Err(EffectConstraintEnvironmentError::MissingEffects { .. })
        ));
        assert_eq!(environment, before);
    }

    #[test]
    fn constraint_environment_classifies_unknown_and_foreign_rows_as_invariants() {
        let issuer = EffectVarIssuer::fresh_prepared().expect("test issuer");
        let variable = EffectVar::issued(issuer, 0);
        let environment = EffectConstraintEnvironment::new(&[EffectConstraintVariable::new(
            variable,
            EffectConstraintEligibility::Bindable,
        )])
        .expect("canonical scope");
        assert_eq!(
            environment.validate_row(&EffectRow::unknown()),
            Err(EffectConstraintEnvironmentError::UnknownRow)
        );
        let foreign = EffectVar::issued(
            EffectVarIssuer::fresh_prepared().expect("foreign issuer"),
            0,
        );
        assert_eq!(
            environment.validate_row(&EffectRow::open(EffectSet::new(), foreign)),
            Err(EffectConstraintEnvironmentError::ForeignVariable { variable: foreign })
        );
    }
}
