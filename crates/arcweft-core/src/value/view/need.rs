//! Borrowed Need payloads retain the task owner's descriptor.

use super::RuntimeValueView;
use crate::task::{NeedId, RuntimeNeedHandle, RuntimeNeedHandleSaveSnapshot, TaskOutcomeContract};

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeNeedView<'a> {
    Live(&'a RuntimeNeedHandle),
    Snapshot(&'a RuntimeNeedHandleSaveSnapshot),
}

impl<'a> RuntimeNeedView<'a> {
    pub(crate) fn need_id(self) -> NeedId {
        match self {
            Self::Live(v) => v.need_id(),
            Self::Snapshot(v) => v.correlation().need,
        }
    }
    pub(crate) fn outcome(self) -> &'a TaskOutcomeContract {
        match self {
            Self::Live(v) => v.outcome(),
            Self::Snapshot(v) => v.outcome(),
        }
    }
    pub(crate) fn request_values(self) -> Box<dyn Iterator<Item = RuntimeValueView<'a>> + 'a> {
        match self {
            Self::Live(v) => Box::new(v.request_values().map(crate::value::RuntimeValue::view)),
            Self::Snapshot(v) => Box::new(
                v.request_values()
                    .map(crate::value::AwbcRuntimeValueSnapshot::view),
            ),
        }
    }
    pub(super) const fn live_value(self) -> Option<&'a RuntimeNeedHandle> {
        match self {
            Self::Live(v) => Some(v),
            Self::Snapshot(_) => None,
        }
    }
}
