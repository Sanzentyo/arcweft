use super::*;
use crate::{
    callable::{CallableParameterPresence, CheckedDeclarationDefault},
    final_analysis::{CheckedImplicitCallable, CheckedLocalUseSite},
    semantic_coordinate::{CheckedSemanticPathStep, StableCheckedValueCoordinate},
};

fn view_defaults(source: &str) -> (Vec<CheckedDeclarationDefault>, [u8; 32]) {
    let world = super::fixture(source, None);
    let report = super::analyze(&world).unwrap_or_else(|error| {
        let expression = match &error {
            FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner } => world
                .project
                .analysis_view()
                .unwrap()
                .modules()
                .find_map(|(_, module)| {
                    module
                        .resolve_expr(*owner)
                        .ok()
                        .map(|expr| expr.kind().clone())
                }),
            _ => None,
        };
        panic!("View defaults check: {error:?}\nexpression: {expression:?}\n{source}")
    });
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
        let abi = input_abi(&report, &world, default.source()).unwrap_or_else(|error| {
            panic!("general expression input ABI for a default: {error:?}\n{source}")
        });
        assert_eq!(abi.coordinate().path(), path);
        let projected_result = abi
            .environment()
            .instantiate_type(checked.value_type().expect("typed default"))
            .expect("default result in its checked lexical environment");
        assert_eq!(abi.result().value_type(), Some(&projected_result));
        assert_eq!(abi.effects(), checked.effects());
        assert_eq!(abi.suspension(), default.suspension());
        assert_eq!(abi.control(), default.control());
        let mut expected = default
            .captures()
            .iter()
            .flat_map(|capture| {
                capture.used_locals().iter().map(|local| {
                    (
                        local.local(),
                        local.origin(),
                        abi.environment()
                            .instantiate_type(local.ty())
                            .expect("captured type in its checked lexical environment"),
                    )
                })
            })
            .collect::<Vec<_>>();
        expected.sort_by(|left, right| left.1.cmp(right.1));
        assert_eq!(
            abi.inputs()
                .iter()
                .map(|input| {
                    let binding = input.binding();
                    assert!(!input.uses().is_empty());
                    (binding.local(), binding.origin(), binding.ty().clone())
                })
                .collect::<Vec<_>>(),
            expected,
        );
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
        r#"struct Holder<T> { callback: T }
view Main(first: Holder<i64 -> i64> = Holder { callback = |input: i64| input + 1 }) { Text("static") }"#,
        r#"struct Holder<T> { callback: T }
view Main(first: Holder<i64 -> i64> = Holder { callback = |input: i64| input + 1 }, value: Holder<i64 -> i64> = first) { Text("static") }"#,
        r#"enum Slot<T> { Empty, Full T }
view Main(first: Slot<i64 -> i64> = .Full(|input: i64| input + 1), value: Slot<i64 -> i64> = first) { Text("static") }"#,
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
fn view_default_callback_alias_keeps_earlier_effects_rigid_in_its_owned_abi() {
    let source = r#"view Main(first: i64 -> i64 = |input: i64| input + 1, value: i64 -> i64 = first) { Text("static") }"#;
    let world = super::fixture(source, None);
    let report = super::analyze(&world).unwrap();
    let facts = report
        .checked_callables()
        .records()
        .find(|facts| facts.parameter_defaults().len() == 2)
        .unwrap();
    let defaults = facts.parameter_defaults().values().collect::<Vec<_>>();
    let alias = defaults[1];
    assert_eq!(alias.captures().len(), 1);
    assert!(alias.effects().is_empty());
    let abi = input_abi(&report, &world, alias.source()).unwrap();
    assert_eq!(abi.inputs().len(), 1);
    let captured = report
        .local(abi.inputs()[0].binding().local())
        .unwrap()
        .ty();
    let projected = abi.environment().instantiate_type(&captured).unwrap();
    assert_eq!(abi.result().value_type(), Some(&projected));
    // The earlier input remains declaration-bound, rather than acquiring a
    // globally pure row from its own default initializer.
    assert!(projected.semantic_identity_digest().is_err());
    assert_eq!(&projected, abi.inputs()[0].binding().ty());
    assert!(abi.environment().semantic_type_identity(&projected).is_ok());
    let header = abi.function_type().unwrap();
    assert!(header.semantic_identity_digest().is_ok());
    assert_ne!(alias.expected(), alias.result());
}

#[test]
fn view_default_callback_invocation_retains_the_non_suspending_gate() {
    let world = super::fixture(
        r#"view Main(first: i64 -> i64 = |input: i64| input + 1, value: i64 = first(1i64)) { Text("static") }"#,
        None,
    );
    let result = super::analyze(&world);
    assert!(
        matches!(
            &result,
            Err(FinalSemanticAnalysisError::ViewParameterDefaultSuspension { .. })
        ),
        "{result:?}"
    );
}

#[test]
fn view_default_record_shorthand_preserves_its_parameter_dependency() {
    for source in [
        "struct Label { value: String }\nview Main(value: String, label: Label = Label { value }) { Text(\"value\") }",
        "struct Label { value: String }\nview Main(value: String, label: Label = { let copy = value; Label { value = copy } }) { Text(\"value\") }",
        "struct Label { value: String }\nview Main(value: String, callback: i64 -> Label = |unused: i64| Label { value }) { Text(\"value\") }",
        "struct Label { value: String }\nview Main(label: Label, value: String = label.value) { Text(\"value\") }",
    ] {
        let (defaults, _) = view_defaults(source);
        let [default] = defaults.as_slice() else {
            panic!("one default");
        };
        assert_eq!(default.captures().len(), 1, "{source}");
        assert_eq!(default.captures()[0].parameter().parameter().get(), 0);
        assert_eq!(default.captures()[0].used_locals().len(), 1);
    }
    let (local, _) = view_defaults(
        "struct Label { value: String }\nview Main(label: Label = { let value = \"inner\"; Label { value } }) { Text(\"value\") }",
    );
    assert!(local[0].captures().is_empty());
}

#[test]
fn view_default_implicit_record_value_retains_its_input() {
    for (argument, captures) in [
        ("Label { value = \"fixed\" }", 0),
        ("Label { value = value }", 1),
        ("Label { value }", 1),
    ] {
        let source = format!(
            "struct Label {{ value: String }}\nview Main(value: String, callback: i64 -> (i64, Label) = (_, {argument})) {{ Text(\"value\") }}"
        );
        let (defaults, _) = view_defaults(&source);
        assert_eq!(defaults[0].captures().len(), captures, "{source}");
        let world = super::fixture(&source, None);
        let report = super::analyze(&world).unwrap();
        let callable = report
            .expressions()
            .find_map(|(_, expression)| {
                if let CheckedExpressionResolution::ImplicitCallable(callable) =
                    expression.resolution()
                {
                    Some(callable)
                } else {
                    None
                }
            })
            .expect("implicit callable");
        assert_eq!(
            callable.captures().len(),
            captures,
            "implicit capture packet: {source}"
        );
    }
}

#[test]
fn implicit_record_inputs_preserve_mixed_source_order_and_stable_identity() {
    fn callable(source: &str) -> (arcweft_lang_hir::identity::ExprId, CheckedImplicitCallable) {
        let world = super::fixture(source, None);
        let report = super::analyze(&world).unwrap_or_else(|error| panic!("{error:?}\n{source}"));
        report
            .expressions()
            .find_map(|(owner, expression)| match expression.resolution() {
                CheckedExpressionResolution::ImplicitCallable(callable) => {
                    Some((owner, callable.as_ref().clone()))
                }
                _ => None,
            })
            .expect("implicit callable")
    }
    let source = r#"
struct Pair { first: i64, middle: i64, last: i64 }
view Main(first: i64, last: i64,
    callback: i64 -> (i64, Pair) = (_, Pair { first, middle = first + last, last })) { Text("value") }
"#;
    let (owner, original) = callable(source);
    let occurrences = original.capture_occurrences();
    assert_eq!(occurrences.len(), 4);
    assert_eq!(original.captures().len(), 2);
    assert!(matches!(
        occurrences[0].lookup_site(),
        CheckedLocalUseSite::RecordField {
            source_ordinal: 0,
            ..
        }
    ));
    assert!(matches!(
        occurrences[1].lookup_site(),
        CheckedLocalUseSite::Expression(_)
    ));
    assert!(matches!(
        occurrences[2].lookup_site(),
        CheckedLocalUseSite::Expression(_)
    ));
    assert!(matches!(
        occurrences[3].lookup_site(),
        CheckedLocalUseSite::RecordField {
            source_ordinal: 2,
            ..
        }
    ));
    assert_eq!(occurrences[0].origin(), occurrences[1].origin());
    assert_eq!(occurrences[2].origin(), occurrences[3].origin());
    assert_ne!(occurrences[0].coordinate(), occurrences[1].coordinate());
    let revised = format!(
        "fn unrelated() -> i64 {{ 99i64 }}\n{}",
        source.replace("first + last", "first  +  last")
    );
    let (revised_owner, revised) = callable(&revised);
    assert_ne!(owner, revised_owner);
    assert_eq!(original.identity(), revised.identity());
    assert_eq!(
        occurrences
            .iter()
            .map(|row| row.coordinate())
            .collect::<Vec<_>>(),
        revised
            .capture_occurrences()
            .iter()
            .map(|row| row.coordinate())
            .collect::<Vec<_>>()
    );
    let (_, reordered) = callable(&source.replace(
        "first, middle = first + last, last",
        "last, middle = first + last, first",
    ));
    assert_ne!(original.identity(), reordered.identity());
    assert_eq!(
        original.captures()[1].origin(),
        reordered.captures()[0].origin()
    );
    assert_eq!(view_defaults(source).0[0].captures().len(), 2);
}

#[test]
fn implicit_inputs_include_nested_creation_and_nominal_selection() {
    for source in [
        "struct Label { value: String }\nview Main(label: Label, callback: i64 -> (i64, String) = (_, label.value)) { Text(\"value\") }",
        "view Main(first: i64, callback: i64 -> (i64, i64 -> i64) = (_, |value: i64| value + first)) { Text(\"value\") }",
    ] {
        let (defaults, _) = view_defaults(source);
        assert_eq!(defaults[0].captures().len(), 1, "{source}");
    }
}

#[test]
fn implicit_record_input_preserves_an_open_generic_binding_type() {
    let source = "struct Box<T> { value: T }\nfn make<T>(value: T) -> (i64 -> (i64, Box<T>) effects {}) { (_, Box { value }) }";
    let world = super::fixture(source, None);
    let report = super::analyze(&world)
        .expect("generic record input seals before closed ownership admission");
    let callable = report
        .expressions()
        .find_map(|(_, expression)| match expression.resolution() {
            CheckedExpressionResolution::ImplicitCallable(callable) => Some(callable),
            _ => None,
        })
        .expect("implicit callable");
    let [capture] = callable.captures() else {
        panic!("one generic input");
    };
    assert_eq!(
        callable.capture_occurrences()[0].value_type(),
        capture.value_type()
    );
    assert!(matches!(
        callable.capture_occurrences()[0].lookup_site(),
        CheckedLocalUseSite::RecordField {
            source_ordinal: 0,
            ..
        }
    ));
    assert!(
        report
            .checked_local_uses()
            .value_transfers()
            .all(|(site, _)| !matches!(site, CheckedLocalUseSite::RecordField { .. }))
    );
}

#[test]
fn deferred_record_shorthand_retains_its_creation_input() {
    let source = "struct Label { value: i64 }\nflow main { let value = 1i64; defer { let label = Label { value }; () } }";
    let world = super::fixture(source, None);
    let report = super::analyze(&world).expect("deferred record input");
    let defer = report
        .statements()
        .find_map(|(_, statement)| match statement.payload() {
            crate::final_analysis::CheckedStatementPayload::Defer(defer) => Some(defer),
            _ => None,
        })
        .expect("defer statement");
    assert_eq!(defer.captures().len(), 1);
    assert_eq!(defer.captures()[0].ty(), &TypeKind::I64);
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
        "struct Label { value: String }\nview Main(label: Label = Label { value }, value: String) { Text(\"value\") }",
        "struct Label { value: String }\nview Main(callback: i64 -> (i64, Label) = (_, Label { value }), value: String) { Text(\"value\") }",
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
