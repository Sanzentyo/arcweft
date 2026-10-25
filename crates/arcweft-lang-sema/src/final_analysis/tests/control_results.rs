use super::*;

#[test]
fn unannotated_control_results_share_context_in_both_branch_orders() {
    for expression in [
        "if flag { [] } else { values }",
        "if flag { values } else { [] }",
        "if let .Some(items) = maybe { [] } else { values }",
        "if let .Some(items) = maybe { items } else { [] }",
        "match flag { true => [], false => values }",
        "match flag { true => values, false => [] }",
        "match flag { _ when flag => [], _ => values }",
        "if flag { match flag { true => [], false => values } } else { values }",
    ] {
        let source = format!(
            "fn root(flag: bool, values: Vec<i64>, maybe: Option<Vec<i64>>) -> bool {{ let joined = {expression}; true }}"
        );
        let world = fixture(&source, None);
        let analysis = analyze(&world).unwrap_or_else(|error| panic!("{expression}: {error:?}"));
        let project = world.project.analysis_view().unwrap();
        let module = project.module(&CanonicalModulePath::crate_root()).unwrap();
        let mut checked = 0;
        for (owner, expression) in module.expressions() {
            if matches!(
                expression.kind(),
                HirExprKind::If(_) | HirExprKind::IfLet(_) | HirExprKind::Match(_)
            ) {
                assert_eq!(
                    analysis.expression(owner).unwrap().value_type(),
                    Some(&TypeKind::Vec(Box::new(TypeKind::I64))),
                    "{source}"
                );
                checked += 1;
            }
        }
        assert!(checked != 0);
    }
}

#[test]
fn control_result_inference_replays_generic_calls_with_the_original_capture() {
    let world = fixture(
        r"
fn identity<T>(value: T) -> T { value }
fn root<T>(flag: bool, values: Vec<T>) -> Vec<T> {
    let choose = |condition: bool| match condition {
        true => identity([])
        false => values
    }
    choose(flag)
}
",
        None,
    );
    let analysis = analyze(&world).expect("a rejected synthesis probe is not a published call");
    assert_eq!(analysis.calls().count(), 2);
    assert!(
        analysis
            .calls()
            .all(|(_, call)| call.selected_application().is_some())
    );
    let closures = analysis
        .expressions()
        .filter_map(|(_, value)| match value.resolution() {
            CheckedExpressionResolution::Closure(closure) => Some(closure),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(closures.len(), 1);
    assert_eq!(closures[0].captures().len(), 1);
}

#[test]
fn control_result_inference_rejects_conflicts_and_missing_element_evidence() {
    for expression in [
        "if flag { [] } else { [] }",
        "if flag { [true] } else { values }",
        "match flag { true => [], false => [] }",
        "match flag { true => identity(1i64), false => values }",
    ] {
        let source = format!(
            "fn identity<T>(value: T) -> T {{ value }}\nfn root(flag: bool, values: Vec<i64>) -> bool {{ let joined = {expression}; true }}"
        );
        let world = fixture(&source, None);
        assert!(analyze(&world).is_err(), "{expression}");
    }
}

#[test]
fn contextual_results_constrain_direct_and_nested_generic_arguments_before_probing() {
    for expression in ["identity([])", "retain(identity([]))"] {
        let source = format!(
            r"
fn identity<T>(value: T) -> T {{ value }}
fn retain<T>(values: Vec<T>) -> Vec<T> {{ values }}
fn root<T>() -> Vec<T> {{ {expression} }}
"
        );
        let world = fixture(&source, None);
        let analysis = analyze(&world).unwrap_or_else(|error| panic!("{expression}: {error:?}"));
        assert_eq!(
            analysis.calls().count(),
            if expression == "identity([])" { 1 } else { 2 }
        );
        assert!(
            analysis
                .calls()
                .all(|(_, call)| call.selected_application().is_some())
        );
    }
}

#[test]
fn deeply_nested_unannotated_diverging_branches_keep_the_never_result() {
    let mut expression = "diverge()".to_owned();
    for _ in 0..32 {
        expression = format!("if flag {{ {expression} }} else {{ diverge() }}");
    }
    let source = format!(
        "fn diverge() -> Never {{ loop {{}} }}\nfn root(flag: bool) {{ let done = {expression}; }}"
    );
    let world = fixture(&source, None);
    let analysis =
        analyze(&world).expect("Never supplies a result context without repeated synthesis");
    let project = world.project.analysis_view().unwrap();
    let module = project.module(&CanonicalModulePath::crate_root()).unwrap();
    let mut controls = 0;
    for (owner, expression) in module.expressions() {
        if matches!(expression.kind(), HirExprKind::If(_)) {
            assert_eq!(
                analysis.expression(owner).unwrap().value_type(),
                Some(&TypeKind::Never)
            );
            controls += 1;
        }
    }
    assert_eq!(controls, 32);
    assert_eq!(analysis.calls().count(), 33);
    assert!(
        analysis
            .calls()
            .all(|(_, call)| call.selected_application().is_some())
    );
}

#[test]
fn inferred_control_result_preserves_polymorphic_prefix_schemes() {
    let world = fixture(
        r#"
entry cli @entry.main { goto @flow.main }
fn left<A, B>(first: A)(second: B) -> i64 { 1i64 }
fn right<A, B>(first: A)(second: B) -> i64 { 42i64 }
fn identity<T>(value: T) -> T { value }
fn apply(handler: i64 -> i64 effects {}, value: i64) -> i64 { handler(value) }
fn selected(pick: bool) -> i64 {
    let prefix = if pick { left("left") } else { right("right") }
    return apply(identity(prefix), 9i64)
}
flow main() -> bool {
    let left_result = selected(identity(true))
    let right_result = selected(identity(false))
    return left_result == 1i64 && right_result == 42i64
}
"#,
        None,
    );
    let project = world.project.analysis_view().unwrap();
    let module = project.module(&CanonicalModulePath::crate_root()).unwrap();
    let analysis = analyze(&world).unwrap_or_else(|error| {
        if let FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner } = &error {
            panic!(
                "{error:?}; source expression: {:?}",
                module.resolve_expr(*owner).unwrap()
            );
        }
        panic!("{error:?}");
    });
    let (owner, _) = module
        .expressions()
        .find(|(_, expression)| matches!(expression.kind(), HirExprKind::If(_)))
        .expect("prefix branch");
    assert!(matches!(analysis.expression(owner).unwrap().value_type(),
        Some(TypeKind::Function { binder, .. }) if !binder.is_empty()));
    assert!(
        analysis
            .calls()
            .all(|(_, call)| call.selected_application().is_some())
    );
}
