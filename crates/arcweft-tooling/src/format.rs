use std::sync::Arc;

use arcweft_lang_syntax::{
    attachment::{
        AttachedCallableContentParameter, AttachedCapabilityMember, AttachedContentPresenceSyntax,
        AttachedContentRoleSyntax, AttachedImplMember, AttachedPathRoot, AttachedStyleBody,
        AttachedStyleEnvironment, AttachedStyleEnvironmentClause, AttachedStyleExpression,
        AttachedStyleMember, AttachedTraitMember, SyntaxAccessError, TypedItemNode,
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
        canonical_callable_edits(source, &parsed)
            .map_err(|error| syntax_attachment_error(&error))?,
    );
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

/// Canonicalizes the dedicated trailing attached-content declaration owned by
/// every admitted callable family. The declaration is already a typed syntax
/// node; this projection only normalizes punctuation spacing and never scans
/// source bytes for an attribute or an ordinary parameter alias.
fn canonical_callable_edits(
    source: &str,
    parsed: &ParsedSource,
) -> Result<Vec<TextEdit>, SyntaxAccessError> {
    let mut edits = Vec::new();
    for item in parsed.items()? {
        match item {
            TypedItemNode::Function(function) => {
                let declaration = function.semantics()?;
                push_attached_content_edit(source, declaration.attached_content(), &mut edits);
            }
            TypedItemNode::Trait(trait_item) => {
                let declaration = trait_item.semantics()?;
                for member in declaration.body().members() {
                    let AttachedTraitMember::Function(function) = member else {
                        continue;
                    };
                    push_attached_content_edit(source, function.attached_content(), &mut edits);
                }
            }
            TypedItemNode::Impl(impl_item) => {
                let declaration = impl_item.semantics()?;
                for member in declaration.body().members() {
                    let AttachedImplMember::Function(function) = member else {
                        continue;
                    };
                    push_attached_content_edit(source, function.attached_content(), &mut edits);
                }
            }
            TypedItemNode::ExternCapability(capability) => {
                let declaration = capability.semantics()?;
                for member in declaration.body().members() {
                    let AttachedCapabilityMember::Function(function) = member else {
                        continue;
                    };
                    push_attached_content_edit(source, function.attached_content(), &mut edits);
                }
            }
            _ => {}
        }
    }
    Ok(edits)
}

fn push_attached_content_edit(
    source: &str,
    attached: Option<&AttachedCallableContentParameter>,
    edits: &mut Vec<TextEdit>,
) {
    let Some(attached) = attached.filter(|attached| !attached.has_recovery()) else {
        return;
    };
    let Some(binding) = attached.binding().value() else {
        return;
    };
    let presence = match attached.presence() {
        AttachedContentPresenceSyntax::Required => "",
        AttachedContentPresenceSyntax::Optional { .. } => "?",
        AttachedContentPresenceSyntax::Defaulted { value, .. } => {
            let Some(value) = source.get(value.syntax().range().as_range()) else {
                return;
            };
            let role = attached_content_role(attached.role());
            let replacement = format!("[{}: {} = {}]", binding.as_str(), role, value.trim(),);
            push_if_changed(
                source,
                attached.syntax().range().start(),
                attached.syntax().range().end(),
                replacement,
                edits,
            );
            return;
        }
    };
    let replacement = format!(
        "[{}{}: {}]",
        binding.as_str(),
        presence,
        attached_content_role(attached.role()),
    );
    push_if_changed(
        source,
        attached.syntax().range().start(),
        attached.syntax().range().end(),
        replacement,
        edits,
    );
}

const fn attached_content_role(role: AttachedContentRoleSyntax) -> &'static str {
    match role {
        AttachedContentRoleSyntax::InlineContent => "InlineContent",
        AttachedContentRoleSyntax::RichContent => "RichContent",
        AttachedContentRoleSyntax::DialogueContent => "DialogueContent",
    }
}

fn push_if_changed(
    source: &str,
    start: usize,
    end: usize,
    replacement: String,
    edits: &mut Vec<TextEdit>,
) {
    if source
        .get(start..end)
        .is_some_and(|authored| authored != replacement)
    {
        edits.push(TextEdit {
            start,
            end,
            replacement,
        });
    }
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
