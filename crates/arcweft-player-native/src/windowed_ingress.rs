//! Adapter-owned ingress for live patching a running windowed native player.
//!
//! The ingress handle is intentionally narrower than a watcher, socket server, or
//! release-policy verifier. Producers hand this layer already verified local
//! bytes or local sidecar files. The handle reserves bounded FIFO capacity,
//! wakes the `winit` event loop, and lets the event-loop owner mutate
//! session/catalog state only at the safe frame boundary.

use crate::patch_endpoint::{NativePatchTransportAction, NativePatchTransportEnvelope};
use crate::windowed_patch::{PatchEventSource, RestartReason, WindowedPatchEvent};
use arcweft_bundle::container::BundleDigest;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use thiserror::Error;
use winit::event_loop::EventLoopProxy;

/// Default number of adapter-originated patch events that may wait for the
/// event loop and safe frame boundary.
pub const DEFAULT_WINDOWED_PATCH_INGRESS_CAPACITY: usize = 32;

/// Adapter ingress configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowedPatchIngressConfig {
    /// Maximum reserved FIFO events awaiting event-loop acceptance or boundary
    /// completion. A capacity of zero rejects every adapter event.
    pub capacity: usize,
}

/// Adapter enqueue acknowledgement returned after the event-loop proxy wakes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowedPatchIngressAccepted {
    pub sequence: u64,
    pub queued: usize,
    pub capacity: usize,
}

/// Last adapter-side ingress report visible before a patch reaches the frame boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowedPatchIngressReport {
    pub state: WindowedPatchIngressReportState,
    pub sequence: Option<u64>,
    pub source: Option<PatchEventSource>,
    pub queued: usize,
    pub capacity: usize,
    pub message: String,
    pub error: Option<WindowedPatchIngressErrorKind>,
}

/// Adapter-side ingress status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowedPatchIngressReportState {
    Idle,
    Enqueued,
    AcceptedByEventLoop,
    CompletedAtFrameBoundary,
    Rejected,
    Closed,
}

/// Stable adapter error kind for reports and CLI tooling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowedPatchIngressErrorKind {
    QueueFull,
    EventLoopClosed,
    PlayerClosed,
    ReadSidecar,
    ReadRestartBundle,
    MalformedIngressMessage,
    WrongBaseRoot,
    UnsupportedTransportAction,
}

/// Adapter-side ingress errors.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WindowedPatchIngressError {
    #[error("windowed patch ingress queue is full: queued {queued}, capacity {capacity}")]
    QueueFull { queued: usize, capacity: usize },
    #[error("windowed patch ingress event loop is closed")]
    EventLoopClosed,
    #[error("windowed patch ingress player is closed")]
    PlayerClosed,
    #[error("failed to read local patch sidecar {path}: {message}")]
    ReadSidecar { path: PathBuf, message: String },
    #[error("failed to read local restart bundle {path}: {message}")]
    ReadRestartBundle { path: PathBuf, message: String },
    #[error("malformed windowed patch ingress message from {event_source:?}: {message}")]
    MalformedIngressMessage {
        event_source: PatchEventSource,
        message: String,
    },
    #[error(
        "windowed patch ingress base root mismatch from {event_source:?}: expected {expected}, actual {actual}"
    )]
    WrongBaseRoot {
        event_source: PatchEventSource,
        expected: BundleDigest,
        actual: BundleDigest,
    },
    #[error("windowed patch ingress action {action:?} from {event_source:?} is not supported here")]
    UnsupportedTransportAction {
        event_source: PatchEventSource,
        action: NativePatchTransportAction,
    },
}

/// Narrow enqueue handle that can be cloned into local development producers.
#[derive(Clone)]
pub struct WindowedPatchIngress {
    sender: mpsc::Sender<WindowedPatchIngressMessage>,
    wake: Arc<dyn WindowedPatchIngressWake>,
    shared: WindowedPatchIngressShared,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WindowedPatchIngressMessage {
    Enqueue(WindowedPatchIngressEnvelope),
    RetainRejected {
        source: PatchEventSource,
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WindowedPatchIngressEnvelope {
    pub(crate) sequence: u64,
    pub(crate) event: WindowedPatchEvent,
}

#[derive(Clone, Debug)]
pub(crate) struct WindowedPatchIngressReceiver {
    receiver: Arc<Mutex<mpsc::Receiver<WindowedPatchIngressMessage>>>,
}

/// Event-loop completion handle. This is intentionally crate-private so only
/// `scene_windowed.rs` can mark events accepted/completed/closed.
#[derive(Clone, Debug)]
pub(crate) struct WindowedPatchIngressCompletion {
    shared: WindowedPatchIngressShared,
}

/// Already materialized local watch sidecar bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowedLocalSidecar {
    bytes: Vec<u8>,
    base_dir: PathBuf,
    source: PatchEventSource,
    expected_base_content_root: Option<BundleDigest>,
    supported_actions: WindowedPatchTransportActionSet,
}

/// Transport actions accepted by a development producer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowedPatchTransportActionSet {
    apply_patch: bool,
    restart_player: bool,
}

#[derive(Clone, Debug)]
struct WindowedPatchIngressShared {
    state: Arc<Mutex<WindowedPatchIngressSharedState>>,
}

#[derive(Clone, Debug)]
struct WindowedPatchIngressSharedState {
    capacity: usize,
    queued: usize,
    next_sequence: u64,
    closed: bool,
    report: WindowedPatchIngressReport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WindowedPatchIngressReservation {
    sequence: u64,
    accepted: WindowedPatchIngressAccepted,
}

trait WindowedPatchIngressWake: Send + Sync {
    fn wake_up(&self) -> Result<(), WindowedPatchIngressWakeError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WindowedPatchIngressWakeError;

#[derive(Debug)]
struct WinitWindowedPatchIngressWake {
    proxy: EventLoopProxy,
}

impl Default for WindowedPatchIngressConfig {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_WINDOWED_PATCH_INGRESS_CAPACITY,
        }
    }
}

impl WindowedPatchIngressReportState {
    /// Stable label for logs, tests, and developer tooling.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Enqueued => "enqueued",
            Self::AcceptedByEventLoop => "accepted_by_event_loop",
            Self::CompletedAtFrameBoundary => "completed_at_frame_boundary",
            Self::Rejected => "rejected",
            Self::Closed => "closed",
        }
    }
}

impl WindowedPatchIngressErrorKind {
    /// Stable label for logs, tests, and developer tooling.
    pub const fn label(self) -> &'static str {
        match self {
            Self::QueueFull => "queue_full",
            Self::EventLoopClosed => "event_loop_closed",
            Self::PlayerClosed => "player_closed",
            Self::ReadSidecar => "read_sidecar",
            Self::ReadRestartBundle => "read_restart_bundle",
            Self::MalformedIngressMessage => "malformed_ingress_message",
            Self::WrongBaseRoot => "wrong_base_root",
            Self::UnsupportedTransportAction => "unsupported_transport_action",
        }
    }
}

impl WindowedPatchIngressError {
    /// Returns the stable error kind.
    pub const fn kind(&self) -> WindowedPatchIngressErrorKind {
        match self {
            Self::QueueFull { .. } => WindowedPatchIngressErrorKind::QueueFull,
            Self::EventLoopClosed => WindowedPatchIngressErrorKind::EventLoopClosed,
            Self::PlayerClosed => WindowedPatchIngressErrorKind::PlayerClosed,
            Self::ReadSidecar { .. } => WindowedPatchIngressErrorKind::ReadSidecar,
            Self::ReadRestartBundle { .. } => WindowedPatchIngressErrorKind::ReadRestartBundle,
            Self::MalformedIngressMessage { .. } => {
                WindowedPatchIngressErrorKind::MalformedIngressMessage
            }
            Self::WrongBaseRoot { .. } => WindowedPatchIngressErrorKind::WrongBaseRoot,
            Self::UnsupportedTransportAction { .. } => {
                WindowedPatchIngressErrorKind::UnsupportedTransportAction
            }
        }
    }
}

impl WindowedPatchIngressWake for WinitWindowedPatchIngressWake {
    fn wake_up(&self) -> Result<(), WindowedPatchIngressWakeError> {
        self.proxy.wake_up();
        Ok(())
    }
}

impl WindowedPatchIngress {
    pub(crate) fn channel(
        proxy: EventLoopProxy,
        config: WindowedPatchIngressConfig,
    ) -> (Self, WindowedPatchIngressReceiver) {
        let (sender, receiver) = mpsc::channel();
        let shared = WindowedPatchIngressShared::new(config);
        (
            Self {
                sender,
                wake: Arc::new(WinitWindowedPatchIngressWake { proxy }),
                shared: shared.clone(),
            },
            WindowedPatchIngressReceiver {
                receiver: Arc::new(Mutex::new(receiver)),
            },
        )
    }

    #[cfg(test)]
    fn with_wake(
        sender: mpsc::Sender<WindowedPatchIngressMessage>,
        wake: Arc<dyn WindowedPatchIngressWake>,
        config: WindowedPatchIngressConfig,
    ) -> Self {
        Self {
            sender,
            wake,
            shared: WindowedPatchIngressShared::new(config),
        }
    }

    /// Returns a crate-private event-loop completion view.
    pub(crate) fn completion(&self) -> WindowedPatchIngressCompletion {
        WindowedPatchIngressCompletion {
            shared: self.shared.clone(),
        }
    }

    /// Enqueues one typed patch event for event-loop-boundary processing.
    pub fn enqueue_patch_event(
        &self,
        event: WindowedPatchEvent,
    ) -> Result<WindowedPatchIngressAccepted, WindowedPatchIngressError> {
        let source = event.source();
        let reservation = self.shared.reserve(source.clone())?;
        let envelope = WindowedPatchIngressEnvelope {
            sequence: reservation.sequence,
            event,
        };
        if self
            .sender
            .send(WindowedPatchIngressMessage::Enqueue(envelope))
            .is_err()
        {
            self.shared.event_loop_closed(reservation.sequence, source);
            return Err(WindowedPatchIngressError::EventLoopClosed);
        }
        if self.wake.wake_up().is_err() {
            self.shared.event_loop_closed(reservation.sequence, source);
            return Err(WindowedPatchIngressError::EventLoopClosed);
        }
        Ok(reservation.accepted)
    }

    /// Alias for embedding APIs that already own a typed event.
    pub fn push_event(
        &self,
        event: WindowedPatchEvent,
    ) -> Result<WindowedPatchIngressAccepted, WindowedPatchIngressError> {
        self.enqueue_patch_event(event)
    }

    /// Enqueues local patch-bundle bytes.
    pub fn push_patch_bundle_bytes(
        &self,
        bytes: impl Into<Vec<u8>>,
        source: PatchEventSource,
    ) -> Result<WindowedPatchIngressAccepted, WindowedPatchIngressError> {
        self.enqueue_patch_event(WindowedPatchEvent::ApplyBundle {
            bytes: bytes.into(),
            source,
        })
    }

    /// Enqueues local transport sidecar JSON bytes. Filesystem/path resolution
    /// remains in the event-loop-owned endpoint validation path.
    pub fn push_transport_sidecar_bytes(
        &self,
        bytes: impl Into<Vec<u8>>,
        base_dir: impl Into<PathBuf>,
        source: PatchEventSource,
    ) -> Result<WindowedPatchIngressAccepted, WindowedPatchIngressError> {
        self.enqueue_patch_event(WindowedPatchEvent::ApplyTransportSidecar {
            bytes: bytes.into(),
            base_dir: base_dir.into(),
            source,
        })
    }

    /// Enqueues a full bundle restart request.
    pub fn restart_with_bundle_bytes(
        &self,
        bytes: impl Into<Vec<u8>>,
        source: PatchEventSource,
        reason: RestartReason,
    ) -> Result<WindowedPatchIngressAccepted, WindowedPatchIngressError> {
        self.enqueue_patch_event(WindowedPatchEvent::RestartWithBundle {
            bytes: bytes.into(),
            source,
            reason,
        })
    }

    /// Reads and enqueues an `arcw run --watch` transport sidecar from the local filesystem.
    pub fn enqueue_local_sidecar_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<WindowedPatchIngressAccepted, WindowedPatchIngressError> {
        let path = path.as_ref();
        let bytes =
            std::fs::read(path).map_err(|error| WindowedPatchIngressError::ReadSidecar {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
        let base_dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        self.enqueue_local_sidecar(WindowedLocalSidecar::new(bytes, base_dir))
    }

    /// Validates a local sidecar envelope and enqueues the typed event it describes.
    pub fn enqueue_local_sidecar(
        &self,
        sidecar: WindowedLocalSidecar,
    ) -> Result<WindowedPatchIngressAccepted, WindowedPatchIngressError> {
        match Self::local_sidecar_event(sidecar) {
            Ok(event) => self.enqueue_patch_event(event),
            Err(error) => {
                self.retain_rejection(&error);
                Err(error)
            }
        }
    }

    /// Returns the latest adapter-side ingress report.
    pub fn last_report(&self) -> WindowedPatchIngressReport {
        self.shared.last_report()
    }

    fn local_sidecar_event(
        sidecar: WindowedLocalSidecar,
    ) -> Result<WindowedPatchEvent, WindowedPatchIngressError> {
        let envelope =
            NativePatchTransportEnvelope::from_json_bytes(&sidecar.bytes).map_err(|error| {
                WindowedPatchIngressError::MalformedIngressMessage {
                    event_source: sidecar.source.clone(),
                    message: error.to_string(),
                }
            })?;
        if let Some(expected) = sidecar.expected_base_content_root {
            let actual = envelope.base_content_root().map_err(|error| {
                WindowedPatchIngressError::MalformedIngressMessage {
                    event_source: sidecar.source.clone(),
                    message: error.to_string(),
                }
            })?;
            if actual != expected {
                return Err(WindowedPatchIngressError::WrongBaseRoot {
                    event_source: sidecar.source,
                    expected,
                    actual,
                });
            }
        }
        let action = envelope.action();
        if !sidecar.supported_actions.contains(action) {
            return Err(WindowedPatchIngressError::UnsupportedTransportAction {
                event_source: sidecar.source,
                action,
            });
        }
        match action {
            NativePatchTransportAction::ApplyPatch => {
                Ok(WindowedPatchEvent::ApplyTransportSidecar {
                    bytes: sidecar.bytes,
                    base_dir: sidecar.base_dir,
                    source: sidecar.source,
                })
            }
            NativePatchTransportAction::RestartPlayer => {
                let path = envelope.resolved_target_bundle_path(&sidecar.base_dir);
                let bytes = std::fs::read(&path).map_err(|error| {
                    WindowedPatchIngressError::ReadRestartBundle {
                        path,
                        message: error.to_string(),
                    }
                })?;
                Ok(WindowedPatchEvent::RestartWithBundle {
                    bytes,
                    source: sidecar.source,
                    reason: RestartReason::RestartRequiredPatch,
                })
            }
        }
    }

    fn retain_rejection(&self, error: &WindowedPatchIngressError) {
        let (source, message) = match error {
            WindowedPatchIngressError::MalformedIngressMessage {
                event_source,
                message,
            } => (event_source.clone(), message.clone()),
            WindowedPatchIngressError::WrongBaseRoot { event_source, .. }
            | WindowedPatchIngressError::UnsupportedTransportAction { event_source, .. } => {
                (event_source.clone(), error.to_string())
            }
            WindowedPatchIngressError::QueueFull { .. }
            | WindowedPatchIngressError::EventLoopClosed
            | WindowedPatchIngressError::PlayerClosed
            | WindowedPatchIngressError::ReadSidecar { .. }
            | WindowedPatchIngressError::ReadRestartBundle { .. } => {
                (PatchEventSource::EmbeddingApi, error.to_string())
            }
        };
        self.shared
            .rejected(Some(source.clone()), message.clone(), Some(error.kind()));
        if self
            .sender
            .send(WindowedPatchIngressMessage::RetainRejected { source, message })
            .is_ok()
        {
            let _ = self.wake.wake_up();
        }
    }
}

impl WindowedPatchIngressCompletion {
    pub(crate) fn accepted_by_event_loop(&self, sequence: u64, source: PatchEventSource) {
        self.shared.accepted_by_event_loop(sequence, source);
    }

    pub(crate) fn completed_at_frame_boundary(&self, processed: usize) {
        self.shared.completed_at_frame_boundary(processed);
    }

    pub(crate) fn close(&self, message: impl Into<String>) {
        self.shared.close(message);
    }
}

impl WindowedPatchIngressReceiver {
    pub(crate) fn drain(&self) -> Vec<WindowedPatchIngressMessage> {
        self.receiver
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .try_iter()
            .collect()
    }
}

impl WindowedPatchIngressShared {
    fn new(config: WindowedPatchIngressConfig) -> Self {
        let report = WindowedPatchIngressReport {
            state: WindowedPatchIngressReportState::Idle,
            sequence: None,
            source: None,
            queued: 0,
            capacity: config.capacity,
            message: "idle".to_owned(),
            error: None,
        };
        Self {
            state: Arc::new(Mutex::new(WindowedPatchIngressSharedState {
                capacity: config.capacity,
                queued: 0,
                next_sequence: 0,
                closed: false,
                report,
            })),
        }
    }

    fn reserve(
        &self,
        source: PatchEventSource,
    ) -> Result<WindowedPatchIngressReservation, WindowedPatchIngressError> {
        let mut state = self.lock();
        if state.closed {
            state.report = WindowedPatchIngressReport {
                state: WindowedPatchIngressReportState::Closed,
                sequence: None,
                source: Some(source),
                queued: state.queued,
                capacity: state.capacity,
                message: "windowed patch ingress player is closed".to_owned(),
                error: Some(WindowedPatchIngressErrorKind::PlayerClosed),
            };
            return Err(WindowedPatchIngressError::PlayerClosed);
        }
        if state.queued >= state.capacity {
            state.report = WindowedPatchIngressReport {
                state: WindowedPatchIngressReportState::Rejected,
                sequence: None,
                source: Some(source),
                queued: state.queued,
                capacity: state.capacity,
                message: "windowed patch ingress queue is full".to_owned(),
                error: Some(WindowedPatchIngressErrorKind::QueueFull),
            };
            return Err(WindowedPatchIngressError::QueueFull {
                queued: state.queued,
                capacity: state.capacity,
            });
        }
        let sequence = state.next_sequence;
        state.next_sequence = state.next_sequence.saturating_add(1);
        state.queued = state.queued.saturating_add(1);
        state.report = WindowedPatchIngressReport {
            state: WindowedPatchIngressReportState::Enqueued,
            sequence: Some(sequence),
            source: Some(source),
            queued: state.queued,
            capacity: state.capacity,
            message: "windowed patch event enqueued for event-loop delivery".to_owned(),
            error: None,
        };
        Ok(WindowedPatchIngressReservation {
            sequence,
            accepted: WindowedPatchIngressAccepted {
                sequence,
                queued: state.queued,
                capacity: state.capacity,
            },
        })
    }

    fn accepted_by_event_loop(&self, sequence: u64, source: PatchEventSource) {
        let mut state = self.lock();
        state.report = WindowedPatchIngressReport {
            state: WindowedPatchIngressReportState::AcceptedByEventLoop,
            sequence: Some(sequence),
            source: Some(source),
            queued: state.queued,
            capacity: state.capacity,
            message: "windowed patch event accepted by event loop".to_owned(),
            error: None,
        };
    }

    fn completed_at_frame_boundary(&self, processed: usize) {
        if processed == 0 {
            return;
        }
        let mut state = self.lock();
        state.queued = state.queued.saturating_sub(processed);
        state.report = WindowedPatchIngressReport {
            state: WindowedPatchIngressReportState::CompletedAtFrameBoundary,
            sequence: None,
            source: None,
            queued: state.queued,
            capacity: state.capacity,
            message: format!("{processed} windowed patch event(s) completed at frame boundary"),
            error: None,
        };
    }

    fn event_loop_closed(&self, sequence: u64, source: PatchEventSource) {
        let mut state = self.lock();
        state.queued = state.queued.saturating_sub(1);
        state.closed = true;
        state.report = WindowedPatchIngressReport {
            state: WindowedPatchIngressReportState::Rejected,
            sequence: Some(sequence),
            source: Some(source),
            queued: state.queued,
            capacity: state.capacity,
            message: "windowed patch event loop is closed".to_owned(),
            error: Some(WindowedPatchIngressErrorKind::EventLoopClosed),
        };
    }

    fn rejected(
        &self,
        source: Option<PatchEventSource>,
        message: String,
        error: Option<WindowedPatchIngressErrorKind>,
    ) {
        let mut state = self.lock();
        state.report = WindowedPatchIngressReport {
            state: WindowedPatchIngressReportState::Rejected,
            sequence: None,
            source,
            queued: state.queued,
            capacity: state.capacity,
            message,
            error,
        };
    }

    fn close(&self, message: impl Into<String>) {
        let mut state = self.lock();
        state.closed = true;
        state.queued = 0;
        state.report = WindowedPatchIngressReport {
            state: WindowedPatchIngressReportState::Closed,
            sequence: None,
            source: None,
            queued: 0,
            capacity: state.capacity,
            message: message.into(),
            error: Some(WindowedPatchIngressErrorKind::PlayerClosed),
        };
    }

    fn last_report(&self) -> WindowedPatchIngressReport {
        self.lock().report.clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, WindowedPatchIngressSharedState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl WindowedLocalSidecar {
    /// Creates a local sidecar payload with the default one-shot development source.
    pub fn new(bytes: Vec<u8>, base_dir: impl Into<PathBuf>) -> Self {
        Self {
            bytes,
            base_dir: base_dir.into(),
            source: PatchEventSource::OneShotSidecar,
            expected_base_content_root: None,
            supported_actions: WindowedPatchTransportActionSet::local_development(),
        }
    }

    /// Overrides the adapter source recorded in retained reports.
    #[must_use]
    pub fn with_source(mut self, source: PatchEventSource) -> Self {
        self.source = source;
        self
    }

    /// Requires the sidecar base root to match the active player root observed by the producer.
    #[must_use]
    pub fn with_expected_base_content_root(mut self, expected: BundleDigest) -> Self {
        self.expected_base_content_root = Some(expected);
        self
    }

    /// Restricts the sidecar actions accepted by this producer.
    #[must_use]
    pub fn with_supported_actions(
        mut self,
        supported_actions: WindowedPatchTransportActionSet,
    ) -> Self {
        self.supported_actions = supported_actions;
        self
    }
}

impl WindowedPatchTransportActionSet {
    /// Accepts the first local development actions emitted by `arcw run --watch`.
    pub const fn local_development() -> Self {
        Self {
            apply_patch: true,
            restart_player: true,
        }
    }

    /// Accepts only live-apply patch actions.
    pub const fn apply_patch_only() -> Self {
        Self {
            apply_patch: true,
            restart_player: false,
        }
    }

    /// Accepts only restart-player actions.
    pub const fn restart_player_only() -> Self {
        Self {
            apply_patch: false,
            restart_player: true,
        }
    }

    pub const fn contains(self, action: NativePatchTransportAction) -> bool {
        match action {
            NativePatchTransportAction::ApplyPatch => self.apply_patch,
            NativePatchTransportAction::RestartPlayer => self.restart_player,
        }
    }
}

impl Default for WindowedPatchIngressReport {
    fn default() -> Self {
        Self {
            state: WindowedPatchIngressReportState::Idle,
            sequence: None,
            source: None,
            queued: 0,
            capacity: DEFAULT_WINDOWED_PATCH_INGRESS_CAPACITY,
            message: "idle".to_owned(),
            error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestWake {
        count: Arc<AtomicUsize>,
        fail: bool,
    }

    impl WindowedPatchIngressWake for TestWake {
        fn wake_up(&self) -> Result<(), WindowedPatchIngressWakeError> {
            self.count.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err(WindowedPatchIngressWakeError)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn ingress_enqueues_patch_event_without_mutating_session_state() {
        let (ingress, receiver, wake_count) =
            ingress_for_test(WindowedPatchIngressConfig::default(), false);
        let event = WindowedPatchEvent::ApplyBundle {
            bytes: b"patch".to_vec(),
            source: PatchEventSource::WatchChannel,
        };

        let accepted = ingress
            .enqueue_patch_event(event.clone())
            .expect("event enqueues");

        assert_eq!(accepted.sequence, 0);
        assert_eq!(accepted.queued, 1);
        assert_eq!(wake_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            receiver.drain(),
            vec![WindowedPatchIngressMessage::Enqueue(
                WindowedPatchIngressEnvelope { sequence: 0, event }
            )]
        );
    }

    #[test]
    fn ingress_preserves_fifo_order_across_coalesced_wakeups() {
        let (ingress, receiver, _wake_count) =
            ingress_for_test(WindowedPatchIngressConfig::default(), false);
        let first = WindowedPatchEvent::ApplyBundle {
            bytes: b"one".to_vec(),
            source: PatchEventSource::WatchChannel,
        };
        let second = WindowedPatchEvent::ApplyTransportSidecar {
            bytes: b"two".to_vec(),
            base_dir: PathBuf::from("."),
            source: PatchEventSource::OneShotSidecar,
        };

        ingress
            .enqueue_patch_event(first.clone())
            .expect("first enqueues");
        ingress
            .enqueue_patch_event(second.clone())
            .expect("second enqueues");

        assert_eq!(
            receiver.drain(),
            vec![
                WindowedPatchIngressMessage::Enqueue(WindowedPatchIngressEnvelope {
                    sequence: 0,
                    event: first,
                }),
                WindowedPatchIngressMessage::Enqueue(WindowedPatchIngressEnvelope {
                    sequence: 1,
                    event: second,
                }),
            ]
        );
    }

    #[test]
    fn ingress_state_enforces_capacity_before_proxy_wake() {
        let (ingress, _receiver, wake_count) =
            ingress_for_test(WindowedPatchIngressConfig { capacity: 1 }, false);

        let first = ingress
            .enqueue_patch_event(WindowedPatchEvent::ApplyBundle {
                bytes: b"one".to_vec(),
                source: PatchEventSource::WatchChannel,
            })
            .expect("first event reserves capacity");
        assert_eq!(first.queued, 1);

        let error = ingress
            .enqueue_patch_event(WindowedPatchEvent::ApplyBundle {
                bytes: b"two".to_vec(),
                source: PatchEventSource::WatchChannel,
            })
            .expect_err("second event is rejected by ingress capacity");

        assert_eq!(
            error,
            WindowedPatchIngressError::QueueFull {
                queued: 1,
                capacity: 1,
            }
        );
        assert_eq!(wake_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            ingress.last_report().error,
            Some(WindowedPatchIngressErrorKind::QueueFull)
        );
    }

    #[test]
    fn completed_boundary_releases_backpressure_slot() {
        let (ingress, _receiver, _wake_count) =
            ingress_for_test(WindowedPatchIngressConfig { capacity: 1 }, false);
        let completion = ingress.completion();
        ingress
            .enqueue_patch_event(WindowedPatchEvent::ApplyBundle {
                bytes: b"one".to_vec(),
                source: PatchEventSource::WatchChannel,
            })
            .expect("event reserves capacity");

        completion.completed_at_frame_boundary(1);

        let next = ingress
            .enqueue_patch_event(WindowedPatchEvent::ApplyBundle {
                bytes: b"two".to_vec(),
                source: PatchEventSource::WatchChannel,
            })
            .expect("completed boundary released capacity");
        assert_eq!(next.queued, 1);
        assert_eq!(next.sequence, 1);
    }

    #[test]
    fn wake_failure_reports_event_loop_closed() {
        let (ingress, _receiver, wake_count) =
            ingress_for_test(WindowedPatchIngressConfig::default(), true);

        let error = ingress
            .enqueue_patch_event(WindowedPatchEvent::ApplyBundle {
                bytes: Vec::new(),
                source: PatchEventSource::WatchChannel,
            })
            .expect_err("closed event loop rejects");

        assert_eq!(error, WindowedPatchIngressError::EventLoopClosed);
        assert_eq!(wake_count.load(Ordering::SeqCst), 1);
        let report = ingress.last_report();
        assert_eq!(
            report.error,
            Some(WindowedPatchIngressErrorKind::EventLoopClosed)
        );
        assert_eq!(report.queued, 0);
    }

    #[test]
    fn closed_ingress_reports_typed_player_closed() {
        let (ingress, _receiver, _wake_count) =
            ingress_for_test(WindowedPatchIngressConfig::default(), false);
        ingress.completion().close("native player closed");

        let error = ingress
            .enqueue_patch_event(WindowedPatchEvent::ApplyBundle {
                bytes: Vec::new(),
                source: PatchEventSource::WatchChannel,
            })
            .expect_err("closed player rejects producer events");

        assert_eq!(error, WindowedPatchIngressError::PlayerClosed);
        let report = ingress.last_report();
        assert_eq!(report.state, WindowedPatchIngressReportState::Closed);
        assert_eq!(
            report.error,
            Some(WindowedPatchIngressErrorKind::PlayerClosed)
        );
    }

    #[test]
    fn malformed_sidecar_retains_rejected_report() {
        let (ingress, receiver, wake_count) =
            ingress_for_test(WindowedPatchIngressConfig::default(), false);

        let error = ingress
            .enqueue_local_sidecar(WindowedLocalSidecar::new(b"not json".to_vec(), "."))
            .expect_err("malformed sidecar rejects");

        assert!(matches!(
            error,
            WindowedPatchIngressError::MalformedIngressMessage { .. }
        ));
        let report = ingress.last_report();
        assert_eq!(report.state, WindowedPatchIngressReportState::Rejected);
        assert_eq!(
            report.error,
            Some(WindowedPatchIngressErrorKind::MalformedIngressMessage)
        );
        assert_eq!(wake_count.load(Ordering::SeqCst), 1);
        assert!(matches!(
            receiver.drain().as_slice(),
            [WindowedPatchIngressMessage::RetainRejected { source, message }]
                if *source == PatchEventSource::OneShotSidecar
                    && message.contains("failed to decode patch transport JSON")
        ));
    }

    #[test]
    fn wrong_base_sidecar_reports_typed_error_before_enqueue() {
        let (ingress, receiver, _wake_count) =
            ingress_for_test(WindowedPatchIngressConfig::default(), false);
        let expected = BundleDigest::of(b"expected");
        let sidecar = WindowedLocalSidecar::new(sidecar_json("apply_patch"), ".")
            .with_expected_base_content_root(expected);

        let error = ingress
            .enqueue_local_sidecar(sidecar)
            .expect_err("wrong base rejects");

        assert!(matches!(
            error,
            WindowedPatchIngressError::WrongBaseRoot {
                expected: error_expected,
                actual,
                ..
            } if error_expected == expected && actual == BundleDigest::ZERO
        ));
        assert_eq!(
            ingress.last_report().error,
            Some(WindowedPatchIngressErrorKind::WrongBaseRoot)
        );
        assert!(matches!(
            receiver.drain().as_slice(),
            [WindowedPatchIngressMessage::RetainRejected { .. }]
        ));
    }

    #[test]
    fn unsupported_transport_action_reports_typed_error() {
        let (ingress, receiver, _wake_count) =
            ingress_for_test(WindowedPatchIngressConfig::default(), false);
        let sidecar = WindowedLocalSidecar::new(sidecar_json("restart_player"), ".")
            .with_supported_actions(WindowedPatchTransportActionSet::apply_patch_only());

        let error = ingress
            .enqueue_local_sidecar(sidecar)
            .expect_err("unsupported action rejects");

        assert!(matches!(
            error,
            WindowedPatchIngressError::UnsupportedTransportAction {
                action: NativePatchTransportAction::RestartPlayer,
                ..
            }
        ));
        assert_eq!(
            ingress.last_report().error,
            Some(WindowedPatchIngressErrorKind::UnsupportedTransportAction)
        );
        assert!(matches!(
            receiver.drain().as_slice(),
            [WindowedPatchIngressMessage::RetainRejected { .. }]
        ));
    }

    fn ingress_for_test(
        config: WindowedPatchIngressConfig,
        fail_wake: bool,
    ) -> (
        WindowedPatchIngress,
        WindowedPatchIngressReceiver,
        Arc<AtomicUsize>,
    ) {
        let (sender, receiver) = mpsc::channel();
        let wake_count = Arc::new(AtomicUsize::new(0));
        let ingress = WindowedPatchIngress::with_wake(
            sender,
            Arc::new(TestWake {
                count: Arc::clone(&wake_count),
                fail: fail_wake,
            }),
            config,
        );
        (
            ingress,
            WindowedPatchIngressReceiver {
                receiver: Arc::new(Mutex::new(receiver)),
            },
            wake_count,
        )
    }

    fn sidecar_json(action: &str) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "schema_version": 1,
            "runner": "native",
            "source": "src/main.arcw",
            "target_bundle": "target.awfb",
            "patch_bundle": "update.awfb",
            "base_content_root": BundleDigest::ZERO.to_string(),
            "target_content_root": BundleDigest::ZERO.to_string(),
            "compatibility": "content-only",
            "operation_count": 0,
            "action": action,
        }))
        .expect("sidecar json encodes")
    }
}
