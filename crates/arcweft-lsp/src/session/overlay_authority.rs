//! Live overlay authority that has not yet reached an accepted profile generation.

use std::{collections::BTreeMap, sync::Arc};

use arcweft_source::SourceDocumentIdentity;

use crate::{
    documents::rebind_overlay,
    profiles::state::{
        AcceptedEnvironmentGeneration, AcceptedProfileEnvironment, AcceptedProfileKey,
        LspProfileState,
    },
    requests::{RequestRegistry, SignatureCancellationReason},
    uri_key::LspUriKey,
};

use super::ArcweftLspSession;

/// Open-document and manifest authority that must not be paired with an older
/// accepted document or semantic world after a rebuild fails.
#[derive(Debug, Default)]
pub(super) struct PendingSignatureAuthority {
    revisions: BTreeMap<PendingSignatureScope, PendingOverlayRevision>,
}

/// Smallest logical authority scope waiting for a complete accepted generation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum PendingSignatureScope {
    Profile(AcceptedProfileKey),
    Document {
        profile: AcceptedProfileKey,
        uri: LspUriKey,
    },
}

/// Exact live overlay evidence retained only until a complete candidate accepts it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingOverlayRevision {
    uri: LspUriKey,
    version: i32,
    expected: SourceDocumentIdentity,
    actual: SourceDocumentIdentity,
    previous_generation: AcceptedEnvironmentGeneration,
}

impl PendingSignatureAuthority {
    pub(super) fn document(
        &self,
        profile: &AcceptedProfileKey,
        uri: &LspUriKey,
    ) -> Option<&PendingOverlayRevision> {
        self.revisions.get(&PendingSignatureScope::Document {
            profile: profile.clone(),
            uri: uri.clone(),
        })
    }

    pub(super) fn profile(&self, profile: &AcceptedProfileKey) -> Option<&PendingOverlayRevision> {
        self.revisions
            .get(&PendingSignatureScope::Profile(profile.clone()))
    }

    fn mark_document(&mut self, profile: AcceptedProfileKey, revision: PendingOverlayRevision) {
        self.revisions.insert(
            PendingSignatureScope::Document {
                profile,
                uri: revision.uri.clone(),
            },
            revision,
        );
    }

    fn mark_profile(&mut self, profile: AcceptedProfileKey, revision: PendingOverlayRevision) {
        self.revisions
            .insert(PendingSignatureScope::Profile(profile), revision);
    }

    /// A successful candidate consumes every pending overlay it accepted and
    /// restores profile-wide admission for its exact profile identity.
    pub(super) fn accept(&mut self, accepted: &AcceptedProfileEnvironment) {
        self.revisions.retain(|scope, revision| match scope {
            PendingSignatureScope::Profile(profile)
            | PendingSignatureScope::Document { profile, .. } => {
                profile != accepted.profile() || !revision.is_accepted_by(accepted)
            }
        });
    }

    pub(super) fn remove_document(&mut self, uri: &LspUriKey) {
        self.revisions.retain(|scope, _| {
            !matches!(
                scope,
                PendingSignatureScope::Document {
                    uri: pending_uri,
                    ..
                } if pending_uri == uri
            )
        });
    }

    pub(super) fn remove_profile(&mut self, profile: &AcceptedProfileKey) {
        self.revisions.retain(|scope, _| match scope {
            PendingSignatureScope::Profile(pending)
            | PendingSignatureScope::Document {
                profile: pending, ..
            } => pending != profile,
        });
    }

    pub(super) fn clear(&mut self) {
        self.revisions.clear();
    }
}

impl PendingOverlayRevision {
    fn new(
        uri: LspUriKey,
        version: i32,
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
        previous_generation: AcceptedEnvironmentGeneration,
    ) -> Self {
        Self {
            uri,
            version,
            expected,
            actual,
            previous_generation,
        }
    }

    pub(super) const fn uri(&self) -> &LspUriKey {
        &self.uri
    }

    pub(super) const fn version(&self) -> i32 {
        self.version
    }

    pub(super) const fn expected(&self) -> &SourceDocumentIdentity {
        &self.expected
    }

    pub(super) const fn actual(&self) -> &SourceDocumentIdentity {
        &self.actual
    }

    pub(super) const fn previous_generation(&self) -> AcceptedEnvironmentGeneration {
        self.previous_generation
    }

    pub(super) fn bytes_changed(&self) -> bool {
        self.expected != self.actual
    }

    fn is_accepted_by(&self, accepted: &AcceptedProfileEnvironment) -> bool {
        accepted.generation() > self.previous_generation
            && accepted.overlays().get(&self.uri).is_some_and(|overlay| {
                overlay.version() == self.version && overlay.logical_identity() == &self.actual
            })
    }
}

impl ArcweftLspSession {
    /// Reuse the already accepted profile state when a newly opened URI is
    /// already part of that accepted project. This prevents a disk-only
    /// generation from publishing between didOpen and overlay acceptance.
    pub(super) fn attach_open_uri_to_accepted_profile(&mut self, uri: &lsp_types::Uri) -> bool {
        let key = LspUriKey::from_uri(uri);
        let profile = self.profiles_by_uri.get(&key).cloned().or_else(|| {
            let mut candidates = Vec::new();
            for profile in self.profiles_by_uri.values().filter(|profile| {
                profile
                    .accepted_environment()
                    .is_some_and(|accepted| accepted.project().sources().by_uri(uri).is_some())
            }) {
                if candidates
                    .iter()
                    .all(|candidate: &&crate::profiles::LspProfile| {
                        !Arc::ptr_eq(candidate.state(), profile.state())
                    })
                {
                    candidates.push(profile);
                }
            }
            (candidates.len() == 1).then(|| candidates[0].clone())
        });
        let Some(profile) = profile else {
            return false;
        };
        let Some(accepted) = profile.accepted_environment() else {
            return false;
        };
        self.profile_keys_by_uri
            .insert(key.clone(), accepted.profile().clone());
        self.profiles_by_uri.insert(key, profile);
        true
    }

    /// Mark the smallest authority scope affected by live bytes before any
    /// metadata-only publication or full rebuild is attempted.
    pub(super) fn mark_signature_authority_pending(
        &mut self,
        uri: &lsp_types::Uri,
        requests: &RequestRegistry,
    ) {
        let uri = LspUriKey::from_uri(uri);
        let Some(snapshot) = self.documents.get_by_key(&uri) else {
            return;
        };
        let mut affected = Vec::<(Arc<LspProfileState>, Arc<AcceptedProfileEnvironment>)>::new();
        for profile in self.profiles_by_uri.values() {
            let Some(accepted) = profile.accepted_environment() else {
                continue;
            };
            if accepted.project().sources().by_uri(&uri.to_uri()).is_none()
                || affected
                    .iter()
                    .any(|(state, _)| Arc::ptr_eq(state, profile.state()))
            {
                continue;
            }
            affected.push((Arc::clone(profile.state()), accepted));
        }

        for (_, accepted) in affected {
            let Some(accepted_source) = accepted.project().sources().by_uri(&uri.to_uri()) else {
                continue;
            };
            let actual = rebind_overlay(snapshot, accepted_source).map_or_else(
                |_| snapshot.source_document().identity().clone(),
                |document| document.identity().clone(),
            );
            let revision = PendingOverlayRevision::new(
                uri.clone(),
                snapshot.version(),
                accepted_source.document().identity().clone(),
                actual,
                accepted.generation(),
            );
            if accepted.profile().manifest_key() == &uri {
                self.pending_signature_authority
                    .mark_profile(accepted.profile().clone(), revision);
                requests.cancel_accepted(&accepted, SignatureCancellationReason::DocumentChanged);
            } else {
                self.pending_signature_authority
                    .mark_document(accepted.profile().clone(), revision);
            }
        }
    }

    pub(super) fn apply_registered_topology_to_profile_state(
        &mut self,
        state: &Arc<LspProfileState>,
        topology: &arcweft_project_loader::topology::LoadedProfileTopology,
        characters: &arcweft_character::catalog::CharacterCatalog,
    ) {
        for profile in self
            .profiles_by_uri
            .values_mut()
            .filter(|profile| Arc::ptr_eq(profile.state(), state))
        {
            crate::profiles::apply_registered_topology(profile, topology, characters.clone());
        }
    }
}
