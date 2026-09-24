use std::sync::Arc;

use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::plan::{
    RuntimeCallableAttachedContract, RuntimeCallableInputSource,
    RuntimeCallableParameterCoordinate, RuntimeCallableParameterInput,
    RuntimeCallableParameterKind, RuntimeCallablePosition, RuntimeCallableRetainedInput,
    RuntimeCallableRetainedRole, RuntimeCallableStateDefinition, RuntimeCallableTransition,
    RuntimeEffectSet, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFunctionInputBindingSeed,
    RuntimeFunctionInputSource, RuntimeFunctionSiteBodyKind, RuntimeFunctionSiteBodySeed,
    RuntimeFunctionSiteDeclarationSeed, RuntimeLocalDeclarationSeed, RuntimePatternSeed,
    RuntimePatternSeedKind, RuntimePlan, RuntimePlanBuilder, RuntimePlanTypeProjection,
    RuntimePlanTypeSeed,
};
use crate::runtime_id::RuntimeCallableStateId;
use crate::task::RuntimeProgramOwner;
use crate::value::{
    RuntimeCallableApplication, RuntimeCallableValue, RuntimeCallableValueError, RuntimeValue,
};

fn captured_identity_plan() -> RuntimePlan {
    let integer = RuntimeCheckedType::Signed(crate::value::RuntimeSignedIntWidth::I64)
        .semantic_identity_digest();
    let function = RuntimeSemanticTypeId::from_bytes([0x8b; 32]);
    let mut builder = RuntimePlanBuilder::new();
    let inputs = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(
                    integer,
                    RuntimePlanTypeProjection::Signed(crate::value::RuntimeSignedIntWidth::I64),
                ),
                RuntimePlanTypeSeed::new(
                    function,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([integer]),
                        result: integer,
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(integer),
                RuntimeLocalDeclarationSeed::new(integer),
            ],
        )
        .unwrap();
    let site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            inputs: Box::new([
                RuntimeFunctionInputBindingSeed {
                    source: RuntimeFunctionInputSource::Capture { position: 0 },
                    input_local: inputs.local_ids()[0].clone(),
                    pattern: RuntimePatternSeed::new(integer, RuntimePatternSeedKind::Discard),
                },
                RuntimeFunctionInputBindingSeed {
                    source: RuntimeFunctionInputSource::Parameter { position: 0 },
                    input_local: inputs.local_ids()[1].clone(),
                    pattern: RuntimePatternSeed::new(integer, RuntimePatternSeedKind::Discard),
                },
            ]),
            result: integer,
            body_kind: RuntimeFunctionSiteBodyKind::Expression,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    builder
        .define_function_site_seed(
            &site,
            RuntimeFunctionSiteBodySeed::Expression(RuntimeExprSeed::new(
                integer,
                RuntimeExprSeedKind::Local(inputs.local_ids()[0].clone()),
            )),
        )
        .unwrap();
    let state = builder.reserve_callable_state_seed().unwrap();
    builder
        .define_callable_state_seed(
            &state,
            RuntimeCallableStateDefinition {
                function_type: function,
                origin: state.clone(),
                position: RuntimeCallablePosition::Unapplied,
                retained: Box::new([RuntimeCallableRetainedInput {
                    role: RuntimeCallableRetainedRole::Capture { position: 0 },
                    ty: integer,
                }]),
                parameters: Box::new([RuntimeCallableParameterInput {
                    coordinate: RuntimeCallableParameterCoordinate {
                        group: 0,
                        parameter: 0,
                    },
                    kind: RuntimeCallableParameterKind::Fixed,
                    abi_ty: integer,
                    binding_ty: integer,
                }]),
                result: integer,
                attached: RuntimeCallableAttachedContract::None,
                transition: RuntimeCallableTransition::Invoke {
                    function: site,
                    captures: Box::new([RuntimeCallableInputSource::Retained { position: 0 }]),
                    arguments: Box::new([RuntimeCallableInputSource::Argument { position: 0 }]),
                },
                partials: Box::new([]),
            },
        )
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn callable_admission_requires_the_complete_retained_layout_and_exact_owner() {
    let owner = RuntimeProgramOwner::Plan(Arc::new(captured_identity_plan()));
    let state = RuntimeCallableStateId::from_zero_based(0).unwrap();
    assert!(matches!(
        RuntimeCallableValue::try_new(owner.clone(), state, []),
        Err(RuntimeCallableValueError::RetainedCount {
            expected: 1,
            actual: 0,
            ..
        })
    ));
    assert!(matches!(
        RuntimeCallableValue::try_new(owner.clone(), state, [RuntimeValue::Bool(true)]),
        Err(RuntimeCallableValueError::RetainedType { position: 0, .. })
    ));
    let callable = RuntimeCallableValue::try_new(owner, state, [RuntimeValue::i64(42)]).unwrap();
    let unrelated = RuntimeProgramOwner::Plan(Arc::new(captured_identity_plan()));
    assert!(matches!(
        callable.validate_for_owner(&unrelated),
        Err(RuntimeCallableValueError::ForeignProgram)
    ));
    for argument in [1, 2] {
        let RuntimeCallableApplication::Invoke(invocation) = callable
            .prepare_group(&[RuntimeValue::i64(argument)], None)
            .unwrap()
        else {
            panic!("terminal state must invoke");
        };
        assert_eq!(invocation.captures, [RuntimeValue::i64(42)]);
        assert_eq!(invocation.arguments, [RuntimeValue::i64(argument)]);
    }
    assert_eq!(callable.retained(), [RuntimeValue::i64(42)]);
    assert!(matches!(
        callable.prepare_group(&[RuntimeValue::Unit], None),
        Err(RuntimeCallableValueError::ArgumentType { position: 0, .. })
    ));
}

#[test]
fn generic_serialization_cannot_publish_or_restore_a_live_callable() {
    let owner = RuntimeProgramOwner::Plan(Arc::new(captured_identity_plan()));
    let callable = RuntimeCallableValue::try_new(
        owner,
        RuntimeCallableStateId::from_zero_based(0).unwrap(),
        [RuntimeValue::i64(42)],
    )
    .unwrap();
    assert!(serde_json::to_value(RuntimeValue::Callable(callable)).is_err());
    assert!(serde_json::from_str::<RuntimeCallableValue>(r#"{"state":1,"retained":[]}"#).is_err());
}

#[test]
fn checked_partial_application_seals_retained_parameter_coordinates() {
    let integer = RuntimeCheckedType::Signed(crate::value::RuntimeSignedIntWidth::I64)
        .semantic_identity_digest();
    let boolean = RuntimeCheckedType::Bool.semantic_identity_digest();
    let initial_type = RuntimeSemanticTypeId::from_bytes([0x93; 32]);
    let partial_type = RuntimeSemanticTypeId::from_bytes([0x94; 32]);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(
                    integer,
                    RuntimePlanTypeProjection::Signed(crate::value::RuntimeSignedIntWidth::I64),
                ),
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    initial_type,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([integer, boolean]),
                        result: integer,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    partial_type,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([boolean]),
                        result: integer,
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(integer),
                RuntimeLocalDeclarationSeed::new(boolean),
            ],
        )
        .unwrap();
    let target = builder
        .push_function_site_seed(
            [integer, boolean]
                .into_iter()
                .enumerate()
                .map(|(position, ty)| RuntimeFunctionInputBindingSeed {
                    source: RuntimeFunctionInputSource::Parameter {
                        position: position as u32,
                    },
                    input_local: admission.local_ids()[position].clone(),
                    pattern: RuntimePatternSeed::new(ty, RuntimePatternSeedKind::Discard),
                }),
            RuntimeExprSeed::new(
                integer,
                RuntimeExprSeedKind::Local(admission.local_ids()[0].clone()),
            ),
        )
        .unwrap();
    builder
        .push_function_site_seed(
            [],
            RuntimeExprSeed::new(
                partial_type,
                RuntimeExprSeedKind::Apply {
                    callee: Box::new(RuntimeExprSeed::new(
                        initial_type,
                        RuntimeExprSeedKind::Function {
                            site: target,
                            captures: Box::new([]),
                        },
                    )),
                    args: Box::new([crate::plan::RuntimeCallArgumentSeed::new(
                        RuntimeExprSeed::new(
                            integer,
                            RuntimeExprSeedKind::Value(RuntimeValue::i64(42)),
                        ),
                        crate::value::RuntimeCallArgumentMode::Value,
                        0,
                    )]),
                },
            ),
        )
        .unwrap();
    let plan = Arc::new(builder.finish().unwrap());
    let (initial, definition) = plan
        .callable_states()
        .iter_with_ids()
        .find(|(_, state)| matches!(state.position, RuntimeCallablePosition::Unapplied))
        .unwrap();
    assert_eq!(definition.partials.len(), 1);
    assert_eq!(
        definition.partials[0].parameters.as_ref(),
        [RuntimeCallableParameterCoordinate {
            group: 0,
            parameter: 0
        }]
    );
    let callable =
        RuntimeCallableValue::try_new(RuntimeProgramOwner::Plan(plan), initial, []).unwrap();
    let partial = callable.try_bind_prefix(&[RuntimeValue::i64(42)]).unwrap();
    assert_eq!(partial.function_type().unwrap(), partial_type);
    assert_eq!(partial.retained(), [RuntimeValue::i64(42)]);
    let RuntimeCallableApplication::Invoke(invocation) = partial
        .prepare_group(&[RuntimeValue::Bool(false)], None)
        .unwrap()
    else {
        panic!("the remaining current-group parameter must invoke the original body");
    };
    assert_eq!(
        invocation.arguments,
        [RuntimeValue::i64(42), RuntimeValue::Bool(false)]
    );
}

fn rest_partial_plan(
    kind: crate::plan::RuntimePlanSequenceKind,
) -> Result<RuntimePlan, crate::plan::RuntimePlanBuildError> {
    let unit = RuntimeCheckedType::Unit.semantic_identity_digest();
    let boolean = RuntimeCheckedType::Bool.semantic_identity_digest();
    let sequence =
        RuntimeCheckedType::Sequence(Box::new(RuntimeCheckedType::Unit)).semantic_identity_digest();
    let initial_type = RuntimeSemanticTypeId::from_bytes([0x91; 32]);
    let partial_type = RuntimeSemanticTypeId::from_bytes([0x92; 32]);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    sequence,
                    RuntimePlanTypeProjection::Sequence { kind, item: unit },
                ),
                RuntimePlanTypeSeed::new(
                    initial_type,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([unit, boolean]),
                        result: unit,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    partial_type,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([boolean]),
                        result: unit,
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(sequence),
                RuntimeLocalDeclarationSeed::new(boolean),
            ],
        )
        .unwrap();
    let site = builder
        .push_function_site_seed(
            [sequence, boolean]
                .into_iter()
                .enumerate()
                .map(|(position, ty)| RuntimeFunctionInputBindingSeed {
                    source: RuntimeFunctionInputSource::Parameter {
                        position: position as u32,
                    },
                    input_local: admission.local_ids()[position].clone(),
                    pattern: RuntimePatternSeed::new(ty, RuntimePatternSeedKind::Discard),
                }),
            RuntimeExprSeed::new(unit, RuntimeExprSeedKind::Value(RuntimeValue::Unit)),
        )
        .unwrap();
    let initial = builder.reserve_callable_state_seed().unwrap();
    let partial = builder.reserve_callable_state_seed().unwrap();
    let coordinate = RuntimeCallableParameterCoordinate {
        group: 0,
        parameter: 0,
    };
    builder
        .define_callable_state_seed(
            &initial,
            RuntimeCallableStateDefinition {
                function_type: initial_type,
                origin: initial.clone(),
                position: RuntimeCallablePosition::Unapplied,
                retained: Box::new([]),
                parameters: Box::new([RuntimeCallableParameterInput {
                    coordinate,
                    kind: RuntimeCallableParameterKind::Rest,
                    abi_ty: unit,
                    binding_ty: sequence,
                }]),
                result: unit,
                attached: RuntimeCallableAttachedContract::Required { ty: boolean },
                transition: RuntimeCallableTransition::Invoke {
                    function: site.clone(),
                    captures: Box::new([]),
                    arguments: Box::new([
                        RuntimeCallableInputSource::Argument { position: 0 },
                        RuntimeCallableInputSource::Attached,
                    ]),
                },
                partials: Box::new([crate::plan::RuntimeCallablePartialTransition {
                    parameters: Box::new([coordinate]),
                    state: partial.clone(),
                    values: Box::new([RuntimeCallableInputSource::Argument { position: 0 }]),
                }]),
            },
        )
        .unwrap();
    builder
        .define_callable_state_seed(
            &partial,
            RuntimeCallableStateDefinition {
                function_type: partial_type,
                origin: initial,
                position: RuntimeCallablePosition::WithinGroup {
                    group: 0,
                    bound: Box::new([coordinate]),
                },
                retained: Box::new([RuntimeCallableRetainedInput {
                    role: RuntimeCallableRetainedRole::Parameter(coordinate),
                    ty: sequence,
                }]),
                parameters: Box::new([]),
                result: unit,
                attached: RuntimeCallableAttachedContract::Required { ty: boolean },
                transition: RuntimeCallableTransition::Invoke {
                    function: site,
                    captures: Box::new([]),
                    arguments: Box::new([
                        RuntimeCallableInputSource::Retained { position: 0 },
                        RuntimeCallableInputSource::Attached,
                    ]),
                },
                partials: Box::new([]),
            },
        )
        .unwrap();
    builder.finish()
}

#[test]
fn rest_binding_rejects_other_sequence_families() {
    for kind in [
        crate::plan::RuntimePlanSequenceKind::Seq,
        crate::plan::RuntimePlanSequenceKind::Slice,
        crate::plan::RuntimePlanSequenceKind::Array,
    ] {
        assert!(matches!(
            rest_partial_plan(kind),
            Err(crate::plan::RuntimePlanBuildError::Plan(
                crate::plan::RuntimePlanError::CallableState(
                    crate::plan::RuntimeCallableStateError::InvalidLayout { .. }
                )
            ))
        ));
    }
}

#[test]
fn partial_rest_binding_retains_a_pack_until_the_attached_argument_arrives() {
    let owner = RuntimeProgramOwner::Plan(Arc::new(
        rest_partial_plan(crate::plan::RuntimePlanSequenceKind::Vec).unwrap(),
    ));
    let callable = RuntimeCallableValue::try_new(
        owner,
        RuntimeCallableStateId::from_zero_based(0).unwrap(),
        [],
    )
    .unwrap();
    assert!(matches!(
        callable.try_bind_prefix(&[RuntimeValue::Bool(true)]),
        Err(RuntimeCallableValueError::ArgumentType { position: 0, .. })
    ));
    let prefix = callable.try_bind_prefix(&[RuntimeValue::Unit]).unwrap();
    let pack = crate::value::runtime_sequence_values(vec![RuntimeValue::Unit]);
    assert_eq!(prefix.retained(), [pack.clone()]);
    assert!(callable.retained().is_empty());
    let (ordinary, attached) = prefix
        .materialize_abi_arguments(&[RuntimeValue::Bool(true)])
        .unwrap();
    let RuntimeCallableApplication::Invoke(invocation) =
        prefix.prepare_group(&ordinary, attached).unwrap()
    else {
        panic!("the completed attached group must invoke");
    };
    assert_eq!(invocation.arguments, [pack, RuntimeValue::Bool(true)]);
}
