use core::ops::Range;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceRange {
    start: usize,
    end: usize,
}

impl SourceRange {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn start(self) -> usize {
        self.start
    }

    pub const fn end(self) -> usize {
        self.end
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    pub const fn as_range(self) -> Range<usize> {
        self.start..self.end
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceAnchor {
    source: SourceName,
    byte_range: Range<usize>,
    start: Option<SourcePosition>,
    end: Option<SourcePosition>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceName {
    Path(String),
    Generated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePosition {
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    source: SourceName,
    range: SourceRange,
    start: Option<SourcePosition>,
    end: Option<SourcePosition>,
}

impl SourceSpan {
    pub fn new(source: SourceName, range: SourceRange) -> Self {
        Self {
            source,
            range,
            start: None,
            end: None,
        }
    }

    #[must_use]
    pub fn with_positions(mut self, start: SourcePosition, end: SourcePosition) -> Self {
        self.start = Some(start);
        self.end = Some(end);
        self
    }

    pub const fn source(&self) -> &SourceName {
        &self.source
    }

    pub const fn range(&self) -> SourceRange {
        self.range
    }

    pub const fn start(&self) -> Option<SourcePosition> {
        self.start
    }

    pub const fn end(&self) -> Option<SourcePosition> {
        self.end
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticCode(String);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticLabelStyle {
    Primary,
    Secondary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticLabel {
    style: DiagnosticLabelStyle,
    span: SourceSpan,
    message: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticApplicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,
    Unspecified,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEdit {
    span: SourceSpan,
    replacement: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticSuggestion {
    message: String,
    edits: Vec<SourceEdit>,
    applicability: DiagnosticApplicability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    severity: DiagnosticSeverity,
    code: Option<DiagnosticCode>,
    message: String,
    span: Option<SourceSpan>,
    labels: Vec<DiagnosticLabel>,
    notes: Vec<String>,
    suggestions: Vec<DiagnosticSuggestion>,
}

impl Diagnostic {
    pub fn new(severity: DiagnosticSeverity, message: impl Into<String>) -> Self {
        Self {
            severity,
            code: None,
            message: message.into(),
            span: None,
            labels: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(DiagnosticCode::new(code));
        self
    }

    #[must_use]
    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    #[must_use]
    pub fn with_label(mut self, label: DiagnosticLabel) -> Self {
        if label.style == DiagnosticLabelStyle::Primary && self.span.is_none() {
            self.span = Some(label.span.clone());
        }
        self.labels.push(label);
        self
    }

    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    #[must_use]
    pub fn with_suggestion(mut self, suggestion: DiagnosticSuggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    pub const fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    pub const fn code(&self) -> Option<&DiagnosticCode> {
        self.code.as_ref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn span(&self) -> Option<&SourceSpan> {
        self.span.as_ref()
    }

    pub fn labels(&self) -> &[DiagnosticLabel] {
        &self.labels
    }

    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    pub fn suggestions(&self) -> &[DiagnosticSuggestion] {
        &self.suggestions
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiagnosticBag {
    diagnostics: std::vec::Vec<Diagnostic>,
}

impl DiagnosticBag {
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Diagnostic> {
        self.diagnostics.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

impl<'a> IntoIterator for &'a DiagnosticBag {
    type IntoIter = std::slice::Iter<'a, Diagnostic>;
    type Item = &'a Diagnostic;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl SourceAnchor {
    pub fn new(source: SourceName, byte_range: Range<usize>) -> Self {
        Self {
            source,
            byte_range,
            start: None,
            end: None,
        }
    }

    #[must_use]
    pub fn with_positions(mut self, start: SourcePosition, end: SourcePosition) -> Self {
        self.start = Some(start);
        self.end = Some(end);
        self
    }

    pub fn generated() -> Self {
        Self::new(SourceName::Generated, 0..0)
    }

    pub fn source(&self) -> &SourceName {
        &self.source
    }

    pub fn byte_range(&self) -> Range<usize> {
        self.byte_range.clone()
    }

    pub fn start(&self) -> Option<SourcePosition> {
        self.start
    }

    pub fn end(&self) -> Option<SourcePosition> {
        self.end
    }

    pub fn to_span(&self) -> SourceSpan {
        let span = SourceSpan::new(
            self.source.clone(),
            SourceRange::new(self.byte_range.start, self.byte_range.end),
        );
        match (self.start, self.end) {
            (Some(start), Some(end)) => span.with_positions(start, end),
            _ => span,
        }
    }
}

impl SourceName {
    pub fn path(value: impl Into<String>) -> Self {
        Self::Path(value.into())
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Path(path) => path,
            Self::Generated => "<generated>",
        }
    }
}

impl SourcePosition {
    pub const fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

impl DiagnosticCode {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl DiagnosticLabel {
    pub fn primary(span: SourceSpan, message: impl Into<Option<String>>) -> Self {
        Self {
            style: DiagnosticLabelStyle::Primary,
            span,
            message: message.into(),
        }
    }

    pub fn secondary(span: SourceSpan, message: impl Into<Option<String>>) -> Self {
        Self {
            style: DiagnosticLabelStyle::Secondary,
            span,
            message: message.into(),
        }
    }

    pub const fn style(&self) -> DiagnosticLabelStyle {
        self.style
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

impl SourceEdit {
    pub fn new(span: SourceSpan, replacement: impl Into<String>) -> Self {
        Self {
            span,
            replacement: replacement.into(),
        }
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }

    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

impl DiagnosticSuggestion {
    pub fn new(message: impl Into<String>, applicability: DiagnosticApplicability) -> Self {
        Self {
            message: message.into(),
            edits: Vec::new(),
            applicability,
        }
    }

    #[must_use]
    pub fn with_edit(mut self, edit: SourceEdit) -> Self {
        self.edits.push(edit);
        self
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn edits(&self) -> &[SourceEdit] {
        &self.edits
    }

    pub const fn applicability(&self) -> DiagnosticApplicability {
        self.applicability
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Diagnostic, DiagnosticApplicability, DiagnosticBag, DiagnosticLabel, DiagnosticSeverity,
        DiagnosticSuggestion, SourceEdit, SourceName, SourcePosition, SourceRange, SourceSpan,
    };

    #[test]
    fn source_span_preserves_range_positions_and_diagnostics() {
        let span = SourceSpan::new(SourceName::path("game.arcw"), SourceRange::new(2, 8))
            .with_positions(SourcePosition::new(1, 2), SourcePosition::new(1, 8));
        assert_eq!(span.range().as_range(), 2..8);
        assert_eq!(span.start(), Some(SourcePosition::new(1, 2)));

        let suggestion = DiagnosticSuggestion::new(
            "replace with compact form",
            DiagnosticApplicability::MachineApplicable,
        )
        .with_edit(SourceEdit::new(span.clone(), "flow opening"));
        let diagnostic = Diagnostic::new(DiagnosticSeverity::Warning, "check")
            .with_code("AWF0103")
            .with_label(DiagnosticLabel::primary(
                span.clone(),
                Some("explicit id".to_owned()),
            ))
            .with_note("style lint")
            .with_suggestion(suggestion);
        let mut bag = DiagnosticBag::default();
        bag.push(diagnostic);
        let diagnostic = bag.iter().next().expect("diagnostic");
        assert_eq!(diagnostic.severity(), DiagnosticSeverity::Warning);
        assert_eq!(diagnostic.message(), "check");
        assert_eq!(diagnostic.code().expect("code").as_str(), "AWF0103");
        assert_eq!(diagnostic.span().expect("span").range().as_range(), 2..8);
        assert_eq!(diagnostic.labels().len(), 1);
        assert_eq!(diagnostic.notes(), &["style lint".to_owned()]);
        assert_eq!(diagnostic.suggestions().len(), 1);
    }
}
