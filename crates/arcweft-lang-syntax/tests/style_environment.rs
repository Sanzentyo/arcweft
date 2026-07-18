use arcweft_lang_syntax::{
    ast::{
        items::Item,
        style::{
            StyleEnvironmentComparisonSyntax, StyleEnvironmentFieldSyntax,
            StyleEnvironmentUnsupportedValueKind, StyleEnvironmentValueSyntax,
        },
    },
    parser::{parse_source, recovery::ParseErrorKind},
    source::ParsedSource,
};

fn style(parsed: &ParsedSource) -> &arcweft_lang_syntax::ast::style::StyleDecl {
    parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Style(style) => Some(style),
            _ => None,
        })
        .expect("style declaration")
}

#[test]
fn environment_wrapper_parses_all_four_fields() {
    let source = r"pub style adaptive {
    when environment(
        color-scheme == dark,
        contrast == more,
        reduced-motion == true,
        text-scale >= 125.5%,
    ) {
        Button:hover { opacity = 900milli }
    }
}
";
    let parsed = parse_source(source);
    assert_eq!(parsed.errors(), &[]);
    let environment = style(&parsed).sheet().body()[0]
        .as_environment()
        .expect("environment wrapper");
    assert_eq!(environment.clauses().len(), 4);
    assert_eq!(
        environment
            .clauses()
            .iter()
            .map(arcweft_lang_syntax::ast::style::StyleEnvironmentClause::field)
            .collect::<Vec<_>>(),
        vec![
            StyleEnvironmentFieldSyntax::ColorScheme,
            StyleEnvironmentFieldSyntax::Contrast,
            StyleEnvironmentFieldSyntax::ReducedMotion,
            StyleEnvironmentFieldSyntax::TextScale,
        ]
    );
    assert_eq!(
        environment.clauses()[3].comparison(),
        StyleEnvironmentComparisonSyntax::GreaterOrEqual
    );
    let text_scale = &environment.clauses()[3];
    assert_eq!(
        &source[text_scale.range().as_range()],
        "text-scale >= 125.5%"
    );
    assert_eq!(&source[text_scale.field_range().as_range()], "text-scale");
    assert_eq!(&source[text_scale.comparison_range().as_range()], ">=");
    assert_eq!(&source[text_scale.value_range().as_range()], "125.5%");
    let StyleEnvironmentValueSyntax::Percentage(percentage) = text_scale.value() else {
        panic!("typed percentage")
    };
    assert_eq!(&source[percentage.integer_range().as_range()], "125");
    assert_eq!(
        &source[percentage
            .fractional_range()
            .expect("fractional digit")
            .as_range()],
        "5"
    );
    assert_eq!(&source[percentage.percent_range().as_range()], "%");
    assert_eq!(
        &source[environment.predicate_range().as_range()],
        "(\n        color-scheme == dark,\n        contrast == more,\n        reduced-motion == true,\n        text-scale >= 125.5%,\n    )"
    );
    assert_eq!(
        &source[environment.body_range().as_range()],
        "\n        Button:hover { opacity = 900milli }\n    "
    );
    assert_eq!(
        &source[environment.scope_range().as_range()],
        "when environment(\n        color-scheme == dark,\n        contrast == more,\n        reduced-motion == true,\n        text-scale >= 125.5%,\n    ) {\n        Button:hover { opacity = 900milli }\n    }"
    );
    assert!(environment.body()[0].as_rule().is_some());
}

#[test]
fn environment_wrapper_parses_nested_implicit_conjunction() {
    let source = r"pub style adaptive {
    when environment(color-scheme == dark) {
        when environment(text-scale < 100%) {
            Button { opacity = 800milli }
        }
    }
}
";
    let parsed = parse_source(source);
    assert_eq!(parsed.errors(), &[]);
    let outer = style(&parsed).sheet().body()[0]
        .as_environment()
        .expect("outer environment");
    let inner = outer.body()[0].as_environment().expect("inner environment");
    assert_eq!(
        outer.clauses()[0].field(),
        StyleEnvironmentFieldSyntax::ColorScheme
    );
    assert_eq!(
        inner.clauses()[0].field(),
        StyleEnvironmentFieldSyntax::TextScale
    );
    assert_eq!(
        &source[outer.predicate_range().as_range()],
        "(color-scheme == dark)"
    );
    assert_eq!(
        &source[inner.predicate_range().as_range()],
        "(text-scale < 100%)"
    );
    assert!(
        outer
            .body_range()
            .as_range()
            .contains(&inner.scope_range().start())
    );
    assert!(outer.body_range().end() >= inner.scope_range().end());
    let rule = inner.body()[0].as_rule().expect("guarded rule");
    assert!(inner.body_range().start() <= rule.range().start());
    assert!(rule.range().end() <= inner.body_range().end());
    assert!(inner.body()[0].as_rule().is_some());
}

#[test]
fn text_scale_parses_all_six_comparisons() {
    let source = r"pub style adaptive {
    when environment(text-scale == 100%) { Button { opacity = 1 } }
    when environment(text-scale != 100%) { Button { opacity = 1 } }
    when environment(text-scale < 100%) { Button { opacity = 1 } }
    when environment(text-scale <= 100%) { Button { opacity = 1 } }
    when environment(text-scale > 100%) { Button { opacity = 1 } }
    when environment(text-scale >= 100%) { Button { opacity = 1 } }
}
";
    let parsed = parse_source(source);
    assert_eq!(parsed.errors(), &[]);
    let comparisons = style(&parsed)
        .sheet()
        .body()
        .iter()
        .map(|item| item.as_environment().expect("environment").clauses()[0].comparison())
        .collect::<Vec<_>>();
    assert_eq!(
        comparisons,
        vec![
            StyleEnvironmentComparisonSyntax::Equal,
            StyleEnvironmentComparisonSyntax::NotEqual,
            StyleEnvironmentComparisonSyntax::Less,
            StyleEnvironmentComparisonSyntax::LessOrEqual,
            StyleEnvironmentComparisonSyntax::Greater,
            StyleEnvironmentComparisonSyntax::GreaterOrEqual,
        ]
    );
}

#[test]
fn arbitrarily_long_percentage_integer_never_overflows_ast() {
    let digits = "9".repeat(20_000);
    let source = format!(
        "pub style adaptive {{\n when environment(text-scale >= {digits}%) {{ Button {{ opacity = 1 }} }}\n}}\n"
    );
    let parsed = parse_source(&source);
    assert_eq!(parsed.errors(), &[]);
    let value = style(&parsed).sheet().body()[0]
        .as_environment()
        .expect("environment")
        .clauses()[0]
        .value();
    let StyleEnvironmentValueSyntax::Percentage(percentage) = value else {
        panic!("lossless percentage")
    };
    assert_eq!(
        percentage.integer_range().end() - percentage.integer_range().start(),
        digits.len()
    );
    assert_eq!(&source[percentage.integer_range().as_range()], digits);
}

#[test]
fn unsupported_percentage_families_are_typed_without_expression_fallback() {
    let cases = [
        (
            "+125%",
            StyleEnvironmentUnsupportedValueKind::SignedPercentage,
        ),
        (
            "1e2%",
            StyleEnvironmentUnsupportedValueKind::ExponentPercentage,
        ),
        (
            "125",
            StyleEnvironmentUnsupportedValueKind::IntegerWithoutPercent,
        ),
        (
            "125.55%",
            StyleEnvironmentUnsupportedValueKind::FractionalPrecision,
        ),
        (
            "clamp(50%, 100%)",
            StyleEnvironmentUnsupportedValueKind::NestedDelimiter,
        ),
    ];
    for (value, expected) in cases {
        let source = format!(
            "pub style adaptive {{\n when environment(text-scale == {value}) {{ Button {{ opacity = 1 }} }}\n}}\n"
        );
        let parsed = parse_source(source);
        assert!(parsed.errors().iter().any(|error| {
            error.kind() == ParseErrorKind::StyleEnvironmentUnsupportedValue
                && error.code() == "syntax.parse.style_environment.unsupported_value"
        }));
        let value = style(&parsed).sheet().body()[0]
            .as_environment()
            .expect("environment")
            .clauses()[0]
            .value();
        assert!(matches!(
            value,
            StyleEnvironmentValueSyntax::Unsupported(unsupported)
                if unsupported.kind() == expected
        ));
    }
}

#[test]
fn arbitrarily_long_fraction_never_overflows_ast() {
    let fractional = "7".repeat(20_000);
    let source = format!(
        "pub style adaptive {{\n when environment(text-scale == 125.{fractional}%) {{ Button {{ opacity = 1 }} }}\n}}\n"
    );
    let parsed = parse_source(source);
    assert!(parsed.errors().iter().any(|error| {
        error.kind() == ParseErrorKind::StyleEnvironmentUnsupportedValue
            && error.code() == "syntax.parse.style_environment.unsupported_value"
    }));
    let value = style(&parsed).sheet().body()[0]
        .as_environment()
        .expect("environment")
        .clauses()[0]
        .value();
    assert!(matches!(
        value,
        StyleEnvironmentValueSyntax::Unsupported(unsupported)
            if unsupported.kind() == StyleEnvironmentUnsupportedValueKind::FractionalPrecision
    ));
}

#[test]
fn missing_clause_comma_has_dedicated_code() {
    let source = r"pub style adaptive {
    when environment(color-scheme == dark contrast == more) {
        Button { opacity = 900milli }
    }
}
";
    let parsed = parse_source(source);
    let error = parsed
        .errors()
        .iter()
        .find(|error| error.kind() == ParseErrorKind::StyleEnvironmentExpectedCommaOrCloseParen)
        .expect("comma diagnostic");
    assert_eq!(
        error.code(),
        "syntax.parse.style_environment.expected_comma_or_close_paren"
    );
    assert_eq!(&source[error.range().as_range()], "dark contrast == more");
}

#[test]
fn unterminated_condition_recovers_at_matching_wrapper_brace() {
    let source = r"pub style adaptive {
    when environment(text-scale >= 125% {
        Button { opacity = 900milli }
    }
    Panel { opacity = 800milli }
}
";
    let parsed = parse_source(source);
    let error = parsed
        .errors()
        .iter()
        .find(|error| error.kind() == ParseErrorKind::StyleEnvironmentUnterminatedCondition)
        .expect("unterminated condition diagnostic");
    assert_eq!(
        error.code(),
        "syntax.parse.style_environment.unterminated_condition"
    );
    assert_eq!(&source[error.range().as_range()], "(text-scale >= 125% ");
    let body = style(&parsed).sheet().body();
    assert!(body[0].as_environment().is_some());
    assert!(body[1].as_rule().is_some(), "next sibling rule survives");
}

#[test]
fn environment_diagnostic_families_are_typed_with_exact_ranges() {
    let cases = [
        (
            "pub style adaptive {\n    when environment color-scheme == dark { Button { opacity = 1 } }\n}\n",
            ParseErrorKind::StyleEnvironmentExpectedOpenParen,
            "color-scheme",
            true,
        ),
        (
            "pub style adaptive {\n    when environment(platform == desktop) { Button { opacity = 1 } }\n}\n",
            ParseErrorKind::StyleEnvironmentExpectedField,
            "platform",
            false,
        ),
        (
            "pub style adaptive {\n    when environment(color-scheme dark) { Button { opacity = 1 } }\n}\n",
            ParseErrorKind::StyleEnvironmentExpectedComparison,
            "dark",
            true,
        ),
        (
            "pub style adaptive {\n    when environment(color-scheme ==) { Button { opacity = 1 } }\n}\n",
            ParseErrorKind::StyleEnvironmentExpectedValue,
            ")",
            true,
        ),
        (
            "pub style adaptive {\n    when environment(color-scheme == dark contrast == more) { Button { opacity = 1 } }\n}\n",
            ParseErrorKind::StyleEnvironmentExpectedCommaOrCloseParen,
            "dark contrast == more",
            false,
        ),
        (
            "pub style adaptive {\n    when environment(color-scheme == dark) Button { opacity = 1 }\n}\n",
            ParseErrorKind::StyleEnvironmentExpectedOpenBrace,
            "Button",
            true,
        ),
        (
            "pub style adaptive {\n    when environment(text-scale >= 125% { Button { opacity = 1 } }\n    Panel { opacity = 1 }\n}\n",
            ParseErrorKind::StyleEnvironmentUnterminatedCondition,
            "(text-scale >= 125% ",
            false,
        ),
        (
            "pub style adaptive {\n    when environment(text-scale == +125%) { Button { opacity = 1 } }\n}\n",
            ParseErrorKind::StyleEnvironmentUnsupportedValue,
            "+125%",
            false,
        ),
        (
            "pub style adaptive {\n    when environment(color-scheme == dark) { token accent = 1 }\n}\n",
            ParseErrorKind::StyleEnvironmentTokenNotAllowed,
            "token accent = 1",
            false,
        ),
    ];

    for (source, kind, marker, zero_width) in cases {
        let parsed = parse_source(source);
        let error = parsed
            .errors()
            .iter()
            .find(|error| error.kind() == kind)
            .unwrap_or_else(|| panic!("missing {kind:?}: {:?}", parsed.errors()));
        assert_eq!(error.code(), kind.code());
        let marker_start = source.find(marker).expect("fixture marker");
        if zero_width {
            assert_eq!(
                error.range(),
                &arcweft_lang_syntax::ast::common::TextRange::new(marker_start, marker_start,),
                "{kind:?}",
            );
        } else {
            assert_eq!(&source[error.range().as_range()], marker, "{kind:?}");
        }
    }
}

#[test]
fn environment_parser_produces_exactly_the_nine_registered_codes() {
    let sources = [
        "pub style adaptive {\n    when environment color-scheme == dark { Button { opacity = 1 } }\n}\n",
        "pub style adaptive {\n    when environment(platform == desktop) { Button { opacity = 1 } }\n}\n",
        "pub style adaptive {\n    when environment(color-scheme dark) { Button { opacity = 1 } }\n}\n",
        "pub style adaptive {\n    when environment(color-scheme ==) { Button { opacity = 1 } }\n}\n",
        "pub style adaptive {\n    when environment(color-scheme == dark contrast == more) { Button { opacity = 1 } }\n}\n",
        "pub style adaptive {\n    when environment(color-scheme == dark) Button { opacity = 1 }\n}\n",
        "pub style adaptive {\n    when environment(text-scale >= 125% { Button { opacity = 1 } }\n    Panel { opacity = 1 }\n}\n",
        "pub style adaptive {\n    when environment(text-scale == +125%) { Button { opacity = 1 } }\n}\n",
        "pub style adaptive {\n    when environment(color-scheme == dark) { token accent = 1 }\n}\n",
    ];
    let produced = sources
        .into_iter()
        .flat_map(|source| parse_source(source).errors().to_vec())
        .filter(|error| {
            matches!(
                error.kind(),
                ParseErrorKind::StyleEnvironmentExpectedOpenParen
                    | ParseErrorKind::StyleEnvironmentExpectedField
                    | ParseErrorKind::StyleEnvironmentExpectedComparison
                    | ParseErrorKind::StyleEnvironmentExpectedValue
                    | ParseErrorKind::StyleEnvironmentExpectedCommaOrCloseParen
                    | ParseErrorKind::StyleEnvironmentExpectedOpenBrace
                    | ParseErrorKind::StyleEnvironmentUnterminatedCondition
                    | ParseErrorKind::StyleEnvironmentUnsupportedValue
                    | ParseErrorKind::StyleEnvironmentTokenNotAllowed
            )
        })
        .map(|error| error.code())
        .collect::<std::collections::BTreeSet<_>>();
    let expected = [
        "syntax.parse.style_environment.expected_open_paren",
        "syntax.parse.style_environment.expected_field",
        "syntax.parse.style_environment.expected_comparison",
        "syntax.parse.style_environment.expected_value",
        "syntax.parse.style_environment.expected_comma_or_close_paren",
        "syntax.parse.style_environment.expected_open_brace",
        "syntax.parse.style_environment.unterminated_condition",
        "syntax.parse.style_environment.unsupported_value",
        "syntax.parse.style_environment.token_not_allowed",
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(produced, expected);
}
