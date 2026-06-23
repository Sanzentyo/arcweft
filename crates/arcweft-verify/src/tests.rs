use super::*;
use crate::smt::{ProofExpr, SmtCheck, SmtEmission, SmtOutcome, SmtProblem, SmtSort, SmtSymbol};
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_syntax::parser::parse_source;

fn report(source: &str, mode: VerificationMode) -> VerificationReport {
    let parsed = parse_source(source.to_owned());
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let tree = parsed.into_typed_tree();
    let hir = lower_to_hir(&tree).expect("fixture lowers");
    verify_module(
        &hir,
        VerificationPolicy {
            mode,
            backend: BackendKind::Emit,
        },
    )
}

fn hir(source: &str) -> arcweft_lang_hir::model::HirModule {
    let parsed = parse_source(source.to_owned());
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let tree = parsed.into_typed_tree();
    lower_to_hir(&tree).expect("fixture lowers")
}

#[test]
fn promotion_without_proof_is_an_obligation() {
    let report = report(
        "flow @flow.opening opening {\n  let summary = promote('flow)\n}\n",
        VerificationMode::Test,
    );
    assert!(report.has_errors());
    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == ProofObligationKind::LifetimePromotion
            && obligation.discharge == ProofDischarge::Missing
    }));
}

#[test]
fn unsafe_lifetime_records_audit() {
    let report = report(
        "flow @flow.opening opening {\n  unsafe lifetime @unsafe.cache reason = \"ok\" {\n    /// SAFETY: owned clone only\n    let summary = promote_unchecked('flow)\n  }\n}\n",
        VerificationMode::Dev,
    );
    assert_eq!(report.unsafe_audit_count(), 1);
    assert!(
        report
            .obligations
            .iter()
            .any(|obligation| matches!(obligation.discharge, ProofDischarge::AuditedUnsafe { .. }))
    );
}

#[test]
fn runtime_parallel_conflict_is_verifier_obligation() {
    let report = report(
        r"
flow @flow.conflict conflict {
    alice[待って。[p]]
    with {
        together {
            signal.set(@signal.current_flow, @flow.a)
            signal.set(@signal.current_flow, @flow.b)
        }
    }
}
",
        VerificationMode::Dev,
    );

    assert!(report.has_errors());
    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == ProofObligationKind::RuntimeConflict
            && obligation.message.contains("write conflict")
    }));
}

#[test]
fn semantic_thread_join_conflict_is_verifier_obligation() {
    let report = report(
        r#"
flow @flow.thread_join thread_join {
    thread worker {
        out 1
        out "bad"
    }
}
"#,
        VerificationMode::Test,
    );

    assert!(report.has_errors());
    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == ProofObligationKind::ThreadJoinTyping
            && obligation.discharge == ProofDischarge::Missing
    }));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.obligation.as_deref().is_some_and(|id| {
            report
                .obligations
                .iter()
                .any(|obligation| obligation.id == id)
        })
    }));
}

#[test]
fn semantic_effect_capability_is_verifier_obligation() {
    let report = report(
        r"
flow @flow.effects effects {
    signal.set(@signal.current_flow, @flow.effects)
}
",
        VerificationMode::Test,
    );

    assert!(report.has_errors());
    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == ProofObligationKind::EffectCapability
            && obligation.discharge == ProofDischarge::Missing
            && obligation.subject.as_deref() == Some("signal.write")
    }));
}

#[test]
fn verifier_uses_adapter_typecheck_env_for_semantic_discharge() {
    let hir = hir(r"
flow @flow.effects effects {
    signal.set(@signal.current_flow, @flow.effects)
}
");

    let without_env = verify_module(
        &hir,
        VerificationPolicy {
            mode: VerificationMode::Test,
            backend: BackendKind::Emit,
        },
    );
    let with_env = verify_module_with_env(
        &hir,
        &TypeCheckEnv::new().with_capability("signal.write"),
        VerificationPolicy {
            mode: VerificationMode::Test,
            backend: BackendKind::Emit,
        },
    );

    assert!(without_env.has_errors());
    assert!(!with_env.has_errors());
    assert!(with_env.obligations.iter().any(|obligation| {
        obligation.kind == ProofObligationKind::EffectCapability
            && obligation.discharge == ProofDischarge::Automatic
            && obligation.subject.as_deref() == Some("signal.write")
    }));
}

#[test]
fn semantic_effect_capability_can_be_discharged_by_effects_clause() {
    let report = report(
        r"
flow @flow.effects effects
effects { signal.write }
{
    signal.set(@signal.current_flow, @flow.effects)
}
",
        VerificationMode::Test,
    );

    assert!(!report.has_errors());
    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == ProofObligationKind::EffectCapability
            && obligation.discharge == ProofDischarge::Automatic
            && obligation.subject.as_deref() == Some("signal.write")
    }));
}

#[test]
fn semantic_state_write_can_be_discharged_by_effects_clause() {
    let report = report(
        r"
flow @flow.registry registry
effects { state.write('flow) }
{
    'flow.flags.seen <- true
}
",
        VerificationMode::Test,
    );

    assert!(!report.has_errors());
    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == ProofObligationKind::UpperLifetimeWrite
            && obligation.discharge == ProofDischarge::Automatic
            && obligation.subject.as_deref() == Some("flow.flags.seen")
    }));
}

#[test]
fn proof_body_issues_are_verifier_obligations() {
    let report = report(
        r"
proof @proof.requires_only {
    requires summary.lifetime >= 'flow
}
",
        VerificationMode::Test,
    );

    assert!(report.has_errors());
    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == ProofObligationKind::ProofBody
            && obligation.discharge == ProofDischarge::Missing
            && obligation.subject.as_deref() == Some("proof.requires_only")
    }));
}

#[test]
fn semantic_cfg_discharge_is_not_overridden_by_verifier_scan() {
    let report = report(
        r"
flow @flow.cancel_cleanup cancel_cleanup {
    alice[待って。[p]]
    with:
        init:
            let focus = 'line.focus?
        defer on completed:
            'line.focus |> drop_optional
        defer on cancelled:
            'line.focus |> drop_optional
        cancel on input(.SkipLine) { out .Skipped }
}
",
        VerificationMode::Test,
    );

    assert!(!report.obligations.iter().any(|obligation| {
        obligation.kind == ProofObligationKind::MustDropDischarge
            && obligation.discharge == ProofDischarge::Missing
            && obligation.subject.as_deref() == Some("line.focus")
    }));
}

#[test]
fn smt_lib_is_stable() {
    let problem = SmtProblem::counterexample(
        "p",
        vec![SmtSymbol::new("focus_discharged", SmtSort::Bool)],
        [],
        ProofExpr::var("focus_discharged"),
        Vec::new(),
    );
    let script = problem
        .emit_smt_lib(SmtEmission::CheckOnly)
        .expect("problem emits");
    assert!(script.contains("(declare-const focus_discharged Bool)"));
    assert!(script.contains("(check-sat)"));
}

#[test]
fn required_solver_checks_are_report_diagnostics() {
    let mut report = VerificationReport {
        policy: VerificationPolicy {
            mode: VerificationMode::Test,
            backend: BackendKind::Oxiz,
        },
        obligations: vec![ProofObligation {
            id: "obligation.0001".to_owned(),
            kind: ProofObligationKind::ProofBody,
            message: "proof body requires solver".to_owned(),
            subject: Some("proof.example".to_owned()),
            source: None,
            discharge: ProofDischarge::Missing,
            smt: Some(SmtProblem::counterexample(
                "obligation.0001",
                vec![SmtSymbol::new("proof_body_valid", SmtSort::Bool)],
                [],
                ProofExpr::var("proof_body_valid"),
                Vec::new(),
            )),
        }],
        ..VerificationReport::default()
    };

    report.record_solver_check(
        "obligation.0001",
        BackendKind::Oxiz,
        Ok(SmtCheck::new(SmtOutcome::Unknown)),
    );

    assert!(report.has_solver_failures());
    assert!(report.has_errors());
    assert_eq!(report.solver_checks.len(), 1);
    assert!(report.solver_checks[0].required);
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("required solver check `obligation.0001`")
    }));
}

#[test]
fn non_required_solver_checks_are_recorded_without_errors() {
    let mut report = VerificationReport {
        policy: VerificationPolicy {
            mode: VerificationMode::Dev,
            backend: BackendKind::Oxiz,
        },
        obligations: vec![ProofObligation {
            id: "obligation.0001".to_owned(),
            kind: ProofObligationKind::ProofBody,
            message: "proof body can be advisory in dev".to_owned(),
            subject: None,
            source: None,
            discharge: ProofDischarge::Missing,
            smt: None,
        }],
        ..VerificationReport::default()
    };

    report.record_solver_check(
        "obligation.0001",
        BackendKind::Oxiz,
        Ok(SmtCheck::new(SmtOutcome::Unknown)),
    );

    assert!(!report.has_solver_failures());
    assert!(!report.has_errors());
    assert_eq!(report.solver_checks.len(), 1);
    assert!(!report.solver_checks[0].required);
}

#[test]
fn unsat_solver_check_records_solver_discharge() {
    let mut report = VerificationReport {
        policy: VerificationPolicy {
            mode: VerificationMode::Test,
            backend: BackendKind::Z3,
        },
        obligations: vec![ProofObligation {
            id: "obligation.0001".to_owned(),
            kind: ProofObligationKind::FunctionContract,
            message: "postcondition".to_owned(),
            subject: Some("function.example.ensures.1".to_owned()),
            source: None,
            discharge: ProofDischarge::Missing,
            smt: None,
        }],
        ..VerificationReport::default()
    };

    report.record_solver_check(
        "obligation.0001",
        BackendKind::Z3,
        Ok(SmtCheck::new(SmtOutcome::Unsat)),
    );

    assert_eq!(
        report.obligations[0].discharge,
        ProofDischarge::Solver {
            backend: BackendKind::Z3
        }
    );
    assert!(!report.has_solver_failures());
}
