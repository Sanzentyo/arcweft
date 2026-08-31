use std::sync::Arc;

use arcweft_lang_syntax::{
    attachment::{
        AttachedPathRoot, AttachedStyleBody, AttachedStyleEnvironment,
        AttachedStyleEnvironmentClause, AttachedStyleExpression, AttachedStyleMember,
        SyntaxAccessError, TypedItemNode,
    },
    expressions::ExpressionProjection,
    incremental::{ParsedSource, SyntaxDatabase},
    literal::{SyntaxLiteralValue, UnitNumberSuffix},
    parser::ParseOptions,
};
use arcweft_source::{SourceDocument, identity::SourceSnapshotId};

use crate::edit::report_from_edits;
use crate::model::{FormatOptions, TextEdit, ToolingDiagnostic, ToolingEditReport, ToolingError};

mod view;

/// Formats an exact source document while preserving authoring sugar by default.
pub fn format_document(
    document: Arc<SourceDocument>,
    _options: FormatOptions,
) -> Result<ToolingEditReport, ToolingError> {
    let mut syntax =
        SyntaxDatabase::try_new().map_err(|error| ToolingError::SyntaxDatabaseUnavailable {
            message: error.to_string(),
        })?;
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(document.display_name().clone()),
            document,
            ParseOptions::default(),
        )
        .map_err(|error| ToolingError::SyntaxAttachmentFailed {
            message: error.to_string(),
        })?;
    let source = parsed.source();
    let mut edits =
        view::canonical_edits(source, &parsed).map_err(|error| syntax_attachment_error(&error))?;
    edits.extend(
        canonical_environment_edits(source, &parsed)
            .map_err(|error| syntax_attachment_error(&error))?,
    );
    let mut report = report_from_edits(source, edits)?;
    report.diagnostics = parsed
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            let range = diagnostic.primary().range();
            ToolingDiagnostic::syntax(diagnostic.message(), range.start(), range.end())
        })
        .collect();
    Ok(report)
}

fn syntax_attachment_error(error: &SyntaxAccessError) -> ToolingError {
    ToolingError::SyntaxAttachmentFailed {
        message: error.to_string(),
    }
}

fn canonical_environment_edits(
    source: &str,
    parsed: &ParsedSource,
) -> Result<Vec<TextEdit>, SyntaxAccessError> {
    let mut edits = Vec::new();
    for item in parsed.items()? {
        let TypedItemNode::Style(style) = item else {
            continue;
        };
        collect_style_body_edits(source, style.semantics()?.body(), &mut edits)?;
    }
    Ok(edits)
}

fn collect_style_body_edits(
    source: &str,
    body: &AttachedStyleBody,
    edits: &mut Vec<TextEdit>,
) -> Result<(), SyntaxAccessError> {
    for member in body.members() {
        let AttachedStyleMember::Environment(environment) = member else {
            continue;
        };
        if let Some(edit) = canonical_environment_edit(source, environment) {
            edits.push(edit);
        }
        collect_style_body_edits(source, environment.body(), edits)?;
    }
    Ok(())
}

fn canonical_environment_edit(
    source: &str,
    environment: &AttachedStyleEnvironment,
) -> Option<TextEdit> {
    let condition = environment.condition();
    if condition.has_recovery() || condition.clauses().is_empty() {
        return None;
    }
    let mut clauses = condition.clauses().iter().collect::<Vec<_>>();
    clauses.sort_by_key(|clause| clause.field().value());
    let canonical = clauses
        .into_iter()
        .map(canonical_environment_clause)
        .collect::<Option<Vec<_>>>()?;
    let replacement = if canonical.len() == 1 {
        canonical[0].clone()
    } else {
        let indentation = line_indentation(source, environment.syntax().range().start());
        let clause_indentation = format!("{indentation}    ");
        let mut replacement = String::new();
        replacement.push('\n');
        for clause in canonical {
            replacement.push_str(&clause_indentation);
            replacement.push_str(&clause);
            replacement.push_str(",\n");
        }
        replacement.push_str(indentation);
        replacement
    };
    let start = condition.open_delimiter().range().end();
    let end = condition.close_delimiter().range().start();
    (source.get(start..end)? != replacement).then_some(TextEdit {
        start,
        end,
        replacement,
    })
}

fn canonical_environment_clause(clause: &AttachedStyleEnvironmentClause) -> Option<String> {
    use arcweft_lang_syntax::attachment::{
        StyleEnvironmentComparisonKind as Comparison, StyleEnvironmentFieldKind as Field,
    };

    let field = match clause.field().value()? {
        Field::ColorScheme => "color-scheme",
        Field::Contrast => "contrast",
        Field::ReducedMotion => "reduced-motion",
        Field::TextScale => "text-scale",
    };
    let comparison = match clause.comparison().value()? {
        Comparison::Equal => "==",
        Comparison::NotEqual => "!=",
        Comparison::Less => "<",
        Comparison::LessOrEqual => "<=",
        Comparison::Greater => ">",
        Comparison::GreaterOrEqual => ">=",
    };
    let AttachedStyleExpression::Authored(value) = clause.value() else {
        return None;
    };
    let value = match value.projection() {
        ExpressionProjection::Path => {
            let path = value.path()?;
            if path.has_recovery()
                || !matches!(path.root(), AttachedPathRoot::ImplicitCrate)
                || path.segments().len() != 1
            {
                return None;
            }
            path.segments()[0].source_text().to_ascii_lowercase()
        }
        ExpressionProjection::Literal(literal) => match literal.value() {
            SyntaxLiteralValue::Bool(value) => value.to_string(),
            SyntaxLiteralValue::Unit {
                value,
                unit: UnitNumberSuffix::Percent,
            } if value.exponent().is_none() && value.suffix().is_none() => {
                canonical_percentage(value.integral_digits(), value.fractional_digits())
            }
            _ => return None,
        },
        _ => return None,
    };
    Some(format!("{field} {comparison} {value}"))
}

fn canonical_percentage(integral: &str, fractional: Option<&str>) -> String {
    let integral = integral.trim_start_matches('0');
    let integral = if integral.is_empty() { "0" } else { integral };
    match fractional {
        None | Some("0") => format!("{integral}%"),
        Some(fractional) => format!("{integral}.{fractional}%"),
    }
}

fn line_indentation(source: &str, offset: usize) -> &str {
    let line_start = source[..offset]
        .rfind('\n')
        .map_or(0, |newline| newline + '\n'.len_utf8());
    let prefix = &source[line_start..offset];
    let indentation_end = prefix
        .find(|ch: char| !matches!(ch, ' ' | '\t'))
        .unwrap_or(prefix.len());
    &prefix[..indentation_end]
}
