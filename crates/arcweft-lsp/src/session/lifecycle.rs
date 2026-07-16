//! Direct profile/workspace lifecycle invalidation under the session write authority.

use std::sync::Arc;

use thiserror::Error;

use crate::{
    documents::rebind_overlay,
    profiles::state::{
        AcceptedEnvironmentReplaceError, AcceptedProfileCandidate, AcceptedProfileEnvironment,
        LspProfileState, ProfileEnvironmentLifecycle,
    },
    requests::{RequestRegistry, SignatureCancellationReason},
    uri_key::LspUriKey,
};

use super::ArcweftLspSession;

/// A lifecycle publication precondition changed before mutation could linearize.
#[derive(Debug, Error)]
pub(crate) enum AcceptedPublicationError {
    #[error("session publication admission is closed")]
    SessionClosing,
    #[error("URI no longer maps to the expected profile state")]
    ProfileStateReplaced,
    #[error("profile publication admission is closed")]
    ProfileClosing,
    #[error("expected accepted environment is no longer current")]
    AcceptedReplaced,
    #[error("candidate profile key differs from mapped profile")]
    ProfileKeyMismatch,
    #[error("candidate overlays differ from current open profile overlays")]
    OverlayCoverageMismatch {
        missing: Box<[LspUriKey]>,
        extra: Box<[LspUriKey]>,
        mismatched: Box<[LspUriKey]>,
    },
    #[error("accepted environment generation overflowed")]
    GenerationOverflow,
}

impl ArcweftLspSession {
    pub(crate) fn publish_accepted_candidate(
        &mut self,
        state: &Arc<LspProfileState>,
        expected: &Arc<AcceptedProfileEnvironment>,
        candidate: AcceptedProfileCandidate,
        requests: Option<&RequestRegistry>,
    ) -> Result<Arc<AcceptedProfileEnvironment>, AcceptedPublicationError> {
        if !self.signature_admission_open {
            return Err(AcceptedPublicationError::SessionClosing);
        }
        if !self
            .profiles_by_uri
            .values()
            .any(|profile| Arc::ptr_eq(profile.state(), state))
        {
            return Err(AcceptedPublicationError::ProfileStateReplaced);
        }
        if state.lifecycle() != ProfileEnvironmentLifecycle::Active {
            return Err(AcceptedPublicationError::ProfileClosing);
        }
        if candidate.profile() != expected.profile() {
            return Err(AcceptedPublicationError::ProfileKeyMismatch);
        }

        let mut missing = Vec::new();
        let mut mismatched = Vec::new();
        let mut expected_uris = std::collections::BTreeSet::new();
        for snapshot in self.documents.snapshots() {
            let Some(accepted_source) = candidate.project().sources().by_uri(snapshot.uri()) else {
                continue;
            };
            let uri = LspUriKey::from_uri(snapshot.uri());
            expected_uris.insert(uri.clone());
            let Some(overlay) = candidate.overlays().get(&uri) else {
                missing.push(uri);
                continue;
            };
            let identity_matches = rebind_overlay(snapshot, accepted_source)
                .is_ok_and(|document| document.identity() == overlay.logical_identity());
            if overlay.version() != snapshot.version() || !identity_matches {
                mismatched.push(uri);
            }
        }
        let extra = candidate
            .overlays()
            .iter()
            .filter(|(uri, _)| !expected_uris.contains(*uri))
            .map(|(uri, _)| uri.clone())
            .collect::<Vec<_>>();
        if !missing.is_empty() || !extra.is_empty() || !mismatched.is_empty() {
            return Err(AcceptedPublicationError::OverlayCoverageMismatch {
                missing: missing.into_boxed_slice(),
                extra: extra.into_boxed_slice(),
                mismatched: mismatched.into_boxed_slice(),
            });
        }

        state
            .replace_accepted_with(Some(expected), candidate, |current| {
                if let Some(current) = current {
                    if let Some(requests) = requests {
                        requests.cancel_accepted(
                            current,
                            SignatureCancellationReason::AcceptedReplaced,
                        );
                    }
                    current.clear_caches();
                }
            })
            .map_err(|error| match error {
                AcceptedEnvironmentReplaceError::ShuttingDown => {
                    AcceptedPublicationError::ProfileClosing
                }
                AcceptedEnvironmentReplaceError::CurrentChanged => {
                    AcceptedPublicationError::AcceptedReplaced
                }
                AcceptedEnvironmentReplaceError::GenerationOverflow => {
                    AcceptedPublicationError::GenerationOverflow
                }
            })
    }

    pub(crate) fn remove_workspace(&mut self, workspace: &LspUriKey, requests: &RequestRegistry) {
        requests.cancel_workspace(workspace, SignatureCancellationReason::WorkspaceRemoved);
        let removed = self
            .profile_keys_by_uri
            .iter()
            .filter(|(_, profile)| profile.workspace_key() == workspace)
            .map(|(uri, _)| uri.clone())
            .collect::<Vec<_>>();
        let mut states = Vec::<Arc<LspProfileState>>::new();
        for uri in removed {
            if let Some(profile) = self.profiles_by_uri.remove(&uri) {
                let state = Arc::clone(profile.state());
                if states.iter().all(|current| !Arc::ptr_eq(current, &state)) {
                    states.push(state);
                }
            }
            self.profile_keys_by_uri.remove(&uri);
            self.analyses_by_uri.remove(&uri);
            self.documents.remove_by_key(&uri);
        }
        for state in states {
            if let Some(accepted) = state.current() {
                accepted.clear_caches();
            }
            state.shutdown();
        }
    }

    pub(crate) fn record_failed_replacement(
        &self,
        state: &Arc<LspProfileState>,
        expected: &Arc<AcceptedProfileEnvironment>,
    ) -> Result<(), AcceptedPublicationError> {
        if !self.signature_admission_open {
            return Err(AcceptedPublicationError::SessionClosing);
        }
        if !self
            .profiles_by_uri
            .values()
            .any(|profile| Arc::ptr_eq(profile.state(), state))
        {
            return Err(AcceptedPublicationError::ProfileStateReplaced);
        }
        if state.lifecycle() != ProfileEnvironmentLifecycle::Active {
            return Err(AcceptedPublicationError::ProfileClosing);
        }
        let accepted = state.accepted_read();
        if accepted
            .as_ref()
            .is_none_or(|current| !Arc::ptr_eq(current, expected))
        {
            return Err(AcceptedPublicationError::AcceptedReplaced);
        }
        Ok(())
    }
}
