//! Remaining iterator storage inspected without constructing a live iterator.

use super::super::{RuntimeIterator, RuntimeRangeIterator, awbc_save::AwbcRuntimeIteratorSnapshot};
use super::{RuntimeTupleView, RuntimeValueSource, RuntimeValueView};
use crate::{plan::RuntimeTraitMethodId, task::RuntimeProgramOwner};

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeIteratorView<'a> {
    Live(&'a RuntimeIterator),
    Snapshot(&'a AwbcRuntimeIteratorSnapshot),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeIteratorKind<'a> {
    Values(RuntimeTupleView<'a>),
    Range(&'a RuntimeRangeIterator),
    Witness(RuntimeIteratorWitnessView<'a>),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeIteratorWitnessView<'a> {
    owner: Option<&'a RuntimeProgramOwner>,
    state: RuntimeValueSource<'a>,
    next: RuntimeTraitMethodId,
}

impl<'a> RuntimeIteratorView<'a> {
    pub(crate) fn kind(self) -> RuntimeIteratorKind<'a> {
        match self {
            Self::Live(RuntimeIterator::Values { items }) => {
                RuntimeIteratorKind::Values(RuntimeTupleView::Deque(items))
            }
            Self::Live(RuntimeIterator::Range(range)) => RuntimeIteratorKind::Range(range),
            Self::Live(RuntimeIterator::Witness(v)) => {
                RuntimeIteratorKind::Witness(RuntimeIteratorWitnessView {
                    owner: Some(v.owner()),
                    state: RuntimeValueSource::Live(v.state()),
                    next: v.next_method(),
                })
            }
            Self::Snapshot(AwbcRuntimeIteratorSnapshot::Values { items }) => {
                RuntimeIteratorKind::Values(RuntimeTupleView::Snapshots(items))
            }
            Self::Snapshot(AwbcRuntimeIteratorSnapshot::Range(range)) => {
                RuntimeIteratorKind::Range(range)
            }
            Self::Snapshot(AwbcRuntimeIteratorSnapshot::Witness { state, next }) => {
                RuntimeIteratorKind::Witness(RuntimeIteratorWitnessView {
                    owner: None,
                    state: RuntimeValueSource::Snapshot(state),
                    next: *next,
                })
            }
        }
    }
    pub(super) const fn live_value(self) -> Option<&'a RuntimeIterator> {
        match self {
            Self::Live(v) => Some(v),
            Self::Snapshot(_) => None,
        }
    }
}

impl<'a> RuntimeIteratorWitnessView<'a> {
    pub(crate) const fn owner(self) -> Option<&'a RuntimeProgramOwner> {
        self.owner
    }
    pub(crate) fn state(self) -> RuntimeValueView<'a> {
        self.state.view()
    }
    pub(crate) const fn next_method(self) -> RuntimeTraitMethodId {
        self.next
    }
}
