use super::*;

fn array_type(item: TypeKind, length: usize) -> TypeKind {
    TypeKind::Array {
        item: Box::new(item),
        len: crate::types::ArrayLength::Const(length),
    }
}

#[test]
fn bracket_sequences_follow_array_and_vec_contexts() {
    let fixture = fixture(
        r#"
fn fixed_numeric() -> Array<i32, 3> {
    let values: Array<i32, 3> = [1i32, 2i32, 3i32]
    values
}
fn fixed_general() -> Array<String, 2> { ["north", "south"] }
fn dynamic_numeric() -> Vec<i32> { [4i32, 5i32] }
fn inferred_numeric() { let values = [6i32, 7i32] }
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("bracket sequences follow their collection context");
    let numeric_array = array_type(TypeKind::I32, 3);
    let general_array = array_type(TypeKind::String, 2);
    let vector = TypeKind::Vec(Box::new(TypeKind::I32));

    assert!(
        analysis
            .expressions()
            .any(|(_, expression)| expression.value_type() == Some(&numeric_array))
    );
    assert!(
        analysis
            .expressions()
            .any(|(_, expression)| expression.value_type() == Some(&general_array))
    );
    assert!(
        analysis
            .expressions()
            .filter(|(_, expression)| expression.value_type() == Some(&vector))
            .count()
            >= 2
    );
}

#[test]
fn bracket_sequences_reject_array_length_mismatches() {
    for (source, description) in [
        (
            "fn invalid() -> Array<i32, 2> { [1i32, 2i32, 3i32] }\n",
            "numeric sequence",
        ),
        (
            "fn invalid() -> Array<String, 2> { [\"north\", \"south\", \"east\"] }\n",
            "general sequence",
        ),
    ] {
        assert!(
            analyze(&fixture(source, None)).is_err(),
            "{description} must not satisfy an Array with a different length"
        );
    }
}

#[test]
fn compact_numeric_elements_fit_the_selected_integer_type() {
    let accepted = fixture(
        "fn unsigned() -> Vec<u8> { [255u8] }\nfn signed() -> Vec<i8> { [127i8] }\nfn wide() -> Vec<u128> { [340282366920938463463374607431768211455u128] }\n",
        None,
    );
    analyze(&accepted).expect("exact integer maxima fit their selected item types");

    for (source, item) in [
        ("fn rejected() -> Vec<u8> { [256u8] }\n", TypeKind::U8),
        ("fn rejected() -> Vec<i8> { [128i8] }\n", TypeKind::I8),
        (
            "fn rejected() -> Vec<u128> { [340282366920938463463374607431768211456u128] }\n",
            TypeKind::U128,
        ),
    ] {
        assert!(matches!(
            analyze(&fixture(source, None)),
            Err(FinalSemanticAnalysisError::CompactNumericElementOutOfRange {
                ordinal: 0,
                item: actual,
                ..
            }) if *actual == item
        ));
    }
}

#[test]
fn array_repeats_preserve_constant_lengths_and_contextual_items() {
    let fixture = fixture(
        r#"
fn repeated_numeric() -> Array<i32, 4> {
    let values: Array<i32, 4> = [0i32; 4i64]
    values
}
fn repeated_general() -> Array<String, 2> { ["echo"; 2u64] }
fn repeated_inferred() { let values = [7i32; 3u64] }
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("integer array-repeat lengths are retained");

    for expected in [
        array_type(TypeKind::I32, 4),
        array_type(TypeKind::String, 2),
        array_type(TypeKind::I32, 3),
    ] {
        assert!(
            analysis
                .expressions()
                .any(|(_, expression)| expression.value_type() == Some(&expected))
        );
    }
}

#[test]
fn array_repeats_reject_length_mismatches_and_runtime_lengths() {
    for (source, description) in [
        (
            "fn invalid() -> Array<i32, 3> { [0i32; 4i64] }\n",
            "numeric repeat",
        ),
        (
            "fn invalid() -> Array<String, 2> { [\"echo\"; 3u64] }\n",
            "general repeat",
        ),
        (
            "fn invalid(length: usize) -> Array<i32, 4> { [0i32; length] }\n",
            "runtime repeat length",
        ),
    ] {
        assert!(
            analyze(&fixture(source, None)).is_err(),
            "{description} must not satisfy a fixed compile-time Array shape"
        );
    }
}

#[test]
fn contextual_map_callbacks_infer_nested_ordinary_call_results() {
    let fixture = fixture(
        r"
fn score(value: i32) -> i32 effects {} { value * 4i32 }
fn scaled(value: i32) -> f64 effects {} { 1.5f64 }
fn inferred(values: Vec<i32>) -> Vec<i32> effects {} {
    let mapped = values.map(|item| score(item))
    mapped
}
fn annotated(values: Vec<i32>) -> Vec<f64> effects {} {
    let mapped: Vec<f64> = values.map(|item: i32| scaled(item))
    mapped
}
fn fixed(values: Array<i32, 3>) -> Array<i32, 3> effects {} {
    values.map(|item| score(item))
}
",
        None,
    );
    let analysis =
        analyze(&fixture).expect("parent callback inference owns the nested call result");
    for expected in [
        TypeKind::Vec(Box::new(TypeKind::I32)),
        TypeKind::Vec(Box::new(TypeKind::F64)),
        array_type(TypeKind::I32, 3),
    ] {
        assert!(
            analysis.calls().any(|(_, call)| {
                call.selected_application()
                    .is_some_and(|application| application.result().value_type() == Some(&expected))
            }),
            "the selected map must retain its exact result: {expected:?}"
        );
    }
}

#[test]
fn contextual_map_callbacks_reject_a_concrete_result_mismatch() {
    for callback in [
        "|item| score(item)",
        "|item| { score(item) }",
        "|item: i32| -> String { score(item) }",
    ] {
        let rejected_call = format!("values.map({callback})");
        let source = format!(
            "fn score(value: i32) -> i32 effects {{}} {{ value * 4i32 }}\n\
             fn rejected(values: Vec<i32>) -> Vec<String> effects {{}} {{\n\
                 {rejected_call}\n\
             }}\n"
        );
        let fixture = fixture(&source, None);
        let analysis = analyze(&fixture).unwrap_or_else(|error| {
            let unavailable = match &error {
                FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner } => {
                    let view = fixture.project.analysis_view().expect("executable HIR");
                    let module = view
                        .module(&CanonicalModulePath::crate_root())
                        .expect("root HIR module");
                    let expression = module.resolve_expr(*owner).expect("failed expression owner");
                    let site = module
                        .source_site(
                            module.provenance().source_identity(),
                            HirSourceQuery::Expr {
                                owner: *owner,
                                role: HirExprSourceRole::Whole,
                            },
                        )
                        .expect("failed expression source query");
                    let text = match site.presence() {
                        HirSourcePresence::Present(HirSourceSite::Span(span)) => {
                            &source[span.range().as_range()]
                        }
                        _ => "<no authored span>",
                    };
                    format!("source={text:?}, expression={:?}", expression.kind())
                }
                _ => String::new(),
            };
            panic!("{callback}: contextual mismatch must retain final call rejection: {error:?}; {unavailable}");
        });
        let rejected = analysis
            .calls()
            .filter_map(|(_, call)| {
                matches!(call.outcome(), CallAnalysisOutcome::Rejected(_)).then_some(call)
            })
            .collect::<Vec<_>>();
        let [call] = rejected.as_slice() else {
            panic!("the exact enclosing Map call must retain one rejection: {rejected:?}");
        };
        assert!(call.selected_application().is_none());
        super::callable_values::assert_unselected_call_has_no_execution(
            &analysis,
            call.outcome().site().expression(),
        );
        let [diagnostic] = call.diagnostics() else {
            panic!("the rejected Map retains its single source-backed diagnostic");
        };
        assert_eq!(diagnostic.code(), CallableDiagnosticCode::NoViableSignature);
        let span = diagnostic
            .span()
            .expect("the rejected Map has its authored source span");
        assert_eq!(&source[span.range().as_range()], rejected_call);
    }
}
