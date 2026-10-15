use super::*;
use crate::engine::{AwaitManyStateRollbackImage, Engine, NativeEngineRollbackImage};
use crate::entry::RuntimeSchemaLimits;
use crate::plan::*;
use crate::task::*;
use crate::value::{AwbcRuntimeValueSnapshot, RuntimeValue};

fn fanout_plan() -> (RuntimePlan, FlowRuntimeId) {
    let boolean = crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest();
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let sequence = crate::pattern::RuntimeSemanticTypeId::from_bytes(
        *blake3::hash(b"arcweft.fixture.native-await-many.seq-bool.v1\0").as_bytes(),
    );
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    sequence,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Seq,
                        item: boolean,
                    },
                ),
            ],
            [RuntimeLocalDeclarationSeed::new(
                RuntimeLocalDeclarationSource::Binding {
                    identity: *blake3::hash(b"arcweft.fixture.native-await-many.item.v1\0")
                        .as_bytes(),
                    declaration: RuntimeLocalBindingDeclaration::new(
                        RuntimeLocalBindingKind::PatternBinding,
                        false,
                        RuntimeLocalBindingStorage::Derived,
                    ),
                },
                boolean,
            )],
        )
        .unwrap();
    let base_coordinate = builder
        .task_plan_construction()
        .reserve_original_source()
        .unwrap();
    let child_coordinate = builder
        .task_plan_construction()
        .reserve_original_source()
        .unwrap();
    let template = |coordinate,
                    family,
                    payload: crate::pattern::RuntimeSemanticTypeId,
                    identity| {
        RuntimeNeedProducerTemplateSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([identity; 32]),
            family,
            contract: NeedProducerContractDigest::from_bytes([11; 32]),
            plan: coordinate,
            producer_site: NeedProducerSiteDigest::from_bytes([identity; 32]),
            payload_type: RuntimeTypeSemanticDigest::from_bytes(*payload.as_bytes()),
            class: TaskClass::Cpu,
            priority: TaskPriority(0),
            cancel_scope: CancelScopeId("native-await-many".into()),
            policy: TaskPolicy::JoinSameKey,
            request: RuntimeHostTaskRequestTemplateSeed {
                capability: HostCapabilityId("fixture".into()),
                operation: "run".into(),
                args: vec![],
            },
            debug_label: "native-await-many".into(),
        }
    };
    let flow = FlowRuntimeId::canonical("native-await-many").unwrap();
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
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([99; 32]),
                None,
                Box::new([]),
                unit,
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: vec![RuntimeFlowOpSeed::AwaitMany {
                    binding: None,
                    target: RuntimeAwaitManyTargetSeed {
                        source: RuntimeExprSeed::new(
                            sequence,
                            RuntimeExprSeedKind::Value(crate::value::runtime_sequence_values(
                                vec![RuntimeValue::Bool(true); 3],
                            )),
                        ),
                        item_binding: admitted.local_ids()[0].clone(),
                        limit: 2,
                        base: template(
                            base_coordinate.clone(),
                            NeedProducerFamily::AwaitManyBase,
                            sequence,
                            21,
                        ),
                        child: template(
                            child_coordinate.clone(),
                            NeedProducerFamily::AwaitManyChild,
                            boolean,
                            22,
                        ),
                    },
                    pending: vec![],
                }]
                .into_boxed_slice(),
            },
        ))
        .unwrap();
    let plan = builder.finish().unwrap();
    let (base, _) = plan.task_plan(&base_coordinate).unwrap();
    let (child, _) = plan.task_plan(&child_coordinate).unwrap();
    assert_eq!(
        plan.control_effect_contracts()
            .get(base.control_effect())
            .unwrap()
            .children(),
        &[child.control_effect()]
    );
    (plan, flow)
}

fn suspended_engine() -> Engine {
    let (plan, flow) = fanout_plan();
    let mut engine = Engine::for_flow(plan, &flow).unwrap();
    for _ in 0..8 {
        let output = engine.step(
            crate::step::RuntimeStepInput::default(),
            crate::step::RuntimeStepOptions::default(),
        );
        assert!(output.output.diagnostics.is_empty(), "{output:?}");
        if matches!(engine.fiber.frame.status, FlowFiberStatus::WaitingMany(_)) {
            return engine;
        }
    }
    panic!("accepted AwaitMany must suspend with its real child journal receipts");
}

fn state(image: &mut NativeEngineRollbackImage) -> &mut AwaitManyStateRollbackImage {
    let FlowFiberStatus::WaitingMany(WaitingManyStatus::Native(state)) =
        &mut image.fiber.frame.status
    else {
        panic!("native scheduler retained the original bounded fanout")
    };
    state
}

#[test]
fn native_fanout_preflight_rejects_corrupt_frontiers_indices_and_results() {
    let engine = suspended_engine();
    let owner = RuntimeProgramOwner::Plan(engine.program_plan());
    let image = engine.inert_rollback_image().unwrap();
    for case in 0..9 {
        let mut corrupt = image.clone();
        let state = state(&mut corrupt);
        assert_eq!(state.next_index, 2);
        assert_eq!(state.in_flight.len(), 2);
        match case {
            0 => state.next_index = 4,
            1 => {
                state.results.pop();
            }
            2 => state.in_flight[1].index = state.in_flight[0].index,
            3 => state.results[0] = Some(AwbcRuntimeValueSnapshot::Bool(true)),
            4 => state.in_flight.clear(),
            5 => state.results[2] = Some(AwbcRuntimeValueSnapshot::Bool(true)),
            6 => state.items[0] = AwbcRuntimeValueSnapshot::Unit,
            7 => state.captured.push(AwbcRuntimeValueSnapshot::Bool(true)),
            8 => {
                state.source_context = crate::value::RuntimeScopeTypeContext::Function {
                    site: crate::runtime_id::RuntimeFunctionSiteId::from_accepted_ordinal(
                        std::num::NonZeroU32::new(u32::MAX).unwrap(),
                    ),
                    instantiation: None,
                };
            }
            _ => unreachable!(),
        }
        assert!(
            Engine::from_rollback_image(corrupt, &owner).is_err(),
            "case {case}"
        );
    }
    let restored = Engine::from_rollback_image(image.clone(), &owner).unwrap();
    assert_eq!(restored.inert_rollback_image().unwrap(), image);
}

#[test]
fn native_fanout_values_borrow_the_enclosing_resource_bank() {
    let engine = suspended_engine();
    let owner = RuntimeProgramOwner::Plan(engine.program_plan());
    let mut image = engine.inert_rollback_image().unwrap();
    let FlowFiberStatus::WaitingMany(WaitingManyStatus::Native(live)) = &engine.fiber.frame.status
    else {
        unreachable!()
    };
    let inert = state(&mut image);
    let limits = RuntimeSchemaLimits {
        // Three complete Need descriptors (30 each), plus three source Bools.
        max_nodes: 93,
        ..RuntimeSchemaLimits::engine_default()
    };
    for state in [live.as_ref() as &dyn SuspendedValues, inert] {
        let mut exact = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        state.admit_suspended_values(&mut exact, None).unwrap();
        let mut shared = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        shared.validate(&RuntimeValue::Bool(false)).unwrap();
        let error = state.admit_suspended_values(&mut shared, None).unwrap_err();
        assert!(matches!(
            error,
            AwbcRuntimeValueSnapshotError::Resource(
                crate::entry::RuntimeSchemaError::BudgetExceeded { budget: "nodes" }
            )
        ));
        assert_eq!(
            state.admit_suspended_values(&mut shared, None).unwrap_err(),
            error
        );
    }
}
