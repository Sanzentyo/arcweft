mod dialogue_action;
mod dialogue_surface;

pub(crate) use dialogue_surface::{
    ScannedContentApplicationBody, ScannedContentApplicationCallee, ScannedDialogueRuby,
    ScannedDialogueSurface, ScannedDialogueSurfaceKind, scan_dialogue_surface,
};

pub use dialogue_action::{
    MAX_DIALOGUE_ACTION_ARGUMENTS, MAX_DIALOGUE_ACTION_ARGUMENTS_TOTAL,
    MAX_DIALOGUE_ACTION_HEAD_BYTES, MAX_DIALOGUE_ACTION_KEY_BYTES, MAX_DIALOGUE_ACTION_VALUE_BYTES,
    MAX_DIALOGUE_POINT_ACTIONS, RichTextArgumentIssue,
};
pub(crate) use dialogue_action::{
    ScannedDialogueActionArgument, ScannedDialogueActionArgumentParts,
    ScannedDialogueActionArgumentValue, ScannedDialogueActionArguments,
    find_dialogue_bracket_boundary, is_dialogue_action_whitespace,
    scan_dialogue_action_argument_value_if_valid, scan_dialogue_action_arguments,
    trim_dialogue_action_whitespace, utf8_boundary_at_or_before,
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
    DialogueActionArgumentUnterminatedQuote,
    DialogueActionArgumentInvalidEscape,
    DialogueActionArgumentEmptyKey,
    DialogueActionArgumentInvalidKey,
    DialogueActionArgumentMissingValue,
    DialogueActionHeadTooLong,
    DialogueActionArgumentTooMany,
    DialogueActionArgumentKeyTooLong,
    DialogueActionArgumentValueTooLong,
    DialoguePointActionLimit,
    DialogueActionArgumentLimit,
}

impl DialogueTextDiagnosticCode {
    /// Stable diagnostic code used by compiler and tooling layers.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DialogueActionArgumentUnterminatedQuote => {
                "syntax.rich_text.argument.unterminated_quote"
            }
            Self::DialogueActionArgumentInvalidEscape => "syntax.rich_text.argument.invalid_escape",
            Self::DialogueActionArgumentEmptyKey => "syntax.rich_text.argument.empty_key",
            Self::DialogueActionArgumentInvalidKey => "syntax.rich_text.argument.invalid_key",
            Self::DialogueActionArgumentMissingValue => "syntax.rich_text.argument.missing_value",
            Self::DialogueActionHeadTooLong => "syntax.rich_text.point_action.head_too_long",
            Self::DialogueActionArgumentTooMany => "syntax.rich_text.argument.too_many",
            Self::DialogueActionArgumentKeyTooLong => "syntax.rich_text.argument.key_too_long",
            Self::DialogueActionArgumentValueTooLong => "syntax.rich_text.argument.value_too_long",
            Self::DialoguePointActionLimit => "syntax.rich_text.content.point_action_limit",
            Self::DialogueActionArgumentLimit => "syntax.rich_text.content.argument_limit",
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
    use super::find_dialogue_bracket_boundary;

    #[test]
    fn quoted_closing_brackets_do_not_end_dialogue_bracket_heads() {
        let source = "[signal .warning note=\"contains ] safely\"]text";
        let boundary =
            find_dialogue_bracket_boundary(source, 0, source.len()).expect("point-action boundary");
        assert_eq!(&source[boundary.close()..boundary.end()], "]");
        assert_eq!(
            &source[..boundary.end()],
            "[signal .warning note=\"contains ] safely\"]"
        );
        assert_eq!(boundary.unterminated_quote_start(), None);
    }
}
