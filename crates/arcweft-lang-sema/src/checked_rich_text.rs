//! Typed dialogue-content validation over the final attached HIR.
//!
//! Point actions resolve against dialogue-owned schemas. Body-bearing
//! presentation operations are attached Content callable applications, so no
//! delimiter, close/open pairing, or source-string reconstruction appears in
//! this semantic boundary.

mod checker;
mod diagnostic;
mod model;
mod prepared;
mod value;

pub(crate) use checker::RichTextContentChecker;
pub use diagnostic::{
    RichTextDiagnostic, RichTextDiagnosticCode, RichTextDiagnosticOwner, RichTextFailureEffect,
    RichTextRelatedSite,
};
pub use model::{
    CheckedAttachedContentArgument, CheckedContentApplicationId, CheckedContentApplicationSite,
    CheckedContentEmission, CheckedContentInsertion, CheckedContentModifier,
    CheckedContentParameter, CheckedContentRuby, CheckedContentValueSource, CheckedDialogueContent,
    CheckedDialogueControl, CheckedDialogueHostEvent, CheckedDialogueMark, CheckedDialogueToken,
    CheckedField, CheckedFieldOrigin, CheckedObjectDepth, CheckedOwnerFields, CheckedRawLiteral,
    CheckedRichTextAction, CheckedRichTextOwner, CheckedRichTextProperty, CheckedRichTextReport,
    CheckedVoiceSource, RichTextDefaultId,
};
pub(crate) use prepared::{
    PreparedCheckedContentCatalog, PreparedCheckedDialogueContent, PreparedCheckedDialogueMark,
    PreparedCheckedDialogueMarkCatalog, PreparedCheckedDialogueToken,
    PreparedCheckedRichTextAction, PreparedCheckedRichTextCheck, PreparedCheckedRichTextReport,
    PreparedContentApplicationRef,
};
pub(crate) use value::parse_color;
pub use value::{
    CheckedAngle, CheckedColor, CheckedDuration, CheckedLength, CheckedRichTextValue, CheckedVec2,
    LengthUnit, Milli, RatioMilli, Seed32,
};
