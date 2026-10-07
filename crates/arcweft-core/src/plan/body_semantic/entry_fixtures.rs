//! Shared actual Entry/Flow aggregate fixtures for executable-role and Entry transcripts.

use crate::entry::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::*;
use crate::runtime_id::RuntimeFunctionSiteId;
use crate::value::RuntimeValue;

fn admit_controller_entry(builder: &mut RuntimePlanBuilder, flow: FlowRuntimeId) {
    let callable = RuntimeCallableRole {
        callable: RuntimeCallableId::from_checked_digest([81; 32]),
        contract: CallableContractHash::from_bytes([82; 32]),
    };
    builder
        .push_callable_executable_seed(RuntimeCallableExecutableSeed {
            callable: callable.callable.clone(),
            contract: callable.contract,
            code: RuntimeCallableExecutableSeedCode::ControllerFlow(flow.clone()),
        })
        .unwrap();
    builder
        .push_flow_executable(RuntimeFlowExecutable {
            flow: flow.clone(),
            contract: FlowContractHash::from_bytes([83; 32]),
            controller: Some(callable.clone()),
        })
        .unwrap();
    let binding = EntryBindingIdentity::from_bytes([84; 32]);
    builder
        .push_entry(RuntimeEntrySpec {
            id: EntryRuntimeId::canonical("agent").unwrap(),
            kind: RuntimeEntryKind::Agent,
            binding,
            target: RuntimeEntryTarget::Controller(flow),
            roles: RuntimeEntryRoles::Agent(Box::new(RuntimeAgentEntryRoles {
                binding,
                controller: callable,
                policy: AgentPolicyHash::from_bytes([85; 32]),
                budget: AgentBudget::default(),
            })),
        })
        .unwrap();
}

pub(super) fn controller_plan(
    padding: bool,
    value: bool,
    passing: RuntimeFunctionParameterPassing,
    name: &str,
) -> (
    RuntimePlan,
    RuntimeTaskPlanCoordinateOwner,
    RuntimeFunctionSiteId,
) {
    let mut builder = RuntimePlanBuilder::new();
    let boolean = RuntimeSemanticTypeId::from_bytes([61; 32]);
    let parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([72; 32]);
    let batch = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                boolean,
                RuntimePlanTypeProjection::Bool,
            )],
            [RuntimeLocalDeclarationSeed::new(
                RuntimeLocalOrigin::Parameter(parameter),
                boolean,
            )],
        )
        .unwrap();
    let local = batch.local_ids()[0].clone();
    if padding {
        builder
            .push_function_site_seed(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([99; 32]),
                RuntimeFunctionSemanticRole::Ordinary,
                [],
                RuntimeExprSeed::new(
                    boolean,
                    RuntimeExprSeedKind::Value(RuntimeValue::Bool(false)),
                ),
            )
            .unwrap();
    }
    let flow = FlowRuntimeId::canonical("controller").unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: vec![RuntimeFlowExecutableParameter {
                identity: parameter,
                coordinate: FlowParameterCoordinate::from_position(0),
                name: name.to_owned(),
                mode: RuntimeFlowParameterMode::Owned,
                passing,
                semantic_identity: boolean,
            }],
        })
        .unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([71; 32]),
                None,
                Box::new([RuntimeFunctionInputBindingSeed {
                    transfer: RuntimeFunctionInputTransfer::Formal,
                    origin: RuntimeFunctionInputOrigin::Parameter(parameter),
                    source: RuntimeFunctionInputSource::Parameter {
                        position: 0,
                        passing,
                    },
                    input_local: local.clone(),
                    pattern: RuntimePatternSeed::new(
                        boolean,
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local,
                        },
                    ),
                    ownership: RuntimeFunctionInputOwnershipRequirement::Owned,
                    unrestricted_bindings: Box::new([]),
                }]),
                boolean,
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                    boolean,
                    RuntimeExprSeedKind::Value(RuntimeValue::Bool(value)),
                ))]),
            },
        ))
        .unwrap();
    admit_controller_entry(&mut builder, flow);
    let owner = builder.task_coordinate_owner(0);
    let plan = builder.finish().unwrap();
    let function = plan.flows()[0].function_site();
    (plan, owner, function)
}
