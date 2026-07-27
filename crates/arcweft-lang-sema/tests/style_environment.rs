use arcweft_lang_hir::lower::lower_document_to_hir;
use arcweft_lang_sema::{
    check::{TypeCheckReport, analyze_types},
    diagnostics::TypeCheckErrorKind,
    env::TypeCheckEnv,
    style::{CheckedStyleEnvironmentClause, StyleDiagnosticCode},
};
use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_view::style::ViewTextScaleComparison;
use std::sync::Arc;

fn analyze(source: &str) -> (Vec<String>, TypeCheckReport) {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://sema/style-environment.arcw")
                .expect("source ID"),
            SourceName::Generated,
            source,
        )
        .expect("source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());
    let syntax_codes = parsed
        .errors()
        .iter()
        .map(|error| error.code().to_owned())
        .collect();
    let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree())
        .expect("style source lowers to HIR");
    (syntax_codes, analyze_types(&hir, &TypeCheckEnv::standard()))
}

fn style_codes(report: &TypeCheckReport) -> Vec<StyleDiagnosticCode> {
    report
        .diagnostics
        .iter()
        .filter_map(|error| match error.kind() {
            TypeCheckErrorKind::Style { diagnostic } => Some(diagnostic.code()),
            _ => None,
        })
        .collect()
}

#[test]
fn legal_environment_atomic_form_count_is_21012() {
    let text_scale_values = 4_000usize - 500 + 1;
    let text_scale_atomic_forms = text_scale_values * 6;
    let enum_boolean_atomic_forms = 2 + 2 + 2;
    assert_eq!(text_scale_atomic_forms, 21_006);
    assert_eq!(text_scale_atomic_forms + enum_boolean_atomic_forms, 21_012);
}

#[test]
fn legal_unique_conjunction_count_is_567188() {
    assert_eq!((1usize + 2).pow(3) * (1 + 21_006) - 1, 567_188);
}

#[test]
fn nested_environment_paths_flatten_into_one_checked_rule_guard() {
    let (syntax_codes, report) = analyze(
        r"pub style adaptive {
    when environment(color-scheme == dark) {
        when environment(text-scale >= 125.5%) {
            Button { opacity = 900milli }
        }
    }
}
",
    );
    assert!(syntax_codes.is_empty());
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let rules = report.style_catalog.sheets()[0].rules();
    assert_eq!(rules.len(), 1);
    let environment = rules[0].environment().expect("checked environment path");
    assert_eq!(environment.wrappers().len(), 2);
    assert_eq!(environment.clauses().len(), 2);
    assert!(matches!(
        environment.clauses()[0],
        CheckedStyleEnvironmentClause::ColorScheme { wrapper, .. }
            if wrapper.value() == 0
    ));
    assert!(matches!(
        environment.clauses()[1],
        CheckedStyleEnvironmentClause::TextScale {
            comparison: ViewTextScaleComparison::GreaterOrEqual,
            value,
            wrapper,
            ..
        } if value.value() == 1_255 && wrapper.value() == 1
    ));
}

#[test]
fn canonical_clause_order_retains_authored_wrapper_indexes() {
    let source = r"pub style adaptive {
    when environment(text-scale >= 125.5%) {
        when environment(color-scheme == dark) {
            Button { opacity = 900milli }
        }
    }
}
";
    let (syntax_codes, report) = analyze(source);
    assert!(syntax_codes.is_empty());
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let environment = report.style_catalog.sheets()[0].rules()[0]
        .environment()
        .expect("checked environment path");
    assert_eq!(environment.wrappers().len(), 2);
    assert_eq!(
        &source[environment.wrappers()[0].predicate_range().as_range()],
        "(text-scale >= 125.5%)"
    );
    assert_eq!(
        &source[environment.wrappers()[1].predicate_range().as_range()],
        "(color-scheme == dark)"
    );
    assert!(
        environment.wrappers()[0].body_range().start()
            <= environment.wrappers()[1].scope_range().start()
    );
    assert!(
        environment.wrappers()[1].scope_range().end()
            <= environment.wrappers()[0].body_range().end()
    );
    assert!(matches!(
        environment.clauses(),
        [
            CheckedStyleEnvironmentClause::ColorScheme { wrapper: inner, .. },
            CheckedStyleEnvironmentClause::TextScale { wrapper: outer, .. },
        ] if inner.value() == 1 && outer.value() == 0
    ));
}

#[test]
fn text_scale_truth_table_covers_six_comparisons_at_boundaries() {
    let comparisons = ["==", "!=", "<", "<=", ">", ">="];
    let source = comparisons
        .iter()
        .map(|comparison| {
            format!(
                "when environment(text-scale {comparison} 50%) {{ Button {{ opacity = 900milli }} }}"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("pub style adaptive {{\n{source}\n}}\n");
    let (syntax_codes, report) = analyze(&source);
    assert!(syntax_codes.is_empty());
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let actual = report.style_catalog.sheets()[0]
        .rules()
        .iter()
        .map(
            |rule| match rule.environment().expect("environment").clauses()[0] {
                CheckedStyleEnvironmentClause::TextScale {
                    comparison, value, ..
                } => {
                    assert_eq!(value.value(), 500);
                    comparison
                }
                _ => panic!("text-scale clause"),
            },
        )
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            ViewTextScaleComparison::Equal,
            ViewTextScaleComparison::NotEqual,
            ViewTextScaleComparison::Less,
            ViewTextScaleComparison::LessOrEqual,
            ViewTextScaleComparison::Greater,
            ViewTextScaleComparison::GreaterOrEqual,
        ]
    );
}

#[test]
fn text_scale_rejects_49_9_and_400_1_percent() {
    for percentage in ["49.9%", "400.1%"] {
        let source = format!(
            "pub style adaptive {{\n when environment(text-scale == {percentage}) {{ Button {{ opacity = 1 }} }}\n}}\n"
        );
        let (syntax_codes, report) = analyze(&source);
        assert!(syntax_codes.is_empty());
        let codes = style_codes(&report);
        assert!(codes.contains(&StyleDiagnosticCode::EnvironmentTextScaleRange));
        assert!(codes.contains(&StyleDiagnosticCode::EnvironmentInvalidPath));
        assert!(report.style_catalog.sheets()[0].rules().is_empty());
    }
}

#[test]
fn text_scale_rejects_second_fractional_digit() {
    let (syntax_codes, report) = analyze(
        r"pub style adaptive {
    when environment(text-scale == 125.55%) { Button { opacity = 1 } }
}
",
    );
    assert_eq!(
        syntax_codes,
        vec!["syntax.parse.style_environment.unsupported_value"]
    );
    let codes = style_codes(&report);
    assert!(codes.contains(&StyleDiagnosticCode::EnvironmentTextScalePrecision));
    assert!(codes.contains(&StyleDiagnosticCode::EnvironmentInvalidPath));
    assert!(report.style_catalog.sheets()[0].rules().is_empty());
}

#[test]
fn enum_boolean_equality_only_truth_table() {
    let (syntax_codes, report) = analyze(
        r"pub style adaptive {
    when environment(color-scheme != dark) { Button { opacity = 1 } }
    when environment(contrast < more) { Button { opacity = 1 } }
    when environment(reduced-motion >= true) { Button { opacity = 1 } }
}
",
    );
    assert!(syntax_codes.is_empty());
    assert_eq!(
        style_codes(&report)
            .into_iter()
            .filter(|code| *code == StyleDiagnosticCode::EnvironmentInvalidComparison)
            .count(),
        3
    );
    assert!(report.style_catalog.sheets()[0].rules().is_empty());
}

#[test]
fn duplicate_fields_in_one_wrapper_and_nested_path_are_rejected() {
    let source = r"pub style adaptive {
    when environment(color-scheme == light, color-scheme == dark) {
        Button { opacity = 1 }
    }
    when environment(contrast == standard) {
        when environment(contrast == more) {
            Button { opacity = 1 }
        }
    }
}
";
    let (syntax_codes, report) = analyze(source);
    assert!(syntax_codes.is_empty());
    let codes = style_codes(&report);
    assert!(codes.contains(&StyleDiagnosticCode::EnvironmentDuplicateField));
    assert!(codes.contains(&StyleDiagnosticCode::EnvironmentDuplicateFieldOnPath));
    assert_eq!(
        codes
            .iter()
            .filter(|code| **code == StyleDiagnosticCode::EnvironmentInvalidPath)
            .count(),
        2
    );
    assert!(report.style_catalog.sheets()[0].rules().is_empty());
    let related = report
        .diagnostics
        .iter()
        .filter_map(|error| match error.kind() {
            TypeCheckErrorKind::Style { diagnostic }
                if matches!(
                    diagnostic.code(),
                    StyleDiagnosticCode::EnvironmentDuplicateField
                        | StyleDiagnosticCode::EnvironmentDuplicateFieldOnPath
                ) =>
            {
                Some(diagnostic.related_ranges())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(related.iter().all(|ranges| ranges.len() == 1));

    let first_color = source.find("color-scheme").expect("first color field");
    let second_color = source[first_color + 1..]
        .find("color-scheme")
        .map(|offset| offset + first_color + 1)
        .expect("second color field");
    let first_contrast = source.find("contrast").expect("first contrast field");
    let second_contrast = source[first_contrast + 1..]
        .find("contrast")
        .map(|offset| offset + first_contrast + 1)
        .expect("second contrast field");
    for error in &report.diagnostics {
        let TypeCheckErrorKind::Style { diagnostic } = error.kind() else {
            continue;
        };
        let related = diagnostic.related_ranges();
        match diagnostic.code() {
            StyleDiagnosticCode::EnvironmentDuplicateField => {
                assert_eq!(
                    diagnostic.range().as_range(),
                    second_color..second_color + "color-scheme".len()
                );
                assert_eq!(
                    related[0].as_range(),
                    first_color..first_color + "color-scheme".len()
                );
            }
            StyleDiagnosticCode::EnvironmentDuplicateFieldOnPath => {
                assert_eq!(
                    diagnostic.range().as_range(),
                    second_contrast..second_contrast + "contrast".len()
                );
                assert_eq!(
                    related[0].as_range(),
                    first_contrast..first_contrast + "contrast".len()
                );
            }
            _ => {}
        }
    }
}
