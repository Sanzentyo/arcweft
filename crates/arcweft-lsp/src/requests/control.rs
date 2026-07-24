//! The single cancellation and publication gate for one admitted signature request.

use std::{
    sync::{
        Arc, Mutex, MutexGuard, PoisonError, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use arcweft_source::SourceDocumentIdentity;

use crate::{
    profiles::state::{AcceptedProfileEnvironment, LspProfileState},
    uri_key::LspUriKey,
};

/// Weak lifecycle binding used to select in-flight requests without retaining a generation.
#[derive(Debug)]
pub(crate) struct SignatureRequestBinding {
    uri: LspUriKey,
    workspace: LspUriKey,
    profile_state: Weak<LspProfileState>,
    accepted: Weak<AcceptedProfileEnvironment>,
    document: SourceDocumentIdentity,
}

/// The sole cancellation flag and result-publication linearization gate.
#[derive(Debug)]
pub(crate) struct RequestControl {
    cancelled: AtomicBool,
    deadline: Instant,
    #[cfg(test)]
    deadline_override: Mutex<Option<Instant>>,
    binding: SignatureRequestBinding,
    gate: Mutex<RequestGateState>,
}

/// Terminal reason assigned by the first lifecycle or client cancellation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SignatureCancellationReason {
    ClientCancelled,
    DeadlineExceeded,
    DocumentChanged,
    DocumentClosed,
    ProfileRemapped,
    ProfileClosing,
    WorkspaceRemoved,
    AcceptedReplaced,
    SessionShutdown,
}

/// Publication state protected by one request's gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RequestGateState {
    Active,
    Cancelled(SignatureCancellationReason),
    Finished,
}

impl SignatureRequestBinding {
    pub(crate) fn new(
        uri: LspUriKey,
        workspace: LspUriKey,
        profile_state: &Arc<LspProfileState>,
        accepted: &Arc<AcceptedProfileEnvironment>,
        document: SourceDocumentIdentity,
    ) -> Self {
        Self {
            uri,
            workspace,
            profile_state: Arc::downgrade(profile_state),
            accepted: Arc::downgrade(accepted),
            document,
        }
    }

    pub(crate) const fn uri(&self) -> &LspUriKey {
        &self.uri
    }

    pub(crate) const fn workspace(&self) -> &LspUriKey {
        &self.workspace
    }

    pub(crate) const fn document(&self) -> &SourceDocumentIdentity {
        &self.document
    }

    pub(crate) fn is_profile_state(&self, state: &Arc<LspProfileState>) -> bool {
        Weak::ptr_eq(&self.profile_state, &Arc::downgrade(state))
    }

    pub(crate) fn profile_state(&self) -> Option<Arc<LspProfileState>> {
        self.profile_state.upgrade()
    }

    pub(crate) fn is_accepted(&self, accepted: &Arc<AcceptedProfileEnvironment>) -> bool {
        Weak::ptr_eq(&self.accepted, &Arc::downgrade(accepted))
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        uri: LspUriKey,
        workspace: LspUriKey,
        profile_state: &Arc<LspProfileState>,
        document: SourceDocumentIdentity,
    ) -> Self {
        Self {
            uri,
            workspace,
            profile_state: Arc::downgrade(profile_state),
            accepted: Weak::new(),
            document,
        }
    }
}

impl RequestControl {
    pub(crate) fn new(deadline: Instant, binding: SignatureRequestBinding) -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            deadline,
            #[cfg(test)]
            deadline_override: Mutex::new(None),
            binding,
            gate: Mutex::new(RequestGateState::Active),
        }
    }

    pub(crate) const fn cancellation_flag(&self) -> &AtomicBool {
        &self.cancelled
    }

    #[cfg(not(test))]
    pub(crate) const fn deadline(&self) -> Instant {
        self.deadline
    }

    #[cfg(test)]
    pub(crate) fn deadline(&self) -> Instant {
        self.deadline_override
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .unwrap_or(self.deadline)
    }

    pub(super) const fn scheduled_deadline(&self) -> Instant {
        self.deadline
    }

    #[cfg(test)]
    pub(crate) fn expire_deadline_for_test(&self) {
        self.deadline_override
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(Instant::now());
    }

    pub(crate) const fn binding(&self) -> &SignatureRequestBinding {
        &self.binding
    }

    pub(crate) fn gate(&self) -> MutexGuard<'_, RequestGateState> {
        self.gate.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn cancel(&self, reason: SignatureCancellationReason) {
        let mut gate = self.gate();
        if *gate == RequestGateState::Active {
            self.cancelled.store(true, Ordering::Release);
            *gate = RequestGateState::Cancelled(reason);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use lsp_types::Uri;
    use std::time::Duration;

    fn control() -> RequestControl {
        let uri = "file:///workspace/control.arcw"
            .parse::<Uri>()
            .expect("URI");
        let workspace = "file:///workspace".parse::<Uri>().expect("workspace URI");
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new(uri.to_string()).expect("document ID"),
            SourceName::path(uri.to_string()),
            "flow @flow.control control {}",
        )
        .expect("source document");
        let state = Arc::new(LspProfileState::new());
        RequestControl::new(
            Instant::now() + Duration::from_mins(1),
            SignatureRequestBinding::for_test(
                LspUriKey::from_uri(&uri),
                LspUriKey::from_uri(&workspace),
                &state,
                document.identity().clone(),
            ),
        )
    }

    #[test]
    fn cancellation_winning_the_gate_prevents_late_publication() {
        let control = control();
        control.cancel(SignatureCancellationReason::ClientCancelled);

        let mut gate = control.gate();
        let published = if *gate == RequestGateState::Active {
            *gate = RequestGateState::Finished;
            true
        } else {
            false
        };

        assert!(!published);
        assert_eq!(
            *gate,
            RequestGateState::Cancelled(SignatureCancellationReason::ClientCancelled)
        );
        assert!(control.cancelled.load(Ordering::Acquire));
    }

    #[test]
    fn publication_winning_the_gate_makes_late_cancellation_inert() {
        let control = control();
        *control.gate() = RequestGateState::Finished;

        control.cancel(SignatureCancellationReason::ClientCancelled);

        assert_eq!(*control.gate(), RequestGateState::Finished);
        assert!(!control.cancelled.load(Ordering::Acquire));
    }
}
