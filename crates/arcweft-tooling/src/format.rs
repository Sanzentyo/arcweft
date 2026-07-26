use crate::edit::report_from_edits;
use crate::model::{FormatOptions, TextEdit, ToolingDiagnostic, ToolingEditReport, ToolingError};
use crate::rich_text_canonicalization::{
    RichTextCanonicalizationContext, rich_text_canonical_edits as canonical_content_edits,
};
use crate::style_environment::canonical_environment_edits;
use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};
use arcweft_source::SourceDocument;
use std::sync::Arc;

mod view;

/// Formats an exact source document while preserving authoring sugar by default.
pub fn format_document(
    document: Arc<SourceDocument>,
    options: FormatOptions,
) -> Result<ToolingEditReport, ToolingError> {
    let parsed = parse_document_with_source(document, ParseOptions::default());
    let source = parsed.source();
    let mut edits = view::canonical_edits(source, &parsed);
    edits.extend(canonical_environment_edits(&parsed));
    if options.canonical_rich_text {
        edits.extend(rich_text_canonical_edits(&parsed));
    }
    let mut report = report_from_edits(source, edits)?;
    report.diagnostics = parsed
        .errors()
        .iter()
        .map(|error| {
            ToolingDiagnostic::syntax(error.message(), error.range().start(), error.range().end())
        })
        .collect();
    Ok(report)
}

fn rich_text_canonical_edits(parsed: &arcweft_lang_syntax::source::ParsedSource) -> Vec<TextEdit> {
    let context = RichTextCanonicalizationContext::from_parsed(parsed);
    canonical_content_edits(parsed, &context)
}
