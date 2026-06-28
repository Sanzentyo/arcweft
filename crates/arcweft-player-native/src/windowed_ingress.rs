//! Adapter-owned ingress for live patching a running windowed native player.
//!
//! The ingress handle is intentionally narrower than a watcher, socket server, or
//! release-policy verifier. Producers hand this layer already verified local bytes
//! or local sidecar files. The handle only validates the sidecar envelope enough to
//! create typed [`WindowedPatchEvent`] values and wakes the event loop; the
//! event-loop owner remains the only code path that mutates session/catalog state.

use crate::patch_endpoint::{NativePatchTransportAction, NativePatchTransportEnvelope};
use crate::windowed_patch::{PatchEventSource, RestartReason, WindowedPatchEvent};
use arcweft_bundle::container::BundleDigest;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use thiserror::Error;
use winit::event_loop::EventLoopProxy;

/// Narrow enqueue handle that can be cloned into local development producers.
#[derive(Clone)]
pub struct WindowedPatchIngress {
    sender: mpsc::Sender<WindowedPatchIngressMessage>,
    wake: Arc<dyn WindowedPatchIngressWake>,
    report: Arc<Mutex<WindowedPatchIngressReport>>,
}

#[derive(Debug)]
pub(crate) struct WindowedPatchIngressReceiver {
    receiver: mpsc::Receiver<WindowedPatchIngressMessage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WindowedPatchIngressMessage {
    Enqueue(WindowedPatchEvent),
    RetainRejected {
        source: PatchEventSource,
        message: String,
    },
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

/// Last adapter-side ingress report visible before a patch reaches the frame boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowedPatchIngressReport {
    pub state: WindowedPatchIngressState,
    pub source: Option<PatchEventSource>,
    pub message: String,
}

/// Adapter-side ingress status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowedPatchIngressState {
    Idle,
    Queued,
    Rejected,
}

/// Adapter-side ingress errors.
#[derive(Debug, Error)]
pub enum WindowedPatchIngressError {
    #[error("windowed player event loop is disconnected")]
    Disconnected,
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

trait WindowedPatchIngressWake: Send + Sync {
    fn wake_up(&self);
}

#[derive(Debug)]
struct WinitWindowedPatchIngressWake {
    proxy: EventLoopProxy,
}

impl WindowedPatchIngressWake for WinitWindowedPatchIngressWake {
    fn wake_up(&self) {
        self.proxy.wake_up();
    }
}

impl WindowedPatchIngress {
    pub(crate) fn channel(proxy: EventLoopProxy) -> (Self, WindowedPatchIngressReceiver) {
        let (sender, receiver) = mpsc::channel();
        (
            Self {
                sender,
                wake: Arc::new(WinitWindowedPatchIngressWake { proxy }),
                report: Arc::new(Mutex::new(WindowedPatchIngressReport::default())),
            },
            WindowedPatchIngressReceiver { receiver },
        )
    }

    #[cfg(test)]
    fn with_wake(
        sender: mpsc::Sender<WindowedPatchIngressMessage>,
        wake: Arc<dyn WindowedPatchIngressWake>,
    ) -> Self {
        Self {
            sender,
            wake,
            report: Arc::new(Mutex::new(WindowedPatchIngressReport::default())),
        }
    }

    /// Enqueues one typed patch event for event-loop-boundary processing.
    pub fn enqueue_patch_event(
        &self,
        event: WindowedPatchEvent,
    ) -> Result<WindowedPatchIngressReport, WindowedPatchIngressError> {
        let source = event.source();
        let report = WindowedPatchIngressReport {
            state: WindowedPatchIngressState::Queued,
            source: Some(source),
            message: "patch event queued for windowed frame boundary".to_owned(),
        };
        self.send_message(WindowedPatchIngressMessage::Enqueue(event), report)
    }

    /// Reads and enqueues an `arcw run --watch` transport sidecar from the local filesystem.
    pub fn enqueue_local_sidecar_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<WindowedPatchIngressReport, WindowedPatchIngressError> {
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
    ) -> Result<WindowedPatchIngressReport, WindowedPatchIngressError> {
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
        self.report
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
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

    fn send_message(
        &self,
        message: WindowedPatchIngressMessage,
        report: WindowedPatchIngressReport,
    ) -> Result<WindowedPatchIngressReport, WindowedPatchIngressError> {
        if self.sender.send(message).is_err() {
            let rejected = WindowedPatchIngressReport {
                state: WindowedPatchIngressState::Rejected,
                source: report.source,
                message: "windowed player event loop is disconnected".to_owned(),
            };
            self.store_report(rejected);
            return Err(WindowedPatchIngressError::Disconnected);
        }
        self.store_report(report.clone());
        self.wake.wake_up();
        Ok(report)
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
            WindowedPatchIngressError::Disconnected
            | WindowedPatchIngressError::ReadSidecar { .. }
            | WindowedPatchIngressError::ReadRestartBundle { .. } => {
                (PatchEventSource::EmbeddingApi, error.to_string())
            }
        };
        let report = WindowedPatchIngressReport {
            state: WindowedPatchIngressState::Rejected,
            source: Some(source.clone()),
            message: message.clone(),
        };
        self.store_report(report);
        if self
            .sender
            .send(WindowedPatchIngressMessage::RetainRejected { source, message })
            .is_ok()
        {
            self.wake.wake_up();
        }
    }

    fn store_report(&self, report: WindowedPatchIngressReport) {
        *self
            .report
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = report;
    }
}

impl WindowedPatchIngressReceiver {
    pub(crate) fn drain(&self) -> impl Iterator<Item = WindowedPatchIngressMessage> + '_ {
        self.receiver.try_iter()
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
            state: WindowedPatchIngressState::Idle,
            source: None,
            message: "idle".to_owned(),
        }
    }
}

impl WindowedPatchIngressState {
    /// Stable label for logs, tests, and developer tooling.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Queued => "queued",
            Self::Rejected => "rejected",
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
    }

    impl WindowedPatchIngressWake for TestWake {
        fn wake_up(&self) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn ingress_enqueues_patch_event_without_mutating_session_state() {
        let (ingress, receiver, wake_count) = ingress_for_test();
        let event = WindowedPatchEvent::ApplyBundle {
            bytes: b"patch".to_vec(),
            source: PatchEventSource::WatchChannel,
        };

        let report = ingress
            .enqueue_patch_event(event.clone())
            .expect("event enqueues");

        assert_eq!(report.state, WindowedPatchIngressState::Queued);
        assert_eq!(wake_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            receiver.drain().collect::<Vec<_>>(),
            vec![WindowedPatchIngressMessage::Enqueue(event)]
        );
    }

    #[test]
    fn ingress_preserves_fifo_order_across_coalesced_wakeups() {
        let (ingress, receiver, _wake_count) = ingress_for_test();
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
            receiver.drain().collect::<Vec<_>>(),
            vec![
                WindowedPatchIngressMessage::Enqueue(first),
                WindowedPatchIngressMessage::Enqueue(second),
            ]
        );
    }

    #[test]
    fn disconnected_ingress_reports_typed_error() {
        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        let wake_count = Arc::new(AtomicUsize::new(0));
        let ingress = WindowedPatchIngress::with_wake(
            sender,
            Arc::new(TestWake {
                count: Arc::clone(&wake_count),
            }),
        );

        let error = ingress
            .enqueue_patch_event(WindowedPatchEvent::ApplyBundle {
                bytes: Vec::new(),
                source: PatchEventSource::WatchChannel,
            })
            .expect_err("closed player reports disconnected");

        assert!(matches!(error, WindowedPatchIngressError::Disconnected));
        assert_eq!(
            ingress.last_report().state,
            WindowedPatchIngressState::Rejected
        );
        assert_eq!(wake_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn malformed_sidecar_retains_rejected_report() {
        let (ingress, receiver, wake_count) = ingress_for_test();

        let error = ingress
            .enqueue_local_sidecar(WindowedLocalSidecar::new(b"not json".to_vec(), "."))
            .expect_err("malformed sidecar rejects");

        assert!(matches!(
            error,
            WindowedPatchIngressError::MalformedIngressMessage { .. }
        ));
        assert_eq!(
            ingress.last_report().state,
            WindowedPatchIngressState::Rejected
        );
        assert_eq!(wake_count.load(Ordering::SeqCst), 1);
        assert!(matches!(
            receiver.drain().collect::<Vec<_>>().as_slice(),
            [WindowedPatchIngressMessage::RetainRejected { source, message }]
                if *source == PatchEventSource::OneShotSidecar
                    && message.contains("failed to decode patch transport JSON")
        ));
    }

    #[test]
    fn wrong_base_sidecar_reports_typed_error_before_enqueue() {
        let (ingress, receiver, _wake_count) = ingress_for_test();
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
            ingress.last_report().state,
            WindowedPatchIngressState::Rejected
        );
        assert!(matches!(
            receiver.drain().collect::<Vec<_>>().as_slice(),
            [WindowedPatchIngressMessage::RetainRejected { .. }]
        ));
    }

    #[test]
    fn unsupported_transport_action_reports_typed_error() {
        let (ingress, receiver, _wake_count) = ingress_for_test();
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
            ingress.last_report().state,
            WindowedPatchIngressState::Rejected
        );
        assert!(matches!(
            receiver.drain().collect::<Vec<_>>().as_slice(),
            [WindowedPatchIngressMessage::RetainRejected { .. }]
        ));
    }

    fn ingress_for_test() -> (
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
            }),
        );
        (
            ingress,
            WindowedPatchIngressReceiver { receiver },
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
