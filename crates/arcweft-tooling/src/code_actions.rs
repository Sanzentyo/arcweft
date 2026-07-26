use crate::format::format_document;
use crate::model::{FormatOptions, TextEdit, ToolingCodeAction, ToolingError};
use arcweft_source::SourceDocument;
use std::sync::Arc;

/// Returns source-level code actions that are safe to expose through LSP.
///
pub fn source_code_actions(
    document: Arc<SourceDocument>,
) -> Result<Vec<ToolingCodeAction>, ToolingError> {
    let mut actions = Vec::new();
    let source_len = document.text().len();
    let report = format_document(
        document,
        FormatOptions {
            canonical_rich_text: true,
        },
    )?;
    if report.changed {
        actions.push(rewrite_action(
            "arcweft.canonicalRichText",
            "Canonicalize inferred rich-text tags",
            source_len,
            report.output,
            Vec::new(),
        ));
    }
    Ok(actions)
}

fn rewrite_action(
    id: impl Into<String>,
    label: impl Into<String>,
    source_len: usize,
    output: String,
    diagnostics: Vec<crate::model::ToolingDiagnostic>,
) -> ToolingCodeAction {
    ToolingCodeAction {
        id: id.into(),
        label: label.into(),
        edit: Some(TextEdit {
            start: 0,
            end: source_len,
            replacement: output,
        }),
        diagnostics,
    }
}
