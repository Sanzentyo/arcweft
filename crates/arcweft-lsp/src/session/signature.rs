//! Typed acquisition of one accepted URI/source/module/HIR request lease.

use std::{
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

use arcweft_lang_sema::signature::{SignatureQuery, SignatureQueryControl, query_signature};
use lsp_server::{ErrorCode, Message, RequestId, Response};
use lsp_types::SignatureHelpParams;

use crate::{
    documents::rebind_overlay,
    profiles::state::{AcceptedProfileEnvironment, ProfileEnvironmentLifecycle},
    requests::{
        RequestGateState, RequestRegistry, SignatureRequestBinding,
        signature::{
            AcceptedDocumentHirLease, PreparedSignatureRequest, SignatureAcquireError,
            SignatureCacheDisposition, SignatureRequestError, SignatureRequestResult,
            SignatureRequestStale, SignatureRequestStamp, SignatureRequestWork,
        },
    },
    uri_key::LspUriKey,
};

use super::ArcweftLspSession;

impl ArcweftLspSession {
    #[allow(
        clippy::needless_pass_by_value,
        clippy::result_large_err,
        clippy::too_many_lines,
        reason = "the ordered acquisition boundary consumes decoded parameters and retains exact rejection evidence through all fourteen admission steps"
    )]
    pub(crate) fn prepare_signature_request(
        &self,
        request_id: RequestId,
        params: SignatureHelpParams,
        requests: &Arc<RequestRegistry>,
    ) -> Result<PreparedSignatureRequest, SignatureAcquireError> {
        let protocol_uri = &params.text_document_position_params.text_document.uri;
        let uri = LspUriKey::from_uri(protocol_uri);
        let snapshot = self
            .documents
            .get(protocol_uri)
            .cloned()
            .ok_or_else(|| SignatureAcquireError::DocumentNotOpen { uri: uri.clone() })?;
        let profile = self
            .profiles_by_uri
            .get(&uri)
            .ok_or_else(|| SignatureAcquireError::ProfileNotMapped { uri: uri.clone() })?;
        let profile_state = Arc::clone(profile.state());
        if profile_state.lifecycle() != ProfileEnvironmentLifecycle::Active {
            return Err(SignatureAcquireError::ProfileClosing);
        }
        let accepted_guard = profile_state.accepted_read();
        let accepted = accepted_guard
            .as_ref()
            .cloned()
            .ok_or(SignatureAcquireError::NoAcceptedEnvironment)?;
        let mapped_profile = self
            .profile_keys_by_uri
            .get(&uri)
            .ok_or(SignatureAcquireError::ProfileKeyMismatch)?;
        if accepted.profile() != mapped_profile {
            return Err(SignatureAcquireError::ProfileKeyMismatch);
        }
        if let Some(pending) = self.pending_signature_authority.profile(mapped_profile) {
            return Err(Self::pending_profile_authority_error(&accepted, pending));
        }

        let project = accepted.project();
        let accepted_identity = project
            .source_identity_by_uri(&uri)
            .cloned()
            .ok_or_else(|| SignatureAcquireError::UriNotAccepted { uri: uri.clone() })?;
        let accepted_source = project.source(&accepted_identity).ok_or_else(|| {
            SignatureAcquireError::DocumentNotAccepted {
                uri: uri.clone(),
                expected: accepted_identity.clone(),
                actual: snapshot.source_document().identity().clone(),
            }
        })?;
        let accepted_document = Arc::clone(accepted_source.document());
        let pending_document = self
            .pending_signature_authority
            .document(mapped_profile, &uri);
        if let Some(pending) = pending_document.filter(|pending| pending.bytes_changed()) {
            return Err(SignatureAcquireError::DocumentNotAccepted {
                uri: pending.uri().clone(),
                expected: pending.expected().clone(),
                actual: pending.actual().clone(),
            });
        }
        let overlay = accepted
            .overlays()
            .get(&uri)
            .ok_or_else(|| SignatureAcquireError::OverlayNotAccepted { uri: uri.clone() })?;
        if overlay.version() != snapshot.version() {
            return Err(SignatureAcquireError::OverlayVersionNotAccepted {
                uri,
                expected: overlay.version(),
                actual: snapshot.version(),
            });
        }
        if overlay.logical_identity() != &accepted_identity {
            return Err(SignatureAcquireError::DocumentNotAccepted {
                uri,
                expected: accepted_identity,
                actual: overlay.logical_identity().clone(),
            });
        }
        let rebound = rebind_overlay(&snapshot, accepted_source).map_err(|_| {
            SignatureAcquireError::DocumentNotAccepted {
                uri: uri.clone(),
                expected: accepted_identity.clone(),
                actual: snapshot.source_document().identity().clone(),
            }
        })?;
        if rebound.identity() != &accepted_identity {
            return Err(SignatureAcquireError::DocumentNotAccepted {
                uri,
                expected: accepted_identity,
                actual: rebound.identity().clone(),
            });
        }
        if rebound.text() != accepted_document.text() {
            return Err(SignatureAcquireError::SourceDigestCollision {
                source: accepted_identity,
            });
        }
        if let Some(pending) = pending_document {
            return Err(SignatureAcquireError::DocumentNotAccepted {
                uri: pending.uri().clone(),
                expected: pending.expected().clone(),
                actual: pending.actual().clone(),
            });
        }
        let module = project.module_key(&accepted_identity).ok_or_else(|| {
            SignatureAcquireError::SourceHasNoHirModule {
                source: accepted_identity.clone(),
            }
        })?;
        project.hir(&module).map_err(|error| match error {
            crate::profiles::accepted_project::AcceptedHirLookupError::MissingModule { key } => {
                SignatureAcquireError::MissingHirModule { module: key }
            }
            crate::profiles::accepted_project::AcceptedHirLookupError::SourceIdentityMismatch {
                key,
                ..
            }
            | crate::profiles::accepted_project::AcceptedHirLookupError::MissingSourceDocument {
                key,
            }
            | crate::profiles::accepted_project::AcceptedHirLookupError::SourceDocumentMismatch {
                key,
                ..
            } => SignatureAcquireError::HirIdentityMismatch { module: key },
        })?;

        let lease = AcceptedDocumentHirLease::new(
            Arc::clone(&accepted),
            Arc::clone(&accepted_document),
            module.clone(),
        );
        let stamp = SignatureRequestStamp::new(
            Arc::clone(&profile_state),
            Arc::clone(&accepted),
            accepted_document,
            uri.clone(),
            snapshot.source_document().identity().clone(),
            snapshot.version(),
            module,
        );
        let binding = SignatureRequestBinding::new(
            uri,
            accepted.profile().workspace_key().clone(),
            &profile_state,
            &accepted,
            snapshot.source_document().identity().clone(),
        );
        let active = requests.admit(request_id.clone(), binding)?;
        drop(accepted_guard);
        Ok(PreparedSignatureRequest::new(
            request_id,
            params.text_document_position_params.position,
            snapshot,
            lease,
            stamp,
            active,
        ))
    }

    fn pending_profile_authority_error(
        accepted: &AcceptedProfileEnvironment,
        pending: &super::overlay_authority::PendingOverlayRevision,
    ) -> SignatureAcquireError {
        if pending.bytes_changed() {
            return SignatureAcquireError::DocumentNotAccepted {
                uri: pending.uri().clone(),
                expected: pending.expected().clone(),
                actual: pending.actual().clone(),
            };
        }
        let Some(overlay) = accepted.overlays().get(pending.uri()) else {
            return SignatureAcquireError::OverlayNotAccepted {
                uri: pending.uri().clone(),
            };
        };
        if overlay.version() != pending.version() {
            return SignatureAcquireError::OverlayVersionNotAccepted {
                uri: pending.uri().clone(),
                expected: overlay.version(),
                actual: pending.version(),
            };
        }
        if overlay.logical_identity() != pending.expected() {
            return SignatureAcquireError::DocumentNotAccepted {
                uri: pending.uri().clone(),
                expected: pending.expected().clone(),
                actual: overlay.logical_identity().clone(),
            };
        }
        SignatureAcquireError::DocumentNotAccepted {
            uri: pending.uri().clone(),
            expected: pending.expected().clone(),
            actual: pending.actual().clone(),
        }
    }

    #[allow(
        clippy::result_large_err,
        reason = "the pre-work boundary preserves exact acquisition and freshness failure evidence"
    )]
    pub(crate) fn signature_work(
        &self,
        prepared: &PreparedSignatureRequest,
    ) -> Result<SignatureRequestWork, SignatureRequestError> {
        let accepted = prepared.stamp().profile_state().accepted_read();
        let control = prepared.control();
        let gate = control.gate();
        self.validate_signature_stamp(prepared.stamp(), control, *gate, accepted.as_ref())?;
        let byte_offset = prepared
            .snapshot()
            .line_index()
            .try_byte_offset_from_position(prepared.position())?;
        let key = prepared.stamp().cache_key(byte_offset);
        let mut cache = prepared.stamp().accepted().signature_cache();
        self.validate_signature_stamp(prepared.stamp(), control, *gate, accepted.as_ref())?;
        let cached = cache.cached(&key);
        self.validate_signature_stamp(prepared.stamp(), control, *gate, accepted.as_ref())?;
        if let Some(outcome) = cached {
            return Ok(SignatureRequestWork::Hit(SignatureRequestResult::new(
                key,
                outcome,
                SignatureCacheDisposition::Hit,
            )));
        }
        Ok(SignatureRequestWork::Miss(key))
    }

    #[allow(
        clippy::result_large_err,
        reason = "the unlocked semantic boundary preserves exact acquisition and query failure evidence"
    )]
    pub(crate) fn compute_signature(
        prepared: &PreparedSignatureRequest,
        key: crate::profiles::caches::SignatureCacheKey,
    ) -> Result<SignatureRequestResult, SignatureRequestError> {
        let byte_offset = key.byte_offset();
        let lease = prepared.lease();
        let query = SignatureQuery::production(
            lease.world(),
            lease.document(),
            lease.hir()?,
            byte_offset,
            SignatureQueryControl::new(
                prepared.control().cancellation_flag(),
                Some(prepared.control().deadline()),
            ),
        )?;
        let outcome = Arc::new(query_signature(query)?);
        Ok(SignatureRequestResult::new(
            key,
            outcome,
            SignatureCacheDisposition::Miss,
        ))
    }

    pub(crate) fn publish_signature_result(
        &self,
        prepared: &PreparedSignatureRequest,
        result: Result<SignatureRequestResult, SignatureRequestError>,
        responses: &crossbeam_channel::Sender<Message>,
    ) {
        self.publish_signature_result_inner(prepared, result, responses, || {});
    }

    /// Publishes an internal error only while the exact prepared authority is
    /// current; otherwise the winning stale/deadline status is published.
    pub(crate) fn publish_signature_worker_panic(
        &self,
        prepared: &PreparedSignatureRequest,
        responses: &crossbeam_channel::Sender<Message>,
    ) {
        let accepted = prepared.stamp().profile_state().accepted_read();
        let control = prepared.control();
        let mut gate = control.gate();
        if *gate == RequestGateState::Finished {
            return;
        }
        let response = match self.validate_signature_stamp(
            prepared.stamp(),
            control,
            *gate,
            accepted.as_ref(),
        ) {
            Ok(()) => Response::new_err(
                prepared.request_id().clone(),
                ErrorCode::InternalError as i32,
                "signature worker panicked".to_owned(),
            ),
            Err(error) => {
                SignatureRequestError::from(error).into_response(prepared.request_id().clone())
            }
        };
        if responses.send(Message::Response(response)).is_ok() {
            *gate = RequestGateState::Finished;
        }
    }

    #[cfg(test)]
    pub(crate) fn publish_signature_result_after_projection(
        &self,
        prepared: &PreparedSignatureRequest,
        result: Result<SignatureRequestResult, SignatureRequestError>,
        responses: &crossbeam_channel::Sender<Message>,
        after_projection: impl FnOnce(),
    ) {
        self.publish_signature_result_inner(prepared, result, responses, after_projection);
    }

    fn publish_signature_result_inner(
        &self,
        prepared: &PreparedSignatureRequest,
        result: Result<SignatureRequestResult, SignatureRequestError>,
        responses: &crossbeam_channel::Sender<Message>,
        after_projection: impl FnOnce(),
    ) {
        let accepted = prepared.stamp().profile_state().accepted_read();
        let control = prepared.control();
        let mut gate = control.gate();
        let mut cache = prepared.stamp().accepted().signature_cache();
        let (mut response, mut insertion) = match self.validate_signature_stamp(
            prepared.stamp(),
            control,
            *gate,
            accepted.as_ref(),
        ) {
            Ok(()) => match result {
                Ok(result) => {
                    match crate::features::signature::signature_help(result.outcome().as_ref()) {
                        Ok(help) => {
                            let (key, outcome, cache) = result.into_parts();
                            let insertion = (cache == SignatureCacheDisposition::Miss)
                                .then_some((key, outcome));
                            (
                                Response::new_ok(prepared.request_id().clone(), help),
                                insertion,
                            )
                        }
                        Err(error) => (
                            SignatureRequestError::from(error)
                                .into_response(prepared.request_id().clone()),
                            None,
                        ),
                    }
                }
                Err(error) => (error.into_response(prepared.request_id().clone()), None),
            },
            Err(error) => (
                SignatureRequestError::from(error).into_response(prepared.request_id().clone()),
                None,
            ),
        };
        after_projection();
        if let Err(error) =
            self.validate_signature_stamp(prepared.stamp(), control, *gate, accepted.as_ref())
        {
            response =
                SignatureRequestError::from(error).into_response(prepared.request_id().clone());
            insertion = None;
        }
        if responses.send(Message::Response(response)).is_ok() {
            *gate = RequestGateState::Finished;
            #[cfg(test)]
            prepared.trigger_executor_fault(
                crate::requests::signature::SignatureExecutorFaultPoint::AfterResponseEnqueue,
            );
            if let Some((key, outcome)) = insertion {
                let _ = cache.insert(
                    key,
                    outcome,
                    prepared.stamp().project().footprint().source_bytes(),
                );
            }
        }
    }

    #[allow(
        clippy::result_large_err,
        clippy::too_many_lines,
        reason = "the package contract requires this deterministic exact-field validation order before publication"
    )]
    fn validate_signature_stamp(
        &self,
        stamp: &SignatureRequestStamp,
        control: &crate::requests::RequestControl,
        gate: RequestGateState,
        current: Option<&Arc<crate::profiles::state::AcceptedProfileEnvironment>>,
    ) -> Result<(), SignatureRequestStale> {
        validate_gate(control, gate)?;
        if control.binding().document() != stamp.protocol_document() {
            return Err(SignatureRequestStale::DocumentChanged {
                expected: stamp.protocol_document().clone(),
                actual: control.binding().document().clone(),
            });
        }
        if !self.signature_admission_open {
            return Err(SignatureRequestStale::SessionClosing);
        }
        if stamp.profile_state().lifecycle() != ProfileEnvironmentLifecycle::Active {
            return Err(SignatureRequestStale::ProfileClosing);
        }
        let snapshot = self.documents.get_by_key(stamp.uri()).ok_or_else(|| {
            SignatureRequestStale::DocumentClosed {
                uri: stamp.uri().clone(),
            }
        })?;
        if snapshot.source_document().identity() != stamp.protocol_document() {
            return Err(SignatureRequestStale::DocumentChanged {
                expected: stamp.protocol_document().clone(),
                actual: snapshot.source_document().identity().clone(),
            });
        }
        if snapshot.version() != stamp.lsp_version() {
            return Err(SignatureRequestStale::DocumentVersionChanged {
                expected: stamp.lsp_version(),
                actual: snapshot.version(),
            });
        }
        let actual_profile = self.profile_keys_by_uri.get(stamp.uri()).cloned();
        if actual_profile.as_ref() != Some(stamp.profile()) {
            return Err(SignatureRequestStale::ProfileRemapped {
                expected: stamp.profile().clone(),
                actual: actual_profile,
            });
        }
        let Some(profile) = self.profiles_by_uri.get(stamp.uri()) else {
            return Err(SignatureRequestStale::ProfileRemapped {
                expected: stamp.profile().clone(),
                actual: None,
            });
        };
        if !Arc::ptr_eq(profile.state(), stamp.profile_state()) {
            return Err(SignatureRequestStale::ProfileStateReplaced);
        }

        let Some(current) = current else {
            return Err(SignatureRequestStale::AcceptedReplaced);
        };
        if current.profile() != stamp.profile() {
            return Err(SignatureRequestStale::ProfileKeyChanged {
                expected: stamp.profile().clone(),
                actual: current.profile().clone(),
            });
        }
        if current.generation() != stamp.generation() {
            return Err(SignatureRequestStale::GenerationChanged {
                expected: stamp.generation(),
                actual: current.generation(),
            });
        }
        let pending = self
            .pending_signature_authority
            .profile(stamp.profile())
            .or_else(|| {
                self.pending_signature_authority
                    .document(stamp.profile(), stamp.uri())
            })
            .filter(|pending| pending.previous_generation() == stamp.generation());
        if let Some(pending) = pending {
            return Err(SignatureRequestStale::DocumentChanged {
                expected: pending.expected().clone(),
                actual: pending.actual().clone(),
            });
        }
        let world = current.world();
        if world.symbols().world() != stamp.world_id() {
            return Err(SignatureRequestStale::WorldIdentityChanged {
                expected: stamp.world_id().clone(),
                actual: world.symbols().world().clone(),
            });
        }
        if *world.symbols().revision() != stamp.symbol_revision() {
            return Err(SignatureRequestStale::SymbolRevisionChanged {
                expected: stamp.symbol_revision(),
                actual: *world.symbols().revision(),
            });
        }
        let environment = world.environment();
        if environment.character_digest() != stamp.character_digest() {
            return Err(SignatureRequestStale::CharacterDigestChanged {
                expected: stamp.character_digest(),
                actual: environment.character_digest(),
            });
        }
        if environment.character_revision() != stamp.character_revision() {
            return Err(SignatureRequestStale::CharacterRevisionChanged {
                expected: stamp.character_revision(),
                actual: environment.character_revision(),
            });
        }
        if environment.environment_digest() != stamp.environment_digest() {
            return Err(SignatureRequestStale::EnvironmentDigestChanged {
                expected: stamp.environment_digest(),
                actual: environment.environment_digest(),
            });
        }
        if !Arc::ptr_eq(world, stamp.world()) {
            return Err(SignatureRequestStale::WorldArcChanged);
        }

        let project = current.project();
        let actual_identity = project.source_identity_by_uri(stamp.uri()).cloned();
        if actual_identity.as_ref() != Some(stamp.accepted_document_identity()) {
            return Err(SignatureRequestStale::UriRemapped {
                expected: stamp.accepted_document_identity().clone(),
                actual: actual_identity,
            });
        }
        let actual_module = project.module_key(stamp.accepted_document_identity());
        if actual_module.as_ref() != Some(stamp.module()) {
            return Err(SignatureRequestStale::ModuleChanged {
                expected: stamp.module().clone(),
                actual: actual_module,
            });
        }
        let Some(accepted_source) = project.source(stamp.accepted_document_identity()) else {
            return Err(SignatureRequestStale::AcceptedDocumentChanged {
                expected: stamp.accepted_document_identity().clone(),
                actual: None,
            });
        };
        let rebound = rebind_overlay(snapshot, accepted_source).map_err(|_| {
            SignatureRequestStale::DocumentChanged {
                expected: stamp.protocol_document().clone(),
                actual: snapshot.source_document().identity().clone(),
            }
        })?;
        if rebound.identity() != stamp.accepted_document_identity()
            || rebound.text() != stamp.accepted_document().text()
        {
            return Err(SignatureRequestStale::DocumentChanged {
                expected: stamp.accepted_document_identity().clone(),
                actual: rebound.identity().clone(),
            });
        }
        let current_document = accepted_source.document();
        if current_document.identity() != stamp.accepted_document_identity()
            || current_document.text() != stamp.accepted_document().text()
        {
            return Err(SignatureRequestStale::AcceptedDocumentChanged {
                expected: stamp.accepted_document_identity().clone(),
                actual: Some(current_document.identity().clone()),
            });
        }
        if !Arc::ptr_eq(current_document, stamp.accepted_document()) {
            return Err(SignatureRequestStale::AcceptedDocumentChanged {
                expected: stamp.accepted_document_identity().clone(),
                actual: Some(current_document.identity().clone()),
            });
        }
        if !Arc::ptr_eq(project.hir_project(), stamp.hir_project()) {
            return Err(SignatureRequestStale::HirChanged {
                module: stamp.module().clone(),
            });
        }
        if project.hir(stamp.module()).is_err() {
            return Err(SignatureRequestStale::HirChanged {
                module: stamp.module().clone(),
            });
        }
        if !Arc::ptr_eq(project, stamp.project()) {
            return Err(SignatureRequestStale::ProjectArcChanged);
        }
        if !Arc::ptr_eq(current, stamp.accepted()) {
            return Err(SignatureRequestStale::AcceptedReplaced);
        }
        validate_gate(control, gate)
    }
}

#[allow(
    clippy::result_large_err,
    reason = "gate failures retain the exact deadline and cancellation reason"
)]
fn validate_gate(
    control: &crate::requests::RequestControl,
    gate: RequestGateState,
) -> Result<(), SignatureRequestStale> {
    match gate {
        RequestGateState::Cancelled(reason) => {
            return Err(SignatureRequestStale::Cancelled { reason });
        }
        RequestGateState::Finished => {
            return Err(SignatureRequestStale::Cancelled {
                reason: crate::requests::SignatureCancellationReason::SessionShutdown,
            });
        }
        RequestGateState::Active => {}
    }
    if control.cancellation_flag().load(Ordering::Acquire) {
        return Err(SignatureRequestStale::Cancelled {
            reason: crate::requests::SignatureCancellationReason::SessionShutdown,
        });
    }
    if Instant::now() >= control.deadline() {
        return Err(SignatureRequestStale::DeadlineExceeded {
            deadline: control.deadline(),
        });
    }
    Ok(())
}
