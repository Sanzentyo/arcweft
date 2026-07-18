use arcweft_lang_hir::{
    lower::lower_to_hir,
    model::HirTopLevelDecl,
    style::{
        HirStyleDecl, HirStyleEnvironmentComparison, HirStyleEnvironmentField,
        HirStyleEnvironmentRecovery, HirStyleEnvironmentValue,
    },
};
use arcweft_lang_syntax::{ast::style::StyleEnvironmentUnsupportedValueKind, parser::parse_source};

fn style(hir: &arcweft_lang_hir::model::HirModule) -> &HirStyleDecl {
    hir.declarations()
        .iter()
        .find_map(|declaration| match declaration {
            HirTopLevelDecl::Style(style) => Some(style),
            _ => None,
        })
        .expect("HIR style")
}

#[test]
fn hir_preserves_condition_clause_and_operand_ranges() {
    let source = r"pub style adaptive {
    when environment(text-scale >= 125.5%) {
        Button { opacity = 900milli }
    }
}
";
    let parsed = parse_source(source);
    assert_eq!(parsed.errors(), &[]);
    let hir = lower_to_hir(parsed.typed_tree()).expect("style lowers");
    let environment = style(&hir).sheet().body()[0]
        .as_environment()
        .expect("environment wrapper");
    let clause = &environment.clauses()[0];
    assert_eq!(clause.field(), &HirStyleEnvironmentField::TextScale);
    assert_eq!(
        clause.comparison(),
        HirStyleEnvironmentComparison::GreaterOrEqual
    );
    assert_eq!(&source[clause.ranges().field().as_range()], "text-scale");
    assert_eq!(&source[clause.ranges().comparison().as_range()], ">=");
    assert_eq!(&source[clause.ranges().value().as_range()], "125.5%");
    assert_eq!(
        &source[clause.ranges().clause().as_range()],
        "text-scale >= 125.5%"
    );
    assert_eq!(
        &source[environment.predicate_range().as_range()],
        "(text-scale >= 125.5%)"
    );
    assert_eq!(
        &source[environment.body_range().as_range()],
        "\n        Button { opacity = 900milli }\n    "
    );
    assert_eq!(
        &source[environment.scope_range().as_range()],
        "when environment(text-scale >= 125.5%) {\n        Button { opacity = 900milli }\n    }"
    );
    assert_eq!(
        &source[environment.body()[0]
            .as_rule()
            .expect("guarded rule")
            .range()
            .as_range()],
        "Button { opacity = 900milli }"
    );
    let HirStyleEnvironmentValue::Percentage(percentage) = clause.value() else {
        panic!("typed percentage")
    };
    assert_eq!(percentage.integer_digits(), "125");
    assert_eq!(percentage.fractional_digits(), Some("5"));
}

#[test]
fn hir_preserves_nested_wrapper_roles_outer_to_inner() {
    let source = r"pub style adaptive {
    when environment(text-scale >= 125.5%) {
        when environment(color-scheme == dark) {
            Button { opacity = 900milli }
        }
    }
}
";
    let parsed = parse_source(source);
    assert_eq!(parsed.errors(), &[]);
    let hir = lower_to_hir(parsed.typed_tree()).expect("style lowers");
    let outer = style(&hir).sheet().body()[0]
        .as_environment()
        .expect("outer wrapper");
    let inner = outer.body()[0].as_environment().expect("inner wrapper");
    assert_eq!(
        &source[outer.predicate_range().as_range()],
        "(text-scale >= 125.5%)"
    );
    assert_eq!(
        &source[inner.predicate_range().as_range()],
        "(color-scheme == dark)"
    );
    assert!(outer.body_range().start() <= inner.scope_range().start());
    assert!(inner.scope_range().end() <= outer.body_range().end());
    let rule = inner.body()[0].as_rule().expect("guarded rule");
    assert!(inner.body_range().start() <= rule.range().start());
    assert!(rule.range().end() <= inner.body_range().end());
}

#[test]
fn hir_recovery_is_typed_not_raw_expression() {
    let source = r"pub style adaptive {
    when environment(text-scale == 125.55%) {
        Button { opacity = 900milli }
    }
}
";
    let parsed = parse_source(source);
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| { error.code() == "syntax.parse.style_environment.unsupported_value" })
    );
    let hir = lower_to_hir(parsed.typed_tree()).expect("recovered style lowers");
    let environment = style(&hir).sheet().body()[0]
        .as_environment()
        .expect("environment wrapper");
    assert!(matches!(
        environment.clauses()[0].value(),
        HirStyleEnvironmentValue::Recovered(HirStyleEnvironmentRecovery::UnsupportedValue(
            StyleEnvironmentUnsupportedValueKind::FractionalPrecision
        ))
    ));
}
