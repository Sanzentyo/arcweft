use super::*;
use arcweft_lang_hir::lower_to_hir;
use arcweft_lang_syntax::parse_source;

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
        cancel on input .SkipLine { out .Skipped }
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
    let problem = SmtProblem {
        name: "p".to_owned(),
        assertions: vec![ProofExpr::App {
            name: "must_drop_discharged".to_owned(),
            args: vec![ProofExpr::Var("'line.focus".to_owned())],
        }],
    };
    assert!(emit_smt_lib(&problem).contains("(check-sat)"));
    assert!(emit_smt_lib(&problem).contains("must_drop_discharged"));
}
