mod dialogue_surface;
mod rich_text_tag;

pub(crate) use dialogue_surface::{
    ScannedDialogueRuby, ScannedDialogueSurface, ScannedDialogueSurfaceKind, ScannedDialogueText,
    ScannedInlineStyle, ScannedInlineStyleKind, scan_dialogue_surface,
};

pub use rich_text_tag::{
    MAX_RICH_TEXT_CONTENT_ARGUMENTS, MAX_RICH_TEXT_CONTENT_TAGS, MAX_RICH_TEXT_TAG_ARGUMENTS,
    MAX_RICH_TEXT_TAG_BODY_BYTES, MAX_RICH_TEXT_TAG_KEY_BYTES, MAX_RICH_TEXT_TAG_VALUE_BYTES,
    RichTextArgumentIssue,
};
pub(crate) use rich_text_tag::{
    ScannedTagArgValue, ScannedTagArgument, ScannedTagArgumentParts, ScannedTagArguments,
    find_dialogue_tag_boundary_before, is_rich_text_whitespace, scan_tag_arguments,
    trim_rich_text_whitespace, utf8_boundary_at_or_before,
};

use crate::ast::common::TextRange;

/// A recoverable diagnostic produced while tokenizing dialogue text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueTextDiagnostic {
    code: DialogueTextDiagnosticCode,
    range: TextRange,
    message: String,
    recovery: String,
}

/// Stable syntax diagnostic identity for dialogue-text parsing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogueTextDiagnosticCode {
    RichTextAttributeUnterminatedQuote,
    RichTextAttributeInvalidEscape,
    RichTextAttributeEmptyKey,
    RichTextAttributeInvalidKey,
    RichTextAttributeMissingValue,
    RichTextTagBodyTooLong,
    RichTextAttributeTooMany,
    RichTextAttributeKeyTooLong,
    RichTextAttributeValueTooLong,
    RichTextContentTagLimit,
    RichTextContentArgumentLimit,
}

impl DialogueTextDiagnosticCode {
    /// Stable diagnostic code used by compiler and tooling layers.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RichTextAttributeUnterminatedQuote => {
                "syntax.rich_text.attribute.unterminated_quote"
            }
            Self::RichTextAttributeInvalidEscape => "syntax.rich_text.attribute.invalid_escape",
            Self::RichTextAttributeEmptyKey => "syntax.rich_text.attribute.empty_key",
            Self::RichTextAttributeInvalidKey => "syntax.rich_text.attribute.invalid_key",
            Self::RichTextAttributeMissingValue => "syntax.rich_text.attribute.missing_value",
            Self::RichTextTagBodyTooLong => "syntax.rich_text.tag.body_too_long",
            Self::RichTextAttributeTooMany => "syntax.rich_text.attribute.too_many",
            Self::RichTextAttributeKeyTooLong => "syntax.rich_text.attribute.key_too_long",
            Self::RichTextAttributeValueTooLong => "syntax.rich_text.attribute.value_too_long",
            Self::RichTextContentTagLimit => "syntax.rich_text.content.tag_limit",
            Self::RichTextContentArgumentLimit => "syntax.rich_text.content.argument_limit",
        }
    }
}

impl DialogueTextDiagnostic {
    fn with_code(
        code: DialogueTextDiagnosticCode,
        range: TextRange,
        message: impl Into<String>,
        recovery: impl Into<String>,
    ) -> Self {
        Self {
            code,
            range,
            message: message.into(),
            recovery: recovery.into(),
        }
    }

    /// Stable structured diagnostic identity.
    pub const fn code(&self) -> DialogueTextDiagnosticCode {
        self.code
    }

    /// Byte range relative to the dialogue source passed to the tokenizer.
    pub const fn range(&self) -> &TextRange {
        &self.range
    }

    /// Human-readable diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Suggested local recovery.
    pub fn recovery(&self) -> &str {
        &self.recovery
    }
}

#[cfg(test)]
mod tests {
    use super::find_dialogue_tag_boundary_before;

    #[test]
    fn quoted_closing_brackets_do_not_end_dialogue_tags() {
        let source = "[effect .warning note=\"contains ] safely\"]text[/effect]";
        let boundary =
            find_dialogue_tag_boundary_before(source, 0, source.len()).expect("tag boundary");
        assert_eq!(&source[boundary.close()..boundary.end()], "]");
        assert_eq!(
            &source[..boundary.end()],
            "[effect .warning note=\"contains ] safely\"]"
        );
        assert_eq!(boundary.unterminated_quote_start(), None);
    }
}
