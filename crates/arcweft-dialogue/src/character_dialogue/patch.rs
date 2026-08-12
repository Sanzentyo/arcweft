//! Immutable `CharacterDialogue` patch operands and merge rules.

use super::{
    CharacterDialogueCleanupValue, CharacterDialogueConfig, CharacterDialogueCustomFieldId,
    CharacterDialogueCustomValue, CharacterDialogueFocusValue, CharacterDialogueHookValue,
    CharacterDialoguePortraitValue, CharacterDialogueRichTextValue, CharacterDialogueStageValue,
    CharacterDialogueStyleValue, CharacterDialogueValueError, CharacterDialogueVoice,
    DialogueLocaleId, PRODUCTION_CHARACTER_DIALOGUE_LIMITS,
    typed_value::{empty_like, empty_runtime_value, replace_runtime_value},
};
use crate::InlineFailurePolicy;
use arcweft_character::id::CharacterLookId;
use arcweft_core::value::{RuntimeNominalRecordValue, RuntimeSeq, RuntimeValue};
use arcweft_view::ViewId;
use core::marker::PhantomData;
use std::collections::BTreeMap;

/// Tri-state checked patch coordinate.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum PatchField<T> {
    #[default]
    Unspecified,
    Set(T),
    Clear,
}

/// Schema-ordinal path to one runtime record leaf.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeFieldPath(Vec<u16>);

/// Field-wise patch for a typed structured value.
#[derive(Clone, Debug, PartialEq)]
pub struct StructuredPatch<T> {
    clear_all: bool,
    assignments: BTreeMap<RuntimeFieldPath, PatchField<RuntimeValue>>,
    marker: PhantomData<fn() -> T>,
}

/// Complete checked reusable `CharacterDialogue` patch.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CharacterDialoguePatch {
    voice: PatchField<CharacterDialogueVoice>,
    look: PatchField<CharacterLookId>,
    stage: PatchField<CharacterDialogueStageValue>,
    portrait: PatchField<CharacterDialoguePortraitValue>,
    focus: PatchField<CharacterDialogueFocusValue>,
    cleanup: PatchField<CharacterDialogueCleanupValue>,
    view: PatchField<ViewId>,
    source_locale: PatchField<DialogueLocaleId>,
    hooks: PatchField<Vec<CharacterDialogueHookValue>>,
    style: StructuredPatch<CharacterDialogueStyleValue>,
    rich_text: StructuredPatch<CharacterDialogueRichTextValue>,
    inline_failure: PatchField<InlineFailurePolicy>,
    custom: BTreeMap<CharacterDialogueCustomFieldId, PatchField<CharacterDialogueCustomValue>>,
}

impl RuntimeFieldPath {
    pub fn try_new(ordinals: impl Into<Vec<u16>>) -> Result<Self, CharacterDialogueValueError> {
        let ordinals = ordinals.into();
        if ordinals.is_empty() {
            return Err(CharacterDialogueValueError::Field {
                field: "structured_patch",
                reason: "field path must contain at least one ordinal".to_owned(),
            });
        }
        let maximum = usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_structured_depth);
        if ordinals.len() > maximum {
            return Err(CharacterDialogueValueError::Limit {
                limit: "structured_depth",
                maximum,
            });
        }
        Ok(Self(ordinals))
    }

    #[must_use]
    pub fn ordinals(&self) -> &[u16] {
        &self.0
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.0.starts_with(&other.0) || other.0.starts_with(&self.0)
    }
}

impl<T> StructuredPatch<T> {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            clear_all: false,
            assignments: BTreeMap::new(),
            marker: PhantomData,
        }
    }

    #[must_use]
    pub const fn clear_all() -> Self {
        Self {
            clear_all: true,
            assignments: BTreeMap::new(),
            marker: PhantomData,
        }
    }

    pub fn try_new(
        clear_all: bool,
        assignments: BTreeMap<RuntimeFieldPath, PatchField<RuntimeValue>>,
    ) -> Result<Self, CharacterDialogueValueError> {
        validate_assignment_paths(assignments.keys())?;
        let maximum = usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_structured_leaves);
        if assignments.len() > maximum {
            return Err(CharacterDialogueValueError::Limit {
                limit: "structured_leaves",
                maximum,
            });
        }
        Ok(Self {
            clear_all,
            assignments,
            marker: PhantomData,
        })
    }

    #[must_use]
    pub const fn clears_all(&self) -> bool {
        self.clear_all
    }

    #[must_use]
    pub const fn assignments(&self) -> &BTreeMap<RuntimeFieldPath, PatchField<RuntimeValue>> {
        &self.assignments
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.clear_all && self.assignments.is_empty()
    }
}

impl<T> Default for StructuredPatch<T> {
    fn default() -> Self {
        Self::empty()
    }
}

impl CharacterDialoguePatch {
    #[must_use]
    pub fn with_voice(mut self, value: PatchField<CharacterDialogueVoice>) -> Self {
        self.voice = value;
        self
    }

    #[must_use]
    pub fn with_look(mut self, value: PatchField<CharacterLookId>) -> Self {
        self.look = value;
        self
    }

    #[must_use]
    pub fn with_stage(mut self, value: PatchField<CharacterDialogueStageValue>) -> Self {
        self.stage = value;
        self
    }

    #[must_use]
    pub fn with_portrait(mut self, value: PatchField<CharacterDialoguePortraitValue>) -> Self {
        self.portrait = value;
        self
    }

    #[must_use]
    pub fn with_focus(mut self, value: PatchField<CharacterDialogueFocusValue>) -> Self {
        self.focus = value;
        self
    }

    #[must_use]
    pub fn with_cleanup(mut self, value: PatchField<CharacterDialogueCleanupValue>) -> Self {
        self.cleanup = value;
        self
    }

    #[must_use]
    pub fn with_view(mut self, value: PatchField<ViewId>) -> Self {
        self.view = value;
        self
    }

    #[must_use]
    pub fn with_source_locale(mut self, value: PatchField<DialogueLocaleId>) -> Self {
        self.source_locale = value;
        self
    }

    #[must_use]
    pub fn with_hooks(mut self, value: PatchField<Vec<CharacterDialogueHookValue>>) -> Self {
        self.hooks = value;
        self
    }

    #[must_use]
    pub fn with_style(mut self, value: StructuredPatch<CharacterDialogueStyleValue>) -> Self {
        self.style = value;
        self
    }

    #[must_use]
    pub fn with_rich_text(
        mut self,
        value: StructuredPatch<CharacterDialogueRichTextValue>,
    ) -> Self {
        self.rich_text = value;
        self
    }

    #[must_use]
    pub fn with_inline_failure(mut self, value: PatchField<InlineFailurePolicy>) -> Self {
        self.inline_failure = value;
        self
    }

    #[must_use]
    pub fn with_custom(
        mut self,
        field: CharacterDialogueCustomFieldId,
        value: PatchField<CharacterDialogueCustomValue>,
    ) -> Self {
        self.custom.insert(field, value);
        self
    }

    #[must_use]
    pub const fn voice(&self) -> &PatchField<CharacterDialogueVoice> {
        &self.voice
    }

    #[must_use]
    pub const fn look(&self) -> &PatchField<CharacterLookId> {
        &self.look
    }

    #[must_use]
    pub const fn view(&self) -> &PatchField<ViewId> {
        &self.view
    }

    #[must_use]
    pub const fn custom(
        &self,
    ) -> &BTreeMap<CharacterDialogueCustomFieldId, PatchField<CharacterDialogueCustomValue>> {
        &self.custom
    }

    fn validate(&self) -> Result<(), CharacterDialogueValueError> {
        let max_patch_fields = usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_patch_fields);
        let standard_fields = [
            &self.voice as &dyn SpecifiedPatch,
            &self.look,
            &self.stage,
            &self.portrait,
            &self.focus,
            &self.cleanup,
            &self.view,
            &self.source_locale,
            &self.hooks,
            &self.inline_failure,
        ]
        .into_iter()
        .filter(|field| field.is_specified())
        .count();
        let field_count = standard_fields
            .checked_add(self.style.assignments.len())
            .and_then(|count| count.checked_add(usize::from(self.style.clear_all)))
            .and_then(|count| count.checked_add(self.rich_text.assignments.len()))
            .and_then(|count| count.checked_add(usize::from(self.rich_text.clear_all)))
            .and_then(|count| count.checked_add(self.custom.len()))
            .ok_or(CharacterDialogueValueError::Limit {
                limit: "patch_fields",
                maximum: max_patch_fields,
            })?;
        if field_count > max_patch_fields {
            return Err(CharacterDialogueValueError::Limit {
                limit: "patch_fields",
                maximum: max_patch_fields,
            });
        }
        if let PatchField::Set(hooks) = &self.hooks
            && hooks.len() > usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_hooks)
        {
            return Err(CharacterDialogueValueError::Limit {
                limit: "hooks",
                maximum: usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_hooks),
            });
        }
        if self.custom.len() > usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_custom_fields) {
            return Err(CharacterDialogueValueError::Limit {
                limit: "custom_fields",
                maximum: usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_custom_fields),
            });
        }
        validate_assignment_paths(self.style.assignments.keys())?;
        validate_assignment_paths(self.rich_text.assignments.keys())
    }
}

trait SpecifiedPatch {
    fn is_specified(&self) -> bool;
}

impl<T> SpecifiedPatch for PatchField<T> {
    fn is_specified(&self) -> bool {
        !matches!(self, Self::Unspecified)
    }
}

pub(super) fn apply_patch(
    base: &CharacterDialogueConfig,
    patch: &CharacterDialoguePatch,
) -> Result<CharacterDialogueConfig, CharacterDialogueValueError> {
    patch.validate()?;
    let mut candidate = base.clone();
    apply_optional(&mut candidate.voice, &patch.voice);
    apply_optional(&mut candidate.look, &patch.look);
    apply_optional(&mut candidate.stage, &patch.stage);
    apply_optional(&mut candidate.portrait, &patch.portrait);
    apply_optional(&mut candidate.focus, &patch.focus);
    apply_optional(&mut candidate.cleanup, &patch.cleanup);
    match &patch.view {
        PatchField::Unspecified => {}
        PatchField::Set(view) => candidate.view = view.clone(),
        PatchField::Clear => candidate.view = standard_dialogue_view(),
    }
    apply_optional(&mut candidate.source_locale, &patch.source_locale);
    match &patch.hooks {
        PatchField::Unspecified => {}
        PatchField::Set(hooks) => candidate.hooks.clone_from(hooks),
        PatchField::Clear => candidate.hooks.clear(),
    }
    if !patch.style.is_empty() {
        candidate.style = apply_style(&candidate.style, &patch.style)?;
    }
    if !patch.rich_text.is_empty() {
        candidate.rich_text = apply_rich_text(&candidate.rich_text, &patch.rich_text)?;
    }
    match &patch.inline_failure {
        PatchField::Unspecified => {}
        PatchField::Set(policy) => candidate.inline_failure = policy.clone(),
        PatchField::Clear => candidate.inline_failure = InlineFailurePolicy::FailLine,
    }
    for (field, value) in &patch.custom {
        match value {
            PatchField::Unspecified => {}
            PatchField::Set(value) => {
                candidate.custom.insert(field.clone(), value.clone());
            }
            PatchField::Clear => {
                candidate.custom.remove(field);
            }
        }
    }
    candidate.validate()?;
    Ok(candidate)
}

fn apply_optional<T: Clone>(target: &mut Option<T>, patch: &PatchField<T>) {
    match patch {
        PatchField::Unspecified => {}
        PatchField::Set(value) => *target = Some(value.clone()),
        PatchField::Clear => *target = None,
    }
}

fn apply_style(
    base: &CharacterDialogueStyleValue,
    patch: &StructuredPatch<CharacterDialogueStyleValue>,
) -> Result<CharacterDialogueStyleValue, CharacterDialogueValueError> {
    let typed = apply_structured(base.typed(), patch)?;
    CharacterDialogueStyleValue::try_new(typed)
}

fn apply_rich_text(
    base: &CharacterDialogueRichTextValue,
    patch: &StructuredPatch<CharacterDialogueRichTextValue>,
) -> Result<CharacterDialogueRichTextValue, CharacterDialogueValueError> {
    let typed = apply_structured(base.typed(), patch)?;
    CharacterDialogueRichTextValue::try_new(typed)
}

fn apply_structured<T>(
    base: &super::CharacterDialogueTypedValue,
    patch: &StructuredPatch<T>,
) -> Result<super::CharacterDialogueTypedValue, CharacterDialogueValueError> {
    let mut typed = if patch.clear_all {
        empty_like(base)?
    } else {
        base.clone()
    };
    let mut runtime = typed.value().clone();
    for (path, operation) in &patch.assignments {
        match operation {
            PatchField::Unspecified => {}
            PatchField::Set(value) => {
                update_path(&mut runtime, path.ordinals(), Some(value.clone()))?;
            }
            PatchField::Clear => {
                update_path(&mut runtime, path.ordinals(), None)?;
            }
        }
    }
    typed = replace_runtime_value(typed, runtime);
    super::CharacterDialogueTypedValue::try_new(
        typed.nominal_type().cloned(),
        typed.layout(),
        typed.into_value(),
    )
}

fn update_path(
    value: &mut RuntimeValue,
    path: &[u16],
    replacement: Option<RuntimeValue>,
) -> Result<(), CharacterDialogueValueError> {
    let Some((&ordinal, tail)) = path.split_first() else {
        return Err(CharacterDialogueValueError::Field {
            field: "structured_patch",
            reason: "field path must not be empty".to_owned(),
        });
    };
    let index = usize::from(ordinal);
    match value {
        RuntimeValue::Tuple(values) => update_fixed_values(values, index, tail, replacement),
        RuntimeValue::Record(fields) => {
            let mut rebuilt = fields
                .iter()
                .map(|field| (field.name().to_owned(), field.value().clone()))
                .collect::<Vec<_>>();
            if tail.is_empty() {
                if let Some(replacement) = replacement {
                    let (_, field) = rebuilt.get_mut(index).ok_or_else(|| {
                        CharacterDialogueValueError::Field {
                            field: "structured_patch",
                            reason: format!("record field ordinal {index} is absent"),
                        }
                    })?;
                    *field = replacement;
                } else if index < rebuilt.len() {
                    rebuilt.remove(index);
                } else {
                    return Err(CharacterDialogueValueError::Field {
                        field: "structured_patch",
                        reason: format!("record field ordinal {index} is absent"),
                    });
                }
            } else {
                let (_, field) =
                    rebuilt
                        .get_mut(index)
                        .ok_or_else(|| CharacterDialogueValueError::Field {
                            field: "structured_patch",
                            reason: format!("record field ordinal {index} is absent"),
                        })?;
                update_path(field, tail, replacement)?;
            }
            *value = RuntimeValue::try_record(rebuilt).map_err(|error| {
                CharacterDialogueValueError::Field {
                    field: "structured_patch",
                    reason: error.to_string(),
                }
            })?;
            Ok(())
        }
        RuntimeValue::NominalRecord(record) => {
            let type_id = record.type_id().clone();
            let layout = record.layout();
            let mut values = record.clone().into_fields();
            update_fixed_values(&mut values, index, tail, replacement)?;
            *value = RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
                type_id, layout, values,
            ));
            Ok(())
        }
        RuntimeValue::Seq(sequence) => {
            let mut values = sequence.clone().into_values();
            update_values(&mut values, index, tail, replacement)?;
            *sequence = RuntimeSeq::values(values);
            Ok(())
        }
        _ => Err(CharacterDialogueValueError::Field {
            field: "structured_patch",
            reason: "field path traverses a non-structured value".to_owned(),
        }),
    }
}

fn update_fixed_values(
    values: &mut [RuntimeValue],
    index: usize,
    tail: &[u16],
    replacement: Option<RuntimeValue>,
) -> Result<(), CharacterDialogueValueError> {
    let value = values
        .get_mut(index)
        .ok_or_else(|| CharacterDialogueValueError::Field {
            field: "structured_patch",
            reason: format!("nominal field ordinal {index} is absent"),
        })?;
    if tail.is_empty() {
        *value = match replacement {
            Some(replacement) => replacement,
            None => empty_runtime_value(value)?,
        };
        return Ok(());
    }
    update_path(value, tail, replacement)
}

fn update_values(
    values: &mut Vec<RuntimeValue>,
    index: usize,
    tail: &[u16],
    replacement: Option<RuntimeValue>,
) -> Result<(), CharacterDialogueValueError> {
    if tail.is_empty() {
        if let Some(replacement) = replacement {
            if index == values.len() {
                values.push(replacement);
            } else if let Some(value) = values.get_mut(index) {
                *value = replacement;
            } else {
                return Err(CharacterDialogueValueError::Field {
                    field: "structured_patch",
                    reason: format!("field ordinal {index} is not contiguous"),
                });
            }
        } else if index < values.len() {
            values.remove(index);
        } else {
            return Err(CharacterDialogueValueError::Field {
                field: "structured_patch",
                reason: format!("field ordinal {index} is absent"),
            });
        }
        return Ok(());
    }
    let value = values
        .get_mut(index)
        .ok_or_else(|| CharacterDialogueValueError::Field {
            field: "structured_patch",
            reason: format!("field ordinal {index} is absent"),
        })?;
    update_path(value, tail, replacement)
}

fn validate_assignment_paths<'a>(
    paths: impl Iterator<Item = &'a RuntimeFieldPath>,
) -> Result<(), CharacterDialogueValueError> {
    let paths = paths.collect::<Vec<_>>();
    if paths
        .iter()
        .enumerate()
        .any(|(index, path)| paths[index + 1..].iter().any(|other| path.overlaps(other)))
    {
        return Err(CharacterDialogueValueError::OverlappingStructuredPaths);
    }
    Ok(())
}

fn standard_dialogue_view() -> ViewId {
    ViewId::try_new_engine_owned("std.view.dialogue")
        .expect("reserved standard dialogue View identity is valid")
}
