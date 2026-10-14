//! Borrowed Need payloads retain the task owner's descriptor.

use super::RuntimeValueView;
use crate::task::{NeedId, RuntimeNeedHandle, RuntimeNeedHandleSaveSnapshot, TaskOutcomeContract};

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeNeedView<'a> {
    Live(&'a RuntimeNeedHandle),
    Snapshot(&'a RuntimeNeedHandleSaveSnapshot),
}

impl<'a> RuntimeNeedView<'a> {
    pub(crate) fn correlation(self) -> crate::task::TaskCorrelation {
        match self {
            Self::Live(value) => value.correlation(),
            Self::Snapshot(value) => value.correlation(),
        }
    }

    pub(crate) fn requires_producer_custody(self) -> bool {
        match self {
            Self::Live(value) => value.requires_producer_custody(),
            Self::Snapshot(value) => value.requires_producer_custody(),
        }
    }

    pub(crate) fn visit_descriptor(
        self,
        visitor: &mut impl crate::task::descriptor::TaskDescriptorVisitor,
        depth: usize,
    ) -> Result<(), crate::entry::RuntimeSchemaError> {
        match self {
            Self::Live(v) => v.visit_descriptor(visitor, depth),
            Self::Snapshot(v) => v.visit_descriptor(visitor, depth),
        }
    }
    pub(crate) fn producer(self) -> &'a crate::task::NeedProducerInstance {
        match self {
            Self::Live(v) => &v.spec().producer,
            Self::Snapshot(v) => &v.spec().producer,
        }
    }

    pub(crate) fn class(self) -> &'a crate::task::TaskClass {
        match self {
            Self::Live(v) => &v.spec().class,
            Self::Snapshot(v) => &v.spec().class,
        }
    }

    pub(crate) fn priority(self) -> crate::task::TaskPriority {
        match self {
            Self::Live(v) => v.spec().priority,
            Self::Snapshot(v) => v.spec().priority,
        }
    }

    pub(crate) fn cancel_scope(self) -> &'a crate::task::CancelScopeId {
        match self {
            Self::Live(v) => &v.spec().cancel_scope,
            Self::Snapshot(v) => &v.spec().cancel_scope,
        }
    }

    pub(crate) fn policy(self) -> crate::task::TaskPolicy {
        match self {
            Self::Live(v) => v.spec().policy,
            Self::Snapshot(v) => v.spec().policy,
        }
    }

    pub(crate) fn validate_identity(self) -> Result<(), crate::task::RuntimeNeedHandleError> {
        match self {
            Self::Live(_) => Ok(()),
            Self::Snapshot(v) => v.validate_identity(),
        }
    }

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
