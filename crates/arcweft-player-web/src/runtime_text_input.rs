//! Normal Web-player runtime text-input bridge.
//!
//! This module is the Web counterpart to the native player text-input bridge: it
//! owns the active focused Arcweft text-control session, keeps browser object
//! identity outside Sans I/O crates, publishes value-shaped runtime commands to
//! the JavaScript `EditContext` glue, and queues validated `TextInput` batches so
//! `app.rs` can route them through `InputController::text_input`.

use crate::edit_context::{
    WebEditContextAdapter, WebEditContextError, WebEditContextFeatureDetection,
    WebEditContextTextUpdate,
};
use arcweft_presentation::hit::HitRect;
#[cfg(test)]
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::text_index::TextIndexSnapshot;
use arcweft_presentation::text_input::{
    CompositionEndReason, TextByteOffset, TextCharacterBounds, TextEditCommand, TextInput,
    TextInputCapabilities, TextInputClientSnapshot, TextInputGeometrySnapshot,
    TextInputHostCommand, TextInputKeyDisposition, TextInputSessionId, TextRange, TextRangeRect,
    TextUtf16Offset,
};
use arcweft_render_wgpu::geometry::{PreparedFrame, PreparedTextInputTarget};
use arcweft_runtime_host::{
    PlayerTextInputBridgeCore, PlayerTextInputEdit, PlayerTextInputFocusedControl,
    PlayerTextInputSyncPhase,
};
use serde::Serialize;
use std::collections::VecDeque;
use thiserror::Error;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, collections::BTreeMap};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue};

#[cfg(target_arch = "wasm32")]
const RUNTIME_COMMAND_EVENT: &str = "arcweft-text-input-runtime-command";

/// Reason recorded when the Web player refreshes platform text-input focus.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebRuntimeTextInputFocusReason {
    Pointer,
    KeyboardTraversal,
    CanvasFocus,
    RedrawRefresh,
    Resize,
    Scroll,
    Visibility,
    #[cfg(test)]
    Fixture,
}

/// Browser/client transform applied to renderer viewport geometry before it is
/// passed to `EditContext` APIs, which expect client-coordinate CSS pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WebTextInputClientTransform {
    viewport_origin_x: f32,
    viewport_origin_y: f32,
}

/// Focused Arcweft text control snapshot plus renderer-backed geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct WebRuntimeTextInputFocusedControl {
    snapshot: TextInputClientSnapshot,
    geometry: TextInputGeometrySnapshot,
    reason: WebRuntimeTextInputFocusReason,
}

/// Text update payload received from `web/player-editcontext.js`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebRuntimeTextInputTextUpdate {
    update_range: TextRange<TextUtf16Offset>,
    text: String,
    selection: TextRange<TextUtf16Offset>,
    observed_text_before: Option<String>,
    composing: bool,
}

/// Validated edit drained by the normal Web player and ready for scene routing.
#[derive(Clone, Debug, PartialEq)]
pub struct WebRuntimeTextInputPlayerEdit {
    input: TextInput,
    key_disposition: TextInputKeyDisposition,
}

/// Result of synchronizing Web player focus/geometry to the JS `EditContext`
/// owner.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WebRuntimeTextInputSync {
    commands: Vec<WebRuntimeTextInputCommand>,
}

/// Serializable browser command emitted by the runtime bridge.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WebRuntimeTextInputCommand {
    Activate {
        snapshot: WebRuntimeTextInputSnapshot,
    },
    UpdateSnapshot {
        snapshot: WebRuntimeTextInputSnapshot,
    },
    UpdateGeometry {
        geometry: WebRuntimeTextInputGeometry,
    },
    CommitComposition {
        session: u64,
    },
    CancelComposition {
        session: u64,
    },
    Deactivate {
        session: u64,
    },
    UnsupportedNoFallback {
        message: String,
    },
}

/// Serializable focused text snapshot consumed by `web/player-editcontext.js`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebRuntimeTextInputSnapshot {
    session: u64,
    revision: u64,
    target: String,
    secure: bool,
    multiline: bool,
    text: String,
    surrounding_text: String,
    selection_start: u32,
    selection_end: u32,
    composition_start: u32,
    composition_end: u32,
    control_rect: WebRuntimeRect,
    caret_rect: WebRuntimeRect,
    character_bounds: Vec<WebRuntimeRangeRect>,
}

/// Serializable geometry consumed by `web/player-editcontext.js`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebRuntimeTextInputGeometry {
    session: u64,
    revision: u64,
    control_rect: WebRuntimeRect,
    caret_rect: WebRuntimeRect,
    selection_rects: Vec<WebRuntimeRangeRect>,
    composition_rects: Vec<WebRuntimeRangeRect>,
    character_bounds: Vec<WebRuntimeRangeRect>,
    text_inset_x: f32,
    text_inset_y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebRuntimeRange {
    start: u32,
    end: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebRuntimeRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebRuntimeRangeRect {
    range: WebRuntimeRange,
    rect: WebRuntimeRect,
}

/// Status returned to JavaScript after one platform callback has been accepted.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebRuntimeTextInputDispatchStatus {
    state: &'static str,
    fallback_installed: bool,
    operation_count: usize,
    pending_edit_count: usize,
    privacy: &'static str,
    key_disposition: &'static str,
}

/// Player-owned Web text-input bridge for one canvas/host id.
#[derive(Debug)]
pub struct WebPlayerTextInputBridge {
    host_id: String,
    core: PlayerTextInputBridgeCore,
    adapter: WebEditContextAdapter,
    detection: WebEditContextFeatureDetection,
    pending: VecDeque<WebRuntimeTextInputPlayerEdit>,
    client_transform: WebTextInputClientTransform,
    unsupported_reported: bool,
}

/// Opaque registry handle owned by the normal `app.rs` player loop.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebPlayerTextInputBridgeHandle {
    host_id: String,
}

/// Bridge-level error. Browser objects never cross this boundary.
#[derive(Debug, Error)]
pub enum WebRuntimeTextInputBridgeError {
    #[error("Web EditContext adapter failed: {0}")]
    EditContext(#[from] WebEditContextError),
    #[error("Web text-input dispatch failed: {0}")]
    Dispatch(#[from] arcweft_runtime_host::TextInputDispatchError),
    #[error("Web text-input dispatch emitted a non-text raw event")]
    NonTextDispatchOutput,
    #[error("unknown Web text-input command `{0}`")]
    UnknownCommand(String),
    #[error("no registered Web player text-input bridge for host `{0}`")]
    MissingRegisteredBridge(String),
    #[error("runtime text-input command serialization failed: {0}")]
    Serialization(String),
    #[error("runtime text-input command event dispatch failed: {0}")]
    EventDispatch(String),
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WEB_PLAYER_TEXT_INPUT_BRIDGES: RefCell<BTreeMap<String, WebPlayerTextInputBridge>> =
        RefCell::new(BTreeMap::new());
}

impl WebTextInputClientTransform {
    pub const fn new(viewport_origin_x: f32, viewport_origin_y: f32) -> Self {
        Self {
            viewport_origin_x,
            viewport_origin_y,
        }
    }

    fn rect(self, rect: HitRect) -> WebRuntimeRect {
        WebRuntimeRect {
            x: rect.x + self.viewport_origin_x,
            y: rect.y + self.viewport_origin_y,
            width: rect.width,
            height: rect.height,
        }
    }
}

impl WebRuntimeTextInputFocusedControl {
    pub fn new(
        snapshot: TextInputClientSnapshot,
        geometry: TextInputGeometrySnapshot,
        reason: WebRuntimeTextInputFocusReason,
    ) -> Self {
        Self {
            snapshot,
            geometry,
            reason,
        }
    }

    pub const fn snapshot(&self) -> &TextInputClientSnapshot {
        &self.snapshot
    }

    pub const fn geometry(&self) -> &TextInputGeometrySnapshot {
        &self.geometry
    }

    pub const fn reason(&self) -> WebRuntimeTextInputFocusReason {
        self.reason
    }
}

impl WebRuntimeTextInputTextUpdate {
    pub fn new(
        update_range: TextRange<TextUtf16Offset>,
        text: impl Into<String>,
        selection: TextRange<TextUtf16Offset>,
    ) -> Self {
        Self {
            update_range,
            text: text.into(),
            selection,
            observed_text_before: None,
            composing: false,
        }
    }

    #[must_use]
    pub fn with_observed_text_before(mut self, observed: impl Into<String>) -> Self {
        self.observed_text_before = Some(observed.into());
        self
    }

    #[must_use]
    pub const fn composing(mut self, composing: bool) -> Self {
        self.composing = composing;
        self
    }

    fn as_edit_context_update(&self) -> WebEditContextTextUpdate {
        let update =
            WebEditContextTextUpdate::new(self.update_range, self.text.clone(), self.selection)
                .composing(self.composing);
        match &self.observed_text_before {
            Some(observed) => update.with_observed_text_before(observed.clone()),
            None => update,
        }
    }
}

impl WebRuntimeTextInputPlayerEdit {
    pub fn into_input(self) -> TextInput {
        self.input
    }

    pub const fn key_disposition(&self) -> TextInputKeyDisposition {
        self.key_disposition
    }
}

impl WebRuntimeTextInputDispatchStatus {
    pub const fn state(&self) -> &'static str {
        self.state
    }

    pub const fn operation_count(&self) -> usize {
        self.operation_count
    }

    pub const fn pending_edit_count(&self) -> usize {
        self.pending_edit_count
    }

    pub const fn privacy(&self) -> &'static str {
        self.privacy
    }

    pub const fn key_disposition(&self) -> &'static str {
        self.key_disposition
    }

    pub const fn fallback_installed(&self) -> bool {
        self.fallback_installed
    }
}

impl WebRuntimeTextInputSync {
    pub fn commands(&self) -> &[WebRuntimeTextInputCommand] {
        &self.commands
    }

    pub fn into_commands(self) -> Vec<WebRuntimeTextInputCommand> {
        self.commands
    }
}

impl WebPlayerTextInputBridge {
    pub fn new(host_id: impl Into<String>, detection: WebEditContextFeatureDetection) -> Self {
        Self {
            host_id: host_id.into(),
            core: PlayerTextInputBridgeCore::default(),
            adapter: WebEditContextAdapter::default(),
            detection,
            pending: VecDeque::new(),
            client_transform: WebTextInputClientTransform::default(),
            unsupported_reported: false,
        }
    }

    pub fn host_id(&self) -> &str {
        &self.host_id
    }

    pub fn set_client_transform(&mut self, transform: WebTextInputClientTransform) {
        self.client_transform = transform;
    }

    pub fn sync_prepared_frame(
        &mut self,
        frame: &PreparedFrame,
        reason: WebRuntimeTextInputFocusReason,
    ) -> Result<WebRuntimeTextInputSync, WebRuntimeTextInputBridgeError> {
        self.sync_focus(
            frame
                .focused_text_input_target()
                .map(|target| Self::from_prepared_target(target, reason)),
        )
    }

    pub fn sync_focus(
        &mut self,
        focused: Option<WebRuntimeTextInputFocusedControl>,
    ) -> Result<WebRuntimeTextInputSync, WebRuntimeTextInputBridgeError> {
        let Some(focused) = focused else {
            return self.blur_active();
        };
        let Ok(capabilities) = self.detection.capabilities() else {
            let _ = self.core.blur_active();
            self.adapter.deactivate();
            return Ok(self.unsupported_sync());
        };
        let focused = PlayerTextInputFocusedControl::new(
            focused.snapshot().clone(),
            focused.geometry().clone(),
            capabilities,
        );
        let snapshot = focused.snapshot();
        let sync = self.core.sync_focus(Some(&focused))?;
        match sync.phase() {
            PlayerTextInputSyncPhase::Activated => {
                self.adapter.activate(
                    snapshot,
                    self.detection,
                    sync.generation(),
                    self.core
                        .active_capabilities()
                        .unwrap_or(TextInputCapabilities::all_supported()),
                )?;
            }
            PlayerTextInputSyncPhase::Updated => {
                self.adapter.update_snapshot(snapshot)?;
            }
            PlayerTextInputSyncPhase::Blurred => {
                self.adapter.deactivate();
            }
            PlayerTextInputSyncPhase::Idle => {}
        }
        Ok(WebRuntimeTextInputSync {
            commands: self.commands_from_transaction(sync.into_commands())?,
        })
    }

    pub fn blur_active(
        &mut self,
    ) -> Result<WebRuntimeTextInputSync, WebRuntimeTextInputBridgeError> {
        let sync = self.core.blur_active();
        if sync.phase() != PlayerTextInputSyncPhase::Idle {
            self.adapter.deactivate();
        }
        Ok(WebRuntimeTextInputSync {
            commands: self.commands_from_transaction(sync.into_commands())?,
        })
    }

    pub fn dispatch_text_update(
        &mut self,
        update: &WebRuntimeTextInputTextUpdate,
    ) -> Result<WebRuntimeTextInputDispatchStatus, WebRuntimeTextInputBridgeError> {
        let event = self
            .adapter
            .text_update_event(&update.as_edit_context_update())?;
        let edit = self
            .core
            .dispatch_platform_event(event, TextInputKeyDisposition::ImeConsumed)?;
        Ok(self.enqueue_edit("textupdate", edit))
    }

    pub fn dispatch_composition_start(
        &mut self,
    ) -> Result<WebRuntimeTextInputDispatchStatus, WebRuntimeTextInputBridgeError> {
        let event = self.adapter.composition_start_event()?;
        let edit = self
            .core
            .dispatch_platform_event(event, TextInputKeyDisposition::ImeConsumed)?;
        Ok(self.enqueue_edit("compositionstart", edit))
    }

    pub fn dispatch_composition_end(
        &mut self,
        cancelled: bool,
    ) -> Result<WebRuntimeTextInputDispatchStatus, WebRuntimeTextInputBridgeError> {
        let reason = if cancelled {
            CompositionEndReason::Cancelled
        } else {
            CompositionEndReason::Committed
        };
        let event = self.adapter.composition_end_event(reason)?;
        let edit = self
            .core
            .dispatch_platform_event(event, TextInputKeyDisposition::ImeConsumed)?;
        Ok(self.enqueue_edit("compositionend", edit))
    }

    pub fn dispatch_command_label(
        &mut self,
        label: &str,
        selecting: bool,
    ) -> Result<WebRuntimeTextInputDispatchStatus, WebRuntimeTextInputBridgeError> {
        let command = command_from_label(label, selecting)?;
        let event = self.adapter.command_event(command)?;
        let edit = self
            .core
            .dispatch_platform_event(event, TextInputKeyDisposition::ShortcutCandidate)?;
        Ok(self.enqueue_edit("command", edit))
    }

    pub fn drain_pending_edits(&mut self) -> Vec<WebRuntimeTextInputPlayerEdit> {
        self.pending.drain(..).collect()
    }

    pub fn key_disposition(&self) -> TextInputKeyDisposition {
        let disposition = if self
            .adapter
            .active()
            .is_some_and(crate::edit_context::WebEditContextActiveSession::is_composing)
        {
            TextInputKeyDisposition::ImeConsumed
        } else {
            TextInputKeyDisposition::ShortcutCandidate
        };
        if self.core.shortcuts_allowed(disposition) {
            disposition
        } else {
            TextInputKeyDisposition::ImeConsumed
        }
    }

    pub fn active_session(&self) -> Option<TextInputSessionId> {
        self.core.active_session()
    }

    fn from_prepared_target(
        target: PreparedTextInputTarget,
        reason: WebRuntimeTextInputFocusReason,
    ) -> WebRuntimeTextInputFocusedControl {
        WebRuntimeTextInputFocusedControl::new(target.snapshot, target.geometry, reason)
    }

    fn unsupported_sync(&mut self) -> WebRuntimeTextInputSync {
        if self.unsupported_reported {
            return WebRuntimeTextInputSync::default();
        }
        self.unsupported_reported = true;
        WebRuntimeTextInputSync {
            commands: vec![WebRuntimeTextInputCommand::UnsupportedNoFallback {
                message: "Web EditContext is unavailable; no DOM text-entry fallback is installed"
                    .to_owned(),
            }],
        }
    }

    fn commands_from_transaction(
        &self,
        commands: Vec<TextInputHostCommand>,
    ) -> Result<Vec<WebRuntimeTextInputCommand>, WebRuntimeTextInputBridgeError> {
        let index = self
            .adapter
            .active()
            .map(crate::edit_context::WebEditContextActiveSession::text_index);
        commands
            .into_iter()
            .map(|command| self.command_from_host_command(command, index))
            .collect()
    }

    fn command_from_host_command(
        &self,
        command: TextInputHostCommand,
        active_index: Option<&TextIndexSnapshot>,
    ) -> Result<WebRuntimeTextInputCommand, WebRuntimeTextInputBridgeError> {
        match command {
            TextInputHostCommand::Activate { snapshot, .. } => {
                Ok(WebRuntimeTextInputCommand::Activate {
                    snapshot: snapshot_to_runtime(&snapshot, self.client_transform)?,
                })
            }
            TextInputHostCommand::Update(snapshot) => {
                Ok(WebRuntimeTextInputCommand::UpdateSnapshot {
                    snapshot: snapshot_to_runtime(&snapshot, self.client_transform)?,
                })
            }
            TextInputHostCommand::UpdateGeometry(geometry) => {
                let fallback_index;
                let index = if let Some(index) = active_index {
                    index
                } else {
                    fallback_index = TextIndexSnapshot::try_new(String::new())?;
                    &fallback_index
                };
                Ok(WebRuntimeTextInputCommand::UpdateGeometry {
                    geometry: geometry_to_runtime(&geometry, index, self.client_transform)?,
                })
            }
            TextInputHostCommand::CommitComposition { session } => {
                Ok(WebRuntimeTextInputCommand::CommitComposition { session: session.0 })
            }
            TextInputHostCommand::CancelComposition { session } => {
                Ok(WebRuntimeTextInputCommand::CancelComposition { session: session.0 })
            }
            TextInputHostCommand::Deactivate { session } => {
                Ok(WebRuntimeTextInputCommand::Deactivate { session: session.0 })
            }
        }
    }

    fn enqueue_edit(
        &mut self,
        state: &'static str,
        edit: PlayerTextInputEdit,
    ) -> WebRuntimeTextInputDispatchStatus {
        let key_disposition = edit.key_disposition();
        let input = edit.into_input();
        let operation_count = input.operations().len();
        let privacy = if input.privacy().is_sensitive() {
            "sensitive"
        } else {
            "plain"
        };
        self.pending.push_back(WebRuntimeTextInputPlayerEdit {
            input,
            key_disposition,
        });
        WebRuntimeTextInputDispatchStatus {
            state,
            fallback_installed: false,
            operation_count,
            pending_edit_count: self.pending.len(),
            privacy,
            key_disposition: key_disposition_label(key_disposition),
        }
    }
}

fn snapshot_to_runtime(
    snapshot: &TextInputClientSnapshot,
    transform: WebTextInputClientTransform,
) -> Result<WebRuntimeTextInputSnapshot, WebRuntimeTextInputBridgeError> {
    let index = TextIndexSnapshot::try_new(snapshot.surrounding_text().to_owned())?;
    let selection = index.utf16_range_from_byte(snapshot.selection())?;
    let composition = snapshot
        .composition()
        .and_then(arcweft_presentation::text_input::TextCompositionUpdate::replacement)
        .unwrap_or(snapshot.selection());
    let composition = index.utf16_range_from_byte(composition)?;
    let character_bounds =
        character_bounds_to_runtime(snapshot.character_bounds(), &index, transform)?;
    let secure = snapshot.options().is_secure();
    let text = if secure {
        String::new()
    } else {
        snapshot.surrounding_text().to_owned()
    };
    Ok(WebRuntimeTextInputSnapshot {
        session: snapshot.session().0,
        revision: snapshot.revision().0,
        target: snapshot.target().id().as_str().to_owned(),
        secure,
        multiline: snapshot.options().is_multiline(),
        text: text.clone(),
        surrounding_text: text,
        selection_start: selection.start().get(),
        selection_end: selection.end().get(),
        composition_start: composition.start().get(),
        composition_end: composition.end().get(),
        control_rect: transform.rect(snapshot.control_rect()),
        caret_rect: transform.rect(snapshot.caret_rect()),
        character_bounds,
    })
}

fn geometry_to_runtime(
    geometry: &TextInputGeometrySnapshot,
    index: &TextIndexSnapshot,
    transform: WebTextInputClientTransform,
) -> Result<WebRuntimeTextInputGeometry, WebRuntimeTextInputBridgeError> {
    Ok(WebRuntimeTextInputGeometry {
        session: geometry.session().0,
        revision: geometry.revision().0,
        control_rect: transform.rect(geometry.viewport_control_rect()),
        caret_rect: transform.rect(geometry.viewport_caret_rect()),
        selection_rects: range_rects_to_runtime(
            geometry.viewport_selection_rects(),
            index,
            transform,
        )?,
        composition_rects: range_rects_to_runtime(
            geometry.viewport_composition_rects(),
            index,
            transform,
        )?,
        character_bounds: character_bounds_to_runtime(
            geometry.viewport_character_bounds(),
            index,
            transform,
        )?,
        text_inset_x: 0.0,
        text_inset_y: 0.0,
    })
}

fn character_bounds_to_runtime(
    bounds: &[TextCharacterBounds],
    index: &TextIndexSnapshot,
    transform: WebTextInputClientTransform,
) -> Result<Vec<WebRuntimeRangeRect>, WebRuntimeTextInputBridgeError> {
    bounds
        .iter()
        .map(|bounds| range_rect_to_runtime(bounds.range, bounds.bounds, index, transform))
        .collect()
}

fn range_rects_to_runtime(
    rects: &[TextRangeRect],
    index: &TextIndexSnapshot,
    transform: WebTextInputClientTransform,
) -> Result<Vec<WebRuntimeRangeRect>, WebRuntimeTextInputBridgeError> {
    rects
        .iter()
        .map(|rect| range_rect_to_runtime(rect.range, rect.bounds, index, transform))
        .collect()
}

fn range_rect_to_runtime(
    range: TextRange<TextByteOffset>,
    rect: HitRect,
    index: &TextIndexSnapshot,
    transform: WebTextInputClientTransform,
) -> Result<WebRuntimeRangeRect, WebRuntimeTextInputBridgeError> {
    let utf16 = index.utf16_range_from_byte(range)?;
    Ok(WebRuntimeRangeRect {
        range: WebRuntimeRange {
            start: utf16.start().get(),
            end: utf16.end().get(),
        },
        rect: transform.rect(rect),
    })
}

fn command_from_label(
    label: &str,
    selecting: bool,
) -> Result<TextEditCommand, WebRuntimeTextInputBridgeError> {
    Ok(match label {
        "move_left" => TextEditCommand::MoveLeft { selecting },
        "move_right" => TextEditCommand::MoveRight { selecting },
        "move_up" => TextEditCommand::MoveUp { selecting },
        "move_down" => TextEditCommand::MoveDown { selecting },
        "move_word_left" => TextEditCommand::MoveWordLeft { selecting },
        "move_word_right" => TextEditCommand::MoveWordRight { selecting },
        "move_line_start" => TextEditCommand::MoveLineStart { selecting },
        "move_line_end" => TextEditCommand::MoveLineEnd { selecting },
        "move_document_start" => TextEditCommand::MoveDocumentStart { selecting },
        "move_document_end" => TextEditCommand::MoveDocumentEnd { selecting },
        "move_page_up" => TextEditCommand::MovePageUp { selecting },
        "move_page_down" => TextEditCommand::MovePageDown { selecting },
        "backspace" => TextEditCommand::Backspace,
        "delete" => TextEditCommand::Delete,
        "delete_word_left" => TextEditCommand::DeleteWordLeft,
        "delete_word_right" => TextEditCommand::DeleteWordRight,
        "select_word" => TextEditCommand::SelectWord,
        "select_line" => TextEditCommand::SelectLine,
        "select_all" => TextEditCommand::SelectAll,
        "copy" => TextEditCommand::Copy,
        "cut" => TextEditCommand::Cut,
        "paste" => TextEditCommand::Paste,
        "submit" => TextEditCommand::Submit,
        "cancel" => TextEditCommand::Cancel,
        other => {
            return Err(WebRuntimeTextInputBridgeError::UnknownCommand(
                other.to_owned(),
            ));
        }
    })
}

const fn key_disposition_label(disposition: TextInputKeyDisposition) -> &'static str {
    match disposition {
        TextInputKeyDisposition::ShortcutCandidate => "shortcut_candidate",
        TextInputKeyDisposition::ImeConsumed => "ime_consumed",
    }
}

impl From<arcweft_presentation::text_index::TextIndexError> for WebRuntimeTextInputBridgeError {
    fn from(error: arcweft_presentation::text_index::TextIndexError) -> Self {
        Self::EditContext(WebEditContextError::TextIndex(error))
    }
}

#[cfg(target_arch = "wasm32")]
impl WebPlayerTextInputBridgeHandle {
    pub fn host_id(&self) -> &str {
        &self.host_id
    }

    pub fn set_client_transform(
        &self,
        transform: WebTextInputClientTransform,
    ) -> Result<(), WebRuntimeTextInputBridgeError> {
        with_registered_bridge_mut(&self.host_id, |bridge| {
            bridge.set_client_transform(transform);
            Ok(())
        })?
    }

    pub fn sync_prepared_frame(
        &self,
        frame: &PreparedFrame,
        reason: WebRuntimeTextInputFocusReason,
    ) -> Result<(), WebRuntimeTextInputBridgeError> {
        let sync = with_registered_bridge_mut(&self.host_id, |bridge| {
            bridge.sync_prepared_frame(frame, reason)
        })??;
        publish_runtime_commands(&self.host_id, sync.commands())
    }

    pub fn blur_active(&self) -> Result<(), WebRuntimeTextInputBridgeError> {
        let sync =
            with_registered_bridge_mut(&self.host_id, WebPlayerTextInputBridge::blur_active)??;
        publish_runtime_commands(&self.host_id, sync.commands())
    }

    pub fn drain_pending_edits(
        &self,
    ) -> Result<Vec<WebRuntimeTextInputPlayerEdit>, WebRuntimeTextInputBridgeError> {
        with_registered_bridge_mut(&self.host_id, |bridge| Ok(bridge.drain_pending_edits()))?
    }

    pub fn key_disposition(
        &self,
    ) -> Result<TextInputKeyDisposition, WebRuntimeTextInputBridgeError> {
        with_registered_bridge_mut(&self.host_id, |bridge| Ok(bridge.key_disposition()))?
    }
}

#[cfg(target_arch = "wasm32")]
pub fn register_runtime_bridge(
    host_id: impl Into<String>,
    detection: WebEditContextFeatureDetection,
) -> WebPlayerTextInputBridgeHandle {
    let host_id = host_id.into();
    WEB_PLAYER_TEXT_INPUT_BRIDGES.with(|bridges| {
        bridges.borrow_mut().insert(
            host_id.clone(),
            WebPlayerTextInputBridge::new(host_id.clone(), detection),
        );
    });
    WebPlayerTextInputBridgeHandle { host_id }
}

#[cfg(target_arch = "wasm32")]
pub fn unregister_runtime_bridge(host_id: &str) {
    WEB_PLAYER_TEXT_INPUT_BRIDGES.with(|bridges| {
        bridges.borrow_mut().remove(host_id);
    });
}

#[cfg(target_arch = "wasm32")]
pub fn dispatch_registered_text_update(
    host_id: &str,
    update: &WebRuntimeTextInputTextUpdate,
) -> Result<WebRuntimeTextInputDispatchStatus, WebRuntimeTextInputBridgeError> {
    with_registered_bridge_mut(host_id, |bridge| bridge.dispatch_text_update(update))?
}

#[cfg(target_arch = "wasm32")]
pub fn dispatch_registered_composition_start(
    host_id: &str,
) -> Result<WebRuntimeTextInputDispatchStatus, WebRuntimeTextInputBridgeError> {
    with_registered_bridge_mut(
        host_id,
        WebPlayerTextInputBridge::dispatch_composition_start,
    )?
}

#[cfg(target_arch = "wasm32")]
pub fn dispatch_registered_composition_end(
    host_id: &str,
    cancelled: bool,
) -> Result<WebRuntimeTextInputDispatchStatus, WebRuntimeTextInputBridgeError> {
    with_registered_bridge_mut(host_id, |bridge| bridge.dispatch_composition_end(cancelled))?
}

#[cfg(target_arch = "wasm32")]
pub fn dispatch_registered_command(
    host_id: &str,
    command: &str,
    selecting: bool,
) -> Result<WebRuntimeTextInputDispatchStatus, WebRuntimeTextInputBridgeError> {
    with_registered_bridge_mut(host_id, |bridge| {
        bridge.dispatch_command_label(command, selecting)
    })?
}

#[cfg(target_arch = "wasm32")]
fn with_registered_bridge_mut<T>(
    host_id: &str,
    f: impl FnOnce(&mut WebPlayerTextInputBridge) -> T,
) -> Result<T, WebRuntimeTextInputBridgeError> {
    WEB_PLAYER_TEXT_INPUT_BRIDGES.with(|bridges| {
        let mut bridges = bridges.borrow_mut();
        let bridge = bridges.get_mut(host_id).ok_or_else(|| {
            WebRuntimeTextInputBridgeError::MissingRegisteredBridge(host_id.to_owned())
        })?;
        Ok(f(bridge))
    })
}

#[cfg(target_arch = "wasm32")]
fn publish_runtime_commands(
    host_id: &str,
    commands: &[WebRuntimeTextInputCommand],
) -> Result<(), WebRuntimeTextInputBridgeError> {
    if commands.is_empty() {
        return Ok(());
    }
    let envelope = WebRuntimeCommandEnvelope { host_id, commands };
    let json = serde_json::to_string(&envelope)
        .map_err(|error| WebRuntimeTextInputBridgeError::Serialization(error.to_string()))?;
    emit_custom_event(RUNTIME_COMMAND_EVENT, &json)
}

#[cfg(target_arch = "wasm32")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebRuntimeCommandEnvelope<'a> {
    host_id: &'a str,
    commands: &'a [WebRuntimeTextInputCommand],
}

#[cfg(target_arch = "wasm32")]
fn emit_custom_event(name: &str, detail: &str) -> Result<(), WebRuntimeTextInputBridgeError> {
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&JsValue::from_str(detail));
    let event = web_sys::CustomEvent::new_with_event_init_dict(name, &init)
        .map_err(js_error_to_string)
        .map_err(WebRuntimeTextInputBridgeError::EventDispatch)?;
    web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| {
            WebRuntimeTextInputBridgeError::EventDispatch("document missing".to_owned())
        })?
        .dispatch_event(event.unchecked_ref())
        .map_err(js_error_to_string)
        .map_err(WebRuntimeTextInputBridgeError::EventDispatch)?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn js_error_to_string(error: JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| "non-string JavaScript error".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_id::PublicId;
    use arcweft_presentation::text_input::{
        TextGeometryTransform, TextInputGeometrySnapshotParts, TextInputOperation,
        TextInputOptions, TextRevision, TextWritingMode,
    };

    fn target(name: &str) -> InteractionTarget {
        InteractionTarget::new(PublicId::try_new(format!("target.{name}")).unwrap())
    }

    fn snapshot(
        session: u64,
        target: InteractionTarget,
        text: &str,
        secure: bool,
    ) -> TextInputClientSnapshot {
        snapshot_with_options(
            session,
            target,
            text,
            TextInputOptions::default().secure(secure),
        )
    }

    fn snapshot_with_options(
        session: u64,
        target: InteractionTarget,
        text: &str,
        options: TextInputOptions,
    ) -> TextInputClientSnapshot {
        let end = TextByteOffset(u32::try_from(text.len()).unwrap());
        TextInputClientSnapshot::new(
            TextInputSessionId(session),
            target,
            TextRevision(1),
            text,
            TextByteOffset(0),
            TextRange::new(end, end),
            HitRect::new(10.0, 20.0, 240.0, 32.0),
            HitRect::new(38.0, 24.0, 1.0, 24.0),
            options,
        )
    }

    fn geometry(session: u64, text: &str) -> TextInputGeometrySnapshot {
        let end = TextByteOffset(u32::try_from(text.len()).unwrap());
        TextInputGeometrySnapshot::new(TextInputGeometrySnapshotParts {
            session: TextInputSessionId(session),
            revision: TextRevision(1),
            writing_mode: TextWritingMode::HorizontalTb,
            text_local_control_rect: HitRect::new(0.0, 0.0, 240.0, 32.0),
            text_local_caret_rect: HitRect::new(28.0, 4.0, 1.0, 24.0),
            text_local_character_bounds: vec![TextCharacterBounds::new(
                TextRange::new(TextByteOffset(0), end),
                HitRect::new(0.0, 0.0, 48.0, 24.0),
            )],
            text_local_selection_rects: vec![TextRangeRect::new(
                TextRange::new(TextByteOffset(0), end),
                HitRect::new(0.0, 0.0, 48.0, 24.0),
            )],
            text_local_composition_rects: Vec::new(),
            text_local_to_viewport: TextGeometryTransform::translation(10.0, 20.0),
            viewport_to_screen: TextGeometryTransform::translation(100.0, 200.0),
        })
    }

    #[test]
    fn activates_and_publishes_client_coordinate_geometry() {
        let target = target("web.plain");
        let mut bridge = WebPlayerTextInputBridge::new(
            "arcweft-canvas",
            WebEditContextFeatureDetection::new(true, true),
        );
        bridge.set_client_transform(WebTextInputClientTransform::new(300.0, 40.0));

        let sync = bridge
            .sync_focus(Some(WebRuntimeTextInputFocusedControl::new(
                snapshot(1, target, "かな", false),
                geometry(1, "かな"),
                WebRuntimeTextInputFocusReason::Fixture,
            )))
            .unwrap();

        assert!(matches!(
            sync.commands()[0],
            WebRuntimeTextInputCommand::Activate { .. }
        ));
        let WebRuntimeTextInputCommand::UpdateGeometry { geometry } = &sync.commands()[1] else {
            panic!("geometry update should follow activation");
        };
        assert!((geometry.caret_rect.x - 338.0).abs() <= f32::EPSILON);
        assert!((geometry.caret_rect.y - 64.0).abs() <= f32::EPSILON);
        assert!(
            geometry
                .character_bounds
                .iter()
                .all(|entry| entry.rect.x >= 300.0)
        );
    }

    #[test]
    fn textupdate_queues_text_input_for_scene_controller() {
        let target = target("web.dispatch");
        let mut bridge = WebPlayerTextInputBridge::new(
            "arcweft-canvas",
            WebEditContextFeatureDetection::new(true, true),
        );
        bridge
            .sync_focus(Some(WebRuntimeTextInputFocusedControl::new(
                snapshot(7, target, "", false),
                geometry(7, ""),
                WebRuntimeTextInputFocusReason::Fixture,
            )))
            .unwrap();

        let status = bridge
            .dispatch_text_update(
                &WebRuntimeTextInputTextUpdate::new(
                    TextRange::new(TextUtf16Offset(0), TextUtf16Offset(0)),
                    "日本語",
                    TextRange::new(TextUtf16Offset(3), TextUtf16Offset(3)),
                )
                .with_observed_text_before(""),
            )
            .unwrap();
        let edits = bridge.drain_pending_edits();

        assert_eq!(status.operation_count, 2);
        assert_eq!(edits.len(), 1);
        assert_eq!(
            edits[0].key_disposition(),
            TextInputKeyDisposition::ImeConsumed
        );
        assert!(matches!(
            edits[0].input.operations(),
            [
                TextInputOperation::Commit(_),
                TextInputOperation::SetSelection(_)
            ]
        ));
    }

    #[test]
    fn runtime_snapshot_exposes_multiline_input_option() {
        let target = target("web.multiline");
        let mut bridge = WebPlayerTextInputBridge::new(
            "arcweft-canvas",
            WebEditContextFeatureDetection::new(true, true),
        );

        let sync = bridge
            .sync_focus(Some(WebRuntimeTextInputFocusedControl::new(
                snapshot_with_options(
                    8,
                    target,
                    "line",
                    TextInputOptions::default().multiline(true),
                ),
                geometry(8, "line"),
                WebRuntimeTextInputFocusReason::Fixture,
            )))
            .unwrap();

        let WebRuntimeTextInputCommand::Activate { snapshot } = &sync.commands()[0] else {
            panic!("activate command expected");
        };
        assert!(snapshot.multiline);
        assert!(!snapshot.secure);
    }

    #[test]
    fn blur_rejects_late_textupdate_without_sample_session() {
        let target = target("web.blur");
        let mut bridge = WebPlayerTextInputBridge::new(
            "arcweft-canvas",
            WebEditContextFeatureDetection::new(true, true),
        );
        bridge
            .sync_focus(Some(WebRuntimeTextInputFocusedControl::new(
                snapshot(9, target, "abc", false),
                geometry(9, "abc"),
                WebRuntimeTextInputFocusReason::Fixture,
            )))
            .unwrap();
        bridge.blur_active().unwrap();

        let error = bridge
            .dispatch_text_update(&WebRuntimeTextInputTextUpdate::new(
                TextRange::new(TextUtf16Offset(0), TextUtf16Offset(0)),
                "x",
                TextRange::new(TextUtf16Offset(1), TextUtf16Offset(1)),
            ))
            .expect_err("late callbacks reject after runtime blur");

        assert!(error.to_string().contains("no active text-input session"));
    }

    #[test]
    fn secure_snapshot_and_geometry_are_redacted_for_js_commands() {
        let target = target("web.secure");
        let mut bridge = WebPlayerTextInputBridge::new(
            "arcweft-canvas",
            WebEditContextFeatureDetection::new(true, true),
        );

        let sync = bridge
            .sync_focus(Some(WebRuntimeTextInputFocusedControl::new(
                snapshot(11, target, "secret", true),
                geometry(11, "secret"),
                WebRuntimeTextInputFocusReason::Fixture,
            )))
            .unwrap();

        let WebRuntimeTextInputCommand::Activate { snapshot } = &sync.commands()[0] else {
            panic!("activate command expected");
        };
        assert!(snapshot.secure);
        assert!(snapshot.text.is_empty());
        assert!(snapshot.character_bounds.is_empty());
        let WebRuntimeTextInputCommand::UpdateGeometry { geometry } = &sync.commands()[1] else {
            panic!("geometry command expected");
        };
        assert!(geometry.character_bounds.is_empty());
        assert!(geometry.selection_rects.is_empty());
    }
}
