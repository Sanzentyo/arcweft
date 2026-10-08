use super::*;

fn digest(
    plan: &RuntimePlan,
    work: u64,
    bytes: u64,
) -> (Result<blake3::Hash, RuntimeBodySemanticError>, (u64, u64)) {
    let mut meter = TaskSemanticMeter::new(work, bytes);
    let result = RuntimeBodySemanticContext::new(plan).stream_row_digest(&mut meter, 0);
    (result, meter.totals())
}

#[test]
fn executable_stream_row_commits_operation_order_and_empty_arm_role() {
    let first = stream_plan(false, true);
    let ordered = stream_plan(true, true);
    let other_arm = stream_plan(false, false);
    let expected = digest(&first, 10_000, 100_000).0.unwrap();
    assert_ne!(expected, digest(&ordered, 10_000, 100_000).0.unwrap());
    assert_ne!(expected, digest(&other_arm, 10_000, 100_000).0.unwrap());
}

#[test]
fn streams_enter_the_complete_source_ordered_executable_prefix() {
    use crate::plan::body_semantic::executable_rows::fixture_prefix;
    let first = fixture_prefix(&stream_plan(false, true));
    assert_ne!(first, fixture_prefix(&stream_plan(true, true)));
    assert_ne!(first, fixture_prefix(&stream_plan(false, false)));
}

#[test]
fn executable_stream_row_commits_actual_runtime_identity_and_error_type() {
    let first = stream_plan(false, true);
    let expected = digest(&first, 10_000, 100_000).0.unwrap();
    for change_identity in [false, true] {
        let mut changed = first.clone();
        let row = &first.stream_plans()[0];
        let id = if change_identity {
            crate::stream::StreamRuntimeId::canonical("other_identity").unwrap()
        } else {
            row.id().clone()
        };
        let error = if change_identity {
            row.error_ty()
        } else {
            row.item_ty()
        };
        changed.inventory.stream_plans[0] = crate::stream::StreamPlan::from_admitted_parts(
            id,
            row.item_ty(),
            error,
            row.ops().to_vec(),
        );
        changed.verify().unwrap();
        assert_ne!(expected, digest(&changed, 10_000, 100_000).0.unwrap());
    }
}

#[test]
fn executable_stream_row_has_exact_shared_limits_and_first_error_precedence() {
    let plan = stream_plan(false, true);
    let (expected, (work, bytes)) = digest(&plan, 10_000, 100_000);
    assert_eq!(expected.unwrap(), digest(&plan, work, bytes).0.unwrap());
    assert!(matches!(
        digest(&plan, work - 1, bytes).0,
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert!(matches!(
        digest(&plan, work, bytes - 1).0,
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::TranscriptBytes
        ))
    ));
    let context = RuntimeBodySemanticContext::new(&plan);
    let mut meter = TaskSemanticMeter::new(0, 100_000);
    meter.charge_work(1).unwrap_err();
    assert!(matches!(
        context.stream_row_digest(&mut meter, 99),
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert_eq!(meter.totals(), (0, 0));
}

#[test]
fn executable_stream_row_resolves_only_its_actual_inventory_row() {
    let plan = stream_plan(false, true);
    let context = RuntimeBodySemanticContext::new(&plan);
    let mut meter = TaskSemanticMeter::new(10_000, 100_000);
    assert!(matches!(
        context.stream_row_digest(&mut meter, 1),
        Err(RuntimeBodySemanticError::MissingRow {
            table: "stream plans",
            ordinal: 1
        })
    ));
    assert_eq!(meter.totals(), (0, 0));
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
    let empty = RuntimePlanBuilder::new().finish().unwrap();
    let mut meter = TaskSemanticMeter::new(10_000, 100_000);
    assert!(matches!(
        RuntimeBodySemanticContext::new(&empty).stream_row_digest(&mut meter, 0),
        Err(RuntimeBodySemanticError::MissingRow {
            table: "stream plans",
            ordinal: 0
        })
    ));
    assert_eq!(meter.totals(), (0, 0));
}
