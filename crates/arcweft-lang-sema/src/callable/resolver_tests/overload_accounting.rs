//! Candidate-contextual physical work and retained semantic-fact matrix.

use std::sync::Arc;

use arcweft_source::SourceSpan;

use crate::{
    callable::{
        CallTargetFacts, EnvironmentCallablePublication, RegisteredCallableCatalogBuilder,
        ResolvedCallable,
    },
    checker::{
        TypeCheckReport, TypeExpressionId, TypeJudgmentRule, TypeJudgmentSubject,
        TypedLoweringEvidenceKind,
    },
    registration::EnvironmentManifestDigest,
    types::{DetachedTypeOwnerId, GenericTypeOwnerId, GenericTypeParameterId, TypeKind},
};

use super::*;

#[test]
fn contextual_enum_shorthand_records_each_candidate_expected_type() {
    const SOURCE: &str = r"
flow @flow.main main {
    let value: String = enum_choice(.Ready)
}
";
    let (base, first_mood, second_mood) = contextual_enum_environment();
    let fixture = accounting_fixture_with_environment(
        "overload-accounting-enum",
        SOURCE,
        vec![
            record(
                "enum_choice",
                0,
                0,
                ordinary_single_parameter_schema("value", first_mood.clone(), TypeKind::String),
            ),
            record(
                "enum_choice",
                1,
                1,
                ordinary_single_parameter_schema("value", second_mood.clone(), TypeKind::String),
            ),
        ],
        base,
    );

    let report = analyze_registered_project_types(&fixture.project.linked_module(), &fixture.world);
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("ambiguous between equally specific signatures")
    }));
    let call = exact_span(&fixture.document, "enum_choice(.Ready)");
    let target = fact_for_span(&report, &call);
    assert!(matches!(target.target(), CallTargetFact::Ambiguous { .. }));

    let evaluations = report.physical_candidate_argument_evaluations();
    assert_eq!(evaluations.len(), 2);
    assert_eq!(
        evaluations
            .iter()
            .map(|evaluation| (evaluation.pass, evaluation.expected.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(first_mood.clone()),
            ),
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(second_mood),
            ),
        ]
    );
    assert!(evaluations.iter().all(|evaluation| {
        evaluation.call_expression == target.expression()
            && evaluation.kind == PhysicalArgumentEvaluationKind::Authored
    }));
    let retained = report
        .retained_argument_inference_facts()
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].2.expected(), Some(&first_mood));
    assert_eq!(retained[0].2.inferred(), Some(&first_mood));
}

#[test]
fn unsuffixed_numeric_probe_fallback_rolls_back_before_exact_winner_replay() {
    const SOURCE: &str = r"
flow @flow.main main {
    let value: String = numeric_choice(7)
}
";
    let fixture = accounting_fixture(
        "overload-accounting-numeric",
        SOURCE,
        vec![
            record(
                "numeric_choice",
                0,
                0,
                unchecked_single_parameter_schema("value", TypeKind::String),
            ),
            record(
                "numeric_choice",
                1,
                1,
                ordinary_single_parameter_schema("value", TypeKind::I64, TypeKind::String),
            ),
        ],
    );

    let report = analyze_registered_project_types(&fixture.project.linked_module(), &fixture.world);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let target = fact_for_span(&report, &exact_span(&fixture.document, "numeric_choice(7)"));
    let CallTargetFact::Selected { selected, .. } = target.target() else {
        panic!("exact numeric overload must win")
    };
    assert!(matches!(
        selected.id(),
        CallableCandidateId::Environment(id) if id.overload().get() == 1
    ));
    assert_eq!(
        report
            .physical_candidate_argument_evaluations()
            .iter()
            .map(|evaluation| (evaluation.pass, evaluation.expected.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Unchecked,
            ),
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(TypeKind::I64),
            ),
            (
                CandidateEvaluationPass::SelectedReplay,
                CandidateExpectedType::Exact(TypeKind::I64),
            ),
        ]
    );
    assert!(
        report.numeric_fallbacks.is_empty(),
        "the unchecked probe fallback must roll back: {:?}",
        report.numeric_fallbacks
    );
    let numeric_targets = report
        .typed_lowering_evidence
        .iter()
        .filter_map(|evidence| match &evidence.kind {
            TypedLoweringEvidenceKind::ResolvedNumericType { target } => Some(target),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(numeric_targets, vec![&TypeKind::I64]);
    let retained = report
        .retained_argument_inference_facts()
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].2.inferred(), Some(&TypeKind::I64));
    assert_eq!(retained[0].2.expected(), Some(&TypeKind::I64));
    assert_single_expected_judgment(
        &report,
        retained[0].2.expression(),
        Some(&TypeKind::I64),
        &TypeKind::I64,
    );
}

#[test]
fn fixed_literal_spread_records_three_passes_per_logical_slot() {
    const SOURCE: &str = r"
flow @flow.main main {
    let value: String = fixed_choice([1i32, 2i32]...)
}
";
    let fixture = accounting_fixture(
        "overload-accounting-fixed-spread",
        SOURCE,
        vec![
            record(
                "fixed_choice",
                0,
                0,
                rest_spread_schema(TypeKind::I32, SpreadArgumentPolicy::FixedLiteralOnly),
            ),
            record(
                "fixed_choice",
                1,
                1,
                rest_spread_schema(TypeKind::String, SpreadArgumentPolicy::FixedLiteralOnly),
            ),
        ],
    );

    let report = analyze_registered_project_types(&fixture.project.linked_module(), &fixture.world);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let target = fact_for_span(
        &report,
        &exact_span(&fixture.document, "fixed_choice([1i32, 2i32]...)"),
    );
    assert!(matches!(target.target(), CallTargetFact::Selected { .. }));
    let evaluations = report.physical_candidate_argument_evaluations();
    assert_eq!(evaluations.len(), 6);
    assert_eq!(
        evaluations
            .iter()
            .map(|evaluation| (evaluation.pass, evaluation.expected.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(TypeKind::I32),
            ),
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(TypeKind::I32),
            ),
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(TypeKind::String),
            ),
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(TypeKind::String),
            ),
            (
                CandidateEvaluationPass::SelectedReplay,
                CandidateExpectedType::Exact(TypeKind::I32),
            ),
            (
                CandidateEvaluationPass::SelectedReplay,
                CandidateExpectedType::Exact(TypeKind::I32),
            ),
        ]
    );
    for chunk in evaluations.chunks_exact(2) {
        assert_eq!(chunk[0].argument.get(), 0);
        assert_eq!(chunk[1].argument.get(), 0);
        assert_eq!(chunk[0].slot.get(), 0);
        assert_eq!(chunk[1].slot.get(), 1);
        assert!(chunk.iter().all(|evaluation| {
            evaluation.call_expression == target.expression()
                && evaluation.kind == PhysicalArgumentEvaluationKind::FixedLiteralSpread
        }));
    }
    let retained = report
        .retained_argument_inference_facts()
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), 2);
    assert_eq!(
        retained
            .iter()
            .map(|(_, argument, slot)| (argument.get(), slot.slot().get()))
            .collect::<Vec<_>>(),
        vec![(0, 0), (0, 1)]
    );
}

#[test]
fn typed_rest_spread_counts_the_container_once_for_each_candidate_pass() {
    const SOURCE: &str = r"
flow @flow.main main {
    let values: Vec<i32> = [1i32, 2i32]
    let value: String = typed_rest_choice(values...)
}
";
    let fixture = accounting_fixture(
        "overload-accounting-typed-rest",
        SOURCE,
        vec![
            record(
                "typed_rest_choice",
                0,
                0,
                rest_spread_schema(TypeKind::I32, SpreadArgumentPolicy::TypedRest),
            ),
            record(
                "typed_rest_choice",
                1,
                1,
                rest_spread_schema(TypeKind::String, SpreadArgumentPolicy::TypedRest),
            ),
        ],
    );

    let report = analyze_registered_project_types(&fixture.project.linked_module(), &fixture.world);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let target = fact_for_span(
        &report,
        &exact_span(&fixture.document, "typed_rest_choice(values...)"),
    );
    assert!(matches!(target.target(), CallTargetFact::Selected { .. }));
    let evaluations = report.physical_candidate_argument_evaluations();
    assert_eq!(evaluations.len(), 3);
    assert_eq!(
        evaluations
            .iter()
            .map(|evaluation| evaluation.pass)
            .collect::<Vec<_>>(),
        vec![
            CandidateEvaluationPass::Probe,
            CandidateEvaluationPass::Probe,
            CandidateEvaluationPass::SelectedReplay,
        ]
    );
    assert!(evaluations.iter().all(|evaluation| {
        evaluation.call_expression == target.expression()
            && evaluation.argument.get() == 0
            && evaluation.slot.get() == 0
            && evaluation.kind == PhysicalArgumentEvaluationKind::TypedRestSpread
            && evaluation.expected == CandidateExpectedType::Unchecked
    }));
    let retained = report
        .retained_argument_inference_facts()
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), 1);
    assert_eq!(
        retained[0].2.inferred(),
        Some(&TypeKind::Vec(Box::new(TypeKind::I32)))
    );
}

#[test]
fn unchecked_winner_replays_without_retaining_rejected_shape_diagnostics() {
    const SOURCE: &str = r"
flow @flow.main main {
    let value: String = clean_choice(1i32)
}
";
    let fixture = accounting_fixture(
        "overload-accounting-unchecked",
        SOURCE,
        vec![
            record(
                "clean_choice",
                0,
                0,
                ordinary_single_parameter_schema("value", TypeKind::String, TypeKind::String),
            ),
            record(
                "clean_choice",
                1,
                1,
                unchecked_single_parameter_schema("value", TypeKind::String),
            ),
        ],
    );

    let report = analyze_registered_project_types(&fixture.project.linked_module(), &fixture.world);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let target = fact_for_span(
        &report,
        &exact_span(&fixture.document, "clean_choice(1i32)"),
    );
    assert!(matches!(target.target(), CallTargetFact::Selected { .. }));
    assert_eq!(
        report
            .physical_candidate_argument_evaluations()
            .iter()
            .map(|evaluation| (evaluation.pass, evaluation.expected.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(TypeKind::String),
            ),
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Unchecked,
            ),
            (
                CandidateEvaluationPass::SelectedReplay,
                CandidateExpectedType::Unchecked,
            ),
        ]
    );
    let retained = report
        .retained_argument_inference_facts()
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].2.inferred(), Some(&TypeKind::I32));
    assert_eq!(retained[0].2.expected(), None);
    assert_eq!(retained[0].2.poison(), CallPoison::Clean);
}

#[test]
fn zero_argument_candidate_materialization_and_comparison_do_not_create_events() {
    const SOURCE: &str = r"
flow @flow.main main {
    let value = zero_choice()
}
";
    let fixture = accounting_fixture(
        "overload-accounting-zero-argument",
        SOURCE,
        vec![
            record(
                "zero_choice",
                0,
                0,
                schema(Vec::new(), SpreadArgumentPolicy::Reject, TypeKind::I32),
            ),
            record(
                "zero_choice",
                1,
                1,
                schema(Vec::new(), SpreadArgumentPolicy::Reject, TypeKind::String),
            ),
        ],
    );

    let report = analyze_registered_project_types(&fixture.project.linked_module(), &fixture.world);
    let target = fact_for_span(&report, &exact_span(&fixture.document, "zero_choice()"));
    let CallTargetFact::Ambiguous { candidates } = target.target() else {
        panic!("zero-argument overloads must reach comparison without argument work")
    };
    assert_eq!(candidates.len(), 2);
    assert!(report.physical_candidate_argument_evaluations().is_empty());
    assert_eq!(report.retained_argument_inference_facts().count(), 0);
}

#[test]
fn nested_overloads_keep_call_candidate_and_pass_contexts_disjoint() {
    let fixture = nested_overload_fixture();
    let report = analyze_registered_project_types(&fixture.project.linked_module(), &fixture.world);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);

    let outer = fact_for_span(
        &report,
        &exact_span(&fixture.document, "outer_choice(inner_choice(1i32))"),
    );
    let inner = fact_for_span(
        &report,
        &exact_span(&fixture.document, "inner_choice(1i32)"),
    );
    let CallTargetFact::Selected {
        selected: outer_selected,
        considered: outer_considered,
    } = outer.target()
    else {
        panic!("exact outer candidate must win")
    };
    let CallTargetFact::Selected {
        selected: inner_selected,
        considered: inner_considered,
    } = inner.target()
    else {
        panic!("exact inner candidate must win")
    };
    assert_eq!(outer_considered.len(), 2);
    assert_eq!(inner_considered.len(), 2);
    assert_ne!(outer.expression(), inner.expression());
    assert_outer_nested_events(&report, outer, outer_selected, outer_considered);
    assert_inner_nested_events(&report, inner, inner_selected, inner_considered);
    assert_nested_event_ownership(&report, outer, outer_considered, inner, inner_considered);
    assert_eq!(report.retained_call_target_facts().count(), 2);
    assert_eq!(report.retained_argument_inference_facts().count(), 2);
}

fn assert_outer_nested_events(
    report: &TypeCheckReport,
    outer: &CallTargetFacts,
    selected: &ResolvedCallable,
    considered: &[ResolvedCallable],
) {
    let events = report
        .physical_candidate_argument_evaluations()
        .iter()
        .filter(|evaluation| evaluation.call_expression == outer.expression())
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 3);
    assert_eq!(
        events
            .iter()
            .map(|evaluation| (evaluation.pass, evaluation.expected.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Unchecked,
            ),
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(TypeKind::I32),
            ),
            (
                CandidateEvaluationPass::SelectedReplay,
                CandidateExpectedType::Exact(TypeKind::I32),
            ),
        ]
    );
    assert_eq!(&events[0].candidate, considered[0].id());
    assert_eq!(&events[1].candidate, selected.id());
    assert_eq!(&events[2].candidate, selected.id());
}

fn assert_inner_nested_events(
    report: &TypeCheckReport,
    inner: &CallTargetFacts,
    selected: &ResolvedCallable,
    considered: &[ResolvedCallable],
) {
    let events = report
        .physical_candidate_argument_evaluations()
        .iter()
        .filter(|evaluation| evaluation.call_expression == inner.expression())
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 9);
    for invocation in events.chunks_exact(3) {
        assert_eq!(
            invocation
                .iter()
                .map(|evaluation| evaluation.pass)
                .collect::<Vec<_>>(),
            vec![
                CandidateEvaluationPass::Probe,
                CandidateEvaluationPass::Probe,
                CandidateEvaluationPass::SelectedReplay,
            ]
        );
        assert_eq!(&invocation[0].candidate, selected.id());
        assert_eq!(&invocation[1].candidate, considered[1].id());
        assert_eq!(&invocation[2].candidate, selected.id());
        assert_eq!(
            invocation
                .iter()
                .map(|evaluation| evaluation.expected.clone())
                .collect::<Vec<_>>(),
            vec![
                CandidateExpectedType::Exact(TypeKind::I32),
                CandidateExpectedType::Exact(TypeKind::String),
                CandidateExpectedType::Exact(TypeKind::I32),
            ]
        );
    }
}

fn assert_nested_event_ownership(
    report: &TypeCheckReport,
    outer: &CallTargetFacts,
    outer_considered: &[ResolvedCallable],
    inner: &CallTargetFacts,
    inner_considered: &[ResolvedCallable],
) {
    assert!(
        report
            .physical_candidate_argument_evaluations()
            .iter()
            .all(|evaluation| {
                (evaluation.call_expression == outer.expression()
                    && outer_considered
                        .iter()
                        .any(|candidate| candidate.id() == &evaluation.candidate))
                    || (evaluation.call_expression == inner.expression()
                        && inner_considered
                            .iter()
                            .any(|candidate| candidate.id() == &evaluation.candidate))
            })
    );
}

#[test]
fn repeated_nested_overload_builds_have_identical_ordered_evidence() {
    let fixture = nested_overload_fixture();
    let linked = fixture.project.linked_module();
    let first = analyze_registered_project_types(&linked, &fixture.world);
    let second = analyze_registered_project_types(&linked, &fixture.world);

    assert_eq!(
        first.physical_candidate_argument_evaluations(),
        second.physical_candidate_argument_evaluations()
    );
    let retained = |report: &TypeCheckReport| {
        report
            .retained_argument_inference_facts()
            .map(|(expression, argument, slot)| (expression, argument, slot.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(retained(&first), retained(&second));
    assert!(!first.physical_candidate_argument_evaluations_overflowed());
    assert!(!second.physical_candidate_argument_evaluations_overflowed());
}

#[test]
fn generic_candidate_substitutions_are_probe_local_and_winner_owned() {
    const SOURCE: &str = r"
flow @flow.main main {
    let value: i32 = generic_choice(1i32, 2i32)
}
";
    let first_parameter =
        GenericTypeParameterId::new(GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(1)), 0);
    let second_parameter =
        GenericTypeParameterId::new(GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(2)), 0);
    let first_generic = TypeKind::GenericParam(first_parameter);
    let second_generic = TypeKind::GenericParam(second_parameter);
    let fixture = direct_projected_fixture(
        "overload-accounting-generic",
        SOURCE,
        vec![
            record(
                "generic_choice",
                0,
                0,
                two_parameter_schema(
                    first_generic.clone(),
                    first_generic.clone(),
                    first_generic.clone(),
                ),
            ),
            record(
                "generic_choice",
                1,
                1,
                two_parameter_schema(
                    second_generic.clone(),
                    TypeKind::Vec(Box::new(second_generic.clone())),
                    second_generic.clone(),
                ),
            ),
        ],
    );

    let report = analyze_registered_project_types(&fixture.project.linked_module(), &fixture.world);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let target = fact_for_span(
        &report,
        &exact_span(&fixture.document, "generic_choice(1i32, 2i32)"),
    );
    assert!(matches!(target.target(), CallTargetFact::Selected { .. }));
    assert_eq!(target.result(), Some(&TypeKind::I32));
    assert_eq!(
        report
            .physical_candidate_argument_evaluations()
            .iter()
            .map(|evaluation| (evaluation.pass, evaluation.expected.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(first_generic.clone()),
            ),
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(TypeKind::I32),
            ),
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(second_generic),
            ),
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(TypeKind::Vec(Box::new(TypeKind::I32))),
            ),
            (
                CandidateEvaluationPass::SelectedReplay,
                CandidateExpectedType::Exact(first_generic.clone()),
            ),
            (
                CandidateEvaluationPass::SelectedReplay,
                CandidateExpectedType::Exact(TypeKind::I32),
            ),
        ]
    );
    let retained = report
        .retained_argument_inference_facts()
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), 2);
    assert_eq!(retained[0].2.inferred(), Some(&TypeKind::I32));
    assert_eq!(retained[0].2.expected(), Some(&first_generic));
    assert_eq!(retained[1].2.inferred(), Some(&TypeKind::I32));
    assert_eq!(retained[1].2.expected(), Some(&TypeKind::I32));
}

#[test]
fn closure_candidate_probes_roll_back_capture_diagnostics_and_lowering_evidence() {
    const SOURCE: &str = r"
flow @flow.main main {
    let captured = 1i32
    let value: String = closure_choice(|item: i32| -> i32 { captured + item })
}
";
    let selected_function = TypeKind::function([TypeKind::I32], TypeKind::I32);
    let rejected_function = TypeKind::function([TypeKind::String], TypeKind::String);
    let fixture = direct_projected_fixture(
        "overload-accounting-closure",
        SOURCE,
        vec![
            record(
                "closure_choice",
                0,
                0,
                ordinary_single_parameter_schema(
                    "callback",
                    selected_function.clone(),
                    TypeKind::String,
                ),
            ),
            record(
                "closure_choice",
                1,
                1,
                ordinary_single_parameter_schema(
                    "callback",
                    rejected_function.clone(),
                    TypeKind::String,
                ),
            ),
        ],
    );

    let report = analyze_registered_project_types(&fixture.project.linked_module(), &fixture.world);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    assert_eq!(report.closure_captures.len(), 1);
    assert_eq!(
        report.closure_captures[0]
            .captures
            .iter()
            .map(|capture| capture.name.as_str())
            .collect::<Vec<_>>(),
        vec!["captured"]
    );
    assert_eq!(
        report
            .physical_candidate_argument_evaluations()
            .iter()
            .map(|evaluation| (evaluation.pass, evaluation.expected.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(selected_function.clone()),
            ),
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(rejected_function.clone()),
            ),
            (
                CandidateEvaluationPass::SelectedReplay,
                CandidateExpectedType::Exact(selected_function.clone()),
            ),
        ]
    );
    let expected_function_evidence = report
        .typed_lowering_evidence
        .iter()
        .filter(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::ExpectedFunctionValue { expected_ty, .. }
                    if expected_ty == &selected_function
            )
        })
        .count();
    assert_eq!(expected_function_evidence, 1);
    let retained = report
        .retained_argument_inference_facts()
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].2.expected(), Some(&selected_function));
    assert_eq!(retained[0].2.poison(), CallPoison::Clean);
    assert_single_expected_judgment(
        &report,
        retained[0].2.expression(),
        None,
        &selected_function,
    );
    assert!(
        report
            .judgments
            .iter()
            .all(|judgment| judgment.expected_type() != Some(&rejected_function))
    );
}

#[test]
fn partial_placeholder_facts_are_candidate_local_and_selected_once() {
    const SOURCE: &str = r"
flow @flow.main main {
    let value: String = partial_choice(_ > 80i32)
}
";
    let selected_function = TypeKind::function([TypeKind::I32], TypeKind::Bool);
    let fixture = direct_projected_fixture(
        "overload-accounting-partial",
        SOURCE,
        vec![
            record(
                "partial_choice",
                0,
                0,
                ordinary_single_parameter_schema(
                    "callback",
                    selected_function.clone(),
                    TypeKind::String,
                ),
            ),
            record(
                "partial_choice",
                1,
                1,
                ordinary_single_parameter_schema("callback", TypeKind::I32, TypeKind::String),
            ),
        ],
    );

    let report = analyze_registered_project_types(&fixture.project.linked_module(), &fixture.world);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let target = fact_for_span(
        &report,
        &exact_span(&fixture.document, "partial_choice(_ > 80i32)"),
    );
    assert!(matches!(target.target(), CallTargetFact::Selected { .. }));
    assert_eq!(target.current_group(), CallableGroupIndex::ZERO);
    assert_eq!(target.next_group(), None);
    assert_eq!(target.function_value_type(), None);
    assert_eq!(
        report
            .physical_candidate_argument_evaluations()
            .iter()
            .filter(|evaluation| evaluation.call_expression == target.expression())
            .map(|evaluation| (evaluation.pass, evaluation.expected.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(selected_function.clone()),
            ),
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(TypeKind::I32),
            ),
            (
                CandidateEvaluationPass::SelectedReplay,
                CandidateExpectedType::Exact(selected_function.clone()),
            ),
        ]
    );
    let partial_evidence = report
        .typed_lowering_evidence
        .iter()
        .filter(|evidence| {
            matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::ExpectedFunctionValue {
                    expected_ty,
                    actual_ty,
                    arity: 1,
                } if expected_ty == &selected_function && actual_ty == &selected_function
            )
        })
        .count();
    assert_eq!(partial_evidence, 1);
    let retained = report
        .retained_argument_inference_facts()
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].2.inferred(), Some(&selected_function));
    assert_eq!(retained[0].2.expected(), Some(&selected_function));
}

fn nested_overload_fixture() -> ResolverFixture {
    const SOURCE: &str = r"
flow @flow.main main {
    let value: String = outer_choice(inner_choice(1i32))
}
";
    accounting_fixture(
        "overload-accounting-nested",
        SOURCE,
        vec![
            record(
                "inner_choice",
                0,
                0,
                ordinary_single_parameter_schema("value", TypeKind::I32, TypeKind::I32),
            ),
            record(
                "inner_choice",
                1,
                1,
                ordinary_single_parameter_schema("value", TypeKind::String, TypeKind::I32),
            ),
            record(
                "outer_choice",
                0,
                2,
                unchecked_single_parameter_schema("value", TypeKind::String),
            ),
            record(
                "outer_choice",
                1,
                3,
                ordinary_single_parameter_schema("value", TypeKind::I32, TypeKind::String),
            ),
        ],
    )
}

fn direct_projected_fixture(
    profile: &str,
    source: &str,
    records: Vec<EnvironmentCallablePublicationRecord>,
) -> ResolverFixture {
    direct_projected_fixture_with_environment(profile, source, records, TypeCheckEnv::standard())
}

fn direct_projected_fixture_with_environment(
    profile: &str,
    source: &str,
    records: Vec<EnvironmentCallablePublicationRecord>,
    base: TypeCheckEnv,
) -> ResolverFixture {
    let (document, project, symbol_world) = root_project_source(profile, source);
    let facts = one_character_facts(&document, symbol_world, &sample_manifest("layers/body.png"));
    let accepted = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(base),
        &project,
        &facts,
        None,
    ))
    .expect("direct projected accounting base world");
    let environment = accepted.environment();
    let mut builder = RegisteredCallableCatalogBuilder::for_nominal_world(
        environment.nominal_world(),
        PRODUCTION_CALLABLE_LIMITS,
    );
    builder
        .add_project(&project, accepted.symbols(), environment.nominal_world())
        .expect("direct projected accounting project catalog");
    let publication = EnvironmentCallablePublication::try_new_projected(
        EnvironmentCallableOwner::Adapter(
            AdapterPackageId::try_new("adapter.overload.accounting.generic")
                .expect("generic accounting adapter id"),
        ),
        environment.nominal_world().stamp(),
        EnvironmentManifestDigest::from_bytes([0xA6; 32]),
        records,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("direct projected generic publication");
    builder
        .add_environment(publication)
        .expect("direct projected generic catalog input");
    let callables = Arc::new(builder.finish().expect("direct projected generic catalog"));
    ResolverFixture {
        document,
        project,
        world: accepted.with_callable_catalog_for_test(callables),
    }
}

fn accounting_fixture(
    profile: &str,
    source: &str,
    records: Vec<EnvironmentCallablePublicationRecord>,
) -> ResolverFixture {
    accounting_fixture_with_environment(profile, source, records, TypeCheckEnv::standard())
}

fn accounting_fixture_with_environment(
    profile: &str,
    source: &str,
    records: Vec<EnvironmentCallablePublicationRecord>,
    base: TypeCheckEnv,
) -> ResolverFixture {
    let (document, project, symbol_world) = root_project_source(profile, source);
    let environment_document = source_document(
        &format!("arcweft-generated://{profile}/adapter"),
        "overload accounting adapter callables",
    );
    let owner = EnvironmentCallableOwner::Adapter(
        AdapterPackageId::try_new("adapter.overload.accounting.matrix")
            .expect("accounting adapter id"),
    );
    let environment_input = source_backed_callable_input(owner, &environment_document, records);
    let facts = one_character_facts_with_environment(
        &document,
        vec![Arc::clone(&document), environment_document],
        symbol_world,
        &sample_manifest("layers/body.png"),
        vec![environment_input],
    );
    let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(base),
        &project,
        &facts,
        None,
    ))
    .expect("overload accounting fixture registration");
    ResolverFixture {
        document,
        project,
        world,
    }
}

fn contextual_enum_environment() -> (TypeCheckEnv, TypeKind, TypeKind) {
    let first_path: arcweft_lang_syntax::types::TypePath = project_path(["FirstMood"]).into();
    let second_path: arcweft_lang_syntax::types::TypePath = project_path(["SecondMood"]).into();
    let first_id = AcceptedNominalId::new(AcceptedNominalOwnerId::Standard, first_path);
    let second_id = AcceptedNominalId::new(AcceptedNominalOwnerId::Standard, second_path);
    let first = TypeKind::AcceptedNominal(AcceptedNominalType::new(
        first_id.clone(),
        Box::<[TypeKind]>::default(),
    ));
    let second = TypeKind::AcceptedNominal(AcceptedNominalType::new(
        second_id.clone(),
        Box::<[TypeKind]>::default(),
    ));
    let base = TypeCheckEnv::standard()
        .try_with_nominal_record(
            AcceptedNominalRecord::try_new(
                first_id,
                0,
                AcceptedNominalSemantics::Opaque,
                AcceptedNominalOrigin::Test,
                None,
            )
            .expect("first contextual enum nominal"),
        )
        .expect("first contextual enum registration")
        .try_with_nominal_record(
            AcceptedNominalRecord::try_new(
                second_id,
                0,
                AcceptedNominalSemantics::Opaque,
                AcceptedNominalOrigin::Test,
                None,
            )
            .expect("second contextual enum nominal"),
        )
        .expect("second contextual enum registration")
        .try_with_enum_variants(first.clone(), ["Ready"])
        .expect("first contextual enum variants")
        .try_with_enum_variants(second.clone(), ["Ready"])
        .expect("second contextual enum variants");
    (base, first, second)
}

fn record(
    name: &str,
    overload: usize,
    ordinal: usize,
    schema: CallableSignatureSchema,
) -> EnvironmentCallablePublicationRecord {
    EnvironmentCallablePublicationRecord::try_new(
        EnvironmentCallableKind::Function,
        CallableLookupKey::Free(callable_path(&[name])),
        CallableOverloadIndex::try_from_usize(overload).expect("overload index"),
        schema,
        CallableDocumentation::missing(),
        None,
        None,
        EnvironmentDeclarationOrdinal::try_from_usize(ordinal).expect("declaration ordinal"),
    )
    .expect("overload accounting publication record")
}

fn unchecked_single_parameter_schema(name: &str, result: TypeKind) -> CallableSignatureSchema {
    single_parameter_schema(
        name,
        CallableParameterType::Unchecked,
        CallableParameterPassing::PositionalOrNamed,
        CallableParameterPresence::Required,
        SpreadArgumentPolicy::Reject,
        result,
    )
}

fn rest_spread_schema(item: TypeKind, spread: SpreadArgumentPolicy) -> CallableSignatureSchema {
    single_parameter_schema(
        "values",
        CallableParameterType::Exact(item),
        CallableParameterPassing::RestPositional,
        CallableParameterPresence::Optional,
        spread,
        TypeKind::String,
    )
}

fn two_parameter_schema(
    first: TypeKind,
    second: TypeKind,
    result: TypeKind,
) -> CallableSignatureSchema {
    let parameters = [("first", first), ("second", second)]
        .into_iter()
        .enumerate()
        .map(|(index, (name, ty))| {
            CallableParameter::try_new(
                CallableParameterIndex::try_from_usize(index).expect("parameter index"),
                Some(CallableName::try_new(name).expect("parameter name")),
                CallableParameterType::Exact(ty),
                CallableParameterPassing::PositionalOrNamed,
                CallableParameterPresence::Required,
                None,
                None,
            )
            .expect("generic accounting parameter")
        })
        .collect::<Vec<_>>();
    schema(parameters, SpreadArgumentPolicy::Reject, result)
}

fn single_parameter_schema(
    name: &str,
    parameter_type: CallableParameterType,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
    spread: SpreadArgumentPolicy,
    result: TypeKind,
) -> CallableSignatureSchema {
    let parameter = CallableParameter::try_new(
        CallableParameterIndex::try_from_usize(0).expect("parameter index"),
        Some(CallableName::try_new(name).expect("parameter name")),
        parameter_type,
        passing,
        presence,
        None,
        None,
    )
    .expect("accounting parameter");
    schema(vec![parameter], spread, result)
}

fn schema(
    parameters: Vec<CallableParameter>,
    spread: SpreadArgumentPolicy,
    result: TypeKind,
) -> CallableSignatureSchema {
    CallableSignatureSchema::try_new(
        vec![
            CallableParameterGroup::try_new(
                CallableGroupIndex::ZERO,
                CallableGroupKind::Initial,
                parameters,
                &PRODUCTION_CALLABLE_LIMITS,
            )
            .expect("accounting parameter group"),
        ],
        result,
        CallableEffectSchema::fixed(EffectRow::closed(crate::effects::EffectSet::new())),
        CallableArgumentPolicy::new(UnknownNamedArgumentPolicy::Reject, spread),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("accounting schema")
}

fn fact_for_span<'a>(report: &'a TypeCheckReport, span: &SourceSpan) -> &'a CallTargetFacts {
    report
        .retained_call_target_facts()
        .find(|facts| facts.call_span() == span)
        .expect("retained call target for exact source span")
}

fn assert_single_expected_judgment(
    report: &TypeCheckReport,
    expression: TypeExpressionId,
    actual: Option<&TypeKind>,
    expected: &TypeKind,
) {
    let judgments = report
        .judgments
        .iter()
        .filter(|judgment| {
            matches!(
                &judgment.subject,
                TypeJudgmentSubject::Expr { id, .. } if *id == expression
            )
        })
        .collect::<Vec<_>>();
    let [judgment] = judgments.as_slice() else {
        panic!("selected replay must retain exactly one argument judgment")
    };
    assert_eq!(judgment.rule, TypeJudgmentRule::Expected);
    if let Some(actual) = actual {
        assert_eq!(&judgment.ty, actual);
    }
    assert_eq!(judgment.expected_type(), Some(expected));
    assert_eq!(report.stats.judgments, report.judgments.len());
    assert!(expression.index() < report.stats.expressions);
}
