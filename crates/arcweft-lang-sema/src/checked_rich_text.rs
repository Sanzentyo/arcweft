//! Typed `RichText` validation over the final attached HIR.
//!
//! This module is the sole semantic consumer of `RichText` tag arguments. It
//! resolves owner-owned schemas, applies defaults only to absent values, and
//! publishes renderer-neutral typed fields. It never reads source text,
//! reconstructs a tag from a range, or retains a raw/unknown executable value.

mod checker;
mod diagnostic;
mod model;
mod prepared;
mod value;

#[cfg(test)]
mod tests;

pub(crate) use checker::RichTextAttributeChecker;
pub use diagnostic::{
    RichTextAttributeDiagnostic, RichTextDiagnosticCode, RichTextDiagnosticOwner,
    RichTextFailureEffect, RichTextRelatedSite,
};
pub use model::{
    CheckedDialogueContent, CheckedDialogueControl, CheckedDialogueHostEvent, CheckedDialogueMark,
    CheckedDialogueToken, CheckedDirectStyleSpan, CheckedField, CheckedFieldOrigin,
    CheckedLayoutSpan, CheckedObjectSpan, CheckedOwnerFields, CheckedRichTextAction,
    CheckedRichTextClose, CheckedRichTextOwner, CheckedRichTextProperty, CheckedRichTextReport,
    CheckedRichTextTag, CheckedStyleSpan, CheckedTransformSpan, CheckedVoiceSource,
    RichTextDefaultId,
};
pub(crate) use prepared::{
    PreparedCheckedDialogueContent, PreparedCheckedDialogueMark,
    PreparedCheckedDialogueMarkCatalog, PreparedCheckedDialogueToken,
    PreparedCheckedRichTextAction, PreparedCheckedRichTextCheck, PreparedCheckedRichTextReport,
    PreparedCheckedRichTextTag,
};
pub use value::{
    CheckedAngle, CheckedColor, CheckedDuration, CheckedEnumValue, CheckedLength,
    CheckedRichTextValue, CheckedVec2, LengthUnit, Milli, RatioMilli, Seed32,
};
