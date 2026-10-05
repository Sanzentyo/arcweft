use arcweft_core::pattern::RuntimeCheckedType;
use arcweft_core::task::{
    CancelScopeId, GenerationId, HostTaskRequest, NeedProducerContractDigest, NeedProducerFamily,
    NeedProducerInstance, NeedProducerSiteDigest, NeedProducerSpec, RuntimeTypeSemanticDigest,
    TaskAdmissionJournal, TaskClass, TaskEnsureError, TaskOutcomeContract, TaskPlanSemanticDigest,
    TaskPolicy, TaskPriority, TaskSpec,
};
use arcweft_core::value::RuntimeValue;

fn specification(policy: TaskPolicy) -> TaskSpec {
    let producer = NeedProducerSpec::new(
        NeedProducerFamily::HostAdapterTask,
        NeedProducerContractDigest::from_bytes([1; 32]),
        TaskPlanSemanticDigest::from_bytes([2; 32]),
        NeedProducerSiteDigest::from_bytes([3; 32]),
        RuntimeTypeSemanticDigest::from_bytes(
            *RuntimeCheckedType::Bool
                .semantic_identity_digest()
                .as_bytes(),
        ),
        RuntimeValue::Tuple(Vec::new()).try_digest(1024).unwrap(),
    );
    TaskSpec {
        generation: GenerationId::new(7),
        producer: NeedProducerInstance::try_from(&producer).unwrap(),
        class: TaskClass::Cpu,
        priority: TaskPriority(0),
        cancel_scope: CancelScopeId("fixture".into()),
        policy,
        outcome: TaskOutcomeContract::new(RuntimeCheckedType::Bool),
        request: HostTaskRequest::custom("fixture", "run", []),
        debug_label: "first diagnostic".into(),
    }
}

#[test]
fn join_reuses_the_complete_receipt_when_only_diagnostics_change() {
    let mut journal = TaskAdmissionJournal::default();
    let mut spec = specification(TaskPolicy::JoinSameKey);
    let first = journal.ensure_task(spec.clone()).unwrap();
    spec.debug_label = "renamed diagnostic".into();
    assert_eq!(journal.ensure_task(spec).unwrap(), first);
    assert_eq!(first.correlation.launch_ordinal.get(), 0);
    assert_eq!(journal.submission(first).unwrap().handle(), first);
}

#[test]
fn conflicting_join_does_not_replace_accepted_work() {
    let mut journal = TaskAdmissionJournal::default();
    let spec = specification(TaskPolicy::JoinSameKey);
    let accepted = journal.ensure_task(spec.clone()).unwrap();
    let mut conflict = spec;
    conflict.request = HostTaskRequest::custom("fixture", "different_operation", []);
    assert!(matches!(
        journal.ensure_task(conflict),
        Err(TaskEnsureError::JoinSpecificationConflict { .. })
    ));
    assert_eq!(
        journal.submission(accepted).unwrap().spec().request,
        HostTaskRequest::custom("fixture", "run", [])
    );
}

#[test]
fn rejected_outcome_does_not_consume_the_next_launch_ordinal() {
    let mut journal = TaskAdmissionJournal::default();
    let spec = specification(TaskPolicy::AlwaysStart);
    let mut invalid = spec.clone();
    invalid.outcome = TaskOutcomeContract::new(RuntimeCheckedType::String);
    assert_eq!(
        journal.ensure_task(invalid),
        Err(TaskEnsureError::OutcomeContractMismatch)
    );
    let first = journal.ensure_task(spec.clone()).unwrap();
    let second = journal.ensure_task(spec).unwrap();
    assert_eq!(first.correlation.launch_ordinal.get(), 1);
    assert_eq!(second.correlation.launch_ordinal.get(), 2);
    assert_ne!(first.correlation.need, second.correlation.need);
    assert_ne!(first.correlation.task_id, second.correlation.task_id);
    assert_eq!(first.correlation.task_key, second.correlation.task_key);
}

#[test]
fn generation_changes_task_identity_while_need_remains_generation_independent() {
    let mut journal = TaskAdmissionJournal::default();
    let mut spec = specification(TaskPolicy::JoinSameKey);
    let first = journal.ensure_task(spec.clone()).unwrap();
    spec.generation = GenerationId::new(8);
    let second = journal.ensure_task(spec).unwrap();
    assert_eq!(first.correlation.need, second.correlation.need);
    assert_ne!(first.correlation.task_key, second.correlation.task_key);
    assert_ne!(first.correlation.task_id, second.correlation.task_id);
}

#[test]
fn need_resolution_keeps_equal_need_ids_in_different_generations_separate() {
    use arcweft_core::task::{
        LogicalEpoch, RuntimeNeedOutcome, RuntimeNeedState, TaskPublicationCursor, TaskSequence,
        resolved_runtime_need_state,
    };
    let mut journal = TaskAdmissionJournal::default();
    let mut spec = specification(TaskPolicy::JoinSameKey);
    let first = journal.ensure_task(spec.clone()).unwrap().correlation;
    spec.generation = GenerationId::new(8);
    let second = journal.ensure_task(spec).unwrap().correlation;
    let cursor = TaskPublicationCursor {
        logical_epoch: LogicalEpoch(1),
        sequence: TaskSequence(1),
    };
    let states = [
        RuntimeNeedState::new(
            first,
            Some(cursor),
            arcweft_need::Need::Ready(RuntimeNeedOutcome::Value(RuntimeValue::Bool(false).into())),
        ),
        RuntimeNeedState::new(
            second,
            Some(cursor),
            arcweft_need::Need::Ready(RuntimeNeedOutcome::Value(RuntimeValue::Bool(true).into())),
        ),
    ];
    assert_eq!(first.need, second.need);
    assert_eq!(
        resolved_runtime_need_state(&states, &first),
        Some(&states[0])
    );
    assert_eq!(
        resolved_runtime_need_state(&states, &second),
        Some(&states[1])
    );
    let mut forged = first;
    forged.producer_contract = NeedProducerContractDigest::from_bytes([9; 32]);
    assert!(resolved_runtime_need_state(&states, &forged).is_none());
}

#[test]
fn publication_position_is_independent_of_the_physical_dispatch_attempt() {
    use arcweft_core::task::{
        LogicalEpoch, TaskDispatchIdentity, TaskEvent, TaskEventKind, TaskPublicationRevision,
        TaskSequence,
    };
    let mut journal = TaskAdmissionJournal::default();
    let correlation = journal
        .ensure_task(specification(TaskPolicy::AlwaysStart))
        .unwrap()
        .correlation;
    let event = TaskEvent::from_dispatch(
        TaskDispatchIdentity::new(correlation, LogicalEpoch(11), TaskSequence(73)),
        TaskPublicationRevision::FIRST,
        TaskEventKind::Cancelled,
    );
    assert_eq!(event.correlation, correlation);
    assert_eq!(event.cursor.logical_epoch, LogicalEpoch(11));
    assert_eq!(event.cursor.sequence, TaskSequence(1));
    assert_ne!(event.cursor.sequence, TaskSequence(73));
}

#[test]
fn infrastructure_diagnostics_bound_utf8_and_reject_oversized_saved_input() {
    use arcweft_core::task::{
        BoundedRuntimeDiagnostic, RuntimeTaskFailure, RuntimeTaskFailureKind,
    };
    let message = format!("{}界", "a".repeat(BoundedRuntimeDiagnostic::MAX_BYTES - 1));
    let failure = RuntimeTaskFailure::new(RuntimeTaskFailureKind::WorkerFailure, message.clone());
    assert_eq!(
        failure.diagnostic.as_str(),
        "a".repeat(BoundedRuntimeDiagnostic::MAX_BYTES - 1)
    );
    assert_eq!(
        serde_json::from_str::<RuntimeTaskFailure>(&serde_json::to_string(&failure).unwrap())
            .unwrap(),
        failure
    );
    assert!(
        serde_json::from_str::<BoundedRuntimeDiagnostic>(&serde_json::to_string(&message).unwrap())
            .is_err()
    );
    assert!(
        serde_json::from_str::<RuntimeTaskFailure>(
            r#"{"kind":"WorkerFailure","diagnostic":"bounded","legacy_fault":"ignored"}"#,
        )
        .is_err()
    );
}
