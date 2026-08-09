//! Candidate-transaction state and mutation rollback.

use super::{
    BTreeMap, BTreeSet, CallTargetFacts, CheckedCallArgumentSlotSource, CheckedExpression, ExprId,
    LocalId, PatternId, PendingCallAnalysis, TypeKind,
};

/// Sole mutable owner for candidate-sensitive semantic facts.
///
/// Every write flows through this owner so candidate probes cannot bypass the
/// rollback journal by mutating one fact map directly.
pub(super) struct SemanticFactState {
    locals: BTreeMap<LocalId, TypeKind>,
    patterns: BTreeMap<PatternId, TypeKind>,
    expressions: BTreeMap<ExprId, CheckedExpression>,
    expression_stack: BTreeSet<ExprId>,
    pending_calls: BTreeMap<ExprId, PendingCallAnalysis>,
    calls: BTreeMap<ExprId, CallTargetFacts>,
    candidate_checkpoints: Vec<usize>,
    candidate_journal: Vec<SemanticFactMutation>,
}

#[derive(Default)]
pub(super) struct CandidateSemanticProjection {
    locals: BTreeMap<LocalId, Option<TypeKind>>,
    patterns: BTreeMap<PatternId, Option<TypeKind>>,
    expressions: BTreeMap<ExprId, Option<CheckedExpression>>,
    pending_calls: BTreeMap<ExprId, Option<PendingCallAnalysis>>,
    calls: BTreeMap<ExprId, Option<CallTargetFacts>>,
}

enum SemanticFactMutation {
    Local {
        owner: LocalId,
        previous: Option<Box<TypeKind>>,
    },
    Pattern {
        owner: PatternId,
        previous: Option<Box<TypeKind>>,
    },
    Expression {
        owner: ExprId,
        previous: Option<Box<CheckedExpression>>,
    },
    PendingCall {
        owner: ExprId,
        previous: Option<Box<PendingCallAnalysis>>,
    },
    Call {
        owner: ExprId,
        previous: Option<Box<CallTargetFacts>>,
    },
}

impl SemanticFactState {
    pub(super) fn new() -> Self {
        Self {
            locals: BTreeMap::new(),
            patterns: BTreeMap::new(),
            expressions: BTreeMap::new(),
            expression_stack: BTreeSet::new(),
            pending_calls: BTreeMap::new(),
            calls: BTreeMap::new(),
            candidate_checkpoints: Vec::new(),
            candidate_journal: Vec::new(),
        }
    }

    pub(super) const fn locals(&self) -> &BTreeMap<LocalId, TypeKind> {
        &self.locals
    }

    pub(super) const fn patterns(&self) -> &BTreeMap<PatternId, TypeKind> {
        &self.patterns
    }

    pub(super) const fn expressions(&self) -> &BTreeMap<ExprId, CheckedExpression> {
        &self.expressions
    }

    pub(super) const fn pending_calls(&self) -> &BTreeMap<ExprId, PendingCallAnalysis> {
        &self.pending_calls
    }

    pub(super) const fn calls(&self) -> &BTreeMap<ExprId, CallTargetFacts> {
        &self.calls
    }

    pub(super) fn begin_expression(&mut self, owner: ExprId) -> bool {
        self.expression_stack.insert(owner)
    }

    pub(super) fn end_expression(&mut self, owner: ExprId) {
        self.expression_stack.remove(&owner);
    }

    pub(super) fn begin_candidate_transaction(&mut self) -> usize {
        let checkpoint = self.candidate_journal.len();
        self.candidate_checkpoints.push(checkpoint);
        checkpoint
    }

    pub(super) fn commit_candidate_transaction(&mut self, checkpoint: usize) {
        let active = self
            .candidate_checkpoints
            .pop()
            .expect("candidate transaction commit has one active checkpoint");
        assert_eq!(active, checkpoint, "candidate transactions commit LIFO");
        if self.candidate_checkpoints.is_empty() {
            self.candidate_journal.clear();
        }
    }

    pub(super) fn rollback_candidate_transaction(&mut self, checkpoint: usize) {
        let active = self
            .candidate_checkpoints
            .pop()
            .expect("candidate transaction rollback has one active checkpoint");
        assert_eq!(active, checkpoint, "candidate transactions roll back LIFO");
        while self.candidate_journal.len() > checkpoint {
            match self
                .candidate_journal
                .pop()
                .expect("journal length was checked")
            {
                SemanticFactMutation::Local { owner, previous } => {
                    restore_map_entry(&mut self.locals, owner, previous.map(|previous| *previous));
                }
                SemanticFactMutation::Pattern { owner, previous } => {
                    restore_map_entry(
                        &mut self.patterns,
                        owner,
                        previous.map(|previous| *previous),
                    );
                }
                SemanticFactMutation::Expression { owner, previous } => {
                    restore_map_entry(
                        &mut self.expressions,
                        owner,
                        previous.map(|previous| *previous),
                    );
                }
                SemanticFactMutation::PendingCall { owner, previous } => {
                    restore_map_entry(
                        &mut self.pending_calls,
                        owner,
                        previous.map(|previous| *previous),
                    );
                }
                SemanticFactMutation::Call { owner, previous } => {
                    restore_map_entry(&mut self.calls, owner, previous.map(|previous| *previous));
                }
            }
        }
        if self.candidate_checkpoints.is_empty() {
            self.candidate_journal.clear();
        }
    }

    pub(super) fn capture_candidate_projection(
        &self,
        checkpoint: usize,
    ) -> CandidateSemanticProjection {
        let mut local_owners = BTreeSet::new();
        let mut pattern_owners = BTreeSet::new();
        let mut expression_owners = BTreeSet::new();
        let mut pending_owners = BTreeSet::new();
        let mut call_owners = BTreeSet::new();
        for mutation in &self.candidate_journal[checkpoint..] {
            match mutation {
                SemanticFactMutation::Local { owner, .. } => {
                    local_owners.insert(*owner);
                }
                SemanticFactMutation::Pattern { owner, .. } => {
                    pattern_owners.insert(*owner);
                }
                SemanticFactMutation::Expression { owner, .. } => {
                    expression_owners.insert(*owner);
                }
                SemanticFactMutation::PendingCall { owner, .. } => {
                    pending_owners.insert(*owner);
                }
                SemanticFactMutation::Call { owner, .. } => {
                    call_owners.insert(*owner);
                }
            }
        }
        CandidateSemanticProjection {
            locals: local_owners
                .into_iter()
                .map(|owner| (owner, self.locals.get(&owner).cloned()))
                .collect(),
            patterns: pattern_owners
                .into_iter()
                .map(|owner| (owner, self.patterns.get(&owner).cloned()))
                .collect(),
            expressions: expression_owners
                .into_iter()
                .map(|owner| (owner, self.expressions.get(&owner).cloned()))
                .collect(),
            pending_calls: pending_owners
                .into_iter()
                .map(|owner| (owner, self.pending_calls.get(&owner).cloned()))
                .collect(),
            calls: call_owners
                .into_iter()
                .map(|owner| (owner, self.calls.get(&owner).cloned()))
                .collect(),
        }
    }

    pub(super) fn apply_candidate_projection(&mut self, projection: CandidateSemanticProjection) {
        assert!(
            self.candidate_checkpoints.is_empty(),
            "deterministic recovery projection publishes outside candidate probes"
        );
        for (owner, value) in projection.locals {
            restore_map_entry(&mut self.locals, owner, value);
        }
        for (owner, value) in projection.patterns {
            restore_map_entry(&mut self.patterns, owner, value);
        }
        for (owner, value) in projection.expressions {
            restore_map_entry(&mut self.expressions, owner, value);
        }
        for (owner, value) in projection.pending_calls {
            restore_map_entry(&mut self.pending_calls, owner, value);
        }
        for (owner, value) in projection.calls {
            restore_map_entry(&mut self.calls, owner, value);
        }
    }

    pub(super) fn set_local_type(&mut self, owner: LocalId, value: TypeKind) -> bool {
        let previous = self.locals.insert(owner, value);
        let replaced = previous.is_some();
        if !self.candidate_checkpoints.is_empty() {
            self.candidate_journal.push(SemanticFactMutation::Local {
                owner,
                previous: previous.map(Box::new),
            });
        }
        replaced
    }

    pub(super) fn set_pattern_type(&mut self, owner: PatternId, value: TypeKind) -> bool {
        let previous = self.patterns.insert(owner, value);
        let replaced = previous.is_some();
        if !self.candidate_checkpoints.is_empty() {
            self.candidate_journal.push(SemanticFactMutation::Pattern {
                owner,
                previous: previous.map(Box::new),
            });
        }
        replaced
    }
}

impl SemanticFactState {
    pub(super) fn set_expression(&mut self, owner: ExprId, value: CheckedExpression) -> bool {
        let previous = self.expressions.insert(owner, value);
        let replaced = previous.is_some();
        if !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::Expression {
                    owner,
                    previous: previous.map(Box::new),
                });
        }
        replaced
    }

    fn remove_expression(&mut self, owner: ExprId) {
        let previous = self.expressions.remove(&owner);
        if previous.is_some() && !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::Expression {
                    owner,
                    previous: previous.map(Box::new),
                });
        }
    }

    pub(super) fn set_pending_call(&mut self, owner: ExprId, value: PendingCallAnalysis) -> bool {
        let previous = self.pending_calls.insert(owner, value);
        let replaced = previous.is_some();
        if !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::PendingCall {
                    owner,
                    previous: previous.map(Box::new),
                });
        }
        replaced
    }

    fn remove_pending_call(&mut self, owner: ExprId) {
        let previous = self.pending_calls.remove(&owner);
        if previous.is_some() && !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::PendingCall {
                    owner,
                    previous: previous.map(Box::new),
                });
        }
    }

    pub(super) fn set_call_fact(&mut self, owner: ExprId, value: CallTargetFacts) -> bool {
        let previous = self.calls.insert(owner, value);
        let replaced = previous.is_some();
        if !self.candidate_checkpoints.is_empty() {
            self.candidate_journal.push(SemanticFactMutation::Call {
                owner,
                previous: previous.map(Box::new),
            });
        }
        replaced
    }

    fn remove_call_fact(&mut self, owner: ExprId) {
        let previous = self.calls.remove(&owner);
        if previous.is_some() && !self.candidate_checkpoints.is_empty() {
            self.candidate_journal.push(SemanticFactMutation::Call {
                owner,
                previous: previous.map(Box::new),
            });
        }
    }

    pub(super) fn prepare_physical_slot_evaluation(
        &mut self,
        source: CheckedCallArgumentSlotSource,
    ) {
        let CheckedCallArgumentSlotSource::Expression(owner) = source else {
            return;
        };
        self.remove_expression(owner);
        self.remove_pending_call(owner);
        self.remove_call_fact(owner);
    }
}

fn restore_map_entry<K: Ord, V>(map: &mut BTreeMap<K, V>, key: K, value: Option<V>) {
    if let Some(value) = value {
        map.insert(key, value);
    } else {
        map.remove(&key);
    }
}
