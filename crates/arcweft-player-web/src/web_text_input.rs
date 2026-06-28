//! Player-owned Web `EditContext` bridge.
//!
//! Browser object identity and DOM events stay in the Web player boundary.  This
//! module exposes only value-shaped wasm entry points that reuse
//! [`crate::edit_context::WebEditContextAdapter`] for range normalization,
//! secure redaction, typed unsupported reporting, and runtime-host dispatch.

use crate::edit_context::WebEditContextFeatureDetection;
use arcweft_presentation::text_input::WebTextInputApiSupport;
use serde::Serialize;

#[cfg(target_arch = "wasm32")]
use crate::edit_context::{WebEditContextAdapter, WebEditContextError, WebEditContextTextUpdate};

#[cfg(target_arch = "wasm32")]
use arcweft_id::PublicId;

#[cfg(target_arch = "wasm32")]
use arcweft_presentation::hit::HitRect;

#[cfg(target_arch = "wasm32")]
use arcweft_presentation::input::{InputEpoch, InteractionTarget, RawInputKind};

#[cfg(target_arch = "wasm32")]
use arcweft_presentation::text_input::{
    CompositionEndReason, TextByteOffset, TextEditCommand, TextInputClientSnapshot,
    TextInputHostCommand, TextInputOptions, TextInputPrivacy, TextInputSessionId, TextRange,
    TextRevision, TextUtf16Offset,
};

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;

#[cfg(target_arch = "wasm32")]
use std::collections::BTreeMap;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WEB_TEXT_INPUT_SESSIONS: RefCell<BTreeMap<String, PlayerOwnedTextInputSession>> =
        RefCell::new(BTreeMap::new());
}

/// Player-visible status for Web text-input installation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerTextInputStatusKind {
    Disabled,
    Ready,
    UnsupportedNoFallback,
    Error,
}

/// Typed Web text-input setup status.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerTextInputStatus {
    state: PlayerTextInputStatusKind,
    api: WebTextInputApiSupportLabel,
    fallback_installed: bool,
    message: Option<String>,
}

/// Stable serializable label for browser API support.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WebTextInputApiSupportLabel {
    EditContext,
    UnsupportedNoFallback,
}

#[derive(Debug)]
#[cfg(target_arch = "wasm32")]
struct PlayerOwnedTextInputSession {
    adapter: WebEditContextAdapter,
    next_epoch: u64,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
#[cfg(target_arch = "wasm32")]
struct TextUpdatePayload {
    update_range_start: u32,
    update_range_end: u32,
    text: String,
    selection_start: u32,
    selection_end: u32,
    observed_text_before: Option<String>,
    composing: bool,
}

impl PlayerTextInputStatus {
    pub const fn ready(api: WebTextInputApiSupportLabel) -> Self {
        Self {
            state: PlayerTextInputStatusKind::Ready,
            api,
            fallback_installed: false,
            message: None,
        }
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self {
            state: PlayerTextInputStatusKind::UnsupportedNoFallback,
            api: WebTextInputApiSupportLabel::UnsupportedNoFallback,
            fallback_installed: false,
            message: Some(message.into()),
        }
    }

    pub const fn disabled() -> Self {
        Self {
            state: PlayerTextInputStatusKind::Disabled,
            api: WebTextInputApiSupportLabel::UnsupportedNoFallback,
            fallback_installed: false,
            message: None,
        }
    }

    pub fn error(error: impl Into<String>) -> Self {
        Self {
            state: PlayerTextInputStatusKind::Error,
            api: WebTextInputApiSupportLabel::UnsupportedNoFallback,
            fallback_installed: false,
            message: Some(error.into()),
        }
    }

    pub const fn state(&self) -> PlayerTextInputStatusKind {
        self.state
    }

    pub const fn fallback_installed(&self) -> bool {
        self.fallback_installed
    }
}

impl From<WebTextInputApiSupport> for WebTextInputApiSupportLabel {
    fn from(support: WebTextInputApiSupport) -> Self {
        match support {
            WebTextInputApiSupport::EditContext => Self::EditContext,
            WebTextInputApiSupport::UnsupportedNoFallback => Self::UnsupportedNoFallback,
        }
    }
}

/// Returns the player-owned status for a feature-detection result.
pub fn status_for_detection(
    detection: WebEditContextFeatureDetection,
    enabled: bool,
) -> PlayerTextInputStatus {
    if !enabled {
        return PlayerTextInputStatus::disabled();
    }
    match detection.api_support() {
        WebTextInputApiSupport::EditContext => {
            PlayerTextInputStatus::ready(WebTextInputApiSupportLabel::EditContext)
        }
        WebTextInputApiSupport::UnsupportedNoFallback => PlayerTextInputStatus::unsupported(
            "Web EditContext is unavailable; no DOM text-entry fallback is installed",
        ),
    }
}

#[cfg(target_arch = "wasm32")]
impl PlayerOwnedTextInputSession {
    fn new(adapter: WebEditContextAdapter) -> Self {
        Self {
            adapter,
            next_epoch: 1,
        }
    }

    fn next_epoch(&mut self) -> InputEpoch {
        let epoch = InputEpoch(self.next_epoch);
        self.next_epoch = self.next_epoch.saturating_add(1);
        epoch
    }
}

#[cfg(target_arch = "wasm32")]
fn sample_snapshot(host_id: &str, initial_text: &str, secure: bool) -> TextInputClientSnapshot {
    let end = TextByteOffset(u32::try_from(initial_text.len()).unwrap_or(u32::MAX));
    TextInputClientSnapshot::new(
        TextInputSessionId(stable_session_id(host_id)),
        InteractionTarget::new(public_target_id(host_id)),
        TextRevision(1),
        initial_text,
        TextByteOffset(0),
        TextRange::new(end, end),
        HitRect::new(0.0, 0.0, 1.0, 1.0),
        HitRect::new(0.0, 0.0, 1.0, 1.0),
        TextInputOptions::default().secure(secure).multiline(true),
    )
}

#[cfg(target_arch = "wasm32")]
fn public_target_id(host_id: &str) -> PublicId {
    let mut value = String::from("target.web.editcontext");
    for ch in host_id.chars() {
        if ch.is_ascii_alphanumeric() {
            value.push('.');
            value.push(ch.to_ascii_lowercase());
        }
    }
    PublicId::try_new(value).unwrap_or_else(|_| {
        PublicId::try_new("target.web.editcontext.host").expect("fallback id is valid")
    })
}

#[cfg(target_arch = "wasm32")]
fn stable_session_id(host_id: &str) -> u64 {
    host_id.bytes().fold(0xA4_10_06_04_A1, |hash, byte| {
        hash.wrapping_mul(16_777_619).wrapping_add(u64::from(byte))
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn arcweft_web_text_input_support_for_host(host_id: String) -> Result<JsValue, JsValue> {
    let element = element_by_id(&host_id)?;
    let detection = WebEditContextFeatureDetection::detect_for_element(&element);
    status_to_js(status_for_detection(detection, true))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn arcweft_web_text_input_activate(
    host_id: String,
    initial_text: String,
    secure: bool,
) -> Result<JsValue, JsValue> {
    let element = element_by_id(&host_id)?;
    let detection = WebEditContextFeatureDetection::detect_for_element(&element);
    let status = status_for_detection(detection, true);
    if status.state() != PlayerTextInputStatusKind::Ready {
        return status_to_js(status);
    }

    let mut adapter = WebEditContextAdapter::default();
    let snapshot = sample_snapshot(&host_id, &initial_text, secure);
    let activation = adapter
        .activate(&snapshot, detection)
        .map_err(error_to_js_value)?;
    WEB_TEXT_INPUT_SESSIONS.with(|sessions| {
        sessions
            .borrow_mut()
            .insert(host_id.clone(), PlayerOwnedTextInputSession::new(adapter));
    });

    command_status_to_js("activated", activation.into_commands(), secure)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn arcweft_web_text_input_dispatch_text_update(
    host_id: String,
    payload: JsValue,
) -> Result<JsValue, JsValue> {
    let payload = text_update_payload_from_js(&payload)?;
    WEB_TEXT_INPUT_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let session = sessions
            .get_mut(&host_id)
            .ok_or_else(|| JsValue::from_str("no active Arcweft Web text-input session"))?;
        let update = WebEditContextTextUpdate::new(
            TextRange::new(
                TextUtf16Offset(payload.update_range_start),
                TextUtf16Offset(payload.update_range_end),
            ),
            payload.text,
            TextRange::new(
                TextUtf16Offset(payload.selection_start),
                TextUtf16Offset(payload.selection_end),
            ),
        )
        .composing(payload.composing);
        let update = match payload.observed_text_before {
            Some(observed) => update.with_observed_text_before(observed),
            None => update,
        };
        let epoch = session.next_epoch();
        let output = session
            .adapter
            .dispatch_text_update(epoch, &update)
            .map_err(error_to_js_value)?;
        dispatch_status_to_js("textupdate", output.raw().kind())
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn arcweft_web_text_input_composition_start(host_id: String) -> Result<JsValue, JsValue> {
    WEB_TEXT_INPUT_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let session = sessions
            .get_mut(&host_id)
            .ok_or_else(|| JsValue::from_str("no active Arcweft Web text-input session"))?;
        let epoch = session.next_epoch();
        let output = session
            .adapter
            .dispatch_composition_start(epoch)
            .map_err(error_to_js_value)?;
        dispatch_status_to_js("compositionstart", output.raw().kind())
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn arcweft_web_text_input_composition_end(
    host_id: String,
    cancelled: bool,
) -> Result<JsValue, JsValue> {
    WEB_TEXT_INPUT_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let session = sessions
            .get_mut(&host_id)
            .ok_or_else(|| JsValue::from_str("no active Arcweft Web text-input session"))?;
        let epoch = session.next_epoch();
        let reason = if cancelled {
            CompositionEndReason::Cancelled
        } else {
            CompositionEndReason::Committed
        };
        let output = session
            .adapter
            .dispatch_composition_end(epoch, reason)
            .map_err(error_to_js_value)?;
        dispatch_status_to_js("compositionend", output.raw().kind())
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn arcweft_web_text_input_dispatch_command(
    host_id: String,
    command: String,
    selecting: bool,
) -> Result<JsValue, JsValue> {
    let command = command_from_label(&command, selecting)?;
    WEB_TEXT_INPUT_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let session = sessions
            .get_mut(&host_id)
            .ok_or_else(|| JsValue::from_str("no active Arcweft Web text-input session"))?;
        let epoch = session.next_epoch();
        let output = session
            .adapter
            .dispatch_command(epoch, command)
            .map_err(error_to_js_value)?;
        dispatch_status_to_js("command", output.raw().kind())
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn arcweft_web_text_input_deactivate(host_id: String) -> Result<JsValue, JsValue> {
    let commands = WEB_TEXT_INPUT_SESSIONS.with(|sessions| {
        sessions
            .borrow_mut()
            .remove(&host_id)
            .map(|mut session| session.adapter.deactivate().into_commands())
            .unwrap_or_default()
    });
    command_status_to_js("deactivated", commands, false)
}

#[cfg(target_arch = "wasm32")]
fn command_from_label(label: &str, selecting: bool) -> Result<TextEditCommand, JsValue> {
    Ok(match label {
        "move_left" => TextEditCommand::MoveLeft { selecting },
        "move_right" => TextEditCommand::MoveRight { selecting },
        "move_word_left" => TextEditCommand::MoveWordLeft { selecting },
        "move_word_right" => TextEditCommand::MoveWordRight { selecting },
        "move_line_start" => TextEditCommand::MoveLineStart { selecting },
        "move_line_end" => TextEditCommand::MoveLineEnd { selecting },
        "backspace" => TextEditCommand::Backspace,
        "delete" => TextEditCommand::Delete,
        "select_all" => TextEditCommand::SelectAll,
        "copy" => TextEditCommand::Copy,
        "cut" => TextEditCommand::Cut,
        "paste" => TextEditCommand::Paste,
        "submit" => TextEditCommand::Submit,
        "cancel" => TextEditCommand::Cancel,
        other => {
            return Err(JsValue::from_str(&format!(
                "unknown Arcweft Web text-input command `{other}`"
            )));
        }
    })
}

#[cfg(target_arch = "wasm32")]
fn element_by_id(host_id: &str) -> Result<web_sys::Element, JsValue> {
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(host_id))
        .ok_or_else(|| JsValue::from_str(&format!("Arcweft text-input host `{host_id}` not found")))
}

#[cfg(target_arch = "wasm32")]
fn text_update_payload_from_js(value: &JsValue) -> Result<TextUpdatePayload, JsValue> {
    Ok(TextUpdatePayload {
        update_range_start: u32_prop(value, "updateRangeStart")?,
        update_range_end: u32_prop(value, "updateRangeEnd")?,
        text: string_prop(value, "text")?,
        selection_start: u32_prop(value, "selectionStart")?,
        selection_end: u32_prop(value, "selectionEnd")?,
        observed_text_before: optional_string_prop(value, "observedTextBefore"),
        composing: bool_prop(value, "composing"),
    })
}

#[cfg(target_arch = "wasm32")]
fn u32_prop(value: &JsValue, name: &str) -> Result<u32, JsValue> {
    let raw = js_sys::Reflect::get(value, &JsValue::from_str(name))?;
    raw.as_f64()
        .and_then(|value| u32::try_from(value as i64).ok())
        .ok_or_else(|| JsValue::from_str(&format!("missing numeric `{name}`")))
}

#[cfg(target_arch = "wasm32")]
fn string_prop(value: &JsValue, name: &str) -> Result<String, JsValue> {
    js_sys::Reflect::get(value, &JsValue::from_str(name))?
        .as_string()
        .ok_or_else(|| JsValue::from_str(&format!("missing string `{name}`")))
}

#[cfg(target_arch = "wasm32")]
fn optional_string_prop(value: &JsValue, name: &str) -> Option<String> {
    js_sys::Reflect::get(value, &JsValue::from_str(name))
        .ok()
        .and_then(|value| value.as_string())
}

#[cfg(target_arch = "wasm32")]
fn bool_prop(value: &JsValue, name: &str) -> bool {
    js_sys::Reflect::get(value, &JsValue::from_str(name))
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

#[cfg(target_arch = "wasm32")]
fn status_to_js(status: PlayerTextInputStatus) -> Result<JsValue, JsValue> {
    let object = js_sys::Object::new();
    set_str(&object, "state", status_kind_label(status.state));
    set_str(&object, "api", api_label(status.api));
    set_bool(&object, "fallbackInstalled", status.fallback_installed);
    if let Some(message) = status.message {
        set_str(&object, "message", &message);
    }
    Ok(object.into())
}

#[cfg(target_arch = "wasm32")]
fn command_status_to_js(
    state: &str,
    commands: Vec<TextInputHostCommand>,
    secure: bool,
) -> Result<JsValue, JsValue> {
    let object = js_sys::Object::new();
    set_str(&object, "state", state);
    set_bool(&object, "fallbackInstalled", false);
    set_bool(&object, "secure", secure);
    set_f64(&object, "hostCommandCount", commands.len() as f64);
    Ok(object.into())
}

#[cfg(target_arch = "wasm32")]
fn dispatch_status_to_js(state: &str, kind: &RawInputKind) -> Result<JsValue, JsValue> {
    let object = js_sys::Object::new();
    set_str(&object, "state", state);
    set_bool(&object, "fallbackInstalled", false);
    if let RawInputKind::Text(input) = kind {
        set_f64(&object, "operationCount", input.operations().len() as f64);
        set_str(
            &object,
            "privacy",
            match input.privacy() {
                TextInputPrivacy::Plain => "plain",
                TextInputPrivacy::Sensitive => "sensitive",
            },
        );
    }
    Ok(object.into())
}

#[cfg(target_arch = "wasm32")]
fn status_kind_label(kind: PlayerTextInputStatusKind) -> &'static str {
    match kind {
        PlayerTextInputStatusKind::Disabled => "disabled",
        PlayerTextInputStatusKind::Ready => "ready",
        PlayerTextInputStatusKind::UnsupportedNoFallback => "unsupported_no_fallback",
        PlayerTextInputStatusKind::Error => "error",
    }
}

#[cfg(target_arch = "wasm32")]
fn api_label(api: WebTextInputApiSupportLabel) -> &'static str {
    match api {
        WebTextInputApiSupportLabel::EditContext => "edit_context",
        WebTextInputApiSupportLabel::UnsupportedNoFallback => "unsupported_no_fallback",
    }
}

#[cfg(target_arch = "wasm32")]
fn set_str(object: &js_sys::Object, name: &str, value: &str) {
    let _ = js_sys::Reflect::set(object, &JsValue::from_str(name), &JsValue::from_str(value));
}

#[cfg(target_arch = "wasm32")]
fn set_bool(object: &js_sys::Object, name: &str, value: bool) {
    let _ = js_sys::Reflect::set(object, &JsValue::from_str(name), &JsValue::from_bool(value));
}

#[cfg(target_arch = "wasm32")]
fn set_f64(object: &js_sys::Object, name: &str, value: f64) {
    let _ = js_sys::Reflect::set(object, &JsValue::from_str(name), &JsValue::from_f64(value));
}

#[cfg(target_arch = "wasm32")]
fn error_to_js_value(error: WebEditContextError) -> JsValue {
    JsValue::from_str(&error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_status_is_typed_and_never_installs_fallback() {
        let status = status_for_detection(WebEditContextFeatureDetection::new(false, false), true);

        assert_eq!(
            status.state(),
            PlayerTextInputStatusKind::UnsupportedNoFallback
        );
        assert!(!status.fallback_installed());
    }

    #[test]
    fn ready_status_requires_constructor_and_element_property() {
        let status = status_for_detection(WebEditContextFeatureDetection::new(true, true), true);

        assert_eq!(status.state(), PlayerTextInputStatusKind::Ready);
        assert!(!status.fallback_installed());
    }

    #[test]
    fn disabled_status_is_explicit() {
        let status = status_for_detection(WebEditContextFeatureDetection::new(true, true), false);

        assert_eq!(status.state(), PlayerTextInputStatusKind::Disabled);
        assert!(!status.fallback_installed());
    }
}
