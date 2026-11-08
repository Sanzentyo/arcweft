use super::*;
use arcweft_presentation::appearance::{
    ColorScheme, ContrastPreference, PresentationEnvironment, PresentationEnvironmentField,
    PresentationEnvironmentValues, TextScaleMilli,
};
use arcweft_view::style::{ViewEnvironmentTest, ViewStyleSheetId, ViewTextScaleComparison};

#[test]
fn style_environment_nested_source_order_and_provenance_survive_canonical_product() {
    let source = r"
style Environment {
  Panel { opacity = 0.1 }
  when environment(text-scale >= 125%) {
    Text { opacity = 0.2 }
    when environment(reduced-motion == true) {
      Text { opacity = 0.3 }
      when environment(contrast == more) {
        Text { opacity = 0.4 }
        when environment(color-scheme == dark) {
          Panel { opacity = 0.5 }
        }
      }
    }
  }
  Panel { opacity = 0.6 }
}
";
    let compiled = project_view_fixture(source, "arcweft-test://style-environment-nested")
        .compile()
        .unwrap();
    let resource = compiled
        .view_product()
        .product()
        .style()
        .unwrap()
        .resource();
    let decoded =
        arcweft_bundle::resource_codec::view::ViewStyleResource::decode_canonical_section(
            &resource.encode_canonical_section().unwrap(),
        )
        .unwrap();
    assert_eq!(&decoded, resource);
    let sheet = decoded
        .program
        .sheet(&ViewStyleSheetId::try_new("style.Environment").unwrap())
        .unwrap();
    assert_eq!(sheet.rules().len(), 6);
    assert_eq!(
        sheet
            .rules()
            .iter()
            .map(arcweft_view::ViewStyleRule::source_order)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4, 5]
    );
    assert!(sheet.rules()[0].environment().is_none());
    assert!(sheet.rules()[5].environment().is_none());
    for (index, rule) in sheet.rules()[1..5].iter().enumerate() {
        let condition = rule.environment().unwrap();
        assert_eq!(condition.wrappers().len(), index + 1);
        assert_eq!(condition.clauses().len(), index + 1);
    }
    let condition = sheet.rules()[4].environment().unwrap();
    assert_eq!(
        condition
            .clauses()
            .iter()
            .map(|clause| clause.field())
            .collect::<Vec<_>>(),
        vec![
            PresentationEnvironmentField::ColorScheme,
            PresentationEnvironmentField::Contrast,
            PresentationEnvironmentField::ReducedMotion,
            PresentationEnvironmentField::TextScale
        ]
    );
    for (clause, expected, wrapper) in [
        (condition.clauses()[0], "color-scheme == dark", 3),
        (condition.clauses()[1], "contrast == more", 2),
        (condition.clauses()[2], "reduced-motion == true", 1),
        (condition.clauses()[3], "text-scale >= 125%", 0),
    ] {
        assert_eq!(clause.wrapper().value(), wrapper);
        let range = &decoded.source_map_refs[clause.source().value() as usize];
        assert_eq!(
            &source[range.start_byte() as usize..range.end_byte() as usize],
            expected
        );
    }
    for wrapper in condition.wrappers() {
        let scope = &decoded.source_map_refs[wrapper.scope_source().value() as usize];
        let predicate = &decoded.source_map_refs[wrapper.predicate_source().value() as usize];
        let body = &decoded.source_map_refs[wrapper.body_source().value() as usize];
        let scope_text = &source[scope.start_byte() as usize..scope.end_byte() as usize];
        let predicate_text =
            &source[predicate.start_byte() as usize..predicate.end_byte() as usize];
        let body_text = &source[body.start_byte() as usize..body.end_byte() as usize];
        assert!(scope_text.starts_with("when environment("));
        assert!(scope_text.ends_with('}'));
        assert!(scope_text.contains(predicate_text) && scope_text.contains(body_text));
        assert!(predicate.start_byte() < body.start_byte());
        assert!(body_text.starts_with('{') && body_text.ends_with('}'));
    }
    for (color, contrast, motion, scale, expected) in [
        (
            ColorScheme::Dark,
            ContrastPreference::More,
            true,
            1250,
            true,
        ),
        (
            ColorScheme::Light,
            ContrastPreference::More,
            true,
            1250,
            false,
        ),
        (
            ColorScheme::Dark,
            ContrastPreference::Standard,
            true,
            1250,
            false,
        ),
        (
            ColorScheme::Dark,
            ContrastPreference::More,
            false,
            1250,
            false,
        ),
        (
            ColorScheme::Dark,
            ContrastPreference::More,
            true,
            1249,
            false,
        ),
        (
            ColorScheme::Dark,
            ContrastPreference::More,
            true,
            4000,
            true,
        ),
    ] {
        let environment = PresentationEnvironment::initial(PresentationEnvironmentValues::new(
            color,
            contrast,
            motion,
            TextScaleMilli::try_new(scale).unwrap(),
        ));
        assert_eq!(condition.matches(environment).matched(), expected);
    }
}

#[test]
fn style_environment_exact_text_scale_literals_and_all_comparisons_use_native_tests() {
    for literal in ["1.25", "125%"] {
        for (operator, comparison) in [
            ("==", ViewTextScaleComparison::Equal),
            ("!=", ViewTextScaleComparison::NotEqual),
            ("<", ViewTextScaleComparison::Less),
            ("<=", ViewTextScaleComparison::LessOrEqual),
            (">", ViewTextScaleComparison::Greater),
            (">=", ViewTextScaleComparison::GreaterOrEqual),
        ] {
            let source = format!(
                "style Environment {{ when environment(text-scale {operator} {literal}) {{ Panel {{ opacity = 1 }} }} }}"
            );
            let compiled =
                project_view_fixture(&source, "arcweft-test://style-environment-comparison")
                    .compile()
                    .unwrap();
            let program = &compiled
                .view_product()
                .product()
                .style()
                .unwrap()
                .resource()
                .program;
            let condition = program
                .sheet(&ViewStyleSheetId::try_new("style.Environment").unwrap())
                .unwrap()
                .rules()[0]
                .environment()
                .unwrap();
            assert_eq!(
                condition.clauses()[0].test(),
                ViewEnvironmentTest::TextScale {
                    comparison,
                    value: TextScaleMilli::try_new(1250).unwrap()
                }
            );
            for scale in [500, 1249, 1250, 1251, 4000] {
                let environment =
                    PresentationEnvironment::initial(PresentationEnvironmentValues::new(
                        ColorScheme::Dark,
                        ContrastPreference::Standard,
                        false,
                        TextScaleMilli::try_new(scale).unwrap(),
                    ));
                let expected = match comparison {
                    ViewTextScaleComparison::Equal => scale == 1250,
                    ViewTextScaleComparison::NotEqual => scale != 1250,
                    ViewTextScaleComparison::Less => scale < 1250,
                    ViewTextScaleComparison::LessOrEqual => scale <= 1250,
                    ViewTextScaleComparison::Greater => scale > 1250,
                    ViewTextScaleComparison::GreaterOrEqual => scale >= 1250,
                };
                assert_eq!(
                    condition.matches(environment).matched(),
                    expected,
                    "{operator} {literal}: {scale}"
                );
            }
        }
    }
    for literal in ["0.5", "50%", "4", "400%"] {
        let source = format!(
            "style Environment {{ when environment(text-scale == {literal}) {{ Panel {{ opacity = 1 }} }} }}"
        );
        project_view_fixture(&source, "arcweft-test://style-environment-boundary")
            .compile()
            .unwrap();
    }
    project_view_fixture("style Environment { when environment(color-scheme == light, contrast == standard, reduced-motion == false, text-scale == 1) { Panel { opacity = 1 } } }", "arcweft-test://style-environment-all-fields").compile().unwrap();
}

#[test]
fn style_environment_rejects_wrong_values_operators_duplicate_fields_and_excessive_nesting() {
    for clause in [
        "color-scheme == unknown",
        "color-scheme == .Dark",
        "color-scheme != dark",
        "contrast == dark",
        "contrast > more",
        "reduced-motion == 1",
        "reduced-motion != false",
        "text-scale == 0.499",
        "text-scale == 4.001",
        "text-scale == 400.1%",
        "text-scale == -1",
        "text-scale == 1px",
        "text-scale == 1i32",
        "text-scale == 1.0001",
        "text-scale == 100.01%",
        "text-scale == token(scale)",
        "text-scale >= 1, text-scale <= 2",
    ] {
        let source = format!(
            "style Environment {{ when environment({clause}) {{ Panel {{ opacity = 1 }} }} }}"
        );
        let error = project_view_fixture(&source, "arcweft-test://style-environment-rejection")
            .compile()
            .unwrap_err();
        assert_eq!(error.stage(), "type-check", "{clause}: {error:?}");
    }
    for source in [
        "style Environment { when environment(color-scheme == dark) { when environment(color-scheme == dark) { Panel { opacity = 1 } } } }",
        "style Environment { when environment(text-scale > 1) { when environment(text-scale < 2) { Panel { opacity = 1 } } } }",
        "style Environment { when environment(color-scheme == dark) { when environment(contrast == more) { when environment(reduced-motion == true) { when environment(text-scale == 1) { when environment(text-scale == 1) { Panel { opacity = 1 } } } } } } }",
    ] {
        let error =
            project_view_fixture(source, "arcweft-test://style-environment-nesting-rejection")
                .compile()
                .unwrap_err();
        assert_eq!(error.stage(), "type-check", "{source}: {error:?}");
    }
}
