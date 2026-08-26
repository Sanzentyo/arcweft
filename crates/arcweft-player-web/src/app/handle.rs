use super::environment::{
    WebEnvironmentError, decode_environment_snapshot, environment_update_result,
};
use super::event_loop;
use super::registry::{self, WebPlayerControl};
use super::{GpuState, PlayerState, WebPlayerError};
use crate::clock::LogicalClockQuantizer;
use crate::edit_context::WebEditContextFeatureDetection;
use crate::host::BrowserTaskBroker;
use crate::runtime_text_input::register_runtime_bridge;
use arcweft_bundle::ArcweftBundle;
use arcweft_core::plan::EntryRuntimeId;
use arcweft_layout::ScalePolicy;
use arcweft_player_scene::dialogue::DialogueVisualClock;
use arcweft_player_scene::fonts::PlayerFontSet;
use arcweft_player_scene::frame::{PlayerFrameFit, PlayerFramePlannerState};
use arcweft_player_scene::images::BundleImageCatalog;
use arcweft_player_scene::input::InputController;
use arcweft_runtime_driver::session::PresentationEnvironmentUpdateError;
use arcweft_runtime_driver::session::{BundleSession, BundleSessionOptions};
use js_sys::{Array, Uint8Array};
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use winit::keyboard::ModifiersState;

#[derive(Clone, Debug, Eq, PartialEq)]
struct WebPlayerOptions {
    frame_fit: PlayerFrameFit,
    additional_font_bytes: Vec<Vec<u8>>,
    entry: Option<EntryRuntimeId>,
}

impl Default for WebPlayerOptions {
    fn default() -> Self {
        Self {
            frame_fit: PlayerFrameFit::design_1280x720(ScalePolicy::Contain),
            additional_font_bytes: Vec::new(),
            entry: None,
        }
    }
}

/// Starts the WebGPU-first browser player using already-fetched bundle/font bytes.
#[wasm_bindgen]
pub fn start_arcweft_player(
    canvas_id: String,
    bundle_bytes: Vec<u8>,
    font_bytes: Vec<u8>,
) -> Result<(), JsValue> {
    let handle = create_arcweft_player(canvas_id, bundle_bytes, font_bytes)?;
    handle
        .retain_for_start()
        .map_err(WebEnvironmentError::into_js_value)
}

#[wasm_bindgen]
pub fn start_arcweft_player_with_options(
    canvas_id: String,
    bundle_bytes: Vec<u8>,
    font_bytes: Vec<u8>,
    options: JsValue,
) -> Result<(), JsValue> {
    let handle = create_arcweft_player_with_options(canvas_id, bundle_bytes, font_bytes, options)?;
    handle
        .retain_for_start()
        .map_err(WebEnvironmentError::into_js_value)
}

#[wasm_bindgen]
pub fn create_arcweft_player(
    canvas_id: String,
    bundle_bytes: Vec<u8>,
    font_bytes: Vec<u8>,
) -> Result<ArcweftWebPlayerHandle, JsValue> {
    create(
        canvas_id,
        bundle_bytes,
        font_bytes,
        WebPlayerOptions::default(),
    )
    .map_err(WebPlayerError::into_js_value)
}

#[wasm_bindgen]
pub fn create_arcweft_player_with_options(
    canvas_id: String,
    bundle_bytes: Vec<u8>,
    font_bytes: Vec<u8>,
    options: JsValue,
) -> Result<ArcweftWebPlayerHandle, JsValue> {
    let options = web_player_options_from_js(&options)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    create(canvas_id, bundle_bytes, font_bytes, options).map_err(WebPlayerError::into_js_value)
}

fn create(
    canvas_id: String,
    bundle_bytes: Vec<u8>,
    font_bytes: Vec<u8>,
    options: WebPlayerOptions,
) -> Result<ArcweftWebPlayerHandle, WebPlayerError> {
    let WebPlayerOptions {
        frame_fit,
        additional_font_bytes,
        entry,
    } = options;
    let mut font_resources = Vec::with_capacity(additional_font_bytes.len().saturating_add(1));
    font_resources.push(font_bytes);
    font_resources.extend(additional_font_bytes);
    let font_set = PlayerFontSet::from_font_resource_bytes(font_resources)
        .map_err(|error| WebPlayerError::Font(error.to_string()))?;

    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or(WebPlayerError::MissingDocument)?;
    let element = document
        .get_element_by_id(&canvas_id)
        .ok_or_else(|| WebPlayerError::MissingCanvas(canvas_id.clone()))?;
    let canvas = element
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| WebPlayerError::NotCanvas(canvas_id.clone()))?;
    let detection = WebEditContextFeatureDetection::detect_for_element(canvas.unchecked_ref());
    let text_input = register_runtime_bridge(canvas_id.clone(), detection);
    let mut session_options = BundleSessionOptions::default();
    session_options.entry = entry;
    let bundle = ArcweftBundle::from_awfb_slice_with_resource_types(
        &bundle_bytes,
        session_options.engine_resource_types.as_ref(),
    )
    .map_err(|error| WebPlayerError::BundleDecode(error.to_string()))?;
    let session = BundleSession::new(&bundle, session_options)
        .map_err(|error| WebPlayerError::Session(error.to_string()))?;
    let broker = BrowserTaskBroker::from_bundle(&bundle)
        .map_err(|error| WebPlayerError::TaskBroker(error.to_string()))?;
    let images = BundleImageCatalog::from_bundle(&bundle)
        .map_err(|error| WebPlayerError::Image(error.to_string()))?;
    let clock = LogicalClockQuantizer::new(16, 4)
        .map_err(|error| WebPlayerError::Session(error.to_string()))?;
    let state = PlayerState {
        canvas,
        window: None,
        gpu: GpuState::Uninitialized,
        session,
        broker,
        images,
        input: InputController::default(),
        frame_planner: PlayerFramePlannerState::new(),
        text_input,
        keyboard_modifiers: ModifiersState::default(),
        frame_fit,
        clock,
        font_set: Some(font_set),
        prepared: None,
        dialogue_visual_clock: DialogueVisualClock::default(),
        fatal: None,
    };
    let control = registry::create_control(canvas_id, state)?;
    if let Err(error) = event_loop::attach(&control) {
        registry::shutdown_after_event_loop_failure(&control);
        return Err(error);
    }
    Ok(ArcweftWebPlayerHandle::created(control))
}

fn web_player_options_from_js(options: &JsValue) -> Result<WebPlayerOptions, WebPlayerError> {
    let mut parsed = WebPlayerOptions::default();
    if let Some(frame_fit) = js_property(options, "frameFit") {
        let fit = js_string_property(&frame_fit, "fit").unwrap_or_else(|| "contain".to_owned());
        let design_width = js_u32_property(&frame_fit, "designWidth")
            .or_else(|| js_u32_property(&frame_fit, "design_width"))
            .unwrap_or(1280);
        let design_height = js_u32_property(&frame_fit, "designHeight")
            .or_else(|| js_u32_property(&frame_fit, "design_height"))
            .unwrap_or(720);
        parsed.frame_fit = match fit.as_str() {
            "raw" | "none" => PlayerFrameFit::raw(),
            "cover" => PlayerFrameFit::design(design_width, design_height, ScalePolicy::Cover),
            "stretch" => PlayerFrameFit::design(design_width, design_height, ScalePolicy::Stretch),
            _ => PlayerFrameFit::design(design_width, design_height, ScalePolicy::Contain),
        };
    }
    parsed.additional_font_bytes = js_u8_array_list_property(options, "additionalFontBytes")?;
    parsed.entry = js_string_property(options, "entry")
        .map(parse_web_entry_selection)
        .transpose()?;
    Ok(parsed)
}

fn parse_web_entry_selection(entry: String) -> Result<EntryRuntimeId, WebPlayerError> {
    EntryRuntimeId::from_source_entity_body(&entry).map_err(|error| {
        WebPlayerError::InvalidEntrySelection {
            entry,
            message: error.to_string(),
        }
    })
}

fn js_property(parent: &JsValue, key: &str) -> Option<JsValue> {
    let value = js_sys::Reflect::get(parent, &JsValue::from_str(key)).ok()?;
    (!value.is_undefined() && !value.is_null()).then_some(value)
}

fn js_string_property(parent: &JsValue, key: &str) -> Option<String> {
    js_property(parent, key)?.as_string()
}

fn js_u32_property(parent: &JsValue, key: &str) -> Option<u32> {
    let number = js_property(parent, key)?.as_f64()?.round();
    if !number.is_finite() || number < 1.0 {
        return None;
    }
    Some(number.min(f64::from(u32::MAX)) as u32)
}

fn js_u8_array_list_property(parent: &JsValue, key: &str) -> Result<Vec<Vec<u8>>, WebPlayerError> {
    let Some(value) = js_property(parent, key) else {
        return Ok(Vec::new());
    };
    if !Array::is_array(&value) {
        return Err(WebPlayerError::Font(format!(
            "`{key}` must be an array of Uint8Array font resources"
        )));
    }
    let array = Array::from(&value);
    array
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let typed = value.dyn_ref::<Uint8Array>().ok_or_else(|| {
                WebPlayerError::Font(format!("`{key}[{index}]` must be a Uint8Array"))
            })?;
            let mut bytes = vec![0; typed.length() as usize];
            typed.copy_to(&mut bytes);
            Ok(bytes)
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WebPlayerHandleDropPolicy {
    ShutdownIfUnretained,
    BorrowedRegistry,
}

/// Stable JavaScript ownership handle for one browser player.
#[wasm_bindgen]
pub struct ArcweftWebPlayerHandle {
    id: u32,
    control: Rc<WebPlayerControl>,
    drop_policy: WebPlayerHandleDropPolicy,
}

impl ArcweftWebPlayerHandle {
    pub(super) fn created(control: Rc<WebPlayerControl>) -> Self {
        Self {
            id: control.id(),
            control,
            drop_policy: WebPlayerHandleDropPolicy::ShutdownIfUnretained,
        }
    }

    pub(super) fn retain_for_start(&self) -> Result<(), WebEnvironmentError> {
        registry::retain(&self.control)
    }

    fn borrowed(control: Rc<WebPlayerControl>) -> Self {
        Self {
            id: control.id(),
            control,
            drop_policy: WebPlayerHandleDropPolicy::BorrowedRegistry,
        }
    }
}

#[wasm_bindgen]
impl ArcweftWebPlayerHandle {
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> u32 {
        self.id
    }

    #[wasm_bindgen(js_name = setEnvironment)]
    pub fn set_environment(&self, snapshot: JsValue) -> Result<JsValue, JsValue> {
        let values =
            decode_environment_snapshot(&snapshot).map_err(WebEnvironmentError::into_js_value)?;
        if self.control.is_closed() {
            return Err(WebEnvironmentError::player_closed(self.id).into_js_value());
        }
        let mut player =
            self.control.player.try_borrow_mut().map_err(|_| {
                WebEnvironmentError::reentrant_update(Some(self.id)).into_js_value()
            })?;
        let state = player
            .as_mut()
            .ok_or_else(|| WebEnvironmentError::player_closed(self.id).into_js_value())?;
        let update = state
            .session
            .update_presentation_environment_provider(values)
            .map_err(|error| match error {
                PresentationEnvironmentUpdateError::RevisionOverflow => {
                    WebEnvironmentError::revision_overflow(self.id)
                }
                PresentationEnvironmentUpdateError::FieldRevisionOverflow { field } => {
                    WebEnvironmentError::field_revision_overflow(self.id, field)
                }
            })
            .map_err(WebEnvironmentError::into_js_value)?;
        let invalidation = state
            .frame_planner
            .apply_environment_update(update)
            .map_err(|error| {
                WebEnvironmentError::invalid_snapshot(format!(
                    "player environment invalidation failed: {error}"
                ))
                .into_js_value()
            })?;
        if invalidation.prepared_work_discarded() {
            state.prepared = None;
        }
        if invalidation.redraw_requested()
            && let Some(window) = state.window.as_ref()
        {
            window.request_redraw();
        }
        Ok(environment_update_result(
            self.id,
            update.current().revision().value(),
            update.effective_changed_fields(),
            invalidation.redraw_requested(),
        ))
    }

    pub fn shutdown(&self) -> Result<(), JsValue> {
        registry::shutdown(&self.control).map_err(WebEnvironmentError::into_js_value)
    }
}

impl Drop for ArcweftWebPlayerHandle {
    fn drop(&mut self) {
        if self.drop_policy == WebPlayerHandleDropPolicy::ShutdownIfUnretained
            && !self.control.registry_retained.get()
        {
            registry::shutdown_on_drop(&self.control);
        }
    }
}

#[wasm_bindgen]
pub fn stop_arcweft_player(player_id: u32) -> Result<(), JsValue> {
    registry::stop(player_id).map_err(WebEnvironmentError::into_js_value)
}

#[wasm_bindgen]
pub fn arcweft_player_handle(player_id: u32) -> Result<ArcweftWebPlayerHandle, JsValue> {
    registry::lookup(player_id)
        .map(ArcweftWebPlayerHandle::borrowed)
        .map_err(WebEnvironmentError::into_js_value)
}
