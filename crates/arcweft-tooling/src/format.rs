use crate::dialogue_sugar::{DialogueSugarContext, DialogueSugarMode, dialogue_text_sugar_edits};
use crate::edit::report_from_edits;
use crate::model::{FormatOptions, TextEdit, ToolingEditReport, ToolingError};
use crate::sugar_expansion::sugar_expansion_edits;
use arcweft_lang_syntax::parser::parse_source;

/// Formats source while preserving authoring sugar by default.
pub fn format_source(
    source: &str,
    options: FormatOptions,
) -> Result<ToolingEditReport, ToolingError> {
    let mut edits = Vec::new();
    if options.expand_sugar {
        edits.extend(sugar_expansion_edits(source));
    } else if options.canonical_rich_text {
        edits.extend(rich_text_canonical_edits(source));
    }
    report_from_edits(source, edits)
}

fn rich_text_canonical_edits(source: &str) -> Vec<TextEdit> {
    let parsed = parse_source(source);
    let context = DialogueSugarContext::from_parsed(&parsed);
    dialogue_text_sugar_edits(source, &parsed, DialogueSugarMode::RichTextOnly, &context)
}
