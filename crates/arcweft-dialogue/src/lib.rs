mod character_dialogue;
pub mod character_presentation;
mod inline_failure;
mod presentation_profile;
mod presentation_revision;
pub mod rich_text;

pub use character_dialogue::{
    CharacterDialogue, CharacterDialogueCharacterType, CharacterDialogueCleanupValue,
    CharacterDialogueConfig, CharacterDialogueContractIdentity, CharacterDialogueCustomFieldId,
    CharacterDialogueCustomValue, CharacterDialogueFocusValue, CharacterDialogueHookValue,
    CharacterDialogueLimits, CharacterDialoguePatch, CharacterDialoguePortraitValue,
    CharacterDialogueRichTextValue, CharacterDialogueRuntimeCustomFieldCatalog,
    CharacterDialogueRuntimeCustomFieldDescriptor, CharacterDialogueRuntimeRole,
    CharacterDialogueRuntimeSchema, CharacterDialogueStageValue, CharacterDialogueStyleValue,
    CharacterDialogueType, CharacterDialogueTypedValue, CharacterDialogueValue,
    CharacterDialogueValueError, CharacterDialogueVoice, CharacterDialogueVoiceId,
    DialogueLocaleId, PRODUCTION_CHARACTER_DIALOGUE_LIMITS, PatchField, RuntimeFieldPath,
    StructuredPatch,
};
pub use inline_failure::{
    FallbackStylePolicy, InlineFailurePolicy, InlineFailureSelection, InlineFallback,
    InlineTextFailure,
};
pub use presentation_profile::DialoguePresentationProfile;
pub use presentation_revision::DialogueProfileRevision;

#[cfg(test)]
mod tests;
