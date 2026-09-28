use super::ScopeExit;
use crate::runtime_id::{RuntimeDeferRegistrationId, RuntimeDeferSiteId};
use crate::value::RuntimeValue;
use serde::{Deserialize, Serialize};

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

/// One dynamic defer registration retained by its dialogue activation.
///
/// The site identifies the executable body, while repeated registrations of
/// the same site remain distinct stack entries with their own captured values.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeLineDeferredRegistration {
    id: RuntimeDeferRegistrationId,
    site: RuntimeDeferSiteId,
    outcome_filter: RuntimeDeferOutcomeFilter,
    captures: Vec<RuntimeValue>,
}

impl RuntimeLineDeferredRegistration {
    #[must_use]
    pub(crate) fn new(
        id: RuntimeDeferRegistrationId,
        site: RuntimeDeferSiteId,
        outcome_filter: RuntimeDeferOutcomeFilter,
        captures: Vec<RuntimeValue>,
    ) -> Self {
        Self {
            id,
            site,
            outcome_filter,
            captures,
        }
    }

    #[must_use]
    pub const fn id(&self) -> RuntimeDeferRegistrationId {
        self.id
    }

    #[must_use]
    pub const fn site(&self) -> RuntimeDeferSiteId {
        self.site
    }

    #[must_use]
    pub const fn outcome_filter(&self) -> RuntimeDeferOutcomeFilter {
        self.outcome_filter
    }

    #[must_use]
    pub fn captures(&self) -> &[RuntimeValue] {
        &self.captures
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        RuntimeDeferRegistrationId,
        RuntimeDeferSiteId,
        RuntimeDeferOutcomeFilter,
        Vec<RuntimeValue>,
    ) {
        (self.id, self.site, self.outcome_filter, self.captures)
    }
}

/// One executor-owned child currently running a line-root defer body.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeDeferInFlight {
    pub(crate) id: RuntimeDeferRegistrationId,
    pub(crate) site: RuntimeDeferSiteId,
}

/// The fixed exit reason and at most one active deferred body for one line.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeDeferUnwindState {
    pub(crate) exit: ScopeExit,
    pub(crate) inflight: Option<RuntimeDeferInFlight>,
}

/// One atomic LIFO transition selected by the shared activation.
pub(crate) enum RuntimeDeferUnwindStep {
    Run(RuntimeLineDeferredRegistration),
    Skipped(RuntimeDeferRegistrationId),
}

/// A lexical defer keeps its capture packet in the owning frame until the
/// shared line ledger has accepted the transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeScopedDeferDecision {
    Run,
    Skipped,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AwbcRuntimeDeferredRegistrationSnapshot {
    id: RuntimeDeferRegistrationId,
    site: RuntimeDeferSiteId,
    outcome_filter: RuntimeDeferOutcomeFilter,
    captures: Vec<crate::value::AwbcRuntimeValueSnapshot>,
}

impl AwbcRuntimeDeferredRegistrationSnapshot {
    pub(crate) fn from_live(
        registration: &RuntimeLineDeferredRegistration,
    ) -> Result<Self, crate::value::AwbcRuntimeValueSnapshotError> {
        Self::from_live_with_owner(registration, None)
    }

    pub(crate) fn from_live_for_program(
        registration: &RuntimeLineDeferredRegistration,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, crate::value::AwbcRuntimeValueSnapshotError> {
        Self::from_live_with_owner(registration, Some(owner))
    }

    fn from_live_with_owner(
        registration: &RuntimeLineDeferredRegistration,
        owner: Option<&crate::task::RuntimeProgramOwner>,
    ) -> Result<Self, crate::value::AwbcRuntimeValueSnapshotError> {
        Ok(Self {
            id: registration.id,
            site: registration.site,
            outcome_filter: registration.outcome_filter,
            captures: registration
                .captures
                .iter()
                .map(|value| match owner {
                    Some(owner) => {
                        crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
                            value, owner,
                        )
                    }
                    None => crate::value::AwbcRuntimeValueSnapshot::from_runtime_value(value),
                })
                .collect::<Result<_, _>>()?,
        })
    }

    pub(crate) fn into_live(
        self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<RuntimeLineDeferredRegistration, crate::value::AwbcRuntimeValueSnapshotError> {
        Ok(RuntimeLineDeferredRegistration::new(
            self.id,
            self.site,
            self.outcome_filter,
            self.captures
                .into_iter()
                .map(|capture| capture.into_runtime_value_for_program(owner))
                .collect::<Result<_, _>>()?,
        ))
    }
}
