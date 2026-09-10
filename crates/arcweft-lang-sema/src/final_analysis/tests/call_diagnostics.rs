use std::sync::atomic::AtomicBool;

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::DiagnosticSeverity;

use super::{
    Fixture, TestCallableOverload, analyze, analyze_with_callable_limits, fixture,
    typed_overload_fixture,
};
use crate::{
    callable::{
        CallableDiagnosticCode, CallableDiagnosticSeverity, CallableLimits,
        CallableQueryLimitError, PRODUCTION_CALLABLE_LIMITS, SemanticSignatureError,
    },
    final_analysis::FinalSemanticAnalysisError,
    signature::{SignatureQuery, SignatureQueryControl, SignatureQueryOutcome, query_signature},
    types::TypeKind,
};

fn overload_fixture(argument: &str) -> Fixture {
    typed_overload_fixture(
        &format!("fn caller() {{ choose({argument}); }}\n"),
        "choose",
        vec![
            TestCallableOverload::strict([TypeKind::I64], TypeKind::I64),
            TestCallableOverload::strict([TypeKind::U64], TypeKind::U64),
        ],
    )
}

#[test]
fn final_call_diagnostics_follow_the_retained_outcome_and_source() {
    for (fixture, expected) in [
        (
            overload_fixture("true"),
            CallableDiagnosticCode::NoViableSignature,
        ),
        (
            overload_fixture("1"),
            CallableDiagnosticCode::AmbiguousOverload,
        ),
        (
            fixture("fn caller(value: i64) { value(); }\n", None),
            CallableDiagnosticCode::NonCallableTarget,
        ),
    ] {
        let analysis = analyze(&fixture)
            .unwrap_or_else(|error| panic!("final {expected:?} tooling evidence: {error:?}"));
        let calls = analysis.calls().collect::<Vec<_>>();
        let [(owner, facts)] = calls.as_slice() else {
            panic!("one final call: {calls:?}");
        };
        let [diagnostic] = facts.diagnostics() else {
            panic!("one diagnostic for final {expected:?}");
        };
        assert_eq!(diagnostic.code(), expected);
        assert_eq!(diagnostic.severity(), CallableDiagnosticSeverity::Error);
        let module = fixture
            .project
            .analysis_view()
            .unwrap()
            .module(&CanonicalModulePath::crate_root())
            .unwrap();
        let expected_span = module.source_anchor(facts.source_query()).unwrap().unwrap();
        assert_eq!(diagnostic.span(), Some(&expected_span));
        assert_eq!(
            analysis.call_diagnostics().collect::<Vec<_>>(),
            [diagnostic]
        );
        assert_eq!(analysis.work().call_diagnostics(), 1);
        let projected = diagnostic.to_source_diagnostic();
        assert_eq!(projected.span(), Some(&expected_span));
        assert_eq!(
            projected.code().map(arcweft_source::DiagnosticCode::as_str),
            Some(expected.as_str())
        );
        assert_eq!(projected.severity(), DiagnosticSeverity::Error);
        super::callable_values::assert_unselected_call_has_no_execution(&analysis, *owner);
    }
}

#[test]
fn final_call_diagnostics_are_shared_with_signature_help() {
    for argument in ["true", "1"] {
        let fixture = overload_fixture(argument);
        let analysis = analyze(&fixture).unwrap();
        let facts = analysis.calls().next().unwrap().1;
        let module = fixture
            .project
            .analysis_view()
            .unwrap()
            .module(&CanonicalModulePath::crate_root())
            .unwrap();
        let offset = fixture.root_document.text().find(argument).unwrap();
        let cancellation = AtomicBool::new(false);
        let SignatureQueryOutcome::Help(help) = query_signature(
            SignatureQuery::production(
                &fixture.registered,
                &fixture.root_document,
                module,
                &analysis,
                offset,
                SignatureQueryControl::new(&cancellation, None),
            )
            .unwrap(),
        )
        .unwrap() else {
            panic!("rejected and ambiguous candidates remain available to signature help");
        };
        assert_eq!(help.diagnostics(), facts.diagnostics());
        assert_eq!(analysis.work().call_diagnostics(), 1);
    }
}

#[test]
fn final_call_diagnostics_do_not_leak_from_losing_candidates() {
    let fixture = overload_fixture("1i64");
    let analysis = analyze(&fixture).unwrap();
    assert!(
        analysis
            .calls()
            .all(|(_, call)| call.selected_application().is_some())
    );
    assert_eq!(analysis.call_diagnostics().count(), 0);
    assert_eq!(analysis.work().call_diagnostics(), 0);
}

#[test]
fn final_call_diagnostics_limit_rejects_the_generation() {
    let fixture = overload_fixture("true");
    let production = PRODUCTION_CALLABLE_LIMITS;
    let limits = |diagnostics| {
        CallableLimits::for_test(
            production.max_path_segments(),
            production.max_groups_per_callable(),
            production.max_parameters_per_callable(),
            production.max_overloads_per_key(),
            production.max_candidates_per_call(),
            production.max_nested_calls(),
            production.max_recovery_nodes(),
            diagnostics,
            production.max_catalog_build_work(),
            production.max_query_work(),
        )
    };
    assert_eq!(
        analyze_with_callable_limits(&fixture, limits(1))
            .unwrap()
            .call_diagnostics()
            .count(),
        1,
    );
    assert!(matches!(
        analyze_with_callable_limits(&fixture, limits(0)),
        Err(FinalSemanticAnalysisError::CallFactsSeal { error, .. })
            if matches!(*error, SemanticSignatureError::Limit(
                CallableQueryLimitError::Diagnostics { actual: 1, limit: 0 }
            ))
    ));
}

#[test]
fn final_call_diagnostics_retain_unselected_argument_sources() {
    let fixture = fixture(
        "fn identity(value: i64) -> i64 { value }\nfn caller(value: i64) { value(identity(1i64)); }\n",
        None,
    );
    let analysis = analyze(&fixture).unwrap();
    let call = analysis
        .calls()
        .find_map(|(_, call)| {
            matches!(
                call.outcome(),
                crate::callable::CallAnalysisOutcome::NonCallable(_)
            )
            .then_some(call)
        })
        .unwrap();
    assert_eq!(call.accounting().logical_argument_checks(), 1);
    assert_eq!(call.accounting().retained_argument_fact_publications(), 1);
    assert_eq!(call.accounting().candidate_argument_probes(), 0);
    assert_eq!(
        analysis
            .calls()
            .filter(|(_, call)| call.selected_application().is_some())
            .count(),
        1
    );
    assert_eq!(analysis.call_diagnostics().count(), 1);

    let invalid = super::fixture("fn caller(value: i64) { value(unknown()); }\n", None);
    assert!(matches!(
        analyze(&invalid),
        Err(FinalSemanticAnalysisError::UnknownCallTarget { .. })
    ));
}

#[test]
fn final_call_diagnostics_preserve_poisoned_type_failure() {
    let fixture = fixture("fn caller() { Vec<i32, i64>.with_capacity(1); }\n", None);
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::PoisonedType)
    ));
}

#[test]
fn final_call_diagnostics_reject_a_foreign_hir_source() {
    let first = overload_fixture("true");
    let second = overload_fixture("true");
    let analysis = analyze(&first).unwrap();
    let call = analysis.calls().next().unwrap().1;
    let foreign_module = second
        .project
        .analysis_view()
        .unwrap()
        .module(&CanonicalModulePath::crate_root())
        .unwrap();
    let result = crate::callable::CallTargetFacts::try_new(
        crate::callable::CallTargetFactsInput {
            enclosing_callable: call.enclosing_callable().cloned(),
            outcome: call.outcome().clone(),
            accounting: call.accounting(),
        },
        foreign_module,
        &PRODUCTION_CALLABLE_LIMITS,
    );
    assert!(matches!(result, Err(SemanticSignatureError::CallSource(_))));
}

#[test]
fn final_call_diagnostics_on_project_bindings_have_no_candidate_execution() {
    let fixture = fixture(
        concat!(
            "pub signal @signal.payload Payload: Watch<i64>\n",
            "fn caller() { @signal.payload(1i64); }\n",
        ),
        None,
    );
    let cancellation = AtomicBool::new(false);
    let (result, physical) =
        crate::final_analysis::analyzer::analyze_final_project_with_physical_trace_for_test(
            fixture.project.analysis_view().unwrap(),
            &fixture.symbols,
            crate::final_analysis::FinalSemanticCatalogs::production(&fixture.registered),
            crate::final_analysis::FinalSemanticAnalysisControl::new(&cancellation),
        );
    let analysis = result.expect("a non-callable project binding retains its diagnostic");
    let (owner, call) = analysis.calls().next().unwrap();
    assert!(matches!(
        call.outcome(),
        crate::callable::CallAnalysisOutcome::NonCallable(_)
    ));
    assert_eq!(
        call.diagnostics()[0].code(),
        CallableDiagnosticCode::NonCallableTarget
    );
    assert_eq!(call.accounting().candidate_argument_probes(), 0);
    assert_eq!(call.accounting().retained_argument_fact_publications(), 1);
    assert!(physical.is_empty());
    super::callable_values::assert_unselected_call_has_no_execution(&analysis, owner);
}
