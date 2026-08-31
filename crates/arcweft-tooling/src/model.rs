use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Syntax-only formatting options.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FormatOptions;

/// A half-open source edit over UTF-8 byte offsets.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextEdit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

/// Whether a diagnostic stopped all editing or left only that line unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolingDiagnosticDisposition {
    Stops,
    Partial,
}

/// One diagnostic produced while computing tooling edits.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolingDiagnostic {
    pub code: String,
    pub message: String,
    pub arguments: BTreeMap<String, String>,
    pub start: usize,
    pub end: usize,
    pub disposition: ToolingDiagnosticDisposition,
}

impl ToolingDiagnostic {
    #[must_use]
    pub fn syntax(message: impl Into<String>, start: usize, end: usize) -> Self {
        Self {
            code: "AWT-PARSE-001".to_owned(),
            message: message.into(),
            arguments: BTreeMap::new(),
            start,
            end,
            disposition: ToolingDiagnosticDisposition::Partial,
        }
    }
}

/// A complete source-edit report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolingEditReport {
    pub status: String,
    pub changed: bool,
    pub edits: Vec<TextEdit>,
    pub output: String,
    pub diagnostics: Vec<ToolingDiagnostic>,
}

/// Error returned when edit application or semantic preconditions fail.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ToolingError {
    #[error("syntax database allocation failed: {message}")]
    SyntaxDatabaseUnavailable { message: String },
    #[error("source attachment failed: {message}")]
    SyntaxAttachmentFailed { message: String },
    #[error("text edit range {start}..{end} is outside source length {len}")]
    RangeOutOfBounds {
        start: usize,
        end: usize,
        len: usize,
    },
    #[error("text edit range {start}..{end} overlaps a later edit")]
    OverlappingEdit { start: usize, end: usize },
    #[error("text edit range {start}..{end} does not align to UTF-8 character boundaries")]
    InvalidCharBoundary { start: usize, end: usize },
}

impl ToolingError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::SyntaxDatabaseUnavailable { .. } | Self::SyntaxAttachmentFailed { .. } => {
                "AWT-SYNTAX-001"
            }
            Self::RangeOutOfBounds { .. }
            | Self::OverlappingEdit { .. }
            | Self::InvalidCharBoundary { .. } => "AWT-EDIT-001",
        }
    }
}
