//! Issuer-owned effect schemes and immutable terms used before callable closure.
//!
//! Schemes are functions of declared row parameters. Inference unknowns are
//! separate functions of their introduction dependencies, with no declaration
//! body. Both share joint closure under whole-row scoped obligations. Terms
//! retain applications and simultaneous substitution until closure; no term
//! issues a semantic type identity.

use std::{
    collections::{BTreeMap, BTreeSet},
    hash::{Hash, Hasher},
    sync::Arc,
};

use arcweft_core::effect_row::EffectDecision;

use super::{
    DecisionControl, DecisionWork, EffectFormula, EffectPredicate, EffectRow, EffectRowError,
};
use crate::types::GenericEffectReference;

mod closure;
mod declaration;
mod read;
mod relation;
pub(crate) use read::{PreparedEffectReadView, SourceEffectParameter};
pub(crate) use scoped::PreparedEffectScopedConstraint;
mod constraints;
use relation::PreparedEffectDefinitions;
mod scope;
pub(crate) use scope::{PreparedEffectReadScope, PreparedEffectScopeEdit};
mod functions;
use functions::{Function, FunctionRef};
mod completion;
pub(crate) use completion::residual::{
    EffectResidualPredicate, EffectResidualPredicateView, ReducedEffectCompletion,
    ReducedEffectFunction, ReducedEffectPredicate, ReducedEffectPredicateView, ResidualFunction,
    ResidualFunctionBody, ResidualFunctionRole, ResidualFunctionVariable,
    ResidualPreparedPredicate, ResidualPreparedPredicateView, ResidualPreparedRow,
    ResidualPreparedRowView, ResidualRowExpression, ResidualRowScope,
};
pub(crate) use completion::{
    BoundScopedEffectCompletion, GuardedEffectFunction, PreparedEffectImportedRef,
    PreparedEffectImportedRelation, PreparedEffectScopedBinding, ScopedEffectCompletion,
};
pub(crate) use functions::{
    PreparedEffectParameterScope, PreparedEffectUnknownEligibility, PreparedEffectUnknownRef,
};
mod scoped;
pub(crate) use closure::{
    ConstantEffectProduct, EffectPredicateObservation, EffectPredicateRegion, EffectRowObservation,
    EffectRowRegion, GuardedEffectPredicate, GuardedEffectRow, PreparedEffectActualInputs,
    PreparedEffectCompletion, PreparedEffectRowProjection, PreparedEffectTerms,
    PrincipalEffectClosure,
};
pub(crate) use declaration::PreparedEffectDeclaration;
use scoped::ScopedObligation;
mod substitution;
pub(crate) use substitution::PreparedEffectTermEdit;
mod transaction;
pub(crate) use transaction::{
    PreparedEffectCheckpoint, PreparedEffectDelta, PreparedEffectReplayMismatch,
    PreparedEffectReplayRoot, PreparedEffectRestoreFailure,
};
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

#[derive(Debug)]
struct PreparedEffectIssuer;

/// Read-only identity of the actual graph owner. Sharing this lease does not
/// copy declarations, restore a candidate, or authorize publishing a delta.
#[derive(Clone, Debug)]
pub(crate) struct PreparedEffectGraphOwner(Arc<PreparedEffectIssuer>);

impl PreparedEffectGraphOwner {
    pub(crate) fn validate(
        &self,
        graph: &PreparedEffectGraph,
    ) -> Result<(), PreparedEffectInvariant> {
        if Arc::ptr_eq(&self.0, &graph.relation.issuer) {
            Ok(())
        } else {
            Err(PreparedEffectInvariant::ForeignIssuer)
        }
    }
}

/// Operational reference only; equality never decides overload exactness.
#[derive(Clone, Debug)]
pub(crate) struct PreparedEffectReference {
    issuer: Arc<PreparedEffectIssuer>,
    ordinal: u64,
}

impl PartialEq for PreparedEffectReference {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &other.issuer) && self.ordinal == other.ordinal
    }
}
impl Eq for PreparedEffectReference {}
impl Hash for PreparedEffectReference {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.issuer).hash(state);
        self.ordinal.hash(state);
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PreparedEffectSchemeRef(PreparedEffectReference);
#[derive(Clone)]
pub(crate) struct PreparedEffectRowRef(PreparedEffectReference, Option<Arc<RowTerm>>);

impl PreparedEffectRowRef {
    fn new(reference: PreparedEffectReference, term: &Arc<RowTerm>) -> Self {
        let application = matches!(
            term.as_ref(),
            RowTerm::Application {
                function: FunctionRef::Scheme(_),
                ..
            }
        )
        .then(|| Arc::clone(term));
        Self(reference, application)
    }
}

impl std::fmt::Debug for PreparedEffectRowRef {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("PreparedEffectRowRef")
            .field(&self.0)
            .finish()
    }
}

impl PartialEq for PreparedEffectRowRef {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for PreparedEffectRowRef {}

impl Hash for PreparedEffectRowRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PreparedEffectPredicateRef(PreparedEffectReference);
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PreparedEffectChoice(PreparedEffectReference);

/// Unknown authored annotations remain canonical unknowns, distinct from terms.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PreparedEffectRow {
    Canonical(EffectRow),
    Term(PreparedEffectRowRef),
}

impl PreparedEffectRow {
    /// Replaying a continuation can issue a fresh term for the same named
    /// application. Its declaration and ordered operands are immutable. Exact
    /// term identity is still required by graph admission and read scopes.
    pub(crate) fn same_continuation_row(&self, other: &Self) -> bool {
        if self == other {
            return true;
        }
        let (Self::Term(left), Self::Term(right)) = (self, other) else {
            return false;
        };
        matches!((left.1.as_deref(), right.1.as_deref()), (
            Some(RowTerm::Application { function: FunctionRef::Scheme(left), arguments: left_arguments, .. }),
            Some(RowTerm::Application { function: FunctionRef::Scheme(right), arguments: right_arguments, .. }),
        ) if left == right && left_arguments == right_arguments)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PreparedEffectPredicate {
    Canonical(EffectPredicate),
    Term(PreparedEffectPredicateRef),
}

impl PreparedEffectPredicate {
    pub(crate) fn constant_truth(&self) -> Option<bool> {
        let Self::Canonical(predicate) = self else {
            return None;
        };
        let membership = predicate.membership();
        if membership.overrides().next().is_some() {
            return None;
        }
        let default = membership.default_decision();
        if default.is_constant(false) {
            Some(false)
        } else if default.is_constant(true) {
            Some(true)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedEffectRelation {
    Equal,
    /// Exact membership for symbolic rows, existing capability coverage when
    /// both final rows have no remaining row references.
    AdmittedSubset,
}

#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub(crate) enum PreparedEffectInvariant {
    #[error("effect term belongs to another preparation issuer")]
    ForeignIssuer,
    #[error("effect reference was removed or belongs to a rolled-back candidate")]
    StaleReference,
    #[error("effect scheme has an invalid declaration/body state")]
    SchemeState,
    #[error("effect scheme parameter occurs more than once")]
    DuplicateParameter,
    #[error("effect parameter was registered with a different introduction contract")]
    ParameterContract,
    #[error("imported effect output belongs to a different scoped relation")]
    ImportedRelationOrigin,
    #[error("effect scheme application has the wrong argument count")]
    ApplicationArity,
    #[error("effect scheme body refers to an undeclared parameter: {0:?}")]
    UndeclaredParameter(GenericEffectReference),
    #[error("effect preparation ordinal domain is exhausted")]
    OrdinalExhausted,
    #[error("effect scheme truth-table count overflows the supported ordinal domain")]
    TruthTableOverflow,
    #[error("effect checkpoints must be completed in stack order")]
    CheckpointOrder,
    #[error("effect candidate delta no longer matches its original graph state")]
    DeltaBase,
    #[error("a solved effect cell retained an unbound Boolean reference")]
    UnclosedCell,
    #[error("principal effect projection contradicts the original typed equations")]
    PrincipalConfirmation,
    #[error("a failed effect-term edit cannot be resumed or committed")]
    TermEditState,
    #[error("a future effect function still needs its actual row function")]
    UnresolvedFuture,
    #[error("a named declaration input still needs its original source scheme")]
    UnresolvedSchemeInput,
    #[error("a completed effect binding does not designate a future function")]
    FutureBinding,
    #[error("a future effect function is bound more than once")]
    DuplicateFutureBinding,
    #[error("an effect read scope is already active")]
    ReadScopeActive,
    #[error("the original effect read dependencies were not supplied for publication")]
    ReadScopeDependencies,
    #[error("effect read scopes contain conflicting definitions")]
    ReadScopeConflict,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum PreparedEffectFailure<E> {
    #[error("effect preparation aborted")]
    Control(#[source] E),
    #[error(transparent)]
    Invariant(#[from] PreparedEffectInvariant),
    #[error(transparent)]
    Row(#[from] EffectRowError),
    #[error(transparent)]
    Declaration(#[from] arcweft_core::effect_row::EffectDeclarationError),
    #[error("prepared effect equations have no solution")]
    Unsatisfiable,
    #[error("prepared effect equations have no pointwise least solution")]
    NonPrincipal,
    #[error("the requested effect observation needs a declaration body")]
    ObservationNeedsBody(PreparedEffectSchemeRef),
    #[error("the requested effect observation is not unconditionally admitted and invariant")]
    ObservationNotTotal,
    #[error("the guarded effect observation cannot be represented by pointwise effect formulas")]
    NonPointwiseObservation,
}

impl<E> PreparedEffectFailure<E> {
    pub(crate) fn split_control(
        self,
    ) -> Result<E, PreparedEffectFailure<std::convert::Infallible>> {
        Err(match self {
            Self::Control(error) => return Ok(error),
            Self::Invariant(error) => PreparedEffectFailure::Invariant(error),
            Self::Row(error) => PreparedEffectFailure::Row(error),
            Self::Declaration(error) => PreparedEffectFailure::Declaration(error),
            Self::Unsatisfiable => PreparedEffectFailure::Unsatisfiable,
            Self::NonPrincipal => PreparedEffectFailure::NonPrincipal,
            Self::ObservationNeedsBody(scheme) => {
                PreparedEffectFailure::ObservationNeedsBody(scheme)
            }
            Self::ObservationNotTotal => PreparedEffectFailure::ObservationNotTotal,
            Self::NonPointwiseObservation => PreparedEffectFailure::NonPointwiseObservation,
        })
    }

    pub(crate) fn map_control<T>(self, map: impl FnOnce(E) -> T) -> PreparedEffectFailure<T> {
        match self {
            Self::Control(error) => PreparedEffectFailure::Control(map(error)),
            Self::Invariant(error) => PreparedEffectFailure::Invariant(error),
            Self::Row(error) => PreparedEffectFailure::Row(error),
            Self::Declaration(error) => PreparedEffectFailure::Declaration(error),
            Self::Unsatisfiable => PreparedEffectFailure::Unsatisfiable,
            Self::NonPrincipal => PreparedEffectFailure::NonPrincipal,
            Self::ObservationNeedsBody(scheme) => {
                PreparedEffectFailure::ObservationNeedsBody(scheme)
            }
            Self::ObservationNotTotal => PreparedEffectFailure::ObservationNotTotal,
            Self::NonPointwiseObservation => PreparedEffectFailure::NonPointwiseObservation,
        }
    }
}

struct Work<'a, C>(&'a mut C);

impl<C: DecisionControl> Work<'_, C> {
    fn share_decision<V>(
        &mut self,
        decision: EffectDecision<V>,
    ) -> Result<Arc<EffectDecision<V>>, PreparedEffectFailure<C::Error>> {
        self.charge(DecisionWork::Node)?;
        Ok(Arc::new(decision))
    }

    fn copy_decision<V: Clone + Ord>(
        &mut self,
        decision: &EffectDecision<V>,
    ) -> Result<EffectDecision<V>, PreparedEffectFailure<C::Error>> {
        decision.copy_with_control(self)
    }

    /// An existing canonical formula can retain its ordering and local node
    /// numbering. Admit its complete copy before allocating any of it.
    fn copy_formula<V: Clone + Ord>(
        &mut self,
        formula: &EffectFormula<V>,
    ) -> Result<EffectFormula<V>, PreparedEffectFailure<C::Error>> {
        formula.copy_with_control(self)
    }
}

impl<C: DecisionControl> DecisionControl for Work<'_, C> {
    type Error = PreparedEffectFailure<C::Error>;
    fn charge(&mut self, work: DecisionWork) -> Result<(), Self::Error> {
        self.0.charge(work).map_err(PreparedEffectFailure::Control)
    }
}

#[derive(Debug)]
struct SchemeHeader {
    parameters: Box<[GenericEffectReference]>,
    domain: EffectPredicate,
}

#[derive(Debug)]
enum SchemeState {
    Declared,
    BodyPrepared {
        body: PreparedEffectRow,
        obligations: Box<[PreparedEffectPredicate]>,
    },
}

#[derive(Debug)]
struct Scheme {
    header: Arc<SchemeHeader>,
    state: SchemeState,
}

#[derive(Debug)]
enum RowTerm {
    Application {
        function: FunctionRef,
        arguments: Box<[PreparedEffectRow]>,
        authority: ApplicationArgumentAuthority,
    },
    Substitution {
        formula: EffectFormula<GenericEffectReference>,
        arguments: BTreeMap<GenericEffectReference, PreparedEffectRow>,
    },
    Union(Box<[PreparedEffectRow]>),
    Select {
        choice: PreparedEffectChoice,
        yes: PreparedEffectRow,
        no: PreparedEffectRow,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ApplicationArgumentAuthority {
    Explicit,
    /// The function applied to the captures issued with its introduction.
    Introduction,
    /// Still subject to introduction scope checks, but no longer the issued application.
    SubstitutedIntroduction,
}

impl ApplicationArgumentAuthority {
    fn after_substitution(self) -> Self {
        match self {
            Self::Introduction => Self::SubstitutedIntroduction,
            authority => authority,
        }
    }
}

enum RowChildren<'a> {
    Slice(std::slice::Iter<'a, PreparedEffectRow>),
    Arguments(std::collections::btree_map::Values<'a, GenericEffectReference, PreparedEffectRow>),
    Pair(std::array::IntoIter<&'a PreparedEffectRow, 2>),
}

impl RowChildren<'_> {
    fn is_empty(&self) -> bool {
        match self {
            Self::Slice(rows) => rows.as_slice().is_empty(),
            Self::Arguments(rows) => rows.len() == 0,
            Self::Pair(rows) => rows.as_slice().is_empty(),
        }
    }
}

impl<'a> Iterator for RowChildren<'a> {
    type Item = &'a PreparedEffectRow;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Slice(rows) => rows.next(),
            Self::Arguments(rows) => rows.next(),
            Self::Pair(rows) => rows.next(),
        }
    }
}

impl RowTerm {
    fn children(&self) -> RowChildren<'_> {
        match self {
            Self::Application { arguments, .. } | Self::Union(arguments) => {
                RowChildren::Slice(arguments.iter())
            }
            Self::Substitution { arguments, .. } => RowChildren::Arguments(arguments.values()),
            Self::Select { yes, no, .. } => RowChildren::Pair([yes, no].into_iter()),
        }
    }
}

#[derive(Debug)]
enum PredicateTerm {
    ImportedPrincipal(Arc<completion::ImportedEffectRelation>),
    Substitution {
        predicate: EffectPredicate,
        arguments: BTreeMap<GenericEffectReference, PreparedEffectRow>,
    },
    Relation {
        relation: PreparedEffectRelation,
        actual: PreparedEffectRow,
        permitted: PreparedEffectRow,
    },
    And(Box<[PreparedEffectPredicate]>),
    Or(Box<[PreparedEffectPredicate]>),
    /// Equality of the two whole predicates at the same input valuation.
    Equivalent([PreparedEffectPredicate; 2]),
    Not(PreparedEffectPredicate),
    Select {
        choice: PreparedEffectChoice,
        yes: PreparedEffectPredicate,
        no: PreparedEffectPredicate,
    },
}

#[derive(Debug)]
pub(crate) struct PreparedEffectGraph {
    relation: PreparedEffectDefinitions,
    next_function: u64,
    next_row: u64,
    next_predicate: u64,
    next_choice: u64,
    next_checkpoint: u64,
    next_delta: u64,
    active_scope: Option<scope::ActiveEffectScope>,
    checkpoints: Vec<u64>,
    state: transaction::PreparedEffectStateId,
    state_ancestors: Vec<transaction::PreparedEffectStateId>,
    next_state: u64,
    journal: Vec<transaction::EffectMutation>,
}

impl Default for PreparedEffectGraph {
    fn default() -> Self {
        Self {
            relation: PreparedEffectDefinitions::empty(),
            next_function: 0,
            next_row: 0,
            next_predicate: 0,
            next_choice: 0,
            next_checkpoint: 0,
            next_delta: 0,
            active_scope: None,
            checkpoints: Vec::new(),
            state: transaction::PreparedEffectStateId::INITIAL,
            state_ancestors: Vec::new(),
            next_state: 1,
            journal: Vec::new(),
        }
    }
}

impl PreparedEffectGraph {
    /// Retention permits omitted authored annotations. A live term must still
    /// belong to this graph; retention is not algebraic completion.
    pub(crate) fn validate_retained_row(
        &self,
        row: &PreparedEffectRow,
    ) -> Result<(), PreparedEffectInvariant> {
        self.read().validate_retained_row(row)
    }

    pub(crate) fn validate_retained_predicate(
        &self,
        predicate: &PreparedEffectPredicate,
    ) -> Result<(), PreparedEffectInvariant> {
        self.read().validate_retained_predicate(predicate)
    }

    #[cfg(test)]
    pub(crate) fn retained_terms(&self) -> (usize, usize) {
        (self.relation.rows.len(), self.relation.predicates.len())
    }

    fn reference(&self, ordinal: u64) -> PreparedEffectReference {
        self.relation.reference(ordinal)
    }

    fn validate(
        &self,
        reference: &PreparedEffectReference,
    ) -> Result<u64, PreparedEffectInvariant> {
        self.relation.validate(reference)
    }

    fn scheme(
        &self,
        reference: &PreparedEffectSchemeRef,
    ) -> Result<&Scheme, PreparedEffectInvariant> {
        self.read().scheme(reference)
    }

    fn row(&self, reference: &PreparedEffectRowRef) -> Result<&RowTerm, PreparedEffectInvariant> {
        self.read().row(reference)
    }

    fn predicate(
        &self,
        reference: &PreparedEffectPredicateRef,
    ) -> Result<&PredicateTerm, PreparedEffectInvariant> {
        self.read().predicate(reference)
    }

    fn validate_row<E>(&self, row: &PreparedEffectRow) -> Result<(), PreparedEffectFailure<E>> {
        match row {
            PreparedEffectRow::Canonical(row) if !row.is_known() => {
                Err(EffectRowError::UnknownRow.into())
            }
            PreparedEffectRow::Canonical(_) => Ok(()),
            PreparedEffectRow::Term(reference) => {
                self.row(reference).map(|_| ()).map_err(Into::into)
            }
        }
    }

    /// Admit a borrowed canonical domain before cloning its decision graph.
    pub(crate) fn declare_borrowed_domain<C: DecisionControl>(
        &mut self,
        parameters: Vec<GenericEffectReference>,
        domain: &EffectPredicate,
        control: &mut C,
    ) -> Result<PreparedEffectSchemeRef, PreparedEffectFailure<C::Error>> {
        domain
            .encode(&mut super::EffectGraphAdmission(control))
            .map_err(PreparedEffectFailure::Control)?;
        self.declare(parameters, domain.clone(), control)
    }

    pub(crate) fn declare<C: DecisionControl>(
        &mut self,
        parameters: Vec<GenericEffectReference>,
        domain: EffectPredicate,
        control: &mut C,
    ) -> Result<PreparedEffectSchemeRef, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let unique = Self::validate_parameters(&parameters, &mut work)?;
        for parameter in domain.variables() {
            work.charge(DecisionWork::Visit)?;
            if !unique.contains(parameter) {
                return Err(PreparedEffectInvariant::UndeclaredParameter(parameter.clone()).into());
            }
        }
        let id = self.insert_function(
            Function::Scheme(Scheme {
                header: Arc::new(SchemeHeader {
                    parameters: parameters.into_boxed_slice(),
                    domain,
                }),
                state: SchemeState::Declared,
            }),
            &mut work,
        )?;
        Ok(PreparedEffectSchemeRef(self.reference(id)))
    }

    fn insert_row<C: DecisionControl>(
        &mut self,
        term: RowTerm,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        let id = self.next_row;
        let next = id
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::OrdinalExhausted)?;
        self.changed(control)?;
        self.next_row = next;
        self.record_mutation(transaction::EffectMutation::Row(id));
        let term = Arc::new(term);
        let reference = PreparedEffectRowRef::new(self.reference(id), &term);
        self.relation.rows.insert(id, term);
        Ok(PreparedEffectRow::Term(reference))
    }

    pub(crate) fn application<C: DecisionControl>(
        &mut self,
        scheme: &PreparedEffectSchemeRef,
        arguments: Vec<PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        self.apply_function(FunctionRef::Scheme(scheme.clone()), arguments, control)
    }

    pub(crate) fn union<C: DecisionControl>(
        &mut self,
        rows: Vec<PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        for row in &rows {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            self.validate_row(row)?;
        }
        self.insert_row(RowTerm::Union(rows.into_boxed_slice()), control)
    }

    fn apply_formula<C: DecisionControl>(
        &mut self,
        formula: EffectFormula<GenericEffectReference>,
        mut arguments: BTreeMap<GenericEffectReference, PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        for row in arguments.values() {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            self.validate_row(row)?;
        }
        // A bare row variable denotes its actual at every effect label.
        if arguments.len() == 1
            && formula.membership().overrides().len() == 0
            && let Some(parameter) = formula.single_reference()
            && let Some(actual) = arguments.remove(parameter)
        {
            return Ok(actual);
        }
        self.insert_row(RowTerm::Substitution { formula, arguments }, control)
    }

    fn apply_predicate<C: DecisionControl>(
        &mut self,
        predicate: EffectPredicate,
        arguments: BTreeMap<GenericEffectReference, PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        for row in arguments.values() {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            self.validate_row(row)?;
        }
        self.insert_predicate(
            PredicateTerm::Substitution {
                predicate,
                arguments,
            },
            control,
        )
    }

    pub(crate) fn prepare_body<C: DecisionControl>(
        &mut self,
        scheme: &PreparedEffectSchemeRef,
        body: PreparedEffectRow,
        obligations: Vec<PreparedEffectPredicate>,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.validate_row(&body)?;
        for predicate in &obligations {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            self.validate_predicate(predicate)?;
        }
        let previous = self.scheme(scheme)?;
        if !matches!(previous.state, SchemeState::Declared) {
            return Err(PreparedEffectInvariant::SchemeState.into());
        }
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        let header = Arc::clone(&previous.header);
        self.changed(control)?;
        self.publish_function(
            scheme.0.ordinal,
            Arc::new(Function::Scheme(Scheme {
                header,
                state: SchemeState::BodyPrepared {
                    body,
                    obligations: obligations.into_boxed_slice(),
                },
            })),
        );
        Ok(())
    }

    /// Rebuild selected equations in a temporary section of the original
    /// relation. The previously prepared bodies survive both outcomes.
    pub(crate) fn revalidate_bodies<'s, C, T, E>(
        &mut self,
        schemes: impl IntoIterator<Item = &'s PreparedEffectSchemeRef>,
        control: &mut C,
        build: impl FnOnce(&mut Self, &mut C) -> Result<T, E>,
    ) -> Result<T, E>
    where
        C: DecisionControl,
        E: From<PreparedEffectFailure<C::Error>>,
    {
        let mut checkpoint = self.checkpoint(control)?;
        let result = (|| {
            for reference in schemes {
                control
                    .charge(DecisionWork::Visit)
                    .map_err(PreparedEffectFailure::Control)?;
                let scheme = self
                    .scheme(reference)
                    .map_err(PreparedEffectFailure::Invariant)?;
                if matches!(scheme.state, SchemeState::BodyPrepared { .. }) {
                    control
                        .charge(DecisionWork::Node)
                        .map_err(PreparedEffectFailure::Control)?;
                    let header = Arc::clone(&scheme.header);
                    self.changed(control)?;
                    self.publish_function(
                        reference.0.ordinal,
                        Arc::new(Function::Scheme(Scheme {
                            header,
                            state: SchemeState::Declared,
                        })),
                    );
                }
            }
            build(self, control)
        })();
        self.rollback(&mut checkpoint)
            .map_err(PreparedEffectFailure::Invariant)?;
        result
    }

    fn validate_predicate(
        &self,
        predicate: &PreparedEffectPredicate,
    ) -> Result<(), PreparedEffectInvariant> {
        if let PreparedEffectPredicate::Term(reference) = predicate {
            self.predicate(reference)?;
        }
        Ok(())
    }

    fn insert_predicate<C: DecisionControl>(
        &mut self,
        term: PredicateTerm,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        let id = self.next_predicate;
        let next = id
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::OrdinalExhausted)?;
        self.changed(control)?;
        self.next_predicate = next;
        self.record_mutation(transaction::EffectMutation::Predicate(id));
        self.relation.predicates.insert(id, Arc::new(term));
        Ok(PreparedEffectPredicate::Term(PreparedEffectPredicateRef(
            self.reference(id),
        )))
    }

    pub(crate) fn relate<C: DecisionControl>(
        &mut self,
        relation: PreparedEffectRelation,
        actual: PreparedEffectRow,
        permitted: PreparedEffectRow,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        self.validate_row(&actual)?;
        self.validate_row(&permitted)?;
        self.insert_predicate(
            PredicateTerm::Relation {
                relation,
                actual,
                permitted,
            },
            control,
        )
    }

    pub(crate) fn and<C: DecisionControl>(
        &mut self,
        predicates: Vec<PreparedEffectPredicate>,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        self.connect_predicates(
            predicates,
            EffectPredicate::unconstrained(),
            PredicateTerm::And,
            control,
        )
    }

    pub(crate) fn or<C: DecisionControl>(
        &mut self,
        predicates: Vec<PreparedEffectPredicate>,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        self.connect_predicates(
            predicates,
            EffectPredicate::impossible(),
            PredicateTerm::Or,
            control,
        )
    }

    fn connect_predicates<C: DecisionControl>(
        &mut self,
        mut predicates: Vec<PreparedEffectPredicate>,
        identity: EffectPredicate,
        connective: impl FnOnce(Box<[PreparedEffectPredicate]>) -> PredicateTerm,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        // Validate every operand in the original graph before simplifying.
        // A connective with one remaining operand can retain that exact term.
        for predicate in &predicates {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            self.validate_predicate(predicate)?;
        }
        predicates.retain(|predicate| {
            !matches!(predicate, PreparedEffectPredicate::Canonical(value) if *value == identity)
        });
        match predicates.len() {
            0 => Ok(PreparedEffectPredicate::Canonical(identity)),
            1 => Ok(predicates.pop().expect("single predicate")),
            _ => self.insert_predicate(connective(predicates.into_boxed_slice()), control),
        }
    }

    fn equivalent<C: DecisionControl>(
        &mut self,
        left: PreparedEffectPredicate,
        right: PreparedEffectPredicate,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        self.validate_predicate(&left)?;
        self.validate_predicate(&right)?;
        match (left.constant_truth(), right.constant_truth()) {
            (Some(left), Some(right)) => Ok(PreparedEffectPredicate::Canonical(if left == right {
                EffectPredicate::unconstrained()
            } else {
                EffectPredicate::impossible()
            })),
            (Some(true), None) => Ok(right),
            (None, Some(true)) => Ok(left),
            (Some(false), None) => self.not(right, control),
            (None, Some(false)) => self.not(left, control),
            (None, None) => {
                self.insert_predicate(PredicateTerm::Equivalent([left, right]), control)
            }
        }
    }

    pub(crate) fn not<C: DecisionControl>(
        &mut self,
        predicate: PreparedEffectPredicate,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        self.validate_predicate(&predicate)?;
        self.insert_predicate(PredicateTerm::Not(predicate), control)
    }

    /// One discrete alternative is shared by every label and parameter valuation.
    /// Closure minimizes scheme cells only; it never prefers false alternatives.
    pub(crate) fn choice<C: DecisionControl>(
        &mut self,
        control: &mut C,
    ) -> Result<PreparedEffectChoice, PreparedEffectFailure<C::Error>> {
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        let id = self.next_choice;
        let next = id
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::OrdinalExhausted)?;
        self.changed(control)?;
        self.next_choice = next;
        self.record_mutation(transaction::EffectMutation::Choice(id));
        self.relation.choices.insert(id);
        Ok(PreparedEffectChoice(self.reference(id)))
    }

    fn validate_choice(
        &self,
        choice: &PreparedEffectChoice,
    ) -> Result<(), PreparedEffectInvariant> {
        self.read().validate_choice(choice)
    }

    pub(crate) fn select_row<C: DecisionControl>(
        &mut self,
        choice: &PreparedEffectChoice,
        yes: PreparedEffectRow,
        no: PreparedEffectRow,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        self.validate_choice(choice)?;
        self.validate_row(&yes)?;
        self.validate_row(&no)?;
        self.insert_row(
            RowTerm::Select {
                choice: choice.clone(),
                yes,
                no,
            },
            control,
        )
    }

    pub(crate) fn select_predicate<C: DecisionControl>(
        &mut self,
        choice: &PreparedEffectChoice,
        yes: PreparedEffectPredicate,
        no: PreparedEffectPredicate,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        self.validate_choice(choice)?;
        self.validate_predicate(&yes)?;
        self.validate_predicate(&no)?;
        self.insert_predicate(
            PredicateTerm::Select {
                choice: choice.clone(),
                yes,
                no,
            },
            control,
        )
    }
}
