use std::sync::Arc;

use super::{BoundTaskSpec, RuntimeProgramOwner, TaskOutcomeBindingError, TaskOutcomeValueError};
use crate::{
    awbc::schema::{
        AwbcProgram, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcStringId, AwbcTypeId,
        AwbcVariantCase, AwbcVariantIdentity,
    },
    entry::RuntimeSchemaLimits,
    pattern::{
        RuntimeBuiltinVariantIdentity, RuntimeCheckedType, RuntimeOpaqueTypeAdmission,
        RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId,
    },
    plan::{RuntimePlan, RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed},
    task::{
        CancelScopeId, FileReadTextRequest, HostTaskRequest, TaskClass, TaskId, TaskKey,
        TaskOutcomeContract, TaskPolicy, TaskPriority, TaskSpec,
    },
    value::{RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeValue},
};

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}

#[test]
fn borrowed_standalone_choice_retains_work_failure_before_a_later_match() {
    use crate::entry::RuntimeSchemaError;
    use crate::entry::schema::value_budget::ValidationWork;
    let expected =
        RuntimeCheckedType::Choice(vec![RuntimeCheckedType::String, RuntimeCheckedType::Bool]);
    let value = crate::value::AwbcRuntimeValueSnapshot::Bool(true);
    let small = ValidationWork::new(RuntimeSchemaLimits {
        max_validation_work: 2,
        ..RuntimeSchemaLimits::engine_default()
    });
    let failure = expected
        .accepts_admitted_value_view(value.view(), &small)
        .unwrap_err();
    assert!(matches!(
        failure,
        RuntimeSchemaError::ValidationWork {
            limit: 2,
            consumed: 3,
            ..
        }
    ));
    assert_eq!(
        RuntimeCheckedType::Bool
            .accepts_admitted_value_view(value.view(), &small)
            .unwrap_err(),
        failure
    );
    let exact = ValidationWork::new(RuntimeSchemaLimits {
        max_validation_work: 3,
        ..RuntimeSchemaLimits::engine_default()
    });
    assert_eq!(
        expected.accepts_admitted_value_view(value.view(), &exact),
        Ok(true)
    );
    assert!(expected.accepts_value(&RuntimeValue::Bool(true)));
}

#[test]
fn borrowed_outcome_uses_the_original_lease_even_for_equal_scalar_rows() {
    use crate::entry::schema::value_encoding::{ValueAdmissionOrigin, ValueResourceAdmission};
    let original = RuntimeProgramOwner::Plan(Arc::new(result_plan()));
    let other = RuntimeProgramOwner::Plan(Arc::new(result_plan()));
    let bound = TaskOutcomeContract::program(semantic(1))
        .bind_program(original.clone(), RuntimeSchemaLimits::engine_default())
        .unwrap();
    let value = crate::value::AwbcRuntimeValueSnapshot::Bool(true);
    let mut resources = ValueResourceAdmission::snapshot(
        RuntimeSchemaLimits::engine_default(),
        ValueAdmissionOrigin::InertSnapshot,
    );
    resources.validate_view(value.view(), 0, None).unwrap();
    assert!(
        matches!(bound.validate_admitted_value_view(value.view(), Some(&other), &mut resources),
        Err(TaskOutcomeValueError::WrongProgramOwner { semantic_type }) if semantic_type == semantic(1))
    );
    bound
        .validate_admitted_value_view(value.view(), Some(&original), &mut resources)
        .unwrap();
    assert_eq!(value, crate::value::AwbcRuntimeValueSnapshot::Bool(true));
}

#[test]
fn queued_inert_outcomes_share_nodes_and_keep_the_first_error() {
    use crate::entry::RuntimeSchemaError;
    use crate::task::{TaskEvent, TaskEventKind, TaskPublicationCursor};
    use crate::value::{
        AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission,
    };
    let outcome = TaskOutcomeContract::new(RuntimeCheckedType::String);
    let correlation = crate::tests::reusable_need("queued outcomes").correlation();
    let events: Vec<TaskEvent<AwbcRuntimeValueSnapshot>> = ["first", "second"]
        .into_iter()
        .map(|value| TaskEvent {
            correlation,
            cursor: TaskPublicationCursor {
                logical_epoch: crate::task::LogicalEpoch(1),
                sequence: crate::task::TaskSequence(1),
            },
            kind: TaskEventKind::Ready(AwbcRuntimeValueSnapshot::String(value.into())),
        })
        .collect();
    let bytes = serde_json::to_vec(&events).unwrap();
    let mut limited = RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits {
        max_nodes: 3,
        ..RuntimeSchemaLimits::engine_default()
    });
    events[0]
        .admit_retained(Some(&outcome), &mut limited)
        .unwrap();
    let first = events[1]
        .admit_retained(Some(&outcome), &mut limited)
        .unwrap_err();
    assert!(matches!(
        first,
        AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded {
            budget: "nodes"
        })
    ));
    assert_eq!(
        events[0]
            .admit_retained(Some(&outcome), &mut limited)
            .unwrap_err(),
        first
    );
    assert_eq!(serde_json::to_vec(&events).unwrap(), bytes);
    let mut exact = RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits {
        max_nodes: 4,
        ..RuntimeSchemaLimits::engine_default()
    });
    for event in &events {
        event.admit_retained(Some(&outcome), &mut exact).unwrap();
    }
}

#[test]
fn retained_need_ready_states_share_one_sticky_outcome_allowance() {
    use crate::entry::RuntimeSchemaError;
    use crate::task::{RuntimeNeedProducerState, TaskPolicy};
    use crate::value::{
        AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission,
    };
    let outcome = TaskOutcomeContract::new(RuntimeCheckedType::Bool);
    let states = [
        RuntimeNeedProducerState::Ready(AwbcRuntimeValueSnapshot::Bool(true)),
        RuntimeNeedProducerState::Ready(AwbcRuntimeValueSnapshot::Bool(false)),
    ];
    let before = serde_json::to_vec(&states).unwrap();
    let mut limited = RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits {
        max_nodes: 3,
        ..RuntimeSchemaLimits::engine_default()
    });
    states[0]
        .admit_retained(&outcome, TaskPolicy::JoinSameKey, &mut limited)
        .unwrap();
    let error = states[1]
        .admit_retained(&outcome, TaskPolicy::AlwaysStart, &mut limited)
        .unwrap_err();
    assert!(matches!(
        error,
        AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded {
            budget: "nodes"
        })
    ));
    assert_eq!(
        states[0]
            .admit_retained(&outcome, TaskPolicy::JoinSameKey, &mut limited)
            .unwrap_err(),
        error
    );
    assert_eq!(serde_json::to_vec(&states).unwrap(), before);
    let mut exact = RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits {
        max_nodes: 4,
        ..RuntimeSchemaLimits::engine_default()
    });
    for state in &states {
        state
            .admit_retained(&outcome, TaskPolicy::JoinSameKey, &mut exact)
            .unwrap();
    }
}

fn result_plan() -> RuntimePlan {
    let producer = RuntimeOpaqueTypeProducerId::try_new("fixture.task-error").unwrap();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(semantic(1), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    semantic(2),
                    RuntimePlanTypeProjection::Opaque {
                        producer,
                        admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                        value_class: RuntimeOpaqueValueClass::Plain,
                        persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    semantic(3),
                    RuntimePlanTypeProjection::Tuple(Box::new([semantic(1)])),
                ),
                RuntimePlanTypeSeed::new(
                    semantic(4),
                    RuntimePlanTypeProjection::Tuple(Box::new([semantic(2)])),
                ),
                RuntimePlanTypeSeed::new(
                    semantic(5),
                    RuntimePlanTypeProjection::Result {
                        value: semantic(1),
                        error: semantic(2),
                        value_payload: semantic(3),
                        error_payload: semantic(4),
                    },
                ),
            ],
            [],
        )
        .unwrap();
    builder.finish().unwrap()
}

fn result_awbc() -> AwbcProgram {
    AwbcProgram {
        strings: vec![
            "Ok".to_owned(),
            "Err".to_owned(),
            "fixture.task-error".to_owned(),
        ],
        runtime_types: vec![
            AwbcRuntimeType::new(semantic(1), AwbcRuntimeTypeShape::Bool),
            AwbcRuntimeType::new(
                semantic(2),
                AwbcRuntimeTypeShape::Opaque {
                    producer: AwbcStringId(2),
                    admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                    value_class: RuntimeOpaqueValueClass::Plain,
                    persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
                    arguments: vec![],
                },
            ),
            AwbcRuntimeType::new(
                semantic(3),
                AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0)]),
            ),
            AwbcRuntimeType::new(
                semantic(4),
                AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(1)]),
            ),
            AwbcRuntimeType::new(
                semantic(5),
                AwbcRuntimeTypeShape::Variant {
                    owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
                    arguments: vec![],
                    cases: vec![
                        AwbcVariantCase {
                            name: AwbcStringId(0),
                            payload: Some(AwbcTypeId(2)),
                        },
                        AwbcVariantCase {
                            name: AwbcStringId(1),
                            payload: Some(AwbcTypeId(3)),
                        },
                    ],
                },
            ),
        ],
        ..AwbcProgram::default()
    }
}

fn opaque_error() -> RuntimeValue {
    RuntimeOpaqueTypeOwner::exact(
        RuntimeOpaqueTypeProducerId::try_new("fixture.task-error").unwrap(),
        semantic(2),
    )
    .try_wrap(RuntimeValue::String("domain error".to_owned()))
    .unwrap()
}

#[test]
fn plan_and_awbc_bound_results_validate_values_with_the_selected_type_table() {
    let plan = Arc::new(result_plan());
    let awbc = Arc::new(result_awbc());
    let limits = RuntimeSchemaLimits::engine_default();
    let contract = TaskOutcomeContract::program(semantic(5));
    let plan_bound = contract
        .bind_program(RuntimeProgramOwner::Plan(plan.clone()), limits)
        .unwrap();
    let awbc_bound = contract
        .bind_program(RuntimeProgramOwner::Awbc(awbc.clone()), limits)
        .unwrap();

    for bound in [&plan_bound, &awbc_bound] {
        assert!(bound.try_result_ok(RuntimeValue::Bool(true)).is_ok());
        assert!(
            bound
                .try_result_ok(RuntimeValue::String("wrong".to_owned()))
                .is_err()
        );
        assert!(bound.try_result_err(opaque_error()).is_ok());
        assert!(
            bound
                .try_result_err(RuntimeValue::String("wrong".to_owned()))
                .is_err()
        );
        assert!(
            bound
                .try_payload(RuntimeValue::result_err(opaque_error()))
                .is_ok()
        );
        assert!(
            bound
                .try_payload(RuntimeValue::String("unwrapped".to_owned()))
                .is_err()
        );
    }

    let expected_owner = RuntimeOpaqueTypeOwner::exact(
        RuntimeOpaqueTypeProducerId::try_new("fixture.task-error").unwrap(),
        semantic(2),
    );
    let expected_error = RuntimeCheckedType::Opaque {
        owner: expected_owner,
    };
    assert_eq!(
        plan_bound.result_error_checked().unwrap(),
        Some(expected_error.clone())
    );
    assert_eq!(
        awbc_bound.result_error_checked().unwrap(),
        Some(expected_error)
    );

    let bool_contract = TaskOutcomeContract::program(semantic(1));
    let bool_bound = bool_contract
        .bind_program(RuntimeProgramOwner::Plan(plan.clone()), limits)
        .unwrap();
    assert!(matches!(
        bool_bound.try_result_ok(RuntimeValue::Bool(true)),
        Err(TaskOutcomeValueError::NotResult { semantic_type }) if semantic_type == semantic(1)
    ));

    let missing = TaskOutcomeContract::program(semantic(90));
    assert!(matches!(
        missing.bind_program(RuntimeProgramOwner::Plan(plan), limits),
        Err(TaskOutcomeBindingError::ProgramType(
            crate::program_types::RuntimeProgramTypeError::Missing { semantic_type }
        )) if semantic_type == semantic(90)
    ));
    assert!(matches!(
        missing.bind_program(RuntimeProgramOwner::Awbc(awbc), limits),
        Err(TaskOutcomeBindingError::ProgramType(
            crate::program_types::RuntimeProgramTypeError::Missing { semantic_type }
        )) if semantic_type == semantic(90)
    ));
}

#[test]
fn standalone_binding_keeps_the_explicit_finite_contract() {
    let contract = TaskOutcomeContract::new(RuntimeCheckedType::String);
    let bound = contract.bind_standalone().unwrap();
    assert!(
        bound
            .try_payload(RuntimeValue::String("ready".to_owned()))
            .is_ok()
    );
    assert!(bound.try_payload(RuntimeValue::Bool(true)).is_err());
    assert!(matches!(
        contract.bind_program(
            RuntimeProgramOwner::Plan(Arc::new(result_plan())),
            RuntimeSchemaLimits::engine_default()
        ),
        Err(TaskOutcomeBindingError::StandaloneRequiresStandaloneBinding)
    ));
}

#[test]
fn standalone_result_cannot_smuggle_a_nested_nominal_without_program_authority() {
    let contract = TaskOutcomeContract::new(RuntimeCheckedType::Result {
        ok: Box::new(RuntimeCheckedType::Nominal {
            nominal: crate::entry::RuntimeNominalTypeId::try_new("fixture.Node").unwrap(),
            semantic_identity: semantic(77),
            layout: crate::entry::TypeLayoutHash::from_bytes([7; 32]),
            arguments: vec![],
        }),
        error: Box::new(RuntimeCheckedType::String),
    });
    assert!(matches!(
        contract.bind_standalone(),
        Err(TaskOutcomeBindingError::InvalidStandaloneType)
    ));
    assert!(contract.try_result_ok(RuntimeValue::Unit).is_err());
}

#[test]
fn joined_waiters_require_the_same_selected_program_and_result_contract() {
    let program = Arc::new(result_plan());
    let limits = RuntimeSchemaLimits::engine_default();
    let contract = TaskOutcomeContract::program(semantic(5));
    let first = contract
        .bind_program(RuntimeProgramOwner::Plan(program.clone()), limits)
        .unwrap();
    let same = contract
        .bind_program(RuntimeProgramOwner::Plan(program), limits)
        .unwrap();
    assert!(first.same_contract(&same));

    let distinct_instance = contract
        .bind_program(RuntimeProgramOwner::Plan(Arc::new(result_plan())), limits)
        .unwrap();
    assert!(!first.same_contract(&distinct_instance));
    let different_type = TaskOutcomeContract::program(semantic(1))
        .bind_program(RuntimeProgramOwner::Plan(Arc::new(result_plan())), limits)
        .unwrap();
    assert!(!first.same_contract(&different_type));
    let standalone = TaskOutcomeContract::new(RuntimeCheckedType::Bool)
        .bind_standalone()
        .unwrap();
    assert!(!first.same_contract(&standalone));
}

#[test]
fn bound_task_spec_binds_its_own_outcome_to_the_exact_program() {
    let limits = RuntimeSchemaLimits::engine_default();
    let spec = crate::tests::task_spec(
        TaskOutcomeContract::program(semantic(5)),
        HostTaskRequest::FileReadText(FileReadTextRequest {
            path: "save:fixture.txt".to_owned(),
        }),
    );
    let submission = crate::tests::task_submission(spec.clone());
    let program = Arc::new(result_plan());
    let bound = BoundTaskSpec::bind(
        submission.clone(),
        Some(RuntimeProgramOwner::Plan(program.clone())),
        limits,
    )
    .unwrap();

    assert!(
        bound.outcome().same_contract(
            &spec
                .outcome
                .bind_program(RuntimeProgramOwner::Plan(program.clone()), limits)
                .unwrap()
        )
    );
    assert!(
        bound
            .outcome()
            .try_payload(RuntimeValue::result_ok(RuntimeValue::Bool(true)))
            .is_ok()
    );
    assert!(
        bound
            .outcome()
            .try_payload(RuntimeValue::result_ok(RuntimeValue::String(
                "wrong".to_owned()
            )))
            .is_err()
    );

    let same_owner = BoundTaskSpec::bind(
        submission.clone(),
        Some(RuntimeProgramOwner::Plan(program.clone())),
        limits,
    )
    .unwrap();
    assert!(bound.same_join_contract(&same_owner));

    let distinct_owner = BoundTaskSpec::bind(
        submission.clone(),
        Some(RuntimeProgramOwner::Plan(Arc::new(result_plan()))),
        limits,
    )
    .unwrap();
    assert!(!bound.same_join_contract(&distinct_owner));

    let mut different_identity = spec.clone();
    different_identity.debug_label = "diagnostic-only label".to_owned();
    let same_contract = BoundTaskSpec::bind(
        crate::tests::task_submission(different_identity),
        Some(RuntimeProgramOwner::Plan(program.clone())),
        limits,
    )
    .unwrap();
    assert!(bound.same_join_contract(&same_contract));

    let standalone = crate::tests::task_submission(crate::tests::task_spec(
        TaskOutcomeContract::new(RuntimeCheckedType::String),
        HostTaskRequest::FileReadText(FileReadTextRequest {
            path: "save:fixture.txt".to_owned(),
        }),
    ));
    assert!(matches!(
        BoundTaskSpec::bind(
            standalone,
            Some(RuntimeProgramOwner::Plan(program.clone())),
            limits
        ),
        Err(TaskOutcomeBindingError::StandaloneRequiresStandaloneBinding)
    ));
    assert!(matches!(
        BoundTaskSpec::bind(submission, None, limits),
        Err(TaskOutcomeBindingError::ProgramRequiresExecutable)
    ));
}
