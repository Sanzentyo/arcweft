//! Adapter-owned ingress for live presentation-environment updates.
//!
//! Producers reserve bounded FIFO capacity and wake the native event loop. The
//! event-loop owner is the only code that mutates the live session and scene
//! planner, and completes each receipt after the frame-boundary transaction.

use arcweft_player_scene::frame::PlayerFrameError;
use arcweft_presentation::appearance::{
    EnvironmentRevision, PresentationEnvironmentFieldSet, PresentationEnvironmentValues,
};
use arcweft_runtime_driver::session::{
    PresentationEnvironmentUpdate, PresentationEnvironmentUpdateError,
};
use std::sync::{Arc, Mutex, mpsc};
use thiserror::Error;
use winit::event_loop::EventLoopProxy;

/// Default number of environment commands that may await frame-boundary
/// processing.
pub const DEFAULT_WINDOWED_ENVIRONMENT_INGRESS_CAPACITY: usize = 32;

/// Bounded environment ingress configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowedEnvironmentIngressConfig {
    /// Maximum reserved commands. A capacity of zero rejects every command.
    pub capacity: usize,
}

/// One event-loop-owned environment source mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowedEnvironmentIngressCommand {
    ReplaceProvider(PresentationEnvironmentValues),
    ClearProvider,
}

/// Producer handle for a running native player's environment source.
#[derive(Clone)]
pub struct WindowedEnvironmentIngress {
    sender: mpsc::Sender<WindowedEnvironmentIngressEnvelope>,
    wake: Arc<dyn WindowedEnvironmentIngressWake>,
    shared: WindowedEnvironmentIngressShared,
}

/// Completion receipt for one accepted environment command.
pub struct WindowedEnvironmentIngressReceipt {
    sequence: u64,
    completion: mpsc::Receiver<WindowedEnvironmentCompletionResult>,
}

/// Latest producer-visible environment ingress state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowedEnvironmentIngressReport {
    state: WindowedEnvironmentIngressReportState,
    sequence: Option<u64>,
    command: Option<WindowedEnvironmentIngressCommand>,
    queued: usize,
    capacity: usize,
    revision: Option<EnvironmentRevision>,
    changed_fields: PresentationEnvironmentFieldSet,
    error: Option<WindowedEnvironmentUpdateErrorKind>,
}

/// Stable ingress lifecycle state for diagnostics and adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowedEnvironmentIngressReportState {
    Idle,
    Enqueued,
    AcceptedByEventLoop,
    CompletedAtFrameBoundary,
    Rejected,
    Closed,
}

/// Stable environment update error discriminator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowedEnvironmentUpdateErrorKind {
    QueueFull,
    SequenceOverflow,
    EventLoopClosed,
    PlayerClosed,
    RevisionOverflow,
    FieldRevisionOverflow,
    PlayerFrame,
}

/// Failure to submit or apply a native environment command.
#[derive(Debug, Error)]
pub enum WindowedEnvironmentUpdateError {
    #[error("windowed environment ingress queue is full: queued {queued}, capacity {capacity}")]
    QueueFull { queued: usize, capacity: usize },
    #[error("windowed environment ingress sequence overflow")]
    SequenceOverflow,
    #[error("windowed environment ingress event loop is closed")]
    EventLoopClosed,
    #[error("windowed environment ingress player is closed")]
    PlayerClosed,
    #[error(transparent)]
    Session(#[from] PresentationEnvironmentUpdateError),
    #[error("windowed environment player update failed: {0}")]
    PlayerFrame(#[from] PlayerFrameError),
}

pub(crate) type WindowedEnvironmentCompletionResult =
    Result<PresentationEnvironmentUpdate, WindowedEnvironmentUpdateError>;

pub(crate) struct WindowedEnvironmentIngressEnvelope {
    sequence: u64,
    command: WindowedEnvironmentIngressCommand,
    completion: mpsc::Sender<WindowedEnvironmentCompletionResult>,
}

pub(crate) struct WindowedEnvironmentIngressReceiver {
    receiver: mpsc::Receiver<WindowedEnvironmentIngressEnvelope>,
}

#[derive(Clone, Debug)]
pub(crate) struct WindowedEnvironmentIngressCompletion {
    shared: WindowedEnvironmentIngressShared,
}

#[derive(Clone, Debug)]
struct WindowedEnvironmentIngressShared {
    state: Arc<Mutex<WindowedEnvironmentIngressSharedState>>,
}

#[derive(Clone, Copy, Debug)]
struct WindowedEnvironmentIngressSharedState {
    capacity: usize,
    queued: usize,
    next_sequence: u64,
    closed: bool,
    report: WindowedEnvironmentIngressReport,
}

trait WindowedEnvironmentIngressWake: Send + Sync {
    fn wake_up(&self);
}

#[derive(Debug)]
struct WinitWindowedEnvironmentIngressWake {
    proxy: EventLoopProxy,
}

impl Default for WindowedEnvironmentIngressConfig {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_WINDOWED_ENVIRONMENT_INGRESS_CAPACITY,
        }
    }
}

impl WindowedEnvironmentIngress {
    pub(crate) fn channel(
        proxy: EventLoopProxy,
        config: WindowedEnvironmentIngressConfig,
    ) -> (Self, WindowedEnvironmentIngressReceiver) {
        let (sender, receiver) = mpsc::channel();
        let shared = WindowedEnvironmentIngressShared::new(config);
        (
            Self {
                sender,
                wake: Arc::new(WinitWindowedEnvironmentIngressWake { proxy }),
                shared,
            },
            WindowedEnvironmentIngressReceiver { receiver },
        )
    }

    /// Replaces the complete four-field platform provider snapshot.
    pub fn replace_provider(
        &self,
        values: PresentationEnvironmentValues,
    ) -> Result<WindowedEnvironmentIngressReceipt, WindowedEnvironmentUpdateError> {
        self.submit(WindowedEnvironmentIngressCommand::ReplaceProvider(values))
    }

    /// Clears the platform provider so theme/default precedence becomes visible.
    pub fn clear_provider(
        &self,
    ) -> Result<WindowedEnvironmentIngressReceipt, WindowedEnvironmentUpdateError> {
        self.submit(WindowedEnvironmentIngressCommand::ClearProvider)
    }

    /// Returns the latest producer-visible transport or boundary report.
    pub fn report(&self) -> WindowedEnvironmentIngressReport {
        self.shared.report()
    }

    fn submit(
        &self,
        command: WindowedEnvironmentIngressCommand,
    ) -> Result<WindowedEnvironmentIngressReceipt, WindowedEnvironmentUpdateError> {
        let receipt = self.shared.reserve_and_send(&self.sender, command)?;
        self.wake.wake_up();
        Ok(receipt)
    }

    pub(crate) fn completion(&self) -> WindowedEnvironmentIngressCompletion {
        WindowedEnvironmentIngressCompletion {
            shared: self.shared.clone(),
        }
    }

    #[cfg(test)]
    fn with_wake(
        sender: mpsc::Sender<WindowedEnvironmentIngressEnvelope>,
        wake: Arc<dyn WindowedEnvironmentIngressWake>,
        config: WindowedEnvironmentIngressConfig,
    ) -> Self {
        Self {
            sender,
            wake,
            shared: WindowedEnvironmentIngressShared::new(config),
        }
    }
}

impl WindowedEnvironmentIngressReceipt {
    /// Monotonic FIFO reservation sequence for this command.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Returns the boundary result when ready, or `None` while still queued.
    pub fn try_wait(&self) -> Option<WindowedEnvironmentCompletionResult> {
        match self.completion.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err(WindowedEnvironmentUpdateError::PlayerClosed))
            }
        }
    }

    /// Blocks until the event-loop owner completes this command.
    pub fn wait(self) -> WindowedEnvironmentCompletionResult {
        self.completion
            .recv()
            .unwrap_or(Err(WindowedEnvironmentUpdateError::PlayerClosed))
    }
}

impl WindowedEnvironmentIngressEnvelope {
    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) const fn command(&self) -> WindowedEnvironmentIngressCommand {
        self.command
    }

    pub(crate) fn complete(self, result: WindowedEnvironmentCompletionResult) {
        let _ = self.completion.send(result);
    }

    pub(crate) fn close(self) {
        self.complete(Err(WindowedEnvironmentUpdateError::PlayerClosed));
    }
}

impl WindowedEnvironmentIngressReceiver {
    pub(crate) fn drain(&self) -> Vec<WindowedEnvironmentIngressEnvelope> {
        self.receiver.try_iter().collect()
    }
}

impl WindowedEnvironmentIngressCompletion {
    pub(crate) fn accepted_by_event_loop(
        &self,
        sequence: u64,
        command: WindowedEnvironmentIngressCommand,
    ) {
        self.shared.accepted_by_event_loop(sequence, command);
    }

    pub(crate) fn completed_at_frame_boundary(
        &self,
        sequence: u64,
        command: WindowedEnvironmentIngressCommand,
        result: &WindowedEnvironmentCompletionResult,
    ) {
        self.shared
            .completed_at_frame_boundary(sequence, command, result);
    }

    pub(crate) fn close(&self) {
        self.shared.close();
    }
}

impl WindowedEnvironmentIngressShared {
    fn new(config: WindowedEnvironmentIngressConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(WindowedEnvironmentIngressSharedState {
                capacity: config.capacity,
                queued: 0,
                next_sequence: 0,
                closed: false,
                report: WindowedEnvironmentIngressReport::idle(config.capacity),
            })),
        }
    }

    fn reserve_and_send(
        &self,
        sender: &mpsc::Sender<WindowedEnvironmentIngressEnvelope>,
        command: WindowedEnvironmentIngressCommand,
    ) -> Result<WindowedEnvironmentIngressReceipt, WindowedEnvironmentUpdateError> {
        let mut state = self.lock();
        if state.closed {
            state.report = WindowedEnvironmentIngressReport::rejected(
                WindowedEnvironmentIngressReportState::Closed,
                None,
                Some(command),
                state.queued,
                state.capacity,
                WindowedEnvironmentUpdateErrorKind::PlayerClosed,
            );
            return Err(WindowedEnvironmentUpdateError::PlayerClosed);
        }
        if state.queued >= state.capacity {
            state.report = WindowedEnvironmentIngressReport::rejected(
                WindowedEnvironmentIngressReportState::Rejected,
                None,
                Some(command),
                state.queued,
                state.capacity,
                WindowedEnvironmentUpdateErrorKind::QueueFull,
            );
            return Err(WindowedEnvironmentUpdateError::QueueFull {
                queued: state.queued,
                capacity: state.capacity,
            });
        }
        let sequence = state.next_sequence;
        let Some(next_sequence) = sequence.checked_add(1) else {
            state.report = WindowedEnvironmentIngressReport::rejected(
                WindowedEnvironmentIngressReportState::Rejected,
                None,
                Some(command),
                state.queued,
                state.capacity,
                WindowedEnvironmentUpdateErrorKind::SequenceOverflow,
            );
            return Err(WindowedEnvironmentUpdateError::SequenceOverflow);
        };
        let (completion, receiver) = mpsc::channel();
        let envelope = WindowedEnvironmentIngressEnvelope {
            sequence,
            command,
            completion,
        };
        if sender.send(envelope).is_err() {
            state.closed = true;
            state.report = WindowedEnvironmentIngressReport::rejected(
                WindowedEnvironmentIngressReportState::Rejected,
                Some(sequence),
                Some(command),
                state.queued,
                state.capacity,
                WindowedEnvironmentUpdateErrorKind::EventLoopClosed,
            );
            return Err(WindowedEnvironmentUpdateError::EventLoopClosed);
        }
        state.next_sequence = next_sequence;
        state.queued += 1;
        state.report = WindowedEnvironmentIngressReport {
            state: WindowedEnvironmentIngressReportState::Enqueued,
            sequence: Some(sequence),
            command: Some(command),
            queued: state.queued,
            capacity: state.capacity,
            revision: None,
            changed_fields: PresentationEnvironmentFieldSet::NONE,
            error: None,
        };
        Ok(WindowedEnvironmentIngressReceipt {
            sequence,
            completion: receiver,
        })
    }

    fn accepted_by_event_loop(&self, sequence: u64, command: WindowedEnvironmentIngressCommand) {
        let mut state = self.lock();
        state.report = WindowedEnvironmentIngressReport {
            state: WindowedEnvironmentIngressReportState::AcceptedByEventLoop,
            sequence: Some(sequence),
            command: Some(command),
            queued: state.queued,
            capacity: state.capacity,
            revision: None,
            changed_fields: PresentationEnvironmentFieldSet::NONE,
            error: None,
        };
    }

    fn completed_at_frame_boundary(
        &self,
        sequence: u64,
        command: WindowedEnvironmentIngressCommand,
        result: &WindowedEnvironmentCompletionResult,
    ) {
        let mut state = self.lock();
        state.queued = state.queued.saturating_sub(1);
        let (revision, changed_fields, error) = match result {
            Ok(update) => (
                Some(update.current().revision()),
                update.effective_changed_fields(),
                None,
            ),
            Err(error) => (
                None,
                PresentationEnvironmentFieldSet::NONE,
                Some(error.kind()),
            ),
        };
        state.report = WindowedEnvironmentIngressReport {
            state: if error.is_some() {
                WindowedEnvironmentIngressReportState::Rejected
            } else {
                WindowedEnvironmentIngressReportState::CompletedAtFrameBoundary
            },
            sequence: Some(sequence),
            command: Some(command),
            queued: state.queued,
            capacity: state.capacity,
            revision,
            changed_fields,
            error,
        };
    }

    fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        state.queued = 0;
        state.report = WindowedEnvironmentIngressReport::rejected(
            WindowedEnvironmentIngressReportState::Closed,
            None,
            None,
            0,
            state.capacity,
            WindowedEnvironmentUpdateErrorKind::PlayerClosed,
        );
    }

    fn report(&self) -> WindowedEnvironmentIngressReport {
        self.lock().report
    }

    #[cfg(test)]
    fn set_next_sequence(&self, next_sequence: u64) {
        self.lock().next_sequence = next_sequence;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, WindowedEnvironmentIngressSharedState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl WindowedEnvironmentIngressReport {
    const fn idle(capacity: usize) -> Self {
        Self {
            state: WindowedEnvironmentIngressReportState::Idle,
            sequence: None,
            command: None,
            queued: 0,
            capacity,
            revision: None,
            changed_fields: PresentationEnvironmentFieldSet::NONE,
            error: None,
        }
    }

    const fn rejected(
        state: WindowedEnvironmentIngressReportState,
        sequence: Option<u64>,
        command: Option<WindowedEnvironmentIngressCommand>,
        queued: usize,
        capacity: usize,
        error: WindowedEnvironmentUpdateErrorKind,
    ) -> Self {
        Self {
            state,
            sequence,
            command,
            queued,
            capacity,
            revision: None,
            changed_fields: PresentationEnvironmentFieldSet::NONE,
            error: Some(error),
        }
    }

    pub const fn state(self) -> WindowedEnvironmentIngressReportState {
        self.state
    }

    pub const fn sequence(self) -> Option<u64> {
        self.sequence
    }

    pub const fn command(self) -> Option<WindowedEnvironmentIngressCommand> {
        self.command
    }

    pub const fn queued(self) -> usize {
        self.queued
    }

    pub const fn capacity(self) -> usize {
        self.capacity
    }

    pub const fn revision(self) -> Option<EnvironmentRevision> {
        self.revision
    }

    pub const fn changed_fields(self) -> PresentationEnvironmentFieldSet {
        self.changed_fields
    }

    pub const fn error(self) -> Option<WindowedEnvironmentUpdateErrorKind> {
        self.error
    }
}

impl WindowedEnvironmentIngressReportState {
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

impl WindowedEnvironmentUpdateErrorKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::QueueFull => "queue_full",
            Self::SequenceOverflow => "sequence_overflow",
            Self::EventLoopClosed => "event_loop_closed",
            Self::PlayerClosed => "player_closed",
            Self::RevisionOverflow => "revision_overflow",
            Self::FieldRevisionOverflow => "field_revision_overflow",
            Self::PlayerFrame => "player_frame",
        }
    }
}

impl WindowedEnvironmentUpdateError {
    pub const fn kind(&self) -> WindowedEnvironmentUpdateErrorKind {
        match self {
            Self::QueueFull { .. } => WindowedEnvironmentUpdateErrorKind::QueueFull,
            Self::SequenceOverflow => WindowedEnvironmentUpdateErrorKind::SequenceOverflow,
            Self::EventLoopClosed => WindowedEnvironmentUpdateErrorKind::EventLoopClosed,
            Self::PlayerClosed => WindowedEnvironmentUpdateErrorKind::PlayerClosed,
            Self::Session(PresentationEnvironmentUpdateError::RevisionOverflow) => {
                WindowedEnvironmentUpdateErrorKind::RevisionOverflow
            }
            Self::Session(PresentationEnvironmentUpdateError::FieldRevisionOverflow { .. }) => {
                WindowedEnvironmentUpdateErrorKind::FieldRevisionOverflow
            }
            Self::PlayerFrame(_) => WindowedEnvironmentUpdateErrorKind::PlayerFrame,
        }
    }
}

impl WindowedEnvironmentIngressWake for WinitWindowedEnvironmentIngressWake {
    fn wake_up(&self) {
        self.proxy.wake_up();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_presentation::appearance::{
        ColorScheme, ContrastPreference, PresentationEnvironmentOverrides, TextScaleMilli,
    };
    use arcweft_runtime_driver::session::SessionEnvironmentState;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Barrier, Mutex};
    use std::thread;

    struct TestWake(AtomicUsize);

    impl WindowedEnvironmentIngressWake for TestWake {
        fn wake_up(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn environment_fifo_order_matches_reserved_sequence_across_threads() {
        let (ingress, receiver, wake) =
            ingress_for_test(WindowedEnvironmentIngressConfig { capacity: 16 });
        let barrier = Arc::new(Barrier::new(9));
        let receipts = Arc::new(Mutex::new(Vec::new()));
        let threads = (0..8)
            .map(|index| {
                let ingress = ingress.clone();
                let barrier = Arc::clone(&barrier);
                let receipts = Arc::clone(&receipts);
                thread::spawn(move || {
                    barrier.wait();
                    let receipt = ingress
                        .replace_provider(values(1_000 + index))
                        .expect("command reserves");
                    receipts
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push(receipt.sequence());
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        for thread in threads {
            thread.join().expect("producer joins");
        }

        let sequences = receiver
            .drain()
            .into_iter()
            .map(|envelope| envelope.sequence())
            .collect::<Vec<_>>();
        assert_eq!(sequences, (0..8).collect::<Vec<_>>());
        let mut receipt_sequences = receipts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        receipt_sequences.sort_unstable();
        assert_eq!(receipt_sequences, sequences);
        assert_eq!(wake.0.load(Ordering::SeqCst), 8);
    }

    #[test]
    fn queue_exact_capacity_and_one_over() {
        let (ingress, _receiver, _) =
            ingress_for_test(WindowedEnvironmentIngressConfig { capacity: 2 });
        ingress.clear_provider().expect("first command reserves");
        ingress
            .replace_provider(values(1_100))
            .expect("second command reserves");
        assert!(matches!(
            ingress.clear_provider(),
            Err(WindowedEnvironmentUpdateError::QueueFull {
                queued: 2,
                capacity: 2
            })
        ));
        assert_eq!(
            ingress.report().error(),
            Some(WindowedEnvironmentUpdateErrorKind::QueueFull)
        );
    }

    #[test]
    fn sequence_overflow_rejects_without_enqueue() {
        let (ingress, receiver, wake) =
            ingress_for_test(WindowedEnvironmentIngressConfig::default());
        ingress.shared.set_next_sequence(u64::MAX);

        assert!(matches!(
            ingress.clear_provider(),
            Err(WindowedEnvironmentUpdateError::SequenceOverflow)
        ));
        assert!(receiver.drain().is_empty());
        assert_eq!(ingress.report().queued(), 0);
        assert_eq!(wake.0.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn receipt_reports_exact_session_update() {
        let (ingress, receiver, _) = ingress_for_test(WindowedEnvironmentIngressConfig::default());
        let receipt = ingress
            .replace_provider(values(1_250))
            .expect("command reserves");
        assert!(receipt.try_wait().is_none());
        let envelope = receiver.drain().pop().expect("command drains");
        let mut environment =
            SessionEnvironmentState::new(None, PresentationEnvironmentOverrides::empty());
        let update = environment
            .replace_provider(values(1_250))
            .expect("session update succeeds");
        envelope.complete(Ok(update));

        assert_eq!(receipt.wait().expect("receipt succeeds"), update);
    }

    #[test]
    fn shutdown_completes_queued_and_rejects_future_updates() {
        let (ingress, receiver, _) = ingress_for_test(WindowedEnvironmentIngressConfig::default());
        let receipt = ingress.clear_provider().expect("command reserves");
        ingress.completion().close();
        for envelope in receiver.drain() {
            envelope.close();
        }

        assert!(matches!(
            receipt.wait(),
            Err(WindowedEnvironmentUpdateError::PlayerClosed)
        ));
        assert!(matches!(
            ingress.clear_provider(),
            Err(WindowedEnvironmentUpdateError::PlayerClosed)
        ));
    }

    #[test]
    fn dropping_producer_handle_does_not_shutdown_player() {
        let (ingress, receiver, _) = ingress_for_test(WindowedEnvironmentIngressConfig::default());
        let retained = ingress.clone();
        drop(ingress);

        retained
            .clear_provider()
            .expect("retained producer submits");
        assert_eq!(receiver.drain().len(), 1);
    }

    fn ingress_for_test(
        config: WindowedEnvironmentIngressConfig,
    ) -> (
        WindowedEnvironmentIngress,
        WindowedEnvironmentIngressReceiver,
        Arc<TestWake>,
    ) {
        let (sender, receiver) = mpsc::channel();
        let wake = Arc::new(TestWake(AtomicUsize::new(0)));
        let ingress =
            WindowedEnvironmentIngress::with_wake(sender, Arc::<TestWake>::clone(&wake), config);
        (
            ingress,
            WindowedEnvironmentIngressReceiver { receiver },
            wake,
        )
    }

    fn values(text_scale: u16) -> PresentationEnvironmentValues {
        PresentationEnvironmentValues::new(
            ColorScheme::Dark,
            ContrastPreference::Standard,
            false,
            TextScaleMilli::try_new(text_scale).expect("test text scale is valid"),
        )
    }
}
