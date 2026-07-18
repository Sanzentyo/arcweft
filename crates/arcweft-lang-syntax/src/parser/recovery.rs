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
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ParseErrorKind {
    /// Ordinary parser recovery without a dedicated diagnostic family.
    Generic,
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
    /// An entry role appears more than once.
    EntryDuplicateRole,
    /// An entry kind does not allow a declared role.
    EntryIncompatibleRole,
    /// A stateful entry declares more than one initial target.
    EntryDuplicateGoto,
    /// An entry kind does not allow an initial target.
    EntryIncompatibleGoto,
    /// An entry kind does not allow route declarations.
    EntryIncompatibleRoute,
    /// An entry omits a role required by its kind.
    EntryMissingRole,
    /// A stateful entry omits its initial target.
    EntryMissingGoto,
    /// An entry role binding is malformed.
    EntryRoleBinding,
    /// An entry role binding omits its value.
    EntryRoleValue,
    /// An entry callable role contains an invalid symbol path.
    EntryRolePath,
    /// A nominal declaration contains an invalid generic parameter list.
    NominalInvalidGenericParameters,
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
    /// Complete registry of repository-owned parser diagnostic kinds.
    pub const ALL: &'static [Self] = &[
        Self::Generic,
        Self::AssertionUnknownMode,
        Self::AssertionInvalidArgument,
        Self::AssertionUnclosedArguments,
        Self::AssertionEmptyConditions,
        Self::AssertionTooManyConditions,
        Self::EntryMissingKind,
        Self::EntryMissingId,
        Self::EntryIdFamily,
        Self::EntryTrailingHead,
        Self::EntryDuplicateRole,
        Self::EntryIncompatibleRole,
        Self::EntryDuplicateGoto,
        Self::EntryIncompatibleGoto,
        Self::EntryIncompatibleRoute,
        Self::EntryMissingRole,
        Self::EntryMissingGoto,
        Self::EntryRoleBinding,
        Self::EntryRoleValue,
        Self::EntryRolePath,
        Self::NominalInvalidGenericParameters,
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
            Self::AssertionUnknownMode => "syntax.assert.unknown_mode",
            Self::AssertionInvalidArgument => "syntax.assert.invalid_argument",
            Self::AssertionUnclosedArguments => "syntax.assert.unclosed_arguments",
            Self::AssertionEmptyConditions => "syntax.assert.empty_conditions",
            Self::AssertionTooManyConditions => "syntax.assert.too_many_conditions",
            Self::EntryMissingKind => "syntax.entry.missing_kind",
            Self::EntryMissingId => "syntax.entry.missing_id",
            Self::EntryIdFamily => "syntax.entry.id_family",
            Self::EntryTrailingHead => "syntax.entry.trailing_head",
            Self::EntryDuplicateRole => "syntax.entry.duplicate_role",
            Self::EntryIncompatibleRole => "syntax.entry.incompatible_role",
            Self::EntryDuplicateGoto => "syntax.entry.duplicate_goto",
            Self::EntryIncompatibleGoto => "syntax.entry.incompatible_goto",
            Self::EntryIncompatibleRoute => "syntax.entry.incompatible_route",
            Self::EntryMissingRole => "syntax.entry.missing_role",
            Self::EntryMissingGoto => "syntax.entry.missing_goto",
            Self::EntryRoleBinding => "syntax.entry.role_binding",
            Self::EntryRoleValue => "syntax.entry.role_value",
            Self::EntryRolePath => "syntax.entry.role_path",
            Self::NominalInvalidGenericParameters => "syntax.nominal.invalid_generic_parameters",
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
            Self::AssertionUnknownMode => "Unknown assertion mode",
            Self::AssertionInvalidArgument => "Invalid assertion argument",
            Self::AssertionUnclosedArguments => "Unclosed assertion argument list",
            Self::AssertionEmptyConditions => "Empty assertion condition list",
            Self::AssertionTooManyConditions => "Too many assertion conditions",
            Self::EntryMissingKind => "Missing entry kind",
            Self::EntryMissingId => "Missing entry public ID",
            Self::EntryIdFamily => "Invalid entry public ID family",
            Self::EntryTrailingHead => "Trailing syntax in entry declaration head",
            Self::EntryDuplicateRole => "Duplicate entry role",
            Self::EntryIncompatibleRole => "Entry role is incompatible with its kind",
            Self::EntryDuplicateGoto => "Duplicate entry initial target",
            Self::EntryIncompatibleGoto => "Entry initial target is incompatible with its kind",
            Self::EntryIncompatibleRoute => "Entry route is incompatible with its kind",
            Self::EntryMissingRole => "Missing required entry role",
            Self::EntryMissingGoto => "Missing entry initial target",
            Self::EntryRoleBinding => "Malformed entry role binding",
            Self::EntryRoleValue => "Missing entry role value",
            Self::EntryRolePath => "Invalid entry role symbol path",
            Self::NominalInvalidGenericParameters => "Invalid nominal generic parameter list",
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
    pub(crate) fn new(
        range: TextRange,
        expected: Vec<String>,
        found: Option<String>,
        message: String,
        recovery: Vec<RecoverySuggestion>,
    ) -> Self {
        Self::new_with_kind(
            ParseErrorKind::Generic,
            range,
            expected,
            found,
            message,
            recovery,
        )
    }

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

    pub(crate) fn rebased(mut self, base_offset: usize) -> Self {
        self.range = TextRange::new(
            self.range.start() + base_offset,
            self.range.end() + base_offset,
        );
        self
    }

    /// Repository-owned parser diagnostic discriminator.
    #[must_use]
    pub const fn kind(&self) -> ParseErrorKind {
        self.kind
    }

    pub(crate) fn with_related(
        mut self,
        range: TextRange,
        message: impl Into<Option<String>>,
    ) -> Self {
        self.related.push(ParseRelatedRange {
            range,
            message: message.into(),
        });
        self
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
    use arcweft_source::{DiagnosticApplicability, SourceDocument, SourceDocumentId, SourceName};

    use super::{ParseError, ParseErrorKind, RecoveryEdit, RecoverySuggestion, TextRange};

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
        let error = ParseError::new(
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
