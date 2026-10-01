use super::*;
use crate::{
    callable::{CallableParameterPresence, CheckedDeclarationDefault},
    semantic_coordinate::{CheckedSemanticPathStep, StableCheckedValueCoordinate},
};

fn view_defaults(source: &str) -> (Vec<CheckedDeclarationDefault>, [u8; 32]) {
    let world = super::fixture(source, None);
    let report = super::analyze(&world)
        .unwrap_or_else(|error| panic!("View defaults check: {error:?}\n{source}"));
    let symbol = world
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.owner() == arcweft_lang_hir::symbol::CallableDeclarationOwner::View)
        .expect("View declaration");
    let facts = report
        .checked_callables()
        .project_callable(symbol.declaration())
        .expect("checked View callable");
    let defaults = facts
        .parameter_defaults()
        .values()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        defaults.len(),
        facts
            .signature()
            .groups()
            .iter()
            .flat_map(|group| group.parameters())
            .filter(|parameter| parameter.presence() == CallableParameterPresence::Defaulted)
            .count()
    );
    for (position, default) in facts.parameter_defaults() {
        let checked = report
            .expression(default.source())
            .expect("checked default source");
        assert_eq!(
            default.expected(),
            facts
                .signature()
                .parameter_type(*position)
                .expect("declared default type")
                .semantic_identity_digest()
                .expect("semantic expected type")
        );
        assert_eq!(
            checked
                .value_type()
                .expect("typed default")
                .semantic_identity_digest()
                .expect("semantic type"),
            default.result()
        );
        assert!(default.effects().is_empty());
        assert_eq!(default.suspension(), CheckedSuspensionRole::NonSuspending);
        let StableCheckedValueCoordinate::Expression(path) = default.coordinate() else {
            panic!("expression default coordinate")
        };
        assert_eq!(
            path.steps(),
            [CheckedSemanticPathStep::ParameterDefault {
                group: 0,
                parameter: u32::try_from(position.parameter().get()).expect("small inventory")
            }]
        );
        for capture in default.captures() {
            assert!(capture.parameter() < *position);
        }
    }
    (defaults, *facts.interface_digest().as_bytes())
}

#[test]
fn view_parameter_defaults_accept_general_checked_values_and_earlier_inputs() {
    for source in [
        r#"view Main(value: String = "hello") { Text(value) }"#,
        "fn fallback() -> String { \"hello\" }\nview Main(value: String = fallback()) { Text(value) }",
        r#"view Main(first: i64, value: (i64, String) = (first, "hello")) { Text("value") }"#,
        "struct Label { value: String }\nview Main(value: Label = Label { value = \"hello\" }) { Text(\"value\") }",
        "enum Toggle { On, Off }\nview Main(value: Toggle = .On) { Text(\"value\") }",
        r#"view Main(first: i64, callback: i64 -> i64 = |value: i64| value + first) { Text("value") }"#,
        r#"view Main(first: i64, callback: i64 -> i64 = _ + first) { Text("value") }"#,
        r#"view Main(first: i64 = 1, second: i64 = first + 1, third: i64 = second + first) { Text(third) }"#,
        r#"view Main(first: f32 = 1.0, second: f32 = first + 1.0) { Button().fx(wave(speed = second)) }"#,
    ] {
        let (defaults, _) = view_defaults(source);
        assert!(!defaults.is_empty());
    }
    let (chain, _) = view_defaults(
        r#"view Main(first: i64 = 1, second: i64 = first + 1, third: i64 = second + first) { Text(third) }"#,
    );
    assert_eq!(
        chain
            .iter()
            .map(|default| default.captures().len())
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
}

#[test]
fn view_default_match_has_a_parameter_root_and_stable_transcript() {
    let original = r#"
view Main(first: i64, second: i64, chosen: i64 = match true {
    true => first
    false => second
}) {
    Text("prefix")
    Text(chosen)
    Text("suffix")
}


"#;
    let changed = original.replace("true => first", "true => second");
    let revised = format!(
        "fn unrelated() -> i64 {{ 99i64 }}\n{}",
        original.replace("match true", "match  true")
    );
    let (first, interface) = view_defaults(original);
    let (second, changed_interface) = view_defaults(&changed);
    let (revision, revised_interface) = view_defaults(&revised);
    assert_ne!(first[0].source(), revision[0].source());
    assert_eq!(first[0].coordinate(), revision[0].coordinate());
    assert_eq!(first[0].expression(), revision[0].expression());
    assert_eq!(interface, revised_interface);
    assert_ne!(first[0].expression(), second[0].expression());
    assert_ne!(interface, changed_interface);
    super::body_roots::assert_match_sensitivity_and_source_revision_invariance(
        "View parameter default",
        original,
        &changed,
        &revised,
    );
    let sibling = original.replace("prefix", "changed prefix");
    assert_eq!(source_match_digest(original), source_match_digest(&sibling));
    assert_eq!(
        view_defaults(&sibling).0[0].expression(),
        first[0].expression()
    );
}

#[test]
fn view_parameter_defaults_reject_self_later_and_latent_forward_inputs() {
    for source in [
        r#"view Main(value: i64 = value) { Text(value) }"#,
        r#"view Main(value: i64 = later, later: i64) { Text(value) }"#,
        r#"view Main(callback: i64 -> i64 = |value: i64| value + later, later: i64) { Text("value") }"#,
        r#"view Main(callback: i64 -> i64 = _ + later, later: i64) { Text("value") }"#,
    ] {
        let world = super::fixture(source, None);
        let error =
            super::analyze(&world).expect_err("forward default dependency must not publish");
        assert!(
            matches!(
                error,
                FinalSemanticAnalysisError::ViewParameterDefaultForwardInput { .. }
                    | FinalSemanticAnalysisError::ExpressionTypeUnavailable { .. }
            ),
            "{source}: {error:?}"
        );
    }
}

#[test]
fn view_parameter_defaults_reject_wrong_types_effect_execution_and_suspension() {
    let wrong = super::fixture(r#"view Main(value: i64 = "wrong") { Text(value) }"#, None);
    assert!(matches!(
        super::analyze(&wrong),
        Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { .. })
    ));
    let effect = super::fixture(
        r#"view Main(value: i64 = { let handle = thread {}; 1i64 }) { Text(value) }"#,
        None,
    );
    let effect_result = super::analyze(&effect);
    assert!(
        matches!(
            effect_result,
            Err(FinalSemanticAnalysisError::ViewParameterDefaultEffects { .. })
        ),
        "{effect_result:?}"
    );
    let suspension = super::fixture(
        r#"view Main(need: Need<i64>, value: i64 = await need) { Text(value) }"#,
        None,
    );
    let suspension_result = super::analyze(&suspension);
    assert!(
        matches!(
            suspension_result,
            Err(FinalSemanticAnalysisError::ViewParameterDefaultEffects { .. })
        ),
        "{suspension_result:?}"
    );
}
