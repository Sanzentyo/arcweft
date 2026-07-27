use super::*;
use crate::smt::{ProofExpr, SmtCheck, SmtEmission, SmtOutcome, SmtProblem, SmtSort, SmtSymbol};
use arcweft_lang_hir::lower::lower_document_to_hir;
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_syntax::parser::parse_source;

fn report(source: &str, mode: VerificationMode) -> VerificationReport {
    let parsed = parse_source(source.to_owned());
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_document_to_hir(parsed.document().as_ref(), parsed.typed_tree())
        .expect("fixture lowers");
    verify_module(
        &hir,
        VerificationPolicy {
            mode,
            backend: BackendKind::Emit,
            allow_trusted_proofs: mode != VerificationMode::Release,
        },
    )
}

fn hir(source: &str) -> arcweft_lang_hir::model::HirModule {
    let parsed = parse_source(source.to_owned());
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    lower_document_to_hir(parsed.document().as_ref(), parsed.typed_tree()).expect("fixture lowers")
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
fn verifier_projects_prefix_and_postfix_try_to_the_same_obligations() {
    let prefix = report(
        "flow @flow.opening opening {\n  let summary = try promote('flow)\n}\n",
        VerificationMode::Test,
    );
    let postfix = report(
        "flow @flow.opening opening {\n  let summary = promote('flow)?\n}\n",
        VerificationMode::Test,
    );

    assert_eq!(prefix.obligations.len(), postfix.obligations.len());
    for (prefix, postfix) in prefix.obligations.iter().zip(&postfix.obligations) {
        assert_eq!(prefix.kind, postfix.kind);
        assert_eq!(prefix.discharge, postfix.discharge);
        assert_eq!(prefix.subject, postfix.subject);
    }
    assert_eq!(prefix.diagnostics.len(), postfix.diagnostics.len());
    for (prefix, postfix) in prefix.diagnostics.iter().zip(&postfix.diagnostics) {
        assert_eq!(prefix.id, postfix.id);
        assert_eq!(prefix.severity, postfix.severity);
        assert_eq!(prefix.obligation, postfix.obligation);
    }
}

#[test]
fn prove_assertion_conditions_create_ordered_unresolved_obligations() {
    let source = "flow assertions {\n  assert.prove(true, false)\n}\n";
    let report = report(source, VerificationMode::Test);
    let obligations = report
        .obligations
        .iter()
        .filter(|obligation| obligation.kind == ProofObligationKind::AssertionProof)
        .collect::<Vec<_>>();

    assert_eq!(obligations.len(), 2);
    assert_eq!(obligations[0].subject.as_deref(), Some("condition.0"));
    assert_eq!(obligations[1].subject.as_deref(), Some("condition.1"));
    assert!(
        obligations
            .iter()
            .all(|obligation| obligation.discharge == ProofDischarge::Missing)
    );
    assert!(
        obligations
            .iter()
            .all(|obligation| obligation.source.is_some())
    );
    assert_eq!(
        report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.id == "verify.proof.unresolved")
            .count(),
        2
    );
}

#[test]
fn semantic_diagnostics_carry_typed_verifier_actions() {
    let report = report(
        r"
flow @flow.effects effects {
    signal.set(@signal.current_flow, @flow.effects)
}
",
        VerificationMode::Test,
    );

    let diagnostic = report
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.actions.iter().any(|action| {
                action.kind == ToolActionKind::GenerateProofStub
                    || action.kind == ToolActionKind::ShowObligation
            })
        })
        .expect("semantic verifier diagnostic exposes typed actions");
    assert!(diagnostic.actions.iter().any(|action| {
        action.kind == ToolActionKind::GenerateProofStub
            && action
                .command
                .as_ref()
                .is_some_and(|command| command.id == "arcweft.verify.generateProofStub")
    }));
}

#[test]
fn verifier_action_source_edit_becomes_diagnostic_suggestion() {
    let action = ToolAction {
        id: "action.generate_proof_stub".to_owned(),
        label: "Generate proof stub".to_owned(),
        kind: ToolActionKind::GenerateProofStub,
        source_edit: None,
        command: None,
    }
    .with_source_edit(
        SourceSpan { start: 0, end: 0 },
        "\nproof @proof.todo {\n    prove _\n}\n",
        ToolActionApplicability::HasPlaceholders,
    );
    let document = arcweft_source::SourceDocument::try_new(
        arcweft_source::SourceDocumentId::try_new("game.arcw").expect("document id"),
        arcweft_source::SourceName::path("game.arcw"),
        "",
    )
    .expect("source document");
    let suggestion = action
        .diagnostic_suggestion(&document)
        .expect("source edit produces suggestion");

    assert_eq!(
        suggestion.applicability(),
        arcweft_source::DiagnosticApplicability::HasPlaceholders
    );
    assert_eq!(suggestion.edits().len(), 1);
    assert!(
        suggestion.edits()[0]
            .replacement()
            .contains("proof @proof.todo")
    );
}

#[test]
fn verifier_host_action_becomes_diagnostic_command() {
    let diagnostic = VerificationDiagnostic {
        id: "diagnostic.obligation.0001".to_owned(),
        severity: Severity::Warning,
        message: "missing proof".to_owned(),
        source: None,
        obligation: Some("obligation.0001".to_owned()),
        related_ids: Vec::new(),
        actions: vec![ToolAction::show_obligation()],
    };
    let document = arcweft_source::SourceDocument::try_new(
        arcweft_source::SourceDocumentId::try_new("game.arcw").expect("document id"),
        arcweft_source::SourceName::path("game.arcw"),
        "",
    )
    .expect("source document");
    let source = diagnostic.source_diagnostic(&document);

    assert!(source.suggestions().is_empty());
    assert_eq!(source.commands().len(), 1);
    assert_eq!(source.commands()[0].id(), "arcweft.verify.showObligation");
    assert_eq!(
        source.commands()[0].arguments(),
        &["obligation.0001".to_owned()]
    );
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
fn proof_insertion_target_generates_source_edit() {
    let report = report(
        r"
flow @flow.effects effects {
    signal.set(@signal.current_flow, @flow.effects)
}
",
        VerificationMode::Test,
    );

    let action = report
        .diagnostics
        .iter()
        .flat_map(|diagnostic| diagnostic.actions.iter())
        .find(|action| action.kind == ToolActionKind::GenerateProofStub)
        .expect("missing proof exposes proof stub action");
    let edit = action
        .source_edit()
        .expect("exact top-level insertion becomes source edit");

    assert_eq!(edit.span().start, edit.span().end);
    assert_eq!(
        edit.applicability(),
        ToolActionApplicability::HasPlaceholders
    );
    assert!(edit.replacement().contains("proof @proof."));
    assert!(edit.replacement().contains("check _"));
}

#[test]
fn proof_insertion_without_target_keeps_host_command() {
    let obligation = ProofObligation {
        id: "obligation.0001".to_owned(),
        kind: ProofObligationKind::LifetimePromotion,
        message: "lifetime promotion requires proof".to_owned(),
        subject: None,
        source: None,
        insertion_target: None,
        discharge: ProofDischarge::Missing,
        smt: None,
    };

    let action = obligation
        .actions()
        .into_iter()
        .find(|action| action.kind == ToolActionKind::GenerateProofStub)
        .expect("proof obligation still exposes host action");

    assert!(action.source_edit().is_none());
    assert!(action.command.is_some());
}

#[test]
fn unsafe_audit_command_waits_for_revision_bound_hir_source_component() {
    let report = report(
        r"
flow @flow.unsafe_demo unsafe_demo {
    unsafe lifetime @unsafe.cache {
        let summary = promote_unchecked('flow)
    }
}
",
        VerificationMode::Test,
    );

    let action = report
        .diagnostics
        .iter()
        .flat_map(|diagnostic| diagnostic.actions.iter())
        .find(|action| action.kind == ToolActionKind::GenerateUnsafeAudit)
        .expect("missing unsafe audit exposes action");
    assert!(action.source_edit().is_none());
    assert!(action.command.is_some());
}

#[test]
fn missing_unsafe_audit_metadata_is_runtime_safety_gap_in_dev() {
    let report = report(
        r"
flow @flow.unsafe_demo unsafe_demo {
    unsafe lifetime @unsafe.cache {
        let summary = promote_unchecked('flow)
    }
}
",
        VerificationMode::Dev,
    );

    assert!(!report.has_errors());
    assert!(report.has_missing_unsafe_audit_metadata());
    assert!(report.has_blocking_runtime_safety_gaps());
}

#[test]
fn unsafe_audit_without_exact_range_keeps_host_command() {
    let obligation = ProofObligation {
        id: "obligation.0002".to_owned(),
        kind: ProofObligationKind::UnsafeLifetimeAudit,
        message: "unsafe lifetime audit requires metadata".to_owned(),
        subject: Some("unsafe.cache".to_owned()),
        source: None,
        insertion_target: None,
        discharge: ProofDischarge::Missing,
        smt: None,
    };

    let action = obligation
        .actions()
        .into_iter()
        .find(|action| action.kind == ToolActionKind::GenerateUnsafeAudit)
        .expect("unsafe audit still exposes host action");

    assert!(action.source_edit().is_none());
    assert!(action.command.is_some());
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
            allow_trusted_proofs: true,
        },
    );
    let with_env = verify_module_with_env(
        &hir,
        &TypeCheckEnv::new().with_capability("signal.write"),
        VerificationPolicy {
            mode: VerificationMode::Test,
            backend: BackendKind::Emit,
            allow_trusted_proofs: true,
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
fn trusted_proof_evidence_is_transitive_auditable_and_policy_controlled() {
    let source = r#"
#[verify.trusted(reason = "validated by signed build metadata")]
proof @proof.external_fact {
    check no_lifetime_below(LineSummary, 'flow)
}

proof @proof.dependent {
    use @proof.external_fact
    check no_lifetime_below(LineSummary, 'flow)
}

flow @flow.proven proven {
    let summary = promote('flow, proof = @proof.dependent)
}
"#;
    let dev = report(source, VerificationMode::Dev);

    assert!(!dev.has_errors());
    assert!(dev.proofs.iter().any(|proof| {
        proof.id == "proof.external_fact"
            && matches!(
                &proof.trust,
                ProofTrustSummary::Trusted { reason }
                    if reason == "validated by signed build metadata"
            )
    }));
    assert!(dev.proofs.iter().any(|proof| {
        proof.id == "proof.dependent"
            && proof.trust == ProofTrustSummary::Verified
            && proof.trusted_dependencies == ["proof.external_fact".to_owned()]
    }));
    assert_eq!(dev.trusted_proofs().count(), 2);
    assert!(dev.obligations.iter().any(|obligation| {
        obligation.kind == ProofObligationKind::LifetimePromotion
            && matches!(
                &obligation.discharge,
                ProofDischarge::TrustedProof {
                    id,
                    trusted_dependencies,
                } if id == "proof.dependent"
                    && trusted_dependencies == &["proof.external_fact".to_owned()]
            )
    }));

    let release = report(source, VerificationMode::Release);
    assert!(release.has_errors());
    assert!(release.diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == Severity::Error
            && diagnostic.obligation.as_deref().is_some_and(|id| {
                release.obligations.iter().any(|obligation| {
                    obligation.id == id
                        && matches!(obligation.discharge, ProofDischarge::TrustedProof { .. })
                })
            })
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
            allow_trusted_proofs: true,
        },
        obligations: vec![ProofObligation {
            id: "obligation.0001".to_owned(),
            kind: ProofObligationKind::ProofBody,
            message: "proof body requires solver".to_owned(),
            subject: Some("proof.example".to_owned()),
            source: None,
            insertion_target: None,
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
            allow_trusted_proofs: true,
        },
        obligations: vec![ProofObligation {
            id: "obligation.0001".to_owned(),
            kind: ProofObligationKind::ProofBody,
            message: "proof body can be advisory in dev".to_owned(),
            subject: None,
            source: None,
            insertion_target: None,
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
            allow_trusted_proofs: true,
        },
        obligations: vec![ProofObligation {
            id: "obligation.0001".to_owned(),
            kind: ProofObligationKind::FunctionContract,
            message: "postcondition".to_owned(),
            subject: Some("function.example.ensures.1".to_owned()),
            source: None,
            insertion_target: None,
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
