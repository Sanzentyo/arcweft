//! Original-program restoration of display-frame runtime values.

use super::{CharacterDialoguePresentationConfigSnapshot, LineDisplayFrame};
use arcweft_core::{
    task::RuntimeProgramOwner,
    value::{AwbcRuntimeValueSnapshot, RuntimeDialogueContentValue, RuntimeRetainedValueAdmission},
};
use arcweft_dialogue::{CharacterDialogueRuntimeSchema, InlineFailurePolicy, InlineTextFailure};
use serde::{Deserialize, Serialize};

pub type LineDisplayStageSnapshot = crate::LineDisplayStageProjection<
    CharacterDialoguePresentationConfigSnapshot,
    AwbcRuntimeValueSnapshot,
    InlineTextFailureSnapshot,
>;

impl crate::LineDisplayStageProjection {
    fn admit_retained(&self, admission: &mut RuntimeRetainedValueAdmission) -> Result<(), String> {
        admission
            .validate_metadata(&(
                &self.line,
                &self.character,
                &self.text_key,
                &self.text,
                &self.base_styles,
                &self.style_contributions,
                &self.display_map,
                &self.host_events,
                &self.unresolved,
            ))
            .map_err(|error| error.to_string())?;
        self.effective.admit_retained_metadata(admission)?;
        admission
            .validate_metadata_collection(self.inline_failures.len())
            .map_err(|error| error.to_string())?;
        for failure in &self.inline_failures {
            admission
                .validate_metadata(&(
                    &failure.expr,
                    &failure.reason,
                    failure.policy.retained_text(),
                ))
                .map_err(|error| error.to_string())?;
            admission
                .validate_metadata_collection(failure.policy.retained_styles().len())
                .map_err(|error| error.to_string())?;
        }
        self.effective.admit_retained_values(admission, None)?;
        admission
            .validate_dialogue_content(&self.content)
            .map_err(|error| error.to_string())?;
        for failure in &self.inline_failures {
            for style in failure.policy.retained_styles() {
                admission
                    .validate(style.typed().value())
                    .map_err(|error| error.to_string())?;
            }
        }
        self.validate().map_err(|error| error.to_string())?;
        if self
            .effective
            .admitted()
            .is_some_and(|value| value.dialogue().character() != &self.character.id)
        {
            return Err(
                "display stage Character differs from its admitted configuration".to_owned(),
            );
        }
        Ok(())
    }
}

impl LineDisplayStageSnapshot {
    pub(crate) fn from_live(frame: &crate::LineDisplayStageProjection) -> Result<Self, String> {
        let owner = frame.effective.program_owner();
        let mut admission = match owner {
            Some(owner) => RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
            ),
            None => RuntimeRetainedValueAdmission::standalone(
                arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
            ),
        };
        frame.admit_retained(&mut admission)?;
        Ok(Self {
            line: frame.line.clone(), character: frame.character.clone(), text_key: frame.text_key.clone(),
            effective: CharacterDialoguePresentationConfigSnapshot::from_live(&frame.effective, owner, None)?,
            text: frame.text.clone(), base_styles: frame.base_styles.clone(), style_contributions: frame.style_contributions.clone(),
            stage_index: frame.stage_index, page_index: frame.page_index, reveal_start: frame.reveal_start, end: frame.end,
            display_map: frame.display_map.clone(), host_events: frame.host_events.clone(), unresolved: frame.unresolved.clone(),
            content: AwbcRuntimeValueSnapshot::from_unrestricted_dialogue_content(&frame.content, owner).map_err(|error| error.to_string())?,
            inline_failures: frame.inline_failures.iter().map(|failure| {
                Ok(InlineTextFailureSnapshot {
                    expr: failure.expr.clone(), reason: failure.reason.clone(),
                    policy: failure.policy.borrow_styles().try_map_styles(&mut |style| {
                        match owner {
                            Some(owner) => AwbcRuntimeValueSnapshot::from_unrestricted_runtime_value_for_program(style.typed().value(), owner),
                            None => AwbcRuntimeValueSnapshot::from_unrestricted_standalone_value(style.typed().value()),
                        }.map_err(|error| error.to_string())
                    })?,
                })
            }).collect::<Result<_, String>>()?,
        })
    }
}

/// Failure metadata retaining inert fallback Style values.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InlineTextFailureSnapshot {
    expr: String,
    reason: String,
    policy: InlineFailurePolicy<AwbcRuntimeValueSnapshot>,
}

/// Same complete display/source metadata with inert runtime-owned fields.
pub type LineDisplayFrameSnapshot = LineDisplayFrame<
    CharacterDialoguePresentationConfigSnapshot,
    AwbcRuntimeValueSnapshot,
    InlineTextFailureSnapshot,
>;

// The owning frame selects the same retained fields in both storage forms.
// Only the borrowed Style projection differs; no live value is reconstructed.
macro_rules! visit_display_frame_values {
    ($frame:ty, |$style:ident| $borrowed_style:expr) => {
        impl $frame {
            /// Visits retained configuration, Content and fallback Style values
            /// without constructing or copying a live runtime carrier.
            pub fn visit_runtime_values(
                &self,
                mut visit: impl for<'value> FnMut(
                    arcweft_core::value::RuntimeBorrowedValue<'value>,
                ) -> Result<(), String>,
            ) -> Result<(), String> {
                self.effective.visit_runtime_values(&mut visit)?;
                let limits = arcweft_core::entry::RuntimeSchemaLimits::engine_default();
                arcweft_core::value::RuntimeBorrowedValue::from(&self.content)
                    .try_visit(limits, &mut visit)
                    .map_err(|error| error.to_string())?;
                for failure in &self.inline_failures {
                    for $style in failure.policy.retained_styles() {
                        let value: arcweft_core::value::RuntimeBorrowedValue<'_> = $borrowed_style;
                        value
                            .try_visit(limits, &mut visit)
                            .map_err(|error| error.to_string())?;
                    }
                }
                Ok(())
            }
        }
    };
}

visit_display_frame_values!(LineDisplayFrame, |style| style.typed().value().into());
visit_display_frame_values!(LineDisplayFrameSnapshot, |style| style.into());

impl LineDisplayFrame {
    /// Bounds every display/source field through its owning typed serde grammar
    /// before validation builds projections or persistence copies the frame.
    pub fn admit_retained_metadata(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), String> {
        self.admit_display_metadata(admission)?;
        self.effective.admit_retained_metadata(admission)?;
        admission
            .validate_metadata_collection(self.inline_failures.len())
            .map_err(|error| error.to_string())?;
        for failure in &self.inline_failures {
            admission
                .validate_metadata(&(
                    &failure.expr,
                    &failure.reason,
                    failure.policy.retained_text(),
                ))
                .map_err(|error| error.to_string())?;
            admission
                .validate_metadata_collection(failure.policy.retained_styles().len())
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// Admits all runtime fields with the enclosing group's original executable
    /// and cumulative allowance, without constructing a display snapshot.
    pub fn admit_retained_values(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        schema: Option<&CharacterDialogueRuntimeSchema>,
    ) -> Result<(), String> {
        self.effective.admit_retained_values(admission, schema)?;
        admission
            .validate_dialogue_content(&self.content)
            .map_err(|error| error.to_string())?;
        for failure in &self.inline_failures {
            for style in failure.policy.retained_styles() {
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
        self.validate().map_err(|error| error.to_string())?;
        if self
            .effective
            .admitted()
            .is_some_and(|value| value.dialogue().character() != &self.character.id)
        {
            return Err(
                "display frame Character differs from its admitted configuration".to_owned(),
            );
        }
        Ok(())
    }
}

impl<E, C, I> LineDisplayFrame<E, C, I> {
    fn admit_display_metadata(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), String> {
        admission
            .validate_metadata(&(
                &self.line,
                &self.character,
                &self.text_key,
                &self.text,
                &self.base_styles,
                &self.style_contributions,
                &self.nodes,
                &self.display_map,
                &self.host_events,
                &self.unresolved,
            ))
            .map_err(|error| error.to_string())
    }

    /// Maps only runtime-owned fields; all display/source metadata moves intact.
    pub fn try_map_runtime<TE, TC, TI, X>(
        self,
        mut effective: impl FnMut(E) -> Result<TE, X>,
        mut content: impl FnMut(C) -> Result<TC, X>,
        mut failure: impl FnMut(I) -> Result<TI, X>,
    ) -> Result<LineDisplayFrame<TE, TC, TI>, X> {
        Ok(LineDisplayFrame {
            line: self.line,
            character: self.character,
            text_key: self.text_key,
            effective: effective(self.effective)?,
            text: self.text,
            base_styles: self.base_styles,
            style_contributions: self.style_contributions,
            nodes: self.nodes,
            display_map: self.display_map,
            host_events: self.host_events,
            inline_failures: self
                .inline_failures
                .into_iter()
                .map(&mut failure)
                .collect::<Result<_, _>>()?,
            unresolved: self.unresolved,
            content: content(self.content)?,
        })
    }
}

impl LineDisplayFrameSnapshot {
    /// Bounds inert metadata in the same enclosing group as live frames.
    pub fn admit_retained_metadata(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), String> {
        self.admit_display_metadata(admission)?;
        self.effective.admit_retained_metadata(admission)?;
        admission
            .validate_metadata_collection(self.inline_failures.len())
            .map_err(|error| error.to_string())?;
        for failure in &self.inline_failures {
            admission
                .validate_metadata(&(
                    &failure.expr,
                    &failure.reason,
                    failure.policy.retained_text(),
                ))
                .map_err(|error| error.to_string())?;
            admission
                .validate_metadata_collection(failure.policy.retained_styles().len())
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// Completes original stored-type and resource checks before restoring any
    /// Content callback or Style runtime carrier from this frame.
    pub fn admit_retained_values(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        schema: Option<&CharacterDialogueRuntimeSchema>,
    ) -> Result<(), String> {
        self.effective.admit_retained_values(admission, schema)?;
        admission
            .validate_snapshot(&self.content)
            .map_err(|error| error.to_string())?;
        for failure in &self.inline_failures {
            for style in failure.policy.retained_styles() {
                let schema = schema.ok_or("fallback Style generation schema is unavailable")?;
                schema
                    .validate_retained_style_snapshot(admission, style)
                    .map_err(|error| error.to_string())?;
            }
        }
        self.validate().map_err(|error| error.to_string())
    }

    pub fn from_live(
        frame: &LineDisplayFrame,
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
        frame.admit_retained_metadata(&mut admission)?;
        frame.admit_retained_values(&mut admission, schema)?;
        let effective = CharacterDialoguePresentationConfigSnapshot::from_live(
            &frame.effective,
            owner,
            schema,
        )?;
        let content =
            AwbcRuntimeValueSnapshot::from_unrestricted_dialogue_content(&frame.content, owner)
                .map_err(|error| error.to_string())?;
        let inline_failures = frame.inline_failures.iter().map(|failure| {
            Ok(InlineTextFailureSnapshot {
                expr: failure.expr.clone(),
                reason: failure.reason.clone(),
                policy: failure.policy.borrow_styles().try_map_styles(&mut |style| {
                    match owner {
                        Some(owner) => AwbcRuntimeValueSnapshot::from_unrestricted_runtime_value_for_program(style.typed().value(), owner),
                        None => AwbcRuntimeValueSnapshot::from_unrestricted_standalone_value(style.typed().value()),
                    }.map_err(|error| error.to_string())
                })?,
            })
        }).collect::<Result<_, String>>()?;
        Ok(Self {
            line: frame.line.clone(),
            character: frame.character.clone(),
            text_key: frame.text_key.clone(),
            effective,
            text: frame.text.clone(),
            base_styles: frame.base_styles.clone(),
            style_contributions: frame.style_contributions.clone(),
            nodes: frame.nodes.clone(),
            display_map: frame.display_map.clone(),
            host_events: frame.host_events.clone(),
            inline_failures,
            unresolved: frame.unresolved.clone(),
            content,
        })
    }

    pub fn into_live(
        self,
        owner: Option<&RuntimeProgramOwner>,
        schema: Option<&CharacterDialogueRuntimeSchema>,
    ) -> Result<LineDisplayFrame, String> {
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
        let frame = self.try_map_runtime(
            |config| config.into_live(owner, schema),
            |content| {
                let value = match owner {
                    Some(owner) => content.into_unrestricted_runtime_value_for_program(owner),
                    None => content.into_unrestricted_standalone_value(),
                }
                .map_err(|error| error.to_string())?;
                RuntimeDialogueContentValue::try_from_runtime_value(&value)
                    .map_err(|error| error.to_string())
            },
            |failure| {
                Ok(InlineTextFailure {
                    expr: failure.expr,
                    reason: failure.reason,
                    policy: failure.policy.try_map_styles(&mut |value| {
                        let value = match owner {
                            Some(owner) => value.into_unrestricted_runtime_value_for_program(owner),
                            None => value.into_unrestricted_standalone_value(),
                        }
                        .map_err(|error| error.to_string())?;
                        let schema =
                            schema.ok_or("fallback Style generation schema is unavailable")?;
                        schema
                            .admit_style(
                                owner.ok_or("fallback Style executable is unavailable")?,
                                &value,
                            )
                            .map_err(|error| error.to_string())
                    })?,
                })
            },
        )?;
        if frame
            .effective
            .admitted()
            .is_some_and(|value| value.dialogue().character() != &frame.character.id)
        {
            return Err(
                "display frame Character differs from its admitted configuration".to_owned(),
            );
        }
        Ok(frame)
    }
}

impl Serialize for LineDisplayFrameSnapshot {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut wire = serializer.serialize_struct("LineDisplayFrameSnapshot", 13)?;
        wire.serialize_field("line", &self.line)?;
        wire.serialize_field("character", &self.character)?;
        wire.serialize_field("text_key", &self.text_key)?;
        wire.serialize_field("effective", &self.effective)?;
        wire.serialize_field("text", &self.text)?;
        wire.serialize_field("base_styles", &self.base_styles)?;
        wire.serialize_field("style_contributions", &self.style_contributions)?;
        wire.serialize_field("nodes", &self.nodes)?;
        wire.serialize_field("display_map", &self.display_map)?;
        wire.serialize_field("host_events", &self.host_events)?;
        wire.serialize_field("inline_failures", &self.inline_failures)?;
        wire.serialize_field("unresolved", &self.unresolved)?;
        wire.serialize_field("content", &self.content)?;
        wire.end()
    }
}

impl Serialize for LineDisplayStageSnapshot {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut wire = serializer.serialize_struct("LineDisplayStageSnapshot", 16)?;
        wire.serialize_field("line", &self.line)?;
        wire.serialize_field("character", &self.character)?;
        wire.serialize_field("text_key", &self.text_key)?;
        wire.serialize_field("effective", &self.effective)?;
        wire.serialize_field("text", &self.text)?;
        wire.serialize_field("base_styles", &self.base_styles)?;
        wire.serialize_field("style_contributions", &self.style_contributions)?;
        wire.serialize_field("stage_index", &self.stage_index)?;
        wire.serialize_field("page_index", &self.page_index)?;
        wire.serialize_field("reveal_start", &self.reveal_start)?;
        wire.serialize_field("end", &self.end)?;
        wire.serialize_field("display_map", &self.display_map)?;
        wire.serialize_field("host_events", &self.host_events)?;
        wire.serialize_field("inline_failures", &self.inline_failures)?;
        wire.serialize_field("unresolved", &self.unresolved)?;
        wire.serialize_field("content", &self.content)?;
        wire.end()
    }
}

impl Serialize for LineDisplayFrame {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        LineDisplayFrameSnapshot::from_live(self, self.effective.program_owner(), None)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl Serialize for crate::LineDisplayStageProjection {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        LineDisplayStageSnapshot::from_live(self)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}
