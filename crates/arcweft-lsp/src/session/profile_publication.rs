//! Profile construction and accepted-environment publication under session authority.

use std::sync::Arc;

use lsp_server::Notification;

use crate::{
    documents::rebind_overlay,
    profiles::{
        LspProfile, LspProfileDiagnostic, LspProfileDiagnosticKind, ProfileRegistrationOverlay,
        file_path_from_uri, register_profile_environment_with_overlays,
        state::{
            AcceptedOverlayEntry, AcceptedOverlaySet, AcceptedProfileCandidate,
            AcceptedProfileEnvironment, AcceptedProfileKey, LspProfileState,
        },
    },
    requests::{RequestRegistry, SignatureCancellationReason},
    uri_key::LspUriKey,
};

use super::{ArcweftLspSession, lifecycle::AcceptedPublicationError};

#[derive(Debug)]
struct ProfileMappingRollback {
    uri: LspUriKey,
    profile: Option<LspProfile>,
    profile_key: Option<AcceptedProfileKey>,
}

impl ArcweftLspSession {
    pub(super) fn refresh_profile_for_uri(
        &mut self,
        uri: &lsp_types::Uri,
        requests: &RequestRegistry,
    ) {
        self.refresh_profile_for_uri_inner(uri, requests, || {});
    }

    #[cfg(test)]
    pub(super) fn refresh_profile_for_uri_with_remap_checkpoint(
        &mut self,
        uri: &lsp_types::Uri,
        requests: &RequestRegistry,
        after_remap_publication: impl FnOnce(),
    ) {
        self.refresh_profile_for_uri_inner(uri, requests, after_remap_publication);
    }

    fn refresh_profile_for_uri_inner(
        &mut self,
        uri: &lsp_types::Uri,
        requests: &RequestRegistry,
        after_remap_publication: impl FnOnce(),
    ) {
        let key = LspUriKey::from_uri(uri);
        let previous_profile = self.profiles_by_uri.get(&key).cloned();
        let previous_accepted = previous_profile
            .as_ref()
            .and_then(LspProfile::accepted_environment);
        let state = previous_profile.as_ref().map_or_else(
            || Arc::new(LspProfileState::new()),
            |profile| Arc::clone(profile.state()),
        );
        let Some(overlays) = self.profile_registration_overlays(uri, previous_accepted.as_ref())
        else {
            return;
        };
        let registered = self.profile_resolver.resolve_candidate_for_uri(
            uri,
            &overlays,
            previous_accepted.as_ref(),
        );
        let registered = match registered {
            Ok(registered) => registered,
            Err(diagnostic) => {
                self.record_profile_construction_failure(
                    key,
                    previous_profile,
                    previous_accepted.as_ref(),
                    &state,
                    diagnostic,
                );
                return;
            }
        };
        let candidate_profile = registered.candidate().profile().clone();
        let profile_remapped = previous_accepted
            .as_ref()
            .is_some_and(|accepted| accepted.profile() != &candidate_profile);
        let publication_state = if profile_remapped {
            Arc::new(LspProfileState::new())
        } else {
            Arc::clone(&state)
        };
        let profile = self
            .profile_resolver
            .profile_from_registered_metadata(&registered, Arc::clone(&publication_state));
        let publication_keys = if profile_remapped {
            self.profiles_by_uri
                .iter()
                .filter(|(_, mapped)| Arc::ptr_eq(mapped.state(), &state))
                .map(|(uri, _)| uri.clone())
                .collect::<Vec<_>>()
        } else {
            vec![key.clone()]
        };
        let previous_mappings = self.stage_profile_publication_mappings(
            &publication_keys,
            &profile,
            &candidate_profile,
        );
        let (candidate, characters, topology) = registered.into_parts();
        let expected = if profile_remapped {
            None
        } else {
            previous_accepted.as_ref()
        };
        let publication =
            self.publish_accepted_candidate(&publication_state, expected, candidate, requests);
        let accepted = match publication {
            Ok(accepted) => accepted,
            Err(error) => {
                self.restore_profile_publication_mappings(
                    previous_mappings,
                    &publication_state,
                    &error,
                );
                return;
            }
        };
        if profile_remapped {
            after_remap_publication();
            let previous = previous_accepted
                .as_ref()
                .expect("profile remapping has a previous accepted environment");
            requests.cancel_accepted(previous, SignatureCancellationReason::ProfileRemapped);
            previous.clear_caches();
            self.pending_signature_authority
                .remove_profile(previous.profile());
            state.shutdown();
        }
        self.apply_registered_topology_to_profile_state(&publication_state, &topology, &characters);
        self.pending_signature_authority.accept(&accepted);
    }

    fn profile_registration_overlays(
        &self,
        uri: &lsp_types::Uri,
        previous: Option<&Arc<AcceptedProfileEnvironment>>,
    ) -> Option<Vec<ProfileRegistrationOverlay>> {
        let snapshots = previous.map_or_else(
            || self.documents.get(uri).cloned().into_iter().collect(),
            |accepted| {
                self.documents
                    .snapshots()
                    .filter(|snapshot| {
                        accepted
                            .project()
                            .sources()
                            .by_uri(snapshot.uri())
                            .is_some()
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            },
        );
        snapshots
            .into_iter()
            .map(|snapshot| {
                let path = file_path_from_uri(snapshot.uri())?;
                let seed = arcweft_project_loader::topology::ProfileTopologyOverlaySeed::try_new(
                    path,
                    snapshot.text().to_owned(),
                )
                .ok()?;
                Some(ProfileRegistrationOverlay::new(
                    seed,
                    LspUriKey::from_uri(snapshot.uri()),
                    snapshot.version(),
                ))
            })
            .collect()
    }

    fn record_profile_construction_failure(
        &mut self,
        key: LspUriKey,
        previous_profile: Option<LspProfile>,
        previous_accepted: Option<&Arc<AcceptedProfileEnvironment>>,
        state: &Arc<LspProfileState>,
        diagnostic: LspProfileDiagnostic,
    ) {
        if let Some(previous) = previous_accepted {
            let _ = self.record_failed_replacement(state, previous);
        }
        let mut profile = previous_profile.unwrap_or_else(|| {
            self.profile_resolver
                .default_with_diagnostic_and_state(diagnostic.clone(), Arc::clone(state))
        });
        profile.replace_diagnostics(diagnostic);
        self.profiles_by_uri.insert(key, profile);
    }

    fn stage_profile_publication_mappings(
        &mut self,
        publication_keys: &[LspUriKey],
        profile: &LspProfile,
        profile_key: &AcceptedProfileKey,
    ) -> Vec<ProfileMappingRollback> {
        publication_keys
            .iter()
            .map(|uri| ProfileMappingRollback {
                uri: uri.clone(),
                profile: self.profiles_by_uri.insert(uri.clone(), profile.clone()),
                profile_key: self
                    .profile_keys_by_uri
                    .insert(uri.clone(), profile_key.clone()),
            })
            .collect()
    }

    fn restore_profile_publication_mappings(
        &mut self,
        mappings: Vec<ProfileMappingRollback>,
        failed_state: &Arc<LspProfileState>,
        error: &AcceptedPublicationError,
    ) {
        for mapping in mappings {
            let profile = mapping.profile.map_or_else(
                || {
                    self.profile_resolver.default_with_diagnostic_and_state(
                        LspProfileDiagnostic::new(
                            LspProfileDiagnosticKind::ProfilePublication,
                            error.to_string(),
                        ),
                        Arc::clone(failed_state),
                    )
                },
                |mut profile| {
                    profile.replace_diagnostics(LspProfileDiagnostic::new(
                        LspProfileDiagnosticKind::ProfilePublication,
                        error.to_string(),
                    ));
                    profile
                },
            );
            self.profiles_by_uri.insert(mapping.uri.clone(), profile);
            if let Some(profile_key) = mapping.profile_key {
                self.profile_keys_by_uri.insert(mapping.uri, profile_key);
            } else {
                self.profile_keys_by_uri.remove(&mapping.uri);
            }
        }
    }

    pub(super) fn evict_signature_document_for_uri(&self, uri: &lsp_types::Uri) {
        let uri = LspUriKey::from_uri(uri);
        let mut accepted = Vec::new();
        for environment in self
            .profiles_by_uri
            .values()
            .filter_map(LspProfile::accepted_environment)
        {
            if accepted
                .iter()
                .all(|current| !Arc::ptr_eq(current, &environment))
            {
                accepted.push(environment);
            }
        }
        for environment in accepted {
            if let Some(document) = environment.project().source_identity_by_uri(&uri) {
                environment.evict_signature_document(document);
            }
        }
    }

    pub(super) fn refresh_profile_for_open_documents(
        &mut self,
        requests: &RequestRegistry,
    ) -> Vec<Notification> {
        self.invalidate_analysis_cache();
        let snapshots = self.documents.snapshots().cloned().collect::<Vec<_>>();
        let mut targets = Vec::<(lsp_types::Uri, Option<Arc<LspProfileState>>)>::new();
        for snapshot in &snapshots {
            let state = self
                .profiles_by_uri
                .get(&LspUriKey::from_uri(snapshot.uri()))
                .map(|profile| Arc::clone(profile.state()));
            if state.as_ref().is_some_and(|state| {
                targets.iter().any(|(_, current)| {
                    current
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, state))
                })
            }) {
                continue;
            }
            targets.push((snapshot.uri().clone(), state));
        }
        for (uri, _) in targets {
            self.refresh_profile_for_uri(&uri, requests);
        }
        snapshots
            .iter()
            .map(|snapshot| self.refresh_document_diagnostics(snapshot))
            .collect()
    }

    pub(super) fn rebuild_profiles_affected_by_uri(
        &mut self,
        changed: &lsp_types::Uri,
        requests: &RequestRegistry,
        allow_unchanged_project: bool,
    ) {
        let keys = self
            .profiles_by_uri
            .iter()
            .filter(|(_, profile)| {
                profile
                    .accepted_environment()
                    .is_some_and(|accepted| accepted.project().sources().by_uri(changed).is_some())
            })
            .fold(
                Vec::<(LspUriKey, Arc<LspProfileState>)>::new(),
                |mut keys, (key, profile)| {
                    if keys
                        .iter()
                        .all(|(_, state)| !Arc::ptr_eq(state, profile.state()))
                    {
                        keys.push((key.clone(), Arc::clone(profile.state())));
                    }
                    keys
                },
            );
        for (key, _) in keys {
            self.rebuild_profile_with_current_overlays(&key, requests, allow_unchanged_project);
        }
    }

    pub(super) fn rebuild_profile_with_current_overlays(
        &mut self,
        key: &LspUriKey,
        requests: &RequestRegistry,
        allow_unchanged_project: bool,
    ) {
        let Some(profile) = self.profiles_by_uri.get(key).cloned() else {
            return;
        };
        let Some(resolved) = profile.resolved_profile() else {
            return;
        };
        let Some(previous) = profile.accepted_environment() else {
            return;
        };
        let manifest_uri = previous.profile().manifest_key().to_uri();
        let Some(manifest_path) = file_path_from_uri(&manifest_uri) else {
            return;
        };
        let mut registration_overlays = Vec::new();
        let mut overlay_entries = Vec::new();
        for snapshot in self.documents.snapshots() {
            let Some(accepted_source) = previous.project().sources().by_uri(snapshot.uri()) else {
                continue;
            };
            let version = snapshot.version();
            let Ok(document) = rebind_overlay(snapshot, accepted_source) else {
                return;
            };
            let Some(path) = file_path_from_uri(snapshot.uri()) else {
                return;
            };
            let Ok(seed) = arcweft_project_loader::topology::ProfileTopologyOverlaySeed::try_new(
                path,
                snapshot.text().to_owned(),
            ) else {
                return;
            };
            let uri = LspUriKey::from_uri(snapshot.uri());
            overlay_entries.push((
                uri.clone(),
                AcceptedOverlayEntry::new(version, document.identity().clone()),
            ));
            registration_overlays.push(ProfileRegistrationOverlay::new(seed, uri, version));
        }
        let Ok(overlays) = AcceptedOverlaySet::try_new(overlay_entries) else {
            return;
        };
        if allow_unchanged_project
            && let Ok(candidate) =
                AcceptedProfileCandidate::try_from_unchanged_project(&previous, overlays.clone())
        {
            let _ = self.replace_profile_candidate(&profile, &previous, candidate, requests);
            return;
        }
        let registered = register_profile_environment_with_overlays(
            &manifest_path,
            resolved.id(),
            &registration_overlays,
            Some(previous.world().environment()),
        );
        let Ok(registered) = registered else {
            let _ = self.record_failed_replacement(profile.state(), &previous);
            return;
        };
        let (candidate, characters, topology) = registered.into_parts();
        let state = Arc::clone(profile.state());
        if self
            .replace_profile_candidate(&profile, &previous, candidate, requests)
            .is_err()
        {
            return;
        }
        self.apply_registered_topology_to_profile_state(&state, &topology, &characters);
    }

    fn replace_profile_candidate(
        &mut self,
        profile: &LspProfile,
        expected: &Arc<AcceptedProfileEnvironment>,
        candidate: AcceptedProfileCandidate,
        requests: &RequestRegistry,
    ) -> Result<Arc<AcceptedProfileEnvironment>, AcceptedPublicationError> {
        let accepted =
            self.publish_accepted_candidate(profile.state(), Some(expected), candidate, requests)?;
        let mapped = self
            .profiles_by_uri
            .iter()
            .filter(|(_, current)| Arc::ptr_eq(current.state(), profile.state()))
            .map(|(uri, _)| uri.clone())
            .collect::<Vec<_>>();
        for uri in mapped {
            self.profile_keys_by_uri
                .insert(uri, accepted.profile().clone());
        }
        self.pending_signature_authority.accept(&accepted);
        Ok(accepted)
    }
}
