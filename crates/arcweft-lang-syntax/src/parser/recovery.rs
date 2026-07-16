use arcweft_source::{
    Diagnostic, DiagnosticApplicability, DiagnosticLabel, DiagnosticSeverity, DiagnosticSuggestion,
    SourceDocument, SourceEdit, SourceRange,
};
use thiserror::Error;

use crate::ast::common::TextRange;

/// Syntax-level parse error with expected tokens and recovery suggestions.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct ParseError {
    code: String,
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
    pub(crate) fn coded(code: &'static str, range: TextRange, message: impl Into<String>) -> Self {
        Self::new(range, Vec::new(), None, message.into(), Vec::new()).with_code(code)
    }

    pub(crate) fn new(
        range: TextRange,
        expected: Vec<String>,
        found: Option<String>,
        message: String,
        recovery: Vec<RecoverySuggestion>,
    ) -> Self {
        Self {
            code: "syntax.parse".to_owned(),
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

    pub(crate) fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = code.into();
        self
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
    pub fn code(&self) -> &str {
        &self.code
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
            .with_code(self.code.clone())
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
