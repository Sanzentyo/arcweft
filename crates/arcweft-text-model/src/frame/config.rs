//! Retains the admitted configuration instead of its lossy render projection.

use arcweft_core::{
    entry::RuntimeValueDigest,
    task::RuntimeProgramOwner,
    value::{AwbcRuntimeValueSnapshot, RuntimeValue},
};
use arcweft_dialogue::{
    CharacterDialogueRuntimeSchema, CharacterDialogueValue, CharacterDialogueValueError,
    InlineFailurePolicy,
};
use arcweft_view::{ViewId, ViewStyleSheetId};
use serde::{Deserialize, Serialize, Serializer};

/// Effective configuration with its original, generation-admitted value.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialoguePresentationConfig {
    source: CharacterDialoguePresentationSource,
    style_sheet: Option<ViewStyleSheetId>,
}

#[derive(Clone, Debug)]
enum CharacterDialoguePresentationSource {
    Standalone {
        view: ViewId,
        inline_failure: InlineFailurePolicy,
    },
    Program {
        owner: RuntimeProgramOwner,
        generation: RuntimeValueDigest,
        config_digest: RuntimeValueDigest,
        value: Box<CharacterDialogueValue>,
    },
}

impl PartialEq for CharacterDialoguePresentationSource {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Standalone {
                    view,
                    inline_failure,
                },
                Self::Standalone {
                    view: other_view,
                    inline_failure: other_policy,
                },
            ) => view == other_view && inline_failure == other_policy,
            (
                Self::Program {
                    generation,
                    config_digest,
                    value,
                    ..
                },
                Self::Program {
                    generation: other_generation,
                    config_digest: other_digest,
                    value: other_value,
                    ..
                },
            ) => {
                generation == other_generation
                    && config_digest == other_digest
                    && value == other_value
            }
            _ => false,
        }
    }
}

/// Inert wire carrier. Its program value is admitted only with the original
/// executable and `CharacterDialogue` generation schema at restoration.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
pub enum CharacterDialoguePresentationConfigSnapshot {
    Standalone {
        view: ViewId,
        style_sheet: Option<ViewStyleSheetId>,
        inline_failure: InlineFailurePolicy<AwbcRuntimeValueSnapshot>,
    },
    Program {
        generation: RuntimeValueDigest,
        config_digest: RuntimeValueDigest,
        style_sheet: Option<ViewStyleSheetId>,
        value: AwbcRuntimeValueSnapshot,
    },
}

impl CharacterDialoguePresentationConfigSnapshot {
    pub fn from_live(
        config: &CharacterDialoguePresentationConfig,
        owner: Option<&RuntimeProgramOwner>,
        schema: Option<&CharacterDialogueRuntimeSchema>,
    ) -> Result<Self, String> {
        match &config.source {
            CharacterDialoguePresentationSource::Standalone { view, inline_failure } => Ok(Self::Standalone {
                view: view.clone(),
                style_sheet: config.style_sheet.clone(),
                inline_failure: inline_failure.borrow_styles().try_map_styles(&mut |style| {
                    match owner { Some(owner) => AwbcRuntimeValueSnapshot::from_unrestricted_runtime_value_for_program(style.typed().value(), owner), None => AwbcRuntimeValueSnapshot::from_unrestricted_standalone_value(style.typed().value()) }
                        .map_err(|error| error.to_string())
                })?,
            }),
            CharacterDialoguePresentationSource::Program { generation, config_digest, value, owner: retained_owner } => {
                let owner = owner.ok_or("CharacterDialogue executable is unavailable")?;
                if !retained_owner.same_program(owner) { return Err("CharacterDialogue executable is stale".to_owned()); }
                if let Some(schema) = schema {
                if !retained_owner.same_program(owner) || !schema.program_owner().same_program(owner) || schema.generation_digest() != *generation {
                    return Err("CharacterDialogue generation or executable is stale".to_owned());
                }
                let admitted = CharacterDialoguePresentationConfig::from_admitted(schema, value, config.style_sheet.clone())
                    .map_err(|error| error.to_string())?;
                if admitted.config_digest() != *config_digest {
                    return Err("CharacterDialogue configuration digest is stale".to_owned());
                }
                }
                Ok(Self::Program {
                    generation: *generation,
                    config_digest: *config_digest,
                    style_sheet: config.style_sheet.clone(),
                    value: AwbcRuntimeValueSnapshot::from_unrestricted_runtime_value_for_program(
                        &RuntimeValue::Opaque(value.opaque().clone()), owner,
                    ).map_err(|error| error.to_string())?,
                })
            }
        }
    }

    pub fn into_live(
        self,
        owner: Option<&RuntimeProgramOwner>,
        schema: Option<&CharacterDialogueRuntimeSchema>,
    ) -> Result<CharacterDialoguePresentationConfig, String> {
        match self {
            Self::Standalone {
                view,
                style_sheet,
                inline_failure,
            } => {
                let inline_failure = inline_failure.try_map_styles(&mut |value| {
                    let value = match owner {
                        Some(owner) => value.into_unrestricted_runtime_value_for_program(owner),
                        None => value.into_unrestricted_standalone_value(),
                    }
                    .map_err(|error| error.to_string())?;
                    let schema = schema.ok_or("fallback Style generation schema is unavailable")?;
                    schema
                        .admit_style(
                            owner.ok_or("fallback Style executable is unavailable")?,
                            &value,
                        )
                        .map_err(|error| error.to_string())
                })?;
                CharacterDialoguePresentationConfig::standalone(view, inline_failure)
                    .with_standalone_style_sheet(style_sheet)
                    .map_err(str::to_owned)
            }
            Self::Program {
                generation,
                config_digest,
                style_sheet,
                value,
            } => {
                let owner = owner.ok_or("CharacterDialogue executable is unavailable")?;
                let schema = schema.ok_or("CharacterDialogue generation schema is unavailable")?;
                if !schema.program_owner().same_program(owner)
                    || schema.generation_digest() != generation
                {
                    return Err("CharacterDialogue generation or executable is stale".to_owned());
                }
                let value = value
                    .into_unrestricted_runtime_value_for_program(owner)
                    .map_err(|error| error.to_string())?;
                let RuntimeValue::Opaque(opaque) = &value else {
                    return Err("CharacterDialogue snapshot has no opaque value".to_owned());
                };
                let value = schema
                    .admit(owner, &value, opaque.semantic_identity())
                    .map_err(|error| error.to_string())?;
                let result =
                    CharacterDialoguePresentationConfig::from_admitted(schema, &value, style_sheet)
                        .map_err(|error| error.to_string())?;
                if result.config_digest() != config_digest {
                    return Err("CharacterDialogue configuration digest is stale".to_owned());
                }
                Ok(result)
            }
        }
    }
}

impl CharacterDialoguePresentationConfig {
    /// Context-free presentation for already resolved, standalone text.
    pub const fn standalone(view: ViewId, inline_failure: InlineFailurePolicy) -> Self {
        Self {
            source: CharacterDialoguePresentationSource::Standalone {
                view,
                inline_failure,
            },
            style_sheet: None,
        }
    }

    /// Retains all admitted roles, custom fields and captures of the original value.
    pub fn from_admitted(
        schema: &CharacterDialogueRuntimeSchema,
        value: &CharacterDialogueValue,
        style_sheet: Option<ViewStyleSheetId>,
    ) -> Result<Self, Box<CharacterDialogueValueError>> {
        value
            .opaque()
            .payload()
            .validate_nesting_depth(arcweft_core::value::MAX_RUNTIME_VALUE_NESTING_DEPTH)
            .map_err(|error| CharacterDialogueValueError::Field {
                field: "runtime_value",
                reason: error.to_string(),
            })?;
        if !value.opaque().payload().ownership().permits_copy() {
            return Err(Box::new(CharacterDialogueValueError::Field {
                field: "runtime_value",
                reason: "retained CharacterDialogue contains an affine value".to_owned(),
            }));
        }
        let value = schema.admit(
            schema.program_owner(),
            &RuntimeValue::Opaque(value.opaque().clone()),
            value.opaque().semantic_identity(),
        )?;
        let config_digest = schema.effective_config_digest(value.dialogue())?;
        let expected_sheet = match value.dialogue().config().style().typed().value() {
            RuntimeValue::EntityRef(reference) => {
                let label = reference
                    .project_label(arcweft_id::DeclarationIdentityFamily::Style)
                    .ok_or_else(|| CharacterDialogueValueError::Field {
                        field: "style_sheet",
                        reason: "Style reference has another entity family".to_owned(),
                    })?;
                Some(
                    ViewStyleSheetId::parse_public(label.to_owned()).map_err(|error| {
                        CharacterDialogueValueError::Field {
                            field: "style_sheet",
                            reason: error.to_string(),
                        }
                    })?,
                )
            }
            _ => None,
        };
        if style_sheet != expected_sheet {
            return Err(Box::new(CharacterDialogueValueError::Field {
                field: "style_sheet",
                reason: "stylesheet differs from the original admitted Style role".to_owned(),
            }));
        }
        Ok(Self {
            source: CharacterDialoguePresentationSource::Program {
                owner: schema.program_owner().clone(),
                generation: schema.generation_digest(),
                config_digest,
                value: Box::new(value),
            },
            style_sheet,
        })
    }

    #[must_use]
    pub const fn view(&self) -> &ViewId {
        match &self.source {
            CharacterDialoguePresentationSource::Standalone { view, .. } => view,
            CharacterDialoguePresentationSource::Program { value, .. } => {
                value.dialogue().config().view()
            }
        }
    }

    #[must_use]
    pub const fn inline_failure(&self) -> &InlineFailurePolicy {
        match &self.source {
            CharacterDialoguePresentationSource::Standalone { inline_failure, .. } => {
                inline_failure
            }
            CharacterDialoguePresentationSource::Program { value, .. } => {
                value.dialogue().config().inline_failure()
            }
        }
    }

    #[must_use]
    pub const fn style_sheet(&self) -> Option<&ViewStyleSheetId> {
        self.style_sheet.as_ref()
    }

    #[must_use]
    pub const fn config_digest(&self) -> RuntimeValueDigest {
        match &self.source {
            CharacterDialoguePresentationSource::Standalone { .. } => RuntimeValueDigest::ZERO,
            CharacterDialoguePresentationSource::Program { config_digest, .. } => *config_digest,
        }
    }

    #[must_use]
    pub const fn program_owner(&self) -> Option<&RuntimeProgramOwner> {
        match &self.source {
            CharacterDialoguePresentationSource::Standalone { .. } => None,
            CharacterDialoguePresentationSource::Program { owner, .. } => Some(owner),
        }
    }

    #[must_use]
    pub const fn admitted(&self) -> Option<&CharacterDialogueValue> {
        match &self.source {
            CharacterDialoguePresentationSource::Standalone { .. } => None,
            CharacterDialoguePresentationSource::Program { value, .. } => Some(value),
        }
    }

    pub(super) fn visit_runtime_values<E>(
        &self,
        visit: &mut impl FnMut(&RuntimeValue) -> Result<(), E>,
    ) -> Result<(), E> {
        match &self.source {
            CharacterDialoguePresentationSource::Program { value, .. } => {
                arcweft_core::value::visit_runtime_value_graph(value.opaque().payload(), visit)
            }
            CharacterDialoguePresentationSource::Standalone { inline_failure, .. } => {
                inline_failure
                    .borrow_styles()
                    .try_map_styles(&mut |style| {
                        arcweft_core::value::visit_runtime_value_graph(
                            style.typed().value(),
                            &mut *visit,
                        )
                    })?;
                Ok(())
            }
        }
    }

    /// Selects a stylesheet for standalone renderer inputs. Program-backed
    /// configurations obtain their stylesheet from their admitted Style role.
    pub fn with_standalone_style_sheet(
        mut self,
        sheet: Option<ViewStyleSheetId>,
    ) -> Result<Self, &'static str> {
        if !matches!(
            self.source,
            CharacterDialoguePresentationSource::Standalone { .. }
        ) {
            return Err("an admitted CharacterDialogue stylesheet cannot be overridden");
        }
        self.style_sheet = sheet;
        Ok(self)
    }
}

impl Serialize for CharacterDialoguePresentationConfig {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.source {
            CharacterDialoguePresentationSource::Standalone {
                view,
                inline_failure,
            } => {
                #[derive(Serialize)]
                struct Standalone<'a> {
                    kind: &'static str,
                    view: &'a ViewId,
                    style_sheet: &'a Option<ViewStyleSheetId>,
                    inline_failure: &'a InlineFailurePolicy,
                }
                Standalone {
                    kind: "standalone",
                    view,
                    style_sheet: &self.style_sheet,
                    inline_failure,
                }
                .serialize(serializer)
            }
            CharacterDialoguePresentationSource::Program {
                owner,
                generation,
                config_digest,
                value,
            } => {
                let value = AwbcRuntimeValueSnapshot::from_unrestricted_runtime_value_for_program(
                    &RuntimeValue::Opaque(value.opaque().clone()),
                    owner,
                )
                .map_err(serde::ser::Error::custom)?;
                CharacterDialoguePresentationConfigSnapshot::Program {
                    generation: *generation,
                    config_digest: *config_digest,
                    style_sheet: self.style_sheet.clone(),
                    value,
                }
                .serialize(serializer)
            }
        }
    }
}
