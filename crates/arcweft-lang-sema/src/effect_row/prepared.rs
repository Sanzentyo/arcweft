//! Issuer-owned effect schemes and immutable terms used before callable closure.
//!
//! Schemes are functions of declared row parameters, never caller-bindable row
//! variables. Terms retain applications and simultaneous substitution until the
//! complete component can be solved. No term issues a semantic type identity.

use std::{
    collections::{BTreeMap, BTreeSet},
    hash::{Hash, Hasher},
    sync::Arc,
};

use super::{
    DecisionControl, DecisionWork, EffectFormula, EffectPredicate, EffectRow, EffectRowError,
};
use crate::types::GenericEffectReference;

mod closure;
mod declaration;
pub(crate) use closure::{
    EffectPredicateObservation, EffectPredicateRegion, EffectRowObservation, EffectRowRegion,
    GuardedEffectPredicate, GuardedEffectRow, PrincipalEffectClosure,
};
pub(crate) use declaration::PreparedEffectDeclaration;
mod substitution;
pub(crate) use substitution::PreparedEffectTermEdit;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

#[derive(Debug)]
struct PreparedEffectIssuer;

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
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PreparedEffectRowRef(PreparedEffectReference);
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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PreparedEffectPredicate {
    Canonical(EffectPredicate),
    Term(PreparedEffectPredicateRef),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedEffectRelation {
    Equal,
    /// Exact membership for symbolic rows, existing capability coverage when
    /// both final rows have no remaining row references.
    AdmittedSubset,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub(crate) enum PreparedEffectInvariant {
    #[error("effect term belongs to another preparation issuer")]
    ForeignIssuer,
    #[error("effect reference was removed or belongs to a rolled-back candidate")]
    StaleReference,
    #[error("effect scheme has an invalid declaration/body state")]
    SchemeState,
    #[error("effect scheme parameter occurs more than once")]
    DuplicateParameter,
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
    #[error("a solved effect cell retained an unbound Boolean reference")]
    UnclosedCell,
    #[error("principal effect projection contradicts the original typed equations")]
    PrincipalConfirmation,
    #[error("a failed effect-term edit cannot be resumed or committed")]
    TermEditState,
}

#[derive(Debug, thiserror::Error)]
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
    #[error("the requested effect observation is not unconditionally admitted and invariant")]
    ObservationNotTotal,
    #[error("the guarded effect observation cannot be represented by pointwise effect formulas")]
    NonPointwiseObservation,
}

struct Work<'a, C>(&'a mut C);
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
        scheme: PreparedEffectSchemeRef,
        arguments: Box<[PreparedEffectRow]>,
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

impl RowTerm {
    fn children(&self) -> impl Iterator<Item = &PreparedEffectRow> {
        let (slice, map, pair) = match self {
            Self::Application { arguments, .. } | Self::Union(arguments) => {
                (arguments.as_ref(), None, [None, None])
            }
            Self::Substitution { arguments, .. } => (&[][..], Some(arguments), [None, None]),
            Self::Select { yes, no, .. } => (&[][..], None, [Some(yes), Some(no)]),
        };
        slice
            .iter()
            .chain(map.into_iter().flat_map(BTreeMap::values))
            .chain(pair.into_iter().flatten())
    }
}

#[derive(Debug)]
enum PredicateTerm {
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
    Not(PreparedEffectPredicate),
    Select {
        choice: PreparedEffectChoice,
        yes: PreparedEffectPredicate,
        no: PreparedEffectPredicate,
    },
}

/// A rollback snapshot shares immutable definitions. Ordinals are never restored.
pub(crate) struct PreparedEffectCheckpoint {
    issuer: Arc<PreparedEffectIssuer>,
    schemes: BTreeMap<u64, Arc<Scheme>>,
    row_limit: u64,
    predicate_limit: u64,
    choice_limit: u64,
    ordinal: u64,
}

pub(crate) struct PreparedEffectGraph {
    issuer: Arc<PreparedEffectIssuer>,
    next_scheme: u64,
    next_row: u64,
    next_predicate: u64,
    next_choice: u64,
    next_checkpoint: u64,
    checkpoints: Vec<u64>,
    choices: BTreeSet<u64>,
    schemes: BTreeMap<u64, Arc<Scheme>>,
    rows: BTreeMap<u64, Arc<RowTerm>>,
    predicates: BTreeMap<u64, Arc<PredicateTerm>>,
}

impl Default for PreparedEffectGraph {
    fn default() -> Self {
        Self {
            issuer: Arc::new(PreparedEffectIssuer),
            next_scheme: 0,
            next_row: 0,
            next_predicate: 0,
            next_choice: 0,
            next_checkpoint: 0,
            checkpoints: Vec::new(),
            choices: BTreeSet::new(),
            schemes: BTreeMap::new(),
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
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
        if let PreparedEffectRow::Term(reference) = row {
            self.row(reference)?;
        }
        Ok(())
    }

    pub(crate) fn validate_retained_predicate(
        &self,
        predicate: &PreparedEffectPredicate,
    ) -> Result<(), PreparedEffectInvariant> {
        self.validate_predicate(predicate)
    }

    #[cfg(test)]
    pub(crate) fn retained_terms(&self) -> (usize, usize) {
        (self.rows.len(), self.predicates.len())
    }

    fn reference(&self, ordinal: u64) -> PreparedEffectReference {
        PreparedEffectReference {
            issuer: Arc::clone(&self.issuer),
            ordinal,
        }
    }

    fn validate(
        &self,
        reference: &PreparedEffectReference,
    ) -> Result<u64, PreparedEffectInvariant> {
        if !Arc::ptr_eq(&self.issuer, &reference.issuer) {
            return Err(PreparedEffectInvariant::ForeignIssuer);
        }
        Ok(reference.ordinal)
    }

    fn scheme(
        &self,
        reference: &PreparedEffectSchemeRef,
    ) -> Result<&Scheme, PreparedEffectInvariant> {
        self.schemes
            .get(&self.validate(&reference.0)?)
            .map(Arc::as_ref)
            .ok_or(PreparedEffectInvariant::StaleReference)
    }

    fn row(&self, reference: &PreparedEffectRowRef) -> Result<&RowTerm, PreparedEffectInvariant> {
        self.rows
            .get(&self.validate(&reference.0)?)
            .map(Arc::as_ref)
            .ok_or(PreparedEffectInvariant::StaleReference)
    }

    fn predicate(
        &self,
        reference: &PreparedEffectPredicateRef,
    ) -> Result<&PredicateTerm, PreparedEffectInvariant> {
        self.predicates
            .get(&self.validate(&reference.0)?)
            .map(Arc::as_ref)
            .ok_or(PreparedEffectInvariant::StaleReference)
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

    pub(crate) fn declare<C: DecisionControl>(
        &mut self,
        parameters: Vec<GenericEffectReference>,
        domain: EffectPredicate,
        control: &mut C,
    ) -> Result<PreparedEffectSchemeRef, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let mut unique = BTreeSet::new();
        for parameter in &parameters {
            work.charge(DecisionWork::Visit)?;
            if !unique.insert(parameter) {
                return Err(PreparedEffectInvariant::DuplicateParameter.into());
            }
        }
        for parameter in domain.variables() {
            work.charge(DecisionWork::Visit)?;
            if !unique.contains(parameter) {
                return Err(PreparedEffectInvariant::UndeclaredParameter(parameter.clone()).into());
            }
        }
        work.charge(DecisionWork::Node)?;
        let id = self.next_scheme;
        self.next_scheme = id
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::OrdinalExhausted)?;
        self.schemes.insert(
            id,
            Arc::new(Scheme {
                header: Arc::new(SchemeHeader {
                    parameters: parameters.into_boxed_slice(),
                    domain,
                }),
                state: SchemeState::Declared,
            }),
        );
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
        self.next_row = id
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::OrdinalExhausted)?;
        self.rows.insert(id, Arc::new(term));
        Ok(PreparedEffectRow::Term(PreparedEffectRowRef(
            self.reference(id),
        )))
    }

    pub(crate) fn application<C: DecisionControl>(
        &mut self,
        scheme: &PreparedEffectSchemeRef,
        arguments: Vec<PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        if self.scheme(scheme)?.header.parameters.len() != arguments.len() {
            return Err(PreparedEffectInvariant::ApplicationArity.into());
        }
        for row in &arguments {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            self.validate_row(row)?;
        }
        self.insert_row(
            RowTerm::Application {
                scheme: scheme.clone(),
                arguments: arguments.into_boxed_slice(),
            },
            control,
        )
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
        arguments: BTreeMap<GenericEffectReference, PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        for row in arguments.values() {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            self.validate_row(row)?;
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
        self.schemes.insert(
            scheme.0.ordinal,
            Arc::new(Scheme {
                header,
                state: SchemeState::BodyPrepared {
                    body,
                    obligations: obligations.into_boxed_slice(),
                },
            }),
        );
        Ok(())
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
        self.next_predicate = id
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::OrdinalExhausted)?;
        self.predicates.insert(id, Arc::new(term));
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
        for predicate in &predicates {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            self.validate_predicate(predicate)?;
        }
        self.insert_predicate(PredicateTerm::And(predicates.into_boxed_slice()), control)
    }

    pub(crate) fn or<C: DecisionControl>(
        &mut self,
        predicates: Vec<PreparedEffectPredicate>,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        for predicate in &predicates {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            self.validate_predicate(predicate)?;
        }
        self.insert_predicate(PredicateTerm::Or(predicates.into_boxed_slice()), control)
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
        self.next_choice = id
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::OrdinalExhausted)?;
        self.choices.insert(id);
        Ok(PreparedEffectChoice(self.reference(id)))
    }

    fn validate_choice(
        &self,
        choice: &PreparedEffectChoice,
    ) -> Result<(), PreparedEffectInvariant> {
        if !self.choices.contains(&self.validate(&choice.0)?) {
            return Err(PreparedEffectInvariant::StaleReference);
        }
        Ok(())
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

    pub(crate) fn checkpoint<C: DecisionControl>(
        &mut self,
        control: &mut C,
    ) -> Result<PreparedEffectCheckpoint, PreparedEffectFailure<C::Error>> {
        for _ in &self.schemes {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
        }
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        let ordinal = self.next_checkpoint;
        self.next_checkpoint = ordinal
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::OrdinalExhausted)?;
        self.checkpoints.push(ordinal);
        Ok(PreparedEffectCheckpoint {
            issuer: Arc::clone(&self.issuer),
            schemes: self.schemes.clone(),
            row_limit: self.next_row,
            predicate_limit: self.next_predicate,
            choice_limit: self.next_choice,
            ordinal,
        })
    }

    fn validate_checkpoint(
        &self,
        checkpoint: &PreparedEffectCheckpoint,
    ) -> Result<(), PreparedEffectInvariant> {
        if !Arc::ptr_eq(&self.issuer, &checkpoint.issuer) {
            return Err(PreparedEffectInvariant::ForeignIssuer);
        }
        if self.checkpoints.last() != Some(&checkpoint.ordinal) {
            return Err(PreparedEffectInvariant::CheckpointOrder);
        }
        Ok(())
    }

    pub(crate) fn commit(
        &mut self,
        checkpoint: &PreparedEffectCheckpoint,
    ) -> Result<(), PreparedEffectInvariant> {
        self.validate_checkpoint(checkpoint)?;
        self.checkpoints.pop();
        Ok(())
    }

    pub(crate) fn rollback(
        &mut self,
        checkpoint: &mut PreparedEffectCheckpoint,
    ) -> Result<(), PreparedEffectInvariant> {
        self.validate_checkpoint(checkpoint)?;
        self.checkpoints.pop();
        self.schemes = std::mem::take(&mut checkpoint.schemes);
        self.rows.retain(|id, _| *id < checkpoint.row_limit);
        self.predicates
            .retain(|id, _| *id < checkpoint.predicate_limit);
        self.choices.retain(|id| *id < checkpoint.choice_limit);
        Ok(())
    }
}
