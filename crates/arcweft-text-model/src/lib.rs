//! Transport-neutral authored and resolved-frame text model.
//!
//! Resolution, playback validation, and renderer-specific projection live in
//! `arcweft-render-text`; this crate owns the shared data passed across those
//! boundaries.

pub mod catalog;
pub mod frame;
pub mod playback;
pub mod reveal;
pub mod rich_effects;
pub mod rich_text;
pub mod style;

pub use catalog::{
    DialogueContentCatalog, DialogueContentCatalogError, DialogueContentSpec,
    DialoguePresentationSnapshot, RichTextAssignOp, RichTextCascadeLayer, RichTextSettingSource,
    RichTextSourceRange, RichTextStyleContribution,
};
pub use frame::{
    CharacterDialoguePresentationConfig, DialoguePresentationCharacter, LineDisplayFrame,
    ResolvedRichTextNode, RichTextControlMarker, RichTextDisplayMap, RichTextHostEventMarker,
    RichTextRange, RichTextRubyAnnotation, RichTextTextRun, RichTextTextSource,
};
pub use playback::{LineDisplayFrameValidationError, LineDisplayStage, LineDisplayStageEnd};
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
    RichTextColor, RichTextFontFamily, RichTextPresentationStyle, RichTextSpanKind, RichTextStyle,
    presentation_from_styles,
};
