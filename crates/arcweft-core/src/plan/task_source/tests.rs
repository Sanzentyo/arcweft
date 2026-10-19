use super::*;
use crate::plan::body_semantic::flow::RuntimeBodyTaskSource;
use crate::plan::construction::source::{host_calls, selected_producers};
use crate::plan::{FlowOp, RuntimePlanBuilder};
use crate::step::RuntimeHostCallMode;
use crate::task::{HostRestartPolicy, TaskClass, TaskPolicy};

fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(u64::MAX, u64::MAX)
}

fn operation(plan: &RuntimePlan) -> &FlowOp {
    &plan
        .function_sites()
        .iter()
        .next()
        .unwrap()
        .body()
        .executable()
        .unwrap()
        .ops()[0]
}

fn host() -> RuntimePlan {
    host_calls::tests::fixture(RuntimeHostCallMode::Suspend, true, true, false)
}

fn coordinate(plan: &RuntimePlan) -> &RuntimeTaskPlanBuildCoordinate {
    let FlowOp::HostCall { target, .. } = operation(plan) else {
        unreachable!()
    };
    &target.producer.plan
}

#[test]
fn completed_task_source_borrows_the_actual_host_endpoint_and_issued_row() {
    for mode in [RuntimeHostCallMode::Immediate, RuntimeHostCallMode::Suspend] {
        let plan = host_calls::tests::fixture(mode, true, true, false);
        let FlowOp::HostCall { target, .. } = operation(&plan) else {
            unreachable!()
        };
        let source = plan
            .task_source(
                &target.producer.plan,
                RuntimeTaskPlanSealLimits::default(),
                &mut meter(),
            )
            .unwrap();
        assert_eq!(
            source.task_plan(),
            plan.task_plan(&target.producer.plan).unwrap()
        );
        let endpoint = source.request_endpoint();
        assert_eq!(
            endpoint.request_source(),
            source.task_plan().definition().request_template().source()
        );
        let Some(RuntimeBodyTaskSource::Host(actual)) = endpoint.task_source() else {
            panic!("the exact original host endpoint is required");
        };
        assert!(std::ptr::eq(actual, target));
        assert!(std::ptr::eq(actual.args.as_ptr(), target.args.as_ptr()));
    }
}

#[test]
fn completed_task_source_borrows_selected_request_metadata_from_the_same_original_function() {
    for request in 0..3 {
        let plan = selected_producers::tests::fixture(
            request,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::MustBeQuiescent,
            TaskClass::Io,
        );
        let FlowOp::StartNeedProducer { target, .. } = operation(&plan) else {
            unreachable!()
        };
        let source = plan
            .task_source(
                target.task_plan(),
                RuntimeTaskPlanSealLimits::default(),
                &mut meter(),
            )
            .unwrap();
        assert_eq!(
            source.task_plan(),
            plan.task_plan(target.task_plan()).unwrap()
        );
        let endpoint = source.request_endpoint();
        let Some(RuntimeBodyTaskSource::Start(actual)) = endpoint.task_source() else {
            panic!("the exact original selected producer is required");
        };
        assert!(std::ptr::eq(actual, target));
        assert!(std::ptr::eq(actual.plan(), target.plan()));
        assert_eq!(
            endpoint.request_source(),
            source.task_plan().definition().request_template().source()
        );
    }
}

#[test]
fn completed_task_source_rejects_equal_key_foreign_and_unsealed_coordinates() {
    let plan = host();
    let other = host();
    assert_eq!(
        plan.executable_semantic_digest(),
        other.executable_semantic_digest()
    );
    assert_eq!(
        plan.task_plan(coordinate(&plan)).unwrap().key(),
        other.task_plan(coordinate(&other)).unwrap().key()
    );
    assert!(
        RuntimeTaskPlanCoordinateOwner::from_completed(
            coordinate(&plan),
            other.task_plan(coordinate(&other)).unwrap()
        )
        .is_none()
    );
    let mut builder = RuntimePlanBuilder::new();
    let unsealed = builder.reserve_task_plan().unwrap();
    assert!(
        RuntimeTaskPlanCoordinateOwner::from_completed(
            &unsealed,
            plan.task_plan(coordinate(&plan)).unwrap()
        )
        .is_none()
    );
    for coordinate in [coordinate(&other), &unsealed] {
        let mut bank = meter();
        assert!(matches!(
            plan.task_source(coordinate, RuntimeTaskPlanSealLimits::default(), &mut bank),
            Err(RuntimeTaskSourceError::ForeignCoordinate)
        ));
        assert_eq!(bank.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    }
}

#[test]
fn completed_task_source_preserves_original_work_without_rehashing_the_transcript() {
    let plan = host();
    let coordinate = coordinate(&plan);
    let mut measured = meter();
    let source = plan
        .task_source(
            coordinate,
            RuntimeTaskPlanSealLimits::default(),
            &mut measured,
        )
        .unwrap();
    let expected = source.task_plan().key();
    let (work, bytes) = measured.totals();
    assert!(work > 0);
    assert_eq!(bytes, 0);
    let mut exact = TaskSemanticMeter::new(work, bytes);
    assert_eq!(
        plan.task_source(coordinate, RuntimeTaskPlanSealLimits::default(), &mut exact)
            .unwrap()
            .task_plan()
            .key(),
        expected
    );
    assert_eq!(exact.totals(), (work, bytes));
    let mut short = TaskSemanticMeter::new(work - 1, 0);
    for _ in 0..2 {
        assert!(matches!(
            plan.task_source(coordinate, RuntimeTaskPlanSealLimits::default(), &mut short),
            Err(RuntimeTaskSourceError::Function(
                RuntimeBodySemanticError::Encoding(TaskSemanticEncodingError::SemanticWork)
            ))
        ));
        assert_eq!(short.status(), Err(TaskSemanticEncodingError::SemanticWork));
    }
    let mut prefixed = meter();
    crate::task::semantic::TaskSemanticEncoder::new(
        b"arcweft.test.task-source-prefix.v1\0",
        &mut prefixed,
    )
    .finish()
    .unwrap();
    let prefix = prefixed.totals();
    assert!(prefix.1 > 0);
    plan.task_source(
        coordinate,
        RuntimeTaskPlanSealLimits::default(),
        &mut prefixed,
    )
    .unwrap();
    assert_eq!(prefixed.totals(), (prefix.0 + work, prefix.1));
    for _ in 0..2 {
        let error = plan
            .task_source(
                coordinate,
                RuntimeTaskPlanSealLimits {
                    max_transcript_bytes: prefix.1 - 1,
                    ..Default::default()
                },
                &mut prefixed,
            )
            .err()
            .unwrap();
        assert!(matches!(
            error,
            RuntimeTaskSourceError::Function(RuntimeBodySemanticError::Encoding(
                TaskSemanticEncodingError::TranscriptBytes
            ))
        ));
        assert_eq!(
            prefixed.status(),
            Err(TaskSemanticEncodingError::TranscriptBytes)
        );
    }
    let error = plan
        .task_source(
            coordinate,
            RuntimeTaskPlanSealLimits {
                max_semantic_work: 0,
                ..Default::default()
            },
            &mut meter(),
        )
        .err()
        .unwrap();
    assert!(matches!(
        error,
        RuntimeTaskSourceError::Function(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
}
