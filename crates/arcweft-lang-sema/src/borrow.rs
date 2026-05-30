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

pub(crate) fn merge_borrow_local_states<'a>(
    states: impl IntoIterator<Item = &'a BorrowLocalState>,
) -> BorrowLocalState {
    let mut has_live = false;
    let mut has_dropped = false;
    let mut has_maybe = false;
    let mut lifetimes = BTreeSet::new();
    for state in states {
        match state {
            BorrowLocalState::Live(_) => has_live = true,
            BorrowLocalState::Dropped => has_dropped = true,
            BorrowLocalState::MaybeDropped(_) => has_maybe = true,
        }
        lifetimes.extend(state.lifetimes().iter().cloned());
    }
    let lifetimes = lifetimes.into_iter().collect::<Vec<_>>();

    match (has_live, has_dropped, has_maybe) {
        (false, true, false) => BorrowLocalState::Dropped,
        (true, false, false) => BorrowLocalState::Live(lifetimes),
        _ => BorrowLocalState::MaybeDropped(lifetimes),
    }
}
