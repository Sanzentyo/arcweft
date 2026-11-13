//! Affine candidate transfers preserve the original term issuer and base state.

use super::{
    Arc, BTreeMap, BTreeSet, DecisionControl, DecisionWork, Function, PredicateTerm,
    PreparedEffectFailure, PreparedEffectGraph, PreparedEffectInvariant, PreparedEffectIssuer,
    RowTerm, ScopedObligation,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PreparedEffectStateId(u64);

impl PreparedEffectStateId {
    pub(super) const INITIAL: Self = Self(0);
}

#[derive(Clone, Copy, Debug)]
pub(super) enum EffectInsertion {
    Row(u64),
    Predicate(u64),
    Choice(u64),
}

/// Immutable definitions are shared. Allocator ordinals are never restored.
pub(crate) struct PreparedEffectCheckpoint {
    issuer: Arc<PreparedEffectIssuer>,
    functions: BTreeMap<u64, Arc<Function>>,
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

    pub(super) fn record_insertion(&mut self, insertion: EffectInsertion) {
        if !self.checkpoints.is_empty() {
            self.insertions.push(insertion);
        }
    }

    pub(crate) fn checkpoint<C: DecisionControl>(
        &mut self,
        control: &mut C,
    ) -> Result<PreparedEffectCheckpoint, PreparedEffectFailure<C::Error>> {
        for () in self
            .functions
            .iter()
            .map(|_| ())
            .chain(self.obligations.iter().map(|_| ()))
        {
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
            functions: self.functions.clone(),
            obligation_limit: self.obligations.len(),
            journal_start: self.insertions.len(),
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
            self.insertions.clear();
        }
        Ok(())
    }

    pub(crate) fn rollback(
        &mut self,
        checkpoint: &mut PreparedEffectCheckpoint,
    ) -> Result<(), PreparedEffectInvariant> {
        self.validate_checkpoint(checkpoint)?;
        self.functions = std::mem::take(&mut checkpoint.functions);
        self.obligations.truncate(checkpoint.obligation_limit);
        for insertion in self.insertions.drain(checkpoint.journal_start..) {
            match insertion {
                EffectInsertion::Row(id) => {
                    self.rows.remove(&id);
                }
                EffectInsertion::Predicate(id) => {
                    self.predicates.remove(&id);
                }
                EffectInsertion::Choice(id) => {
                    self.choices.remove(&id);
                }
            }
        }
        self.state = checkpoint.state;
        self.checkpoints.pop();
        if self.checkpoints.is_empty() {
            self.insertions.clear();
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
        for (id, function) in &self.functions {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            if !checkpoint
                .functions
                .get(id)
                .is_some_and(|base| Arc::ptr_eq(base, function))
            {
                functions.insert(*id, Arc::clone(function));
            }
        }
        for () in self.insertions[checkpoint.journal_start..]
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
        for insertion in self.insertions.drain(checkpoint.journal_start..) {
            match insertion {
                EffectInsertion::Row(id) => {
                    delta
                        .rows
                        .insert(id, self.rows.remove(&id).expect("journal owns a live row"));
                }
                EffectInsertion::Predicate(id) => {
                    delta.predicates.insert(
                        id,
                        self.predicates
                            .remove(&id)
                            .expect("journal owns a live predicate"),
                    );
                }
                EffectInsertion::Choice(id) => {
                    assert!(self.choices.remove(&id));
                    delta.choices.insert(id);
                }
            }
        }
        self.functions = std::mem::take(&mut checkpoint.functions);
        self.state = checkpoint.state;
        self.checkpoints.pop();
        if self.checkpoints.is_empty() {
            self.insertions.clear();
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
            self.record_insertion(EffectInsertion::Row(*id));
        }
        for id in predicates.keys() {
            self.record_insertion(EffectInsertion::Predicate(*id));
        }
        for id in &choices {
            self.record_insertion(EffectInsertion::Choice(*id));
        }
        self.functions.extend(functions);
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
