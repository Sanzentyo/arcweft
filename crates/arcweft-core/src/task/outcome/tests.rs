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
    let spec = TaskSpec::new(
        TaskId("task".to_owned()),
        TaskKey("request".to_owned()),
        TaskClass::Io,
        TaskPriority(0),
        CancelScopeId("scope".to_owned()),
        TaskPolicy::JoinSameKey,
        HostTaskRequest::FileReadText(FileReadTextRequest {
            path: "save:fixture.txt".to_owned(),
        }),
    )
    .with_outcome(TaskOutcomeContract::program(semantic(5)));
    let program = Arc::new(result_plan());
    let bound = BoundTaskSpec::bind(
        spec.clone(),
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
        spec.clone(),
        Some(RuntimeProgramOwner::Plan(program.clone())),
        limits,
    )
    .unwrap();
    assert!(bound.same_join_contract(&same_owner));

    let distinct_owner = BoundTaskSpec::bind(
        spec.clone(),
        Some(RuntimeProgramOwner::Plan(Arc::new(result_plan()))),
        limits,
    )
    .unwrap();
    assert!(!bound.same_join_contract(&distinct_owner));

    let mut different_identity = spec.clone();
    different_identity.id = TaskId("waiter".to_owned());
    different_identity.debug_label = "diagnostic-only label".to_owned();
    let same_contract = BoundTaskSpec::bind(
        different_identity,
        Some(RuntimeProgramOwner::Plan(program.clone())),
        limits,
    )
    .unwrap();
    assert!(bound.same_join_contract(&same_contract));

    let standalone = TaskSpec::new(
        TaskId("standalone".to_owned()),
        TaskKey("request".to_owned()),
        TaskClass::Io,
        TaskPriority(0),
        CancelScopeId("scope".to_owned()),
        TaskPolicy::JoinSameKey,
        HostTaskRequest::FileReadText(FileReadTextRequest {
            path: "save:fixture.txt".to_owned(),
        }),
    )
    .with_outcome(TaskOutcomeContract::new(RuntimeCheckedType::String));
    assert!(matches!(
        BoundTaskSpec::bind(
            standalone,
            Some(RuntimeProgramOwner::Plan(program.clone())),
            limits
        ),
        Err(TaskOutcomeBindingError::StandaloneRequiresStandaloneBinding)
    ));
    assert!(matches!(
        BoundTaskSpec::bind(spec, None, limits),
        Err(TaskOutcomeBindingError::ProgramRequiresExecutable)
    ));
}
