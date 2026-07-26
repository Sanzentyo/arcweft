use crate::canonicalize_source;
use crate::format::format_source;
use crate::model::{
    CanonicalizationInput, FormatOptions, TextEdit, ToolingCodeAction, ToolingError,
};

/// Returns source-level code actions that are safe to expose through LSP.
///
/// Semantic canonicalization is omitted when the adapter has no checked
/// inventory. Syntax-owned actions remain available in that state.
pub fn source_code_actions(
    source: &str,
    input: CanonicalizationInput<'_>,
) -> Result<Vec<ToolingCodeAction>, ToolingError> {
    let mut actions = Vec::new();
    if matches!(input, CanonicalizationInput::Checked(_)) {
        let report = canonicalize_source(source, input)?;
        if report.changed {
            let diagnostics = report.diagnostics;
            actions.push(rewrite_action(
                "arcweft.canonicalizeSugar",
                "Canonicalize Arcweft sugar",
                source,
                report.output,
                diagnostics,
            ));
        }
    }
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
