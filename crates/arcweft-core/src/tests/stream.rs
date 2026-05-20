use crate::{engine::*, pattern::*, plan::*, source::*, step::*, stream::*, task::*, value::*};

#[test]
fn stream_plan_drains_source_queue_and_emits_stream_items() {
    let source_id = SourceId("source.camera".to_owned());
    let stream_id = StreamRuntimeId("stream.rms".to_owned());
    let source = SourcePlan {
        id: source_id.clone(),
        item_ty: "Frame".to_owned(),
        error_ty: "CaptureError".to_owned(),
        from: RuntimeExpr::Local("camera".to_owned()),
        policy: SourcePolicy {
            backpressure: BackpressurePolicy::BoundedQueue {
                capacity: 4,
                on_overflow: OverflowPolicy::Error,
            },
            replay: ReplayPolicy::HashOnly,
            privacy: PrivacyPolicy::Transient,
            max_queue: 4,
        },
        handlers: vec![SourceHandlerPlan::Item {
            pattern: RuntimePattern::Ident("frame".to_owned()),
            ops: vec![SourceOp::Yield(RuntimeExpr::Local("frame".to_owned()))],
        }],
    };
    let stream = StreamPlan {
        id: stream_id.clone(),
        item_ty: "Frame".to_owned(),
        error_ty: "CaptureError".to_owned(),
        ops: vec![StreamOp::ForNext {
            pattern: RuntimePattern::Ident("frame".to_owned()),
            source: RuntimeExpr::EntityRef(source_id.0.clone()),
            body: vec![StreamOp::Yield {
                expr: RuntimeExpr::Local("frame".to_owned()),
            }],
        }],
    };
    let plan = RuntimePlan::new(None, Vec::new(), Vec::new())
        .expect("empty plan is valid")
        .with_generation_plans(vec![stream], vec![source]);
    let mut engine = Engine::new(plan);

    let output = super::runtime_step(
        &mut engine,
        RuntimeStepInput {
            source_events: vec![SourceEvent {
                source: source_id.clone(),
                sequence: TaskSequence(0),
                kind: SourceEventKind::Item("frame0".into()),
            }],
            ..RuntimeStepInput::default()
        },
    );

    assert_eq!(
        output.effects.stream_events,
        vec![StreamEvent {
            stream: stream_id.clone(),
            sequence: TaskSequence(0),
            kind: SourceEventKind::Item("frame0".into()),
        }]
    );
    assert!(
        engine
            .fiber()
            .source_states
            .get(&source_id)
            .expect("source state exists")
            .queue
            .is_empty()
    );
    assert_eq!(
        engine
            .fiber()
            .stream_states
            .get(&stream_id)
            .expect("stream state exists")
            .queue
            .iter()
            .map(RuntimePayload::label)
            .collect::<Vec<_>>(),
        vec!["frame0".to_owned()]
    );
}
