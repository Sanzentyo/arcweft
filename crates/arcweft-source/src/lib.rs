use core::ops::Range;

pub mod diagnostic;
pub mod document;
pub mod identity;

pub use diagnostic::{
    Diagnostic, DiagnosticApplicability, DiagnosticBag, DiagnosticCode, DiagnosticCommand,
    DiagnosticLabel, DiagnosticLabelStyle, DiagnosticSeverity, DiagnosticSuggestion, SourceEdit,
};
pub use document::{
    MAX_REGISTRATION_SOURCE_BYTES, SourceDocument, SourceDocumentError, SourceDocumentId,
    SourceDocumentIdError, SourceDocumentIdentity, SourceRevision, SourceSetRevision,
    SourceSetRevisionError, SourceSpan, SourceSpanError,
};
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

/// Semantic source reference around a complete revision-bound span.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceAnchor {
    span: SourceSpan,
}

impl SourceAnchor {
    #[must_use]
    pub const fn from_span(span: SourceSpan) -> Self {
        Self { span }
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }

    pub fn source(&self) -> &SourceDocumentIdentity {
        self.span.source()
    }

    pub fn byte_range(&self) -> Range<usize> {
        self.span.range().as_range()
    }

    pub fn to_span(&self) -> SourceSpan {
        self.span.clone()
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceName {
    Path(String),
    Generated,
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

/// A display position derived from an accepted document's line index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePosition {
    pub line: u32,
    pub column: u32,
}

impl SourcePosition {
    pub const fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}
