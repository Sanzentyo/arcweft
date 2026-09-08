//! Typed zero-width dialogue point-action payloads and recovery issues.
//!
//! Body-bearing presentation operations are content-call identities in the
//! attached application. This module therefore contains only the small value
//! vocabularies shared by point actions and their diagnostics; it has no close
//! marker inventory.

use arcweft_lang_syntax::expressions::{SyntaxRichTextHostEvent, SyntaxRichTextIssue};
use arcweft_lang_syntax::text::RichTextArgumentIssue;

/// Typed zero-width dialogue control admitted by bracket syntax.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialogueControl {
    Page,
    LineWait,
    HardBreak,
    TimedWait,
    Clear,
    Reset,
    Speed,
}

/// Host event admitted by a zero-width dialogue action.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextHostEvent {
    Voice,
    Face,
    Pose,
    Show,
    Hide,
    Move,
    Scale,
    Rotate,
    Animation,
    StageShake,
    TimedCue,
    Call,
    Signal,
}

impl From<SyntaxRichTextHostEvent> for HirRichTextHostEvent {
    fn from(value: SyntaxRichTextHostEvent) -> Self {
        match value {
            SyntaxRichTextHostEvent::Voice => Self::Voice,
            SyntaxRichTextHostEvent::Face => Self::Face,
            SyntaxRichTextHostEvent::Pose => Self::Pose,
            SyntaxRichTextHostEvent::Show => Self::Show,
            SyntaxRichTextHostEvent::Hide => Self::Hide,
            SyntaxRichTextHostEvent::Move => Self::Move,
            SyntaxRichTextHostEvent::Scale => Self::Scale,
            SyntaxRichTextHostEvent::Rotate => Self::Rotate,
            SyntaxRichTextHostEvent::Animation => Self::Animation,
            SyntaxRichTextHostEvent::StageShake => Self::StageShake,
            SyntaxRichTextHostEvent::TimedCue => Self::TimedCue,
            SyntaxRichTextHostEvent::Call => Self::Call,
            SyntaxRichTextHostEvent::Signal => Self::Signal,
        }
    }
}

/// Opaque decoded point-action argument value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRichTextValue(Box<str>);

impl HirRichTextValue {
    pub(crate) const fn new(value: Box<str>) -> Self {
        Self(value)
    }

    /// Returns decoded UTF-8 without quote or escape spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Exact malformed point-action argument families.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextArgumentIssue {
    EmptyKey,
    InvalidKey,
    InvalidEscape,
    UnterminatedQuote,
    KeyTooLong,
    ValueTooLong,
    MissingValue,
    DecoderFailure,
}

impl From<RichTextArgumentIssue> for HirRichTextArgumentIssue {
    fn from(value: RichTextArgumentIssue) -> Self {
        match value {
            RichTextArgumentIssue::EmptyKey => Self::EmptyKey,
            RichTextArgumentIssue::InvalidKey => Self::InvalidKey,
            RichTextArgumentIssue::InvalidEscape => Self::InvalidEscape,
            RichTextArgumentIssue::UnterminatedQuote => Self::UnterminatedQuote,
            RichTextArgumentIssue::KeyTooLong => Self::KeyTooLong,
            RichTextArgumentIssue::ValueTooLong => Self::ValueTooLong,
            RichTextArgumentIssue::MissingValue => Self::MissingValue,
            RichTextArgumentIssue::DecoderFailure => Self::DecoderFailure,
        }
    }
}

/// Typed point-action and content-call recovery issue vocabulary.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRichTextIssue {
    InvalidPayload,
    ForeignNestedExpression,
    Argument(HirRichTextArgumentIssue),
}

impl From<SyntaxRichTextIssue> for HirRichTextIssue {
    fn from(value: SyntaxRichTextIssue) -> Self {
        match value {
            SyntaxRichTextIssue::InvalidPayload => Self::InvalidPayload,
            SyntaxRichTextIssue::ForeignNestedExpression => Self::ForeignNestedExpression,
            SyntaxRichTextIssue::Argument(issue) => Self::Argument(issue.into()),
        }
    }
}
