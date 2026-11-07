//! Registration identities and outcomes for executable scope cleanup.

use crate::runtime_id::RuntimeDeferRegistrationId;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub enum ScopeExit {
    #[default]
    Completed,
    Cancelled,
    Failed,
}

/// Exit condition that admits one deferred callback.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeDeferOutcomeFilter {
    Always,
    Completed,
    Cancelled,
    Failed,
}

impl RuntimeDeferOutcomeFilter {
    #[must_use]
    pub const fn matches(self, exit: ScopeExit) -> bool {
        matches!(
            (self, exit),
            (Self::Always, _)
                | (Self::Completed, ScopeExit::Completed)
                | (Self::Cancelled, ScopeExit::Cancelled)
                | (Self::Failed, ScopeExit::Failed)
        )
    }
}

/// The first unissued registration in one runtime owner. The final ordinal
/// remains the exhausted frontier, so every issued ID compares below it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub(crate) struct RuntimeDeferRegistrationFrontier(NonZeroU64);

#[derive(Debug)]
pub(crate) struct RuntimePreparedDeferredRegistration {
    id: RuntimeDeferRegistrationId,
    next: NonZeroU64,
}

impl RuntimeDeferRegistrationFrontier {
    pub(crate) const fn initial() -> Self {
        Self(NonZeroU64::MIN)
    }

    pub(crate) fn prepare(self) -> Option<RuntimePreparedDeferredRegistration> {
        let next = NonZeroU64::new(self.0.get().checked_add(1)?)?;
        Some(RuntimePreparedDeferredRegistration {
            id: RuntimeDeferRegistrationId::from_allocated(self.0),
            next,
        })
    }

    pub(crate) fn commit(
        &mut self,
        prepared: RuntimePreparedDeferredRegistration,
    ) -> RuntimeDeferRegistrationId {
        assert_eq!(
            self.0,
            prepared.id.get(),
            "defer registration proof must match its owner's current frontier"
        );
        self.0 = prepared.next;
        prepared.id
    }

    pub(crate) fn contains(self, id: RuntimeDeferRegistrationId) -> bool {
        id.get() < self.0
    }
}

impl RuntimePreparedDeferredRegistration {
    pub(crate) const fn id(&self) -> RuntimeDeferRegistrationId {
        self.id
    }
}

/// Pending packets are pushed in allocation order. Any running or retiring
/// packet was popped from that stack and must be newer than the pending tail.
pub(crate) fn defer_registration_stack_is_ordered(
    pending: impl IntoIterator<Item = RuntimeDeferRegistrationId>,
    started: Option<RuntimeDeferRegistrationId>,
) -> bool {
    pending
        .into_iter()
        .chain(started)
        .try_fold(None, |previous, id| {
            if previous.is_some_and(|previous| previous >= id) {
                return Err(());
            }
            Ok(Some(id))
        })
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_frontier_exhausts_without_reissuing_or_wrapping() {
        let mut frontier = RuntimeDeferRegistrationFrontier(NonZeroU64::new(u64::MAX - 1).unwrap());
        let last = frontier.prepare().expect("last allocatable registration");
        let id = last.id();
        assert!(!frontier.contains(id));
        assert_eq!(frontier.commit(last), id);
        assert!(frontier.contains(id));
        assert!(frontier.prepare().is_none());
        assert!(!frontier.contains(RuntimeDeferRegistrationId::from_allocated(NonZeroU64::MAX)));
        assert!(serde_json::from_str::<RuntimeDeferRegistrationFrontier>("0").is_err());
    }
}
