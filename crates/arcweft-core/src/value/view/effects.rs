//! Borrowed effect and callable values; inspection never activates a carrier.

use super::super::{
    RuntimeCallableValue, RuntimeCommand, RuntimeOpaquePersistence, RuntimeOpaqueValue,
    RuntimeOpaqueValueClass, RuntimeReductionValue,
    awbc_save::{
        AwbcRuntimeCallableSnapshot, AwbcRuntimeCommandSnapshot, AwbcRuntimeOpaqueSnapshot,
        AwbcRuntimeReductionSnapshot,
    },
};
use super::{RuntimeTupleView, RuntimeValueSource, RuntimeValueView};
use crate::{
    entry::{RuntimeCommandConstructorId, RuntimeCommandTargetId},
    pattern::{RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId},
    runtime_id::RuntimeCallableStateId,
    task::RuntimeProgramOwner,
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeOpaqueView<'a> {
    Live(&'a RuntimeOpaqueValue),
    Snapshot(&'a AwbcRuntimeOpaqueSnapshot),
}

impl<'a> RuntimeOpaqueView<'a> {
    pub(crate) fn producer(self) -> &'a RuntimeOpaqueTypeProducerId {
        match self {
            Self::Live(v) => v.producer(),
            Self::Snapshot(v) => &v.producer,
        }
    }
    pub(crate) fn semantic_identity(self) -> RuntimeSemanticTypeId {
        match self {
            Self::Live(v) => v.semantic_identity(),
            Self::Snapshot(v) => v.semantic_identity,
        }
    }
    pub(crate) fn value_class(self) -> RuntimeOpaqueValueClass {
        match self {
            Self::Live(v) => v.value_class(),
            Self::Snapshot(v) => v.value_class,
        }
    }
    pub(crate) fn persistence(self) -> RuntimeOpaquePersistence {
        match self {
            Self::Live(v) => v.persistence(),
            Self::Snapshot(v) => v.persistence,
        }
    }
    pub(crate) fn payload(self) -> RuntimeValueView<'a> {
        match self {
            Self::Live(v) => v.payload().view(),
            Self::Snapshot(v) => v.payload.view(),
        }
    }
    pub(super) const fn live_value(self) -> Option<&'a RuntimeOpaqueValue> {
        match self {
            Self::Live(v) => Some(v),
            Self::Snapshot(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeReductionView<'a> {
    Live(&'a RuntimeReductionValue),
    Snapshot(&'a AwbcRuntimeReductionSnapshot),
}

impl<'a> RuntimeReductionView<'a> {
    pub(crate) fn owner(self) -> &'a RuntimeOpaqueTypeOwner {
        match self {
            Self::Live(v) => v.owner(),
            Self::Snapshot(v) => &v.owner,
        }
    }
    pub(crate) fn state(self) -> RuntimeValueView<'a> {
        match self {
            Self::Live(v) => v.state().view(),
            Self::Snapshot(v) => v.state.view(),
        }
    }
    pub(crate) fn commands(self) -> RuntimeCommandListView<'a> {
        match self {
            Self::Live(v) => RuntimeCommandListView::Live(v.commands()),
            Self::Snapshot(v) => RuntimeCommandListView::Snapshot(&v.commands),
        }
    }
    pub(super) const fn live_value(self) -> Option<&'a RuntimeReductionValue> {
        match self {
            Self::Live(v) => Some(v),
            Self::Snapshot(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeCommandListView<'a> {
    Live(&'a [RuntimeCommand]),
    Snapshot(&'a [AwbcRuntimeCommandSnapshot]),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeCommandView<'a> {
    constructor: &'a RuntimeCommandConstructorId,
    target: &'a RuntimeCommandTargetId,
    payload: RuntimeValueSource<'a>,
}

impl<'a> RuntimeCommandListView<'a> {
    pub(crate) fn len(self) -> usize {
        match self {
            Self::Live(v) => v.len(),
            Self::Snapshot(v) => v.len(),
        }
    }
    pub(crate) fn get(self, index: usize) -> Option<RuntimeCommandView<'a>> {
        Some(match self {
            Self::Live(values) => {
                let v = values.get(index)?;
                RuntimeCommandView {
                    constructor: v.constructor(),
                    target: v.target(),
                    payload: RuntimeValueSource::Live(v.payload().value()),
                }
            }
            Self::Snapshot(values) => {
                let v = values.get(index)?;
                RuntimeCommandView {
                    constructor: &v.constructor,
                    target: &v.target,
                    payload: RuntimeValueSource::Snapshot(&v.payload),
                }
            }
        })
    }
    pub(crate) fn iter(self) -> impl ExactSizeIterator<Item = RuntimeCommandView<'a>> {
        (0..self.len())
            .map(move |index| self.get(index).expect("command list owns each indexed row"))
    }
}

impl<'a> RuntimeCommandView<'a> {
    pub(crate) const fn constructor(self) -> &'a RuntimeCommandConstructorId {
        self.constructor
    }
    pub(crate) const fn target(self) -> &'a RuntimeCommandTargetId {
        self.target
    }
    pub(crate) fn payload(self) -> RuntimeValueView<'a> {
        self.payload.view()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeCallableView<'a> {
    Live(&'a RuntimeCallableValue),
    Snapshot(&'a AwbcRuntimeCallableSnapshot),
}

impl<'a> RuntimeCallableView<'a> {
    pub(crate) fn state(self) -> RuntimeCallableStateId {
        match self {
            Self::Live(v) => v.state(),
            Self::Snapshot(v) => v.state,
        }
    }
    pub(crate) fn owner(self) -> Option<&'a RuntimeProgramOwner> {
        match self {
            Self::Live(v) => Some(v.owner()),
            Self::Snapshot(_) => None,
        }
    }
    pub(crate) fn retained(self) -> RuntimeTupleView<'a> {
        match self {
            Self::Live(v) => RuntimeTupleView::Values(v.retained()),
            Self::Snapshot(v) => RuntimeTupleView::Snapshots(&v.retained),
        }
    }
    pub(super) const fn live_value(self) -> Option<&'a RuntimeCallableValue> {
        match self {
            Self::Live(v) => Some(v),
            Self::Snapshot(_) => None,
        }
    }
}
