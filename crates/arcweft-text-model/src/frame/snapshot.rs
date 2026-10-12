//! Original-program restoration of display-frame runtime values.

use super::{CharacterDialoguePresentationConfigSnapshot, LineDisplayFrame};
use arcweft_core::{
    task::RuntimeProgramOwner,
    value::{AwbcRuntimeValueSnapshot, RuntimeDialogueContentValue},
};
use arcweft_dialogue::{CharacterDialogueRuntimeSchema, InlineFailurePolicy, InlineTextFailure};
use serde::{Deserialize, Serialize};

pub type LineDisplayStageSnapshot = crate::LineDisplayStageProjection<
    CharacterDialoguePresentationConfigSnapshot,
    AwbcRuntimeValueSnapshot,
    InlineTextFailureSnapshot,
>;

impl LineDisplayStageSnapshot {
    pub(crate) fn from_live(frame: &crate::LineDisplayStageProjection) -> Result<Self, String> {
        frame.validate().map_err(|error| error.to_string())?;
        if frame
            .effective
            .admitted()
            .is_some_and(|value| value.dialogue().character() != &frame.character.id)
        {
            return Err(
                "display stage Character differs from its admitted configuration".to_owned(),
            );
        }
        let owner = frame.effective.program_owner();
        Ok(Self {
            line: frame.line.clone(), character: frame.character.clone(), text_key: frame.text_key.clone(),
            effective: CharacterDialoguePresentationConfigSnapshot::from_live(&frame.effective, owner, None)?,
            text: frame.text.clone(), base_styles: frame.base_styles.clone(), style_contributions: frame.style_contributions.clone(),
            stage_index: frame.stage_index, page_index: frame.page_index, reveal_start: frame.reveal_start, end: frame.end,
            display_map: frame.display_map.clone(), host_events: frame.host_events.clone(), unresolved: frame.unresolved.clone(),
            content: match owner {
                Some(owner) => AwbcRuntimeValueSnapshot::from_unrestricted_runtime_value_for_program(&frame.content.clone().into_runtime_value(), owner),
                None => AwbcRuntimeValueSnapshot::from_unrestricted_standalone_value(&frame.content.clone().into_runtime_value()),
            }.map_err(|error| error.to_string())?,
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

impl LineDisplayFrame {
    /// Visits every retained value, including original roles/custom fields,
    /// Content callbacks and fallback Styles, before presentation publication.
    pub fn visit_runtime_values<E>(
        &self,
        mut visit: impl FnMut(&arcweft_core::value::RuntimeValue) -> Result<(), E>,
    ) -> Result<(), E> {
        self.effective.visit_runtime_values(&mut visit)?;
        arcweft_core::value::visit_runtime_value_graph(
            &self.content.clone().into_runtime_value(),
            &mut visit,
        )?;
        for failure in &self.inline_failures {
            failure
                .policy
                .borrow_styles()
                .try_map_styles(&mut |style| {
                    arcweft_core::value::visit_runtime_value_graph(
                        style.typed().value(),
                        &mut visit,
                    )
                })?;
        }
        Ok(())
    }
}

impl<E, C, I> LineDisplayFrame<E, C, I> {
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
    pub fn from_live(
        frame: &LineDisplayFrame,
        owner: Option<&RuntimeProgramOwner>,
        schema: Option<&CharacterDialogueRuntimeSchema>,
    ) -> Result<Self, String> {
        frame.validate().map_err(|error| error.to_string())?;
        if frame
            .effective
            .admitted()
            .is_some_and(|value| value.dialogue().character() != &frame.character.id)
        {
            return Err(
                "display frame Character differs from its admitted configuration".to_owned(),
            );
        }
        frame.clone().try_map_runtime(
            |config| CharacterDialoguePresentationConfigSnapshot::from_live(&config, owner, schema),
            |content| match owner { Some(owner) => AwbcRuntimeValueSnapshot::from_unrestricted_runtime_value_for_program(&content.into_runtime_value(), owner), None => AwbcRuntimeValueSnapshot::from_unrestricted_standalone_value(&content.into_runtime_value()) }.map_err(|error| error.to_string()),
            |failure| Ok(InlineTextFailureSnapshot {
                expr: failure.expr,
                reason: failure.reason,
                policy: failure.policy.borrow_styles().try_map_styles(&mut |style| {
                    match owner { Some(owner) => AwbcRuntimeValueSnapshot::from_unrestricted_runtime_value_for_program(style.typed().value(), owner), None => AwbcRuntimeValueSnapshot::from_unrestricted_standalone_value(style.typed().value()) }.map_err(|error| error.to_string())
                })?,
            }),
        )
    }

    pub fn into_live(
        self,
        owner: Option<&RuntimeProgramOwner>,
        schema: Option<&CharacterDialogueRuntimeSchema>,
    ) -> Result<LineDisplayFrame, String> {
        self.validate().map_err(|error| error.to_string())?;
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
