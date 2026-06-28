//! Typed patch event queue for windowed native players.
//!
//! The queue is deliberately independent of filesystem/watch/socket transport
//! so adapters can inject typed events into the event-loop-owned runtime state.

use arcweft_bundle::patch::PatchCompatibility;
use std::collections::VecDeque;
use thiserror::Error;

/// Event-loop boundary reached by the native window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameBoundary {
    BeforeRuntimeStep,
    AfterRuntimeStep,
    BeforeRender,
    AfterRenderSubmitted,
}

impl FrameBoundary {
    /// Returns whether session/catalog mutation is allowed at this boundary.
    pub const fn is_patch_commit_safe(self) -> bool {
        matches!(self, Self::AfterRenderSubmitted)
    }

    /// Stable label used by deterministic smoke reports and regeneration
    /// manifests.
    pub const fn label(self) -> &'static str {
        match self {
            Self::BeforeRuntimeStep => "before_runtime_step",
            Self::AfterRuntimeStep => "after_runtime_step",
            Self::BeforeRender => "before_render",
            Self::AfterRenderSubmitted => "after_render_submitted",
        }
    }
}

/// Patch event delivered to a windowed native event loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WindowedPatchEvent {
    ApplyBundle {
        bytes: Vec<u8>,
        source: PatchEventSource,
    },
    ApplyTransportSidecar {
        bytes: Vec<u8>,
        source: PatchEventSource,
    },
    RestartWithBundle {
        bytes: Vec<u8>,
        reason: RestartReason,
    },
}

/// Source adapter that produced a patch event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatchEventSource {
    WatchChannel,
    OneShotSidecar,
    FileWatch,
    LocalSocket,
    EmbeddingApi,
}

/// Why a windowed session restart was requested.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RestartReason {
    CodeGenerationalUnsupported,
    RestartRequiredPatch,
    Manual,
}

/// Windowed patch state-machine label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowedPatchState {
    Idle,
    Queued,
    Preparing,
    ReadyToCommit,
    Committing,
    RestartingSession,
    Applied,
    Rejected,
}

/// Last patch report retained for debug overlays, logs, and tooling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowedPatchReport {
    pub state: WindowedPatchState,
    pub source: Option<PatchEventSource>,
    pub message: String,
    pub compatibility: Option<PatchCompatibility>,
}

/// Queue operation failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WindowedPatchError {
    #[error("cannot commit a windowed patch at frame boundary {0:?}")]
    UnsafeBoundary(FrameBoundary),
    #[error("no patch is queued")]
    NoQueuedPatch,
}

/// FIFO event queue with the latest user-visible patch report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowedPatchQueue {
    events: VecDeque<WindowedPatchEvent>,
    report: WindowedPatchReport,
}

impl Default for WindowedPatchQueue {
    fn default() -> Self {
        Self {
            events: VecDeque::new(),
            report: WindowedPatchReport {
                state: WindowedPatchState::Idle,
                source: None,
                message: "idle".to_owned(),
                compatibility: None,
            },
        }
    }
}

impl WindowedPatchEvent {
    /// Returns the event source.
    pub fn source(&self) -> PatchEventSource {
        match self {
            Self::ApplyBundle { source, .. } | Self::ApplyTransportSidecar { source, .. } => {
                source.clone()
            }
            Self::RestartWithBundle { .. } => PatchEventSource::EmbeddingApi,
        }
    }
}

impl PatchEventSource {
    /// Stable label used by deterministic smoke reports and regeneration
    /// manifests.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::WatchChannel => "watch_channel",
            Self::OneShotSidecar => "one_shot_sidecar",
            Self::FileWatch => "file_watch",
            Self::LocalSocket => "local_socket",
            Self::EmbeddingApi => "embedding_api",
        }
    }
}

impl RestartReason {
    /// Stable label used by deterministic smoke reports and regeneration
    /// manifests.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::CodeGenerationalUnsupported => "code_generational_unsupported",
            Self::RestartRequiredPatch => "restart_required_patch",
            Self::Manual => "manual",
        }
    }
}

impl WindowedPatchState {
    /// Stable label used by deterministic smoke reports and regeneration
    /// manifests.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Queued => "queued",
            Self::Preparing => "preparing",
            Self::ReadyToCommit => "ready_to_commit",
            Self::Committing => "committing",
            Self::RestartingSession => "restarting_session",
            Self::Applied => "applied",
            Self::Rejected => "rejected",
        }
    }
}

impl WindowedPatchQueue {
    /// Enqueues a patch event for event-loop-boundary processing.
    pub fn push(&mut self, event: WindowedPatchEvent) {
        let source = event.source();
        self.events.push_back(event);
        self.report = WindowedPatchReport {
            state: WindowedPatchState::Queued,
            source: Some(source),
            message: "patch queued".to_owned(),
            compatibility: None,
        };
    }

    /// Pops the oldest event when the frame boundary is safe for mutation.
    pub fn pop_ready(
        &mut self,
        boundary: FrameBoundary,
    ) -> Result<WindowedPatchEvent, WindowedPatchError> {
        if !boundary.is_patch_commit_safe() {
            return Err(WindowedPatchError::UnsafeBoundary(boundary));
        }
        self.events
            .pop_front()
            .ok_or(WindowedPatchError::NoQueuedPatch)
    }

    /// Records that patch preparation has started.
    pub fn preparing(&mut self, source: PatchEventSource, message: impl Into<String>) {
        self.report = WindowedPatchReport {
            state: WindowedPatchState::Preparing,
            source: Some(source),
            message: message.into(),
            compatibility: None,
        };
    }

    /// Records that a patch has been prepared and is awaiting commit.
    pub fn ready_to_commit(
        &mut self,
        source: PatchEventSource,
        compatibility: PatchCompatibility,
        message: impl Into<String>,
    ) {
        self.report = WindowedPatchReport {
            state: WindowedPatchState::ReadyToCommit,
            source: Some(source),
            message: message.into(),
            compatibility: Some(compatibility),
        };
    }

    /// Records a successful live application.
    pub fn applied(
        &mut self,
        source: PatchEventSource,
        compatibility: PatchCompatibility,
        message: impl Into<String>,
    ) {
        self.report = WindowedPatchReport {
            state: WindowedPatchState::Applied,
            source: Some(source),
            message: message.into(),
            compatibility: Some(compatibility),
        };
    }

    /// Records that a windowed session restart is underway.
    pub fn restarting(
        &mut self,
        source: PatchEventSource,
        compatibility: PatchCompatibility,
        message: impl Into<String>,
    ) {
        self.report = WindowedPatchReport {
            state: WindowedPatchState::RestartingSession,
            source: Some(source),
            message: message.into(),
            compatibility: Some(compatibility),
        };
    }

    /// Records a rejected patch without dropping the running session.
    pub fn reject(&mut self, source: PatchEventSource, message: impl Into<String>) {
        self.report = WindowedPatchReport {
            state: WindowedPatchState::Rejected,
            source: Some(source),
            message: message.into(),
            compatibility: None,
        };
    }

    /// Returns the latest patch report.
    pub const fn last_report(&self) -> &WindowedPatchReport {
        &self.report
    }

    /// Returns the number of queued events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns whether no events are queued.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_preserves_event_order_at_safe_boundary() {
        let mut queue = WindowedPatchQueue::default();
        queue.push(WindowedPatchEvent::ApplyBundle {
            bytes: b"one".to_vec(),
            source: PatchEventSource::WatchChannel,
        });
        queue.push(WindowedPatchEvent::ApplyTransportSidecar {
            bytes: b"two".to_vec(),
            source: PatchEventSource::OneShotSidecar,
        });

        let first = queue
            .pop_ready(FrameBoundary::AfterRenderSubmitted)
            .expect("safe boundary pops");
        let second = queue
            .pop_ready(FrameBoundary::AfterRenderSubmitted)
            .expect("safe boundary pops");

        assert!(matches!(
            first,
            WindowedPatchEvent::ApplyBundle { bytes, .. } if bytes == b"one"
        ));
        assert!(matches!(
            second,
            WindowedPatchEvent::ApplyTransportSidecar { bytes, .. } if bytes == b"two"
        ));
        assert!(queue.is_empty());
    }

    #[test]
    fn queue_rejects_mutation_before_safe_boundary() {
        let mut queue = WindowedPatchQueue::default();
        queue.push(WindowedPatchEvent::ApplyBundle {
            bytes: Vec::new(),
            source: PatchEventSource::WatchChannel,
        });

        let error = queue
            .pop_ready(FrameBoundary::BeforeRender)
            .expect_err("unsafe boundary rejects");

        assert_eq!(
            error,
            WindowedPatchError::UnsafeBoundary(FrameBoundary::BeforeRender)
        );
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn reports_last_patch_state() {
        let mut queue = WindowedPatchQueue::default();

        queue.ready_to_commit(
            PatchEventSource::EmbeddingApi,
            PatchCompatibility::ContentOnly,
            "ready",
        );
        assert_eq!(queue.last_report().state, WindowedPatchState::ReadyToCommit);
        assert_eq!(
            queue.last_report().compatibility,
            Some(PatchCompatibility::ContentOnly)
        );

        queue.reject(PatchEventSource::EmbeddingApi, "invalid patch");
        assert_eq!(queue.last_report().state, WindowedPatchState::Rejected);
        assert_eq!(queue.last_report().message, "invalid patch");
    }
}
