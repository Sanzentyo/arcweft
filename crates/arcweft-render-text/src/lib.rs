//! Sans I/O text source resolution for Arcweft players.
//!
//! The root is a deliberate facade. Responsibility modules own the authored
//! model, dialogue resolution, playback projection, presentation metadata, and
//! canonical post-resolution document respectively.

pub mod catalog;
pub mod frame;
pub mod playback;
pub mod resolved_document;
pub mod rich_effects;
pub mod rich_text;
pub mod style;

pub use arcweft_presentation::fx::FxApplication;

mod resolve_frame;

pub use catalog::{
    LineDisplayArg, LineDisplayCatalog, LineDisplaySpec, RichTextAssignOp, RichTextCascadeLayer,
    RichTextSettingSource, RichTextSourceRange, RichTextStyleContribution,
};
pub use frame::{
    LineDisplayFrame, RichTextControlMarker, RichTextDisplayMap, RichTextHostEventMarker,
    RichTextRange, RichTextRubyAnnotation, RichTextTextRun, RichTextTextSource,
};
pub use playback::{LineDisplayFrameValidationError, LineDisplayStage, LineDisplayStageEnd};
pub use resolve_frame::{LineDisplayError, RuntimeLineContext};
pub use resolved_document::{
    LanguageTag, ResolvedTextDocument, ResolvedTextRuby, ResolvedTextRun, ResolvedTextRunSource,
    ResolvedTextStyle, TextColor, TextDocumentRevision, TextFontFamily, TextResolveError,
    TextSlant, TextStyleCascade, TextWeight,
};
pub use rich_effects::{
    Milli, RichTextAngle, RichTextEffectDescriptor, RichTextEffectPhase, RichTextEffectTarget,
    RichTextInlineDirection, RichTextJlreqStrictness, RichTextLayout, RichTextObjectProxy,
    RichTextObjectProxyDeclaration, RichTextParam, RichTextPresentation, RichTextRubyPosition,
    RichTextShaderRef, RichTextStateScope, RichTextTransform, RichTextTransformOrigin,
    RichTextVec2, RichTextVerticalLatinMode, RichTextWritingMode, parse_decimal_milli,
    parse_milli_token, parse_z_index_token,
};
pub use rich_text::{
    DialogueHostEvent, FallbackStylePolicy, InlineFailurePolicy, InlineFallback, InlineTextFailure,
    RichTextControl, RichTextDocument, RichTextNode,
};
pub use style::{
    RichTextColor, RichTextFontFamily, RichTextPresentationStyle, RichTextStyle,
    presentation_from_styles,
};
