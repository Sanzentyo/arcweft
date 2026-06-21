use crate::dialogue_sugar::{DialogueSugarContext, DialogueSugarMode, dialogue_text_sugar_edits};
use crate::edit::report_from_edits;
use crate::model::{FormatOptions, TextEdit, ToolingDiagnostic, ToolingEditReport, ToolingError};
use crate::sugar_expansion::sugar_expansion_edits;
use arcweft_lang_syntax::parser::{ParseOptions, SourceDialect, parse_document, parse_source};

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
    if dialect == SourceDialect::Agent && options.expand_sugar {
        return Err(ToolingError::UnsupportedFormatOption {
            option: "expand_sugar",
            dialect: "Agent",
        });
    }
    if dialect == SourceDialect::Agent && options.canonical_rich_text {
        return Err(ToolingError::UnsupportedFormatOption {
            option: "canonical_rich_text",
            dialect: "Agent",
        });
    }
    let mut edits = Vec::new();
    if options.expand_sugar {
        edits.extend(sugar_expansion_edits(source));
    } else if options.canonical_rich_text {
        edits.extend(rich_text_canonical_edits(source));
    }
    let mut report = report_from_edits(source, edits)?;
    if dialect == SourceDialect::Agent {
        report.diagnostics = agent_format_diagnostics(source);
    }
    Ok(report)
}

fn rich_text_canonical_edits(source: &str) -> Vec<TextEdit> {
    let parsed = parse_source(source);
    let context = DialogueSugarContext::from_parsed(&parsed);
    dialogue_text_sugar_edits(source, &parsed, DialogueSugarMode::RichTextOnly, &context)
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
    .map(|error| ToolingDiagnostic {
        message: error.message().to_owned(),
        start: error.range().start(),
        end: error.range().end(),
    })
    .collect()
}
