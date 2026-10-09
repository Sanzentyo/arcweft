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

#[test]
fn collection_sum_rejects_noninteger_elements() {
    let source = "fn rejected(values: Vec<bool>) -> i64 effects {} { values.sum() }\n";
    let fixture = fixture(source, None);
    let error =
        analyze(&fixture).expect_err("noninteger Sum has no admitted analysis or execution");
    let FinalSemanticAnalysisError::UnknownCallTarget {
        owner,
        kind,
        name,
        call_source,
    } = error
    else {
        panic!("Sum's integer applicability must reject its exact authored method: {error:?}");
    };
    assert_eq!(kind, UnknownCallKind::Method);
    assert_eq!(name, "sum");
    assert_eq!(&source[call_source.range().as_range()], "values.sum()");
    let view = fixture
        .project
        .analysis_view()
        .expect("executable source HIR");
    let module = view
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let HirExprKind::Call(call) = module
        .resolve_expr(owner)
        .expect("rejected method owner")
        .kind()
    else {
        panic!("the rejection belongs to the authored Call");
    };
    assert!(call.arguments().is_empty());
    let arcweft_lang_hir::expr::HirCallCallee::UnresolvedDot {
        value_receiver,
        member,
        ..
    } = call.callee()
    else {
        panic!("Sum rejects the selected collection method source");
    };
    assert_eq!(member.resolved().expect("method name").as_str(), "sum");
    let HirExprKind::Path(receiver) = module
        .resolve_expr(*value_receiver)
        .expect("receiver source")
        .kind()
    else {
        panic!("the rejected receiver is the typed values parameter");
    };
    assert_eq!(
        receiver
            .as_resolved()
            .expect("receiver path")
            .lexical_name(),
        Some("values")
    );
    let declaration = fixture
        .symbols
        .callable_symbols()
        .next()
        .expect("one source declaration")
        .declaration();
    let signature = fixture
        .registered
        .environment()
        .callable_catalog()
        .project_record(declaration)
        .expect("registered source signature");
    assert_eq!(
        signature
            .schema()
            .parameter_type(crate::callable::CallableParameterCoordinate::new(
                CallableGroupIndex::ZERO,
                CallableParameterIndex::try_from_usize(0).unwrap(),
            )),
        Some(&TypeKind::Vec(Box::new(TypeKind::Bool)))
    );
}

#[test]
fn contextual_map_returns_infer_through_scopes_and_all_branch_exits() {
    for callback in [
        "|item| { scope forwarding { return score(item) } }",
        "|item| { if item > 0i64 { return score(item) } else { return score(item) } }",
        "|item| { scope forwarding { return score(item) }\n0i64 }",
        "|item| { let nested = || { return \"inner\" }\nscope forwarding { return score(item) } }",
        "|item| { scope forwarding { let value = loop { break score(item) }\nvalue } }",
    ] {
        let source = format!(
            "fn score(value: i64) -> i64 effects {{}} {{ value * 4i64 }}\nfn scoped(values: Vec<i64>) -> Vec<i64> effects {{}} {{ values.map({callback}) }}\n"
        );
        let world = fixture(&source, None);
        let report = analyze(&world).unwrap_or_else(|error| panic!("{callback}: {error:?}"));
        assert!(
            report
                .calls()
                .all(|(_, call)| call.selected_application().is_some())
        );
        let expected = TypeKind::Vec(Box::new(TypeKind::I64));
        assert!(report.calls().any(|(_, call)| {
            call.selected_application()
                .is_some_and(|application| application.result().value_type() == Some(&expected))
        }));
        let callbacks = report.expressions().filter_map(|(_, expression)| {
            matches!(expression.resolution(), CheckedExpressionResolution::Closure(_)).then_some(expression)
        }).filter(|expression| matches!(expression.value_type(), Some(TypeKind::Function { params, .. }) if params.len() == 1 && params.first() == Some(&TypeKind::I64))).collect::<Vec<_>>();
        let [callback] = callbacks.as_slice() else {
            panic!("one accepted item callback: {callbacks:?}")
        };
        let Some(TypeKind::Function { return_type, .. }) = callback.value_type() else {
            unreachable!()
        };
        assert_eq!(return_type.as_ref(), &TypeKind::I64);
    }
}

#[test]
fn inferred_callable_return_keeps_the_nearest_frame_and_never_prefix() {
    let world = fixture(
        "fn holder() -> Unit effects {} {\nlet callback = |value: i64| {\nlet inner = || { return \"inner\" }\nscope outer { return value }\ntrue\n}\n}\n",
        None,
    );
    let report =
        analyze(&world).expect("scopes and dead tails cannot change the receiving callable result");
    let mut results = report
        .expressions()
        .filter_map(|(_, expression)| {
            if !matches!(
                expression.resolution(),
                CheckedExpressionResolution::Closure(_)
            ) {
                return None;
            }
            let Some(TypeKind::Function {
                params,
                return_type,
                ..
            }) = expression.value_type()
            else {
                panic!("checked closure type")
            };
            Some((params.len(), return_type.as_ref().clone()))
        })
        .collect::<Vec<_>>();
    results.sort_by_key(|(arity, _)| *arity);
    assert_eq!(results, [(0, TypeKind::String), (1, TypeKind::I64)]);
    let targets = report
        .statements()
        .filter_map(|(_, statement)| {
            let CheckedStatementPayload::ControlTransfer(target) = statement.payload() else {
                return None;
            };
            target.return_target()
        })
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 2);
    assert_ne!(targets[0], targets[1]);
}

#[test]
fn inferred_unit_return_keeps_unit_through_a_named_scope() {
    let world = fixture(
        "fn holder() -> Unit effects {} { let callback = || { scope early { return () } } }\n",
        None,
    );
    let report =
        analyze(&world).expect("the exact Unit Return statement supplies Unit to its callable");
    let callbacks = report
        .expressions()
        .filter_map(|(_, expression)| {
            if matches!(
                expression.resolution(),
                CheckedExpressionResolution::Closure(_)
            ) {
                Some(expression)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let [callback] = callbacks.as_slice() else {
        panic!("one source closure")
    };
    let Some(TypeKind::Function {
        params,
        return_type,
        ..
    }) = callback.value_type()
    else {
        panic!("checked closure result")
    };
    assert!(params.is_empty());
    assert_eq!(return_type.as_ref(), &TypeKind::Unit);
}

#[test]
fn contextual_map_return_does_not_hide_a_unit_fallthrough() {
    let callback = "|item| { if item > 0i64 { return score(item) } }";
    let call_text = format!("values.map({callback})");
    let source = format!(
        "fn score(value: i64) -> i64 effects {{}} {{ value }}\nfn rejected(values: Vec<i64>) -> Vec<i64> effects {{}} {{ {call_text} }}\n"
    );
    let world = fixture(&source, None);
    let report = analyze(&world).expect("a rejected callback retains its precise call diagnostic");
    let rejected = report
        .calls()
        .filter_map(|(_, call)| {
            matches!(call.outcome(), CallAnalysisOutcome::Rejected(_)).then_some(call)
        })
        .collect::<Vec<_>>();
    let [call] = rejected.as_slice() else {
        panic!("only the authored Map is rejected: {rejected:?}")
    };
    assert!(call.selected_application().is_none());
    let [diagnostic] = call.diagnostics() else {
        panic!("one authored rejection")
    };
    assert_eq!(diagnostic.code(), CallableDiagnosticCode::NoViableSignature);
    assert_eq!(
        &source[diagnostic.span().unwrap().range().as_range()],
        call_text
    );
    super::callable_values::assert_unselected_call_has_no_execution(
        &report,
        call.outcome().site().expression(),
    );
}

#[test]
fn scoped_closure_return_keeps_the_exact_declared_result_rejection() {
    let source = "fn holder() -> Unit effects {} { let callback = |value: i64| -> String { scope forwarding { return value } } }\n";
    let world = fixture(source, None);
    let error = analyze(&world)
        .expect_err("a named scope cannot change the receiving String result contract");
    let FinalSemanticAnalysisError::ReturnValueTypeMismatch {
        expected, actual, ..
    } = error
    else {
        panic!("exact scoped Return mismatch: {error:?}")
    };
    assert_eq!(expected.as_ref(), &TypeKind::String);
    assert_eq!(actual.as_ref(), &TypeKind::I64);
}

#[test]
fn scoped_out_completes_its_dialogue_application_then_resumes_the_callable() {
    let world = fixture(
        "pub character alice { display = \"Alice\" }\nflow row() -> i64 {\n    alice()[本文。[p]] with {\n        init { scope staging { out () } }\n    }\n    return 7i64\n}\n",
        None,
    );
    let report =
        analyze(&world).expect("the exact Output receiver resumes its outside continuation");
    let outputs = report
        .statements()
        .filter_map(|(_, statement)| {
            let CheckedStatementPayload::ControlTransfer(target) = statement.payload() else {
                return None;
            };
            target.output().map(|output| output.application())
        })
        .collect::<Vec<_>>();
    let [application] = outputs.as_slice() else {
        panic!("one admitted scoped Output")
    };
    let checked = report
        .expression(*application)
        .expect("the exact receiving application remains checked");
    assert!(matches!(
        checked.resolution(),
        CheckedExpressionResolution::DialogueApplication { .. }
    ));
    assert!(
        checked
            .value_type()
            .is_some_and(|ty| ty != &TypeKind::Never)
    );
    let returned = report
        .statements()
        .filter_map(|(_, statement)| {
            let CheckedStatementPayload::ControlTransfer(target) = statement.payload() else {
                return None;
            };
            target.return_target()
        })
        .collect::<Vec<_>>();
    assert_eq!(returned.len(), 1);
    assert!(
        report
            .expressions()
            .any(|(_, expression)| expression.value_type() == Some(&TypeKind::I64))
    );
}

#[test]
fn named_scope_cannot_manufacture_an_output_receiving_boundary() {
    let source = "fn rejected() -> Unit { scope staging { out () } }\n";
    let world = fixture(source, None);
    let error = analyze(&world).expect_err("Out requires its actual attached line-plan receiver");
    let FinalSemanticAnalysisError::ControlTransfer(
        arcweft_lang_hir::project::HirControlTransferResolutionError::UnresolvedTarget {
            statement,
            kind: arcweft_lang_hir::project::HirControlTransferKind::Out,
        },
    ) = error
    else {
        panic!("exact unowned Output rejection: {error:?}")
    };
    let project = world.project.analysis_view().expect("clean authored HIR");
    let module = project.module(&CanonicalModulePath::crate_root()).unwrap();
    assert!(matches!(
        module.resolve_stmt(statement).unwrap().kind(),
        arcweft_lang_hir::stmt::HirStmtKind::Out { .. }
    ));
}

#[test]
fn empty_never_match_preserves_scoped_result_and_let_else_divergence() {
    let world = fixture(
        "fn exhausted(bottom: Never) -> i64 { scope exhausted { (match bottom {}) } }\nfn inspect(input: Option<i64>, bottom: Never) -> i64 {\n    let Some(value) = input else { (match bottom {}) }\n    value\n}\n",
        None,
    );
    let report = analyze(&world).expect("the uninhabited Match cannot create normal continuation");
    let project = world.project.analysis_view().unwrap();
    let module = project.module(&CanonicalModulePath::crate_root()).unwrap();
    let matches = module
        .expressions()
        .filter_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .collect::<Vec<_>>();
    assert!(!matches.is_empty());
    for owner in matches {
        assert_eq!(
            report.expression(owner).unwrap().value_type(),
            Some(&TypeKind::Never)
        );
        let checked = report
            .checked_match_with_control(
                project,
                &world.symbols,
                owner,
                crate::final_analysis::CheckedMatchLimits::PRODUCTION,
                crate::final_analysis::FinalSemanticAnalysisControl::new(
                    &std::sync::atomic::AtomicBool::new(false),
                ),
            )
            .expect("empty Never coverage is exhaustive");
        assert!(checked.coverage().exhaustive());
    }
}

#[test]
fn empty_match_cannot_hide_an_inhabited_domain_or_unit_failure_branch() {
    let world = fixture(
        "fn rejected(flag: bool) -> i64 { scope empty { match flag {} } }\n",
        None,
    );
    let report =
        analyze(&world).expect("source query retains the authored empty Match for coverage");
    let project = world.project.analysis_view().unwrap();
    let module = project.module(&CanonicalModulePath::crate_root()).unwrap();
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .unwrap();
    assert!(matches!(
        report.checked_match_with_control(
            project,
            &world.symbols,
            owner,
            crate::final_analysis::CheckedMatchLimits::PRODUCTION,
            crate::final_analysis::FinalSemanticAnalysisControl::new(
                &std::sync::atomic::AtomicBool::new(false)
            )
        ),
        Err(
            crate::final_analysis::semantic_transcript::SemanticTranscriptError::NonExhaustive {
                witness: crate::final_analysis::match_coverage::CheckedCoverageWitness::Bool(_)
            }
        ),
    ));
    let failed = fixture(
        "fn inspect(input: Option<i64>, flag: bool) -> i64 {\n    let Some(value) = input else { match flag { true => ()\nfalse => () } }\n    value\n}\n",
        None,
    );
    assert!(matches!(
        analyze(&failed),
        Err(FinalSemanticAnalysisError::LocalUse(
            crate::final_analysis::CheckedLocalUseError::LetElseContinues { .. },
        ))
    ));
}
