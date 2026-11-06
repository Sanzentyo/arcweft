use super::*;
use arcweft_core::awbc::schema::{AwbcEffectKind, AwbcEffectPayload};
use arcweft_core::effect::{RuntimeEffectExpr, RuntimeRegistryReadKind};
use arcweft_core::engine::{FlowExit, FlowFiberStatus};
use arcweft_core::executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor};
use arcweft_core::pattern::RuntimeCheckedType;
use arcweft_core::plan::{
    RuntimeEffectSet, RuntimeEvaluatedEffectSeed, RuntimeExecutableBodySeed,
    RuntimeFunctionDefinitionIdentity, RuntimeFunctionSiteDeclarationSeed,
};
use arcweft_core::step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepStopReason};
use arcweft_id::{DeclarationName, EffectId, LifetimeRegistryKey, LifetimeScopeKind};

#[test]
fn registry_read_and_write_lowering_preserve_the_original_key_and_type() {
    let flow = flow_id("registry");
    let key = LifetimeRegistryKey::new(
        LifetimeScopeKind::Flow,
        ["flags", "既読"]
            .into_iter()
            .map(|name| DeclarationName::try_new(name).unwrap())
            .collect(),
    );
    let effects = RuntimeEffectSet::try_from_effects([EffectId::state_write(&key)]).unwrap();
    let boolean = RuntimeCheckedType::Bool.semantic_identity_digest();
    let value = |flag| {
        RuntimeExprSeed::new(
            boolean,
            RuntimeExprSeedKind::Value(RuntimeValue::Bool(flag)),
        )
    };
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                boolean,
                RuntimePlanTypeProjection::Bool,
            )],
            [],
        )
        .unwrap();
    builder.push_flow_schema(flow_schema(&flow)).unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([61; 32]),
                None,
                Box::new([]),
                boolean,
                effects.clone(),
            ),
            RuntimeExecutableBodySeed {
                effects,
                ops: Box::new([
                    RuntimeFlowOpSeed::EvaluatedEffect(RuntimeEvaluatedEffectSeed::RegistryWrite {
                        key: key.clone(),
                        value: value(true),
                    }),
                    RuntimeFlowOpSeed::EvaluatedEffect(RuntimeEvaluatedEffectSeed::RegistryWrite {
                        key: key.clone(),
                        value: value(false),
                    }),
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        boolean,
                        RuntimeExprSeedKind::RegistryRead {
                            key: key.clone(),
                            kind: RuntimeRegistryReadKind::Required,
                        },
                    )),
                ]),
            },
        ))
        .unwrap();
    builder
        .push_flow_executable(flow_executable(&flow))
        .unwrap();
    builder
        .push_entry(flow_entry("registry", flow.clone()))
        .unwrap();
    let plan = builder.finish().unwrap();
    let operation = plan.flows()[0]
        .body()
        .ops()
        .iter()
        .find_map(|op| match op {
            arcweft_core::plan::FlowOp::EvaluatedEffect(effect) => Some(effect),
            _ => None,
        })
        .unwrap();
    let mut unbound = AwbcInventory::new(AwbcLowerOptions::default());
    unbound.intern_runtime_primitives();
    assert!(
        unbound
            .intern_evaluated_effect(operation, plan.flows()[0].body().effects())
            .is_none()
    );
    assert!(unbound.program.effect_plans.is_empty());
    assert_eq!(
        unbound.take_diagnostics()[0].path,
        "effect.registry_value_type"
    );
    let report = lower_plan(&plan);
    let program = &report.program;
    assert_eq!(
        program.effect_plans.len(),
        1,
        "one typed plan is interned for both RHS expressions"
    );
    let effect = &program.effect_plans[0];
    assert_eq!(effect.kind(), AwbcEffectKind::RegistryWrite);
    let AwbcEffectPayload::RegistryWrite {
        key: actual,
        value_type,
    } = &effect.payload
    else {
        panic!("registry payload was erased")
    };
    assert_eq!(actual, &key);
    assert_eq!(
        program.runtime_types[value_type.index()].semantic_identity(),
        boolean
    );
    let signature = &program.signatures[effect.signature.index()];
    assert_eq!(signature.params.as_slice(), &[*value_type]);
    assert!(signature.result.is_none());
    assert_eq!(
        program.strings[effect.capability.unwrap().index()],
        "state.write"
    );
    let declared = &program.effect_sets[signature.effects.index()].effects;
    assert_eq!(declared.len(), 1);
    assert_eq!(
        program.strings[declared[0].index()],
        EffectId::state_write(&key).as_str()
    );
    let emits = program
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            AwbcInstruction::EmitEffect { effect, args } => Some((*effect, args)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(emits.len(), 2);
    assert_eq!(emits[0].0, emits[1].0);
    assert_eq!(emits[0].1.len(), 1);
    assert_eq!(emits[1].1.len(), 1);
    // Static addresses never become value operands; each original RHS does.
    for op in plan.flows()[0].body().ops() {
        if let arcweft_core::plan::FlowOp::EvaluatedEffect(RuntimeEffectExpr::RegistryWrite {
            value,
            ..
        }) = op
        {
            assert_eq!(
                plan.type_table()
                    .get(value.ty())
                    .unwrap()
                    .semantic_identity(),
                boolean
            );
        }
    }
    assert!(program.instructions.iter().any(|instruction| matches!(
        instruction,
        AwbcInstruction::ReadRegistry { key: read, kind: RuntimeRegistryReadKind::Required, .. }
            if read == &key
    )));

    let decoded =
        AwbcProgram::decode_canonical(&program.encode_canonical().unwrap(), Default::default())
            .unwrap();
    let native = ArcweftRuntimeExecutor::from_runtime_flow_invocation(
        plan.seal_flow_invocation(flow, []).unwrap(),
        ArcweftExecutionTier::RuntimePlanVm,
    )
    .unwrap();
    let product = ArcweftRuntimeExecutor::from_awbc_product(decoded, AwbcEntryId(0)).unwrap();
    for mut executor in [native, product] {
        for _ in 0..16 {
            let step = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
            assert!(step.output.diagnostics.is_empty(), "{step:?}");
            if step.stop_reason == RuntimeStepStopReason::Done {
                break;
            }
        }
        assert_eq!(
            executor.fiber().frame.status,
            FlowFiberStatus::Done(FlowExit::Return("false".into()))
        );
    }
}
