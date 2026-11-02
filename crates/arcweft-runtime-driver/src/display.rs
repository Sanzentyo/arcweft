use crate::dialogue::{
    BundlePresentationTransition, DialogueAdvanceTarget, DialoguePresentationOperation,
    DialoguePresentationStore, DialoguePresentationStoreError, DialogueViewDefinition,
};
use crate::fx_runtime::{BundleFxRuntimeError, BundleFxRuntimeSnapshot};
use crate::presentation_handles::{
    PresentationHandleDiagnostic, PresentationHandleRecord, filter_presentation_action_buttons,
    filter_presentation_focus_groups, filter_presentation_focus_navigation,
    filter_presentation_scroll_regions, filter_presentation_surfaces,
    filter_presentation_text_inputs, hidden_focus_diagnostics,
};
use crate::presentation_projection::PresentationProjection;
pub use crate::presentation_projection::PresentationProjectionError;
use crate::view_projection::ProjectedViewResources;
use crate::view_runtime::BundleViewFrame;
use arcweft_bundle::resource_codec::{
    ViewRuntimeActionButton, ViewRuntimeFocusGroup, ViewRuntimeFocusNavigation,
    ViewRuntimeScrollRegion, ViewRuntimeSurface, ViewRuntimeTextControl,
};
use arcweft_character::presentation_name::{
    AcceptedCharacterPresentationCatalog, CharacterNameResolutionLocales,
};
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::plan::{
    FlowEvent, RuntimeDialogueContentApplicationKey, RuntimeDialogueContentSlot,
    RuntimeDialogueValueBinding,
};
use arcweft_core::presentation::RuntimePresentationFrame;
use arcweft_core::value::{
    RuntimeCharacterDialogueProducerId, RuntimeDialogueContentValue, RuntimeOpaqueValue,
    RuntimeValue,
};
use arcweft_dialogue::character_presentation::CharacterPresentationTargetEvidence;
use arcweft_dialogue::{
    CharacterDialogueRolePayloadCodec, CharacterDialogueRuntimeSchema, CharacterDialogueType,
};
use arcweft_id::{DeclarationIdentityFamily, LocaleTag};
use arcweft_interaction_model::presentation::PresentationImage;
use arcweft_layout::ScalePolicy;
use arcweft_manifest_model::ProjectLocaleSpec;
use arcweft_presentation::fx::FxDiagnostic;
use arcweft_render_text::{RuntimeLineContext, resolve_materialized_frame};
use arcweft_text_model::{
    CharacterDialoguePresentationConfig, DialogueContentCatalog, DialogueContentFragmentTemplate,
    DialogueContentSpec, DialoguePresentationCharacter,
    project_character_dialogue_rich_text_properties,
};
use arcweft_view::{ViewStyleProgram, ViewStyleSheetId};
use core::fmt;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Choice metadata shared by native and Web presentation hosts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleChoice {
    pub id: Option<String>,
    pub label: String,
}

/// Runtime-selected player viewport fit.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleViewportFit {
    pub design_width: u32,
    pub design_height: u32,
    pub scale_policy: ScalePolicy,
}

/// Display frames and non-fatal display diagnostics resolved from one VM step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayResolution {
    pub dialogue_operations: Vec<DialoguePresentationOperation>,
    pub diagnostics: Vec<String>,
}

/// Checked dynamic presentation context for one static dialogue-content row.
///
/// The static bundle catalog deliberately cannot manufacture character or
/// `CharacterDialogue` values. Runtime integrations provide this boundary
/// only after validating the exact value selected for the emitted line.
pub trait DialogueRuntimeContextProvider {
    fn context_for(
        &self,
        content: &DialogueContentSpec,
        target: &RuntimeOpaqueValue,
        values: &[RuntimeDialogueValueBinding],
    ) -> Result<RuntimeLineContext, DialogueRuntimeContextError>;
}

/// Runtime view over one accepted Character catalog generation and locale.
pub(crate) struct CatalogDialogueRuntimeContextProvider<'a> {
    catalog: &'a AcceptedCharacterPresentationCatalog,
    active_locale: &'a LocaleTag,
    project_locale: &'a ProjectLocaleSpec,
    schema: Option<&'a CharacterDialogueRuntimeSchema>,
    style_program: Option<&'a ViewStyleProgram>,
}

impl<'a> CatalogDialogueRuntimeContextProvider<'a> {
    #[must_use]
    pub(crate) const fn new(
        catalog: &'a AcceptedCharacterPresentationCatalog,
        active_locale: &'a LocaleTag,
        project_locale: &'a ProjectLocaleSpec,
        schema: Option<&'a CharacterDialogueRuntimeSchema>,
        style_program: Option<&'a ViewStyleProgram>,
    ) -> Self {
        Self {
            catalog,
            active_locale,
            project_locale,
            schema,
            style_program,
        }
    }
}

impl DialogueRuntimeContextProvider for CatalogDialogueRuntimeContextProvider<'_> {
    fn context_for(
        &self,
        content: &DialogueContentSpec,
        target: &RuntimeOpaqueValue,
        values: &[RuntimeDialogueValueBinding],
    ) -> Result<RuntimeLineContext, DialogueRuntimeContextError> {
        let generation = self.catalog.generation();
        if content.character().semantic_digest() != generation.semantic_digest() {
            return Err(DialogueRuntimeContextError::Rejected {
                line: content.line().clone(),
                reason: "Character presentation plan is stale for the active catalog generation"
                    .to_owned(),
            });
        }
        let reject = |reason: String| DialogueRuntimeContextError::Rejected {
            line: content.line().clone(),
            reason,
        };
        let admitted = self.schema.map(|schema| {
            schema
                .admit(
                    schema.program_owner(),
                    &RuntimeValue::Opaque(target.clone()),
                    target.semantic_identity(),
                )
                .map_err(|error| reject(error.to_string()))
        });
        let admitted = admitted.transpose()?;
        let character = match content.character().target() {
            CharacterPresentationTargetEvidence::Exact(character) => {
                if let Some(value) = admitted.as_ref() {
                    if value.dialogue().character() != character {
                        return Err(reject(
                            "dialogue target does not match the checked Character".to_owned(),
                        ));
                    }
                } else if target.producer() != &RuntimeCharacterDialogueProducerId::get()
                    || target.semantic_identity()
                        != CharacterDialogueType::exact(character.clone())
                            .runtime_semantic_identity()
                {
                    return Err(reject(
                        "dialogue target does not match the checked Character".to_owned(),
                    ));
                }
                character
            }
            CharacterPresentationTargetEvidence::RuntimeCharacterDialogue { generation } => {
                let Some(schema) = self.schema else {
                    return Err(reject(
                        "runtime CharacterDialogue schema is unavailable".to_owned(),
                    ));
                };
                if schema.generation_digest() != *generation {
                    return Err(reject(
                        "runtime CharacterDialogue generation does not match the checked target"
                            .to_owned(),
                    ));
                }
                admitted
                    .as_ref()
                    .expect("schema admission completed above")
                    .dialogue()
                    .character()
            }
        };
        let resolved = self
            .catalog
            .data()
            .resolve(
                character,
                CharacterNameResolutionLocales::new(
                    self.active_locale,
                    self.project_locale.source(),
                    self.project_locale.fallback(),
                ),
            )
            .map_err(|error| DialogueRuntimeContextError::Rejected {
                line: content.line().clone(),
                reason: error.to_string(),
            })?;
        let presentation = content.presentation();
        let (effective, base_styles) = if let (Some(schema), Some(value)) =
            (self.schema, admitted.as_ref())
        {
            let config = value.dialogue().config();
            let style_sheet = match config.style().typed().value() {
                RuntimeValue::EntityRef(reference)
                    if reference.project_family() == Some(DeclarationIdentityFamily::Style) =>
                {
                    let label = reference
                        .project_label(DeclarationIdentityFamily::Style)
                        .ok_or_else(|| {
                            reject("Style reference has another entity family".to_owned())
                        })?;
                    let sheet = ViewStyleSheetId::parse_public(label.to_owned())
                        .map_err(|error| reject(error.to_string()))?;
                    if !self
                        .style_program
                        .is_some_and(|program| program.sheet(&sheet).is_some())
                    {
                        return Err(reject(format!(
                            "selected Style sheet `{sheet:?}` is absent from the accepted View product"
                        )));
                    }
                    Some(sheet)
                }
                RuntimeValue::Opaque(style) => {
                    let properties = CharacterDialogueRolePayloadCodec::RichTextProperties
                        .decode_properties(style.payload())
                        .map_err(|error| reject(error.to_string()))?;
                    if !properties.is_empty() {
                        return Err(reject(
                            "structured CharacterDialogue Style cannot select a View sheet"
                                .to_owned(),
                        ));
                    }
                    None
                }
                _ => {
                    return Err(reject(
                        "admitted Style role has no supported presentation value".to_owned(),
                    ));
                }
            };
            let RuntimeValue::Opaque(rich_text) = config.rich_text().typed().value() else {
                return Err(reject(
                    "admitted RichText role has no opaque payload".to_owned(),
                ));
            };
            let properties = CharacterDialogueRolePayloadCodec::RichTextProperties
                .decode_properties(rich_text.payload())
                .map_err(|error| reject(error.to_string()))?;
            let styles = project_character_dialogue_rich_text_properties(&properties)
                .map_err(|error| reject(error.to_string()))?;
            (
                CharacterDialoguePresentationConfig::from_admitted(schema, value, style_sheet)
                    .map_err(|error| reject(error.to_string()))?,
                styles,
            )
        } else {
            (
                CharacterDialoguePresentationConfig::standalone(
                    presentation.view().clone(),
                    presentation.inline_failure().clone(),
                ),
                Vec::new(),
            )
        };
        Ok(RuntimeLineContext::new(
            values.to_vec(),
            DialoguePresentationCharacter {
                id: character.clone(),
                display_name: resolved.value().to_owned(),
            },
            effective,
            base_styles,
            content.inline_styles().to_vec(),
        ))
    }
}

/// Failure to obtain one checked dynamic dialogue presentation context.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DialogueRuntimeContextError {
    #[error("checked runtime CharacterDialogue context is unavailable for line {line:?}")]
    Unavailable {
        line: arcweft_core::plan::RuntimeLineId,
    },
    #[error("checked runtime CharacterDialogue context was rejected for line {line:?}: {reason}")]
    Rejected {
        line: arcweft_core::plan::RuntimeLineId,
        reason: String,
    },
}

/// Failure to atomically apply one presentation update.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BundlePresentationUpdateError {
    #[error(transparent)]
    Dialogue(#[from] DialoguePresentationStoreError),
    #[error(transparent)]
    Projection(#[from] PresentationProjectionError),
}

/// Current portable presentation state consumed by renderer adapters.
///
/// This value is a diagnostic/render input model, not a DOM instruction set.
#[derive(Clone, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(bound(
    serialize = "F: Serialize + PartialEq",
    deserialize = "F: Deserialize<'de>"
))]
pub struct BundlePresentationSnapshot<F = arcweft_text_model::LineDisplayFrame> {
    pub revision: u64,
    pub dialogue: DialoguePresentationStore<F>,
    pub choices: Vec<BundleChoice>,
    pub images: Vec<PresentationImage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presentation_handles: Vec<PresentationHandleRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport_fit: Option<BundleViewportFit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text_inputs: Vec<ViewRuntimeTextControl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_buttons: Vec<ViewRuntimeActionButton>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scroll_regions: Vec<ViewRuntimeScrollRegion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surfaces: Vec<ViewRuntimeSurface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_groups: Vec<ViewRuntimeFocusGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_navigation: Vec<ViewRuntimeFocusNavigation>,
    /// Mount-scoped executable View output for this presentation revision.
    #[serde(default, skip_serializing_if = "is_default")]
    pub view: BundleViewFrame<F>,
    /// Activation-relative logical clock and complete live Fx save state.
    pub fx: BundleFxRuntimeSnapshot,
    /// Typed failures consumed unchanged by native, Web, headless, and Agent observers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fx_diagnostics: Vec<FxDiagnostic>,
}

impl<F> Default for BundlePresentationSnapshot<F> {
    fn default() -> Self {
        Self {
            revision: 0,
            dialogue: DialoguePresentationStore::default(),
            choices: Vec::new(),
            images: Vec::new(),
            presentation_handles: Vec::new(),
            viewport_fit: None,
            text_inputs: Vec::new(),
            action_buttons: Vec::new(),
            scroll_regions: Vec::new(),
            surfaces: Vec::new(),
            focus_groups: Vec::new(),
            focus_navigation: Vec::new(),
            view: BundleViewFrame::default(),
            fx: BundleFxRuntimeSnapshot::default(),
            fx_diagnostics: Vec::new(),
        }
    }
}

impl<F> BundlePresentationSnapshot<F> {
    /// Visits every retained frame once through the owning presentation grammar.
    pub(crate) fn try_visit_frames<E>(
        &self,
        mut visit: impl FnMut(&F) -> Result<(), E>,
    ) -> Result<(), E> {
        for dialogue in self.dialogue.iter() {
            for entry in dialogue.entries() {
                visit(entry.frame())?;
            }
        }
        for mount in &self.view.mounts {
            for text in &mount.text {
                match &text.value {
                    crate::view_runtime::BundleViewTextValue::CharacterDisplayName { frame }
                    | crate::view_runtime::BundleViewTextValue::DisplayFrame { frame, .. } => {
                        visit(frame)?;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    pub(crate) fn try_map_frames<T, E>(
        self,
        mut map: impl FnMut(F) -> Result<T, E>,
    ) -> Result<BundlePresentationSnapshot<T>, E> {
        Ok(BundlePresentationSnapshot {
            revision: self.revision,
            dialogue: self.dialogue.try_map_frames(&mut map)?,
            choices: self.choices,
            images: self.images,
            presentation_handles: self.presentation_handles,
            viewport_fit: self.viewport_fit,
            text_inputs: self.text_inputs,
            action_buttons: self.action_buttons,
            scroll_regions: self.scroll_regions,
            surfaces: self.surfaces,
            focus_groups: self.focus_groups,
            focus_navigation: self.focus_navigation,
            view: self.view.try_map_frames(&mut map)?,
            fx: self.fx,
            fx_diagnostics: self.fx_diagnostics,
        })
    }
}

/// Runtime resources that affect portable presentation state.
#[derive(Clone, Copy)]
pub(crate) struct BundlePresentationResources<'a> {
    pub(crate) text_inputs: &'a [ViewRuntimeTextControl],
    pub(crate) action_buttons: &'a [ViewRuntimeActionButton],
    pub(crate) scroll_regions: &'a [ViewRuntimeScrollRegion],
    pub(crate) surfaces: &'a [ViewRuntimeSurface],
    pub(crate) focus_groups: &'a [ViewRuntimeFocusGroup],
    pub(crate) focus_navigation: &'a [ViewRuntimeFocusNavigation],
}

/// Resolves dialogue flow events into host-renderable, Sans I/O display frames.
pub fn resolve_display_frames(
    catalog: &DialogueContentCatalog,
    events: &[FlowEvent],
    context_provider: Option<&dyn DialogueRuntimeContextProvider>,
) -> DisplayResolution {
    events
        .iter()
        .fold(DisplayResolution::default(), |mut resolution, event| {
            if let FlowEvent::DialogueLine {
                activation,
                line,
                template,
                target,
                values,
            } = event
                && let Some(spec) = catalog.find(&RuntimeDialogueContentApplicationKey::new(
                    line.clone(),
                    *template,
                ))
            {
                let Some(provider) = context_provider else {
                    resolution.diagnostics.push(
                        DialogueRuntimeContextError::Unavailable { line: line.clone() }.to_string(),
                    );
                    return resolution;
                };
                let Some(template) = catalog.find_template(spec.template_id()) else {
                    resolution.diagnostics.push(format!(
                        "dialogue line {} references missing content template {}",
                        line.public_label(),
                        spec.template_id()
                    ));
                    return resolution;
                };
                let content_value = match build_dialogue_content_value(
                    spec,
                    template,
                    activation.artifact(),
                    values,
                ) {
                    Ok(value) => value,
                    Err(error) => {
                        resolution.diagnostics.push(error);
                        return resolution;
                    }
                };
                let context = match provider.context_for(spec, target, values) {
                    Ok(context) => context,
                    Err(error) => {
                        resolution.diagnostics.push(error.to_string());
                        return resolution;
                    }
                };
                let fragment_catalog = match catalog.fragment_catalog(activation.artifact()) {
                    Ok(catalog) => catalog,
                    Err(error) => {
                        resolution.diagnostics.push(error.to_string());
                        return resolution;
                    }
                };
                let materialized =
                    match arcweft_text_model::DialogueContentMaterializer::new(&fragment_catalog)
                        .materialize_with_policy(&content_value, context.inline_failure_policy())
                    {
                        Ok(materialized) => materialized,
                        Err(error) => {
                            resolution.diagnostics.push(error.to_string());
                            return resolution;
                        }
                    };
                let context = context.with_materialized_bindings(materialized.bindings());
                match resolve_materialized_frame(spec, &materialized, &content_value, &context) {
                    Ok(frame) => {
                        let view = frame.effective.view().clone();
                        resolution
                            .dialogue_operations
                            .push(DialoguePresentationOperation::append(
                                activation.clone(),
                                DialogueViewDefinition::new(view),
                                frame,
                            ));
                    }
                    Err(error) => resolution.diagnostics.push(error.to_string()),
                }
            }
            resolution
        })
}

fn build_dialogue_content_value(
    spec: &DialogueContentSpec,
    template: &DialogueContentFragmentTemplate,
    artifact: arcweft_core::effect::RuntimeArtifactFingerprint,
    values: &[RuntimeDialogueValueBinding],
) -> Result<RuntimeDialogueContentValue, String> {
    if spec.template_id() != template.id() || spec.template_digest() != template.digest() {
        return Err(format!(
            "dialogue line {} has a stale content template identity",
            spec.line().public_label()
        ));
    }
    let slots = template
        .slots()
        .iter()
        .map(|slot| RuntimeDialogueContentSlot::new(slot.slot(), slot.role(), slot.semantic_type()))
        .collect::<Vec<_>>();
    RuntimeDialogueContentValue::try_from_evaluated_bindings_parts(
        artifact,
        spec.template_id(),
        spec.template_digest(),
        &slots,
        values,
    )
    .map_err(|error| error.to_string())
}

impl BundlePresentationSnapshot {
    /// Uses the same frame admission as the inert wire producer before copying
    /// presentation state or accepting a direct live snapshot candidate.
    pub(crate) fn validate_runtime_context(
        &self,
        owner: &arcweft_core::task::RuntimeProgramOwner,
        schema: Option<&arcweft_dialogue::CharacterDialogueRuntimeSchema>,
    ) -> Result<(), String> {
        let mut admission = arcweft_core::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
        );
        self.admit_retained(&mut admission, schema)
    }

    pub(crate) fn admit_retained(
        &self,
        admission: &mut arcweft_core::value::RuntimeRetainedValueAdmission,
        schema: Option<&arcweft_dialogue::CharacterDialogueRuntimeSchema>,
    ) -> Result<(), String> {
        self.try_visit_frames(|frame| frame.admit_retained_metadata(admission))?;
        self.try_visit_frames(|frame| {
            if schema.is_some() && frame.effective.admitted().is_none() {
                return Err(
                    "session display frame has no generation-admitted CharacterDialogue".to_owned(),
                );
            }
            frame.admit_retained_values(admission, schema)
        })
    }

    pub(crate) fn advance_fx_clock(&mut self, milliseconds: u64) {
        match self.fx.advance_millis(milliseconds) {
            Ok(()) => {
                self.fx_diagnostics.clear();
                self.revision = self.revision.saturating_add(1);
            }
            Err(error) => self.record_fx_error(&error),
        }
    }

    pub(crate) fn record_fx_error(&mut self, error: &BundleFxRuntimeError) {
        let diagnostic = error.diagnostic();
        if self.fx_diagnostics.as_slice() != [diagnostic.clone()] {
            self.fx_diagnostics = vec![diagnostic];
            self.revision = self.revision.saturating_add(1);
        }
    }

    pub(crate) fn update(
        &mut self,
        resolution: &DisplayResolution,
        status: &FlowFiberStatus,
        frame: RuntimePresentationFrame<'_>,
        resources: BundlePresentationResources<'_>,
    ) -> Result<Vec<PresentationHandleDiagnostic>, BundlePresentationUpdateError> {
        let mut next_dialogue = self.dialogue.clone();
        next_dialogue.apply_operations(&resolution.dialogue_operations)?;
        next_dialogue.synchronize_waiting_activation(waiting_dialogue_activation(status))?;
        let next_choices = choices_from_status(status);
        let projection = PresentationProjection::try_new(
            frame,
            arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
        )?;
        let next_presentation_handles = projection.handles;
        let next_images = projection.images;
        let next_viewport_fit = projection.viewport_fit;
        let mut handle_diagnostics = Vec::new();
        let next_text_inputs = filter_presentation_text_inputs(
            resources.text_inputs.to_vec(),
            &next_presentation_handles,
        );
        let next_action_buttons = filter_presentation_action_buttons(
            resources.action_buttons.to_vec(),
            &next_presentation_handles,
        );
        let next_scroll_regions = filter_presentation_scroll_regions(
            resources.scroll_regions.to_vec(),
            &next_presentation_handles,
        );
        let next_surfaces =
            filter_presentation_surfaces(resources.surfaces.to_vec(), &next_presentation_handles);
        let next_focus_groups = filter_presentation_focus_groups(
            resources.focus_groups.to_vec(),
            &next_presentation_handles,
        );
        let raw_focus_navigation = resources.focus_navigation.to_vec();
        handle_diagnostics.extend(hidden_focus_diagnostics(
            &next_presentation_handles,
            &raw_focus_navigation,
        ));
        let next_focus_navigation =
            filter_presentation_focus_navigation(raw_focus_navigation, &next_presentation_handles);
        if self.dialogue != next_dialogue
            || self.choices != next_choices
            || self.images != next_images
            || self.presentation_handles != next_presentation_handles
            || self.viewport_fit != next_viewport_fit
            || self.text_inputs != next_text_inputs
            || self.action_buttons != next_action_buttons
            || self.scroll_regions != next_scroll_regions
            || self.surfaces != next_surfaces
            || self.focus_groups != next_focus_groups
            || self.focus_navigation != next_focus_navigation
        {
            self.revision = self.revision.saturating_add(1);
            self.dialogue = next_dialogue;
            self.choices = next_choices;
            self.images = next_images;
            self.presentation_handles = next_presentation_handles;
            self.viewport_fit = next_viewport_fit;
            self.text_inputs = next_text_inputs;
            self.action_buttons = next_action_buttons;
            self.scroll_regions = next_scroll_regions;
            self.surfaces = next_surfaces;
            self.focus_groups = next_focus_groups;
            self.focus_navigation = next_focus_navigation;
        }
        Ok(handle_diagnostics)
    }

    pub(crate) fn replace_view_frame(&mut self, view: BundleViewFrame) {
        if self.view != view {
            self.revision = self.revision.saturating_add(1);
            self.view = view;
        }
    }

    pub(crate) fn replace_view_resources(&mut self, resources: ProjectedViewResources) {
        if self.images != resources.images
            || self.text_inputs != resources.text_inputs
            || self.action_buttons != resources.action_buttons
            || self.scroll_regions != resources.scroll_regions
            || self.surfaces != resources.surfaces
            || self.focus_groups != resources.focus_groups
            || self.focus_navigation != resources.focus_navigation
        {
            self.revision = self.revision.saturating_add(1);
            self.images = resources.images;
            self.text_inputs = resources.text_inputs;
            self.action_buttons = resources.action_buttons;
            self.scroll_regions = resources.scroll_regions;
            self.surfaces = resources.surfaces;
            self.focus_groups = resources.focus_groups;
            self.focus_navigation = resources.focus_navigation;
        }
    }

    pub(crate) fn advance_dialogue(
        &mut self,
        target: DialogueAdvanceTarget,
    ) -> (
        BundlePresentationTransition,
        Option<arcweft_core::runtime_id::DialogueActivationId>,
    ) {
        let before = self.dialogue.clone();
        let result = self.dialogue.advance_dialogue(target);
        if self.dialogue != before {
            self.revision = self.revision.saturating_add(1);
        }
        result
    }
}

impl<F: Clone> BundlePresentationSnapshot<F> {
    #[must_use]
    pub fn redacted_for_observation(&self) -> Self {
        Self {
            text_inputs: self
                .text_inputs
                .iter()
                .map(ViewRuntimeTextControl::redacted_for_observation)
                .collect(),
            view: self.view.redacted_for_observation(),
            ..self.clone()
        }
    }
}

fn waiting_dialogue_activation(
    status: &FlowFiberStatus,
) -> Option<&arcweft_core::runtime_id::DialogueActivationId> {
    match status {
        FlowFiberStatus::Dialogue(status) => status.waiting_presentation_activation(),
        _ => None,
    }
}

impl<F: fmt::Debug + Clone> fmt::Debug for BundlePresentationSnapshot<F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BundlePresentationSnapshot")
            .field("revision", &self.revision)
            .field("dialogue", &self.dialogue)
            .field("choices", &self.choices)
            .field("images", &self.images)
            .field("presentation_handles", &self.presentation_handles)
            .field("viewport_fit", &self.viewport_fit)
            .field("text_inputs", &self.redacted_for_observation().text_inputs)
            .field("action_buttons", &self.action_buttons)
            .field("scroll_regions", &self.scroll_regions)
            .field("surfaces", &self.surfaces)
            .field("focus_groups", &self.focus_groups)
            .field("focus_navigation", &self.focus_navigation)
            .field("view", &self.view.redacted_for_observation())
            .field("fx", &self.fx)
            .field("fx_diagnostics", &self.fx_diagnostics)
            .finish()
    }
}

impl BundleViewportFit {
    pub const fn raw() -> Self {
        Self {
            design_width: 0,
            design_height: 0,
            scale_policy: ScalePolicy::Raw,
        }
    }

    pub const fn design(design_width: u32, design_height: u32, scale_policy: ScalePolicy) -> Self {
        Self {
            design_width,
            design_height,
            scale_policy,
        }
    }
}

fn choices_from_status(status: &FlowFiberStatus) -> Vec<BundleChoice> {
    let FlowFiberStatus::Choice(state) = status else {
        return Vec::new();
    };
    state
        .options
        .iter()
        .map(|option| BundleChoice {
            id: option.id.clone(),
            label: option.label.clone(),
        })
        .collect()
}

fn is_default<T>(value: &T) -> bool
where
    T: Default + PartialEq,
{
    value == &T::default()
}

#[cfg(test)]
mod tests;
