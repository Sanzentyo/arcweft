use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    style::{HirStyleEnvironmentId, HirStyleId},
};
use arcweft_lang_sema::{check::analyze_types, env::TypeCheckEnv};
use arcweft_lang_syntax::{
    ast::{common::TextRange, items::Item, style::StyleEnvironmentBlock},
    parser::{ParseOptions, parse_document_with_source},
    source::ParsedSource,
};
use arcweft_presentation::appearance::{
    PresentationEnvironmentField, PresentationEnvironmentFieldSet,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_tooling::{
    edit::apply_text_edits,
    model::FormatOptions,
    style_environment::{
        StyleEnvironmentCodeActionInput, StyleEnvironmentCodeActionKind,
        StyleEnvironmentCompletionInput, StyleEnvironmentCompletionSite, StyleEnvironmentEditInput,
        StyleEnvironmentEditInvalidation, StyleEnvironmentFormatInput, StyleEnvironmentHoverInput,
        StyleEnvironmentHoverSubject, StyleEnvironmentIntrinsicTarget,
        StyleEnvironmentSemanticEdit, StyleEnvironmentSemanticKind, complete_style_environment,
        format_style_environment, hover_style_environment, navigate_style_environment,
        style_environment_code_actions, style_environment_edit_invalidation,
        style_environment_semantic_spans,
    },
};
mod support;
use std::sync::Arc;
use support::format_fixture;

#[test]
fn formatter_orders_fields_canonically_and_normalizes_percentage() {
    let source = r"pub style adaptive {
    when environment(text-scale>=125.0%, color-scheme == DARK) {
        Button { opacity = 90% }
    }
}
";
    let report = format_fixture(source, FormatOptions::default()).expect("format report");
    assert_eq!(
        report.output,
        r"pub style adaptive {
    when environment(
        color-scheme == dark,
        text-scale >= 125%,
    ) {
        Button { opacity = 90% }
    }
}
"
    );
    let second = format_fixture(&report.output, FormatOptions::default()).expect("second format");
    assert!(!second.changed);
    assert_eq!(second.output, report.output);
}

#[test]
fn formatter_is_idempotent_for_recovered_environment_nodes() {
    let source = r"pub style adaptive {
    when environment(text-scale == clamp(50%, 100%)) {
        Button { opacity = 90% }
    }
}
";
    let first = format_fixture(source, FormatOptions::default()).expect("format report");
    assert!(!first.changed);
    assert_eq!(first.output, source);
    let second = format_fixture(&first.output, FormatOptions::default()).expect("second format");
    assert_eq!(second.output, first.output);
}

#[test]
fn single_node_formatter_returns_a_condition_content_edit() {
    let source = r"pub style adaptive {
    when environment(text-scale>=125.0%) {
        Button { opacity = 90% }
    }
}
";
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://tooling/style-environment/single-node-formatter-returns-a-condition-content-edit")
                .expect("fixture document ID"),
            SourceName::path("single-node-formatter-returns-a-condition-content-edit.arcw"),
            source,
        )
        .expect("fixture source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());
    assert_eq!(parsed.errors(), &[]);
    let environment = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Style(style) => style.sheet().body()[0].as_environment(),
            _ => None,
        })
        .expect("environment wrapper");
    let result = format_style_environment(StyleEnvironmentFormatInput {
        node: environment,
        cst: parsed.syntax(),
    });
    assert!(!result.canonical);
    let output = apply_text_edits(source, &result.edits).expect("valid edit");
    assert!(output.contains("when environment(text-scale >= 125%)"));

    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://tooling/style-environment/single-node-formatter-returns-a-condition-content-edit")
                .expect("fixture document ID"),
            SourceName::path("single-node-formatter-returns-a-condition-content-edit.arcw"),
            output.as_str(),
        )
        .expect("fixture source document"),
    );
    let reparsed = parse_document_with_source(document, ParseOptions::default());
    let environment = reparsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Style(style) => style.sheet().body()[0].as_environment(),
            _ => None,
        })
        .expect("formatted environment wrapper");
    let second = format_style_environment(StyleEnvironmentFormatInput {
        node: environment,
        cst: reparsed.syntax(),
    });
    assert!(second.canonical);
    assert!(second.edits.is_empty());
}

#[test]
fn completion_suppresses_fields_already_used_on_effective_path() {
    let used =
        PresentationEnvironmentFieldSet::from_field(PresentationEnvironmentField::ColorScheme)
            .union(PresentationEnvironmentFieldSet::from_field(
                PresentationEnvironmentField::ReducedMotion,
            ));
    let items = complete_style_environment(StyleEnvironmentCompletionInput {
        site: StyleEnvironmentCompletionSite::Field { used_on_path: used },
        replace: TextRange::new(10, 12),
    });
    assert_eq!(
        items.iter().map(|item| item.label).collect::<Vec<_>>(),
        vec!["contrast", "text-scale"]
    );
    assert!(
        items
            .iter()
            .all(|item| item.replace == TextRange::new(10, 12))
    );
}

#[test]
fn completion_comparisons_are_field_specific() {
    let enum_items = complete_style_environment(StyleEnvironmentCompletionInput {
        site: StyleEnvironmentCompletionSite::Comparison {
            field: PresentationEnvironmentField::ColorScheme,
        },
        replace: TextRange::new(0, 0),
    });
    assert_eq!(
        enum_items.iter().map(|item| item.label).collect::<Vec<_>>(),
        vec!["=="]
    );
    let scale_items = complete_style_environment(StyleEnvironmentCompletionInput {
        site: StyleEnvironmentCompletionSite::Comparison {
            field: PresentationEnvironmentField::TextScale,
        },
        replace: TextRange::new(0, 0),
    });
    assert_eq!(
        scale_items
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec!["==", "!=", "<", "<=", ">", ">="]
    );
}

#[test]
fn completion_values_are_closed_and_canonical() {
    let color = complete_style_environment(StyleEnvironmentCompletionInput {
        site: StyleEnvironmentCompletionSite::Value {
            field: PresentationEnvironmentField::ColorScheme,
        },
        replace: TextRange::new(0, 0),
    });
    assert_eq!(
        color
            .iter()
            .map(|item| item.insert_text)
            .collect::<Vec<_>>(),
        vec!["light", "dark"]
    );
    let scale = complete_style_environment(StyleEnvironmentCompletionInput {
        site: StyleEnvironmentCompletionSite::Value {
            field: PresentationEnvironmentField::TextScale,
        },
        replace: TextRange::new(0, 0),
    });
    assert!(scale.iter().all(|item| item.insert_text.ends_with('%')));
    assert!(!scale.iter().any(|item| item.insert_text.contains('(')));
}

#[test]
fn hover_uses_checked_value_and_source_range() {
    let source = valid_environment_source("color-scheme == dark");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://tooling/style-environment/hover-uses-checked-value-and-source-range")
                .expect("fixture document ID"),
            SourceName::path("hover-uses-checked-value-and-source-range.arcw"),
            source.as_str(),
        )
        .expect("fixture source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());
    let environment = first_environment(&parsed);
    let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree()).unwrap();
    let report = analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let checked = report.style_catalog.sheets()[0].rules()[0]
        .environment()
        .unwrap();
    let value = environment.clauses()[0].value_range();
    let hover = hover_style_environment(StyleEnvironmentHoverInput {
        position: value.start(),
        ast: environment,
        checked: Some(checked),
    })
    .unwrap();
    assert_eq!(hover.range, checked.clauses()[0].range());
    assert!(hover.markdown.contains("color-scheme == dark"));
}

#[test]
fn wrapper_hover_uses_complete_environment_scope() {
    let source = valid_environment_source("color-scheme == dark");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://tooling/style-environment/wrapper-hover-uses-complete-environment-scope")
                .expect("fixture document ID"),
            SourceName::path("wrapper-hover-uses-complete-environment-scope.arcw"),
            source.as_str(),
        )
        .expect("fixture source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());
    let environment = first_environment(&parsed);
    let hover = hover_style_environment(StyleEnvironmentHoverInput {
        position: environment.when_range().start(),
        ast: environment,
        checked: None,
    })
    .unwrap();

    assert_eq!(hover.subject, StyleEnvironmentHoverSubject::Wrapper);
    assert_eq!(hover.range, environment.scope_range());
}

#[test]
fn hover_on_recovered_value_remains_typed_partial() {
    let source = valid_environment_source("text-scale == clamp(50%, 100%)");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://tooling/style-environment/hover-on-recovered-value-remains-typed-partial")
                .expect("fixture document ID"),
            SourceName::path("hover-on-recovered-value-remains-typed-partial.arcw"),
            source.as_str(),
        )
        .expect("fixture source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());
    let environment = first_environment(&parsed);
    let value = environment.clauses()[0].value_range();
    let hover = hover_style_environment(StyleEnvironmentHoverInput {
        position: value.start(),
        ast: environment,
        checked: None,
    })
    .unwrap();
    assert_eq!(hover.range, value);
    assert!(hover.markdown.contains("recovered"));
    assert!(hover.markdown.contains("50%..=400%"));
}

#[test]
fn semantic_spans_cover_keyword_field_operator_value_unit() {
    let source = valid_environment_source("text-scale >= 125.5%");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://tooling/style-environment/semantic-spans-cover-keyword-field-operator-value-unit")
                .expect("fixture document ID"),
            SourceName::path("semantic-spans-cover-keyword-field-operator-value-unit.arcw"),
            source.as_str(),
        )
        .expect("fixture source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());
    let environment = first_environment(&parsed);
    let spans = style_environment_semantic_spans(environment);
    let kinds = spans.iter().map(|span| span.kind).collect::<Vec<_>>();
    for expected in [
        StyleEnvironmentSemanticKind::Keyword,
        StyleEnvironmentSemanticKind::Intrinsic,
        StyleEnvironmentSemanticKind::Field,
        StyleEnvironmentSemanticKind::Operator,
        StyleEnvironmentSemanticKind::Number,
        StyleEnvironmentSemanticKind::Unit,
        StyleEnvironmentSemanticKind::Punctuation,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {spans:?}");
    }
}

#[test]
fn canonicalize_action_requires_complete_distinct_clauses() {
    let complete = valid_environment_source("text-scale >= 125%, color-scheme == dark");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://tooling/style-environment/canonicalize-action-requires-complete-distinct-clauses")
                .expect("fixture document ID"),
            SourceName::path("canonicalize-action-requires-complete-distinct-clauses.arcw"),
            complete.as_str(),
        )
        .expect("fixture source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());
    let actions = style_environment_code_actions(StyleEnvironmentCodeActionInput {
        node: first_environment(&parsed),
        source: &complete,
        diagnostics: &[],
    });
    assert!(
        actions.iter().any(|action| {
            action.kind == StyleEnvironmentCodeActionKind::CanonicalizeFieldOrder
        })
    );

    for clauses in [
        "color-scheme == dark, color-scheme == light",
        "unknown == dark",
    ] {
        let source = valid_environment_source(clauses);
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://tooling/style-environment/canonicalize-action-requires-complete-distinct-clauses")
                    .expect("fixture document ID"),
                SourceName::path("canonicalize-action-requires-complete-distinct-clauses.arcw"),
                source.as_str(),
            )
            .expect("fixture source document"),
        );
        let parsed = parse_document_with_source(document, ParseOptions::default());
        let actions = style_environment_code_actions(StyleEnvironmentCodeActionInput {
            node: first_environment(&parsed),
            source: &source,
            diagnostics: &[],
        });
        assert!(!actions.iter().any(|action| {
            action.kind == StyleEnvironmentCodeActionKind::CanonicalizeFieldOrder
        }));
    }
}

#[test]
fn equality_action_only_for_enum_and_boolean() {
    let source = valid_environment_source(
        "color-scheme != dark, reduced-motion != true, text-scale != 125%",
    );
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://tooling/style-environment/equality-action-only-for-enum-and-boolean")
                .expect("fixture document ID"),
            SourceName::path("equality-action-only-for-enum-and-boolean.arcw"),
            source.as_str(),
        )
        .expect("fixture source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());
    let actions = style_environment_code_actions(StyleEnvironmentCodeActionInput {
        node: first_environment(&parsed),
        source: &source,
        diagnostics: &[],
    });
    assert_eq!(
        actions
            .iter()
            .filter(|action| action.kind == StyleEnvironmentCodeActionKind::ReplaceWithEquality)
            .count(),
        2
    );
}

#[test]
fn percent_action_only_for_checked_unsigned_decimal() {
    for (value, expected) in [
        ("50", true),
        ("125.5", true),
        ("400", true),
        ("49", false),
        ("400.1", false),
        ("-125", false),
        ("1e2", false),
        ("125.55", false),
    ] {
        let source = valid_environment_source(&format!("text-scale == {value}"));
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://tooling/style-environment/percent-action-only-for-checked-unsigned-decimal")
                    .expect("fixture document ID"),
                SourceName::path("percent-action-only-for-checked-unsigned-decimal.arcw"),
                source.as_str(),
            )
            .expect("fixture source document"),
        );
        let parsed = parse_document_with_source(document, ParseOptions::default());
        let actions = style_environment_code_actions(StyleEnvironmentCodeActionInput {
            node: first_environment(&parsed),
            source: &source,
            diagnostics: &[],
        });
        assert_eq!(
            actions
                .iter()
                .any(|action| { action.kind == StyleEnvironmentCodeActionKind::AddPercentUnit }),
            expected,
            "value {value}"
        );
    }
}

#[test]
fn intrinsic_navigation_returns_typed_target() {
    let source = valid_environment_source("color-scheme == dark");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://tooling/style-environment/intrinsic-navigation-returns-typed-target")
                .expect("fixture document ID"),
            SourceName::path("intrinsic-navigation-returns-typed-target.arcw"),
            source.as_str(),
        )
        .expect("fixture source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());
    let environment = first_environment(&parsed);
    let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree()).unwrap();
    let report = analyze_types(&hir, &TypeCheckEnv::standard());
    let checked = report.style_catalog.sheets()[0].rules()[0]
        .environment()
        .unwrap();
    let value = environment.clauses()[0].value_range();
    let navigation = navigate_style_environment(value.start(), environment, Some(checked)).unwrap();
    assert_eq!(navigation.origin, value);
    assert_eq!(navigation.target, StyleEnvironmentIntrinsicTarget::Dark);
}

#[test]
fn typed_environment_edit_invalidation_is_field_local() {
    let sheet = HirStyleId::new(7);
    let environment = HirStyleEnvironmentId::new(3);
    assert_eq!(
        style_environment_edit_invalidation(StyleEnvironmentEditInput {
            sheet,
            environment,
            edit: StyleEnvironmentSemanticEdit::Clause,
        }),
        Some(StyleEnvironmentEditInvalidation::Subtree { sheet, environment })
    );
    assert_eq!(
        style_environment_edit_invalidation(StyleEnvironmentEditInput {
            sheet,
            environment,
            edit: StyleEnvironmentSemanticEdit::WrapperAncestry,
        }),
        Some(StyleEnvironmentEditInvalidation::Sheet { sheet })
    );
    assert_eq!(
        style_environment_edit_invalidation(StyleEnvironmentEditInput {
            sheet,
            environment,
            edit: StyleEnvironmentSemanticEdit::Unchanged,
        }),
        None
    );
}

fn valid_environment_source(clauses: &str) -> String {
    format!(
        "pub style adaptive {{\n    when environment({clauses}) {{\n        Button {{ opacity = 90% }}\n    }}\n}}\n"
    )
}

fn first_environment(parsed: &ParsedSource) -> &StyleEnvironmentBlock {
    parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Style(style) => style.sheet().body()[0].as_environment(),
            _ => None,
        })
        .expect("environment wrapper")
}
