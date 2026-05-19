use arcweft_source::SourceAnchor;
use thiserror::Error;

use crate::ast::common::TextRange;

/// Syntax-level parse error with expected tokens and recovery suggestions.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct ParseError {
    range: TextRange,
    expected: Vec<String>,
    found: Option<String>,
    message: String,
    recovery: Vec<RecoverySuggestion>,
    anchor: SourceAnchor,
}

/// Suggested local edit or strategy for recovering from an error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoverySuggestion {
    pub(crate) message: String,
}

impl ParseError {
    pub(crate) fn new(
        range: TextRange,
        expected: Vec<String>,
        found: Option<String>,
        message: String,
        recovery: Vec<RecoverySuggestion>,
        anchor: SourceAnchor,
    ) -> Self {
        Self {
            range,
            expected,
            found,
            message,
            recovery,
            anchor,
        }
    }

    pub(crate) fn rebased(mut self, base_offset: usize) -> Self {
        self.range = TextRange::new(
            self.range.start() + base_offset,
            self.range.end() + base_offset,
        );
        self
    }

    /// Error byte range.
    pub const fn range(&self) -> &TextRange {
        &self.range
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

    /// Source anchor for tooling integrations.
    pub const fn anchor(&self) -> &SourceAnchor {
        &self.anchor
    }
}

impl RecoverySuggestion {
    /// Recovery message.
    pub fn message(&self) -> &str {
        &self.message
    }
}
