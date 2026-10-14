//! Retains the admitted configuration instead of its lossy render projection.

use arcweft_core::{
    entry::RuntimeValueDigest,
    task::RuntimeProgramOwner,
    value::{AwbcRuntimeValueSnapshot, RuntimeRetainedValueAdmission, RuntimeValue},
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
    pub(super) fn admit_retained_metadata(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), String> {
        match self {
            Self::Standalone {
                view,
                style_sheet,
                inline_failure,
            } => {
                admission
                    .validate_metadata(&(view, style_sheet, inline_failure.retained_text()))
                    .map_err(|error| error.to_string())?;
                admission
                    .validate_metadata_collection(inline_failure.retained_styles().len())
                    .map_err(|error| error.to_string())?;
            }
            Self::Program { style_sheet, .. } => admission
                .validate_metadata(style_sheet)
                .map_err(|error| error.to_string())?,
        }
        Ok(())
    }

    pub(super) fn admit_retained_values(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        schema: Option<&CharacterDialogueRuntimeSchema>,
    ) -> Result<(), String> {
        match self {
            Self::Standalone { inline_failure, .. } => {
                for style in inline_failure.retained_styles() {
                    let schema = schema.ok_or("fallback Style generation schema is unavailable")?;
                    schema
                        .validate_retained_style_snapshot(admission, style)
                        .map_err(|error| error.to_string())?;
                }
            }
            Self::Program {
                generation, value, ..
            } => {
                let schema = schema.ok_or("CharacterDialogue generation schema is unavailable")?;
                if !admission
                    .program_owner()
                    .is_some_and(|owner| schema.program_owner().same_program(owner))
                    || schema.generation_digest() != *generation
                {
                    return Err("CharacterDialogue generation or executable is stale".to_owned());
                }
                admission
                    .validate_snapshot(value)
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }

    pub fn from_live(
        config: &CharacterDialoguePresentationConfig,
        owner: Option<&RuntimeProgramOwner>,
        schema: Option<&CharacterDialogueRuntimeSchema>,
    ) -> Result<Self, String> {
        let mut admission = match owner {
            Some(owner) => RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
            ),
            None => RuntimeRetainedValueAdmission::standalone(
                arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
            ),
        };
        config.admit_retained_metadata(&mut admission)?;
        config.admit_retained_values(&mut admission, schema)?;
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
                Ok(Self::Program {
                    generation: *generation,
                    config_digest: *config_digest,
                    style_sheet: config.style_sheet.clone(),
                    value: AwbcRuntimeValueSnapshot::from_unrestricted_opaque(value.opaque(), Some(owner)).map_err(|error| error.to_string())?,
                })
            }
        }
    }

    pub fn into_live(
        self,
        owner: Option<&RuntimeProgramOwner>,
        schema: Option<&CharacterDialogueRuntimeSchema>,
    ) -> Result<CharacterDialoguePresentationConfig, String> {
        let mut admission = match owner {
            Some(owner) => RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
            ),
            None => RuntimeRetainedValueAdmission::standalone(
                arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
            ),
        };
        self.admit_retained_metadata(&mut admission)?;
        self.admit_retained_values(&mut admission, schema)?;
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
    pub(super) fn admit_retained_metadata(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), String> {
        if let Some(sheet) = &self.style_sheet {
            admission
                .validate_metadata_string(sheet.as_str())
                .map_err(|error| error.to_string())?;
        }
        if let CharacterDialoguePresentationSource::Standalone {
            view,
            inline_failure,
        } = &self.source
        {
            admission
                .validate_metadata_string(view.as_str())
                .map_err(|error| error.to_string())?;
            if let Some(text) = inline_failure.retained_text() {
                admission
                    .validate_metadata_string(text)
                    .map_err(|error| error.to_string())?;
            }
            admission
                .validate_metadata_collection(inline_failure.retained_styles().len())
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub(super) fn admit_retained_values(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        schema: Option<&CharacterDialogueRuntimeSchema>,
    ) -> Result<(), String> {
        match &self.source {
            CharacterDialoguePresentationSource::Standalone { inline_failure, .. } => {
                for style in inline_failure.retained_styles() {
                    if let Some(schema) = schema {
                        schema
                            .validate_retained_style(admission, style.typed().value())
                            .map_err(|error| error.to_string())?;
                    } else {
                        admission
                            .validate(style.typed().value())
                            .map_err(|error| error.to_string())?;
                    }
                }
            }
            CharacterDialoguePresentationSource::Program {
                owner,
                generation,
                config_digest,
                value,
            } => {
                if !admission
                    .program_owner()
                    .is_some_and(|original| original.same_program(owner))
                {
                    return Err("CharacterDialogue executable is stale".to_owned());
                }
                if let Some(schema) = schema {
                    if !schema.program_owner().same_program(owner)
                        || schema.generation_digest() != *generation
                    {
                        return Err(
                            "CharacterDialogue generation or executable is stale".to_owned()
                        );
                    }
                    schema
                        .validate_retained_dialogue(admission, value)
                        .map_err(|error| error.to_string())?;
                    if schema
                        .effective_config_digest(value.dialogue())
                        .map_err(|error| error.to_string())?
                        != *config_digest
                    {
                        return Err("CharacterDialogue configuration digest is stale".to_owned());
                    }
                } else {
                    admission
                        .validate_opaque(value.opaque())
                        .map_err(|error| error.to_string())?;
                }
                let expected_sheet = match value.dialogue().config().style().typed().value() {
                    RuntimeValue::EntityRef(reference) => Some(
                        reference
                            .project_label(arcweft_id::DeclarationIdentityFamily::Style)
                            .ok_or("Style reference has another entity family")?,
                    ),
                    _ => None,
                };
                if self.style_sheet.as_ref().map(ViewStyleSheetId::as_str) != expected_sheet {
                    return Err(
                        "stylesheet differs from the original admitted Style role".to_owned()
                    );
                }
            }
        }
        Ok(())
    }

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
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            schema.program_owner().clone(),
            arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
        );
        admission
            .validate_metadata(&style_sheet)
            .map_err(CharacterDialogueValueError::from)?;
        schema.validate_retained_dialogue(&mut admission, value)?;
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
                value: Box::new(value.clone()),
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

    pub(super) fn visit_runtime_values(
        &self,
        visit: &mut impl for<'value> FnMut(
            arcweft_core::value::RuntimeBorrowedValue<'value>,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        let limits = arcweft_core::entry::RuntimeSchemaLimits::engine_default();
        match &self.source {
            CharacterDialoguePresentationSource::Program { value, .. } => {
                arcweft_core::value::RuntimeBorrowedValue::from(value.opaque())
                    .try_visit(limits, visit)
                    .map_err(|error| error.to_string())
            }
            CharacterDialoguePresentationSource::Standalone { inline_failure, .. } => {
                for style in inline_failure.retained_styles() {
                    arcweft_core::value::RuntimeBorrowedValue::from(style.typed().value())
                        .try_visit(limits, &mut *visit)
                        .map_err(|error| error.to_string())?;
                }
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
        CharacterDialoguePresentationConfigSnapshot::from_live(self, self.program_owner(), None)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}
