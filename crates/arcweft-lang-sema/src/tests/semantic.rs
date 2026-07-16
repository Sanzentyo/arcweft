use super::support::*;

fn semantic_report(source: &str, env: &TypeCheckEnv) -> SemanticReport {
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("fixture lowers to HIR");
    analyze_semantics(
        &hir,
        env,
        SemanticPolicy {
            mode: SemanticMode::Test,
            allow_trusted_proofs: true,
        },
    )
}

#[test]
fn semantic_pass_reports_line_must_drop_until_consumed() {
    let missing = semantic_report(
        r"
flow @flow.must_drop must_drop {
    alice[待って。[p]]
    with:
        init:
            let focus = 'line.focus?
}
",
        &TypeCheckEnv::new(),
    );

    assert!(missing.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::MustDropDischarge
            && obligation.discharge == SemanticDischarge::Missing
            && obligation.subject.as_deref() == Some("line.focus")
    }));

    let discharged = semantic_report(
        r"
flow @flow.must_drop must_drop {
    alice[待って。[p]]
    with:
        init:
            let focus = 'line.focus?
            defer { 'line.focus |> drop_optional }
}
",
        &TypeCheckEnv::new(),
    );

    assert!(!discharged.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::MustDropDischarge
            && obligation.discharge == SemanticDischarge::Missing
            && obligation.subject.as_deref() == Some("line.focus")
    }));
}

#[test]
fn semantic_cfg_requires_must_drop_discharge_on_every_branch() {
    let report = semantic_report(
        r"
flow @flow.branch_drop branch_drop {
    alice[待って。[p]]
    with:
        init:
            let focus = 'line.focus?
            if should_release {
                defer { 'line.focus |> drop_optional }
            }
}
",
        &TypeCheckEnv::new(),
    );

    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::MustDropDischarge
            && obligation.discharge == SemanticDischarge::Missing
            && obligation.subject.as_deref() == Some("line.focus")
    }));
}

#[test]
fn semantic_cfg_applies_defer_outcomes_to_cancel_paths() {
    let completed_only = semantic_report(
        r"
flow @flow.cancel_cleanup cancel_cleanup {
    alice[待って。[p]]
    with:
        init:
            let focus = 'line.focus?
        defer on completed:
            'line.focus |> drop_optional
        cancel on input(.SkipLine) { out .Skipped }
}
",
        &TypeCheckEnv::new(),
    );

    assert!(completed_only.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::MustDropDischarge
            && obligation.discharge == SemanticDischarge::Missing
            && obligation.subject.as_deref() == Some("line.focus")
    }));

    let completed_and_cancelled = semantic_report(
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
        &TypeCheckEnv::new(),
    );

    assert!(
        !completed_and_cancelled
            .obligations
            .iter()
            .any(|obligation| {
                obligation.kind == SemanticObligationKind::MustDropDischarge
                    && obligation.discharge == SemanticDischarge::Missing
                    && obligation.subject.as_deref() == Some("line.focus")
            })
    );
}

#[test]
fn semantic_pass_requires_capability_for_upper_lifetime_writes() {
    let source = r"
flow @flow.registry registry {
    'flow.flags.seen <- true
}
";
    let missing = semantic_report(source, &TypeCheckEnv::new());
    assert!(missing.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::UpperLifetimeWrite
            && obligation.discharge == SemanticDischarge::Missing
            && obligation.subject.as_deref() == Some("flow.flags.seen")
    }));

    let allowed = semantic_report(
        source,
        &TypeCheckEnv::new().with_capability("state.write(flow)"),
    );
    assert!(allowed.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::UpperLifetimeWrite
            && obligation.discharge == SemanticDischarge::Automatic
            && obligation.subject.as_deref() == Some("flow.flags.seen")
    }));

    let allowed_by_contract = semantic_report(
        r"
flow @flow.registry_contract registry_contract
effects { state.write('flow) }
{
    'flow.flags.seen <- true
}
",
        &TypeCheckEnv::new(),
    );
    assert!(allowed_by_contract.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::UpperLifetimeWrite
            && obligation.discharge == SemanticDischarge::Automatic
            && obligation.subject.as_deref() == Some("flow.flags.seen")
    }));
}

#[test]
fn semantic_pass_accepts_known_formal_proof_reference() {
    let report = semantic_report(
        r"
proof @proof.line_summary_to_flow {
    check no_lifetime_below(LineSummary, 'flow)
}

flow @flow.proven proven {
    let summary = promote('flow, proof = @proof.line_summary_to_flow)
}
",
        &TypeCheckEnv::new(),
    );

    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::LifetimePromotion
            && matches!(
                obligation.discharge,
                SemanticDischarge::FormalProof { ref id }
                    if id == "proof.line_summary_to_flow"
            )
    }));
}

#[test]
fn semantic_pass_rejects_formal_proof_for_wrong_lifetime_target() {
    let report = semantic_report(
        r"
proof @proof.line_only {
    check no_lifetime_below(LineSummary, 'line)
}

flow @flow.proven proven {
    let summary = promote('flow, proof = @proof.line_only)
}
",
        &TypeCheckEnv::new(),
    );

    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::LifetimePromotion
            && obligation.discharge == SemanticDischarge::Missing
            && obligation.subject.as_deref() == Some("'flow")
    }));
}

#[test]
fn semantic_pass_requires_checked_proof_body_for_discharge() {
    let report = semantic_report(
        r"
proof @proof.requires_only {
    requires summary.lifetime >= 'flow
}

flow @flow.unproven unproven {
    let summary = promote('flow, proof = @proof.requires_only)
}
",
        &TypeCheckEnv::new(),
    );

    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::ProofBody
            && obligation.discharge == SemanticDischarge::Missing
            && obligation.subject.as_deref() == Some("proof.requires_only")
    }));
    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::LifetimePromotion
            && obligation.discharge == SemanticDischarge::Missing
    }));
}

#[test]
fn semantic_pass_rejects_unjustified_proof_assume() {
    let report = semantic_report(
        r"
proof @proof.assume_only {
    assume no_lifetime_below(LineSummary, 'flow)
    check no_lifetime_below(LineSummary, 'flow)
}
",
        &TypeCheckEnv::new(),
    );

    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::ProofBody
            && obligation.discharge == SemanticDischarge::Missing
            && obligation
                .subject
                .as_deref()
                .is_some_and(|subject| subject.contains("no_lifetime_below"))
    }));
}

#[test]
fn semantic_pass_propagates_trust_through_proof_dependencies() {
    let report = semantic_report(
        r#"
#[verify.trusted(reason = "generated by signed build tooling")]
proof @proof.manifest_hashes {
    check no_lifetime_below(LineSummary, 'flow)
}

proof @proof.dependent {
    use @proof.manifest_hashes
    assume no_lifetime_below(LineSummary, 'flow), proof = @proof.manifest_hashes
    check no_lifetime_below(LineSummary, 'flow)
}

flow @flow.proven proven {
    let summary = promote('flow, proof = @proof.dependent)
}
"#,
        &TypeCheckEnv::new(),
    );

    assert!(!report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::ProofBody
            && obligation.discharge == SemanticDischarge::Missing
    }));
    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::LifetimePromotion
            && matches!(
                obligation.discharge,
                SemanticDischarge::TrustedProof {
                    ref id,
                    ref trusted_dependencies,
                } if id == "proof.dependent"
                    && trusted_dependencies == &["proof.manifest_hashes".to_owned()]
            )
    }));
    assert!(report.proofs.iter().any(|proof| {
        proof.id == "proof.dependent"
            && proof.trusted_dependencies == ["proof.manifest_hashes".to_owned()]
    }));
}

#[test]
fn semantic_pass_reports_unknown_proof_dependency() {
    let report = semantic_report(
        r"
proof @proof.missing_dependency {
    use @proof.missing
    check no_lifetime_below(LineSummary, 'flow)
}
",
        &TypeCheckEnv::new(),
    );

    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::ProofBody
            && obligation.discharge == SemanticDischarge::Missing
            && obligation.subject.as_deref() == Some("proof.missing")
    }));
}

#[test]
fn semantic_pass_reports_thread_join_result_type_conflicts() {
    let report = semantic_report(
        r#"
flow @flow.thread_join thread_join {
    thread worker {
        out 1
        out "bad"
    }
}
"#,
        &TypeCheckEnv::new(),
    );

    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::ThreadJoinTyping
            && obligation.discharge == SemanticDischarge::Missing
    }));
}

#[test]
fn semantic_pass_reports_thread_join_fallthrough_unit_conflicts() {
    let report = semantic_report(
        r"
flow @flow.thread_join thread_join {
    thread worker {
        if ready {
            out 1
        }
    }
}
",
        &TypeCheckEnv::new(),
    );

    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::ThreadJoinTyping
            && obligation.discharge == SemanticDischarge::Missing
    }));
}

#[test]
fn semantic_pass_ignores_cleanup_out_for_thread_join_result() {
    let report = semantic_report(
        r#"
flow @flow.thread_join thread_join {
    thread worker {
        defer { out "cleanup" }
        out 1
    }
}
"#,
        &TypeCheckEnv::new(),
    );

    assert!(!report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::ThreadJoinTyping
            && obligation.discharge == SemanticDischarge::Missing
    }));
}

#[test]
fn semantic_pass_reports_line_child_task_write_conflicts() {
    let report = semantic_report(
        r"
flow @flow.line_conflict line_conflict {
    alice[待って。[p]]
    with:
        thread left:
            signal.set(@signal.current_flow, @flow.a)
        thread right:
            signal.set(@signal.current_flow, @flow.b)
}
",
        &TypeCheckEnv::new(),
    );

    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::RuntimeConflict
            && obligation.discharge == SemanticDischarge::Missing
            && obligation
                .subject
                .as_deref()
                .is_some_and(|subject| subject.contains("signal.current_flow"))
    }));
}

#[test]
fn semantic_pass_reports_sibling_thread_write_conflicts() {
    let report = semantic_report(
        r"
flow @flow.conflict conflict {
    thread left {
        signal.set(@signal.current_flow, @flow.a)
    }
    thread right {
        signal.set(@signal.current_flow, @flow.b)
    }
}
",
        &TypeCheckEnv::new(),
    );

    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::RuntimeConflict
            && obligation.discharge == SemanticDischarge::Missing
            && obligation
                .subject
                .as_deref()
                .is_some_and(|subject| subject.contains("signal.current_flow"))
    }));
}

#[test]
fn semantic_pass_requires_effect_capability_for_signal_and_metric_writes() {
    let source = r"
flow @flow.effects effects {
    signal.set(@signal.current_flow, @flow.effects)
    metric.set(@metric.frame_count, 1)
}
";
    let missing = semantic_report(source, &TypeCheckEnv::new());
    assert!(missing.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::EffectCapability
            && obligation.discharge == SemanticDischarge::Missing
            && obligation.subject.as_deref() == Some("signal.write")
    }));
    assert!(missing.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::EffectCapability
            && obligation.discharge == SemanticDischarge::Missing
            && obligation.subject.as_deref() == Some("metric.write")
    }));

    let allowed = semantic_report(
        source,
        &TypeCheckEnv::new()
            .with_capability("signal.write")
            .with_capability("metric.write"),
    );
    assert!(allowed.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::EffectCapability
            && obligation.discharge == SemanticDischarge::Automatic
            && obligation.subject.as_deref() == Some("signal.write")
    }));
    assert!(allowed.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::EffectCapability
            && obligation.discharge == SemanticDischarge::Automatic
            && obligation.subject.as_deref() == Some("metric.write")
    }));

    let from_contract = semantic_report(
        r"
flow @flow.effects effects
effects { signal.write, metric.write }
{
    signal.set(@signal.current_flow, @flow.effects)
    metric.set(@metric.frame_count, 1)
}
",
        &TypeCheckEnv::new(),
    );
    assert!(from_contract.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::EffectCapability
            && obligation.discharge == SemanticDischarge::Automatic
            && obligation.subject.as_deref() == Some("signal.write")
    }));
    assert!(from_contract.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::EffectCapability
            && obligation.discharge == SemanticDischarge::Automatic
            && obligation.subject.as_deref() == Some("metric.write")
    }));
}

#[test]
fn semantic_pass_records_unsafe_audit_and_audited_promotion() {
    let report = semantic_report(
        r#"
flow @flow.audit audit {
    unsafe lifetime @unsafe.cache reason = "owned clone" {
        /// SAFETY: value is cloned before promotion and does not borrow line state.
        let cached = promote_unchecked('flow)
    }
}
"#,
        &TypeCheckEnv::new(),
    );

    assert_eq!(report.unsafe_audits.len(), 1);
    assert!(report.unsafe_audits[0].has_reason);
    assert!(report.unsafe_audits[0].has_safety_doc);
    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::LifetimePromotion
            && matches!(
                obligation.discharge,
                SemanticDischarge::AuditedUnsafe { .. }
            )
    }));
}

#[test]
fn semantic_pass_rejects_empty_unsafe_lifetime_audit() {
    let report = semantic_report(
        r#"
flow @flow.audit audit {
    unsafe lifetime @unsafe.empty reason = "owned clone" {
        /// SAFETY: this block has no unchecked promotion.
        let x = 1
    }
}
"#,
        &TypeCheckEnv::new(),
    );

    assert!(report.obligations.iter().any(|obligation| {
        obligation.kind == SemanticObligationKind::UnsafeLifetimeAudit
            && obligation.discharge == SemanticDischarge::Missing
            && obligation.subject.as_deref() == Some("unsafe.empty")
    }));
}
