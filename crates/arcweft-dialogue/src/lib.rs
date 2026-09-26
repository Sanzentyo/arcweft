mod character_dialogue;
pub mod character_presentation;
mod inline_failure;
mod presentation_profile;
mod presentation_revision;
pub mod rich_text;

pub use character_dialogue::{
    CharacterDialogue, CharacterDialogueCharacterDeclaration, CharacterDialogueCharacterType,
    CharacterDialogueCleanupValue, CharacterDialogueConfig, CharacterDialogueContractIdentity,
    CharacterDialogueCustomFieldId, CharacterDialogueCustomValue, CharacterDialogueFocusValue,
    CharacterDialogueGenerationBindingError, CharacterDialogueGenerationDeclaration,
    CharacterDialogueGenerationDeclarationCodecError, CharacterDialogueGenerationDeclarationError,
    CharacterDialogueHookValue, CharacterDialogueLimits, CharacterDialoguePatch,
    CharacterDialoguePolicyCaseSpec, CharacterDialoguePolicyTypeGraph,
    CharacterDialoguePolicyTypeSchema, CharacterDialoguePolicyVariantOwner,
    CharacterDialoguePortraitValue, CharacterDialoguePresentationContract,
    CharacterDialogueRichTextColor, CharacterDialogueRichTextProperties,
    CharacterDialogueRichTextProperty, CharacterDialogueRichTextPropertyValue,
    CharacterDialogueRichTextValue, CharacterDialogueRolePayloadCodec,
    CharacterDialogueRolePayloadSchema, CharacterDialogueRuntimeCustomFieldCatalog,
    CharacterDialogueRuntimeCustomFieldDescriptor, CharacterDialogueRuntimeDefault,
    CharacterDialogueRuntimeDefaultCatalog, CharacterDialogueRuntimeExternalCallBackend,
    CharacterDialogueRuntimeRole, CharacterDialogueRuntimeRoleBody,
    CharacterDialogueRuntimeRoleType, CharacterDialogueRuntimeRoleTypes,
    CharacterDialogueRuntimeSchema, CharacterDialogueStageValue, CharacterDialogueStyleValue,
    CharacterDialogueType, CharacterDialogueTypeReference, CharacterDialogueTypeReferenceMapError,
    CharacterDialogueTypedValue, CharacterDialogueValue, CharacterDialogueValueError,
    CharacterDialogueVisualManifestEvidence, CharacterDialogueVisualType, CharacterDialogueVoice,
    CharacterDialogueVoiceId, DialogueLocaleId, PRODUCTION_CHARACTER_DIALOGUE_LIMITS, PatchField,
    RuntimeFieldPath, StructuredPatch,
};
pub use inline_failure::{
    FallbackStylePolicy, InlineFailurePolicy, InlineFailurePolicyDecodeError,
    InlineFailureSelection, InlineFallback, InlineTextFailure,
};
pub use presentation_profile::DialoguePresentationProfile;
pub use presentation_revision::DialogueProfileRevision;

#[cfg(test)]
mod tests;
