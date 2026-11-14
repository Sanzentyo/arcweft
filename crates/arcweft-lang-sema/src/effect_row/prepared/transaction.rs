//! Affine candidate transfers preserve the original term issuer and base state.

use super::{
    Arc, BTreeMap, BTreeSet, DecisionControl, DecisionWork, Function, PredicateTerm,
    PreparedEffectFailure, PreparedEffectGraph, PreparedEffectInvariant, PreparedEffectIssuer,
    RowTerm, ScopedObligation,
};

mod replay;
pub(crate) use replay::{PreparedEffectReplayMismatch, PreparedEffectReplayRoot};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PreparedEffectStateId(u64);

impl PreparedEffectStateId {
    pub(super) const INITIAL: Self = Self(0);
}

#[derive(Debug)]
pub(super) enum EffectMutation {
    Function {
        id: u64,
        previous: Option<Arc<Function>>,
    },
    Row(u64),
    Predicate(u64),
    Choice(u64),
}

/// Only the mutation journal frontier is retained. Definitions outside the
/// candidate are neither copied nor enumerated; allocators are never restored.
pub(crate) struct PreparedEffectCheckpoint {
    issuer: Arc<PreparedEffectIssuer>,
    obligation_limit: usize,
    journal_start: usize,
    state: PreparedEffectStateId,
    ordinal: u64,
}

/// Owns every effect fact produced by an extracted candidate, including body
/// definitions of declarations that existed before that candidate started.
/// References are operational capabilities and remain unchanged on transfer.
#[derive(Debug)]
pub(crate) struct PreparedEffectDelta {
    data: Box<PreparedEffectDeltaData>,
}

#[derive(Debug)]
struct PreparedEffectDeltaData {
    issuer: Arc<PreparedEffectIssuer>,
    base: PreparedEffectStateId,
    result: PreparedEffectStateId,
    functions: BTreeMap<u64, Arc<Function>>,
    obligations: Vec<ScopedObligation>,
    rows: BTreeMap<u64, Arc<RowTerm>>,
    predicates: BTreeMap<u64, Arc<PredicateTerm>>,
    choices: BTreeSet<u64>,
}

/// A failed preflight returns the original affine payload for retry or discard.
#[derive(Debug)]
pub(crate) struct PreparedEffectRestoreFailure<E> {
    error: PreparedEffectFailure<E>,
    delta: PreparedEffectDelta,
}

impl<E> PreparedEffectRestoreFailure<E> {
    pub(crate) fn into_parts(self) -> (PreparedEffectFailure<E>, PreparedEffectDelta) {
        (self.error, self.delta)
    }
}

impl PreparedEffectGraph {
    pub(crate) fn owner<C: DecisionControl>(
        &self,
        control: &mut C,
    ) -> Result<super::PreparedEffectGraphOwner, PreparedEffectFailure<C::Error>> {
        control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?;
        Ok(super::PreparedEffectGraphOwner(Arc::clone(&self.issuer)))
    }

    /// The stamp identifies mutable preparation state, never semantic equality.
    /// Rolling back restores the old state; the allocator never reuses a stamp.
    pub(super) fn changed(&mut self) -> Result<(), PreparedEffectInvariant> {
        let next = self
            .next_state
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::OrdinalExhausted)?;
        self.state = PreparedEffectStateId(self.next_state);
        self.next_state = next;
        Ok(())
    }

    pub(super) fn record_mutation(&mut self, insertion: EffectMutation) {
        if !self.checkpoints.is_empty() {
            self.journal.push(insertion);
        }
    }

    pub(super) fn publish_function(&mut self, id: u64, function: Arc<Function>) {
        let previous = self.functions.insert(id, function);
        self.record_mutation(EffectMutation::Function { id, previous });
    }

    pub(crate) fn checkpoint<C: DecisionControl>(
        &mut self,
        control: &mut C,
    ) -> Result<PreparedEffectCheckpoint, PreparedEffectFailure<C::Error>> {
        control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?;
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
            obligation_limit: self.obligations.len(),
            journal_start: self.journal.len(),
            state: self.state,
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
        if self.checkpoints.is_empty() {
            self.journal.clear();
        }
        Ok(())
    }

    pub(crate) fn rollback(
        &mut self,
        checkpoint: &mut PreparedEffectCheckpoint,
    ) -> Result<(), PreparedEffectInvariant> {
        self.validate_checkpoint(checkpoint)?;
        self.obligations.truncate(checkpoint.obligation_limit);
        for insertion in self.journal.drain(checkpoint.journal_start..).rev() {
            match insertion {
                EffectMutation::Function { id, previous } => {
                    if let Some(previous) = previous {
                        self.functions.insert(id, previous);
                    } else {
                        self.functions.remove(&id);
                    }
                }
                EffectMutation::Row(id) => {
                    self.rows.remove(&id);
                }
                EffectMutation::Predicate(id) => {
                    self.predicates.remove(&id);
                }
                EffectMutation::Choice(id) => {
                    self.choices.remove(&id);
                }
            }
        }
        self.state = checkpoint.state;
        self.checkpoints.pop();
        if self.checkpoints.is_empty() {
            self.journal.clear();
        }
        Ok(())
    }

    /// Preflight is fallible; detaching and restoring the base is one infallible
    /// move after it succeeds. An aborted extraction leaves its checkpoint live.
    pub(crate) fn extract_delta<C: DecisionControl>(
        &mut self,
        checkpoint: &mut PreparedEffectCheckpoint,
        control: &mut C,
    ) -> Result<PreparedEffectDelta, PreparedEffectFailure<C::Error>> {
        self.validate_checkpoint(checkpoint)?;
        control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?;
        let mut functions = BTreeMap::new();
        for insertion in &self.journal[checkpoint.journal_start..] {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            if let EffectMutation::Function { id, .. } = insertion
                && !functions.contains_key(id)
            {
                control
                    .charge(DecisionWork::Node)
                    .map_err(PreparedEffectFailure::Control)?;
                functions.insert(*id, Arc::clone(&self.functions[id]));
            }
        }
        for () in self.journal[checkpoint.journal_start..]
            .iter()
            .map(|_| ())
            .chain(
                self.obligations[checkpoint.obligation_limit..]
                    .iter()
                    .map(|_| ()),
            )
        {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
        }
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        let mut delta = Box::new(PreparedEffectDeltaData {
            issuer: Arc::clone(&self.issuer),
            base: checkpoint.state,
            result: self.state,
            functions,
            obligations: self.obligations.split_off(checkpoint.obligation_limit),
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
            choices: BTreeSet::new(),
        });
        for insertion in self.journal.drain(checkpoint.journal_start..).rev() {
            match insertion {
                EffectMutation::Function { id, previous } => {
                    if let Some(previous) = previous {
                        self.functions.insert(id, previous);
                    } else {
                        self.functions.remove(&id);
                    }
                }
                EffectMutation::Row(id) => {
                    delta
                        .rows
                        .insert(id, self.rows.remove(&id).expect("journal owns a live row"));
                }
                EffectMutation::Predicate(id) => {
                    delta.predicates.insert(
                        id,
                        self.predicates
                            .remove(&id)
                            .expect("journal owns a live predicate"),
                    );
                }
                EffectMutation::Choice(id) => {
                    assert!(self.choices.remove(&id));
                    delta.choices.insert(id);
                }
            }
        }
        self.state = checkpoint.state;
        self.checkpoints.pop();
        if self.checkpoints.is_empty() {
            self.journal.clear();
        }
        Ok(PreparedEffectDelta { data: delta })
    }

    fn validate_delta<C: DecisionControl>(
        &self,
        delta: &PreparedEffectDelta,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let delta = &delta.data;
        control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?;
        if !Arc::ptr_eq(&self.issuer, &delta.issuer) {
            return Err(PreparedEffectInvariant::ForeignIssuer.into());
        }
        if self.state != delta.base {
            return Err(PreparedEffectInvariant::DeltaBase.into());
        }
        for () in delta
            .functions
            .iter()
            .map(|_| ())
            .chain(delta.obligations.iter().map(|_| ()))
            .chain(delta.rows.iter().map(|_| ()))
            .chain(delta.predicates.iter().map(|_| ()))
            .chain(delta.choices.iter().map(|_| ()))
        {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
        }
        Ok(())
    }

    pub(crate) fn restore_delta<C: DecisionControl>(
        &mut self,
        delta: PreparedEffectDelta,
        control: &mut C,
    ) -> Result<(), PreparedEffectRestoreFailure<C::Error>> {
        if let Err(error) = self.validate_delta(&delta, control) {
            return Err(PreparedEffectRestoreFailure { error, delta });
        }
        let PreparedEffectDeltaData {
            functions,
            mut obligations,
            rows,
            predicates,
            choices,
            result,
            ..
        } = *delta.data;
        for id in rows.keys() {
            self.record_mutation(EffectMutation::Row(*id));
        }
        for id in predicates.keys() {
            self.record_mutation(EffectMutation::Predicate(*id));
        }
        for id in &choices {
            self.record_mutation(EffectMutation::Choice(*id));
        }
        for (id, function) in functions {
            self.publish_function(id, function);
        }
        self.rows.extend(rows);
        self.predicates.extend(predicates);
        self.choices.extend(choices);
        self.obligations.append(&mut obligations);
        self.state = result;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
