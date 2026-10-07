use super::*;

fn group() -> crate::runtime_id::RuntimeLineTaskGroupId {
    crate::runtime_id::RuntimeLineTaskGroupId::from_zero_based(0).unwrap()
}

fn row(
    plan: &RuntimePlan,
    owner: &crate::plan::RuntimeTaskPlanCoordinateOwner,
    work: u64,
    bytes: u64,
) -> (Result<blake3::Hash, RuntimeBodySemanticError>, (u64, u64)) {
    let context = RuntimeBodySemanticContext::new(plan);
    let mut meter = TaskSemanticMeter::new(work, bytes);
    let result = context
        .line_semantic(
            &mut meter,
            group(),
            owner,
            &mut |_| panic!("no task references"),
            crate::plan::RuntimeTaskPlanSealLimits::default(),
        )
        .and_then(|semantic| semantic.executable_row_digest(&context, &mut meter));
    (result, meter.totals())
}

#[test]
fn executable_line_row_commits_the_actual_completed_tree_and_excludes_mark_spelling() {
    let first = actual_line_plan(true, true, "cancel", "mark");
    let renamed = actual_line_plan(true, true, "cancel", "renamed");
    let expected = row(&first.0, &first.1, 100_000, 1_000_000).0.unwrap();
    assert_eq!(
        expected,
        row(&renamed.0, &renamed.1, 100_000, 1_000_000).0.unwrap()
    );
    for changed in [
        actual_line_plan(false, true, "cancel", "mark"),
        actual_line_plan(true, false, "cancel", "mark"),
        actual_line_plan(true, true, "other_cancel", "mark"),
    ] {
        assert_ne!(
            expected,
            row(&changed.0, &changed.1, 100_000, 1_000_000).0.unwrap()
        );
    }
}

#[test]
fn executable_line_row_reuses_its_proof_with_exact_shared_limits() {
    let (plan, owner) = actual_line_plan(true, true, "cancel", "mark");
    let context = RuntimeBodySemanticContext::new(&plan);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let semantic = context
        .line_semantic(
            &mut meter,
            group(),
            &owner,
            &mut |_| panic!("no task references"),
            crate::plan::RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let before = meter.totals();
    let expected = semantic
        .executable_row_digest(&context, &mut meter)
        .unwrap();
    let (work, bytes) = meter.totals();
    // Two owner tags and two digest atoms, regardless of the Line tree size.
    assert_eq!(work - before.0, 4);
    assert_eq!(
        bytes - before.1,
        b"arcweft.runtime-plan.executable-row.v1\0".len() as u64 + 66
    );
    assert_ne!(expected.as_bytes(), semantic.digest().as_bytes());
    assert_eq!(expected, row(&plan, &owner, work, bytes).0.unwrap());
    for (work, bytes, error) in [
        (work - 1, bytes, TaskSemanticEncodingError::SemanticWork),
        (work, bytes - 1, TaskSemanticEncodingError::TranscriptBytes),
    ] {
        assert!(matches!(
            row(&plan, &owner, work, bytes).0,
            Err(RuntimeBodySemanticError::Encoding(actual)) if actual == error
        ));
    }
}

#[test]
fn executable_line_proof_rejects_a_foreign_inventory_and_preserves_first_error() {
    let (plan, owner) = actual_line_plan(true, true, "cancel", "mark");
    let context = RuntimeBodySemanticContext::new(&plan);
    let mut preparation = TaskSemanticMeter::new(100_000, 1_000_000);
    let semantic = context
        .line_semantic(
            &mut preparation,
            group(),
            &owner,
            &mut |_| panic!("no task references"),
            crate::plan::RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let foreign = plan.clone();
    let foreign_context = RuntimeBodySemanticContext::new(&foreign);
    let mut rejected = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        semantic.executable_row_digest(&foreign_context, &mut rejected),
        Err(RuntimeBodySemanticError::ForeignLineTranscript)
    ));
    assert_eq!(rejected.totals(), (0, 0));
    assert_eq!(
        rejected.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
    let mut poisoned = TaskSemanticMeter::new(0, 1_000_000);
    poisoned.charge_work(1).unwrap_err();
    assert!(matches!(
        semantic.executable_row_digest(&foreign_context, &mut poisoned),
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert_eq!(poisoned.totals(), (0, 0));
}

#[test]
fn line_proof_requires_the_actual_row_before_charging_transcript_bytes() {
    let (plan, owner) = actual_line_plan(true, true, "cancel", "mark");
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let missing = crate::runtime_id::RuntimeLineTaskGroupId::from_zero_based(1).unwrap();
    assert!(matches!(
        RuntimeBodySemanticContext::new(&plan).line_semantic(
            &mut meter,
            missing,
            &owner,
            &mut |_| panic!("no task references"),
            crate::plan::RuntimeTaskPlanSealLimits::default(),
        ),
        Err(RuntimeBodySemanticError::MissingRow {
            table: "line task groups",
            ordinal: 1
        })
    ));
    assert_eq!(meter.totals(), (0, 0));
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}
