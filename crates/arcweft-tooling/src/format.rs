use crate::dialogue_sugar::{DialogueSugarContext, DialogueSugarMode, dialogue_text_sugar_edits};
use crate::edit::report_from_edits;
use crate::model::{FormatOptions, TextEdit, ToolingDiagnostic, ToolingEditReport, ToolingError};
use arcweft_lang_syntax::parser::{ParseOptions, SourceDialect, parse_document, parse_source};

mod view;

/// Formats source while preserving authoring sugar by default.
pub fn format_source(
    source: &str,
    options: FormatOptions,
) -> Result<ToolingEditReport, ToolingError> {
    format_source_with_dialect(source, SourceDialect::Game, options)
}

/// Formats source for a known Arcweft dialect.
pub fn format_source_with_dialect(
    source: &str,
    dialect: SourceDialect,
    options: FormatOptions,
) -> Result<ToolingEditReport, ToolingError> {
    if dialect == SourceDialect::Agent && options.canonical_rich_text {
        return Err(ToolingError::UnsupportedFormatOption {
            option: "canonical_rich_text",
            dialect: "Agent",
        });
    }
    let parsed = (dialect == SourceDialect::Game).then(|| parse_source(source));
    let mut edits = parsed
        .as_ref()
        .map_or_else(Vec::new, |parsed| view::canonical_edits(source, parsed));
    if options.canonical_rich_text
        && let Some(parsed) = parsed.as_ref()
    {
        edits.extend(rich_text_canonical_edits(parsed));
    }
    let mut report = report_from_edits(source, edits)?;
    if dialect == SourceDialect::Agent {
        report.diagnostics = agent_format_diagnostics(source);
    } else if let Some(parsed) = parsed {
        report.diagnostics = parsed
            .errors()
            .iter()
            .map(|error| {
                ToolingDiagnostic::syntax(
                    error.message(),
                    error.range().start(),
                    error.range().end(),
                )
            })
            .collect();
    }
    Ok(report)
}

fn rich_text_canonical_edits(parsed: &arcweft_lang_syntax::source::ParsedSource) -> Vec<TextEdit> {
    let context = DialogueSugarContext::from_parsed(parsed);
    dialogue_text_sugar_edits(parsed, DialogueSugarMode::RichTextOnly, &context)
}

fn agent_format_diagnostics(source: &str) -> Vec<ToolingDiagnostic> {
    parse_document(
        source.to_owned(),
        ParseOptions {
            source_dialect: SourceDialect::Agent,
        },
    )
    .errors()
    .iter()
    .map(|error| {
        ToolingDiagnostic::syntax(error.message(), error.range().start(), error.range().end())
    })
    .collect()
}
