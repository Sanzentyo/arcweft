use super::*;

#[test]
fn generic_result_constructor_retains_enclosing_parameter_identity() {
    let fixture = fixture(
        r"
fn wrap<T>(value: T) -> Result<T, String> {
    Ok(value)
}
",
        None,
    );
    analyze(&fixture).expect("constructor inference preserves the enclosing generic parameter");
}

#[test]
fn contextual_unit_variant_closes_with_the_later_argument_type() {
    let fixture = fixture(
        r"
fn choose<T>(input: Option<T>, fallback: T) -> T {
    if let Some(value) = input { value } else { fallback }
}
flow main() -> i64 { return choose(None, 42i64) }
",
        None,
    );
    let analysis = analyze(&fixture).expect("the enclosing candidate closes the contextual owner");
    assert_selected_calls(&analysis, 1);
    let variants = analysis
        .expressions()
        .filter_map(|(_, expression)| match expression.resolution() {
            CheckedExpressionResolution::Variant(variant) => Some(variant),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(variants.len(), 1);
    let variant = variants[0];
    let expected = TypeKind::Option(Box::new(TypeKind::I64));
    assert_eq!(variant.owner().ty(), expected);
    assert_eq!(
        variant.owner().semantic_type(),
        expected.semantic_identity_digest().expect("closed owner")
    );
    assert_eq!(variant.ordinal(), 1);
    assert!(variant.selected().payload().is_unit());
}

#[test]
fn generic_closure_call_reuses_its_rigid_parameter() {
    let fixture = fixture(
        r"
fn apply<T>(value: T) -> T {
    let handler = |item: T| item
    handler(value)
}
",
        None,
    );
    analyze(&fixture).expect("a function value shares its enclosing rigid type parameter");
}

#[test]
fn generic_constructor_cannot_bind_an_enclosing_parameter_to_another_type() {
    let fixture = fixture(
        r"
fn wrap<T>(value: T) -> Result<String, String> {
    Ok(value)
}
",
        None,
    );
    assert!(
        analyze(&fixture).is_err(),
        "T must remain rigid during constructor inference"
    );
}

#[test]
fn recursive_generic_call_keeps_its_enclosing_type_rigid() {
    let fixture = fixture(
        r"
fn repeat<T>(value: T, count: i64) -> T {
    if count == 0i64 { value } else { repeat(value, count - 1i64) }
}
",
        None,
    );
    analyze(&fixture).expect("recursive generic calls retain the enclosing type identity");
}

#[test]
fn a_shared_generic_prefix_retains_each_accepted_call_execution() {
    let fixture = fixture(
        r#"
fn choose<A, B>(first: A)(second: B) -> B { second }
flow main() -> i64 {
    let prefix = choose(1i64)
    let text = prefix("text")
    return prefix(42i64)
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("each use opens the shared prefix independently");
    assert_selected_calls(&analysis, 3);
    let terminal_results = analysis
        .calls()
        .filter_map(|(_, facts)| {
            match facts
                .selected_application()
                .expect("selected call")
                .result()
            {
                crate::callable::CheckedCallResult::Value(ty) => Some(ty.clone()),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(terminal_results, [TypeKind::String, TypeKind::I64]);
}

#[test]
fn each_saved_group_reopens_only_its_remaining_generic_parameters() {
    let fixture = fixture(
        r#"
fn choose<A, B, C>(first: A)(second: B)(third: C) -> C { third }
flow main() -> i64 {
    let first = choose(1i64)
    let bool_prefix = first(true)
    let text = bool_prefix("text")
    let text_prefix = first("first")
    return text_prefix(42i64)
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("saved groups retain independent residual scopes");
    assert_selected_calls(&analysis, 5);
    let terminal_results = analysis
        .calls()
        .filter_map(|(_, facts)| {
            match facts
                .selected_application()
                .expect("selected call")
                .result()
            {
                crate::callable::CheckedCallResult::Value(ty) => Some(ty.clone()),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(terminal_results, [TypeKind::String, TypeKind::I64]);
}

#[test]
fn contextual_project_payload_constructor_participates_in_parent_inference() {
    let fixture = fixture(
        r"
enum Slot<T> { Empty, Full T }
fn fallback<T>(input: Slot<T>, value: T) -> T {
    if let .Full(item) = input { item } else { value }
}
flow main() -> i64 { return fallback(.Full(42i64), 0i64) }
",
        None,
    );
    let analysis = analyze(&fixture).unwrap_or_else(|error| {
        if let FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner } = &error {
            let executable = fixture
                .project
                .executable_view()
                .expect("executable fixture");
            let expression = executable
                .modules()
                .find_map(|(_, module)| module.resolve_expr(*owner).ok())
                .expect("failed expression exists");
            panic!(
                "constructor inference failed: {error:?}; expression: {:?}",
                expression.kind()
            );
        }
        panic!("constructor inference failed: {error:?}");
    });
    assert_selected_calls(&analysis, 2);
    let project = fixture
        .project
        .executable_view()
        .expect("executable fixture");
    let constructors = analysis
        .calls()
        .filter_map(|(_, facts)| {
            let application = facts.selected_application().expect("selected call");
            analysis
                .execution_projection()
                .variant_constructor(project, application)
                .expect("exact completed constructor projection")
        })
        .collect::<Vec<_>>();
    assert_eq!(constructors.len(), 1);
    let constructor = &constructors[0];
    let TypeKind::ProjectNominal(nominal) = constructor.owner().ty() else {
        panic!("constructor retains the project nominal");
    };
    assert_eq!(nominal.arguments(), &[TypeKind::I64]);
    assert_eq!(constructor.ordinal(), 1);
    assert_eq!(constructor.owner().cases().len(), 2);
    assert_eq!(
        analysis
            .project_variant_owner(constructor.owner().semantic_type())
            .expect("catalog projection")
            .as_ref(),
        Some(constructor.owner()),
    );
    let patterns = analysis
        .patterns()
        .filter_map(|(_, pattern)| match pattern.resolution() {
            CheckedPatternResolution::Variant(variant) => Some(variant),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(patterns.len(), 1);
    let pattern = patterns[0];
    let TypeKind::ProjectNominal(nominal) = pattern.owner().ty() else {
        panic!("generic pattern retains the project nominal");
    };
    assert!(matches!(
        nominal.arguments(),
        [TypeKind::GenericParam(
            crate::types::GenericTypeReference::Free(_)
        )]
    ));
    assert_eq!(
        analysis
            .project_variant_owner(pattern.owner().semantic_type())
            .expect("generic catalog projection")
            .as_ref(),
        Some(pattern.owner()),
    );
}

#[test]
fn contextual_unit_constructor_call_closes_from_a_later_argument() {
    let fixture = fixture(
        r"
enum Slot<T> { Empty, Full T }
fn fallback<T>(input: Slot<T>, value: T) -> T { value }
flow main() -> i64 { return fallback(.Empty(), 42i64) }
",
        None,
    );
    let analysis = analyze(&fixture).expect("a later argument closes the unit constructor call");
    assert_selected_calls(&analysis, 2);
}

#[test]
fn contextual_constructor_call_closes_an_unselected_case_parameter() {
    let fixture = fixture(
        r#"
enum Either<A, B> { Left A, Right B }
fn fallback<A, B>(input: Either<A, B>, value: A, other: B) -> A {
    if let .Left(item) = input { item } else { value }
}
flow main() -> i64 { return fallback(.Left(42i64), 0i64, "other") }
"#,
        None,
    );
    let analysis =
        analyze(&fixture).expect("the complete constructor owner follows parent inference");
    assert_selected_calls(&analysis, 2);
}

#[test]
fn contextual_constructor_sources_combine_complementary_type_evidence() {
    for (left, right) in [
        (".Left(1i64)", ".Right(\"two\")"),
        (".Right(\"two\")", ".Left(1i64)"),
    ] {
        let source = format!(
            "enum Either<A, B> {{ Left A, Right B }}\n\
             fn combine<A, B>(left: Either<A, B>, right: Either<A, B>) -> i64 {{ 42i64 }}\n\
             flow main() -> i64 {{ return combine({left}, {right}) }}"
        );
        let fixture = fixture(&source, None);
        let analysis = analyze(&fixture).unwrap_or_else(|error| {
            panic!("complementary constructor arguments {left}, {right}: {error:?}");
        });
        assert_selected_calls(&analysis, 3);
        let project = fixture
            .project
            .executable_view()
            .expect("executable fixture");
        let constructors = analysis
            .calls()
            .filter_map(|(_, facts)| {
                analysis
                    .execution_projection()
                    .variant_constructor(
                        project,
                        facts.selected_application().expect("selected call"),
                    )
                    .expect("completed constructor")
            })
            .collect::<Vec<_>>();
        assert_eq!(constructors.len(), 2);
        for constructor in constructors {
            let TypeKind::ProjectNominal(nominal) = constructor.owner().ty() else {
                panic!("constructor retains its nominal owner");
            };
            assert_eq!(nominal.arguments(), [TypeKind::I64, TypeKind::String]);
        }
    }
}

fn assert_selected_calls(analysis: &FinalSemanticAnalysis, expected_count: usize) {
    let calls = analysis.calls().collect::<Vec<_>>();
    assert_eq!(calls.len(), expected_count);
    for (owner, facts) in calls {
        let application = facts.selected_application().unwrap_or_else(|| {
            panic!("call {owner:?} was not accepted: {:?}", facts.diagnostics())
        });
        let expression = analysis
            .expression(owner)
            .expect("call expression is retained");
        assert_eq!(
            expression.execution_plan().call_application(),
            Some(application.digest()),
            "call {owner:?} lost its execution authority: {:?}",
            expression.resolution(),
        );
    }
}
