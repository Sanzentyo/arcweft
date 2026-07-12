use super::support::resolve_mount_path;
use super::{EvaluationFailure, ViewEvaluator};
use crate::view_runtime::value::{fx_scalar_text, runtime_scalar_text};
use crate::view_runtime::{
    BundleViewDiagnosticCode, BundleViewTextOutput, BundleViewTextValue, MountedView,
};
use arcweft_bundle::resource_codec::ViewDefinitionResource;
use arcweft_bundle::resource_codec::view::ViewTextSourceKind;

impl ViewEvaluator<'_> {
    pub(super) fn resolve_text(
        &self,
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
        let value = match &source.kind {
            ViewTextSourceKind::Literal { value } => BundleViewTextValue::Plain {
                value: value.clone(),
            },
            ViewTextSourceKind::Projection { path } => {
                let projected =
                    resolve_mount_path(&mounted.runtime_parameters, self.root_bindings, path)
                        .ok_or_else(|| {
                            EvaluationFailure::new(
                                BundleViewDiagnosticCode::MissingInput,
                                Some(instruction),
                                format!("text projection `{}` has no value", path.join(".")),
                            )
                        })?;
                let text = runtime_scalar_text(projected).ok_or_else(|| {
                    EvaluationFailure::new(
                        BundleViewDiagnosticCode::UnsupportedTextValue,
                        Some(instruction),
                        format!(
                            "text projection `{}` is not a deterministic scalar",
                            path.join(".")
                        ),
                    )
                })?;
                BundleViewTextValue::Plain { value: text }
            }
            ViewTextSourceKind::Local { name } => {
                let slots = self.local_slots(&definition.public_id, name);
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
                BundleViewTextValue::Plain {
                    value: fx_scalar_text(value),
                }
            }
            ViewTextSourceKind::Localized { key, locale } => BundleViewTextValue::Localized {
                key: key.clone(),
                locale: locale.clone(),
            },
            ViewTextSourceKind::RichTextDocument { document } => {
                BundleViewTextValue::RichTextDocument {
                    document: *document,
                }
            }
            ViewTextSourceKind::DisplayFrame { frame } => {
                BundleViewTextValue::DisplayFrame { frame: *frame }
            }
        };
        let targets = self
            .program
            .text_blocks
            .iter()
            .filter(|block| {
                block.text_source == source_id
                    && block.view.as_deref() == Some(definition.public_id.as_str())
            })
            .map(|block| block.public_id.clone())
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
}
