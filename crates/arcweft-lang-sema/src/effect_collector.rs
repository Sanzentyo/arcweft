//! Integration facade used by the existing checker traversal.
//!
//! This type deliberately does not walk HIR a second time. `TypeChecker`
//! enters a callable before checking its body, and records named calls and
//! primitive effects at the existing expression/statement dispatch points.

use std::collections::BTreeMap;

use crate::{
    effect_analysis::{EffectAnalysisReport, analyze_effects},
    effect_model::{
        CallEdge, CallableFacts, CallableId, CallableKind, DuplicateCallableError, EffectContract,
        EffectProgram, EffectSite, ExternalCallable, Visibility,
    },
    effect_row::{EffectRow, EffectVar, EffectVarSupply},
    effects::{EffectId, EffectSet},
};

/// Stateful facts collector embedded in `TypeChecker`.
#[derive(Clone, Debug, Default)]
pub struct EffectCollector {
    program: EffectProgram,
    current: Option<CallableId>,
    known: BTreeMap<String, Option<CallableId>>,
    inferred_rows: BTreeMap<CallableId, EffectVar>,
    effect_vars: EffectVarSupply,
    transaction_depth: usize,
    transaction_journal: Vec<EffectMutation>,
}

#[derive(Clone, Debug)]
enum EffectMutation {
    RegisteredCallable {
        id: CallableId,
        source_name: String,
        previous_known: PreviousKnownCallable,
    },
    InferredRowInserted {
        callable: CallableId,
    },
    RecordedCall {
        caller: CallableId,
    },
    RecordedEffect {
        caller: CallableId,
    },
}

#[derive(Clone, Debug)]
enum PreviousKnownCallable {
    Missing,
    Ambiguous,
    Known(CallableId),
}

#[derive(Clone, Debug)]
pub(crate) struct EffectCollectorCheckpoint {
    journal_start: usize,
    current: Option<CallableId>,
    effect_vars: EffectVarSupply,
}

impl EffectCollector {
    pub fn new(available_capabilities: Option<EffectSet>) -> Self {
        Self {
            program: available_capabilities.map_or_else(EffectProgram::new, |effects| {
                EffectProgram::new().with_available_capabilities(effects)
            }),
            current: None,
            known: BTreeMap::new(),
            inferred_rows: BTreeMap::new(),
            effect_vars: EffectVarSupply::default(),
            transaction_depth: 0,
            transaction_journal: Vec::new(),
        }
    }

    pub(crate) fn checkpoint(&mut self) -> EffectCollectorCheckpoint {
        self.transaction_depth += 1;
        EffectCollectorCheckpoint {
            journal_start: self.transaction_journal.len(),
            current: self.current.clone(),
            effect_vars: self.effect_vars.clone(),
        }
    }

    pub(crate) fn rollback(&mut self, checkpoint: EffectCollectorCheckpoint) {
        while self.transaction_journal.len() > checkpoint.journal_start {
            match self
                .transaction_journal
                .pop()
                .expect("journal length was checked")
            {
                EffectMutation::RegisteredCallable {
                    id,
                    source_name,
                    previous_known,
                } => {
                    self.program.remove_callable(&id);
                    match previous_known {
                        PreviousKnownCallable::Known(previous) => {
                            self.known.insert(source_name, Some(previous));
                        }
                        PreviousKnownCallable::Ambiguous => {
                            self.known.insert(source_name, None);
                        }
                        PreviousKnownCallable::Missing => {
                            self.known.remove(&source_name);
                        }
                    }
                }
                EffectMutation::InferredRowInserted { callable } => {
                    self.inferred_rows.remove(&callable);
                }
                EffectMutation::RecordedCall { caller } => self
                    .program
                    .callable_mut(&caller)
                    .expect("entered callable remains registered")
                    .pop_call(),
                EffectMutation::RecordedEffect { caller } => self
                    .program
                    .callable_mut(&caller)
                    .expect("entered callable remains registered")
                    .pop_direct_effect(),
            }
        }
        self.current = checkpoint.current;
        self.effect_vars = checkpoint.effect_vars;
        self.transaction_depth = self
            .transaction_depth
            .checked_sub(1)
            .expect("effect checkpoints roll back exactly once");
    }

    pub(crate) fn commit(&mut self, checkpoint: &EffectCollectorCheckpoint) {
        self.transaction_depth = self
            .transaction_depth
            .checked_sub(1)
            .expect("effect checkpoints commit exactly once");
        if self.transaction_depth == 0 {
            self.transaction_journal.truncate(checkpoint.journal_start);
        }
    }

    pub fn register_callable(
        &mut self,
        source_name: impl Into<String>,
        id: CallableId,
        kind: CallableKind,
        visibility: Visibility,
        contract: EffectContract,
    ) -> Result<(), DuplicateCallableError> {
        let source_name = source_name.into();
        let previous_known = match self.known.get(&source_name) {
            Some(Some(callable)) => PreviousKnownCallable::Known(callable.clone()),
            Some(None) => PreviousKnownCallable::Ambiguous,
            None => PreviousKnownCallable::Missing,
        };
        self.program
            .insert(CallableFacts::new(id.clone(), kind, visibility).with_contract(contract))?;
        if self.transaction_depth != 0 {
            self.transaction_journal
                .push(EffectMutation::RegisteredCallable {
                    id: id.clone(),
                    source_name: source_name.clone(),
                    previous_known,
                });
        }
        self.known
            .entry(source_name)
            .and_modify(|existing| *existing = None)
            .or_insert(Some(id));
        Ok(())
    }

    /// Allocates the open row owned by a callable whose value semantics are
    /// defined by the ordinary function/closure inference model.
    pub(crate) fn ensure_inferred_effect_row(&mut self, callable: &CallableId) -> EffectRow {
        let tail = if let Some(tail) = self.inferred_rows.get(callable) {
            *tail
        } else {
            let tail = self.effect_vars.fresh();
            self.inferred_rows.insert(callable.clone(), tail);
            if self.transaction_depth != 0 {
                self.transaction_journal
                    .push(EffectMutation::InferredRowInserted {
                        callable: callable.clone(),
                    });
            }
            tail
        };
        EffectRow::open(EffectSet::new(), tail)
    }

    /// Returns the fresh open row owned by an analyzable function body.
    pub fn inferred_effect_row(&self, callable: &CallableId) -> Option<EffectRow> {
        self.inferred_rows
            .get(callable)
            .copied()
            .map(|tail| EffectRow::open(EffectSet::new(), tail))
    }

    /// Returns the registered callable identity for a source-level name.
    pub(crate) fn registered_callable(&self, source_name: &str) -> Option<&CallableId> {
        self.known.get(source_name).and_then(Option::as_ref)
    }

    pub fn enter(&mut self, id: CallableId) -> Option<CallableId> {
        self.current.replace(id)
    }

    pub fn restore(&mut self, previous: Option<CallableId>) {
        self.current = previous;
    }

    pub fn current_callable(&self) -> Option<CallableId> {
        self.current.clone()
    }

    pub fn record_named_call(
        &mut self,
        source_name: &str,
        external_effects: Option<EffectSet>,
        site: EffectSite,
    ) {
        let Some(current) = self.current.clone() else {
            return;
        };
        let Some(edge) = (match (
            self.known.get(source_name).and_then(Option::as_ref),
            external_effects,
        ) {
            (Some(callee), _) => Some(CallEdge::local(callee.clone(), site)),
            (None, Some(effects)) => Some(CallEdge::external(
                ExternalCallable::new(source_name, effects),
                site,
            )),
            (None, None) => None,
        }) else {
            return;
        };
        if self.transaction_depth != 0 {
            self.transaction_journal.push(EffectMutation::RecordedCall {
                caller: current.clone(),
            });
        }
        self.current_facts_mut(&current).record_call(edge);
    }

    pub fn record_dynamic_call(
        &mut self,
        label: impl Into<String>,
        effects: Option<EffectSet>,
        site: EffectSite,
    ) {
        let Some(current) = self.current.clone() else {
            return;
        };
        if self.transaction_depth != 0 {
            self.transaction_journal.push(EffectMutation::RecordedCall {
                caller: current.clone(),
            });
        }
        self.current_facts_mut(&current)
            .record_call(CallEdge::dynamic(label, effects, site));
    }

    pub fn record_local_call(&mut self, callee: CallableId, site: EffectSite) {
        let Some(current) = self.current.clone() else {
            return;
        };
        if self.transaction_depth != 0 {
            self.transaction_journal.push(EffectMutation::RecordedCall {
                caller: current.clone(),
            });
        }
        self.current_facts_mut(&current)
            .record_call(CallEdge::local(callee, site));
    }

    pub fn record_local_call_from(
        &mut self,
        caller_id: &CallableId,
        target: CallableId,
        site: EffectSite,
    ) {
        if self.transaction_depth != 0 {
            self.transaction_journal.push(EffectMutation::RecordedCall {
                caller: caller_id.clone(),
            });
        }
        self.current_facts_mut(caller_id)
            .record_call(CallEdge::local(target, site));
    }

    pub fn record_effect(&mut self, effect: EffectId, site: EffectSite) {
        let Some(current) = self.current.clone() else {
            return;
        };
        if self.transaction_depth != 0 {
            self.transaction_journal
                .push(EffectMutation::RecordedEffect {
                    caller: current.clone(),
                });
        }
        self.current_facts_mut(&current).record_effect(effect, site);
    }

    pub fn finish(self) -> EffectAnalysisReport {
        analyze_effects(&self.program, &self.inferred_rows)
    }

    fn current_facts_mut(&mut self, id: &CallableId) -> &mut CallableFacts {
        // Production code should expose `EffectProgram::callable_mut` rather
        // than indexing private storage. Kept explicit here so the integration
        // point cannot silently drop facts.
        self.program
            .callable_mut(id)
            .expect("entered callable must have been registered")
    }
}
