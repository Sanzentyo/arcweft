//! Effect inference must preserve application-specific rows and invocation timing.

use super::{analyze, fixture};
use crate::{
    callable::{
        CallableCandidateId, CheckedCallApplication, CheckedProjectFunctionRuntimeInput,
        CheckedProjectFunctionRuntimeOutcome, select_project_function_runtime,
    },
    effect_row::EffectRowTail,
    final_analysis::FinalSemanticAnalysis,
    types::TypeKind,
};
use arcweft_lang_hir::{identity::ExprId, symbol::CallableDeclarationKey};

fn project_applications<'a>(
    analysis: &'a FinalSemanticAnalysis,
    name: &str,
) -> Vec<(ExprId, &'a CheckedCallApplication)> {
    analysis
        .calls()
        .filter_map(|(owner, call)| {
            let application = call.selected_application()?;
            let CallableCandidateId::Project(CallableDeclarationKey::Existing(declaration)) =
                application.core().candidates().selected().id()
            else {
                return None;
            };
            if declaration.name() != name {
                return None;
            }
            Some((owner, application))
        })
        .collect()
}

fn application_rows(analysis: &FinalSemanticAnalysis, name: &str) -> Vec<Vec<String>> {
    let mut rows = project_applications(analysis, name)
        .into_iter()
        .map(|(_, application)| {
            assert_eq!(application.core().effects().tail(), EffectRowTail::Closed);
            application.core().effects().concrete().to_labels()
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

#[test]
fn effectful_terminal_prefix_cannot_be_passed_to_a_pure_callback() {
    let fixture = fixture(
        r#"
fn staged(first: i64)(second: i64) -> i64 effects { fs.read } { first + second }
fn invoke(handler: i64 -> i64 effects {}, value: i64) -> i64 { handler(value) }
fn caller() {
    let prefix = staged(1i64)
    invoke(prefix, 41i64);
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("effect mismatch remains inspectable by tooling");
    let rejected = analysis
        .calls()
        .filter(|(_, call)| {
            matches!(
                call.outcome(),
                crate::callable::CallAnalysisOutcome::Rejected(_)
            )
        })
        .collect::<Vec<_>>();
    let [(owner, _)] = rejected.as_slice() else {
        panic!("only the pure callback invocation is rejected");
    };
    super::callable_values::assert_unselected_call_has_no_execution(&analysis, *owner);
    let prefixes = project_applications(&analysis, "staged");
    let [(_, prefix)] = prefixes.as_slice() else {
        panic!("the prefix itself is valid");
    };
    assert!(prefix.core().effects().concrete().is_empty());
    let Some(TypeKind::Function { effects, .. }) = prefix.result().value_type() else {
        panic!("retained terminal group");
    };
    assert_eq!(effects.concrete().to_labels(), ["fs.read"]);
}

#[test]
fn explicit_callback_rows_combine_only_when_invoked() {
    let fixture = fixture(
        r#"
fn reader(value: i64) -> i64 effects { fs.read } { value }
fn writer(value: i64) -> i64 effects { fs.write } { value }
fn both(first: i64 -> i64 effects { fs.read }, second: i64 -> i64 effects { fs.write }, value: i64) -> i64 {
    second(first(value))
}
fn first_only(first: i64 -> i64 effects { fs.read }, unused: i64 -> i64 effects { fs.write }, value: i64) -> i64 {
    first(value)
}
flow main() -> i64 {
    let a = both(|value: i64| reader(value), |value: i64| writer(value), 20i64)
    let b = first_only(|value: i64| reader(value), |value: i64| writer(value), 22i64)
    return a + b
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("explicit callback row control");
    assert_eq!(
        application_rows(&analysis, "both"),
        [vec!["fs.read".to_owned(), "fs.write".to_owned()]]
    );
    assert_eq!(
        application_rows(&analysis, "first_only"),
        [vec!["fs.read".to_owned()]]
    );
}

#[test]
fn uninvoked_inferred_callback_does_not_contribute_an_invocation_effect() {
    let fixture = fixture(
        r#"
fn writer(value: i64) -> i64 effects { fs.write } { value }
fn ignore(unused: i64 -> i64, value: i64) -> i64 { value }
flow main() -> i64 { return ignore(|value: i64| writer(value), 42i64) }
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("an unused inferred callback has no invocation edge");
    assert_eq!(
        application_rows(&analysis, "ignore"),
        [Vec::<String>::new()]
    );
    let calls = project_applications(&analysis, "ignore");
    let [(owner, application)] = calls.as_slice() else {
        panic!("one ignore call")
    };
    let selection = select_project_function_runtime(
        application,
        analysis
            .checked_callable_join(*owner)
            .expect("exact checked join"),
        analysis.checked_callables(),
    )
    .expect("parameter ABI projection")
    .expect("project function");
    assert!(selection.effects().is_empty());
    let callback = &selection.current_group_materialization()[0];
    let TypeKind::Function { effects, .. } = callback.abi_type() else {
        panic!("callback ABI")
    };
    assert_eq!(effects.tail(), EffectRowTail::Closed);
    assert_eq!(effects.concrete().to_labels(), ["fs.write"]);
    assert_eq!(callback.binding_type(), callback.abi_type());
    let instance = selection.close_instance(None).expect("closed callable ABI");
    assert!(
        matches!(instance.function_type(), TypeKind::Function { params, .. }
        if params.first() == Some(callback.abi_type()))
    );
}

#[test]
fn curried_callback_prefix_abi_keeps_the_closed_parameter_row() {
    let fixture = fixture(
        r#"
fn writer(value: i64) -> i64 effects { fs.write } { value }
fn ignore(unused: i64 -> i64)(value: i64) -> i64 { value }
flow main() -> i64 {
    let prefix = ignore(|value: i64| writer(value))
    return prefix(42i64)
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("curried callback semantic analysis");
    let calls = project_applications(&analysis, "ignore");
    assert_eq!(calls.len(), 2);
    let mut produced = None;
    let mut consumed = None;
    for (owner, application) in calls {
        let selection = select_project_function_runtime(
            application,
            analysis
                .checked_callable_join(owner)
                .expect("exact checked join"),
            analysis.checked_callables(),
        )
        .expect("prefix ABI projection")
        .expect("project function");
        assert!(selection.effects().is_empty());
        if let CheckedProjectFunctionRuntimeInput::Continuation { abi } = selection.input() {
            consumed = Some(abi.clone());
        }
        if let CheckedProjectFunctionRuntimeOutcome::Continue { abi, .. } = selection.outcome() {
            produced = Some(abi.clone());
        }
    }
    let produced = produced.expect("one prefix producer");
    assert_eq!(consumed.as_ref(), Some(&produced));
    let [TypeKind::Function { effects, .. }] = produced.prefix_types() else {
        panic!("the prefix retains one callback binding")
    };
    assert_eq!(effects.tail(), EffectRowTail::Closed);
    assert_eq!(effects.concrete().to_labels(), ["fs.write"]);
}

#[test]
fn contextual_effect_rows_survive_value_expression_boundaries() {
    let callback = TypeKind::function_with_effects(
        [TypeKind::I64],
        TypeKind::I64,
        crate::effect_row::EffectRow::closed(
            [crate::effects::EffectId::parse("fs.write").expect("effect identity")]
                .into_iter()
                .collect(),
        ),
    );
    for (name, parameter, argument, expected) in [
        (
            "block",
            "i64 -> i64",
            "{ let offset = 0i64\n |value| writer(value + offset) }",
            callback.clone(),
        ),
        (
            "conditional",
            "i64 -> i64",
            "if true { |value| writer(value) } else { |value| writer(value) }",
            callback.clone(),
        ),
        (
            "tuple",
            "(i64 -> i64, i64)",
            "(|value| writer(value), 1i64)",
            TypeKind::Tuple(vec![callback.clone(), TypeKind::I64]),
        ),
        (
            "sequence",
            "Vec<i64 -> i64>",
            "[|value| writer(value)]",
            TypeKind::Vec(Box::new(callback)),
        ),
    ] {
        let source = format!(
            "fn writer(value: i64) -> i64 effects {{ fs.write }} {{ value }}\n\
             fn ignore(unused: {parameter}, value: i64) -> i64 {{ value }}\n\
             flow main() -> i64 {{ return ignore({argument}, 42i64) }}"
        );
        let fixture = fixture(&source, None);
        let analysis = analyze(&fixture).unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert_eq!(
            application_rows(&analysis, "ignore"),
            [Vec::<String>::new()],
            "{name}"
        );
        let calls = project_applications(&analysis, "ignore");
        let [(owner, application)] = calls.as_slice() else {
            panic!("{name}: one selected call");
        };
        let selection = select_project_function_runtime(
            application,
            analysis
                .checked_callable_join(*owner)
                .expect("checked join"),
            analysis.checked_callables(),
        )
        .unwrap_or_else(|error| panic!("{name}: {error:?}"))
        .expect("project function");
        assert_eq!(
            selection.current_group_materialization()[0].abi_type(),
            &expected,
            "{name}"
        );
    }
}

#[test]
fn inferred_callback_row_preserves_the_exposed_nonempty_contract() {
    let fixture = fixture(
        r#"
fn reader(value: i64) -> i64 effects { fs.read } { value }
fn apply(handler: i64 -> i64, value: i64) -> i64 { handler(value) }
flow main() -> i64 { return apply(|value: i64| reader(value), 42i64) }
"#,
        None,
    );
    let analysis =
        analyze(&fixture).expect("omitted callback effects are inferred from the argument");
    assert_eq!(
        application_rows(&analysis, "apply"),
        [vec!["fs.read".to_owned()]]
    );
}

#[test]
fn inferred_callback_rows_join_independent_invoked_parameters() {
    let fixture = fixture(
        r#"
fn reader(value: i64) -> i64 effects { fs.read } { value }
fn writer(value: i64) -> i64 effects { fs.write } { value }
fn both(first: i64 -> i64, second: i64 -> i64, value: i64) -> i64 { second(first(value)) }
flow main() -> i64 {
    return both(|value: i64| reader(value), |value: i64| writer(value), 42i64)
}
"#,
        None,
    );
    let analysis =
        analyze(&fixture).expect("independent callback rows flow into the invoking body");
    assert_eq!(
        application_rows(&analysis, "both"),
        [vec!["fs.read".to_owned(), "fs.write".to_owned()]]
    );
}

#[test]
fn uninvoked_callback_row_remains_latent() {
    let fixture = fixture(
        r#"
fn reader(value: i64) -> i64 effects { fs.read } { value }
fn writer(value: i64) -> i64 effects { fs.write } { value }
fn first_only(first: i64 -> i64, unused: i64 -> i64, value: i64) -> i64 { first(value) }
flow main() -> i64 {
    return first_only(|value: i64| reader(value), |value: i64| writer(value), 42i64)
}
"#,
        None,
    );
    let analysis =
        analyze(&fixture).expect("creating an unused callback does not invoke its effect row");
    assert_eq!(
        application_rows(&analysis, "first_only"),
        [vec!["fs.read".to_owned()]]
    );
}

#[test]
fn one_declaration_keeps_distinct_rows_at_distinct_applications() {
    let fixture = fixture(
        r#"
fn reader(value: i64) -> i64 effects { fs.read } { value }
fn apply(handler: i64 -> i64, value: i64) -> i64 { handler(value) }
flow main() -> i64 {
    let pure = apply(|value: i64| value, 20i64)
    let reading = apply(|value: i64| reader(value), 22i64)
    return pure + reading
}
"#,
        None,
    );
    let analysis =
        analyze(&fixture).expect("one source body retains separate closed effect instances");
    assert_eq!(
        application_rows(&analysis, "apply"),
        [Vec::<String>::new(), vec!["fs.read".to_owned()]]
    );
}

#[test]
fn curried_function_types_expose_effects_only_on_the_terminal_group() {
    let fixture = fixture(
        r#"
fn staged(first: i64)(second: i64)(third: i64) -> i64 effects { fs.read } {
    first + second + third
}
flow main() -> i64 {
    let first = staged(1i64)
    let second = first(2i64)
    return second(39i64)
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("each curried group has a checked effect boundary");
    let applications = project_applications(&analysis, "staged");
    assert_eq!(applications.len(), 3);
    for (_, application) in applications {
        let group = application.core().current_group().get();
        assert_eq!(application.core().effects().tail(), EffectRowTail::Closed);
        let expected_call = if group == 2 {
            vec!["fs.read"]
        } else {
            Vec::new()
        };
        assert_eq!(
            application.core().effects().concrete().to_labels(),
            expected_call
        );
        let mut result = application
            .result()
            .value_type()
            .expect("value or continuation");
        for remaining in group + 1..3 {
            let TypeKind::Function {
                return_type,
                effects,
                ..
            } = result
            else {
                panic!("remaining group {remaining} is a function type");
            };
            let expected = if remaining == 2 {
                vec!["fs.read"]
            } else {
                Vec::new()
            };
            assert_eq!(effects.tail(), EffectRowTail::Closed);
            assert_eq!(
                effects.concrete().to_labels(),
                expected,
                "remaining group {remaining}"
            );
            result = return_type;
        }
        assert_eq!(result, &TypeKind::I64);
    }
}
