use super::support::resolve_mount_path;
use super::{EvaluationFailure, ViewEvaluator};
use crate::presentation_handles::PresentationHandleId;
use crate::view_runtime::value::{fx_scalar_text, runtime_scalar_text};
use crate::view_runtime::{
    BundleViewDiagnosticCode, BundleViewTextOutput, BundleViewTextTarget, BundleViewTextValue,
    MountedView,
};
use arcweft_bundle::resource_codec::ViewDefinitionResource;
use arcweft_bundle::resource_codec::ViewRuntimeControlVisualStyle;
use arcweft_bundle::resource_codec::view::{
    DialogueTextProjection, ViewParameterRole, ViewTextSourceKind,
};

impl ViewEvaluator<'_> {
    pub(super) fn resolve_text(
        &self,
        handle: &PresentationHandleId,
        definition: &ViewDefinitionResource,
        mounted: &MountedView,
        source_id: &str,
        instruction: usize,
    ) -> Result<BundleViewTextOutput, EvaluationFailure> {
        let source = self
            .text
            .and_then(|text| {
                text.sources
                    .iter()
                    .find(|source| source.public_id == source_id)
            })
            .ok_or_else(|| {
                EvaluationFailure::new(
                    BundleViewDiagnosticCode::MissingTextSource,
                    Some(instruction),
                    format!("View text source `{source_id}` does not exist"),
                )
            })?;
        let value =
            self.resolve_text_value(handle, definition, mounted, &source.kind, instruction)?;
        let targets = self
            .program
            .text_blocks
            .iter()
            .filter(|block| {
                block.text_source == source_id
                    && block.view.as_deref() == Some(definition.public_id.as_str())
            })
            .map(|block| BundleViewTextTarget {
                public_id: block.public_id.clone(),
                containing_scroll_region: block.containing_scroll_region.clone(),
                bounds: block.bounds,
                selection_policy: block.selection_policy,
                style: ViewRuntimeControlVisualStyle::default(),
            })
            .collect();
        let redaction = self.text.and_then(|text| {
            text.redactions
                .iter()
                .find(|redaction| redaction.text_source == source_id)
        });
        Ok(BundleViewTextOutput {
            source_id: source_id.to_owned(),
            targets,
            value,
            classification: redaction.map_or_else(Default::default, |value| value.classification),
            replacement: redaction.and_then(|value| value.replacement.clone()),
        })
    }

    fn resolve_text_value(
        &self,
        handle: &PresentationHandleId,
        definition: &ViewDefinitionResource,
        mounted: &MountedView,
        source: &ViewTextSourceKind,
        instruction: usize,
    ) -> Result<BundleViewTextValue, EvaluationFailure> {
        match source {
            ViewTextSourceKind::Literal { value } => Ok(BundleViewTextValue::Plain {
                value: value.clone(),
            }),
            ViewTextSourceKind::Projection { path } => {
                self.resolve_projected_text(mounted, path, instruction)
            }
            ViewTextSourceKind::Local { name } => {
                self.resolve_local_text(definition, mounted, name, instruction)
            }
            ViewTextSourceKind::Localized { key, locale } => {
                self.resolve_localized_text(key, locale.as_deref(), instruction)
            }
            ViewTextSourceKind::RichTextDocument { document } => {
                self.resolve_rich_text_document(document, instruction)
            }
            ViewTextSourceKind::DisplayFrame { frame } => {
                self.resolve_display_frame(frame, instruction)
            }
            ViewTextSourceKind::Dialogue {
                parameter,
                projection,
            } => {
                self.resolve_dialogue_text(handle, definition, parameter, *projection, instruction)
            }
        }
    }

    fn resolve_dialogue_text(
        &self,
        handle: &PresentationHandleId,
        definition: &ViewDefinitionResource,
        parameter: &str,
        projection: DialogueTextProjection,
        instruction: usize,
    ) -> Result<BundleViewTextValue, EvaluationFailure> {
        if !definition.parameters.iter().any(|candidate| {
            candidate.name == parameter && candidate.role == ViewParameterRole::Dialogue
        }) {
            return Err(EvaluationFailure::new(
                BundleViewDiagnosticCode::MissingInput,
                Some(instruction),
                format!(
                    "View `{}` has no dialogue parameter `{parameter}`",
                    definition.public_id
                ),
            ));
        }
        let input = self.dialogue_inputs.get(handle).ok_or_else(|| {
            EvaluationFailure::new(
                BundleViewDiagnosticCode::MissingDialogueInput,
                Some(instruction),
                format!(
                    "View `{}` requires typed dialogue input `{parameter}` for handle `{handle}`",
                    definition.public_id
                ),
            )
        })?;
        match projection {
            DialogueTextProjection::CharacterDisplayName => {
                Ok(BundleViewTextValue::CharacterDisplayName {
                    frame: Box::new(input.frame.clone()),
                })
            }
            DialogueTextProjection::Content => {
                let stage_index_value = input.state.stage.index.get();
                let stage_index = usize::try_from(stage_index_value).map_err(|_| {
                    EvaluationFailure::new(
                        BundleViewDiagnosticCode::InvalidDisplayStage,
                        Some(instruction),
                        format!("dialogue stage {stage_index_value} exceeds this platform"),
                    )
                })?;
                input.frame.validate().map_err(|error| {
                    EvaluationFailure::new(
                        BundleViewDiagnosticCode::InvalidDisplayStage,
                        Some(instruction),
                        format!("dialogue display frame is invalid: {error}"),
                    )
                })?;
                if input.frame.stage(stage_index).is_none() {
                    return Err(EvaluationFailure::new(
                        BundleViewDiagnosticCode::InvalidDisplayStage,
                        Some(instruction),
                        format!("dialogue display frame has no stage {stage_index_value}"),
                    ));
                }
                Ok(BundleViewTextValue::DisplayFrame {
                    frame: Box::new(input.frame.clone()),
                    stage_index: stage_index_value,
                })
            }
        }
    }

    fn resolve_projected_text(
        &self,
        mounted: &MountedView,
        path: &[String],
        instruction: usize,
    ) -> Result<BundleViewTextValue, EvaluationFailure> {
        let projected = resolve_mount_path(&mounted.runtime_parameters, self.root_bindings, path)
            .ok_or_else(|| {
            EvaluationFailure::new(
                BundleViewDiagnosticCode::MissingInput,
                Some(instruction),
                format!("text projection `{}` has no value", path.join(".")),
            )
        })?;
        let value = runtime_scalar_text(projected).ok_or_else(|| {
            EvaluationFailure::new(
                BundleViewDiagnosticCode::UnsupportedTextValue,
                Some(instruction),
                format!(
                    "text projection `{}` is not a deterministic scalar",
                    path.join(".")
                ),
            )
        })?;
        Ok(BundleViewTextValue::Plain { value })
    }

    fn resolve_local_text(
        &self,
        definition: &ViewDefinitionResource,
        mounted: &MountedView,
        name: &str,
        instruction: usize,
    ) -> Result<BundleViewTextValue, EvaluationFailure> {
        let slots = self.local_slots(definition.public_id.as_str(), name);
        let slot = slots.first().copied().ok_or_else(|| {
            EvaluationFailure::new(
                BundleViewDiagnosticCode::MissingInput,
                Some(instruction),
                format!("text local `{name}` has no typed slot"),
            )
        })?;
        if !mounted.initialized_state.contains(&slot) {
            return Err(EvaluationFailure::new(
                BundleViewDiagnosticCode::MissingInput,
                Some(instruction),
                format!("text local `{name}` is not initialized"),
            ));
        }
        let value = mounted
            .state
            .state()
            .nth(usize::from(slot))
            .expect("validated local slot exists");
        Ok(BundleViewTextValue::Plain {
            value: fx_scalar_text(value),
        })
    }

    fn resolve_localized_text(
        &self,
        key: &str,
        locale: Option<&str>,
        instruction: usize,
    ) -> Result<BundleViewTextValue, EvaluationFailure> {
        let document = self
            .text
            .and_then(|text| text.localized_document(key, locale))
            .ok_or_else(|| {
                EvaluationFailure::new(
                    BundleViewDiagnosticCode::MissingLocalizedText,
                    Some(instruction),
                    format!("localized text `{key}` for locale {locale:?} does not exist"),
                )
            })?;
        Ok(BundleViewTextValue::Localized {
            key: key.to_owned(),
            locale: locale.map(str::to_owned),
            document: Box::new(document.clone()),
        })
    }

    fn resolve_rich_text_document(
        &self,
        public_id: &str,
        instruction: usize,
    ) -> Result<BundleViewTextValue, EvaluationFailure> {
        let document = self
            .text
            .and_then(|text| text.rich_text_document(public_id))
            .ok_or_else(|| {
                EvaluationFailure::new(
                    BundleViewDiagnosticCode::MissingRichTextDocument,
                    Some(instruction),
                    format!("RichText document `{public_id}` does not exist"),
                )
            })?;
        Ok(BundleViewTextValue::RichTextDocument {
            document: Box::new(document.clone()),
        })
    }

    fn resolve_display_frame(
        &self,
        public_id: &str,
        instruction: usize,
    ) -> Result<BundleViewTextValue, EvaluationFailure> {
        let entry = self
            .text
            .and_then(|text| text.display_frame(public_id))
            .ok_or_else(|| {
                EvaluationFailure::new(
                    BundleViewDiagnosticCode::MissingDisplayFrame,
                    Some(instruction),
                    format!("display frame `{public_id}` does not exist"),
                )
            })?;
        let stage_index = usize::try_from(entry.stage_index).map_err(|_| {
            EvaluationFailure::new(
                BundleViewDiagnosticCode::InvalidDisplayStage,
                Some(instruction),
                format!(
                    "display frame `{public_id}` stage {} exceeds this platform",
                    entry.stage_index
                ),
            )
        })?;
        entry.frame.validate().map_err(|error| {
            EvaluationFailure::new(
                BundleViewDiagnosticCode::InvalidDisplayStage,
                Some(instruction),
                format!("display frame `{public_id}` is invalid: {error}"),
            )
        })?;
        if entry.frame.stage(stage_index).is_none() {
            return Err(EvaluationFailure::new(
                BundleViewDiagnosticCode::InvalidDisplayStage,
                Some(instruction),
                format!(
                    "display frame `{public_id}` has no stage {}",
                    entry.stage_index
                ),
            ));
        }
        Ok(BundleViewTextValue::DisplayFrame {
            frame: Box::new(entry.frame.clone()),
            stage_index: entry.stage_index,
        })
    }
}
