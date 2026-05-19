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
pub struct Diagnostic {
    severity: DiagnosticSeverity,
    message: String,
    span: Option<SourceSpan>,
}

impl Diagnostic {
    pub fn new(severity: DiagnosticSeverity, message: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
            span: None,
        }
    }

    #[must_use]
    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub const fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn span(&self) -> Option<&SourceSpan> {
        self.span.as_ref()
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
}

impl SourceName {
    pub fn path(value: impl Into<String>) -> Self {
        Self::Path(value.into())
    }
}

impl SourcePosition {
    pub const fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Diagnostic, DiagnosticBag, DiagnosticSeverity, SourceName, SourcePosition, SourceRange,
        SourceSpan,
    };

    #[test]
    fn source_span_preserves_range_positions_and_diagnostics() {
        let span = SourceSpan::new(SourceName::path("game.awft"), SourceRange::new(2, 8))
            .with_positions(SourcePosition::new(1, 2), SourcePosition::new(1, 8));
        assert_eq!(span.range().as_range(), 2..8);
        assert_eq!(span.start(), Some(SourcePosition::new(1, 2)));

        let mut bag = DiagnosticBag::default();
        bag.push(Diagnostic::new(DiagnosticSeverity::Warning, "check").with_span(span));
        let diagnostic = bag.iter().next().expect("diagnostic");
        assert_eq!(diagnostic.severity(), DiagnosticSeverity::Warning);
        assert_eq!(diagnostic.message(), "check");
    }
}
