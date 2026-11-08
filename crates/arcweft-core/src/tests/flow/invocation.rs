//! Borrowed Flow inventory admission before creating additional value owners.

use super::{
    drain, flow_id, manual_local_source, string_type, zero_parameter_flow_invocation_plan,
};
use crate::effect_row::{EffectFormula, EffectPredicate};
use crate::engine::Engine;
use crate::entry::{
    FlowParameterCoordinate, RuntimeFlowExecutableParameter, RuntimeFlowParameterMode,
    RuntimeSchemaError, RuntimeSchemaLimits,
};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    FlowEvent, FlowRuntimeId, RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeExprSeed,
    RuntimeExprSeedKind, RuntimeFlowInvocationError, RuntimeFlowSchema, RuntimeFlowSeed,
    RuntimeFunctionDefinitionIdentity, RuntimeFunctionInputBindingSeed, RuntimeFunctionInputOrigin,
    RuntimeFunctionInputSource, RuntimeFunctionInputTransfer, RuntimeFunctionParameterIdentity,
    RuntimeFunctionParameterPassing, RuntimeFunctionSiteDeclarationSeed,
    RuntimeFunctionTypeContract, RuntimeLocalDeclarationSeed, RuntimeLocalReadSeed,
    RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlan, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimeTypeBinder,
    RuntimeValueOwnershipRequirement,
};
use crate::program_types::RuntimeFunctionEffectInstantiationError;
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeFlowParameterBinding, RuntimeFunctionApplyError,
    RuntimeLocalReadMode, RuntimeValue,
};

fn string_inputs_plan(count: usize) -> (RuntimePlan, FlowRuntimeId) {
    string_inputs_plan_with_last_literal(count, None)
}

fn string_inputs_plan_with_last_literal(
    count: usize,
    last_literal: Option<&str>,
) -> (RuntimePlan, FlowRuntimeId) {
    let mut builder = RuntimePlanBuilder::new();
    let function = RuntimeSemanticTypeId::from_bytes([0xb1; 32]);
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(string_type(), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    function,
                    RuntimePlanTypeProjection::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            RuntimeTypeBinder::EMPTY,
                            EffectPredicate::unconstrained(),
                            EffectFormula::empty(),
                        ),
                        parameters: vec![string_type(); count].into_boxed_slice(),
                        result: string_type(),
                    },
                ),
            ],
            (0..count).map(|index| {
                RuntimeLocalDeclarationSeed::new(
                    manual_local_source(
                        &format!("arcweft-core.fixture.borrowed-invocation.input{index}"),
                        false,
                    ),
                    string_type(),
                )
            }),
        )
        .unwrap();
    let flow = flow_id("flow.borrowed_inputs");
    let schema = RuntimeFlowSchema {
        flow: flow.clone(),
        parameters: (0..count)
            .map(|index| RuntimeFlowExecutableParameter {
                identity: RuntimeFunctionParameterIdentity::from_accepted_identity(
                    [u8::try_from(0xc1 + index).unwrap(); 32],
                ),
                coordinate: FlowParameterCoordinate::from_position(u32::try_from(index).unwrap()),
                name: format!("input{index}"),
                mode: RuntimeFlowParameterMode::Owned,
                passing: RuntimeFunctionParameterPassing::Value,
                semantic_identity: string_type(),
            })
            .collect(),
    };
    let inputs = admitted
        .local_ids()
        .iter()
        .zip(&schema.parameters)
        .map(|(local, formal)| RuntimeFunctionInputBindingSeed {
            transfer: RuntimeFunctionInputTransfer::Formal,
            origin: RuntimeFunctionInputOrigin::Parameter(formal.identity),
            source: RuntimeFunctionInputSource::Parameter {
                position: formal.coordinate.position(),
                passing: formal.passing,
            },
            input_local: local.clone(),
            pattern: RuntimePatternSeed::new(
                string_type(),
                if formal.coordinate.position() as usize + 1 == count && last_literal.is_some() {
                    RuntimePatternSeedKind::Literal(RuntimeValue::String(
                        last_literal.unwrap().into(),
                    ))
                } else {
                    RuntimePatternSeedKind::Bind {
                        mutable: false,
                        local: local.clone(),
                    }
                },
            ),
            ownership: RuntimeValueOwnershipRequirement::Owned,
            unrestricted_bindings: Box::new([]),
        })
        .collect();
    builder.push_flow_schema(schema).unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xb2; 32]),
                Some(function),
                inputs,
                string_type(),
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([crate::plan::RuntimeFlowOpSeed::ReturnExpr(
                    RuntimeExprSeed::new(
                        string_type(),
                        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                            admitted.local_ids()[0].clone(),
                            RuntimeLocalReadMode::Copy,
                        )),
                    ),
                )]),
            },
        ))
        .unwrap();
    (builder.finish().unwrap(), flow)
}

fn borrowed_bindings(
    values: &[RuntimeValue; 2],
) -> [RuntimeFlowParameterBinding<&RuntimeValue>; 2] {
    [0, 1].map(|index| RuntimeFlowParameterBinding {
        parameter: FlowParameterCoordinate::from_position(u32::try_from(index).unwrap()),
        value: &values[index],
    })
}

fn transfer_input_reads(
    plan: &RuntimePlan,
    flow: &FlowRuntimeId,
    order: [usize; 2],
) -> crate::plan::RuntimeFlowTransfer<crate::value::RuntimeExpr> {
    let function = plan
        .function_sites()
        .get(
            plan.flows()
                .iter()
                .find(|candidate| &candidate.id == flow)
                .unwrap()
                .function_site(),
        )
        .unwrap();
    let locals = function
        .parameter_inputs()
        .map(|input| input.input_local())
        .collect::<Vec<_>>();
    crate::plan::RuntimeFlowTransfer {
        target: crate::plan::RuntimeFlowTransferTarget::Static(flow.clone()),
        arguments: order
            .into_iter()
            .map(|index| RuntimeFlowParameterBinding {
                parameter: FlowParameterCoordinate::from_position(index as u32),
                value: crate::value::RuntimeExpr::from_admitted_parts(
                    plan.local_declarations().get(locals[index]).unwrap().ty(),
                    crate::value::RuntimeExprKind::Local(
                        crate::value::RuntimeLocalRead::from_admitted_parts(
                            locals[index],
                            RuntimeLocalReadMode::Move,
                        ),
                    ),
                ),
            })
            .collect(),
    }
}

#[test]
fn flow_transfer_moves_the_complete_inventory_in_authored_order() {
    let (plan, flow) = string_inputs_plan(2);
    let transfer = transfer_input_reads(&plan, &flow, [1, 0]);
    let mut env = crate::value::RuntimeEnv::default();
    for argument in &transfer.arguments {
        let local = crate::plan::RuntimeFlowTransfer::input_local(argument).unwrap();
        env.set(
            local,
            RuntimeValue::String(format!("input{}", argument.parameter.position())),
        );
    }
    let (values, binding) = env.transfer_flow_inputs(&plan, &flow, &transfer).unwrap();
    assert!(binding.is_some());
    assert_eq!(
        values.input_order().unwrap(),
        transfer.input_order().unwrap()
    );
    for (argument, original) in values.arguments.iter().zip(&transfer.arguments) {
        assert_eq!(argument.parameter, original.parameter);
        assert_eq!(
            argument.value,
            RuntimeValue::String(format!("input{}", argument.parameter.position()))
        );
        assert!(
            env.get(crate::plan::RuntimeFlowTransfer::input_local(original).unwrap())
                .is_none()
        );
    }
}

#[test]
fn flow_transfer_rejects_a_later_bad_input_before_moving_the_first() {
    let (plan, flow) = string_inputs_plan(2);
    let transfer = transfer_input_reads(&plan, &flow, [0, 1]);
    let first = crate::plan::RuntimeFlowTransfer::input_local(&transfer.arguments[0]).unwrap();
    let second = crate::plan::RuntimeFlowTransfer::input_local(&transfer.arguments[1]).unwrap();
    for later_value in [None, Some(RuntimeValue::Bool(false))] {
        let mut env = crate::value::RuntimeEnv::default();
        env.set(first, RuntimeValue::String("still owned by caller".into()));
        if let Some(value) = &later_value {
            env.set(second, value.clone());
        }
        let error = env
            .transfer_flow_inputs(&plan, &flow, &transfer)
            .unwrap_err();
        match later_value {
            None => assert!(
                matches!(error, crate::value::RuntimeEvalError::UninitializedLocal(local) if local == second)
            ),
            Some(_) => assert!(matches!(
                error,
                crate::value::RuntimeEvalError::FlowInvocation(_)
            )),
        }
        assert_eq!(
            env.get(first),
            Some(&RuntimeValue::String("still owned by caller".into()))
        );
        assert_eq!(env.get(second), later_value.as_ref());
    }
}

#[test]
fn flow_transfer_rejects_aliased_owners_before_reading_or_moving_values() {
    let (plan, flow) = string_inputs_plan(2);
    let mut transfer = transfer_input_reads(&plan, &flow, [0, 1]);
    transfer.arguments[1].value = transfer.arguments[0].value.clone();
    let local = crate::plan::RuntimeFlowTransfer::input_local(&transfer.arguments[0]).unwrap();
    let (_registry, need) = crate::tests::pending_string_need(string_type());
    let mut env = crate::value::RuntimeEnv::default();
    env.set(local, RuntimeValue::NeedHandle(need));
    assert!(matches!(env.transfer_flow_inputs(&plan, &flow, &transfer),
        Err(crate::value::RuntimeEvalError::FlowInvocation(error))
            if *error == RuntimeFlowInvocationError::DuplicateTransferInput { local }));
    assert!(matches!(env.get(local), Some(RuntimeValue::NeedHandle(_))));
}

#[test]
fn retained_flow_transfer_shares_one_value_budget_across_all_inputs() {
    let (plan, flow) = string_inputs_plan(2);
    let plan = std::sync::Arc::new(plan);
    let owner = crate::task::RuntimeProgramOwner::Plan(plan.clone());
    let values = [
        RuntimeValue::String("first".into()),
        RuntimeValue::String("second".into()),
    ];
    let live = crate::plan::RuntimeFlowTransfer {
        target: crate::plan::RuntimeFlowTransferTarget::Static(flow.clone()),
        arguments: Box::new(borrowed_bindings(&values)),
    };
    let (declaration, _) = plan.validate_flow_transfer_inputs(&flow, &live).unwrap();
    let retained = live
        .try_map(|value| {
            crate::value::AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, &owner)
        })
        .unwrap();
    let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
        owner,
        RuntimeSchemaLimits {
            max_nodes: 1,
            ..RuntimeSchemaLimits::engine_default()
        },
    );
    let error = plan
        .admit_retained_flow_transfer_inputs(
            &flow,
            &retained,
            declaration.type_instantiation.as_deref(),
            &mut admission,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded {
            budget: "nodes"
        })
    ));
    assert_eq!(admission.validate_metadata(&0u32).unwrap_err(), error);
}

#[test]
fn retained_flow_transfer_rejects_a_forged_value_that_fails_its_original_pattern() {
    let (plan, flow) = string_inputs_plan_with_last_literal(2, Some("required"));
    let plan = std::sync::Arc::new(plan);
    let owner = crate::task::RuntimeProgramOwner::Plan(plan.clone());
    let valid = [
        RuntimeValue::String("first".into()),
        RuntimeValue::String("required".into()),
    ];
    let live = crate::plan::RuntimeFlowTransfer {
        target: crate::plan::RuntimeFlowTransferTarget::Static(flow.clone()),
        arguments: Box::new(borrowed_bindings(&valid)),
    };
    let (declaration, _) = plan.validate_flow_transfer_inputs(&flow, &live).unwrap();
    let mut retained = live
        .try_map(|value| {
            crate::value::AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, &owner)
        })
        .unwrap();
    plan.admit_retained_flow_transfer_inputs(
        &flow,
        &retained,
        declaration.type_instantiation.as_deref(),
        &mut crate::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            RuntimeSchemaLimits::engine_default(),
        ),
    )
    .unwrap();
    retained.arguments[1].value =
        crate::value::AwbcRuntimeValueSnapshot::from_admitted_runtime_value(
            &RuntimeValue::String("forged".into()),
            &owner,
        )
        .unwrap();
    assert!(
        plan.admit_retained_flow_transfer_inputs(
            &flow,
            &retained,
            declaration.type_instantiation.as_deref(),
            &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                owner,
                RuntimeSchemaLimits::engine_default()
            )
        )
        .is_err()
    );
}

#[test]
fn borrowed_flow_invocation_admits_the_complete_inventory_and_executes_its_inputs() {
    let (plan, flow) = string_inputs_plan(2);
    let values = [
        RuntimeValue::String("first input".into()),
        RuntimeValue::String("second input".into()),
    ];
    let invocation = plan
        .seal_flow_invocation_copying(
            flow,
            borrowed_bindings(&values),
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap();
    let mut engine = Engine::for_flow_invocation(invocation).unwrap();
    assert_eq!(
        drain(&mut engine).flow_events,
        [FlowEvent::Return {
            value: "first input".into()
        }]
    );
    assert_eq!(values[0], RuntimeValue::String("first input".into()));
}

#[test]
fn borrowed_flow_invocation_shares_resources_across_roots_before_type_inference() {
    let (plan, flow) = string_inputs_plan(2);
    let values = [
        RuntimeValue::String("first".into()),
        RuntimeValue::Bool(false),
    ];
    let limits = RuntimeSchemaLimits {
        max_nodes: 1,
        ..RuntimeSchemaLimits::engine_default()
    };
    let error = plan
        .seal_flow_invocation_copying(flow, borrowed_bindings(&values), limits)
        .unwrap_err();
    assert!(
        matches!(error, RuntimeFlowInvocationError::InputAdmission(source)
        if *source == AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded { budget: "nodes" }))
    );
}

#[test]
fn borrowed_flow_invocation_refuses_affine_inputs_before_copying_or_type_inference() {
    let (plan, flow) = string_inputs_plan(2);
    let (_registry, need) = crate::tests::pending_string_need(string_type());
    let values = [
        RuntimeValue::String("first".into()),
        RuntimeValue::NeedHandle(need),
    ];
    let error = plan
        .seal_flow_invocation_copying(
            flow,
            borrowed_bindings(&values),
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap_err();
    assert!(
        matches!(error, RuntimeFlowInvocationError::NonCopyableParameter { parameter }
        if parameter == FlowParameterCoordinate::from_position(1))
    );
}

#[test]
fn borrowed_flow_invocation_retains_the_original_joint_type_failure() {
    let (plan, flow) = string_inputs_plan(2);
    let values = [
        RuntimeValue::String("first".into()),
        RuntimeValue::Bool(false),
    ];
    assert!(matches!(
        plan.seal_flow_invocation_copying(flow, borrowed_bindings(&values), RuntimeSchemaLimits::engine_default()),
        Err(RuntimeFlowInvocationError::FunctionInputs(RuntimeFunctionApplyError::InputInstantiation {
            source: RuntimeFunctionEffectInstantiationError::InputRelation { ordinal: 1, expected }, ..
        })) if expected == string_type()
    ));
}

#[test]
fn borrowed_flow_invocation_bounds_collection_before_inspecting_values() {
    let limits = RuntimeSchemaLimits::engine_default();
    let (plan, _) = zero_parameter_flow_invocation_plan();
    let never = std::iter::from_fn(|| -> Option<RuntimeFlowParameterBinding<&RuntimeValue>> {
        panic!("unknown Flow must not poll its borrowed input iterator")
    });
    assert!(matches!(
        plan.seal_flow_invocation_copying(flow_id("flow.unknown"), never, limits),
        Err(RuntimeFlowInvocationError::UnknownFlow { .. })
    ));
    let (plan, flow) = zero_parameter_flow_invocation_plan();
    let value = RuntimeValue::Unit;
    let polled = std::cell::Cell::new(false);
    let bindings = std::iter::repeat_with(|| {
        assert!(
            !polled.replace(true),
            "only the first excess binding can be polled"
        );
        RuntimeFlowParameterBinding {
            parameter: FlowParameterCoordinate::from_position(0),
            value: &value,
        }
    })
    .take(usize::MAX);
    assert!(matches!(
        plan.seal_flow_invocation_copying(flow, bindings, limits),
        Err(RuntimeFlowInvocationError::BindingCount {
            expected: 0,
            actual: 1,
            ..
        })
    ));
    assert!(polled.get());
}

#[test]
fn borrowed_flow_invocation_bounds_group_storage_before_collecting_inputs() {
    let (plan, flow) = string_inputs_plan(2);
    let limits = RuntimeSchemaLimits {
        max_sequence_items: 1,
        ..RuntimeSchemaLimits::engine_default()
    };
    let bindings = std::iter::from_fn(|| -> Option<RuntimeFlowParameterBinding<&RuntimeValue>> {
        panic!("an oversized input group must be refused before collecting its rows")
    });
    assert!(matches!(
        plan.seal_flow_invocation_copying(flow, bindings, limits),
        Err(RuntimeFlowInvocationError::InputAdmission(source))
            if *source == AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded {
                budget: "sequence_items",
            })
    ));
}

fn owned_invocation(values: [RuntimeValue; 2]) -> crate::plan::RuntimeFlowInvocation {
    let (plan, flow) = string_inputs_plan(2);
    plan.seal_flow_invocation(
        flow,
        values
            .into_iter()
            .enumerate()
            .map(|(index, value)| RuntimeFlowParameterBinding {
                parameter: FlowParameterCoordinate::from_position(u32::try_from(index).unwrap()),
                value,
            }),
    )
    .unwrap()
}

#[test]
fn repeated_flow_invocation_retains_the_exact_program_and_original_inputs() {
    let invocation = owned_invocation([
        RuntimeValue::String("first input".into()),
        RuntimeValue::String("second input".into()),
    ]);
    let repeated = invocation
        .try_repeat_unrestricted(RuntimeSchemaLimits::engine_default())
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(invocation.plan(), repeated.plan()));
    for invocation in [invocation, repeated] {
        let mut engine = Engine::for_flow_invocation(invocation).unwrap();
        assert_eq!(
            drain(&mut engine).flow_events,
            [FlowEvent::Return {
                value: "first input".into()
            }]
        );
    }
}

#[test]
fn repeated_flow_invocation_refuses_string_growth_under_the_selected_limits() {
    let invocation = owned_invocation([
        RuntimeValue::String("x".repeat(64)),
        RuntimeValue::String("second".into()),
    ]);
    let limits = RuntimeSchemaLimits {
        max_string_bytes: 32,
        ..RuntimeSchemaLimits::engine_default()
    };
    assert!(matches!(invocation.try_repeat_unrestricted(limits),
        Err(RuntimeFlowInvocationError::InputAdmission(source))
            if *source == AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded { budget: "string_bytes" })
    ));
}

#[test]
fn repeated_flow_invocation_shares_node_limits_and_preserves_custody_after_refusal() {
    let invocation = owned_invocation([
        RuntimeValue::String("first".into()),
        RuntimeValue::String("second".into()),
    ]);
    let limits = RuntimeSchemaLimits {
        max_nodes: 1,
        ..RuntimeSchemaLimits::engine_default()
    };
    assert!(matches!(invocation.try_repeat_unrestricted(limits),
        Err(RuntimeFlowInvocationError::InputAdmission(source))
            if *source == AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded { budget: "nodes" })
    ));
    let mut engine = Engine::for_flow_invocation(invocation).unwrap();
    assert_eq!(
        drain(&mut engine).flow_events,
        [FlowEvent::Return {
            value: "first".into()
        }]
    );
}

#[test]
fn repeated_flow_invocation_bounds_its_retained_identity_before_copying() {
    let invocation = owned_invocation([
        RuntimeValue::String("first".into()),
        RuntimeValue::String("second".into()),
    ]);
    let limits = RuntimeSchemaLimits {
        max_string_bytes: 16,
        ..RuntimeSchemaLimits::engine_default()
    };
    assert!(matches!(invocation.try_repeat_unrestricted(limits),
        Err(RuntimeFlowInvocationError::InputAdmission(source))
            if *source == AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded { budget: "string_bytes" })
    ));
}

#[test]
fn borrowed_flow_invocation_bounds_group_storage_after_admitting_its_identity() {
    let (plan, flow) = string_inputs_plan(3);
    let limits = RuntimeSchemaLimits {
        max_sequence_items: 2,
        ..RuntimeSchemaLimits::engine_default()
    };
    crate::value::RuntimeRetainedValueAdmission::standalone(limits)
        .validate_metadata(&flow)
        .unwrap();
    let bindings = std::iter::from_fn(|| -> Option<RuntimeFlowParameterBinding<&RuntimeValue>> {
        panic!("the admitted identity must not hide an oversized input group")
    });
    assert!(
        matches!(plan.seal_flow_invocation_copying(flow, bindings, limits),
            Err(RuntimeFlowInvocationError::InputAdmission(source))
                if *source == AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded { budget: "sequence_items" })
        )
    );
}
