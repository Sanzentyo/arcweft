use crate::format::format_source;
use crate::id_context::materialize_ids;
use crate::model::{FormatOptions, TextEdit, ToolingCodeAction, ToolingError};

/// Returns source-level code actions that are safe to expose through LSP.
pub fn source_code_actions(source: &str) -> Result<Vec<ToolingCodeAction>, ToolingError> {
    let mut actions = Vec::new();
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: true,
            canonical_rich_text: false,
        },
    )?;
    if report.changed {
        actions.push(rewrite_action(
            "arcweft.expandSugar",
            "Expand Arcweft sugar",
            source,
            report.output,
        ));
    }
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: false,
            canonical_rich_text: true,
        },
    )?;
    if report.changed {
        actions.push(rewrite_action(
            "arcweft.canonicalRichText",
            "Canonicalize inferred rich-text tags",
            source,
            report.output,
        ));
    }
    let report = materialize_ids(source)?;
    actions.extend(report.edits.into_iter().map(|edit| ToolingCodeAction {
        id: "arcweft.materializeId".to_owned(),
        label: "Materialize inferred Arcweft ID".to_owned(),
        edit: Some(edit),
    }));
    Ok(actions)
}

fn rewrite_action(
    id: impl Into<String>,
    label: impl Into<String>,
    source: &str,
    output: String,
) -> ToolingCodeAction {
    ToolingCodeAction {
        id: id.into(),
        label: label.into(),
        edit: Some(TextEdit {
            start: 0,
            end: source.len(),
            replacement: output,
        }),
    }
}
