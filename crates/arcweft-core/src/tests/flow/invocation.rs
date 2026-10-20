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

fn two_string_inputs_plan() -> (RuntimePlan, FlowRuntimeId) {
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
                        parameters: Box::new([string_type(), string_type()]),
                        result: string_type(),
                    },
                ),
            ],
            ["first", "second"].map(|name| {
                RuntimeLocalDeclarationSeed::new(
                    manual_local_source(
                        &format!("arcweft-core.fixture.borrowed-invocation.{name}"),
                        false,
                    ),
                    string_type(),
                )
            }),
        )
        .unwrap();
    let flow = flow_id("flow.borrowed_inputs");
    let parameters = [0xc1, 0xc2]
        .map(|marker| RuntimeFunctionParameterIdentity::from_accepted_identity([marker; 32]));
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: parameters
                .iter()
                .enumerate()
                .map(|(index, identity)| RuntimeFlowExecutableParameter {
                    identity: *identity,
                    coordinate: FlowParameterCoordinate::from_position(
                        u32::try_from(index).unwrap(),
                    ),
                    name: format!("input{index}"),
                    mode: RuntimeFlowParameterMode::Owned,
                    passing: RuntimeFunctionParameterPassing::Value,
                    semantic_identity: string_type(),
                })
                .collect(),
        })
        .unwrap();
    let inputs = admitted
        .local_ids()
        .iter()
        .enumerate()
        .map(|(index, local)| RuntimeFunctionInputBindingSeed {
            transfer: RuntimeFunctionInputTransfer::Formal,
            origin: RuntimeFunctionInputOrigin::Parameter(parameters[index]),
            source: RuntimeFunctionInputSource::Parameter {
                position: u32::try_from(index).unwrap(),
                passing: RuntimeFunctionParameterPassing::Value,
            },
            input_local: local.clone(),
            pattern: RuntimePatternSeed::new(
                string_type(),
                RuntimePatternSeedKind::Bind {
                    mutable: false,
                    local: local.clone(),
                },
            ),
            ownership: RuntimeValueOwnershipRequirement::Unrestricted,
            unrestricted_bindings: Box::new([]),
        })
        .collect();
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

#[test]
fn borrowed_flow_invocation_admits_the_complete_inventory_and_executes_its_inputs() {
    let (plan, flow) = two_string_inputs_plan();
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
    let (plan, flow) = two_string_inputs_plan();
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
    let (plan, flow) = two_string_inputs_plan();
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
        matches!(error, RuntimeFlowInvocationError::InputAdmission(source)
        if *source == AwbcRuntimeValueSnapshotError::AffineValue)
    );
}

#[test]
fn borrowed_flow_invocation_retains_the_original_joint_type_failure() {
    let (plan, flow) = two_string_inputs_plan();
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
    let (plan, flow) = two_string_inputs_plan();
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
