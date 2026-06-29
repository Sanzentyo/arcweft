//! Trace capture for native-player text-input bridge validation.

use super::NativeTextInputFocusReason;
use super::backend::NativeTextInputBackendIdentity;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::text_input::{
    PlatformTextInputEvent, TextByteOffset, TextCompositionUpdate, TextControlWriteBack,
    TextControlWriteBackKind, TextInput, TextInputCapabilities, TextInputCapabilitySupport,
    TextInputClientSnapshot, TextInputFocusGeneration, TextInputGeometrySnapshot,
    TextInputKeyDisposition, TextInputOperation, TextInputSecurityPolicy, TextInputSessionId,
    TextRange,
};
use arcweft_runtime_host::TextInputDispatchError;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeTextInputTraceOptions {
    output: Option<PathBuf>,
}

#[derive(Debug)]
pub(crate) struct NativeTextInputTraceWriter {
    output: Option<PathBuf>,
    records: Vec<NativeTextInputTraceRecord>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "record", rename_all = "snake_case")]
pub enum NativeTextInputTraceRecord {
    BackendSelected {
        backend: NativeTextInputBackendIdentity,
    },
    Capabilities {
        backend: NativeTextInputBackendIdentity,
        capabilities: TraceCapabilities,
    },
    BackendUnavailable {
        backend: NativeTextInputBackendIdentity,
        reason: String,
    },
    Focus {
        backend: NativeTextInputBackendIdentity,
        reason: &'static str,
        generation: u64,
        secure_redacted: bool,
        snapshot: TraceSnapshot,
    },
    Snapshot {
        backend: NativeTextInputBackendIdentity,
        phase: &'static str,
        generation: u64,
        secure_redacted: bool,
        snapshot: TraceSnapshot,
    },
    Geometry {
        backend: NativeTextInputBackendIdentity,
        secure_redacted: bool,
        geometry: TraceGeometry,
    },
    Blur {
        backend: NativeTextInputBackendIdentity,
        ended_session: Option<u64>,
    },
    KeyDisposition {
        backend: NativeTextInputBackendIdentity,
        key: String,
        disposition: &'static str,
    },
    PlatformEvent {
        backend: NativeTextInputBackendIdentity,
        adapter: String,
        session: u64,
        serial: u64,
        generation: u64,
        target: String,
        kind: &'static str,
        secure_redacted: bool,
    },
    RoutedTextInput {
        backend: NativeTextInputBackendIdentity,
        session: u64,
        serial: u64,
        operation_kinds: Vec<&'static str>,
        privacy: &'static str,
        key_disposition: &'static str,
        secure_redacted: bool,
    },
    RuntimeWriteBack {
        backend: NativeTextInputBackendIdentity,
        target: String,
        session: u64,
        kind: &'static str,
        revision: u64,
        selection: TraceTextRange,
        secure_redacted: bool,
        value_len: usize,
    },
    DispatchRejected {
        backend: NativeTextInputBackendIdentity,
        reason: String,
    },
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TraceCapabilities {
    surrounding_text: &'static str,
    delete_surrounding: &'static str,
    reconversion: &'static str,
    composition_segments: &'static str,
    character_bounds: &'static str,
    programmatic_commit: &'static str,
    programmatic_cancel: &'static str,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TraceSnapshot {
    session: u64,
    revision: u64,
    target: String,
    control_rect: TraceRect,
    caret_rect: Option<TraceRect>,
    surrounding_text: Option<String>,
    selection: Option<TraceTextRange>,
    composition: Option<TraceComposition>,
    character_bounds_count: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TraceGeometry {
    session: u64,
    revision: u64,
    writing_mode: String,
    viewport_control_rect: TraceRect,
    screen_control_rect: TraceRect,
    screen_caret_rect: Option<TraceRect>,
    screen_character_bounds_count: usize,
    screen_selection_rect_count: usize,
    screen_composition_rect_count: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TraceComposition {
    preedit_len: usize,
    selection: TraceTextRange,
    replacement: Option<TraceTextRange>,
    segment_count: usize,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct TraceTextRange {
    start: u32,
    end: u32,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct TraceRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl NativeTextInputTraceOptions {
    pub fn write_to(path: impl Into<PathBuf>) -> Self {
        Self {
            output: Some(path.into()),
        }
    }

    pub const fn output(&self) -> Option<&PathBuf> {
        self.output.as_ref()
    }
}

impl NativeTextInputTraceWriter {
    pub(crate) fn new(options: NativeTextInputTraceOptions) -> Self {
        Self {
            output: options.output,
            records: Vec::new(),
        }
    }

    pub(crate) fn record_capabilities(
        &mut self,
        backend: NativeTextInputBackendIdentity,
        capabilities: TextInputCapabilities,
    ) {
        self.records.push(NativeTextInputTraceRecord::Capabilities {
            backend,
            capabilities: TraceCapabilities::from(capabilities),
        });
    }

    pub(crate) fn record_backend_selected(&mut self, backend: NativeTextInputBackendIdentity) {
        self.records
            .push(NativeTextInputTraceRecord::BackendSelected { backend });
    }

    pub(crate) fn record_backend_unavailable(
        &mut self,
        backend: NativeTextInputBackendIdentity,
        reason: &str,
    ) {
        self.records
            .push(NativeTextInputTraceRecord::BackendUnavailable {
                backend,
                reason: reason.to_owned(),
            });
    }

    pub(crate) fn record_focus(
        &mut self,
        backend: NativeTextInputBackendIdentity,
        reason: NativeTextInputFocusReason,
        snapshot: &TextInputClientSnapshot,
        generation: TextInputFocusGeneration,
        security: TextInputSecurityPolicy,
    ) {
        self.records.push(NativeTextInputTraceRecord::Focus {
            backend,
            reason: focus_reason_label(reason),
            generation: generation.0,
            secure_redacted: security == TextInputSecurityPolicy::SecureRedacted,
            snapshot: TraceSnapshot::from_snapshot(snapshot, security),
        });
    }

    pub(crate) fn record_snapshot(
        &mut self,
        backend: NativeTextInputBackendIdentity,
        phase: &'static str,
        snapshot: &TextInputClientSnapshot,
        generation: TextInputFocusGeneration,
        security: TextInputSecurityPolicy,
    ) {
        self.records.push(NativeTextInputTraceRecord::Snapshot {
            backend,
            phase,
            generation: generation.0,
            secure_redacted: security == TextInputSecurityPolicy::SecureRedacted,
            snapshot: TraceSnapshot::from_snapshot(snapshot, security),
        });
    }

    pub(crate) fn record_geometry(
        &mut self,
        backend: NativeTextInputBackendIdentity,
        geometry: &TextInputGeometrySnapshot,
        security: TextInputSecurityPolicy,
    ) {
        self.records.push(NativeTextInputTraceRecord::Geometry {
            backend,
            secure_redacted: security == TextInputSecurityPolicy::SecureRedacted,
            geometry: TraceGeometry::from_geometry(geometry, security),
        });
    }

    pub(crate) fn record_blur(
        &mut self,
        backend: NativeTextInputBackendIdentity,
        ended_session: Option<TextInputSessionId>,
    ) {
        self.records.push(NativeTextInputTraceRecord::Blur {
            backend,
            ended_session: ended_session.map(|session| session.0),
        });
    }

    pub(crate) fn record_key_disposition(
        &mut self,
        backend: NativeTextInputBackendIdentity,
        key: &str,
        disposition: TextInputKeyDisposition,
    ) {
        self.records
            .push(NativeTextInputTraceRecord::KeyDisposition {
                backend,
                key: key.to_owned(),
                disposition: key_disposition_label(disposition),
            });
    }

    pub(crate) fn record_platform_event(
        &mut self,
        backend: NativeTextInputBackendIdentity,
        event: &PlatformTextInputEvent,
        security: TextInputSecurityPolicy,
    ) {
        let context = event.context();
        self.records
            .push(NativeTextInputTraceRecord::PlatformEvent {
                backend,
                adapter: format!("{:?}", context.adapter()),
                session: context.session().0,
                serial: context.serial().0,
                generation: context.generation().0,
                target: format!("{:?}", context.target()),
                kind: platform_event_kind(event),
                secure_redacted: security == TextInputSecurityPolicy::SecureRedacted,
            });
    }

    pub(crate) fn record_routed_text_input(
        &mut self,
        backend: NativeTextInputBackendIdentity,
        input: &TextInput,
        key_disposition: TextInputKeyDisposition,
        security: TextInputSecurityPolicy,
    ) {
        self.records
            .push(NativeTextInputTraceRecord::RoutedTextInput {
                backend,
                session: input.session().0,
                serial: input.serial().0,
                operation_kinds: input.operations().iter().map(operation_kind).collect(),
                privacy: if input.privacy().is_sensitive() {
                    "sensitive"
                } else {
                    "plain"
                },
                key_disposition: key_disposition_label(key_disposition),
                secure_redacted: security == TextInputSecurityPolicy::SecureRedacted,
            });
    }

    pub(crate) fn record_runtime_write_back(
        &mut self,
        backend: NativeTextInputBackendIdentity,
        write_back: &TextControlWriteBack,
    ) {
        let secure_redacted = write_back.value().is_sensitive();
        self.records
            .push(NativeTextInputTraceRecord::RuntimeWriteBack {
                backend,
                target: write_back.target().id().as_str().to_owned(),
                session: write_back.session().0,
                kind: write_back_kind_label(write_back.kind()),
                revision: write_back.revision().0,
                selection: TraceTextRange::from(write_back.selection()),
                secure_redacted,
                value_len: if secure_redacted {
                    0
                } else {
                    write_back.value().as_str().len()
                },
            });
    }

    pub(crate) fn record_dispatch_rejection(
        &mut self,
        backend: NativeTextInputBackendIdentity,
        error: &TextInputDispatchError,
    ) {
        self.records
            .push(NativeTextInputTraceRecord::DispatchRejected {
                backend,
                reason: error.to_string(),
            });
    }

    #[cfg(test)]
    pub(crate) fn records_for_tests(&self) -> Vec<String> {
        self.records
            .iter()
            .map(|record| serde_json::to_string(record).expect("trace record serializes"))
            .collect()
    }

    fn flush(&self) -> std::io::Result<()> {
        let Some(output) = &self.output else {
            return Ok(());
        };
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&self.records)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        fs::write(output, bytes)
    }
}

impl Drop for NativeTextInputTraceWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

impl From<TextInputCapabilities> for TraceCapabilities {
    fn from(value: TextInputCapabilities) -> Self {
        Self {
            surrounding_text: capability_label(value.surrounding_text),
            delete_surrounding: capability_label(value.delete_surrounding),
            reconversion: capability_label(value.reconversion),
            composition_segments: capability_label(value.composition_segments),
            character_bounds: capability_label(value.character_bounds),
            programmatic_commit: capability_label(value.programmatic_commit),
            programmatic_cancel: capability_label(value.programmatic_cancel),
        }
    }
}

impl TraceSnapshot {
    fn from_snapshot(
        snapshot: &TextInputClientSnapshot,
        security: TextInputSecurityPolicy,
    ) -> Self {
        let secure = security == TextInputSecurityPolicy::SecureRedacted;
        Self {
            session: snapshot.session().0,
            revision: snapshot.revision().0,
            target: format!("{:?}", snapshot.target()),
            control_rect: TraceRect::from(snapshot.control_rect()),
            caret_rect: (!secure).then(|| TraceRect::from(snapshot.caret_rect())),
            surrounding_text: (!secure).then(|| snapshot.surrounding_text().to_owned()),
            selection: (!secure).then(|| TraceTextRange::from(snapshot.selection())),
            composition: (!secure)
                .then(|| {
                    snapshot
                        .composition()
                        .map(TraceComposition::from_composition)
                })
                .flatten(),
            character_bounds_count: if secure {
                0
            } else {
                snapshot.character_bounds().len()
            },
        }
    }
}

impl TraceGeometry {
    fn from_geometry(
        geometry: &TextInputGeometrySnapshot,
        security: TextInputSecurityPolicy,
    ) -> Self {
        let secure = security == TextInputSecurityPolicy::SecureRedacted;
        Self {
            session: geometry.session().0,
            revision: geometry.revision().0,
            writing_mode: format!("{:?}", geometry.writing_mode()),
            viewport_control_rect: TraceRect::from(geometry.viewport_control_rect()),
            screen_control_rect: TraceRect::from(geometry.screen_control_rect()),
            screen_caret_rect: (!secure).then(|| TraceRect::from(geometry.screen_caret_rect())),
            screen_character_bounds_count: if secure {
                0
            } else {
                geometry.screen_character_bounds().len()
            },
            screen_selection_rect_count: if secure {
                0
            } else {
                geometry.screen_selection_rects().len()
            },
            screen_composition_rect_count: if secure {
                0
            } else {
                geometry.screen_composition_rects().len()
            },
        }
    }
}

impl TraceComposition {
    fn from_composition(value: &TextCompositionUpdate) -> Self {
        Self {
            preedit_len: value.preedit().len(),
            selection: TraceTextRange::from(value.selection()),
            replacement: value.replacement().map(TraceTextRange::from),
            segment_count: value.segments().len(),
        }
    }
}

impl From<TextRange<TextByteOffset>> for TraceTextRange {
    fn from(value: TextRange<TextByteOffset>) -> Self {
        Self {
            start: value.start().0,
            end: value.end().0,
        }
    }
}

impl From<HitRect> for TraceRect {
    fn from(value: HitRect) -> Self {
        Self {
            x: value.x,
            y: value.y,
            width: value.width,
            height: value.height,
        }
    }
}

const fn capability_label(value: TextInputCapabilitySupport) -> &'static str {
    match value {
        TextInputCapabilitySupport::Unsupported => "unsupported",
        TextInputCapabilitySupport::Supported => "supported",
        TextInputCapabilitySupport::Limited => "limited",
        TextInputCapabilitySupport::VersionDependent => "version_dependent",
        TextInputCapabilitySupport::HostDependent => "host_dependent",
        TextInputCapabilitySupport::SecureRedacted => "secure_redacted",
    }
}

const fn key_disposition_label(value: TextInputKeyDisposition) -> &'static str {
    match value {
        TextInputKeyDisposition::ShortcutCandidate => "shortcut_candidate",
        TextInputKeyDisposition::ImeConsumed => "ime_consumed",
    }
}

const fn focus_reason_label(value: NativeTextInputFocusReason) -> &'static str {
    match value {
        NativeTextInputFocusReason::Pointer => "pointer",
        NativeTextInputFocusReason::RedrawRefresh => "redraw_refresh",
        #[cfg(test)]
        NativeTextInputFocusReason::Fixture => "fixture",
    }
}

const fn platform_event_kind(value: &PlatformTextInputEvent) -> &'static str {
    match value {
        PlatformTextInputEvent::StartComposition(_) => "start_composition",
        PlatformTextInputEvent::SetComposition { .. } => "set_composition",
        PlatformTextInputEvent::Commit { .. } => "commit",
        PlatformTextInputEvent::EndComposition { .. } => "end_composition",
        PlatformTextInputEvent::DeleteSurrounding { .. } => "delete_surrounding",
        PlatformTextInputEvent::SetSelection { .. } => "set_selection",
        PlatformTextInputEvent::Command { .. } => "command",
        PlatformTextInputEvent::Batch { .. } => "batch",
    }
}

const fn operation_kind(value: &TextInputOperation) -> &'static str {
    match value {
        TextInputOperation::StartComposition => "start_composition",
        TextInputOperation::SetComposition(_) => "set_composition",
        TextInputOperation::Commit(_) => "commit",
        TextInputOperation::EndComposition { .. } => "end_composition",
        TextInputOperation::DeleteSurrounding { .. } => "delete_surrounding",
        TextInputOperation::SetSelection(_) => "set_selection",
        TextInputOperation::Command(_) => "command",
    }
}

const fn write_back_kind_label(value: TextControlWriteBackKind) -> &'static str {
    match value {
        TextControlWriteBackKind::Change => "change",
        TextControlWriteBackKind::Submit => "submit",
    }
}
