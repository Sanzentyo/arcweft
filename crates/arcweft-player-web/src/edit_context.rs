//! Web `EditContext` text-input adapter.
//!
//! This module owns browser `EditContext` feature detection and event
//! normalization. It never installs hidden DOM text-entry fallbacks.

#[cfg(target_arch = "wasm32")]
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::{InputEpoch, InteractionTarget};
use arcweft_presentation::text_index::{TextIndexError, TextIndexSnapshot};
use arcweft_presentation::text_input::{
    CompositionEndReason, PlatformTextInputContext, PlatformTextInputEvent, PlatformTextSelection,
    TextByteOffset, TextCommit, TextCompositionUpdate, TextInputAdapterKind, TextInputCapabilities,
    TextInputCapabilitySupport, TextInputClientSnapshot, TextInputFocusGeneration,
    TextInputGeometrySnapshot, TextInputHostCommand, TextInputKeyDisposition, TextInputOperation,
    TextInputSecurityPolicy, TextInputSerial, TextInputSessionId, TextRange, TextRevision,
    TextSelectionAffinity, TextUtf16Offset, WebTextInputApiSupport,
};
use arcweft_runtime_host::text_input_dispatch::{
    TextInputDispatchError, TextInputDispatchOutput, TextInputDispatchState,
    TextInputFocusTransaction, web_edit_context_capabilities,
};
use thiserror::Error;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebEditContextCapability {
    EditContextConstructor,
    ElementAssociation,
    SurroundingText,
    CharacterBounds,
    CompositionSegments,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WebEditContextFeatureDetection {
    edit_context_constructor: bool,
    element_edit_context_property: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebEditContextTextUpdate {
    update_range: TextRange<TextUtf16Offset>,
    text: String,
    selection: TextRange<TextUtf16Offset>,
    observed_text_before: Option<String>,
    composing: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WebEditContextActiveSession {
    session: TextInputSessionId,
    target: InteractionTarget,
    generation: TextInputFocusGeneration,
    revision: TextRevision,
    index: TextIndexSnapshot,
    capabilities: TextInputCapabilities,
    security: TextInputSecurityPolicy,
    composing: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WebEditContextAdapter {
    dispatch: TextInputDispatchState,
    active: Option<WebEditContextActiveSession>,
    next_serial: u64,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum WebEditContextError {
    #[error("Web EditContext is unavailable; no DOM text-entry fallback is allowed")]
    WebEditContextUnavailable,
    #[error("Web EditContext capability {capability:?} is secure-redacted")]
    SecureRedacted {
        capability: WebEditContextCapability,
    },
    #[error("Web EditContext selection {selection:?} is outside update range {update_range:?}")]
    SelectionOutsideUpdate {
        update_range: TextRange<TextUtf16Offset>,
        selection: TextRange<TextUtf16Offset>,
    },
    #[error(
        "stale Web EditContext text snapshot at revision {revision:?}: expected `{expected}`, observed `{observed}`"
    )]
    StaleTextSnapshot {
        revision: TextRevision,
        expected: String,
        observed: String,
    },
    #[error(transparent)]
    TextIndex(#[from] TextIndexError),
    #[error(transparent)]
    Dispatch(#[from] TextInputDispatchError),
    #[cfg(target_arch = "wasm32")]
    #[error("JavaScript EditContext call failed: {0}")]
    JavaScript(String),
}

impl WebEditContextFeatureDetection {
    pub const fn new(edit_context_constructor: bool, element_edit_context_property: bool) -> Self {
        Self {
            edit_context_constructor,
            element_edit_context_property,
        }
    }

    pub const fn api_support(self) -> WebTextInputApiSupport {
        if self.edit_context_constructor && self.element_edit_context_property {
            WebTextInputApiSupport::EditContext
        } else {
            WebTextInputApiSupport::UnsupportedNoFallback
        }
    }

    pub fn capabilities(self) -> Result<TextInputCapabilities, TextInputDispatchError> {
        web_edit_context_capabilities(self.api_support())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn detect_for_element(element: &web_sys::Element) -> Self {
        let edit_context_constructor = web_sys::window()
            .and_then(|window| {
                js_sys::Reflect::get(&window, &JsValue::from_str("EditContext")).ok()
            })
            .is_some_and(|value| value.is_function());
        let element_edit_context_property =
            js_sys::Reflect::has(element.as_ref(), &JsValue::from_str("editContext"))
                .unwrap_or(false);
        Self::new(edit_context_constructor, element_edit_context_property)
    }
}

impl WebEditContextTextUpdate {
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
    pub fn with_observed_text_before(mut self, observed_text_before: impl Into<String>) -> Self {
        self.observed_text_before = Some(observed_text_before.into());
        self
    }

    #[must_use]
    pub const fn composing(mut self, composing: bool) -> Self {
        self.composing = composing;
        self
    }

    pub const fn update_range(&self) -> TextRange<TextUtf16Offset> {
        self.update_range
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn selection(&self) -> TextRange<TextUtf16Offset> {
        self.selection
    }

    fn is_selection_only(&self) -> bool {
        self.text.is_empty() && self.update_range.start() == self.update_range.end()
    }
}

impl WebEditContextAdapter {
    pub const fn active(&self) -> Option<&WebEditContextActiveSession> {
        self.active.as_ref()
    }

    pub const fn next_serial_value(&self) -> u64 {
        self.next_serial
    }

    pub fn activate(
        &mut self,
        snapshot: &TextInputClientSnapshot,
        detection: WebEditContextFeatureDetection,
    ) -> Result<TextInputFocusTransaction, WebEditContextError> {
        let capabilities = match detection.capabilities() {
            Ok(capabilities) => capabilities,
            Err(TextInputDispatchError::WebEditContextUnavailable) => {
                return Err(WebEditContextError::WebEditContextUnavailable);
            }
            Err(error) => return Err(error.into()),
        };
        let security = TextInputSecurityPolicy::from_options(snapshot.options());
        let capabilities = capabilities.narrow_for_security(security);
        let activation = self
            .dispatch
            .activate_with_capabilities(snapshot, capabilities);
        self.active = Some(WebEditContextActiveSession {
            session: snapshot.session(),
            target: snapshot.target().clone(),
            generation: activation.generation(),
            revision: snapshot.revision(),
            index: TextIndexSnapshot::try_new(snapshot.surrounding_text().to_owned())?,
            capabilities,
            security,
            composing: false,
        });
        Ok(activation)
    }

    pub fn deactivate(&mut self) -> TextInputFocusTransaction {
        self.active = None;
        self.dispatch
            .blur(arcweft_presentation::text_input::TextInputBlurPolicy::PlatformDefault)
    }

    pub fn dispatch_composition_start(
        &mut self,
        epoch: InputEpoch,
    ) -> Result<TextInputDispatchOutput, WebEditContextError> {
        let context = self.next_context()?;
        let event = PlatformTextInputEvent::StartComposition(context);
        let output = self.dispatch.dispatch_platform_event(
            epoch,
            event,
            TextInputKeyDisposition::ImeConsumed,
        )?;
        if let Some(active) = &mut self.active {
            active.composing = true;
        }
        Ok(output)
    }

    pub fn dispatch_composition_end(
        &mut self,
        epoch: InputEpoch,
        reason: CompositionEndReason,
    ) -> Result<TextInputDispatchOutput, WebEditContextError> {
        let context = self.next_context()?;
        let event = PlatformTextInputEvent::EndComposition { context, reason };
        let output = self.dispatch.dispatch_platform_event(
            epoch,
            event,
            TextInputKeyDisposition::ImeConsumed,
        )?;
        if let Some(active) = &mut self.active {
            active.composing = false;
        }
        Ok(output)
    }

    pub fn dispatch_text_update(
        &mut self,
        epoch: InputEpoch,
        update: &WebEditContextTextUpdate,
    ) -> Result<TextInputDispatchOutput, WebEditContextError> {
        let (event, post_index, composing_after) = self.platform_event_for_text_update(update)?;
        let output = self.dispatch.dispatch_platform_event(
            epoch,
            event,
            TextInputKeyDisposition::ImeConsumed,
        )?;
        if let Some(active) = &mut self.active {
            active.index = post_index;
            active.composing = composing_after;
        }
        Ok(output)
    }

    pub fn geometry_command(
        &self,
        geometry: &TextInputGeometrySnapshot,
    ) -> Result<TextInputHostCommand, WebEditContextError> {
        Ok(self.dispatch.update_geometry(geometry)?)
    }

    fn platform_event_for_text_update(
        &mut self,
        update: &WebEditContextTextUpdate,
    ) -> Result<(PlatformTextInputEvent, TextIndexSnapshot, bool), WebEditContextError> {
        let active = self
            .active
            .as_ref()
            .ok_or(TextInputDispatchError::NoActiveSession)?;
        if let Some(observed) = &update.observed_text_before
            && observed != active.index.as_str()
        {
            return Err(WebEditContextError::StaleTextSnapshot {
                revision: active.revision,
                expected: active.index.as_str().to_owned(),
                observed: observed.clone(),
            });
        }
        let replacement = active.index.byte_range_from_utf16(update.update_range())?;
        let post_index = if update.is_selection_only() {
            active.index.clone()
        } else {
            active
                .index
                .replace_utf16_range(update.update_range(), update.text())?
        };
        let selection = post_index.byte_range_from_utf16(update.selection())?;
        let was_composing = active.composing;
        let context = self.next_context()?;
        let selection_op = TextInputOperation::SetSelection(PlatformTextSelection::new(
            selection,
            TextSelectionAffinity::Downstream,
        ));
        if update.is_selection_only() {
            return Ok((
                PlatformTextInputEvent::SetSelection {
                    context,
                    selection: PlatformTextSelection::new(
                        selection,
                        TextSelectionAffinity::Downstream,
                    ),
                },
                post_index,
                was_composing,
            ));
        }
        let composing = was_composing || update.composing;
        let operations = if composing {
            let preedit_selection = preedit_selection_range(update)?;
            vec![
                TextInputOperation::SetComposition(
                    TextCompositionUpdate::new(update.text().to_owned(), preedit_selection)
                        .with_replacement(replacement),
                ),
                selection_op,
            ]
        } else {
            vec![
                TextInputOperation::Commit(
                    TextCommit::new(update.text().to_owned()).with_replacement(replacement),
                ),
                selection_op,
            ]
        };
        Ok((
            PlatformTextInputEvent::Batch {
                context,
                operations,
            },
            post_index,
            composing,
        ))
    }

    fn next_context(&mut self) -> Result<PlatformTextInputContext, WebEditContextError> {
        let active = self
            .active
            .as_ref()
            .ok_or(TextInputDispatchError::NoActiveSession)?;
        self.next_serial = self.next_serial.saturating_add(1);
        Ok(PlatformTextInputContext::new(
            TextInputAdapterKind::WebEditContext,
            active.session,
            active.generation,
            active.target.clone(),
            TextInputSerial(self.next_serial),
        ))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn create_edit_context_object() -> Result<JsValue, WebEditContextError> {
        let window = web_sys::window().ok_or(WebEditContextError::WebEditContextUnavailable)?;
        let constructor =
            js_sys::Reflect::get(&window, &JsValue::from_str("EditContext")).map_err(js_error)?;
        if !constructor.is_function() {
            return Err(WebEditContextError::WebEditContextUnavailable);
        }
        js_sys::Reflect::construct(&constructor.unchecked_into(), &js_sys::Array::new())
            .map_err(js_error)
    }
}

impl WebEditContextActiveSession {
    pub const fn session(&self) -> TextInputSessionId {
        self.session
    }

    pub const fn capabilities(&self) -> TextInputCapabilities {
        self.capabilities
    }

    pub const fn security(&self) -> TextInputSecurityPolicy {
        self.security
    }

    pub fn text_index(&self) -> &TextIndexSnapshot {
        &self.index
    }

    pub const fn character_bounds_policy(&self) -> TextInputCapabilitySupport {
        self.capabilities.character_bounds
    }
}

fn preedit_selection_range(
    update: &WebEditContextTextUpdate,
) -> Result<TextRange<TextByteOffset>, WebEditContextError> {
    let update_start = update.update_range().start().0;
    let relative_start = update
        .selection()
        .start()
        .0
        .checked_sub(update_start)
        .ok_or(WebEditContextError::SelectionOutsideUpdate {
            update_range: update.update_range(),
            selection: update.selection(),
        })?;
    let relative_end = update.selection().end().0.checked_sub(update_start).ok_or(
        WebEditContextError::SelectionOutsideUpdate {
            update_range: update.update_range(),
            selection: update.selection(),
        },
    )?;
    let preedit_index = TextIndexSnapshot::new(update.text().to_owned());
    if relative_start > preedit_index.len_utf16().0 || relative_end > preedit_index.len_utf16().0 {
        return Err(WebEditContextError::SelectionOutsideUpdate {
            update_range: update.update_range(),
            selection: update.selection(),
        });
    }
    preedit_index
        .byte_range_from_utf16(TextRange::new(
            TextUtf16Offset(relative_start),
            TextUtf16Offset(relative_end),
        ))
        .map_err(Into::into)
}

#[cfg(target_arch = "wasm32")]
fn rect_init(rect: HitRect) -> JsValue {
    let object = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &object,
        &JsValue::from_str("x"),
        &JsValue::from_f64(f64::from(rect.x)),
    );
    let _ = js_sys::Reflect::set(
        &object,
        &JsValue::from_str("y"),
        &JsValue::from_f64(f64::from(rect.y)),
    );
    let _ = js_sys::Reflect::set(
        &object,
        &JsValue::from_str("width"),
        &JsValue::from_f64(f64::from(rect.width)),
    );
    let _ = js_sys::Reflect::set(
        &object,
        &JsValue::from_str("height"),
        &JsValue::from_f64(f64::from(rect.height)),
    );
    object.into()
}

#[cfg(target_arch = "wasm32")]
fn js_error(error: JsValue) -> WebEditContextError {
    WebEditContextError::JavaScript(
        error
            .as_string()
            .unwrap_or_else(|| "non-string JavaScript error".to_owned()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_id::PublicId;
    use arcweft_presentation::hit::HitRect;
    use arcweft_presentation::input::{InteractionTarget, RawInputKind};
    use arcweft_presentation::text_input::{TextInputOptions, TextInputPrivacy};

    fn target() -> InteractionTarget {
        InteractionTarget::new(PublicId::try_new("target.web.editcontext").unwrap())
    }

    fn snapshot(text: &str, secure: bool) -> TextInputClientSnapshot {
        let end = TextByteOffset(u32::try_from(text.len()).unwrap());
        TextInputClientSnapshot::new(
            TextInputSessionId(12),
            target(),
            TextRevision(7),
            text,
            TextByteOffset(0),
            TextRange::new(end, end),
            HitRect::new(0.0, 0.0, 320.0, 32.0),
            HitRect::new(0.0, 0.0, 1.0, 32.0),
            TextInputOptions::default().secure(secure),
        )
    }

    #[test]
    fn unsupported_activation_reports_typed_error_without_dispatch_state() {
        let mut adapter = WebEditContextAdapter::default();
        let error = adapter
            .activate(
                &snapshot("", false),
                WebEditContextFeatureDetection::new(false, false),
            )
            .expect_err("EditContext absence rejects activation");

        assert_eq!(error, WebEditContextError::WebEditContextUnavailable);
        assert!(adapter.active().is_none());
    }

    #[test]
    fn secure_activation_narrows_character_bounds_to_redacted() {
        let mut adapter = WebEditContextAdapter::default();
        let activation = adapter
            .activate(
                &snapshot("secret", true),
                WebEditContextFeatureDetection::new(true, true),
            )
            .expect("EditContext present activates");
        let TextInputHostCommand::Activate {
            snapshot,
            capabilities,
            ..
        } = &activation.commands()[0]
        else {
            panic!("activation command expected");
        };

        assert!(snapshot.surrounding_text().is_empty());
        assert_eq!(
            capabilities.character_bounds,
            TextInputCapabilitySupport::SecureRedacted
        );
        assert_eq!(
            adapter.active().unwrap().character_bounds_policy(),
            TextInputCapabilitySupport::SecureRedacted
        );
    }

    #[test]
    fn surrogate_replacement_uses_canonical_byte_range() {
        let mut adapter = WebEditContextAdapter::default();
        adapter
            .activate(
                &snapshot("a😀b", false),
                WebEditContextFeatureDetection::new(true, true),
            )
            .unwrap();
        let update = WebEditContextTextUpdate::new(
            TextRange::new(TextUtf16Offset(1), TextUtf16Offset(3)),
            "👩‍💻",
            TextRange::new(TextUtf16Offset(6), TextUtf16Offset(6)),
        )
        .with_observed_text_before("a😀b");

        let (event, post_index, _) = adapter.platform_event_for_text_update(&update).unwrap();

        let PlatformTextInputEvent::Batch { operations, .. } = event else {
            panic!("replacement plus selection should be grouped");
        };
        let TextInputOperation::Commit(commit) = &operations[0] else {
            panic!("plain textupdate commits replacement");
        };
        assert_eq!(
            commit.replacement(),
            Some(TextRange::new(TextByteOffset(1), TextByteOffset(5)))
        );
        assert_eq!(post_index.as_str(), "a👩‍💻b");
    }

    #[test]
    fn invalid_utf16_range_rejects_before_serial_advances() {
        let mut adapter = WebEditContextAdapter::default();
        adapter
            .activate(
                &snapshot("a😀b", false),
                WebEditContextFeatureDetection::new(true, true),
            )
            .unwrap();
        let update = WebEditContextTextUpdate::new(
            TextRange::new(TextUtf16Offset(2), TextUtf16Offset(3)),
            "x",
            TextRange::new(TextUtf16Offset(2), TextUtf16Offset(2)),
        );

        let error = adapter
            .platform_event_for_text_update(&update)
            .expect_err("mid-surrogate range rejects");

        assert!(matches!(error, WebEditContextError::TextIndex(_)));
        assert_eq!(adapter.next_serial_value(), 0);
    }

    #[test]
    fn composition_preedit_to_commit_flow_marks_ime_consumed() {
        let mut adapter = WebEditContextAdapter::default();
        adapter
            .activate(
                &snapshot("", false),
                WebEditContextFeatureDetection::new(true, true),
            )
            .unwrap();
        let start = adapter
            .dispatch_composition_start(InputEpoch(1))
            .expect("composition starts");
        assert_eq!(
            start.key_disposition(),
            TextInputKeyDisposition::ImeConsumed
        );
        let update = WebEditContextTextUpdate::new(
            TextRange::new(TextUtf16Offset(0), TextUtf16Offset(0)),
            "にほんご",
            TextRange::new(TextUtf16Offset(4), TextUtf16Offset(4)),
        )
        .composing(true);
        let output = adapter
            .dispatch_text_update(InputEpoch(2), &update)
            .expect("preedit routes");
        let RawInputKind::Text(input) = output.raw().kind() else {
            panic!("text input expected");
        };
        assert_eq!(input.privacy(), TextInputPrivacy::Plain);
        assert!(matches!(
            input.operations(),
            [
                TextInputOperation::SetComposition(_),
                TextInputOperation::SetSelection(_)
            ]
        ));

        let end = adapter
            .dispatch_composition_end(InputEpoch(3), CompositionEndReason::Committed)
            .expect("composition ends");
        let RawInputKind::Text(input) = end.raw().kind() else {
            panic!("text input expected");
        };
        assert!(matches!(
            input.operations(),
            [TextInputOperation::EndComposition { .. }]
        ));
    }
}
