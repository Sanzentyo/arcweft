use super::*;
use arcweft_core::awbc::schema::{AwbcEffectKind, AwbcEffectPayload};
use arcweft_core::effect::RuntimeEffectExpr;
use arcweft_core::plan::{
    RuntimeEffectSet, RuntimeEvaluatedEffectSeed, RuntimeExecutableBodySeed,
    RuntimeFunctionDefinitionIdentity, RuntimeFunctionSiteDeclarationSeed,
};
use arcweft_id::{DeclarationName, EffectId, LifetimeRegistryKey, LifetimeScopeKind};

#[test]
fn registry_write_lowering_retains_original_value_type_key_and_effect_row() {
    let flow = flow_id("registry");
    let key = LifetimeRegistryKey::new(
        LifetimeScopeKind::Flow,
        ["flags", "既読"]
            .into_iter()
            .map(|name| DeclarationName::try_new(name).unwrap())
            .collect(),
    );
    let effects = RuntimeEffectSet::try_from_effects([EffectId::state_write(&key)]).unwrap();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(type_id(2), RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(type_id(3), RuntimePlanTypeProjection::Bool),
            ],
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
                type_id(2),
                effects.clone(),
            ),
            RuntimeExecutableBodySeed {
                effects,
                ops: Box::new([
                    RuntimeFlowOpSeed::EvaluatedEffect(RuntimeEvaluatedEffectSeed::RegistryWrite {
                        key: key.clone(),
                        value: bool_expr(true),
                    }),
                    RuntimeFlowOpSeed::EvaluatedEffect(RuntimeEvaluatedEffectSeed::RegistryWrite {
                        key: key.clone(),
                        value: bool_expr(false),
                    }),
                    RuntimeFlowOpSeed::ReturnExpr(unit_expr()),
                ]),
            },
        ))
        .unwrap();
    builder
        .push_flow_executable(flow_executable(&flow))
        .unwrap();
    builder.push_entry(flow_entry("registry", flow)).unwrap();
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
        type_id(3)
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
                type_id(3)
            );
        }
    }
}
