//! Borrow binding and branch-merge helpers used by the type checker.

use super::{
    BorrowLocalState, BorrowStateCheckpoint, BorrowStateDelta, BorrowStateJournalEntry, Pattern,
    TypeChecker, TypeKind, collect_type_kind_lifetimes, merge_borrow_local_states,
    pattern_bindings_with_fallback, pattern_bindings_with_nominal_fields,
};
use crate::diagnostics::TypeCheckError;
use std::collections::BTreeSet;

impl TypeChecker<'_> {
    pub(super) fn bind_function_param(&mut self, pattern: &Pattern, ty: &TypeKind) {
        for (name, binding_ty) in
            pattern_bindings_with_nominal_fields(pattern, ty, &self.nominal_fields)
        {
            self.bind_local(name, binding_ty);
        }
    }

    pub(super) fn register_borrow_bindings(&mut self, pattern: &Pattern, fallback: &TypeKind) {
        for (name, binding_ty) in pattern_bindings_with_fallback(pattern, fallback) {
            let mut lifetimes = Vec::new();
            collect_type_kind_lifetimes(&binding_ty, &mut lifetimes);
            if lifetimes.is_empty() {
                continue;
            }
            self.stats.borrow_binding_groups += 1;
            self.stats.borrow_bindings += lifetimes.len();
            self.clear_borrow_local(&name);
            for lifetime in &lifetimes {
                self.add_active_borrow_lifetime(lifetime);
            }
            self.record_active_borrow_depth();
            self.set_borrow_local_state(name, BorrowLocalState::Live(lifetimes));
        }
    }

    pub(super) fn release_borrow_local(&mut self, name: &str) {
        let Some(state) = self.take_borrow_local_state(name) else {
            return;
        };
        match state {
            BorrowLocalState::Live(lifetimes) => {
                for lifetime in &lifetimes {
                    self.remove_active_borrow_lifetime(lifetime);
                }
                self.set_borrow_local_state(name.to_owned(), BorrowLocalState::Dropped);
            }
            BorrowLocalState::MaybeDropped(lifetimes) => {
                self.errors.push(TypeCheckError::new(format!(
                    "borrowed local `{name}` may already have been dropped on another control-flow path"
                )));
                for lifetime in &lifetimes {
                    self.remove_active_borrow_lifetime(lifetime);
                }
                self.set_borrow_local_state(name.to_owned(), BorrowLocalState::Dropped);
            }
            BorrowLocalState::Dropped => {
                self.errors.push(TypeCheckError::new(format!(
                    "borrowed local `{name}` was already dropped"
                )));
                self.set_borrow_local_state(name.to_owned(), BorrowLocalState::Dropped);
            }
        }
    }

    pub(super) fn clear_borrow_local(&mut self, name: &str) {
        let Some(state) = self.take_borrow_local_state(name) else {
            return;
        };
        for lifetime in state.lifetimes() {
            self.remove_active_borrow_lifetime(lifetime);
        }
    }

    pub(super) fn remove_active_borrow_lifetime(&mut self, lifetime: &str) {
        let Some(count) = self.active_borrow_lifetimes.get_mut(lifetime) else {
            return;
        };
        self.stats.active_borrow_removes += 1;
        self.active_borrow_total = self.active_borrow_total.saturating_sub(1);
        *count -= 1;
        if *count == 0 {
            self.active_borrow_lifetimes.remove(lifetime);
        }
    }

    pub(super) fn add_active_borrow_lifetime(&mut self, lifetime: &str) {
        *self
            .active_borrow_lifetimes
            .entry(lifetime.to_owned())
            .or_insert(0) += 1;
        self.active_borrow_total += 1;
    }

    pub(super) fn checkpoint_borrow_state(&mut self) -> BorrowStateCheckpoint {
        self.stats.borrow_state_snapshots += 1;
        BorrowStateCheckpoint {
            journal_start: self.borrow_state_journal.len(),
        }
    }

    pub(super) fn capture_borrow_state_delta(
        &mut self,
        checkpoint: BorrowStateCheckpoint,
    ) -> BorrowStateDelta {
        let touched = self.borrow_state_touched_names(checkpoint);
        let changes = touched
            .into_iter()
            .map(|name| super::BorrowStateDeltaEntry {
                state: self.borrow_local_lifetimes.get(&name).cloned(),
                name,
            })
            .collect::<Vec<_>>();
        self.stats.borrow_state_delta_entries += changes.len();
        BorrowStateDelta { changes }
    }

    pub(super) fn restore_borrow_state(&mut self, checkpoint: BorrowStateCheckpoint) {
        self.stats.borrow_state_restores += 1;
        while self.borrow_state_journal.len() > checkpoint.journal_start {
            let entry = self
                .borrow_state_journal
                .pop()
                .expect("journal length is checked before pop");
            match entry.previous {
                Some(previous) => {
                    self.borrow_local_lifetimes.insert(entry.name, previous);
                }
                None => {
                    self.borrow_local_lifetimes.remove(&entry.name);
                }
            }
        }
        self.rebuild_active_borrows();
    }

    pub(super) fn merge_borrow_state_from_deltas(
        &mut self,
        base: BorrowStateCheckpoint,
        paths: &[&BorrowStateDelta],
    ) {
        self.stats.borrow_state_merges += 1;
        self.restore_borrow_state(base);
        let merge_keys = paths
            .iter()
            .flat_map(|path| path.changes.iter().map(|entry| entry.name.clone()))
            .collect::<BTreeSet<_>>();
        self.stats.borrow_state_merge_keys += merge_keys.len();

        for name in merge_keys {
            let Some(base_state) = self.borrow_local_lifetimes.get(&name).cloned() else {
                continue;
            };
            let path_states = paths
                .iter()
                .map(|path| {
                    path.changes
                        .iter()
                        .find(|entry| entry.name == name)
                        .and_then(|entry| entry.state.as_ref())
                        .unwrap_or(&base_state)
                })
                .collect::<Vec<_>>();
            self.set_borrow_local_state(name, merge_borrow_local_states(path_states));
        }
        self.rebuild_active_borrows();
    }

    pub(super) fn clear_borrow_state(&mut self) {
        self.borrow_local_lifetimes.clear();
        self.borrow_state_journal.clear();
        self.clear_active_borrows();
    }

    pub(super) fn rebuild_active_borrows(&mut self) {
        self.clear_active_borrows();
        let lifetimes = self
            .borrow_local_lifetimes
            .values()
            .flat_map(BorrowLocalState::lifetimes)
            .cloned()
            .collect::<Vec<_>>();
        for lifetime in &lifetimes {
            self.add_active_borrow_lifetime(lifetime);
        }
        self.record_active_borrow_depth();
    }

    fn set_borrow_local_state(&mut self, name: String, state: BorrowLocalState) {
        self.record_borrow_state_previous(&name);
        self.borrow_local_lifetimes.insert(name, state);
    }

    fn take_borrow_local_state(&mut self, name: &str) -> Option<BorrowLocalState> {
        self.record_borrow_state_previous(name);
        self.borrow_local_lifetimes.remove(name)
    }

    fn record_borrow_state_previous(&mut self, name: &str) {
        self.borrow_state_journal.push(BorrowStateJournalEntry {
            name: name.to_owned(),
            previous: self.borrow_local_lifetimes.get(name).cloned(),
        });
    }

    fn borrow_state_touched_names(&self, checkpoint: BorrowStateCheckpoint) -> BTreeSet<String> {
        self.borrow_state_journal
            .iter()
            .skip(checkpoint.journal_start)
            .map(|entry| entry.name.clone())
            .collect()
    }
}
