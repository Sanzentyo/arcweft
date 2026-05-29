//! Borrow-state facts used by the semantic type checker.

use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Debug)]
pub(crate) struct BorrowStateSnapshot {
    pub(crate) borrow_local_lifetimes: HashMap<String, BorrowLocalState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BorrowLocalState {
    Live(Vec<String>),
    Dropped,
    MaybeDropped(Vec<String>),
}

impl BorrowLocalState {
    pub(crate) fn lifetimes(&self) -> &[String] {
        match self {
            Self::Live(lifetimes) | Self::MaybeDropped(lifetimes) => lifetimes,
            Self::Dropped => &[],
        }
    }
}

pub(crate) fn merge_borrow_local_states(states: &[&BorrowLocalState]) -> BorrowLocalState {
    let has_live = states
        .iter()
        .any(|state| matches!(state, BorrowLocalState::Live(_)));
    let has_dropped = states
        .iter()
        .any(|state| matches!(state, BorrowLocalState::Dropped));
    let has_maybe = states
        .iter()
        .any(|state| matches!(state, BorrowLocalState::MaybeDropped(_)));
    let lifetimes = states
        .iter()
        .flat_map(|state| state.lifetimes())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    match (has_live, has_dropped, has_maybe) {
        (false, true, false) => BorrowLocalState::Dropped,
        (true, false, false) => BorrowLocalState::Live(lifetimes),
        _ => BorrowLocalState::MaybeDropped(lifetimes),
    }
}
