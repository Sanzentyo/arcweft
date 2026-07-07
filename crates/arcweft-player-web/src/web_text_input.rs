//! Value-shaped wasm entry points for player-owned Web `EditContext` input.
//!
//! This module no longer owns a sample text session.  The active session belongs
//! to [`crate::runtime_text_input::WebPlayerTextInputBridge`], which is created
//! by the normal Web player runtime loop after `startArcweftWebPlayer(...)` starts
//! the wasm player.  JavaScript calls back with primitive values only; the bridge
//! validates focus generation, session identity, secure redaction, and text ranges
//! before `app.rs` drains edits into `InputController::text_input`.

use crate::edit_context::WebEditContextFeatureDetection;
use arcweft_presentation::text_input::WebTextInputApiSupport;
use serde::Serialize;

#[cfg(target_arch = "wasm32")]
use crate::runtime_text_input::{
    WebRuntimeTextInputBridgeError, WebRuntimeTextInputDispatchStatus,
    WebRuntimeTextInputTextUpdate, dispatch_registered_command,
    dispatch_registered_composition_end, dispatch_registered_composition_start,
    dispatch_registered_text_update,
};

#[cfg(target_arch = "wasm32")]
use arcweft_presentation::text_input::{TextRange, TextUtf16Offset};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsValue, prelude::*};

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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebTextInputCommandIntent {
    name: &'static str,
    selecting: bool,
    suppressed_during_composition: bool,
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

impl WebTextInputCommandIntent {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            selecting: false,
            suppressed_during_composition: false,
        }
    }

    pub const fn selecting(mut self, selecting: bool) -> Self {
        self.selecting = selecting;
        self
    }

    pub const fn suppressed_during_composition(mut self) -> Self {
        self.suppressed_during_composition = true;
        self
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub const fn is_selecting(&self) -> bool {
        self.selecting
    }

    pub const fn suppressed(&self) -> bool {
        self.suppressed_during_composition
    }
}

pub fn command_intent_for_key_event(
    key: &str,
    ctrl_key: bool,
    meta_key: bool,
    alt_key: bool,
    shift_key: bool,
) -> Option<WebTextInputCommandIntent> {
    let shortcut = meta_key || ctrl_key;
    if shortcut && !alt_key {
        match key.to_ascii_lowercase().as_str() {
            "a" => {
                return Some(
                    WebTextInputCommandIntent::new("select_all").suppressed_during_composition(),
                );
            }
            "c" => {
                return Some(
                    WebTextInputCommandIntent::new("copy").suppressed_during_composition(),
                );
            }
            "x" => {
                return Some(WebTextInputCommandIntent::new("cut").suppressed_during_composition());
            }
            "v" => {
                return Some(
                    WebTextInputCommandIntent::new("paste").suppressed_during_composition(),
                );
            }
            _ => {}
        }
    }
    let selecting = shift_key;
    let name = match key {
        "ArrowLeft" if alt_key || ctrl_key => "move_word_left",
        "ArrowLeft" => "move_left",
        "ArrowRight" if alt_key || ctrl_key => "move_word_right",
        "ArrowRight" => "move_right",
        "ArrowUp" => "move_up",
        "ArrowDown" => "move_down",
        "Home" if shortcut => "move_document_start",
        "Home" => "move_line_start",
        "End" if shortcut => "move_document_end",
        "End" => "move_line_end",
        "PageUp" => "move_page_up",
        "PageDown" => "move_page_down",
        "Backspace" if alt_key || ctrl_key => "delete_word_left",
        "Backspace" => "backspace",
        "Delete" if alt_key || ctrl_key => "delete_word_right",
        "Delete" => "delete",
        "Enter" => "submit",
        "Escape" => {
            return Some(WebTextInputCommandIntent::new("cancel").suppressed_during_composition());
        }
        _ => return None,
    };
    Some(WebTextInputCommandIntent::new(name).selecting(selecting))
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
#[wasm_bindgen]
pub fn arcweft_web_text_input_support_for_host(host_id: String) -> Result<JsValue, JsValue> {
    let element = element_by_id(&host_id)?;
    let detection = WebEditContextFeatureDetection::detect_for_element(&element);
    status_to_js(status_for_detection(detection, true))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn arcweft_web_text_input_runtime_dispatch_text_update(
    host_id: String,
    payload: JsValue,
) -> Result<JsValue, JsValue> {
    let payload = text_update_payload_from_js(&payload)?;
    let update = WebRuntimeTextInputTextUpdate::new(
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
    dispatch_status_to_js(dispatch_registered_text_update(&host_id, &update)?)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn arcweft_web_text_input_runtime_composition_start(
    host_id: String,
) -> Result<JsValue, JsValue> {
    dispatch_status_to_js(dispatch_registered_composition_start(&host_id)?)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn arcweft_web_text_input_runtime_composition_end(
    host_id: String,
    cancelled: bool,
) -> Result<JsValue, JsValue> {
    dispatch_status_to_js(dispatch_registered_composition_end(&host_id, cancelled)?)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn arcweft_web_text_input_runtime_dispatch_command(
    host_id: String,
    command: String,
    selecting: bool,
) -> Result<JsValue, JsValue> {
    dispatch_status_to_js(dispatch_registered_command(&host_id, &command, selecting)?)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn arcweft_web_text_input_create_edit_context(
    initial_text: String,
    secure: bool,
) -> Result<JsValue, JsValue> {
    let window = web_sys::window()
        .ok_or_else(|| JsValue::from_str("Web EditContext is unavailable: window missing"))?;
    let constructor = js_sys::Reflect::get(&window, &JsValue::from_str("EditContext"))?;
    if !constructor.is_function() {
        return Err(JsValue::from_str(
            "Web EditContext is unavailable: constructor missing",
        ));
    }
    let options = js_sys::Object::new();
    set_str(
        &options,
        "text",
        if secure { "" } else { initial_text.as_str() },
    );
    let args = js_sys::Array::new();
    args.push(&options);
    let constructor: js_sys::Function = constructor.unchecked_into();
    js_sys::Reflect::construct(&constructor, &args)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn arcweft_web_text_input_command_for_key_event(
    key: String,
    ctrl_key: bool,
    meta_key: bool,
    alt_key: bool,
    shift_key: bool,
) -> Result<JsValue, JsValue> {
    Ok(
        command_intent_for_key_event(&key, ctrl_key, meta_key, alt_key, shift_key)
            .map_or(JsValue::NULL, command_intent_to_js),
    )
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
fn dispatch_status_to_js(status: WebRuntimeTextInputDispatchStatus) -> Result<JsValue, JsValue> {
    let object = js_sys::Object::new();
    set_str(&object, "state", status.state());
    set_bool(&object, "fallbackInstalled", status.fallback_installed());
    set_f64(&object, "operationCount", status.operation_count() as f64);
    set_f64(
        &object,
        "pendingEditCount",
        status.pending_edit_count() as f64,
    );
    set_str(&object, "privacy", status.privacy());
    set_str(&object, "keyDisposition", status.key_disposition());
    Ok(object.into())
}

#[cfg(target_arch = "wasm32")]
fn command_intent_to_js(intent: WebTextInputCommandIntent) -> JsValue {
    let object = js_sys::Object::new();
    set_str(&object, "name", intent.name());
    set_bool(&object, "selecting", intent.is_selecting());
    set_bool(&object, "suppressedDuringComposition", intent.suppressed());
    object.into()
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
impl From<WebRuntimeTextInputBridgeError> for JsValue {
    fn from(error: WebRuntimeTextInputBridgeError) -> Self {
        JsValue::from_str(&error.to_string())
    }
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

    #[test]
    fn key_event_command_intent_is_owned_by_rust() {
        assert_eq!(
            command_intent_for_key_event("ArrowLeft", true, false, false, true).unwrap(),
            WebTextInputCommandIntent::new("move_word_left").selecting(true)
        );
        assert_eq!(
            command_intent_for_key_event("PageDown", false, false, false, false).unwrap(),
            WebTextInputCommandIntent::new("move_page_down")
        );
        assert!(
            command_intent_for_key_event("c", true, false, false, false)
                .unwrap()
                .suppressed()
        );
        assert!(command_intent_for_key_event("z", false, false, false, false).is_none());
    }
}
