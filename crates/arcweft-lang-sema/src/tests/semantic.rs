use super::support::*;

fn semantic_report(source: &str, env: &TypeCheckEnv) -> SemanticReport {
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("fixture lowers to HIR");
    analyze_semantics(
        &hir,
        env,
        SemanticPolicy {
            mode: SemanticMode::Test,
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
