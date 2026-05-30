//! Borrow binding and branch-merge helpers used by the type checker.

use super::{
    BorrowLocalState, BorrowStateSnapshot, Pattern, TypeChecker, TypeKind,
    collect_type_kind_lifetimes, merge_borrow_local_states, pattern_bindings_with_fallback,
};
use crate::diagnostics::TypeCheckError;
use std::collections::HashMap;

impl TypeChecker<'_> {
    pub(super) fn bind_function_param(&mut self, pattern: &Pattern, ty: &TypeKind) {
        for (name, binding_ty) in pattern_bindings_with_fallback(pattern, ty) {
            self.locals.insert(name, binding_ty);
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
            self.active_borrows.extend(lifetimes.iter().cloned());
            self.record_active_borrow_depth();
            self.borrow_local_lifetimes
                .insert(name, BorrowLocalState::Live(lifetimes));
        }
    }

    pub(super) fn release_borrow_local(&mut self, name: &str) {
        let Some(state) = self.borrow_local_lifetimes.get(name).cloned() else {
            return;
        };
        match state {
            BorrowLocalState::Live(lifetimes) => {
                for lifetime in &lifetimes {
                    self.remove_active_borrow_lifetime(lifetime);
                }
                self.borrow_local_lifetimes
                    .insert(name.to_owned(), BorrowLocalState::Dropped);
            }
            BorrowLocalState::MaybeDropped(lifetimes) => {
                self.errors.push(TypeCheckError::new(format!(
                    "borrowed local `{name}` may already have been dropped on another control-flow path"
                )));
                for lifetime in &lifetimes {
                    self.remove_active_borrow_lifetime(lifetime);
                }
                self.borrow_local_lifetimes
                    .insert(name.to_owned(), BorrowLocalState::Dropped);
            }
            BorrowLocalState::Dropped => {
                self.errors.push(TypeCheckError::new(format!(
                    "borrowed local `{name}` was already dropped"
                )));
            }
        }
    }

    pub(super) fn clear_borrow_local(&mut self, name: &str) {
        let Some(state) = self.borrow_local_lifetimes.remove(name) else {
            return;
        };
        for lifetime in state.lifetimes() {
            self.remove_active_borrow_lifetime(lifetime);
        }
    }

    pub(super) fn remove_active_borrow_lifetime(&mut self, lifetime: &str) {
        if let Some(index) = self
            .active_borrows
            .iter()
            .position(|active| active == lifetime)
        {
            self.active_borrows.remove(index);
        }
    }

    pub(super) fn snapshot_borrow_state(&mut self) -> BorrowStateSnapshot {
        self.stats.borrow_state_snapshots += 1;
        self.stats.borrow_state_cloned_bindings += self.borrow_local_lifetimes.len();
        BorrowStateSnapshot {
            borrow_local_lifetimes: self.borrow_local_lifetimes.clone(),
        }
    }

    pub(super) fn restore_borrow_state(&mut self, snapshot: BorrowStateSnapshot) {
        self.stats.borrow_state_restores += 1;
        self.borrow_local_lifetimes = snapshot.borrow_local_lifetimes;
        self.rebuild_active_borrows();
    }

    pub(super) fn merge_borrow_state_from_paths(
        &mut self,
        base: &BorrowStateSnapshot,
        paths: &[BorrowStateSnapshot],
    ) {
        self.stats.borrow_state_merges += 1;
        let mut merged = HashMap::new();
        for (name, base_state) in &base.borrow_local_lifetimes {
            let states = paths
                .iter()
                .map(|path| path.borrow_local_lifetimes.get(name).unwrap_or(base_state));
            merged.insert(name.clone(), merge_borrow_local_states(states));
        }
        self.borrow_local_lifetimes = merged;
        self.rebuild_active_borrows();
    }

    pub(super) fn rebuild_active_borrows(&mut self) {
        self.active_borrows.clear();
        self.active_borrows.extend(
            self.borrow_local_lifetimes
                .values()
                .flat_map(BorrowLocalState::lifetimes)
                .cloned(),
        );
        self.record_active_borrow_depth();
    }
}
