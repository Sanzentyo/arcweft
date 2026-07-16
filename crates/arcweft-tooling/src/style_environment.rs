//! Transport-neutral formatting helpers for native Style environment guards.

use arcweft_lang_hir::style::{HirStyleEnvironmentId, HirStyleId};
use arcweft_lang_sema::style::{CheckedStyleEnvironmentClause, CheckedStyleEnvironmentPath};
use arcweft_lang_syntax::{
    ast::{
        common::TextRange,
        items::Item,
        style::{
            StyleBodyItem, StyleEnvironmentBlock, StyleEnvironmentClause,
            StyleEnvironmentComparisonSyntax, StyleEnvironmentFieldSyntax,
            StyleEnvironmentUnsupportedValueKind, StyleEnvironmentValueSyntax,
        },
    },
    cst::SyntaxNode,
    source::ParsedSource,
};
use arcweft_presentation::appearance::{
    ColorScheme, ContrastPreference, PresentationEnvironmentField, PresentationEnvironmentFieldSet,
};

use crate::model::TextEdit;

/// Required syntax input for formatting one environment wrapper.
#[derive(Clone, Copy, Debug)]
pub struct StyleEnvironmentFormatInput<'a> {
    pub node: &'a StyleEnvironmentBlock,
    pub cst: &'a SyntaxNode,
}

/// Deterministic edit result for one environment wrapper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleEnvironmentFormatResult {
    pub edits: Box<[TextEdit]>,
    pub canonical: bool,
}

/// Typed completion location within an environment condition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleEnvironmentCompletionSite {
    Field {
        used_on_path: PresentationEnvironmentFieldSet,
    },
    Comparison {
        field: PresentationEnvironmentField,
    },
    Value {
        field: PresentationEnvironmentField,
    },
    Delimiter,
}

/// Transport-neutral completion request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StyleEnvironmentCompletionInput {
    pub site: StyleEnvironmentCompletionSite,
    pub replace: TextRange,
}

/// Semantic class of one environment completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleEnvironmentCompletionKind {
    Field,
    Operator,
    EnumValue,
    Boolean,
    Number,
    Punctuation,
}

/// One closed, canonical completion item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StyleEnvironmentCompletionItem {
    pub label: &'static str,
    pub insert_text: &'static str,
    pub replace: TextRange,
    pub kind: StyleEnvironmentCompletionKind,
}

/// Byte-position hover request over one typed environment wrapper.
#[derive(Clone, Copy, Debug)]
pub struct StyleEnvironmentHoverInput<'a> {
    pub position: usize,
    pub ast: &'a StyleEnvironmentBlock,
    pub checked: Option<&'a CheckedStyleEnvironmentPath>,
}

/// Typed hover subject independent from LSP markup types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleEnvironmentHoverSubject {
    Wrapper,
    Field(PresentationEnvironmentField),
    Comparison(PresentationEnvironmentField),
    Value(PresentationEnvironmentField),
    Recovered,
}

/// One source-backed environment hover.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleEnvironmentHover {
    pub range: TextRange,
    pub subject: StyleEnvironmentHoverSubject,
    pub markdown: String,
}

/// Repository-owned semantic-token class for environment syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleEnvironmentSemanticKind {
    Keyword,
    Intrinsic,
    Field,
    Operator,
    EnumValue,
    Boolean,
    Number,
    Unit,
    Punctuation,
    Recovered,
}

/// One exact typed source range and its environment semantic class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StyleEnvironmentSemanticSpan {
    pub range: TextRange,
    pub kind: StyleEnvironmentSemanticKind,
}

/// Stable diagnostic identity attached to a tooling action.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ToolingDiagnosticId(Box<str>);

impl ToolingDiagnosticId {
    pub fn new(value: impl Into<Box<str>>) -> Option<Self> {
        let value = value.into();
        (!value.is_empty()).then_some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Narrow safe repair supported for environment syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleEnvironmentCodeActionKind {
    CanonicalizeFieldOrder,
    ReplaceWithEquality,
    AddPercentUnit,
}

/// Transport-neutral environment code action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleEnvironmentCodeAction {
    pub kind: StyleEnvironmentCodeActionKind,
    pub title: &'static str,
    pub edits: Box<[TextEdit]>,
    pub diagnostics: Box<[ToolingDiagnosticId]>,
}

/// Required typed state for environment repairs.
#[derive(Clone, Copy, Debug)]
pub struct StyleEnvironmentCodeActionInput<'a> {
    pub node: &'a StyleEnvironmentBlock,
    pub source: &'a str,
    pub diagnostics: &'a [ToolingDiagnosticId],
}

/// Definition-like intrinsic target owned by the native Style language.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleEnvironmentIntrinsicTarget {
    EnvironmentWrapper,
    ColorScheme,
    Contrast,
    ReducedMotion,
    TextScale,
    Light,
    Dark,
    StandardContrast,
    MoreContrast,
    BooleanTrue,
    BooleanFalse,
    PercentageUnit,
}

/// Source origin and typed intrinsic navigation target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StyleEnvironmentNavigationResult {
    pub origin: TextRange,
    pub target: StyleEnvironmentIntrinsicTarget,
}

/// Typed semantic edit class supplied by the incremental HIR differ.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleEnvironmentSemanticEdit {
    Unchanged,
    Clause,
    WrapperAncestry,
}

/// Stable identities required to classify one environment edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StyleEnvironmentEditInput {
    pub sheet: HirStyleId,
    pub environment: HirStyleEnvironmentId,
    pub edit: StyleEnvironmentSemanticEdit,
}

/// Exact environment invalidation boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleEnvironmentEditInvalidation {
    Subtree {
        sheet: HirStyleId,
        environment: HirStyleEnvironmentId,
    },
    Sheet {
        sheet: HirStyleId,
    },
}

/// Produces the smallest condition-content edit for one environment wrapper.
#[must_use]
pub fn format_style_environment(
    input: StyleEnvironmentFormatInput<'_>,
) -> StyleEnvironmentFormatResult {
    let root = input
        .cst
        .ancestors()
        .last()
        .unwrap_or_else(|| input.cst.clone());
    let source = root.to_string();
    let Some(edit) = canonical_condition_edit(input.node, &source) else {
        return StyleEnvironmentFormatResult {
            edits: Box::new([]),
            canonical: true,
        };
    };
    StyleEnvironmentFormatResult {
        canonical: false,
        edits: Box::new([edit]),
    }
}

/// Returns only canonical candidates valid for the typed completion site.
#[must_use]
pub fn complete_style_environment(
    input: StyleEnvironmentCompletionInput,
) -> Box<[StyleEnvironmentCompletionItem]> {
    let mut items = Vec::new();
    match input.site {
        StyleEnvironmentCompletionSite::Field { used_on_path } => {
            for (field, label) in [
                (PresentationEnvironmentField::ColorScheme, "color-scheme"),
                (PresentationEnvironmentField::Contrast, "contrast"),
                (
                    PresentationEnvironmentField::ReducedMotion,
                    "reduced-motion",
                ),
                (PresentationEnvironmentField::TextScale, "text-scale"),
            ] {
                if !used_on_path.contains(field) {
                    items.push(completion_item(
                        label,
                        input.replace,
                        StyleEnvironmentCompletionKind::Field,
                    ));
                }
            }
        }
        StyleEnvironmentCompletionSite::Comparison { field } => {
            let comparisons: &[&str] = if field == PresentationEnvironmentField::TextScale {
                &["==", "!=", "<", "<=", ">", ">="]
            } else {
                &["=="]
            };
            items.extend(comparisons.iter().map(|comparison| {
                completion_item(
                    comparison,
                    input.replace,
                    StyleEnvironmentCompletionKind::Operator,
                )
            }));
        }
        StyleEnvironmentCompletionSite::Value { field } => {
            let values: &[(&str, StyleEnvironmentCompletionKind)] = match field {
                PresentationEnvironmentField::ColorScheme => &[
                    ("light", StyleEnvironmentCompletionKind::EnumValue),
                    ("dark", StyleEnvironmentCompletionKind::EnumValue),
                ],
                PresentationEnvironmentField::Contrast => &[
                    ("standard", StyleEnvironmentCompletionKind::EnumValue),
                    ("more", StyleEnvironmentCompletionKind::EnumValue),
                ],
                PresentationEnvironmentField::ReducedMotion => &[
                    ("true", StyleEnvironmentCompletionKind::Boolean),
                    ("false", StyleEnvironmentCompletionKind::Boolean),
                ],
                PresentationEnvironmentField::TextScale => &[
                    ("50%", StyleEnvironmentCompletionKind::Number),
                    ("100%", StyleEnvironmentCompletionKind::Number),
                    ("125%", StyleEnvironmentCompletionKind::Number),
                    ("200%", StyleEnvironmentCompletionKind::Number),
                    ("400%", StyleEnvironmentCompletionKind::Number),
                ],
            };
            items.extend(
                values
                    .iter()
                    .map(|(value, kind)| completion_item(value, input.replace, *kind)),
            );
        }
        StyleEnvironmentCompletionSite::Delimiter => items.push(completion_item(
            ",",
            input.replace,
            StyleEnvironmentCompletionKind::Punctuation,
        )),
    }
    items.into_boxed_slice()
}

/// Returns the most specific typed environment hover at a byte offset.
#[must_use]
pub fn hover_style_environment(
    input: StyleEnvironmentHoverInput<'_>,
) -> Option<StyleEnvironmentHover> {
    let clause = input
        .ast
        .clauses()
        .iter()
        .find(|clause| range_contains(clause.range(), input.position));
    let Some(clause) = clause else {
        return (range_contains(input.ast.when_range(), input.position)
            || range_contains(input.ast.intrinsic_range(), input.position)
            || range_contains(input.ast.condition_range(), input.position))
        .then(|| StyleEnvironmentHover {
            range: input.ast.range(),
            subject: StyleEnvironmentHoverSubject::Wrapper,
            markdown: "Native Style guard evaluated against the checked presentation environment."
                .to_owned(),
        });
    };
    let field = syntax_field(clause.field());
    if range_contains(clause.field_range(), input.position) {
        return Some(field.map_or_else(
            || recovered_hover(clause.field_range()),
            |field| StyleEnvironmentHover {
                range: clause.field_range(),
                subject: StyleEnvironmentHoverSubject::Field(field),
                markdown: field_markdown(field).to_owned(),
            },
        ));
    }
    if range_contains(clause.comparison_range(), input.position) {
        return Some(field.map_or_else(
            || recovered_hover(clause.comparison_range()),
            |field| StyleEnvironmentHover {
                range: clause.comparison_range(),
                subject: StyleEnvironmentHoverSubject::Comparison(field),
                markdown: comparison_markdown(field).to_owned(),
            },
        ));
    }
    if range_contains(clause.value_range(), input.position) {
        let checked = input.checked.and_then(|path| {
            path.clauses()
                .iter()
                .copied()
                .find(|checked| ranges_overlap(checked.range(), clause.range()))
        });
        return Some(match (field, checked) {
            (Some(field), Some(checked)) => StyleEnvironmentHover {
                range: checked.range(),
                subject: StyleEnvironmentHoverSubject::Value(field),
                markdown: checked_clause_markdown(checked),
            },
            (Some(field), None) => StyleEnvironmentHover {
                range: clause.value_range(),
                subject: StyleEnvironmentHoverSubject::Value(field),
                markdown: recovered_value_markdown(field, clause.value()),
            },
            (None, _) => recovered_hover(clause.value_range()),
        });
    }
    Some(field.map_or_else(
        || recovered_hover(clause.range()),
        |field| StyleEnvironmentHover {
            range: clause.range(),
            subject: StyleEnvironmentHoverSubject::Field(field),
            markdown: field_markdown(field).to_owned(),
        },
    ))
}

/// Emits semantic spans exclusively from typed AST ranges.
#[must_use]
pub fn style_environment_semantic_spans(
    node: &StyleEnvironmentBlock,
) -> Box<[StyleEnvironmentSemanticSpan]> {
    let mut spans = vec![
        StyleEnvironmentSemanticSpan {
            range: node.when_range(),
            kind: StyleEnvironmentSemanticKind::Keyword,
        },
        StyleEnvironmentSemanticSpan {
            range: node.intrinsic_range(),
            kind: StyleEnvironmentSemanticKind::Intrinsic,
        },
    ];
    let condition = node.condition_range();
    if condition.end() > condition.start() {
        spans.push(StyleEnvironmentSemanticSpan {
            range: TextRange::new(condition.start(), condition.start() + 1),
            kind: StyleEnvironmentSemanticKind::Punctuation,
        });
        if node.condition_closed() && condition.end() > condition.start() + 1 {
            spans.push(StyleEnvironmentSemanticSpan {
                range: TextRange::new(condition.end() - 1, condition.end()),
                kind: StyleEnvironmentSemanticKind::Punctuation,
            });
        }
    }
    for clause in node.clauses() {
        spans.push(StyleEnvironmentSemanticSpan {
            range: clause.field_range(),
            kind: if clause.field() == StyleEnvironmentFieldSyntax::Unknown {
                StyleEnvironmentSemanticKind::Recovered
            } else {
                StyleEnvironmentSemanticKind::Field
            },
        });
        spans.push(StyleEnvironmentSemanticSpan {
            range: clause.comparison_range(),
            kind: if clause.comparison() == StyleEnvironmentComparisonSyntax::Unsupported {
                StyleEnvironmentSemanticKind::Recovered
            } else {
                StyleEnvironmentSemanticKind::Operator
            },
        });
        match clause.value() {
            StyleEnvironmentValueSyntax::Identifier { range } => {
                spans.push(StyleEnvironmentSemanticSpan {
                    range: *range,
                    kind: StyleEnvironmentSemanticKind::EnumValue,
                });
            }
            StyleEnvironmentValueSyntax::Boolean { range, .. } => {
                spans.push(StyleEnvironmentSemanticSpan {
                    range: *range,
                    kind: StyleEnvironmentSemanticKind::Boolean,
                });
            }
            StyleEnvironmentValueSyntax::Percentage(percentage) => {
                spans.push(StyleEnvironmentSemanticSpan {
                    range: TextRange::new(
                        percentage.integer_range().start(),
                        percentage.percent_range().start(),
                    ),
                    kind: StyleEnvironmentSemanticKind::Number,
                });
                spans.push(StyleEnvironmentSemanticSpan {
                    range: percentage.percent_range(),
                    kind: StyleEnvironmentSemanticKind::Unit,
                });
            }
            StyleEnvironmentValueSyntax::Unsupported(value) => {
                spans.push(StyleEnvironmentSemanticSpan {
                    range: value.range(),
                    kind: StyleEnvironmentSemanticKind::Recovered,
                });
            }
        }
    }
    spans.sort_by_key(|span| (span.range.start(), span.range.end()));
    spans.into_boxed_slice()
}

/// Computes only repairs that are provably semantics preserving or complete a checked literal.
#[must_use]
pub fn style_environment_code_actions(
    input: StyleEnvironmentCodeActionInput<'_>,
) -> Box<[StyleEnvironmentCodeAction]> {
    let mut actions = Vec::new();
    if clauses_are_complete_and_distinct(input.node)
        && let Some(edit) = canonical_condition_edit(input.node, input.source)
    {
        actions.push(StyleEnvironmentCodeAction {
            kind: StyleEnvironmentCodeActionKind::CanonicalizeFieldOrder,
            title: "Canonicalize environment field order",
            edits: Box::new([edit]),
            diagnostics: input.diagnostics.to_vec().into_boxed_slice(),
        });
    }
    for clause in input.node.clauses() {
        if equality_repair_is_safe(clause) {
            actions.push(StyleEnvironmentCodeAction {
                kind: StyleEnvironmentCodeActionKind::ReplaceWithEquality,
                title: "Replace with equality comparison",
                edits: Box::new([TextEdit {
                    start: clause.comparison_range().start(),
                    end: clause.comparison_range().end(),
                    replacement: "==".to_owned(),
                }]),
                diagnostics: input.diagnostics.to_vec().into_boxed_slice(),
            });
        }
        if percent_unit_repair_is_safe(clause, input.source) {
            actions.push(StyleEnvironmentCodeAction {
                kind: StyleEnvironmentCodeActionKind::AddPercentUnit,
                title: "Add percent unit",
                edits: Box::new([TextEdit {
                    start: clause.value_range().end(),
                    end: clause.value_range().end(),
                    replacement: "%".to_owned(),
                }]),
                diagnostics: input.diagnostics.to_vec().into_boxed_slice(),
            });
        }
    }
    actions.into_boxed_slice()
}

/// Resolves an environment token to an intrinsic language target without inventing a URI.
#[must_use]
pub fn navigate_style_environment(
    position: usize,
    ast: &StyleEnvironmentBlock,
    checked: Option<&CheckedStyleEnvironmentPath>,
) -> Option<StyleEnvironmentNavigationResult> {
    if range_contains(ast.when_range(), position) || range_contains(ast.intrinsic_range(), position)
    {
        return Some(StyleEnvironmentNavigationResult {
            origin: if range_contains(ast.when_range(), position) {
                ast.when_range()
            } else {
                ast.intrinsic_range()
            },
            target: StyleEnvironmentIntrinsicTarget::EnvironmentWrapper,
        });
    }
    let clause = ast
        .clauses()
        .iter()
        .find(|clause| range_contains(clause.range(), position))?;
    let field = syntax_field(clause.field())?;
    if range_contains(clause.field_range(), position)
        || range_contains(clause.comparison_range(), position)
    {
        return Some(StyleEnvironmentNavigationResult {
            origin: if range_contains(clause.field_range(), position) {
                clause.field_range()
            } else {
                clause.comparison_range()
            },
            target: field_target(field),
        });
    }
    if !range_contains(clause.value_range(), position) {
        return None;
    }
    if let StyleEnvironmentValueSyntax::Percentage(percentage) = clause.value()
        && range_contains(percentage.percent_range(), position)
    {
        return Some(StyleEnvironmentNavigationResult {
            origin: percentage.percent_range(),
            target: StyleEnvironmentIntrinsicTarget::PercentageUnit,
        });
    }
    let checked_target = checked.and_then(|path| {
        path.clauses()
            .iter()
            .copied()
            .find(|checked| ranges_overlap(checked.range(), clause.range()))
            .map(checked_value_target)
    });
    let target = checked_target.unwrap_or_else(|| match clause.value() {
        StyleEnvironmentValueSyntax::Boolean { value: true, .. } => {
            StyleEnvironmentIntrinsicTarget::BooleanTrue
        }
        StyleEnvironmentValueSyntax::Boolean { value: false, .. } => {
            StyleEnvironmentIntrinsicTarget::BooleanFalse
        }
        _ => field_target(field),
    });
    Some(StyleEnvironmentNavigationResult {
        origin: clause.value_range(),
        target,
    })
}

/// Maps a typed incremental HIR edit to its smallest valid invalidation boundary.
#[must_use]
pub const fn style_environment_edit_invalidation(
    input: StyleEnvironmentEditInput,
) -> Option<StyleEnvironmentEditInvalidation> {
    match input.edit {
        StyleEnvironmentSemanticEdit::Unchanged => None,
        StyleEnvironmentSemanticEdit::Clause => Some(StyleEnvironmentEditInvalidation::Subtree {
            sheet: input.sheet,
            environment: input.environment,
        }),
        StyleEnvironmentSemanticEdit::WrapperAncestry => {
            Some(StyleEnvironmentEditInvalidation::Sheet { sheet: input.sheet })
        }
    }
}

pub(crate) fn canonical_environment_edits(parsed: &ParsedSource) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    for item in parsed.typed_tree().items() {
        let Item::Style(style) = item else {
            continue;
        };
        collect_body_edits(style.sheet().body(), parsed.source(), &mut edits);
    }
    edits
}

fn collect_body_edits(body: &[StyleBodyItem], source: &str, edits: &mut Vec<TextEdit>) {
    for item in body {
        let StyleBodyItem::Environment(environment) = item else {
            continue;
        };
        if let Some(edit) = canonical_condition_edit(environment, source) {
            edits.push(edit);
        }
        collect_body_edits(environment.body(), source, edits);
    }
}

fn canonical_condition_edit(environment: &StyleEnvironmentBlock, source: &str) -> Option<TextEdit> {
    let range = environment.condition_range();
    if range.end() <= range.start() + 1
        || source.get(range.start()..range.start() + 1) != Some("(")
        || source.get(range.end() - 1..range.end()) != Some(")")
    {
        return None;
    }
    let mut clauses = environment.clauses().iter().collect::<Vec<_>>();
    if clauses.is_empty()
        || clauses.iter().any(|clause| {
            clause.field() == StyleEnvironmentFieldSyntax::Unknown
                || clause.comparison() == StyleEnvironmentComparisonSyntax::Unsupported
                || matches!(clause.value(), StyleEnvironmentValueSyntax::Unsupported(_))
        })
    {
        return None;
    }
    clauses.sort_by_key(|clause| clause.field());
    let canonical_clauses = clauses
        .into_iter()
        .map(|clause| canonical_clause(clause, source))
        .collect::<Option<Vec<_>>>()?;
    let replacement = if canonical_clauses.len() == 1 {
        canonical_clauses[0].clone()
    } else {
        let indentation = line_indentation(source, environment.range().start());
        let clause_indentation = format!("{indentation}    ");
        let mut replacement = String::new();
        replacement.push('\n');
        for clause in canonical_clauses {
            replacement.push_str(&clause_indentation);
            replacement.push_str(&clause);
            replacement.push_str(",\n");
        }
        replacement.push_str(indentation);
        replacement
    };
    let start = range.start() + 1;
    let end = range.end() - 1;
    (source.get(start..end) != Some(replacement.as_str())).then_some(TextEdit {
        start,
        end,
        replacement,
    })
}

fn canonical_clause(clause: &StyleEnvironmentClause, source: &str) -> Option<String> {
    let field = match clause.field() {
        StyleEnvironmentFieldSyntax::ColorScheme => "color-scheme",
        StyleEnvironmentFieldSyntax::Contrast => "contrast",
        StyleEnvironmentFieldSyntax::ReducedMotion => "reduced-motion",
        StyleEnvironmentFieldSyntax::TextScale => "text-scale",
        StyleEnvironmentFieldSyntax::Unknown => return None,
    };
    let comparison = match clause.comparison() {
        StyleEnvironmentComparisonSyntax::Equal => "==",
        StyleEnvironmentComparisonSyntax::NotEqual => "!=",
        StyleEnvironmentComparisonSyntax::Less => "<",
        StyleEnvironmentComparisonSyntax::LessOrEqual => "<=",
        StyleEnvironmentComparisonSyntax::Greater => ">",
        StyleEnvironmentComparisonSyntax::GreaterOrEqual => ">=",
        StyleEnvironmentComparisonSyntax::Unsupported => return None,
    };
    let value = match clause.value() {
        StyleEnvironmentValueSyntax::Identifier { range } => {
            source.get(range.as_range())?.to_ascii_lowercase()
        }
        StyleEnvironmentValueSyntax::Boolean { value, .. } => value.to_string(),
        StyleEnvironmentValueSyntax::Percentage(percentage) => {
            let integer = source.get(percentage.integer_range().as_range())?;
            let integer = integer.trim_start_matches('0');
            let integer = if integer.is_empty() { "0" } else { integer };
            match percentage
                .fractional_range()
                .and_then(|range| source.get(range.as_range()))
            {
                None | Some("0") => format!("{integer}%"),
                Some(fractional) => format!("{integer}.{fractional}%"),
            }
        }
        StyleEnvironmentValueSyntax::Unsupported(_) => return None,
    };
    Some(format!("{field} {comparison} {value}"))
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

const fn completion_item(
    label: &'static str,
    replace: TextRange,
    kind: StyleEnvironmentCompletionKind,
) -> StyleEnvironmentCompletionItem {
    StyleEnvironmentCompletionItem {
        label,
        insert_text: label,
        replace,
        kind,
    }
}

const fn syntax_field(field: StyleEnvironmentFieldSyntax) -> Option<PresentationEnvironmentField> {
    match field {
        StyleEnvironmentFieldSyntax::ColorScheme => Some(PresentationEnvironmentField::ColorScheme),
        StyleEnvironmentFieldSyntax::Contrast => Some(PresentationEnvironmentField::Contrast),
        StyleEnvironmentFieldSyntax::ReducedMotion => {
            Some(PresentationEnvironmentField::ReducedMotion)
        }
        StyleEnvironmentFieldSyntax::TextScale => Some(PresentationEnvironmentField::TextScale),
        StyleEnvironmentFieldSyntax::Unknown => None,
    }
}

const fn field_target(field: PresentationEnvironmentField) -> StyleEnvironmentIntrinsicTarget {
    match field {
        PresentationEnvironmentField::ColorScheme => StyleEnvironmentIntrinsicTarget::ColorScheme,
        PresentationEnvironmentField::Contrast => StyleEnvironmentIntrinsicTarget::Contrast,
        PresentationEnvironmentField::ReducedMotion => {
            StyleEnvironmentIntrinsicTarget::ReducedMotion
        }
        PresentationEnvironmentField::TextScale => StyleEnvironmentIntrinsicTarget::TextScale,
    }
}

const fn field_markdown(field: PresentationEnvironmentField) -> &'static str {
    match field {
        PresentationEnvironmentField::ColorScheme => {
            "`color-scheme` is a closed `light | dark` environment field."
        }
        PresentationEnvironmentField::Contrast => {
            "`contrast` is a closed `standard | more` environment field."
        }
        PresentationEnvironmentField::ReducedMotion => {
            "`reduced-motion` is a boolean environment field."
        }
        PresentationEnvironmentField::TextScale => {
            "`text-scale` is a checked percentage in `50%..=400%`."
        }
    }
}

const fn comparison_markdown(field: PresentationEnvironmentField) -> &'static str {
    if matches!(field, PresentationEnvironmentField::TextScale) {
        "Text scale supports `==`, `!=`, `<`, `<=`, `>`, and `>=`."
    } else {
        "Enum and boolean environment fields support equality only."
    }
}

fn recovered_hover(range: TextRange) -> StyleEnvironmentHover {
    StyleEnvironmentHover {
        range,
        subject: StyleEnvironmentHoverSubject::Recovered,
        markdown: "Recovered environment syntax; this partial node is not executable.".to_owned(),
    }
}

fn recovered_value_markdown(
    field: PresentationEnvironmentField,
    value: &StyleEnvironmentValueSyntax,
) -> String {
    let state = if matches!(value, StyleEnvironmentValueSyntax::Unsupported(_)) {
        "recovered"
    } else {
        "syntactically typed"
    };
    format!("{} Current value is {state}.", field_markdown(field))
}

fn checked_clause_markdown(clause: CheckedStyleEnvironmentClause) -> String {
    match clause {
        CheckedStyleEnvironmentClause::ColorScheme { value, .. } => format!(
            "Checked `color-scheme == {}`.",
            match value {
                ColorScheme::Light => "light",
                ColorScheme::Dark => "dark",
            }
        ),
        CheckedStyleEnvironmentClause::Contrast { value, .. } => format!(
            "Checked `contrast == {}`.",
            match value {
                ContrastPreference::Standard => "standard",
                ContrastPreference::More => "more",
            }
        ),
        CheckedStyleEnvironmentClause::ReducedMotion { value, .. } => {
            format!("Checked `reduced-motion == {value}`.")
        }
        CheckedStyleEnvironmentClause::TextScale {
            comparison, value, ..
        } => format!(
            "Checked text-scale comparison `{comparison:?}` against `{}milli`.",
            value.value()
        ),
    }
}

const fn checked_value_target(
    clause: CheckedStyleEnvironmentClause,
) -> StyleEnvironmentIntrinsicTarget {
    match clause {
        CheckedStyleEnvironmentClause::ColorScheme {
            value: ColorScheme::Light,
            ..
        } => StyleEnvironmentIntrinsicTarget::Light,
        CheckedStyleEnvironmentClause::ColorScheme {
            value: ColorScheme::Dark,
            ..
        } => StyleEnvironmentIntrinsicTarget::Dark,
        CheckedStyleEnvironmentClause::Contrast {
            value: ContrastPreference::Standard,
            ..
        } => StyleEnvironmentIntrinsicTarget::StandardContrast,
        CheckedStyleEnvironmentClause::Contrast {
            value: ContrastPreference::More,
            ..
        } => StyleEnvironmentIntrinsicTarget::MoreContrast,
        CheckedStyleEnvironmentClause::ReducedMotion { value: true, .. } => {
            StyleEnvironmentIntrinsicTarget::BooleanTrue
        }
        CheckedStyleEnvironmentClause::ReducedMotion { value: false, .. } => {
            StyleEnvironmentIntrinsicTarget::BooleanFalse
        }
        CheckedStyleEnvironmentClause::TextScale { .. } => {
            StyleEnvironmentIntrinsicTarget::TextScale
        }
    }
}

fn clauses_are_complete_and_distinct(node: &StyleEnvironmentBlock) -> bool {
    let mut used = PresentationEnvironmentFieldSet::NONE;
    for clause in node.clauses() {
        let Some(field) = syntax_field(clause.field()) else {
            return false;
        };
        if used.contains(field)
            || clause.comparison() == StyleEnvironmentComparisonSyntax::Unsupported
            || matches!(clause.value(), StyleEnvironmentValueSyntax::Unsupported(_))
        {
            return false;
        }
        used = used.union(PresentationEnvironmentFieldSet::from_field(field));
    }
    !node.clauses().is_empty()
}

fn equality_repair_is_safe(clause: &StyleEnvironmentClause) -> bool {
    if matches!(
        clause.comparison(),
        StyleEnvironmentComparisonSyntax::Equal | StyleEnvironmentComparisonSyntax::Unsupported
    ) {
        return false;
    }
    matches!(
        (clause.field(), clause.value()),
        (
            StyleEnvironmentFieldSyntax::ColorScheme | StyleEnvironmentFieldSyntax::Contrast,
            StyleEnvironmentValueSyntax::Identifier { .. }
        ) | (
            StyleEnvironmentFieldSyntax::ReducedMotion,
            StyleEnvironmentValueSyntax::Boolean { .. }
        )
    )
}

fn percent_unit_repair_is_safe(clause: &StyleEnvironmentClause, source: &str) -> bool {
    if clause.field() != StyleEnvironmentFieldSyntax::TextScale {
        return false;
    }
    let StyleEnvironmentValueSyntax::Unsupported(value) = clause.value() else {
        return false;
    };
    if value.kind() != StyleEnvironmentUnsupportedValueKind::IntegerWithoutPercent {
        return false;
    }
    source
        .get(value.range().as_range())
        .is_some_and(unsigned_percentage_in_range)
}

fn unsigned_percentage_in_range(value: &str) -> bool {
    let mut parts = value.split('.');
    let Some(integer) = parts.next() else {
        return false;
    };
    let fractional = parts.next();
    if parts.next().is_some()
        || integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || fractional.is_some_and(|digits| {
            digits.len() != 1 || !digits.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return false;
    }
    let Ok(integer) = integer.parse::<u16>() else {
        return false;
    };
    let fractional = fractional
        .and_then(|digits| digits.bytes().next())
        .map_or(0, |digit| u16::from(digit - b'0'));
    let tenths = integer.saturating_mul(10).saturating_add(fractional);
    (500..=4_000).contains(&tenths)
}

const fn range_contains(range: TextRange, position: usize) -> bool {
    (range.start() <= position && position < range.end())
        || (range.start() == range.end() && range.start() == position)
}

const fn ranges_overlap(left: TextRange, right: TextRange) -> bool {
    left.start() < right.end() && right.start() < left.end()
}
