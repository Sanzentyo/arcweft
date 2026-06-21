use arcweft_lang_hir::id_context::{
    IdContextEntry, IdContextMaterialization, IdContextOption, collect_id_context,
};

use crate::edit::report_from_edits;
use crate::model::{InlayHint, TextEdit, ToolingEditReport, ToolingError};

/// Rewrites ID-context relative IDs to normalized explicit IDs.
pub fn materialize_ids(source: &str) -> Result<ToolingEditReport, ToolingError> {
    let edits = id_context_edits(source);
    report_from_edits(source, edits)
}

/// Computes inferred-ID inlay hints for relative ID positions.
pub fn inferred_id_hints(source: &str) -> Vec<InlayHint> {
    id_context_hints(source)
}

fn id_context_edits(source: &str) -> Vec<TextEdit> {
    collect_id_context(source)
        .entries()
        .iter()
        .map(id_context_edit)
        .collect()
}

fn id_context_edit(entry: &IdContextEntry) -> TextEdit {
    match entry.materialization() {
        IdContextMaterialization::Replace { range, normalized } => TextEdit {
            start: range.start(),
            end: range.end(),
            replacement: format!("@{normalized}"),
        },
        IdContextMaterialization::InsertDialogueOptions {
            insert,
            call_has_options,
            options_has_any,
            options,
        } => {
            let joined = options
                .iter()
                .map(IdContextOption::as_assignment)
                .collect::<Vec<_>>()
                .join(", ");
            let replacement = if *call_has_options {
                if *options_has_any {
                    format!(", {joined}")
                } else {
                    joined
                }
            } else {
                format!("({joined})")
            };
            TextEdit {
                start: insert.start(),
                end: insert.end(),
                replacement,
            }
        }
    }
}

fn id_context_hints(source: &str) -> Vec<InlayHint> {
    collect_id_context(source)
        .entries()
        .iter()
        .map(|entry| match entry.materialization() {
            IdContextMaterialization::Replace { range, normalized } => InlayHint {
                position: range.end(),
                label: format!("@{normalized}"),
            },
            IdContextMaterialization::InsertDialogueOptions {
                insert, options, ..
            } => InlayHint {
                position: insert.start(),
                label: options
                    .iter()
                    .map(IdContextOption::as_assignment)
                    .collect::<Vec<_>>()
                    .join(", "),
            },
        })
        .collect()
}
