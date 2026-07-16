use std::collections::BTreeMap;

use arcweft_lang_sema::canonicalization::{
    CheckedCanonicalizationInventory, SemanticDataUnavailable,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Syntax-only formatting options. Semantic expansion is intentionally absent.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FormatOptions {
    /// Rewrite inferred rich-text tags into explicit canonical spans.
    pub canonical_rich_text: bool,
}

/// Required input state for semantic canonicalization.
#[derive(Clone, Copy, Debug)]
pub enum CanonicalizationInput<'a> {
    Checked(&'a CheckedCanonicalizationInventory),
    Unavailable(&'a SemanticDataUnavailable),
}

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

/// Typed, stable canonicalization diagnostic kinds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolingDiagnosticKind {
    SpeakerExpressionUnresolved {
        reference: String,
        state: String,
    },
    SpeakerExpressionNonSpeaker {
        reference: String,
        resolved_type: String,
    },
    SpeakerSurfaceInconsistent {
        reason: String,
    },
}

impl ToolingDiagnosticKind {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::SpeakerExpressionUnresolved { .. } => "AWT-CANON-003",
            Self::SpeakerExpressionNonSpeaker { .. } => "AWT-CANON-004",
            Self::SpeakerSurfaceInconsistent { .. } => "AWT-CANON-005",
        }
    }

    #[must_use]
    pub const fn disposition(&self) -> ToolingDiagnosticDisposition {
        ToolingDiagnosticDisposition::Partial
    }

    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::SpeakerExpressionUnresolved { reference, state } => format!(
                "speaker expression `{reference}` is {state}; the line was not canonicalized"
            ),
            Self::SpeakerExpressionNonSpeaker {
                reference,
                resolved_type,
            } => format!(
                "speaker expression `{reference}` resolves to non-speaker type `{resolved_type}`; the line was not canonicalized"
            ),
            Self::SpeakerSurfaceInconsistent { reason } => format!(
                "checked speaker-line surface is inconsistent with the parsed source ({reason}); the line was not canonicalized"
            ),
        }
    }

    #[must_use]
    pub fn arguments(&self) -> BTreeMap<String, String> {
        match self {
            Self::SpeakerExpressionUnresolved { reference, state } => BTreeMap::from([
                ("reference".to_owned(), reference.clone()),
                ("state".to_owned(), state.clone()),
            ]),
            Self::SpeakerExpressionNonSpeaker {
                reference,
                resolved_type,
            } => BTreeMap::from([
                ("reference".to_owned(), reference.clone()),
                ("resolved_type".to_owned(), resolved_type.clone()),
            ]),
            Self::SpeakerSurfaceInconsistent { reason } => {
                BTreeMap::from([("reason".to_owned(), reason.clone())])
            }
        }
    }
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
    pub fn from_kind(kind: &ToolingDiagnosticKind, start: usize, end: usize) -> Self {
        Self {
            code: kind.code().to_owned(),
            message: kind.message(),
            arguments: kind.arguments(),
            start,
            end,
            disposition: kind.disposition(),
        }
    }

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

/// Inlay hint data independent from any concrete LSP transport.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InlayHint {
    pub position: usize,
    pub label: String,
}

/// Tooling code action data independent from any concrete LSP transport.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolingCodeAction {
    pub id: String,
    pub label: String,
    pub edit: Option<TextEdit>,
    pub diagnostics: Vec<ToolingDiagnostic>,
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
    #[error("semantic data is unavailable for `{document}`: {reason}")]
    SemanticDataUnavailable { document: String, reason: String },
    #[error(
        "semantic inventory for `{document}` is stale: expected {expected_revision}/{expected_len} bytes, got {actual_revision}/{actual_len} bytes"
    )]
    StaleSemanticInventory {
        document: String,
        expected_revision: String,
        actual_revision: String,
        expected_len: usize,
        actual_len: usize,
    },
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
    #[error("failed to canonicalize dialogue text at {start}..{end}: {source}")]
    DialogueCanonicalization {
        start: usize,
        end: usize,
        #[source]
        source: Box<Self>,
    },
}

impl ToolingError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::SemanticDataUnavailable { .. } => "AWT-CANON-001",
            Self::StaleSemanticInventory { .. } => "AWT-CANON-002",
            Self::RangeOutOfBounds { .. }
            | Self::OverlappingEdit { .. }
            | Self::InvalidCharBoundary { .. }
            | Self::DialogueCanonicalization { .. } => "AWT-EDIT-001",
        }
    }
}
