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
        &source[environment.condition_range().as_range()],
        "(text-scale >= 125.5%)"
    );
    let HirStyleEnvironmentValue::Percentage(percentage) = clause.value() else {
        panic!("typed percentage")
    };
    assert_eq!(percentage.integer_digits(), "125");
    assert_eq!(percentage.fractional_digits(), Some("5"));
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
