use crate::format::format_source;
use crate::model::{FormatOptions, TextEdit, ToolingCodeAction, ToolingError};

/// Returns source-level code actions that are safe to expose through LSP.
///
pub fn source_code_actions(source: &str) -> Result<Vec<ToolingCodeAction>, ToolingError> {
    let mut actions = Vec::new();
    let report = format_source(
        source,
        FormatOptions {
            canonical_rich_text: true,
        },
    )?;
    if report.changed {
        actions.push(rewrite_action(
            "arcweft.canonicalRichText",
            "Canonicalize inferred rich-text tags",
            source,
            report.output,
            Vec::new(),
        ));
    }
    Ok(actions)
}

fn rewrite_action(
    id: impl Into<String>,
    label: impl Into<String>,
    source: &str,
    output: String,
    diagnostics: Vec<crate::model::ToolingDiagnostic>,
) -> ToolingCodeAction {
    ToolingCodeAction {
        id: id.into(),
        label: label.into(),
        edit: Some(TextEdit {
            start: 0,
            end: source.len(),
            replacement: output,
        }),
        diagnostics,
    }
}
