//! Compare candidate relations through actual paired ports, without completing
//! their rows or equating generation-local allocation coordinates.

use super::{PreparedEffectDelta, PreparedEffectDeltaData};
use crate::effect_row::prepared::{
    Arc, BTreeMap, BTreeSet, DecisionControl, DecisionWork, Function, FunctionRef, PredicateTerm,
    PreparedEffectChoice, PreparedEffectFailure, PreparedEffectGraph, PreparedEffectImportedRef,
    PreparedEffectInvariant, PreparedEffectPredicate, PreparedEffectReference, PreparedEffectRow,
    PreparedEffectSchemeRef, PreparedEffectUnknownRef, RowTerm, SchemeState,
};

/// The consumer pairs ports by their real expression/declaration ownership.
/// Unobserved term allocations are not semantic evidence. Every new function,
/// including one not reached by a row, must have an actual paired owner port.
pub(crate) enum PreparedEffectReplayRoot<'a> {
    Scheme(&'a PreparedEffectSchemeRef, &'a PreparedEffectSchemeRef),
    Unknown(&'a PreparedEffectUnknownRef, &'a PreparedEffectUnknownRef),
    Imported(&'a PreparedEffectImportedRef, &'a PreparedEffectImportedRef),
    Row(&'a PreparedEffectRow, &'a PreparedEffectRow),
    Predicate(&'a PreparedEffectPredicate, &'a PreparedEffectPredicate),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedEffectReplayMismatch {
    Issuer,
    Base,
    FunctionInventory,
    FunctionIdentity,
    FunctionDefinition,
    ImportOrigin,
    Row,
    Predicate,
    ChoiceIdentity,
    Obligations,
}

enum Pair<'a> {
    Function(&'a FunctionRef, &'a FunctionRef),
    Definition(u64, u64),
    Row(&'a PreparedEffectRow, &'a PreparedEffectRow),
    Predicate(&'a PreparedEffectPredicate, &'a PreparedEffectPredicate),
}

/// Only identities that affect equations are bijective: functions and global
/// choices. Sharing an immutable row/predicate DAG node is an implementation
/// detail, so those nodes use visited pairs rather than an identity bijection.
#[derive(Default)]
struct Correspondence {
    forward: BTreeMap<u64, u64>,
    backward: BTreeMap<u64, u64>,
}

impl Correspondence {
    fn bind<C: DecisionControl>(
        &mut self,
        left: u64,
        right: u64,
        control: &mut C,
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?;
        if let Some(mapped) = self.forward.get(&left) {
            return Ok(*mapped == right);
        }
        if self.backward.contains_key(&right) {
            return Ok(false);
        }
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        self.forward.insert(left, right);
        self.backward.insert(right, left);
        Ok(true)
    }
}

struct Replay<'a, 'c, C> {
    base: &'a PreparedEffectGraph,
    left: &'a PreparedEffectDeltaData,
    right: &'a PreparedEffectDeltaData,
    control: &'c mut C,
    functions: Correspondence,
    choices: Correspondence,
    definitions: BTreeSet<(u64, u64)>,
    rows: BTreeSet<(u64, u64)>,
    predicates: BTreeSet<(u64, u64)>,
    pending: Vec<Pair<'a>>,
}

impl PreparedEffectGraph {
    /// Exact replay of the retained relation, not equality of one observation
    /// or one chosen solution. Neither affine payload nor the live base changes
    /// on success, mismatch, cancellation or budget exhaustion.
    pub(crate) fn replay_delta_mismatch<'a, C: DecisionControl>(
        &'a self,
        left_delta: &'a PreparedEffectDelta,
        right_delta: &'a PreparedEffectDelta,
        roots: impl IntoIterator<Item = PreparedEffectReplayRoot<'a>>,
        control: &mut C,
    ) -> Result<Option<PreparedEffectReplayMismatch>, PreparedEffectFailure<C::Error>> {
        control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?;
        let (left, right) = (&*left_delta.data, &*right_delta.data);
        if !Arc::ptr_eq(&left.issuer, &right.issuer) || !Arc::ptr_eq(&self.issuer, &left.issuer) {
            return Ok(Some(PreparedEffectReplayMismatch::Issuer));
        }
        if left.base != right.base || self.state != left.base {
            return Ok(Some(PreparedEffectReplayMismatch::Base));
        }
        self.validate_delta(left_delta, control)?;
        self.validate_delta(right_delta, control)?;
        let mut replay = Replay {
            base: self,
            left,
            right,
            control,
            functions: Correspondence::default(),
            choices: Correspondence::default(),
            definitions: BTreeSet::new(),
            rows: BTreeSet::new(),
            predicates: BTreeSet::new(),
            pending: Vec::new(),
        };
        // A body installed on a declaration from the shared base is anchored
        // by that original declaration, even if no expression observes it.
        for id in left.functions.keys() {
            replay.visit()?;
            if self.functions.contains_key(id) {
                if !right.functions.contains_key(id) {
                    return Ok(Some(PreparedEffectReplayMismatch::FunctionInventory));
                }
                replay.push(Pair::Definition(*id, *id))?;
            }
        }
        for id in right.functions.keys() {
            replay.visit()?;
            if self.functions.contains_key(id) && !left.functions.contains_key(id) {
                return Ok(Some(PreparedEffectReplayMismatch::FunctionInventory));
            }
        }
        if left.obligations.len() != right.obligations.len() {
            return Ok(Some(PreparedEffectReplayMismatch::Obligations));
        }
        for (left, right) in left.obligations.iter().zip(&right.obligations) {
            replay.visit()?;
            if !replay.parameters(&left.parameters, &right.parameters)? {
                return Ok(Some(PreparedEffectReplayMismatch::Obligations));
            }
            replay.push(Pair::Predicate(&left.domain, &right.domain))?;
            replay.push(Pair::Predicate(&left.predicate, &right.predicate))?;
        }
        for root in roots {
            replay.visit()?;
            let mismatch = match root {
                PreparedEffectReplayRoot::Scheme(left, right) => {
                    replay.root_function(&left.0, &right.0, |value| {
                        matches!(value, Function::Scheme(_))
                    })?
                }
                PreparedEffectReplayRoot::Unknown(left, right) => {
                    replay.root_function(left.reference(), right.reference(), |value| {
                        matches!(value, Function::Unknown { .. })
                    })?
                }
                PreparedEffectReplayRoot::Imported(left, right) => {
                    replay.root_function(&left.0, &right.0, |value| {
                        matches!(value, Function::Imported(_))
                    })?
                }
                PreparedEffectReplayRoot::Row(left, right) => {
                    replay.push(Pair::Row(left, right))?;
                    None
                }
                PreparedEffectReplayRoot::Predicate(left, right) => {
                    replay.push(Pair::Predicate(left, right))?;
                    None
                }
            };
            if mismatch.is_some() {
                return Ok(mismatch);
            }
        }
        while let Some(pair) = replay.pending.pop() {
            replay.visit()?;
            let mismatch = match pair {
                Pair::Function(left, right) => {
                    if std::mem::discriminant(left) == std::mem::discriminant(right) {
                        replay.function_reference(left.reference(), right.reference())?
                    } else {
                        Some(PreparedEffectReplayMismatch::FunctionDefinition)
                    }
                }
                Pair::Definition(left, right) => replay.definition(left, right)?,
                Pair::Row(left, right) => replay.row(left, right)?,
                Pair::Predicate(left, right) => replay.predicate(left, right)?,
            };
            if mismatch.is_some() {
                return Ok(mismatch);
            }
        }
        // An unpaired inference unknown still changes the quantified relation.
        // Never infer its owner from ordinal order or matching row observations.
        for id in left.functions.keys() {
            replay.visit()?;
            if !self.functions.contains_key(id) && !replay.functions.forward.contains_key(id) {
                return Ok(Some(PreparedEffectReplayMismatch::FunctionInventory));
            }
        }
        for id in right.functions.keys() {
            replay.visit()?;
            if !self.functions.contains_key(id) && !replay.functions.backward.contains_key(id) {
                return Ok(Some(PreparedEffectReplayMismatch::FunctionInventory));
            }
        }
        Ok(None)
    }
}

impl<'a, C: DecisionControl> Replay<'a, '_, C> {
    fn visit(&mut self) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)
    }

    fn push(&mut self, pair: Pair<'a>) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        self.pending.push(pair);
        Ok(())
    }

    fn validate_reference(
        &self,
        reference: &PreparedEffectReference,
    ) -> Result<u64, PreparedEffectInvariant> {
        if !Arc::ptr_eq(&reference.issuer, &self.left.issuer) {
            return Err(PreparedEffectInvariant::ForeignIssuer);
        }
        Ok(reference.ordinal)
    }

    fn parameters(
        &mut self,
        left: &[crate::types::GenericEffectReference],
        right: &[crate::types::GenericEffectReference],
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        if left.len() != right.len() {
            return Ok(false);
        }
        for (left, right) in left.iter().zip(right) {
            self.visit()?;
            if left != right {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn formula(
        &mut self,
        left: &crate::effect_row::EffectFormula<crate::types::GenericEffectReference>,
        right: &crate::effect_row::EffectFormula<crate::types::GenericEffectReference>,
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        left.encode(&mut crate::effect_row::EffectEqualityControl(self.control))
            .map_err(PreparedEffectFailure::Control)?;
        right
            .encode(&mut crate::effect_row::EffectEqualityControl(self.control))
            .map_err(PreparedEffectFailure::Control)?;
        Ok(left == right)
    }

    fn function_reference(
        &mut self,
        left: &PreparedEffectReference,
        right: &PreparedEffectReference,
    ) -> Result<Option<PreparedEffectReplayMismatch>, PreparedEffectFailure<C::Error>> {
        let (left, right) = (
            self.validate_reference(left)?,
            self.validate_reference(right)?,
        );
        let (left_base, right_base) = (
            self.base.functions.contains_key(&left),
            self.base.functions.contains_key(&right),
        );
        if left_base || right_base {
            return Ok((!left_base || !right_base || left != right)
                .then_some(PreparedEffectReplayMismatch::FunctionIdentity));
        }
        if !self.left.functions.contains_key(&left) || !self.right.functions.contains_key(&right) {
            return Err(PreparedEffectInvariant::StaleReference.into());
        }
        if !self.functions.bind(left, right, self.control)? {
            return Ok(Some(PreparedEffectReplayMismatch::FunctionIdentity));
        }
        self.push(Pair::Definition(left, right))?;
        Ok(None)
    }

    fn root_function(
        &mut self,
        left: &PreparedEffectReference,
        right: &PreparedEffectReference,
        has_role: fn(&Function) -> bool,
    ) -> Result<Option<PreparedEffectReplayMismatch>, PreparedEffectFailure<C::Error>> {
        let (left_id, right_id) = (
            self.validate_reference(left)?,
            self.validate_reference(right)?,
        );
        let left_definition = self
            .left
            .functions
            .get(&left_id)
            .or_else(|| self.base.functions.get(&left_id));
        let right_definition = self
            .right
            .functions
            .get(&right_id)
            .or_else(|| self.base.functions.get(&right_id));
        if !left_definition.is_some_and(|value| has_role(value))
            || !right_definition.is_some_and(|value| has_role(value))
        {
            return Err(PreparedEffectInvariant::StaleReference.into());
        }
        self.function_reference(left, right)
    }

    fn definition(
        &mut self,
        left: u64,
        right: u64,
    ) -> Result<Option<PreparedEffectReplayMismatch>, PreparedEffectFailure<C::Error>> {
        if self.definitions.contains(&(left, right)) {
            return Ok(None);
        }
        self.control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        self.definitions.insert((left, right));
        let (left, right) = (&*self.left.functions[&left], &*self.right.functions[&right]);
        match (left, right) {
            (Function::Scheme(left), Function::Scheme(right)) => {
                if !self.parameters(&left.header.parameters, &right.header.parameters)?
                    || !left
                        .header
                        .domain
                        .equal_with(&right.header.domain, self.control)
                        .map_err(PreparedEffectFailure::Control)?
                {
                    return Ok(Some(PreparedEffectReplayMismatch::FunctionDefinition));
                }
                match (&left.state, &right.state) {
                    (SchemeState::Declared, SchemeState::Declared) => {}
                    (
                        SchemeState::BodyPrepared {
                            body: left,
                            obligations: left_obligations,
                        },
                        SchemeState::BodyPrepared {
                            body: right,
                            obligations: right_obligations,
                        },
                    ) => {
                        self.push(Pair::Row(left, right))?;
                        if !self.predicate_slice(left_obligations, right_obligations)? {
                            return Ok(Some(PreparedEffectReplayMismatch::FunctionDefinition));
                        }
                    }
                    _ => return Ok(Some(PreparedEffectReplayMismatch::FunctionDefinition)),
                }
            }
            (
                Function::Unknown {
                    parameters: left,
                    eligibility: left_role,
                },
                Function::Unknown {
                    parameters: right,
                    eligibility: right_role,
                },
            ) => {
                if !self.parameters(left, right)? || left_role != right_role {
                    return Ok(Some(PreparedEffectReplayMismatch::FunctionDefinition));
                }
            }
            (Function::Imported(left), Function::Imported(right)) => {
                // A frozen child is a lexical relation owner, not a pointwise
                // row. Compare its original issuer/selection and actual fixed
                // inputs; distinct partial-specialization wrappers may share it.
                if !Arc::ptr_eq(
                    &left.completion.graph.issuer,
                    &right.completion.graph.issuer,
                ) || left.selected.reference() != right.selected.reference()
                    || left.completion.fixed.len() != right.completion.fixed.len()
                    || left.futures.len() != right.futures.len()
                {
                    return Ok(Some(PreparedEffectReplayMismatch::ImportOrigin));
                }
                for ((left_id, left), (right_id, right)) in
                    left.completion.fixed.iter().zip(&right.completion.fixed)
                {
                    self.visit()?;
                    if left_id != right_id
                        || !left
                            .equal_with(right, self.control)
                            .map_err(PreparedEffectFailure::Control)?
                    {
                        return Ok(Some(PreparedEffectReplayMismatch::ImportOrigin));
                    }
                }
                for ((left_input, left), (right_input, right)) in
                    left.futures.iter().zip(&right.futures)
                {
                    self.visit()?;
                    if left_input != right_input {
                        return Ok(Some(PreparedEffectReplayMismatch::ImportOrigin));
                    }
                    if let Some(mismatch) =
                        self.function_reference(left.reference(), right.reference())?
                    {
                        return Ok(Some(mismatch));
                    }
                }
            }
            _ => return Ok(Some(PreparedEffectReplayMismatch::FunctionDefinition)),
        }
        Ok(None)
    }

    fn choice(
        &mut self,
        left: &PreparedEffectChoice,
        right: &PreparedEffectChoice,
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        let (left, right) = (
            self.validate_reference(&left.0)?,
            self.validate_reference(&right.0)?,
        );
        match (
            self.left.choices.contains(&left),
            self.right.choices.contains(&right),
        ) {
            (false, false) => {
                if !self.base.choices.contains(&left) || !self.base.choices.contains(&right) {
                    return Err(PreparedEffectInvariant::StaleReference.into());
                }
                Ok(left == right)
            }
            (true, true) => self.choices.bind(left, right, self.control),
            _ => Ok(false),
        }
    }

    fn row_slice(
        &mut self,
        left: &'a [PreparedEffectRow],
        right: &'a [PreparedEffectRow],
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        if left.len() != right.len() {
            return Ok(false);
        }
        for (left, right) in left.iter().zip(right) {
            self.push(Pair::Row(left, right))?;
        }
        Ok(true)
    }

    fn predicate_slice(
        &mut self,
        left: &'a [PreparedEffectPredicate],
        right: &'a [PreparedEffectPredicate],
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        if left.len() != right.len() {
            return Ok(false);
        }
        for (left, right) in left.iter().zip(right) {
            self.push(Pair::Predicate(left, right))?;
        }
        Ok(true)
    }

    fn arguments(
        &mut self,
        left: &'a BTreeMap<crate::types::GenericEffectReference, PreparedEffectRow>,
        right: &'a BTreeMap<crate::types::GenericEffectReference, PreparedEffectRow>,
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        if left.len() != right.len() {
            return Ok(false);
        }
        for ((left_key, left), (right_key, right)) in left.iter().zip(right) {
            self.visit()?;
            if left_key != right_key {
                return Ok(false);
            }
            self.push(Pair::Row(left, right))?;
        }
        Ok(true)
    }

    fn row(
        &mut self,
        left: &'a PreparedEffectRow,
        right: &'a PreparedEffectRow,
    ) -> Result<Option<PreparedEffectReplayMismatch>, PreparedEffectFailure<C::Error>> {
        let (left, right) = match (left, right) {
            (PreparedEffectRow::Canonical(left), PreparedEffectRow::Canonical(right)) => {
                let matches = left
                    .equal_with(right, self.control)
                    .map_err(PreparedEffectFailure::Control)?;
                return Ok((!matches).then_some(PreparedEffectReplayMismatch::Row));
            }
            (PreparedEffectRow::Term(left), PreparedEffectRow::Term(right)) => (left, right),
            _ => return Ok(Some(PreparedEffectReplayMismatch::Row)),
        };
        let (left, right) = (
            self.validate_reference(&left.0)?,
            self.validate_reference(&right.0)?,
        );
        let left_term = self
            .left
            .rows
            .get(&left)
            .or_else(|| self.base.rows.get(&left))
            .ok_or(PreparedEffectInvariant::StaleReference)?;
        let right_term = self
            .right
            .rows
            .get(&right)
            .or_else(|| self.base.rows.get(&right))
            .ok_or(PreparedEffectInvariant::StaleReference)?;
        if self.rows.contains(&(left, right)) {
            return Ok(None);
        }
        self.control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        self.rows.insert((left, right));
        let matches = match (&**left_term, &**right_term) {
            (
                RowTerm::Application {
                    function: left,
                    arguments: left_arguments,
                },
                RowTerm::Application {
                    function: right,
                    arguments: right_arguments,
                },
            ) => {
                self.push(Pair::Function(left, right))?;
                self.row_slice(left_arguments, right_arguments)?
            }
            (
                RowTerm::Substitution {
                    formula: left,
                    arguments: left_arguments,
                },
                RowTerm::Substitution {
                    formula: right,
                    arguments: right_arguments,
                },
            ) => self.formula(left, right)? && self.arguments(left_arguments, right_arguments)?,
            (RowTerm::Union(left), RowTerm::Union(right)) => self.row_slice(left, right)?,
            (
                RowTerm::Select {
                    choice: left,
                    yes: left_yes,
                    no: left_no,
                },
                RowTerm::Select {
                    choice: right,
                    yes: right_yes,
                    no: right_no,
                },
            ) => {
                if !self.choice(left, right)? {
                    return Ok(Some(PreparedEffectReplayMismatch::ChoiceIdentity));
                }
                self.push(Pair::Row(left_yes, right_yes))?;
                self.push(Pair::Row(left_no, right_no))?;
                true
            }
            _ => false,
        };
        Ok((!matches).then_some(PreparedEffectReplayMismatch::Row))
    }

    fn predicate(
        &mut self,
        left: &'a PreparedEffectPredicate,
        right: &'a PreparedEffectPredicate,
    ) -> Result<Option<PreparedEffectReplayMismatch>, PreparedEffectFailure<C::Error>> {
        let (left, right) = match (left, right) {
            (
                PreparedEffectPredicate::Canonical(left),
                PreparedEffectPredicate::Canonical(right),
            ) => {
                let matches = left
                    .equal_with(right, self.control)
                    .map_err(PreparedEffectFailure::Control)?;
                return Ok((!matches).then_some(PreparedEffectReplayMismatch::Predicate));
            }
            (PreparedEffectPredicate::Term(left), PreparedEffectPredicate::Term(right)) => {
                (left, right)
            }
            _ => return Ok(Some(PreparedEffectReplayMismatch::Predicate)),
        };
        let (left, right) = (
            self.validate_reference(&left.0)?,
            self.validate_reference(&right.0)?,
        );
        let left_term = self
            .left
            .predicates
            .get(&left)
            .or_else(|| self.base.predicates.get(&left))
            .ok_or(PreparedEffectInvariant::StaleReference)?;
        let right_term = self
            .right
            .predicates
            .get(&right)
            .or_else(|| self.base.predicates.get(&right))
            .ok_or(PreparedEffectInvariant::StaleReference)?;
        if self.predicates.contains(&(left, right)) {
            return Ok(None);
        }
        self.control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        self.predicates.insert((left, right));
        let matches = match (&**left_term, &**right_term) {
            (
                PredicateTerm::Substitution {
                    predicate: left,
                    arguments: left_arguments,
                },
                PredicateTerm::Substitution {
                    predicate: right,
                    arguments: right_arguments,
                },
            ) => {
                left.equal_with(right, self.control)
                    .map_err(PreparedEffectFailure::Control)?
                    && self.arguments(left_arguments, right_arguments)?
            }
            (
                PredicateTerm::Relation {
                    relation: left,
                    actual: left_actual,
                    permitted: left_permitted,
                },
                PredicateTerm::Relation {
                    relation: right,
                    actual: right_actual,
                    permitted: right_permitted,
                },
            ) => {
                if left != right {
                    return Ok(Some(PreparedEffectReplayMismatch::Predicate));
                }
                self.push(Pair::Row(left_actual, right_actual))?;
                self.push(Pair::Row(left_permitted, right_permitted))?;
                true
            }
            (PredicateTerm::And(left), PredicateTerm::And(right))
            | (PredicateTerm::Or(left), PredicateTerm::Or(right)) => {
                self.predicate_slice(left, right)?
            }
            (PredicateTerm::Not(left), PredicateTerm::Not(right)) => {
                self.push(Pair::Predicate(left, right))?;
                true
            }
            (
                PredicateTerm::Select {
                    choice: left,
                    yes: left_yes,
                    no: left_no,
                },
                PredicateTerm::Select {
                    choice: right,
                    yes: right_yes,
                    no: right_no,
                },
            ) => {
                if !self.choice(left, right)? {
                    return Ok(Some(PreparedEffectReplayMismatch::ChoiceIdentity));
                }
                self.push(Pair::Predicate(left_yes, right_yes))?;
                self.push(Pair::Predicate(left_no, right_no))?;
                true
            }
            _ => false,
        };
        Ok((!matches).then_some(PreparedEffectReplayMismatch::Predicate))
    }
}

#[cfg(test)]
mod tests;
