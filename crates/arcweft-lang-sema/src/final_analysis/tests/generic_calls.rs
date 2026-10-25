use super::*;

#[test]
fn traversal_retains_the_callback_effect_and_builds_a_first_class_need() {
    let world = fixture(
        r"
fn schedule(paths: Vec<String>, read: String -> Need<Result<i64, String>> effects { fs.read }) -> Need<Result<Vec<i64>, String>> effects { fs.read } {
    let pending = paths.traverse(read)
    pending.parallel(limit = 2)
}
",
        None,
    );
    let analysis = analyze(&world).expect("a stored traversal retains its callback contract");
    let callback = TypeKind::function_with_effects(
        [TypeKind::String],
        TypeKind::Need(Box::new(TypeKind::Result {
            ok: Box::new(TypeKind::I64),
            error: Box::new(TypeKind::String),
        })),
        crate::effect_row::EffectRow::closed(
            crate::effects::EffectSet::from_labels(&["fs.read"]).unwrap(),
        ),
    );
    let expected = TypeKind::Need(Box::new(TypeKind::Result {
        ok: Box::new(TypeKind::Vec(Box::new(TypeKind::I64))),
        error: Box::new(TypeKind::String),
    }));
    let mut constructed = 0;
    let mut scheduled = 0;
    for (_, facts) in analysis.calls() {
        let application = facts
            .selected_application()
            .expect("selected traversal call");
        match application.core().candidates().selected().id() {
            crate::callable::CallableCandidateId::CollectionMethod(
                crate::callable::CollectionMethodId::Traverse { .. },
            ) => {
                assert_eq!(
                    application.result().value_type(),
                    Some(&TypeKind::Traversal {
                        source: Box::new(TypeKind::Vec(Box::new(TypeKind::String))),
                        mapping: Box::new(callback.clone()),
                    })
                );
                constructed += 1;
            }
            crate::callable::CallableCandidateId::CollectionMethod(
                crate::callable::CollectionMethodId::Parallel { .. },
            ) => {
                assert_eq!(application.result().value_type(), Some(&expected));
                scheduled += 1;
            }
            _ => {}
        }
    }
    assert_eq!((constructed, scheduled), (1, 1));
}

#[test]
fn traversal_preserves_rigid_child_types_and_a_dynamic_parallel_limit() {
    let world = fixture(
        r"
fn schedule<T, E>(paths: Vec<String>, read: String -> Need<Result<T, E>> effects { fs.read }, limit: u32) -> Need<Result<Vec<T>, E>> effects { fs.read } {
    let pending = paths.traverse(read)
    pending.parallel(limit = limit)
}
",
        None,
    );
    let analysis = analyze(&world).expect("traversal preserves the enclosing generic identities");
    assert_selected_calls(&analysis, 2);
}

#[test]
fn traversal_accepts_authored_capability_references_and_array_sources() {
    let world = fixture(
        r#"
extern capability fs {
    fn read_text(path: String) -> Need<Result<String, String>> effects { fs.read }
}
fn schedule(paths: Vec<String>) -> Need<Result<Vec<String>, String>> effects { fs.read } {
    let work = paths.traverse(fs.read_text).parallel(limit = 2)
    work
}
fn schedule_array() -> Need<Result<Vec<String>, String>> effects { fs.read } {
    let read = fs.read_text
    let work = ["a.txt", "b.txt"].traverse(read).parallel(limit = 2)
    work
}
"#,
        None,
    );
    let analysis =
        analyze(&world).expect("authored capability callbacks retain their source owner");
    assert_selected_calls(&analysis, 4);
}

#[test]
fn traversal_respects_a_lexical_receiver_shadowing_a_capability_namespace() {
    let world = fixture(
        r"
extern capability fs {
    fn read_text(path: String) -> Need<Result<String, String>> effects { fs.read }
}
struct Reader {
    read_text: String -> Need<Result<i64, String>> effects { fs.read },
}
fn schedule(paths: Vec<String>, fs: Reader) -> Need<Result<Vec<i64>, String>> effects { fs.read } {
    paths.traverse(fs.read_text).parallel(limit = 2)
}
",
        None,
    );
    let analysis = analyze(&world).expect("the actual lexical receiver owns the callback type");
    assert_selected_calls(&analysis, 2);
}

#[test]
fn traversal_rejects_callbacks_outside_its_need_result_contract() {
    for callback in [
        "i64",
        "String -> i64 effects {}",
        "String -> Need<i64> effects {}",
        "i64 -> Need<Result<i64, String>> effects {}",
    ] {
        let source = format!(
            "fn schedule(paths: Vec<String>, read: {callback}) -> Need<Result<Vec<i64>, String>> {{ paths.traverse(read).parallel(limit = 2) }}"
        );
        let world = fixture(&source, None);
        assert!(
            analyze(&world).is_err(),
            "invalid traversal callback: {callback}"
        );
    }
}

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
fn collect_destination_is_tied_to_the_sequence_item_type() {
    let fixture = fixture(
        r#"
fn collect_explicit(items: Seq<i64>) -> Vec<i64> {
    items.collect<Vec<i64>>()
}
fn collect_generic<T>(items: Seq<T>) -> Vec<T> {
    items.collect<Vec<T>>()
}
fn collect_inferred(items: Vec<i64>) -> Vec<i64> {
    items.collect()
}
flow main() -> Vec<i64> {
    let xs = [1i64, 2i64, 3i64]
    let ys = xs.map(|x| x + 1i64).collect<Vec<i64>>()
    return ys
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("collect returns the item-matched Vec destination");
    let mut collection_item_types = Vec::new();
    for (_, facts) in analysis.calls() {
        let application = facts.selected_application().expect("selected call");
        if let crate::callable::CallableCandidateId::CollectionMethod(
            crate::callable::CollectionMethodId::Collect { item },
        ) = application.core().candidates().selected().id()
        {
            let crate::callable::CheckedCallResult::Value(result) = application.result() else {
                panic!("collect has a value result");
            };
            assert_eq!(result, &TypeKind::Vec(Box::new(item.clone())));
            collection_item_types.push(item.clone());
        }
    }
    assert_eq!(collection_item_types.len(), 4);
    assert_eq!(
        collection_item_types
            .iter()
            .filter(|item| **item == TypeKind::I64)
            .count(),
        3
    );
    assert_eq!(
        collection_item_types
            .iter()
            .filter(|item| matches!(item, TypeKind::GenericParam(_)))
            .count(),
        1
    );
}

#[test]
fn collect_rejects_a_destination_with_a_different_item_type() {
    let fixture = fixture(
        r#"
fn collect_wrong_item(items: Seq<i64>) -> Vec<String> {
    items.collect<Vec<String>>()
}
flow main() -> String { return "done" }
"#,
        None,
    );
    let analysis =
        analyze(&fixture).expect("rejected destination keeps its final source-backed call fact");
    assert!(
        analysis.call_diagnostics().any(|diagnostic| {
            diagnostic.code() == crate::callable::CallableDiagnosticCode::NoViableSignature
        }),
        "the selected Vec destination must retain the source sequence item type"
    );
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
    let analysis =
        analyze(&fixture).expect("rejected constructor keeps its final source-backed call fact");
    assert!(
        analysis.call_diagnostics().any(|diagnostic| {
            diagnostic.code() == crate::callable::CallableDiagnosticCode::NoViableSignature
        }),
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
            let executable = fixture.project.analysis_view().expect("executable fixture");
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
    let project = fixture.project.analysis_view().expect("executable fixture");
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
fn correlated_ordinary_call_closes_from_a_later_parent_argument() {
    let fixture = fixture(
        r"
fn empty<T>() -> Option<T> { None }
fn fallback<T>(input: Option<T>, value: T) -> T { value }
flow main() -> i64 { return fallback(empty(), 42i64) }
",
        None,
    );
    let analysis = analyze(&fixture).expect("parent sources close an ordinary child call");
    assert_selected_calls(&analysis, 2);
}

#[test]
fn correlated_child_keeps_the_exact_candidate_when_an_overload_shares_its_result() {
    let generic_owner = generic_test_owner(73);
    let generic_parameter = TypeKind::generic_parameter(GenericTypeParameterId::new(
        GenericParameterOwnerId::AcceptedNominal(generic_owner.clone()),
        0,
    ));
    let generic_issuer = CallableGenericParameterIssuer::accepted_nominal(generic_owner, 1, 0)
        .expect("one accepted generic parameter");
    let fixture = typed_overload_fixture(
        r#"
fn consume<T>(selected: i64, later: T) -> T { later }
flow main() -> String { return consume(choose(42i64), "later") }
"#,
        "choose",
        vec![
            TestCallableOverload::strict([TypeKind::I64], TypeKind::I64),
            TestCallableOverload::strict([generic_parameter], TypeKind::I64)
                .with_generic_issuer(generic_issuer),
        ],
    );
    let analysis = analyze(&fixture).expect("the selected child call seals with its parent");
    assert_selected_calls(&analysis, 2);

    let (child_owner, child) = analysis
        .calls()
        .find_map(|(owner, facts)| {
            let application = facts.selected_application()?;
            matches!(
                application.result(),
                crate::callable::CheckedCallResult::Value(TypeKind::I64)
            )
            .then_some((owner, application))
        })
        .expect("the nested overload call returns i64");
    assert_eq!(
        analysis
            .expression(child_owner)
            .expect("the nested call has a checked-expression fact")
            .type_selection(),
        Some(CheckedTypeSelection::Expected),
        "the child fact retains its exact parent argument expectation"
    );
    let crate::callable::CallableCandidateId::Environment(selected) =
        child.core().candidates().selected().id()
    else {
        panic!("the nested overload retains its environment candidate identity");
    };
    assert_eq!(
        selected.overload().get(),
        0,
        "the exact concrete candidate wins even though the other candidate has the same result"
    );
}

#[test]
fn correlated_ordinary_calls_combine_complementary_parent_evidence() {
    for (left, right) in [
        ("left(1i64)", "right(\"two\")"),
        ("right(\"two\")", "left(1i64)"),
    ] {
        let source = format!(
            "enum Either<A, B> {{ Left A, Right B }}\n\
             fn left<A, B>(value: A) -> Either<A, B> {{ .Left(value) }}\n\
             fn right<A, B>(value: B) -> Either<A, B> {{ .Right(value) }}\n\
             fn combine<A, B>(left: Either<A, B>, right: Either<A, B>) -> i64 {{ 42i64 }}\n\
             flow main() -> i64 {{ return combine({left}, {right}) }}"
        );
        let fixture = fixture(&source, None);
        let analysis = analyze(&fixture).unwrap_or_else(|error| {
            panic!("correlated ordinary arguments {left}, {right}: {error:?}");
        });
        assert_selected_calls(&analysis, 5);
        let closed_owners = analysis
            .calls()
            .filter_map(|(owner, _)| analysis.expression(owner)?.value_type())
            .filter_map(|ty| match ty {
                TypeKind::ProjectNominal(nominal)
                    if nominal.arguments() == [TypeKind::I64, TypeKind::String] =>
                {
                    Some(nominal)
                }
                _ => None,
            })
            .count();
        assert_eq!(
            closed_owners, 2,
            "both child results close under the same parent"
        );
    }
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
        let project = fixture.project.analysis_view().expect("executable fixture");
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

#[test]
fn function_scheme_use_callback_keeps_independent_prefix_instances() {
    let fixture = fixture(
        r#"
fn choose<A, B>(first: A)(second: B) -> B { second }
fn apply<T>(handler: T -> T effects {}, value: T) -> T { handler(value) }
flow main() -> i64 {
    let prefix = choose("saved")
    let text = prefix("text")
    return apply(prefix, 42i64)
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("later parent argument specializes the saved scheme");
    assert_selected_calls(&analysis, 4);
    let uses = analysis
        .expressions()
        .filter_map(|(owner, expression)| {
            expression
                .function_specialization()
                .map(|witness| (owner, expression, witness))
        })
        .collect::<Vec<_>>();
    assert_eq!(uses.len(), 1);
    let (owner, expression, witness) = uses[0];
    assert_eq!(witness.owner(), owner);
    assert_eq!(expression.source_value_type(), Some(witness.source_type()));
    assert!(
        matches!(witness.source_type(), TypeKind::Function { binder, .. } if !binder.is_empty())
    );
    assert!(
        matches!(witness.specialized_type(), TypeKind::Function { binder, params, return_type, .. }
        if binder.is_empty() && params.as_ref() == [TypeKind::I64] && **return_type == TypeKind::I64)
    );
    assert!(analysis.calls().any(|(_, call)| matches!(
        call.selected_application().map(|call| call.result()),
        Some(crate::callable::CheckedCallResult::Value(TypeKind::String))
    )));
}

#[test]
fn function_scheme_use_pending_call_retains_its_original_call_result() {
    for actual in ["identity(prefix)", "identity(identity(prefix))"] {
        let source = format!(
            r#"
fn choose<A, B>(first: A)(second: B) -> B {{ second }}
fn identity<T>(value: T) -> T {{ value }}
fn apply<T>(handler: T -> T effects {{}}, value: T) -> T {{ handler(value) }}
flow main() -> i64 {{ let prefix = choose("saved"); return apply({actual}, 42i64) }}
"#
        );
        let fixture = fixture(&source, None);
        let analysis = analyze(&fixture).unwrap_or_else(|error| panic!("{actual}: {error:?}"));
        let uses = analysis
            .expressions()
            .filter_map(|(owner, expression)| {
                expression
                    .function_specialization()
                    .map(|witness| (owner, expression, witness))
            })
            .collect::<Vec<_>>();
        assert_eq!(uses.len(), 1, "one pending value conversion for {actual}");
        let (owner, expression, witness) = uses[0];
        let call = analysis
            .call(owner)
            .and_then(|call| call.selected_application())
            .expect("Call and Specialize retain the same expression owner");
        assert_eq!(call.result().value_type(), Some(witness.source_type()));
        assert_eq!(expression.value_type(), Some(witness.specialized_type()));
        assert_eq!(
            expression.execution_plan().unwrap().call_application(),
            Some(call.digest())
        );
    }
}

#[test]
fn function_scheme_use_future_only_call_head_keeps_correlated_inference() {
    // The future-only head still belongs to the live producing Call. There
    // is no previously quantified source scheme to instantiate at this use.
    for actual in ["choose(\"pending\")", "identity(choose(\"pending\"))"] {
        let source = format!(
            r#"
fn choose<A, B>(first: A)(second: B) -> B {{ second }}
fn identity<T>(value: T) -> T {{ value }}
fn apply<T>(handler: T -> T effects {{}}, value: T) -> T {{ handler(value) }}
flow main() -> i64 {{ return apply({actual}, 42i64) }}
"#
        );
        let fixture = fixture(&source, None);
        let analysis = analyze(&fixture).unwrap_or_else(|error| panic!("{actual}: {error:?}"));
        assert!(
            analysis
                .calls()
                .all(|(_, call)| call.selected_application().is_some())
        );
        assert!(
            analysis
                .expressions()
                .all(|(_, expression)| expression.function_specialization().is_none())
        );
        assert!(analysis.calls().any(|(_, call)| matches!(call.selected_application().unwrap().result().value_type(), Some(TypeKind::Function { binder, params, return_type, .. }) if binder.is_empty() && params.as_ref() == [TypeKind::I64] && **return_type == TypeKind::I64)));
    }
}

#[test]
fn function_scheme_use_typed_local_retains_its_generic_source() {
    let fixture = fixture(
        r#"
fn choose<A, B>(first: A)(second: B) -> B { second }
flow main() -> i64 {
    let prefix = choose("saved")
    let handler: i64 -> i64 effects {} = prefix
    let text = prefix("still generic")
    return handler(42i64)
}
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("typed local specializes only its initializer use");
    assert_selected_calls(&analysis, 3);
    assert_eq!(
        analysis
            .expressions()
            .filter(|(_, expression)| expression.function_specialization().is_some())
            .count(),
        1
    );
}

#[test]
fn function_scheme_use_root_call_specializes_before_closing_its_component() {
    let fixture = fixture(
        r#"
fn choose<A, B>(first: A)(second: B) -> B { second }
fn identity<T>(value: T) -> T { value }
flow main() -> i64 {
    let prefix = choose("saved")
    let handler: i64 -> i64 effects {} = identity(prefix)
    return handler(42i64)
}
"#,
        None,
    );
    let analysis =
        analyze(&fixture).expect("root call result is specialized inside its own component");
    assert_selected_calls(&analysis, 3);
    let (owner, expression) = analysis
        .expressions()
        .find(|(_, expression)| expression.function_specialization().is_some())
        .expect("root call witness");
    let source = expression.source_value_type().unwrap();
    assert_eq!(
        analysis
            .call(owner)
            .unwrap()
            .selected_application()
            .unwrap()
            .result()
            .value_type(),
        Some(source)
    );
    assert!(matches!(source, TypeKind::Function { binder, .. } if !binder.is_empty()));
}

#[test]
fn function_scheme_use_return_and_tail_preserve_the_source_scheme() {
    for body in [
        "return prefix",
        "prefix",
        "return identity(prefix)",
        "identity(prefix)",
    ] {
        let source = format!(
            r#"
fn choose<A, B>(first: A)(second: B) -> B {{ second }}
fn identity<T>(value: T) -> T {{ value }}
fn maker() -> (i64 -> i64 effects {{}}) {{
    let prefix = choose("saved")
    {body}
}}
flow main() -> i64 {{ let handler = maker(); return handler(42i64) }}
"#
        );
        let fixture = fixture(&source, None);
        let analysis = analyze(&fixture).unwrap_or_else(|error| panic!("{body}: {error:?}"));
        assert!(
            analysis
                .calls()
                .all(|(_, call)| call.selected_application().is_some())
        );
        assert_eq!(
            analysis
                .expressions()
                .filter(|(_, expression)| expression.function_specialization().is_some())
                .count(),
            1,
            "{body}"
        );
    }
}

#[test]
fn function_scheme_use_named_value_uses_the_same_specialization_boundary() {
    let fixture = fixture(
        r"
fn identity<T>(value: T) -> T { value }
fn apply(handler: i64 -> i64 effects {}, value: i64) -> i64 { handler(value) }
flow main() -> i64 { return apply(identity, 42i64) }
",
        None,
    );
    let analysis = analyze(&fixture).expect("named scheme is a checked value use");
    assert_selected_calls(&analysis, 2);
    assert_eq!(
        analysis
            .expressions()
            .filter(|(_, expression)| expression.function_specialization().is_some())
            .count(),
        1
    );
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
            expression
                .execution_plan()
                .expect("selected call has execution authority")
                .call_application(),
            Some(application.digest()),
            "call {owner:?} lost its execution authority: {:?}",
            expression.resolution(),
        );
    }
}

#[test]
fn traversal_accepts_captured_need_result_closure() {
    assert_traversal_callback_source("values.traverse(|path| request(prefix, path))", 3);
}

#[test]
fn traversal_accepts_computed_collection_and_block_callback() {
    assert_traversal_callback_source(
        "paths(values).traverse({ let retained = prefix; |path| request(retained, path) })",
        4,
    );
}

#[test]
fn contextual_callback_preserves_known_parts_of_a_parametric_result() {
    let world = fixture(
        r#"
fn empty<T>() -> Option<T> { None }
fn pair<A, B>(left: A, right: B) -> (A, B) { (left, right) }
fn apply<T>(callback: i64 -> (Option<bool>, T) effects {}) -> (Option<bool>, T) {
    callback(0i64)
}
fn root() -> (Option<bool>, String) {
    apply(|value| pair(empty(), "retained"))
}
"#,
        None,
    );
    let analysis =
        analyze(&world).expect("known contextual structure still guides a nested generic call");
    assert_selected_calls(&analysis, 4);
}

#[test]
fn traversal_accepts_callback_with_explicit_return() {
    assert_traversal_callback_source(
        "values.traverse(|path| { return request(prefix, path) })",
        3,
    );
}

#[test]
fn traversal_accepts_callback_with_a_statement_before_its_tail() {
    assert_traversal_callback_source(
        "values.traverse(|path| { let pending = request(prefix, path); pending })",
        3,
    );
}

#[test]
fn contextual_callback_creation_keeps_nested_body_effects_latent() {
    let world = fixture(
        r#"
fn root(read: String -> String effects { fs.read }) -> bool -> String effects {} {
    |flag| { let retained = |path: String| read(path); "ready" }
}
"#,
        None,
    );
    let analysis = analyze(&world).expect("creating a nested callback does not execute it");
    assert_selected_calls(&analysis, 1);
    let mut closures = 0;
    for (_, expression) in analysis.expressions() {
        if !matches!(
            expression.resolution(),
            CheckedExpressionResolution::Closure(_)
        ) {
            continue;
        }
        let Some(TypeKind::Function {
            params, effects, ..
        }) = expression.value_type()
        else {
            panic!("checked closure has its complete function type")
        };
        let expected = if params == &[TypeKind::String] {
            crate::effects::EffectSet::from_labels(&["fs.read"]).unwrap()
        } else {
            assert_eq!(params, &[TypeKind::Bool]);
            crate::effects::EffectSet::new()
        };
        assert_eq!(effects, &crate::effect_row::EffectRow::closed(expected));
        closures += 1;
    }
    assert_eq!(closures, 2);
}

#[test]
fn contextual_callback_rejects_effectful_return_against_a_pure_function_type() {
    let world = fixture(
        r#"
fn apply(callback: String -> String effects {}) -> String { callback("ready") }
fn root(read: String -> String effects { fs.read }) -> String {
    apply(|path| { return read(path) })
}
"#,
        None,
    );
    let analysis = analyze(&world).expect("the rejected callback retains its call diagnostic");
    let rejected = analysis
        .calls()
        .filter(|(_, call)| call.selected_application().is_none())
        .collect::<Vec<_>>();
    let [(_, call)] = rejected.as_slice() else {
        panic!("one rejected pure callback application")
    };
    let [diagnostic] = call.diagnostics() else {
        panic!("one retained callback effect diagnostic")
    };
    assert_eq!(
        diagnostic.code(),
        crate::callable::CallableDiagnosticCode::NoViableSignature
    );
}

fn assert_traversal_callback_source(expression: &str, calls: usize) {
    let source = format!(
        r#"
extern capability fs {{
    fn read_text(prefix: String, path: String) -> Need<Result<String, String>> effects {{ fs.read }}
}}
fn request(prefix: String, path: String) -> Need<Result<String, String>> effects {{ fs.read }} {{
    fs.read_text(prefix, path)
}}
fn paths(values: Vec<String>) -> Vec<String> {{ values }}
fn root(values: Vec<String>, prefix: String) -> Traversal<Vec<String>, String -> Need<Result<String, String>> effects {{ fs.read }}> effects {{}} {{
    {expression}
}}
"#
    );
    let world = fixture(&source, None);
    let analysis = analyze(&world).expect("contextual callback retains its checked call facts");
    assert_selected_calls(&analysis, calls);
    let closures = analysis
        .expressions()
        .filter(|(_, fact)| matches!(fact.resolution(), CheckedExpressionResolution::Closure(_)))
        .collect::<Vec<_>>();
    assert_eq!(closures.len(), 1);
    let Some(TypeKind::Function {
        params,
        return_type,
        effects,
        ..
    }) = closures[0].1.value_type()
    else {
        panic!("callback retains its full function type")
    };
    assert_eq!(params, &[TypeKind::String]);
    assert_eq!(
        return_type.as_ref(),
        &TypeKind::Need(Box::new(TypeKind::Result {
            ok: Box::new(TypeKind::String),
            error: Box::new(TypeKind::String),
        }))
    );
    assert_eq!(
        effects,
        &crate::effect_row::EffectRow::closed(
            crate::effects::EffectSet::from_labels(&["fs.read"]).unwrap()
        )
    );
}

#[test]
fn traversal_await_match_infers_an_empty_error_branch_without_a_result_annotation() {
    let world = fixture(
        r#"
extern capability fs {
    fn read_text(path: String) -> Need<Result<String, String>> effects { fs.read }
}
flow main() -> String effects { fs.read } {
    let paths = ["a.txt", "b.txt"]
    let values = match (await paths.traverse(fs.read_text).parallel(limit = 2)) {
        .Ok(values) => values
        .Err(_) => []
    }
    "done"
}
"#,
        None,
    );
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let analysis = analyze(&world).expect("traversal result arms share their collection type");
    assert_selected_calls(&analysis, 2);
    let (owner, _) = module
        .expressions()
        .find(|(_, expression)| matches!(expression.kind(), HirExprKind::Match(_)))
        .expect("result Match");
    assert_eq!(
        analysis.expression(owner).unwrap().value_type(),
        Some(&TypeKind::Vec(Box::new(TypeKind::String)))
    );
    let checked = analysis
        .checked_match_with_control(
            project,
            &world.symbols,
            owner,
            CheckedMatchLimits::PRODUCTION,
            FinalSemanticAnalysisControl::new(&AtomicBool::new(false)),
        )
        .expect("exhaustive result transcript");
    assert!(checked.coverage().exhaustive());
    assert_eq!(checked.arms().len(), 2);
}
