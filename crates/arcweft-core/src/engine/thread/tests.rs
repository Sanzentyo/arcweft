use super::*;
use crate::engine::{FlowExit, FlowFiberStatus};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::*;
use crate::step::{RuntimeStepInput, RuntimeStepOptions};
use crate::task::*;
use crate::value::RuntimeValue;

fn engine(explicit_return: bool) -> Engine {
    engine_with_carrier(explicit_return, false)
}

fn engine_with_carrier(explicit_return: bool, ordinary_need: bool) -> Engine {
    let mut builder = RuntimePlanBuilder::new();
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let boolean = crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest();
    let thread = RuntimeSemanticTypeId::from_bytes([0xde; 32]);
    let need = RuntimeSemanticTypeId::from_bytes([0xdf; 32]);
    let handle = if ordinary_need { need } else { thread };
    let locals = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(thread, RuntimePlanTypeProjection::ThreadHandle(boolean)),
                RuntimePlanTypeSeed::new(need, RuntimePlanTypeProjection::Need(boolean)),
                RuntimePlanTypeSeed::new(
                    RuntimeSemanticTypeId::from_bytes([0xe0; 32]),
                    RuntimePlanTypeProjection::ThreadHandle(unit),
                ),
                RuntimePlanTypeSeed::new(
                    RuntimeSemanticTypeId::from_bytes([0xe1; 32]),
                    RuntimePlanTypeProjection::Need(unit),
                ),
            ],
            [RuntimeLocalDeclarationSeed::new(
                RuntimeLocalDeclarationSource::Binding {
                    identity: [0xd9; 32],
                    declaration: RuntimeLocalBindingDeclaration::new(
                        RuntimeLocalBindingKind::PatternBinding,
                        false,
                        RuntimeLocalBindingStorage::Derived,
                    ),
                },
                handle,
            )],
        )
        .unwrap();
    let result = RuntimeExprSeed::new(
        boolean,
        RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
    );
    let definition = RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xda; 32]);
    let function = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition,
            role: RuntimeFunctionSemanticRole::Ordinary,
            function_type: None,
            inputs: Box::new([]),
            result: boolean,
            body_kind: RuntimeFunctionSiteBodyKind::Executable,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    builder
        .define_function_site_seed(
            &function,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(result.clone())]),
            }),
        )
        .unwrap();
    let coordinate = builder
        .task_plan_construction()
        .reserve_original_source()
        .unwrap();
    let flow = FlowRuntimeId::canonical("program.result.local").unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: vec![],
        })
        .unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xdb; 32]),
                None,
                Box::new([]),
                unit,
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([
                    RuntimeFlowOpSeed::ProgramTask {
                        name: None,
                        binding: RuntimePatternSeed::new(
                            handle,
                            RuntimePatternSeedKind::Bind {
                                local: locals.local_ids()[0].clone(),
                                mutable: false,
                            },
                        ),
                        captures: vec![],
                        producer: NeedProducerDefinition {
                            function: definition,
                            family: NeedProducerFamily::StructuredTaskPlan,
                            contract: NeedProducerContractDigest::from_bytes([0xdc; 32]),
                            plan: coordinate,
                            producer_site: NeedProducerSiteDigest::from_bytes([0xdd; 32]),
                            handle_type: if ordinary_need {
                                crate::task::TaskHandleType::Need {
                                    payload: RuntimeTypeSemanticDigest::from_bytes(
                                        *boolean.as_bytes(),
                                    ),
                                }
                            } else {
                                crate::task::TaskHandleType::ThreadHandle {
                                    payload: RuntimeTypeSemanticDigest::from_bytes(
                                        *boolean.as_bytes(),
                                    ),
                                }
                            },
                            class: TaskClass::Cpu,
                            priority: TaskPriority(0),
                            cancel_scope: CancelScopeId("program.result".into()),
                            policy: TaskPolicy::AlwaysStart,
                            debug_label: "local Bool result".into(),
                        },
                        body: if explicit_return {
                            vec![RuntimeFlowOpSeed::ReturnExpr(result)]
                        } else {
                            vec![]
                        },
                    },
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        unit,
                        RuntimeExprSeedKind::Value(RuntimeValue::Unit),
                    )),
                ]),
            },
        ))
        .unwrap();
    Engine::for_flow(builder.finish().unwrap(), &flow).unwrap()
}

#[test]
fn thread_handle_admission_requires_program_custody_and_its_exact_payload() {
    let mut engine = engine(true);
    let mut receipt = None;
    for _ in 0..64 {
        let step = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(step.stats.executed_ops <= 1);
        assert!(step.output.diagnostics.is_empty());
        if let Some(task) = step.output.requests.tasks.into_iter().next() {
            receipt = Some(task);
            break;
        }
    }
    let program = RuntimeValue::NeedHandle(
        RuntimeNeedHandle::try_from(receipt.expect("actual Program task receipt")).unwrap(),
    );
    let ty = |marker| {
        engine
            .plan
            .type_table()
            .id_for_semantic(RuntimeSemanticTypeId::from_bytes([marker; 32]))
            .unwrap()
    };
    let limits = crate::entry::RuntimeSchemaLimits::engine_default();
    assert!(
        engine
            .plan
            .validate_live_value(ty(0xde), &program, limits)
            .is_ok()
    );
    assert!(
        engine
            .plan
            .validate_live_value(ty(0xdf), &program, limits)
            .is_err()
    );
    assert!(
        engine
            .plan
            .validate_live_value(ty(0xe0), &program, limits)
            .is_err()
    );

    let reusable = crate::tests::reusable_need_with_outcome(
        "thread-handle-kind",
        TaskOutcomeContract::new(crate::pattern::RuntimeCheckedType::Unit),
    );
    let mut journal = TaskAdmissionJournal::default();
    let accepted = journal
        .ensure_task(reusable.reusable_spec().unwrap().clone())
        .unwrap();
    let host = RuntimeNeedHandle::try_from(journal.submission(accepted).unwrap()).unwrap();
    assert!(host.requires_producer_custody());
    assert_eq!(
        host.outcome().payload_semantic_identity().as_bytes(),
        crate::pattern::RuntimeCheckedType::Unit
            .semantic_identity_digest()
            .as_bytes(),
    );
    let host = RuntimeValue::NeedHandle(host);
    assert!(
        engine
            .plan
            .validate_live_value(ty(0xe1), &host, limits)
            .is_ok()
    );
    assert!(
        engine
            .plan
            .validate_live_value(ty(0xe0), &host, limits)
            .is_err()
    );
}

fn retire(
    engine: &mut Engine,
    rollback_each_step: bool,
) -> (TaskSubmission, ProgramTaskCompletion) {
    let (mut tasks, mut reports) = retire_all(engine, rollback_each_step);
    assert_eq!(tasks.len(), 1);
    assert_eq!(reports.len(), 1);
    (tasks.pop().unwrap(), reports.pop().unwrap())
}

fn retire_all(
    engine: &mut Engine,
    rollback_each_step: bool,
) -> (Vec<TaskSubmission>, Vec<ProgramTaskCompletion>) {
    let mut tasks = Vec::new();
    let mut reports = Vec::new();
    for _ in 0..64 {
        let step = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(step.stats.executed_ops <= 1);
        assert!(step.output.diagnostics.is_empty(), "{:?}", step.output);
        tasks.extend(step.output.requests.tasks);
        reports.extend(step.output.requests.program_task_completions);
        if rollback_each_step {
            let owner = RuntimeProgramOwner::Plan(Arc::clone(&engine.plan));
            *engine = Engine::from_rollback_image(engine.inert_rollback_image().unwrap(), &owner)
                .unwrap();
        }
        if matches!(engine.fiber().frame.status, FlowFiberStatus::Done(_)) {
            return (tasks, reports);
        }
    }
    panic!("the actual Program body failed to retire without a Host");
}

#[test]
fn ordinary_program_need_retains_source_custody_and_moves_its_result_once() {
    let mut engine = engine_with_carrier(true, true);
    let (task, report) = retire(&mut engine, true);
    assert_eq!(report.terminal(), &ProgramTaskTerminal::Completed);
    let handle = RuntimeNeedHandle::try_from(task.clone()).unwrap();
    assert!(matches!(
        handle.spec().producer.handle_type(),
        crate::task::TaskHandleType::Need { .. }
    ));
    let value = RuntimeValue::NeedHandle(handle);
    let lookup = |marker| {
        engine
            .plan
            .type_table()
            .id_for_semantic(RuntimeSemanticTypeId::from_bytes([marker; 32]))
            .unwrap()
    };
    assert!(
        engine
            .plan
            .validate_live_value(lookup(0xdf), &value, RuntimeSchemaLimits::engine_default())
            .is_ok()
    );
    assert!(
        engine
            .plan
            .validate_live_value(lookup(0xde), &value, RuntimeSchemaLimits::engine_default())
            .is_err()
    );
    let correlation = task.handle().correlation;
    assert_eq!(
        engine
            .need_producers
            .take_ready_for_correlation(&correlation)
            .unwrap()
            .into_value(),
        RuntimeValue::Bool(true)
    );
    assert!(
        engine
            .need_producers
            .take_ready_for_correlation(&correlation)
            .is_err()
    );
}

#[test]
fn ordinary_program_need_moves_an_affine_capture_and_retains_it_through_rollback() {
    let (plan, flow, _) = crate::plan::task_source::owned_program_need_fixture(
        crate::value::RuntimeLocalReadMode::Move,
    );
    let mut engine = Engine::for_flow(plan, &flow).unwrap();
    let (tasks, reports) = retire_all(&mut engine, true);
    assert_eq!(tasks.len(), 2);
    assert_eq!(reports.len(), 2);
    assert!(
        reports
            .iter()
            .all(|report| report.terminal() == &ProgramTaskTerminal::Completed)
    );
    let result = engine
        .need_producers
        .take_ready_for_correlation(&tasks[1].handle().correlation)
        .unwrap()
        .into_value();
    assert!(
        matches!(result, RuntimeValue::NeedHandle(handle) if handle.correlation() == tasks[0].handle().correlation)
    );
    assert!(
        engine
            .need_producers
            .take_ready_for_correlation(&tasks[1].handle().correlation)
            .is_err()
    );
    assert_eq!(
        engine
            .need_producers
            .take_ready_for_correlation(&tasks[0].handle().correlation)
            .unwrap()
            .into_value(),
        RuntimeValue::String("owned result".into())
    );
}

#[test]
fn program_non_unit_result_survives_each_rollback_and_ack_cannot_republish_it() {
    let mut engine = engine(true);
    let (task, report) = retire(&mut engine, true);
    assert!(matches!(
        engine.fiber().frame.status,
        FlowFiberStatus::Done(FlowExit::Return(_))
    ));
    assert_eq!(report.terminal(), &ProgramTaskTerminal::Completed);
    let correlation = task.handle().correlation;
    assert_eq!(
        engine
            .need_producers
            .launch_for_correlation(&correlation)
            .unwrap()
            .need_publication_cursor(),
        Some(crate::task::RuntimeNeedPublicationCursor::Program(
            crate::task::ProgramTaskValuePhase::Terminal
        )),
    );
    assert_eq!(
        engine
            .need_producers
            .ready_for_correlation(&correlation)
            .unwrap()
            .value(),
        &RuntimeValue::Bool(true)
    );
    assert!(
        engine
            .need_producers
            .launch_for_correlation(&correlation)
            .unwrap()
            .publication()
            .is_none()
    );
    let dispatch = TaskDispatchIdentity::new(correlation, LogicalEpoch(1), TaskSequence(1));
    let bound = BoundTaskSpec::bind(
        task.clone(),
        Some(RuntimeProgramOwner::Plan(Arc::clone(&engine.plan))),
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    for kind in [
        TaskEventKind::Ready(RuntimeValue::Bool(false).into()),
        TaskEventKind::Cancelled,
        TaskEventKind::InfrastructureFailure(RuntimeTaskFailure::new(
            RuntimeTaskFailureKind::WorkerFailure,
            "unproved Host failure",
        )),
        TaskEventKind::Progress(Progress::new(0.5).unwrap()),
        TaskEventKind::ProgramTerminal(ProgramTaskTerminal::Completed),
    ] {
        let packet =
            TaskEvent::from_dispatch(dispatch.clone(), TaskPublicationRevision::FIRST, kind);
        assert!(
            matches!(packet.validate_launch(&bound), Err(TaskCompletionError::DispatchMismatch { task_id }) if task_id == task.task_id())
        );
    }
    let forged = TaskEvent::from_dispatch(
        dispatch.clone(),
        TaskPublicationRevision::FIRST,
        TaskEventKind::Ready(RuntimeValue::Bool(false).into()),
    );
    let rejected = engine
        .need_producers
        .publish_task_event_owned(forged)
        .unwrap_err();
    let (reason, event) = rejected.into_parts();
    assert_eq!(reason, NeedProducerAdmissionError::InvalidPublicationSource);
    assert!(
        matches!(event.kind, TaskEventKind::Ready(value) if value.value() == &RuntimeValue::Bool(false))
    );
    assert_eq!(
        engine
            .need_producers
            .take_ready_for_correlation(&correlation)
            .unwrap()
            .into_value(),
        RuntimeValue::Bool(true)
    );
    assert!(
        engine
            .need_producers
            .take_ready_for_correlation(&correlation)
            .is_err()
    );
    let transferred = Some(crate::task::RuntimeNeedPublicationCursor::Program(
        crate::task::ProgramTaskValuePhase::Transferred,
    ));
    assert_eq!(
        engine
            .need_producers
            .launch_for_correlation(&correlation)
            .unwrap()
            .need_publication_cursor(),
        transferred
    );
    let event = report
        .bind_publication(&task, dispatch, TaskPublicationRevision::FIRST)
        .unwrap()
        .into_event();
    assert!(
        engine
            .need_producers
            .publication_for_task_event(&event)
            .is_none()
    );
    assert!(matches!(
        engine
            .need_producers
            .publish_task_event_owned(event)
            .unwrap(),
        NeedProducerOwnedTaskEventDisposition::Published
    ));
    assert!(matches!(
        engine
            .need_producers
            .launch_for_correlation(&correlation)
            .unwrap()
            .state(),
        RuntimeNeedProducerState::ReadyTransferred
    ));
    assert_eq!(
        engine
            .need_producers
            .launch_for_correlation(&correlation)
            .unwrap()
            .need_publication_cursor(),
        transferred
    );
}

#[test]
fn program_resume_replays_only_unacknowledged_retirement_metadata_once() {
    let mut engine = engine(true);
    let (task, _) = retire(&mut engine, false);
    let owner = RuntimeProgramOwner::Plan(Arc::clone(&engine.plan));
    let mut admission = RuntimeRetainedValueAdmission::for_program(
        owner.clone(),
        RuntimeSchemaLimits::engine_default(),
    );
    let saved = engine
        .need_producers
        .snapshot_retained(&owner, &mut admission)
        .unwrap();
    let mut resumed = NeedProducerRegistry::default();
    resumed
        .restore_registry(saved.into_admitted_live(&owner).unwrap(), &mut admission)
        .unwrap();
    let replay = resumed.take_program_retirement_replays();
    assert_eq!(replay.len(), 1);
    assert_eq!(replay[0].handle(), task.handle());
    assert_eq!(replay[0].terminal(), &ProgramTaskTerminal::Completed);
    assert!(resumed.take_program_retirement_replays().is_empty());
    assert_eq!(
        resumed
            .take_ready_for_correlation(&task.handle().correlation)
            .unwrap()
            .into_value(),
        RuntimeValue::Bool(true)
    );
    let event = replay
        .into_iter()
        .next()
        .unwrap()
        .bind_publication(
            &task,
            TaskDispatchIdentity::new(task.handle().correlation, LogicalEpoch(2), TaskSequence(1)),
            TaskPublicationRevision::FIRST,
        )
        .unwrap()
        .into_event();
    resumed.publish_task_event_owned(event).unwrap();
    let saved = resumed.snapshot_retained(&owner, &mut admission).unwrap();
    let mut acknowledged = NeedProducerRegistry::default();
    acknowledged
        .restore_registry(saved.into_admitted_live(&owner).unwrap(), &mut admission)
        .unwrap();
    assert!(acknowledged.take_program_retirement_replays().is_empty());
    assert!(matches!(
        acknowledged
            .launch_for_correlation(&task.handle().correlation)
            .unwrap()
            .state(),
        RuntimeNeedProducerState::ReadyTransferred
    ));
}

#[test]
fn non_unit_program_fallthrough_fails_instead_of_publishing_completed_unit() {
    let mut engine = engine(false);
    let mut reports = Vec::new();
    for _ in 0..64 {
        let step = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(step.stats.executed_ops <= 1);
        reports.extend(step.output.requests.program_task_completions);
        if !reports.is_empty() {
            assert_eq!(reports.len(), 1);
            let ProgramTaskTerminal::Failed(failure) = reports[0].terminal() else {
                panic!(
                    "a Bool fallthrough cannot publish successful Unit: {:?}",
                    reports[0]
                );
            };
            assert_eq!(failure.kind, RuntimeTaskFailureKind::WorkerFailure);
            let launch = engine
                .need_producers
                .launch_for_correlation(&reports[0].handle().correlation)
                .unwrap();
            assert!(launch.task_terminal());
            assert!(launch.publication().is_none());
            assert!(
                matches!(launch.state(), RuntimeNeedProducerState::InfrastructureFailure(actual) if actual == failure)
            );
            assert!(
                engine
                    .need_producers
                    .ready_for_correlation(&reports[0].handle().correlation)
                    .is_none()
            );
            return;
        }
        assert!(
            !matches!(engine.fiber().frame.status, FlowFiberStatus::Failed(_)),
            "{:?}",
            step.output.diagnostics
        );
    }
    panic!("a Bool Program's implicit Unit fallthrough must fail");
}
