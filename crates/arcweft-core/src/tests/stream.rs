//! Stream plan construction through the sole typed runtime-plan builder.

use crate::{
    engine::Engine,
    pattern::RuntimeSemanticTypeId,
    plan::{
        RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimeStreamOpSeed,
        RuntimeStreamPlanSeed,
    },
    step::{RuntimeStepInput, RuntimeStepOptions},
    stream::StreamRuntimeId,
};

const STRING_TYPE_MARKER: u8 = 1;

fn string_type() -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([STRING_TYPE_MARKER; 32])
}

#[test]
fn typed_stream_plan_is_admitted_and_runs_without_a_flow() {
    let stream_id =
        StreamRuntimeId::from_source_entity_body("stream.rms").expect("stream source ID lowers");
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [RuntimePlanTypeSeed::new(
                string_type(),
                RuntimePlanTypeProjection::String,
            )],
            [],
            [],
            [],
        )
        .expect("typed stream scalar admission");
    builder
        .push_stream_plan_seed(RuntimeStreamPlanSeed {
            id: stream_id.clone(),
            item_ty: string_type(),
            error_ty: string_type(),
            ops: vec![RuntimeStreamOpSeed::Return],
        })
        .expect("typed stream admission");
    let plan = builder.finish().expect("typed stream plan is valid");

    assert_eq!(plan.stream_plans().len(), 1);
    let mut engine = Engine::new(plan);
    let output = engine
        .step(RuntimeStepInput::default(), RuntimeStepOptions::default())
        .output;

    assert!(output.effects.stream_events.is_empty());
    assert!(engine.fiber().stream_states.contains_key(&stream_id));
}
