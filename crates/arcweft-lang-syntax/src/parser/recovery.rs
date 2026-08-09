use arcweft_source::{
    Diagnostic, DiagnosticApplicability, DiagnosticLabel, DiagnosticSeverity, DiagnosticSuggestion,
    SourceDocument, SourceEdit, SourceRange,
};
use thiserror::Error;

use crate::ast::common::TextRange;

/// Closed discriminator for diagnostics emitted by the repository-owned parser.
///
/// Stable transport spellings are exposed through [`Self::code`]. The enum
/// itself is not a serialized wire format.
///
/// Parser kinds are selected through typed parser APIs; transport code strings
/// cannot be converted back into this owner.
///
/// ```compile_fail
/// use arcweft_lang_syntax::parser::recovery::ParseErrorKind;
///
/// let _: ParseErrorKind = "syntax.assert.unknown_mode".parse().unwrap();
/// ```
///
/// ```compile_fail
/// use arcweft_lang_syntax::parser::recovery::ParseErrorKind;
///
/// let _: ParseErrorKind = ParseErrorKind::try_from("syntax.assert.unknown_mode").unwrap();
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ParseErrorKind {
    /// Ordinary parser recovery without a dedicated diagnostic family.
    Generic,
    /// Expression prefix operators exceed the inclusive parser depth limit.
    ExpressionPrefixDepthLimit,
    /// The assertion mode is not one of the accepted closed modes.
    AssertionUnknownMode,
    /// An assertion argument or its surrounding syntax is malformed.
    AssertionInvalidArgument,
    /// An assertion argument list has no closing parenthesis.
    AssertionUnclosedArguments,
    /// An assertion has no conditions.
    AssertionEmptyConditions,
    /// An assertion exceeds the checked condition limit.
    AssertionTooManyConditions,
    /// An entry declaration omits its explicit entry kind.
    EntryMissingKind,
    /// An entry declaration omits its canonical public ID.
    EntryMissingId,
    /// An entry declaration ID does not belong to the `entry` family.
    EntryIdFamily,
    /// An entry declaration head contains trailing syntax.
    EntryTrailingHead,
    /// An entry role binding is malformed.
    EntryRoleBinding,
    /// An entry role binding omits its value.
    EntryRoleValue,
    /// An entry callable role contains an invalid symbol path.
    EntryRolePath,
    /// A nominal declaration contains an invalid generic parameter list.
    NominalInvalidGenericParameters,
    /// The proof-only `verify.trusted` attribute is attached to another item.
    ProofTrustedNotProof,
    /// A proof carries more than one `verify.trusted` attribute.
    ProofTrustedDuplicate,
    /// A trusted proof does not declare its required `reason` argument.
    ProofTrustedReasonMissing,
    /// A trusted proof declares its `reason` argument more than once.
    ProofTrustedReasonDuplicate,
    /// A trusted proof reason is not a string literal.
    ProofTrustedReasonNotString,
    /// A decoded trusted proof reason is empty or Unicode whitespace only.
    ProofTrustedReasonEmpty,
    /// A trusted proof attribute contains an unknown named argument.
    ProofTrustedUnknownArgument,
    /// A trusted proof attribute contains a positional argument.
    ProofTrustedPositionalArgument,
    /// An inline Style patch contains a selector rule.
    StyleInlineSelectorNotSupported,
    /// A named Style rule contains a malformed nested selector.
    StyleMalformedSelector,
    /// An environment guard is missing its opening parenthesis.
    StyleEnvironmentExpectedOpenParen,
    /// An environment clause is missing a supported field.
    StyleEnvironmentExpectedField,
    /// An environment clause is missing a supported comparison.
    StyleEnvironmentExpectedComparison,
    /// An environment clause is missing a value.
    StyleEnvironmentExpectedValue,
    /// Environment clauses are not separated or terminated correctly.
    StyleEnvironmentExpectedCommaOrCloseParen,
    /// An environment body is missing or does not close.
    StyleEnvironmentExpectedOpenBrace,
    /// An environment condition has no closing parenthesis.
    StyleEnvironmentUnterminatedCondition,
    /// An environment value is outside the closed grammar.
    StyleEnvironmentUnsupportedValue,
    /// An environment body contains a sheet-owned token declaration.
    StyleEnvironmentTokenNotAllowed,
    /// A View part export occurs after the leading export block.
    ViewExportPartMisplaced,
    /// A View expression contains more than one part modifier.
    ViewDuplicatePartModifier,
    /// A View part export omits the `part` keyword.
    ViewExportPartMissingPart,
    /// A View part export contains more than one `as` keyword.
    ViewExportPartDuplicateAs,
    /// A View part export contains trailing syntax.
    ViewExportPartTrailingSyntax,
    /// A View part export omits its local name.
    ViewExportPartMissingLocal,
    /// A View part export contains an invalid local name.
    ViewExportPartInvalidLocalName,
    /// A View part export omits its `as` keyword.
    ViewExportPartMissingAs,
    /// A View part export omits its public name.
    ViewExportPartMissingPublic,
    /// A View part export contains an invalid public name.
    ViewExportPartInvalidPublicName,
    /// A View part modifier omits its local name.
    ViewPartMissingName,
    /// A View part modifier contains trailing syntax.
    ViewPartTrailingSyntax,
    /// A View part modifier contains an invalid local name.
    ViewPartInvalidLocalName,
}

impl ParseErrorKind {
    pub(crate) const ALL: [Self; 47] = [
        Self::Generic,
        Self::ExpressionPrefixDepthLimit,
        Self::AssertionUnknownMode,
        Self::AssertionInvalidArgument,
        Self::AssertionUnclosedArguments,
        Self::AssertionEmptyConditions,
        Self::AssertionTooManyConditions,
        Self::EntryMissingKind,
        Self::EntryMissingId,
        Self::EntryIdFamily,
        Self::EntryTrailingHead,
        Self::EntryRoleBinding,
        Self::EntryRoleValue,
        Self::EntryRolePath,
        Self::NominalInvalidGenericParameters,
        Self::ProofTrustedNotProof,
        Self::ProofTrustedDuplicate,
        Self::ProofTrustedReasonMissing,
        Self::ProofTrustedReasonDuplicate,
        Self::ProofTrustedReasonNotString,
        Self::ProofTrustedReasonEmpty,
        Self::ProofTrustedUnknownArgument,
        Self::ProofTrustedPositionalArgument,
        Self::StyleInlineSelectorNotSupported,
        Self::StyleMalformedSelector,
        Self::StyleEnvironmentExpectedOpenParen,
        Self::StyleEnvironmentExpectedField,
        Self::StyleEnvironmentExpectedComparison,
        Self::StyleEnvironmentExpectedValue,
        Self::StyleEnvironmentExpectedCommaOrCloseParen,
        Self::StyleEnvironmentExpectedOpenBrace,
        Self::StyleEnvironmentUnterminatedCondition,
        Self::StyleEnvironmentUnsupportedValue,
        Self::StyleEnvironmentTokenNotAllowed,
        Self::ViewExportPartMisplaced,
        Self::ViewDuplicatePartModifier,
        Self::ViewExportPartMissingPart,
        Self::ViewExportPartDuplicateAs,
        Self::ViewExportPartTrailingSyntax,
        Self::ViewExportPartMissingLocal,
        Self::ViewExportPartInvalidLocalName,
        Self::ViewExportPartMissingAs,
        Self::ViewExportPartMissingPublic,
        Self::ViewExportPartInvalidPublicName,
        Self::ViewPartMissingName,
        Self::ViewPartTrailingSyntax,
        Self::ViewPartInvalidLocalName,
    ];

    /// Stable code projected into shared diagnostics and protocol adapters.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Generic => "syntax.parse",
            Self::ExpressionPrefixDepthLimit => "syntax.expr.prefix_depth_limit",
            Self::AssertionUnknownMode => "syntax.assert.unknown_mode",
            Self::AssertionInvalidArgument => "syntax.assert.invalid_argument",
            Self::AssertionUnclosedArguments => "syntax.assert.unclosed_arguments",
            Self::AssertionEmptyConditions => "syntax.assert.empty_conditions",
            Self::AssertionTooManyConditions => "syntax.assert.too_many_conditions",
            Self::EntryMissingKind => "syntax.entry.missing_kind",
            Self::EntryMissingId => "syntax.entry.missing_id",
            Self::EntryIdFamily => "syntax.entry.id_family",
            Self::EntryTrailingHead => "syntax.entry.trailing_head",
            Self::EntryRoleBinding => "syntax.entry.role_binding",
            Self::EntryRoleValue => "syntax.entry.role_value",
            Self::EntryRolePath => "syntax.entry.role_path",
            Self::NominalInvalidGenericParameters => "syntax.nominal.invalid_generic_parameters",
            Self::ProofTrustedNotProof => "syntax.proof.trusted.not_proof",
            Self::ProofTrustedDuplicate => "syntax.proof.trusted.duplicate",
            Self::ProofTrustedReasonMissing => "syntax.proof.trusted.reason_missing",
            Self::ProofTrustedReasonDuplicate => "syntax.proof.trusted.reason_duplicate",
            Self::ProofTrustedReasonNotString => "syntax.proof.trusted.reason_not_string",
            Self::ProofTrustedReasonEmpty => "syntax.proof.trusted.reason_empty",
            Self::ProofTrustedUnknownArgument => "syntax.proof.trusted.unknown_argument",
            Self::ProofTrustedPositionalArgument => "syntax.proof.trusted.positional_argument",
            Self::StyleInlineSelectorNotSupported => "style::inline_selector_not_supported",
            Self::StyleMalformedSelector => "style::malformed_selector",
            Self::StyleEnvironmentExpectedOpenParen => {
                "syntax.parse.style_environment.expected_open_paren"
            }
            Self::StyleEnvironmentExpectedField => "syntax.parse.style_environment.expected_field",
            Self::StyleEnvironmentExpectedComparison => {
                "syntax.parse.style_environment.expected_comparison"
            }
            Self::StyleEnvironmentExpectedValue => "syntax.parse.style_environment.expected_value",
            Self::StyleEnvironmentExpectedCommaOrCloseParen => {
                "syntax.parse.style_environment.expected_comma_or_close_paren"
            }
            Self::StyleEnvironmentExpectedOpenBrace => {
                "syntax.parse.style_environment.expected_open_brace"
            }
            Self::StyleEnvironmentUnterminatedCondition => {
                "syntax.parse.style_environment.unterminated_condition"
            }
            Self::StyleEnvironmentUnsupportedValue => {
                "syntax.parse.style_environment.unsupported_value"
            }
            Self::StyleEnvironmentTokenNotAllowed => {
                "syntax.parse.style_environment.token_not_allowed"
            }
            Self::ViewExportPartMisplaced => "view::export_part_misplaced",
            Self::ViewDuplicatePartModifier => "view::duplicate_part_modifier",
            Self::ViewExportPartMissingPart => "view::export_part_missing_part",
            Self::ViewExportPartDuplicateAs => "view::export_part_duplicate_as",
            Self::ViewExportPartTrailingSyntax => "view::export_part_trailing_syntax",
            Self::ViewExportPartMissingLocal => "view::export_part_missing_local",
            Self::ViewExportPartInvalidLocalName => "view::export_part_invalid_local_name",
            Self::ViewExportPartMissingAs => "view::export_part_missing_as",
            Self::ViewExportPartMissingPublic => "view::export_part_missing_public",
            Self::ViewExportPartInvalidPublicName => "view::export_part_invalid_public_name",
            Self::ViewPartMissingName => "view::part_missing_name",
            Self::ViewPartTrailingSyntax => "view::part_trailing_syntax",
            Self::ViewPartInvalidLocalName => "view::part_invalid_local_name",
        }
    }

    /// Stable English renderer label for this parser diagnostic kind.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Generic => "Parse error",
            Self::ExpressionPrefixDepthLimit => "Expression prefix depth limit exceeded",
            Self::AssertionUnknownMode => "Unknown assertion mode",
            Self::AssertionInvalidArgument => "Invalid assertion argument",
            Self::AssertionUnclosedArguments => "Unclosed assertion argument list",
            Self::AssertionEmptyConditions => "Empty assertion condition list",
            Self::AssertionTooManyConditions => "Too many assertion conditions",
            Self::EntryMissingKind => "Missing entry kind",
            Self::EntryMissingId => "Missing entry public ID",
            Self::EntryIdFamily => "Invalid entry public ID family",
            Self::EntryTrailingHead => "Trailing syntax in entry declaration head",
            Self::EntryRoleBinding => "Malformed entry role binding",
            Self::EntryRoleValue => "Missing entry role value",
            Self::EntryRolePath => "Invalid entry role symbol path",
            Self::NominalInvalidGenericParameters => "Invalid nominal generic parameter list",
            Self::ProofTrustedNotProof => "Trusted proof attribute on a non-proof item",
            Self::ProofTrustedDuplicate => "Duplicate trusted proof attribute",
            Self::ProofTrustedReasonMissing => "Missing trusted proof reason",
            Self::ProofTrustedReasonDuplicate => "Duplicate trusted proof reason",
            Self::ProofTrustedReasonNotString => "Trusted proof reason is not a string",
            Self::ProofTrustedReasonEmpty => "Trusted proof reason is empty",
            Self::ProofTrustedUnknownArgument => "Unknown trusted proof argument",
            Self::ProofTrustedPositionalArgument => "Positional trusted proof argument",
            Self::StyleInlineSelectorNotSupported => "Selector rule in inline Style",
            Self::StyleMalformedSelector => "Malformed Style selector",
            Self::StyleEnvironmentExpectedOpenParen => "Expected environment opening parenthesis",
            Self::StyleEnvironmentExpectedField => "Expected environment field",
            Self::StyleEnvironmentExpectedComparison => "Expected environment comparison",
            Self::StyleEnvironmentExpectedValue => "Expected environment value",
            Self::StyleEnvironmentExpectedCommaOrCloseParen => {
                "Expected environment clause separator"
            }
            Self::StyleEnvironmentExpectedOpenBrace => "Expected environment body opening brace",
            Self::StyleEnvironmentUnterminatedCondition => "Unterminated environment condition",
            Self::StyleEnvironmentUnsupportedValue => "Unsupported environment value",
            Self::StyleEnvironmentTokenNotAllowed => "Style token in environment body",
            Self::ViewExportPartMisplaced => "Misplaced View part export",
            Self::ViewDuplicatePartModifier => "Duplicate View part modifier",
            Self::ViewExportPartMissingPart => "Missing `part` keyword in View export",
            Self::ViewExportPartDuplicateAs => "Duplicate `as` keyword in View part export",
            Self::ViewExportPartTrailingSyntax => "Trailing syntax in View part export",
            Self::ViewExportPartMissingLocal => "Missing local View part name",
            Self::ViewExportPartInvalidLocalName => "Invalid local View part name",
            Self::ViewExportPartMissingAs => "Missing `as` keyword in View part export",
            Self::ViewExportPartMissingPublic => "Missing public View part name",
            Self::ViewExportPartInvalidPublicName => "Invalid public View part name",
            Self::ViewPartMissingName => "Missing View part modifier name",
            Self::ViewPartTrailingSyntax => "Trailing syntax in View part modifier",
            Self::ViewPartInvalidLocalName => "Invalid View part modifier name",
        }
    }
}

const _: [(); ParseErrorKind::ALL.len()] = [(); 47];

/// Syntax-level parse error with expected tokens and recovery suggestions.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct ParseError {
    kind: ParseErrorKind,
    range: TextRange,
    related: Vec<ParseRelatedRange>,
    expected: Vec<String>,
    found: Option<String>,
    message: String,
    recovery: Vec<RecoverySuggestion>,
}

/// Secondary source range attached to one syntax diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseRelatedRange {
    range: TextRange,
    message: Option<String>,
}

/// Suggested local edit or strategy for recovering from an error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoverySuggestion {
    pub(crate) message: String,
    pub(crate) edits: Vec<RecoveryEdit>,
    pub(crate) applicability: DiagnosticApplicability,
}

/// Suggested parse-recovery source edit relative to the parsed source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryEdit {
    range: TextRange,
    replacement: String,
}

impl ParseError {
    #[cfg(test)]
    pub(crate) fn new_with_kind(
        kind: ParseErrorKind,
        range: TextRange,
        expected: Vec<String>,
        found: Option<String>,
        message: String,
        recovery: Vec<RecoverySuggestion>,
    ) -> Self {
        Self {
            kind,
            range,
            related: Vec::new(),
            expected,
            found,
            message,
            recovery,
        }
    }

    /// Repository-owned parser diagnostic discriminator.
    #[must_use]
    pub const fn kind(&self) -> ParseErrorKind {
        self.kind
    }

    /// Stable diagnostic code used by compiler and tooling integrations.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.kind.code()
    }

    /// Stable English renderer label for this diagnostic kind.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        self.kind.label()
    }

    /// Error byte range.
    pub const fn range(&self) -> &TextRange {
        &self.range
    }

    /// Secondary ranges that explain earlier or otherwise related syntax.
    pub fn related(&self) -> &[ParseRelatedRange] {
        &self.related
    }

    /// Expected syntax fragments.
    pub fn expected(&self) -> &[String] {
        &self.expected
    }

    /// Found fragment, if known.
    pub fn found(&self) -> Option<&str> {
        self.found.as_deref()
    }

    /// Human-readable diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Recovery suggestions.
    pub fn recovery(&self) -> &[RecoverySuggestion] {
        &self.recovery
    }

    /// Builds the shared Arcweft diagnostic representation for CLI/LSP/Agent renderers.
    ///
    /// # Panics
    ///
    /// Panics if `document` is not the exact source document that was parsed.
    pub fn diagnostic(&self, document: &SourceDocument) -> Diagnostic {
        let span = document
            .span(SourceRange::new(self.range.start(), self.range.end()))
            .expect("a parser range belongs to the document that was parsed");
        let mut diagnostic = Diagnostic::new(DiagnosticSeverity::Error, self.message.clone())
            .with_code(self.kind.code())
            .with_label(DiagnosticLabel::primary(
                span,
                self.found
                    .as_ref()
                    .map(|found| format!("found `{found}` here")),
            ));
        for related in &self.related {
            let span = document
                .span(SourceRange::new(related.range.start(), related.range.end()))
                .expect("a related parser range belongs to the parsed document");
            diagnostic =
                diagnostic.with_label(DiagnosticLabel::secondary(span, related.message.clone()));
        }
        if !self.expected.is_empty() {
            diagnostic = diagnostic.with_note(format!("expected: {}", self.expected.join(", ")));
        }
        for suggestion in &self.recovery {
            diagnostic = diagnostic.with_suggestion(suggestion.diagnostic_suggestion(document));
        }
        diagnostic
    }
}

impl ParseRelatedRange {
    /// Related byte range in the parsed source.
    pub const fn range(&self) -> &TextRange {
        &self.range
    }

    /// Optional explanation rendered with the secondary diagnostic label.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

impl RecoverySuggestion {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            edits: Vec::new(),
            applicability: DiagnosticApplicability::Unspecified,
        }
    }

    #[must_use]
    pub fn with_edit(mut self, edit: RecoveryEdit) -> Self {
        self.edits.push(edit);
        self
    }

    #[must_use]
    pub fn with_applicability(mut self, applicability: DiagnosticApplicability) -> Self {
        self.applicability = applicability;
        self
    }

    /// Recovery message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Recovery edits, when the suggestion can be expressed as a concrete patch.
    pub fn edits(&self) -> &[RecoveryEdit] {
        &self.edits
    }

    pub const fn applicability(&self) -> DiagnosticApplicability {
        self.applicability
    }

    fn diagnostic_suggestion(&self, document: &SourceDocument) -> DiagnosticSuggestion {
        self.edits.iter().fold(
            DiagnosticSuggestion::new(self.message.clone(), self.applicability),
            |suggestion, edit| suggestion.with_edit(edit.source_edit(document)),
        )
    }
}

impl RecoveryEdit {
    pub fn new(range: TextRange, replacement: impl Into<String>) -> Self {
        Self {
            range,
            replacement: replacement.into(),
        }
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }

    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    fn source_edit(&self, document: &SourceDocument) -> SourceEdit {
        SourceEdit::new(
            document
                .span(SourceRange::new(self.range.start(), self.range.end()))
                .expect("a parser recovery range belongs to the document that was parsed"),
            self.replacement.clone(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use arcweft_source::{DiagnosticApplicability, SourceDocument, SourceDocumentId, SourceName};

    use super::{ParseError, ParseErrorKind, RecoveryEdit, RecoverySuggestion, TextRange};

    const EXPECTED_PARSE_ERROR_KINDS: [(ParseErrorKind, &str, &str); 47] = [
        (ParseErrorKind::Generic, "syntax.parse", "Parse error"),
        (
            ParseErrorKind::ExpressionPrefixDepthLimit,
            "syntax.expr.prefix_depth_limit",
            "Expression prefix depth limit exceeded",
        ),
        (
            ParseErrorKind::AssertionUnknownMode,
            "syntax.assert.unknown_mode",
            "Unknown assertion mode",
        ),
        (
            ParseErrorKind::AssertionInvalidArgument,
            "syntax.assert.invalid_argument",
            "Invalid assertion argument",
        ),
        (
            ParseErrorKind::AssertionUnclosedArguments,
            "syntax.assert.unclosed_arguments",
            "Unclosed assertion argument list",
        ),
        (
            ParseErrorKind::AssertionEmptyConditions,
            "syntax.assert.empty_conditions",
            "Empty assertion condition list",
        ),
        (
            ParseErrorKind::AssertionTooManyConditions,
            "syntax.assert.too_many_conditions",
            "Too many assertion conditions",
        ),
        (
            ParseErrorKind::EntryMissingKind,
            "syntax.entry.missing_kind",
            "Missing entry kind",
        ),
        (
            ParseErrorKind::EntryMissingId,
            "syntax.entry.missing_id",
            "Missing entry public ID",
        ),
        (
            ParseErrorKind::EntryIdFamily,
            "syntax.entry.id_family",
            "Invalid entry public ID family",
        ),
        (
            ParseErrorKind::EntryTrailingHead,
            "syntax.entry.trailing_head",
            "Trailing syntax in entry declaration head",
        ),
        (
            ParseErrorKind::EntryRoleBinding,
            "syntax.entry.role_binding",
            "Malformed entry role binding",
        ),
        (
            ParseErrorKind::EntryRoleValue,
            "syntax.entry.role_value",
            "Missing entry role value",
        ),
        (
            ParseErrorKind::EntryRolePath,
            "syntax.entry.role_path",
            "Invalid entry role symbol path",
        ),
        (
            ParseErrorKind::NominalInvalidGenericParameters,
            "syntax.nominal.invalid_generic_parameters",
            "Invalid nominal generic parameter list",
        ),
        (
            ParseErrorKind::ProofTrustedNotProof,
            "syntax.proof.trusted.not_proof",
            "Trusted proof attribute on a non-proof item",
        ),
        (
            ParseErrorKind::ProofTrustedDuplicate,
            "syntax.proof.trusted.duplicate",
            "Duplicate trusted proof attribute",
        ),
        (
            ParseErrorKind::ProofTrustedReasonMissing,
            "syntax.proof.trusted.reason_missing",
            "Missing trusted proof reason",
        ),
        (
            ParseErrorKind::ProofTrustedReasonDuplicate,
            "syntax.proof.trusted.reason_duplicate",
            "Duplicate trusted proof reason",
        ),
        (
            ParseErrorKind::ProofTrustedReasonNotString,
            "syntax.proof.trusted.reason_not_string",
            "Trusted proof reason is not a string",
        ),
        (
            ParseErrorKind::ProofTrustedReasonEmpty,
            "syntax.proof.trusted.reason_empty",
            "Trusted proof reason is empty",
        ),
        (
            ParseErrorKind::ProofTrustedUnknownArgument,
            "syntax.proof.trusted.unknown_argument",
            "Unknown trusted proof argument",
        ),
        (
            ParseErrorKind::ProofTrustedPositionalArgument,
            "syntax.proof.trusted.positional_argument",
            "Positional trusted proof argument",
        ),
        (
            ParseErrorKind::StyleInlineSelectorNotSupported,
            "style::inline_selector_not_supported",
            "Selector rule in inline Style",
        ),
        (
            ParseErrorKind::StyleMalformedSelector,
            "style::malformed_selector",
            "Malformed Style selector",
        ),
        (
            ParseErrorKind::StyleEnvironmentExpectedOpenParen,
            "syntax.parse.style_environment.expected_open_paren",
            "Expected environment opening parenthesis",
        ),
        (
            ParseErrorKind::StyleEnvironmentExpectedField,
            "syntax.parse.style_environment.expected_field",
            "Expected environment field",
        ),
        (
            ParseErrorKind::StyleEnvironmentExpectedComparison,
            "syntax.parse.style_environment.expected_comparison",
            "Expected environment comparison",
        ),
        (
            ParseErrorKind::StyleEnvironmentExpectedValue,
            "syntax.parse.style_environment.expected_value",
            "Expected environment value",
        ),
        (
            ParseErrorKind::StyleEnvironmentExpectedCommaOrCloseParen,
            "syntax.parse.style_environment.expected_comma_or_close_paren",
            "Expected environment clause separator",
        ),
        (
            ParseErrorKind::StyleEnvironmentExpectedOpenBrace,
            "syntax.parse.style_environment.expected_open_brace",
            "Expected environment body opening brace",
        ),
        (
            ParseErrorKind::StyleEnvironmentUnterminatedCondition,
            "syntax.parse.style_environment.unterminated_condition",
            "Unterminated environment condition",
        ),
        (
            ParseErrorKind::StyleEnvironmentUnsupportedValue,
            "syntax.parse.style_environment.unsupported_value",
            "Unsupported environment value",
        ),
        (
            ParseErrorKind::StyleEnvironmentTokenNotAllowed,
            "syntax.parse.style_environment.token_not_allowed",
            "Style token in environment body",
        ),
        (
            ParseErrorKind::ViewExportPartMisplaced,
            "view::export_part_misplaced",
            "Misplaced View part export",
        ),
        (
            ParseErrorKind::ViewDuplicatePartModifier,
            "view::duplicate_part_modifier",
            "Duplicate View part modifier",
        ),
        (
            ParseErrorKind::ViewExportPartMissingPart,
            "view::export_part_missing_part",
            "Missing `part` keyword in View export",
        ),
        (
            ParseErrorKind::ViewExportPartDuplicateAs,
            "view::export_part_duplicate_as",
            "Duplicate `as` keyword in View part export",
        ),
        (
            ParseErrorKind::ViewExportPartTrailingSyntax,
            "view::export_part_trailing_syntax",
            "Trailing syntax in View part export",
        ),
        (
            ParseErrorKind::ViewExportPartMissingLocal,
            "view::export_part_missing_local",
            "Missing local View part name",
        ),
        (
            ParseErrorKind::ViewExportPartInvalidLocalName,
            "view::export_part_invalid_local_name",
            "Invalid local View part name",
        ),
        (
            ParseErrorKind::ViewExportPartMissingAs,
            "view::export_part_missing_as",
            "Missing `as` keyword in View part export",
        ),
        (
            ParseErrorKind::ViewExportPartMissingPublic,
            "view::export_part_missing_public",
            "Missing public View part name",
        ),
        (
            ParseErrorKind::ViewExportPartInvalidPublicName,
            "view::export_part_invalid_public_name",
            "Invalid public View part name",
        ),
        (
            ParseErrorKind::ViewPartMissingName,
            "view::part_missing_name",
            "Missing View part modifier name",
        ),
        (
            ParseErrorKind::ViewPartTrailingSyntax,
            "view::part_trailing_syntax",
            "Trailing syntax in View part modifier",
        ),
        (
            ParseErrorKind::ViewPartInvalidLocalName,
            "view::part_invalid_local_name",
            "Invalid View part modifier name",
        ),
    ];

    #[test]
    fn parse_error_kind_inventory_is_complete_unique_and_stable() {
        let expected = EXPECTED_PARSE_ERROR_KINDS;
        assert_eq!(ParseErrorKind::ALL, expected.map(|entry| entry.0));
        assert_eq!(ParseErrorKind::ALL.len(), 47);
        assert_eq!(
            ParseErrorKind::ALL
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len(),
            ParseErrorKind::ALL.len()
        );
        assert_eq!(
            ParseErrorKind::ALL
                .iter()
                .map(|kind| kind.code())
                .collect::<BTreeSet<_>>()
                .len(),
            ParseErrorKind::ALL.len()
        );
        for (kind, code, label) in expected {
            assert_eq!(kind.code(), code);
            assert_eq!(kind.label(), label);

            let error = ParseError::new_with_kind(
                kind,
                TextRange::new(2, 4),
                Vec::new(),
                None,
                "typed payload".to_owned(),
                Vec::new(),
            );
            assert_eq!(error.kind(), kind);
            assert_eq!(error.code(), code);
            assert_eq!(error.label(), label);
            assert_eq!(error.range(), &TextRange::new(2, 4));
            assert!(error.expected().is_empty());
            assert_eq!(error.found(), None);
            assert_eq!(error.message(), "typed payload");
            assert!(error.recovery().is_empty());
        }
    }

    #[test]
    fn generic_error_preserves_structured_payload_and_shared_projection() {
        let source = "alpha bad omega";
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-generated://parse-error-payload/0")
                .expect("valid source id"),
            SourceName::Generated,
            source,
        )
        .expect("valid source document");
        let error = ParseError::new_with_kind(
            ParseErrorKind::Generic,
            TextRange::new(6, 9),
            vec!["value".to_owned()],
            Some("bad".to_owned()),
            "expected a value".to_owned(),
            vec![
                RecoverySuggestion::new("replace the invalid token")
                    .with_edit(RecoveryEdit::new(TextRange::new(6, 9), "value"))
                    .with_applicability(DiagnosticApplicability::MachineApplicable),
            ],
        );

        assert_eq!(error.kind(), ParseErrorKind::Generic);
        assert_eq!(error.code(), "syntax.parse");
        assert_eq!(error.label(), "Parse error");
        assert_eq!(error.range(), &TextRange::new(6, 9));
        assert_eq!(error.expected(), &["value"]);
        assert_eq!(error.found(), Some("bad"));
        assert_eq!(error.message(), "expected a value");
        assert_eq!(
            error.recovery()[0].applicability(),
            DiagnosticApplicability::MachineApplicable
        );
        assert_eq!(
            error.recovery()[0].edits()[0].range(),
            &TextRange::new(6, 9)
        );
        assert_eq!(error.recovery()[0].edits()[0].replacement(), "value");

        let diagnostic = error.diagnostic(&document);
        assert_eq!(
            diagnostic
                .code()
                .map(arcweft_source::DiagnosticCode::as_str),
            Some("syntax.parse")
        );
        assert_eq!(diagnostic.message(), "expected a value");
        assert_eq!(diagnostic.labels()[0].message(), Some("found `bad` here"));
        assert_eq!(diagnostic.notes(), &["expected: value"]);
        assert_eq!(
            diagnostic.suggestions()[0].applicability(),
            DiagnosticApplicability::MachineApplicable
        );
        assert_eq!(
            diagnostic.suggestions()[0].edits()[0]
                .span()
                .range()
                .as_range(),
            6..9
        );
        assert_eq!(
            diagnostic.suggestions()[0].edits()[0].replacement(),
            "value"
        );
    }
}
