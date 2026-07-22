//! Globally bounded active-request registry and weak deadline scheduler.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Condvar, Mutex, PoisonError, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use lsp_server::RequestId;
use thiserror::Error;

use crate::{
    profiles::state::{AcceptedProfileEnvironment, LspProfileState, ProfileEnvironmentLifecycle},
    uri_key::LspUriKey,
};

use super::{RequestControl, SignatureCancellationReason, SignatureRequestBinding};

pub(crate) const SIGNATURE_REQUEST_DEADLINE: Duration = Duration::from_millis(250);
pub(crate) const MAX_ACTIVE_SIGNATURE_REQUESTS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct DeadlineToken(u64);

#[derive(Debug)]
struct RegistryState {
    admission_open: bool,
    active: BTreeMap<RequestId, Arc<RequestControl>>,
}

#[derive(Debug)]
struct DeadlineScheduler {
    state: Mutex<DeadlineSchedulerState>,
    changed: Condvar,
    thread: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug)]
struct DeadlineSchedulerState {
    closed: bool,
    next_token: u64,
    deadlines: BTreeMap<(Instant, u64), Weak<RequestControl>>,
    fired: BTreeSet<(Instant, u64)>,
}

/// Registry shared by protocol intake, lifecycle invalidation, and request workers.
#[derive(Debug)]
pub(crate) struct RequestRegistry {
    state: Mutex<RegistryState>,
    scheduler: Arc<DeadlineScheduler>,
    request_deadline: Duration,
    shutdown: AtomicBool,
}

/// Non-cloneable cleanup guard for one exact active entry and deadline token.
#[derive(Debug)]
pub(crate) struct ActiveRequest {
    registry: Arc<RequestRegistry>,
    id: RequestId,
    control: Arc<RequestControl>,
    deadline_token: DeadlineToken,
}

/// Bounded request admission failed without creating a live request.
#[derive(Debug, Error)]
pub(crate) enum RequestAdmissionError {
    #[error("request id is already active")]
    DuplicateRequestId { id: RequestId },
    #[error("signature active-request limit exceeded")]
    ActiveLimit { observed: usize, maximum: usize },
    #[error("global signature admission is closed")]
    AdmissionClosed,
    #[error("profile signature admission is closed")]
    ProfileClosing,
    #[error("signature worker queue is closed")]
    QueueClosed,
    #[error("signature deadline could not be represented")]
    DeadlineOverflow,
    #[error("signature deadline token exhausted")]
    DeadlineTokenOverflow,
}

impl RequestRegistry {
    pub(super) fn try_new() -> Result<Arc<Self>, std::io::Error> {
        Self::try_new_with_deadline(SIGNATURE_REQUEST_DEADLINE)
    }

    pub(super) fn try_new_with_deadline(
        request_deadline: Duration,
    ) -> Result<Arc<Self>, std::io::Error> {
        let scheduler = Arc::new(DeadlineScheduler {
            state: Mutex::new(DeadlineSchedulerState {
                closed: false,
                next_token: 0,
                deadlines: BTreeMap::new(),
                fired: BTreeSet::new(),
            }),
            changed: Condvar::new(),
            thread: Mutex::new(None),
        });
        let scheduler_for_thread = Arc::clone(&scheduler);
        let handle = thread::Builder::new()
            .name("arcweft-signature-deadlines".to_owned())
            .spawn(move || scheduler_for_thread.run())?;
        scheduler
            .thread
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(handle);
        Ok(Arc::new(Self {
            state: Mutex::new(RegistryState {
                admission_open: true,
                active: BTreeMap::new(),
            }),
            scheduler,
            request_deadline,
            shutdown: AtomicBool::new(false),
        }))
    }

    pub(crate) fn admit(
        self: &Arc<Self>,
        id: RequestId,
        binding: SignatureRequestBinding,
    ) -> Result<ActiveRequest, RequestAdmissionError> {
        let Some(profile_state) = binding.profile_state() else {
            return Err(RequestAdmissionError::ProfileClosing);
        };
        if profile_state.lifecycle() != ProfileEnvironmentLifecycle::Active {
            return Err(RequestAdmissionError::ProfileClosing);
        }
        let deadline = Instant::now()
            .checked_add(self.request_deadline)
            .ok_or(RequestAdmissionError::DeadlineOverflow)?;
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if !state.admission_open {
            return Err(RequestAdmissionError::AdmissionClosed);
        }
        if state.active.contains_key(&id) {
            return Err(RequestAdmissionError::DuplicateRequestId { id });
        }
        let observed =
            state
                .active
                .len()
                .checked_add(1)
                .ok_or(RequestAdmissionError::ActiveLimit {
                    observed: usize::MAX,
                    maximum: MAX_ACTIVE_SIGNATURE_REQUESTS,
                })?;
        if observed > MAX_ACTIVE_SIGNATURE_REQUESTS {
            return Err(RequestAdmissionError::ActiveLimit {
                observed,
                maximum: MAX_ACTIVE_SIGNATURE_REQUESTS,
            });
        }
        let control = Arc::new(RequestControl::new(deadline, binding));
        let deadline_token = self.scheduler.register(deadline, &control)?;
        state.active.insert(id.clone(), Arc::clone(&control));
        Ok(ActiveRequest {
            registry: Arc::clone(self),
            id,
            control,
            deadline_token,
        })
    }

    pub(crate) fn cancel(&self, id: &RequestId, reason: SignatureCancellationReason) {
        let control = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .active
            .get(id)
            .cloned();
        if let Some(control) = control {
            control.cancel(reason);
        }
    }

    pub(crate) fn cancel_uri(&self, uri: &LspUriKey, reason: SignatureCancellationReason) {
        self.cancel_matching(|binding| binding.uri() == uri, reason);
    }

    pub(crate) fn cancel_workspace(
        &self,
        workspace: &LspUriKey,
        reason: SignatureCancellationReason,
    ) {
        self.cancel_matching(|binding| binding.workspace() == workspace, reason);
    }

    pub(crate) fn cancel_profile_state(
        &self,
        profile_state: &Arc<LspProfileState>,
        reason: SignatureCancellationReason,
    ) {
        self.cancel_matching(|binding| binding.is_profile_state(profile_state), reason);
    }

    pub(crate) fn cancel_accepted(
        &self,
        accepted: &Arc<AcceptedProfileEnvironment>,
        reason: SignatureCancellationReason,
    ) {
        self.cancel_matching(|binding| binding.is_accepted(accepted), reason);
    }

    pub(crate) fn cancel_all(&self, reason: SignatureCancellationReason) {
        self.cancel_matching(|_| true, reason);
    }

    pub(crate) fn close_admission(&self) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .admission_open = false;
    }

    pub(crate) fn shutdown(&self) {
        if self.shutdown.swap(true, Ordering::AcqRel) {
            return;
        }
        self.close_admission();
        self.scheduler.shutdown();
        debug_assert!(
            self.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .active
                .is_empty(),
            "request guards must be dropped before registry shutdown"
        );
    }

    fn cancel_matching(
        &self,
        predicate: impl Fn(&SignatureRequestBinding) -> bool,
        reason: SignatureCancellationReason,
    ) {
        let controls = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .active
            .values()
            .filter(|control| predicate(control.binding()))
            .cloned()
            .collect::<Vec<_>>();
        for control in controls {
            control.cancel(reason);
        }
    }

    fn finish(&self, id: &RequestId, control: &Arc<RequestControl>, token: DeadlineToken) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state
            .active
            .get(id)
            .is_some_and(|current| Arc::ptr_eq(current, control))
        {
            state.active.remove(id);
        }
        drop(state);
        self.scheduler.remove(control.deadline(), token);
    }

    #[cfg(test)]
    pub(crate) fn active_len(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .active
            .len()
    }

    #[cfg(test)]
    fn deadline_len(&self) -> usize {
        self.scheduler
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .deadlines
            .len()
    }
}

impl ActiveRequest {
    pub(crate) const fn control(&self) -> &Arc<RequestControl> {
        &self.control
    }
}

impl Drop for ActiveRequest {
    fn drop(&mut self) {
        self.registry
            .finish(&self.id, &self.control, self.deadline_token);
    }
}

impl DeadlineScheduler {
    fn register(
        &self,
        deadline: Instant,
        control: &Arc<RequestControl>,
    ) -> Result<DeadlineToken, RequestAdmissionError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.closed {
            return Err(RequestAdmissionError::AdmissionClosed);
        }
        let token = state.next_token;
        state.next_token = token
            .checked_add(1)
            .ok_or(RequestAdmissionError::DeadlineTokenOverflow)?;
        state
            .deadlines
            .insert((deadline, token), Arc::downgrade(control));
        self.changed.notify_one();
        Ok(DeadlineToken(token))
    }

    fn remove(&self, deadline: Instant, token: DeadlineToken) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.deadlines.remove(&(deadline, token.0));
        state.fired.remove(&(deadline, token.0));
        drop(state);
        self.changed.notify_one();
    }

    fn run(&self) {
        loop {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            while !state.closed && state.deadlines.is_empty() {
                state = self
                    .changed
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
            if state.closed {
                return;
            }
            let Some((&(deadline, token), control)) = state
                .deadlines
                .iter()
                .find(|(key, _)| !state.fired.contains(*key))
            else {
                state = self
                    .changed
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
                drop(state);
                continue;
            };
            let now = Instant::now();
            if now < deadline {
                let (next, _) = self
                    .changed
                    .wait_timeout(state, deadline.saturating_duration_since(now))
                    .unwrap_or_else(PoisonError::into_inner);
                drop(next);
                continue;
            }
            let control = control.clone();
            state.fired.insert((deadline, token));
            drop(state);
            if let Some(control) = control.upgrade() {
                control.cancel(SignatureCancellationReason::DeadlineExceeded);
            }
        }
    }

    fn shutdown(&self) {
        {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.closed = true;
            state.deadlines.clear();
            state.fired.clear();
            self.changed.notify_all();
        }
        if let Some(handle) = self
            .thread
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = handle.join();
        }
    }
}

impl Drop for RequestRegistry {
    fn drop(&mut self) {
        self.scheduler.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::requests::RequestGateState;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use lsp_types::Uri;

    fn binding(state: &Arc<LspProfileState>, suffix: usize) -> SignatureRequestBinding {
        let uri = format!("file:///workspace/request-{suffix}.arcw")
            .parse::<Uri>()
            .expect("URI");
        let workspace = "file:///workspace".parse::<Uri>().expect("workspace URI");
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new(uri.to_string()).expect("document ID"),
            SourceName::path(uri.to_string()),
            format!("flow @flow.request_{suffix} request_{suffix} {{}}"),
        )
        .expect("source document");
        SignatureRequestBinding::for_test(
            LspUriKey::from_uri(&uri),
            LspUriKey::from_uri(&workspace),
            state,
            document.identity().clone(),
        )
    }

    #[test]
    fn active_guard_is_the_only_registry_cleanup_path() {
        let registry = RequestRegistry::try_new().expect("request registry");
        let state = Arc::new(LspProfileState::new());
        let active = registry
            .admit(RequestId::from(1), binding(&state, 1))
            .expect("admitted request");
        assert_eq!(registry.active_len(), 1);
        drop(active);
        assert_eq!(registry.active_len(), 0);
        registry.shutdown();
    }

    #[test]
    fn duplicate_and_one_over_active_limit_create_no_extra_entry() {
        let registry = RequestRegistry::try_new().expect("request registry");
        let state = Arc::new(LspProfileState::new());
        let first = registry
            .admit(RequestId::from(1), binding(&state, 1))
            .expect("first request");
        assert!(matches!(
            registry.admit(RequestId::from(1), binding(&state, 2)),
            Err(RequestAdmissionError::DuplicateRequestId { .. })
        ));
        let mut active = vec![first];
        for id in 2..=MAX_ACTIVE_SIGNATURE_REQUESTS {
            active.push(
                registry
                    .admit(
                        RequestId::from(i32::try_from(id).expect("bounded request id fits i32")),
                        binding(&state, id),
                    )
                    .expect("within active limit"),
            );
        }
        assert!(matches!(
            registry.admit(RequestId::from(33), binding(&state, 33)),
            Err(RequestAdmissionError::ActiveLimit {
                observed: 33,
                maximum: MAX_ACTIVE_SIGNATURE_REQUESTS,
            })
        ));
        assert_eq!(registry.active_len(), MAX_ACTIVE_SIGNATURE_REQUESTS);
        drop(active);
        registry.shutdown();
    }

    #[test]
    fn unknown_cancellation_is_not_retained_and_first_reason_wins() {
        let registry = RequestRegistry::try_new().expect("request registry");
        registry.cancel(
            &RequestId::from(404),
            SignatureCancellationReason::ClientCancelled,
        );
        assert_eq!(registry.active_len(), 0);

        let state = Arc::new(LspProfileState::new());
        let active = registry
            .admit(RequestId::from(1), binding(&state, 1))
            .expect("admitted request");
        registry.cancel(
            &RequestId::from(1),
            SignatureCancellationReason::ClientCancelled,
        );
        registry.cancel(
            &RequestId::from(1),
            SignatureCancellationReason::SessionShutdown,
        );
        assert_eq!(
            *active.control().gate(),
            RequestGateState::Cancelled(SignatureCancellationReason::ClientCancelled)
        );
        drop(active);
        registry.shutdown();
    }

    #[test]
    fn request_binding_keeps_profile_state_weak() {
        let registry = RequestRegistry::try_new().expect("request registry");
        let state = Arc::new(LspProfileState::new());
        let before = Arc::strong_count(&state);
        let active = registry
            .admit(RequestId::from(1), binding(&state, 1))
            .expect("admitted request");
        assert_eq!(Arc::strong_count(&state), before);
        drop(active);
        registry.shutdown();
    }

    #[test]
    fn deadline_token_is_removed_only_when_active_guard_drops() {
        let registry = RequestRegistry::try_new().expect("request registry");
        let state = Arc::new(LspProfileState::new());
        let active = registry
            .admit(RequestId::from(1), binding(&state, 1))
            .expect("admitted request");
        let until = Instant::now() + Duration::from_secs(1);
        while !active.control().cancellation_flag().load(Ordering::Acquire)
            && Instant::now() < until
        {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(active.control().cancellation_flag().load(Ordering::Acquire));
        assert_eq!(registry.deadline_len(), 1);
        drop(active);
        assert_eq!(registry.deadline_len(), 0);
        registry.shutdown();
    }
}
