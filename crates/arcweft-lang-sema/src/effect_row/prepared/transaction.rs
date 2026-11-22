//! Affine candidate transfers preserve the original term issuer and base state.

use super::{
    Arc, BTreeMap, BTreeSet, DecisionControl, DecisionWork, Function, PredicateTerm,
    PreparedEffectFailure, PreparedEffectGraph, PreparedEffectInvariant, PreparedEffectIssuer,
    PreparedEffectReadScope, RowTerm, ScopedObligation, scope::ActiveEffectScope,
};

mod replay;
pub(crate) use replay::{PreparedEffectReplayMismatch, PreparedEffectReplayRoot};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
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
    ancestor_limit: usize,
    ordinal: u64,
    reads: EffectCheckpointReads,
}

enum EffectCheckpointReads {
    Inherited,
    Opened {
        previous: Option<PreparedEffectReadScope>,
    },
}

/// Owns every effect fact produced by an extracted candidate, including body
/// definitions of declarations that existed before that candidate started.
/// References are operational capabilities and remain unchanged on transfer.
#[derive(Debug)]
pub(crate) struct PreparedEffectDelta {
    contents: PreparedEffectDeltaContents,
    pub(in crate::effect_row::prepared) dependencies: Option<super::PreparedEffectReadScope>,
}

#[derive(Debug)]
enum PreparedEffectDeltaContents {
    Unchanged {
        issuer: Arc<PreparedEffectIssuer>,
        base: PreparedEffectStateId,
        ordinal: u64,
    },
    Transition(Arc<PreparedEffectDeltaData>),
}

impl PreparedEffectDelta {
    pub(super) fn layer(&self) -> Option<&Arc<PreparedEffectDeltaData>> {
        match &self.contents {
            PreparedEffectDeltaContents::Unchanged { .. } => None,
            PreparedEffectDeltaContents::Transition(layer) => Some(layer),
        }
    }

    fn issuer(&self) -> &Arc<PreparedEffectIssuer> {
        match &self.contents {
            PreparedEffectDeltaContents::Unchanged { issuer, .. } => issuer,
            PreparedEffectDeltaContents::Transition(layer) => &layer.issuer,
        }
    }

    fn base(&self) -> PreparedEffectStateId {
        match &self.contents {
            PreparedEffectDeltaContents::Unchanged { base, .. } => *base,
            PreparedEffectDeltaContents::Transition(layer) => layer.base,
        }
    }

    fn ordinal(&self) -> u64 {
        match &self.contents {
            PreparedEffectDeltaContents::Unchanged { ordinal, .. } => *ordinal,
            PreparedEffectDeltaContents::Transition(layer) => layer.ordinal,
        }
    }

    /// A detached write permit can enter a publication without its probe
    /// read scope only when no other detached permit supplied its inputs.
    pub(crate) fn publishes_without_read_scope(&self) -> bool {
        self.dependencies.is_none()
    }

    pub(crate) fn has_writes(&self) -> bool {
        self.layer().is_some_and(|layer| layer.has_writes())
    }
}

#[derive(Debug)]
pub(in crate::effect_row::prepared) struct PreparedEffectDeltaData {
    pub(in crate::effect_row::prepared) issuer: Arc<PreparedEffectIssuer>,
    pub(in crate::effect_row::prepared) base: PreparedEffectStateId,
    pub(super) result: PreparedEffectStateId,
    pub(super) ancestors: Vec<PreparedEffectStateId>,
    pub(in crate::effect_row::prepared) ordinal: u64,
    pub(in crate::effect_row::prepared) inputs: Option<Arc<[u64]>>,
    pub(in crate::effect_row::prepared) functions: BTreeMap<u64, Arc<Function>>,
    pub(in crate::effect_row::prepared) obligations: Vec<Arc<ScopedObligation>>,
    pub(in crate::effect_row::prepared) rows: BTreeMap<u64, Arc<RowTerm>>,
    pub(in crate::effect_row::prepared) predicates: BTreeMap<u64, Arc<PredicateTerm>>,
    pub(in crate::effect_row::prepared) choices: BTreeSet<u64>,
}

impl PreparedEffectDeltaData {
    fn has_writes(&self) -> bool {
        !self.functions.is_empty()
            || !self.obligations.is_empty()
            || !self.rows.is_empty()
            || !self.predicates.is_empty()
            || !self.choices.is_empty()
    }
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

/// Publication failure returns every original affine permit, including parents.
#[derive(Debug)]
pub(crate) struct PreparedEffectRestoreBatchFailure<E> {
    error: PreparedEffectFailure<E>,
    deltas: Vec<PreparedEffectDelta>,
}

impl<E> PreparedEffectRestoreBatchFailure<E> {
    pub(crate) fn into_parts(self) -> (PreparedEffectFailure<E>, Vec<PreparedEffectDelta>) {
        (self.error, self.deltas)
    }
}

struct PreparedEffectPublication {
    functions: BTreeMap<u64, Arc<Function>>,
    rows: BTreeMap<u64, Arc<RowTerm>>,
    predicates: BTreeMap<u64, Arc<PredicateTerm>>,
    choices: BTreeSet<u64>,
    layers: BTreeMap<u64, Arc<PreparedEffectDeltaData>>,
    result: PreparedEffectStateId,
    ancestors: Vec<PreparedEffectStateId>,
    next_state: u64,
}

impl PreparedEffectGraph {
    pub(crate) fn owner<C: DecisionControl>(
        &self,
        control: &mut C,
    ) -> Result<super::PreparedEffectGraphOwner, PreparedEffectFailure<C::Error>> {
        control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?;
        Ok(super::PreparedEffectGraphOwner(Arc::clone(
            &self.relation.issuer,
        )))
    }

    /// The stamp identifies mutable preparation state, never semantic equality.
    /// Rolling back restores the old state; the allocator never reuses a stamp.
    pub(super) fn changed<C: DecisionControl>(
        &mut self,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let next = self
            .next_state
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::OrdinalExhausted)?;
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        self.state_ancestors.push(self.state);
        self.state = PreparedEffectStateId(self.next_state);
        self.next_state = next;
        Ok(())
    }

    pub(super) fn retains_state(&self, state: PreparedEffectStateId) -> bool {
        state == self.state || self.state_ancestors.binary_search(&state).is_ok()
    }

    pub(super) fn record_mutation(&mut self, insertion: EffectMutation) {
        if !self.checkpoints.is_empty() {
            self.journal.push(insertion);
        }
    }

    pub(super) fn publish_function(&mut self, id: u64, function: Arc<Function>) {
        let previous = self.function_writes().insert(id, function);
        self.record_mutation(EffectMutation::Function { id, previous });
    }

    fn function_writes(&mut self) -> &mut BTreeMap<u64, Arc<Function>> {
        if let Some(scope) = &mut self.active_scope {
            &mut scope.functions
        } else {
            &mut self.relation.functions
        }
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
            issuer: Arc::clone(&self.relation.issuer),
            obligation_limit: self.relation.obligations.len(),
            journal_start: self.journal.len(),
            state: self.state,
            ancestor_limit: self.state_ancestors.len(),
            ordinal,
            reads: EffectCheckpointReads::Inherited,
        })
    }

    /// Attach selected reads to a checkpoint that must be extracted or rolled
    /// back. Ordinary nested checkpoints inherit the same active function map.
    pub(crate) fn checkpoint_in_read_scope<C: DecisionControl>(
        &mut self,
        input: &PreparedEffectReadScope,
        control: &mut C,
    ) -> Result<PreparedEffectCheckpoint, PreparedEffectFailure<C::Error>> {
        input.validate(self, control)?;
        input.require_enclosing(self.active_scope.as_ref(), control)?;
        let mut checkpoint = self.checkpoint(control)?;
        let input = input.clone();
        let previous = match &mut self.active_scope {
            Some(active) => Some(std::mem::replace(&mut active.input, input)),
            None => {
                self.active_scope = Some(ActiveEffectScope {
                    input,
                    functions: BTreeMap::new(),
                });
                None
            }
        };
        checkpoint.reads = EffectCheckpointReads::Opened { previous };
        Ok(checkpoint)
    }

    pub(crate) fn validate_checkpoint(
        &self,
        checkpoint: &PreparedEffectCheckpoint,
    ) -> Result<(), PreparedEffectInvariant> {
        if !Arc::ptr_eq(&self.relation.issuer, &checkpoint.issuer) {
            return Err(PreparedEffectInvariant::ForeignIssuer);
        }
        if self.checkpoints.last() != Some(&checkpoint.ordinal) {
            return Err(PreparedEffectInvariant::CheckpointOrder);
        }
        Ok(())
    }

    /// An edit with no retained writes or ancestry needs no detached layer.
    pub(super) fn checkpoint_has_transition(
        &self,
        checkpoint: &PreparedEffectCheckpoint,
    ) -> Result<bool, PreparedEffectInvariant> {
        self.validate_checkpoint(checkpoint)?;
        Ok(self.state != checkpoint.state
            || self.state_ancestors.len() != checkpoint.ancestor_limit
            || self.journal.len() != checkpoint.journal_start
            || self.relation.obligations.len() != checkpoint.obligation_limit)
    }

    pub(crate) fn commit(
        &mut self,
        checkpoint: &PreparedEffectCheckpoint,
    ) -> Result<(), PreparedEffectInvariant> {
        self.validate_checkpoint(checkpoint)?;
        if matches!(checkpoint.reads, EffectCheckpointReads::Opened { .. }) {
            return Err(PreparedEffectInvariant::ReadScopeActive);
        }
        self.checkpoints.pop();
        if self.checkpoints.is_empty() {
            self.journal.clear();
        }
        Ok(())
    }

    fn leave_checkpoint_scope(&mut self, checkpoint: &mut PreparedEffectCheckpoint) {
        let EffectCheckpointReads::Opened { previous } =
            std::mem::replace(&mut checkpoint.reads, EffectCheckpointReads::Inherited)
        else {
            return;
        };
        match previous {
            Some(previous) => {
                self.active_scope
                    .as_mut()
                    .expect("the scope belongs to the closing checkpoint")
                    .input = previous;
            }
            None => {
                debug_assert!(
                    self.active_scope
                        .as_ref()
                        .is_some_and(|active| active.functions.is_empty())
                );
                self.active_scope = None;
            }
        }
    }

    pub(crate) fn rollback(
        &mut self,
        checkpoint: &mut PreparedEffectCheckpoint,
    ) -> Result<(), PreparedEffectInvariant> {
        self.validate_checkpoint(checkpoint)?;
        self.relation
            .obligations
            .truncate(checkpoint.obligation_limit);
        while self.journal.len() > checkpoint.journal_start {
            let insertion = self.journal.pop().expect("validated journal frontier");
            match insertion {
                EffectMutation::Function { id, previous } => {
                    if let Some(previous) = previous {
                        self.function_writes().insert(id, previous);
                    } else {
                        self.function_writes().remove(&id);
                    }
                }
                EffectMutation::Row(id) => {
                    self.relation.rows.remove(&id);
                }
                EffectMutation::Predicate(id) => {
                    self.relation.predicates.remove(&id);
                }
                EffectMutation::Choice(id) => {
                    self.relation.choices.remove(&id);
                }
            }
        }
        self.state = checkpoint.state;
        self.state_ancestors.truncate(checkpoint.ancestor_limit);
        self.checkpoints.pop();
        if self.checkpoints.is_empty() {
            self.journal.clear();
        }
        self.leave_checkpoint_scope(checkpoint);
        Ok(())
    }

    /// Preflight is fallible; detaching and restoring the base is one infallible
    /// move after it succeeds. An aborted extraction leaves its checkpoint live.
    pub(crate) fn extract_delta<C: DecisionControl>(
        &mut self,
        checkpoint: &mut PreparedEffectCheckpoint,
        control: &mut C,
    ) -> Result<PreparedEffectDelta, PreparedEffectFailure<C::Error>> {
        let has_transition = self.checkpoint_has_transition(checkpoint)?;
        control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?;
        let ordinal = self.next_delta;
        let next_delta = ordinal
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::OrdinalExhausted)?;
        if !has_transition {
            let contents = PreparedEffectDeltaContents::Unchanged {
                issuer: Arc::clone(&self.relation.issuer),
                base: checkpoint.state,
                ordinal,
            };
            return Ok(self.finish_extraction(checkpoint, next_delta, contents));
        }
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
                functions.insert(
                    *id,
                    Arc::clone(
                        self.read()
                            .function_definition(*id)
                            .expect("journal owns a function"),
                    ),
                );
            }
        }
        for () in self.journal[checkpoint.journal_start..]
            .iter()
            .map(|_| ())
            .chain(
                self.relation.obligations[checkpoint.obligation_limit..]
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
        for _ in &self.state_ancestors[checkpoint.ancestor_limit..] {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
        }
        let mut delta = PreparedEffectDeltaData {
            issuer: Arc::clone(&self.relation.issuer),
            base: checkpoint.state,
            result: self.state,
            ancestors: self.state_ancestors.split_off(checkpoint.ancestor_limit),
            ordinal,
            inputs: self
                .active_scope
                .as_ref()
                .and_then(|scope| scope.input.detached())
                .map(|data| Arc::clone(&data.ids)),
            functions,
            obligations: self
                .relation
                .obligations
                .split_off(checkpoint.obligation_limit),
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
            choices: BTreeSet::new(),
        };
        while self.journal.len() > checkpoint.journal_start {
            let insertion = self.journal.pop().expect("validated journal frontier");
            match insertion {
                EffectMutation::Function { id, previous } => {
                    if let Some(previous) = previous {
                        self.function_writes().insert(id, previous);
                    } else {
                        self.function_writes().remove(&id);
                    }
                }
                EffectMutation::Row(id) => {
                    delta.rows.insert(
                        id,
                        self.relation
                            .rows
                            .remove(&id)
                            .expect("journal owns a live row"),
                    );
                }
                EffectMutation::Predicate(id) => {
                    delta.predicates.insert(
                        id,
                        self.relation
                            .predicates
                            .remove(&id)
                            .expect("journal owns a live predicate"),
                    );
                }
                EffectMutation::Choice(id) => {
                    assert!(self.relation.choices.remove(&id));
                    delta.choices.insert(id);
                }
            }
        }
        let contents = PreparedEffectDeltaContents::Transition(Arc::new(delta));
        Ok(self.finish_extraction(checkpoint, next_delta, contents))
    }

    fn finish_extraction(
        &mut self,
        checkpoint: &mut PreparedEffectCheckpoint,
        next_delta: u64,
        contents: PreparedEffectDeltaContents,
    ) -> PreparedEffectDelta {
        self.next_delta = next_delta;
        self.state = checkpoint.state;
        self.checkpoints.pop();
        if self.checkpoints.is_empty() {
            self.journal.clear();
        }
        // The delta already owns this issuer and base. A live read at that
        // same revision adds no definitions or ancestry to its permit.
        let dependencies = self
            .active_scope
            .as_ref()
            .map(|scope| &scope.input)
            .filter(|input| !input.is_live_at(checkpoint.state))
            .cloned();
        self.leave_checkpoint_scope(checkpoint);
        PreparedEffectDelta {
            contents,
            dependencies,
        }
    }

    pub(in crate::effect_row::prepared) fn validate_delta<C: DecisionControl>(
        &self,
        delta: &PreparedEffectDelta,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.validate_delta_base(delta, control)?;
        Self::charge_delta_contents(delta, control)
    }

    pub(super) fn validate_delta_base<C: DecisionControl>(
        &self,
        delta: &PreparedEffectDelta,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.validate_delta_issuer(delta, control)?;
        if self.state != delta.base() {
            return Err(PreparedEffectInvariant::DeltaBase.into());
        }
        if let Some(scope) = &delta.dependencies {
            scope.validate_ancestry(self, |_| false, control)?;
        }
        Ok(())
    }

    fn validate_delta_issuer<C: DecisionControl>(
        &self,
        delta: &PreparedEffectDelta,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?;
        if !Arc::ptr_eq(&self.relation.issuer, delta.issuer()) {
            return Err(PreparedEffectInvariant::ForeignIssuer.into());
        }
        Ok(())
    }

    fn charge_delta_contents<C: DecisionControl>(
        delta: &PreparedEffectDelta,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let Some(delta) = delta.layer() else {
            return Ok(());
        };
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
        match self.prepare_publication(std::iter::once(&delta), control) {
            Ok(publication) => {
                self.publish_preflight(publication);
                Ok(())
            }
            Err(error) => Err(PreparedEffectRestoreFailure { error, delta }),
        }
    }

    /// Read leases never authorize publication. Every transitive source layer
    /// must be supplied with its original affine permit in this one operation.
    pub(crate) fn restore_deltas<C: DecisionControl>(
        &mut self,
        deltas: Vec<PreparedEffectDelta>,
        control: &mut C,
    ) -> Result<(), PreparedEffectRestoreBatchFailure<C::Error>> {
        match self.prepare_publication(deltas.iter(), control) {
            Ok(publication) => {
                self.publish_preflight(publication);
                Ok(())
            }
            Err(error) => Err(PreparedEffectRestoreBatchFailure { error, deltas }),
        }
    }

    fn prepare_publication<'a, C: DecisionControl>(
        &self,
        inputs: impl Iterator<Item = &'a PreparedEffectDelta>,
        control: &mut C,
    ) -> Result<PreparedEffectPublication, PreparedEffectFailure<C::Error>> {
        let mut permits = BTreeSet::new();
        let mut ordered = BTreeMap::new();
        for delta in inputs {
            self.validate_delta_issuer(delta, control)?;
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            if !permits.insert(delta.ordinal()) {
                return Err(PreparedEffectInvariant::ReadScopeConflict.into());
            }
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            ordered.insert((delta.base(), delta.ordinal()), delta);
        }
        if permits.is_empty() {
            return Err(PreparedEffectInvariant::ReadScopeDependencies.into());
        }
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        let mut witnessed = BTreeSet::from([self.state]);
        let detached =
            self.delta_scope_data(ordered.values().copied(), control, |delta, control| {
                if !witnessed.contains(&delta.base()) {
                    return Err(PreparedEffectInvariant::DeltaBase.into());
                }
                if let Some(scope) = &delta.dependencies {
                    scope.validate_ancestry(self, |base| witnessed.contains(&base), control)?;
                }
                Self::charge_delta_contents(delta, control)?;
                if let Some(layer) = delta.layer() {
                    for &state in layer.ancestors.iter().chain(std::iter::once(&layer.result)) {
                        control
                            .charge(DecisionWork::Node)
                            .map_err(PreparedEffectFailure::Control)?;
                        witnessed.insert(state);
                    }
                }
                Ok(())
            })?;
        // The publication assembles a new set of indexes. Only a retained
        // read scope shares them with its successors.
        let (mut functions, mut rows, mut predicates, mut choices, mut layers) = detached
            .map_or_else(Default::default, |data| {
                (
                    Arc::into_inner(data.functions).expect("new function index"),
                    Arc::into_inner(data.rows).expect("new row index"),
                    Arc::into_inner(data.predicates).expect("new predicate index"),
                    Arc::into_inner(data.choices).expect("new choice index"),
                    data.layers,
                )
            });
        let inherited = self
            .active_scope
            .as_ref()
            .and_then(|scope| scope.input.detached());
        for id in layers.keys() {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            let inherited = inherited.is_some_and(|scope| scope.layers.contains_key(id));
            match (permits.contains(id), inherited) {
                (true, true) => return Err(PreparedEffectInvariant::ReadScopeConflict.into()),
                (false, false) => {
                    return Err(PreparedEffectInvariant::ReadScopeDependencies.into());
                }
                _ => {}
            }
        }
        for () in functions
            .iter()
            .map(|_| ())
            .chain(rows.iter().map(|_| ()))
            .chain(predicates.iter().map(|_| ()))
            .chain(choices.iter().map(|_| ()))
            .chain(
                layers
                    .values()
                    .filter(|layer| permits.contains(&layer.ordinal))
                    .flat_map(|layer| layer.obligations.iter().map(|_| ())),
            )
        {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
        }
        if self.active_scope.is_some() {
            // The live overlay also belongs to the target checkpoint when
            // its input scope has no detached layers.
            let target = self.read();
            functions.retain(|id, function| {
                !target
                    .function_definition(*id)
                    .is_some_and(|before| Arc::ptr_eq(before, function))
            });
            if let Some(inherited) = inherited {
                rows.retain(|id, _| !inherited.rows.contains_key(id));
                predicates.retain(|id, _| !inherited.predicates.contains_key(id));
                choices.retain(|id| !inherited.choices.contains(id));
            }
            layers.retain(|id, _| permits.contains(id));
        }
        let (result, next_state) = if layers.is_empty() {
            witnessed.remove(&self.state);
            (self.state, self.next_state)
        } else if permits.len() == 1 {
            let delta = ordered.first_key_value().expect("one publication permit").1;
            let result = delta
                .layer()
                .map_or_else(|| delta.base(), |layer| layer.result);
            witnessed.remove(&result);
            (result, self.next_state)
        } else {
            (
                PreparedEffectStateId(self.next_state),
                self.next_state
                    .checked_add(1)
                    .ok_or(PreparedEffectInvariant::OrdinalExhausted)?,
            )
        };
        for _ in &witnessed {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
        }
        let ancestors = witnessed.into_iter().collect();
        Ok(PreparedEffectPublication {
            functions,
            rows,
            predicates,
            choices,
            layers,
            result,
            ancestors,
            next_state,
        })
    }

    /// All fallible work, dependency checks and allocation charges precede the
    /// first live mutation. The journal retains only changes to this live base.
    fn publish_preflight(&mut self, publication: PreparedEffectPublication) {
        let PreparedEffectPublication {
            functions,
            rows,
            predicates,
            choices,
            layers,
            result,
            ancestors,
            next_state,
        } = publication;
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
        self.relation.rows.extend(rows);
        self.relation.predicates.extend(predicates);
        self.relation.choices.extend(choices);
        self.relation.obligations.extend(
            layers
                .values()
                .flat_map(|layer| layer.obligations.iter().map(Arc::clone)),
        );
        self.state_ancestors.extend(ancestors);
        self.state = result;
        self.next_state = next_state;
    }
}

#[cfg(test)]
mod tests;
