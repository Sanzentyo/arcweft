//! Transport-neutral authored and resolved-frame text model.
//!
//! Resolution, playback validation, and renderer-specific projection live in
//! `arcweft-render-text`; this crate owns the shared data passed across those
//! boundaries.

pub mod catalog;
pub mod character_dialogue_style;
pub mod content;
pub mod frame;
pub mod playback;
pub mod prepared_context;
pub mod projected_range;
pub mod reveal;
pub mod rich_effects;
pub mod rich_text;
pub mod style;

pub use catalog::{
    DialogueContentCatalog, DialogueContentCatalogError, DialogueContentSpec,
    DialogueContentSpecError, DialoguePresentationSnapshot, RichTextAssignOp, RichTextCascadeLayer,
    RichTextSettingSource, RichTextSourceRange, RichTextStyleContribution,
};
pub use character_dialogue_style::{
    CharacterDialogueRichTextProjectionError, project_character_dialogue_rich_text_properties,
};
pub use content::{
    DialogueContentFragmentCatalog, DialogueContentFragmentCatalogError,
    DialogueContentFragmentTemplate, DialogueContentFragmentTemplateError,
    DialogueContentMaterializationError, DialogueContentMaterializer, DialogueContentNodeOrigin,
    DialogueContentTemplateEffect, DialogueContentTemplateMark, DialogueContentTemplateSlot,
    MaterializedDialogueContent, MaterializedDialogueEffect, MaterializedDialogueMark,
};
pub use frame::{
    CharacterDialoguePresentationConfig, CharacterDialoguePresentationConfigSnapshot,
    DialoguePresentationCharacter, InlineTextFailureSnapshot, LineDisplayFrame,
    LineDisplayFrameSnapshot, LineDisplayStageSnapshot, ResolvedRichTextNode,
    RichTextControlMarker, RichTextDisplayMap, RichTextHostEventMarker, RichTextNodeCount,
    RichTextNodeCountOverflow, RichTextNodeIndex, RichTextNodeIndexOverflow, RichTextNodeRange,
    RichTextRange, RichTextRubyAnnotation, RichTextTextRun, RichTextTextRunRange,
    RichTextTextSource,
};
pub use playback::{
    LineDisplayFrameValidationError, LineDisplayStage, LineDisplayStageEnd,
    LineDisplayStageProjection,
};
pub use prepared_context::{
    DialoguePreparedTextRole, PreparedDisplayStageContext, PreparedTextContext,
    PreparedTextContextError, PreparedTextDocumentContext, PreparedTextOwnerKind,
};
pub use projected_range::{
    ProjectedTextCoordinate, ProjectedTextRangeDomain, ProjectedTextRangeError,
};
pub use reveal::{
    DialogueRevealElapsed, DialogueRevealEvaluation, DialogueRevealPolicy, evaluate_dialogue_reveal,
};
pub use rich_effects::{
    Milli, RichTextAngle, RichTextInlineDirection, RichTextJlreqStrictness, RichTextLayout,
    RichTextObjectProxy, RichTextObjectProxyDeclaration, RichTextParam, RichTextPresentation,
    RichTextRubyPosition, RichTextTextProxyField, RichTextTextProxyFieldKind,
    RichTextTextProxyFieldSchema, RichTextTextProxyLength, RichTextTextProxyLengthUnit,
    RichTextTextProxyScalar, RichTextTextProxySchema, RichTextTransform, RichTextTransformOrigin,
    RichTextVec2, RichTextVerticalLatinMode, RichTextWritingMode,
};
pub use rich_text::{
    DialogueHostEvent, DialogueVoiceSource, RichTextControl, RichTextDocument, RichTextNode,
};
pub use style::{
    RichTextColor, RichTextFontFamily, RichTextPresentationStyle, RichTextStyle,
    presentation_from_styles,
};
