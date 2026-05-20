use crate::{engine::*, pattern::*, plan::*, source::*, step::*, task::*, value::*};

#[test]
fn source_policy_is_pure_data() {
    let policy = SourcePolicy {
        backpressure: BackpressurePolicy::BoundedQueue {
            capacity: 8,
            on_overflow: OverflowPolicy::Coalesce,
        },
        replay: ReplayPolicy::HashOnly,
        privacy: PrivacyPolicy::Transient,
        max_queue: 8,
    };

    assert!(matches!(
        policy.backpressure,
        BackpressurePolicy::BoundedQueue {
            capacity: 8,
            on_overflow: OverflowPolicy::Coalesce,
        }
    ));
    assert_eq!(policy.replay, ReplayPolicy::HashOnly);
    assert_eq!(policy.privacy, PrivacyPolicy::Transient);
}

#[test]
fn normalizes_source_events_by_source_and_sequence() {
    let events: Vec<RuntimeSourceEvent> = vec![
        SourceEvent {
            source: SourceId("source.b".to_owned()),
            sequence: TaskSequence(2),
            kind: SourceEventKind::Item("b2".into()),
        },
        SourceEvent {
            source: SourceId("source.a".to_owned()),
            sequence: TaskSequence(9),
            kind: SourceEventKind::Item("a9".into()),
        },
        SourceEvent {
            source: SourceId("source.a".to_owned()),
            sequence: TaskSequence(1),
            kind: SourceEventKind::Item("a1".into()),
        },
    ];

    let normalized = normalize_source_events(events);
    let keys = normalized
        .iter()
        .map(|event| (event.source.0.as_str(), event.sequence))
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        vec![
            ("source.a", TaskSequence(1)),
            ("source.a", TaskSequence(9)),
            ("source.b", TaskSequence(2)),
        ]
    );
}

#[test]
fn source_runtime_latest_backpressure_keeps_latest_item() {
    let mut state = SourceRuntimeState::new(
        SourceId("source.camera".to_owned()),
        SourcePolicy {
            backpressure: BackpressurePolicy::LatestOnly,
            replay: ReplayPolicy::HashOnly,
            privacy: PrivacyPolicy::Transient,
            max_queue: 1,
        },
    );

    state.apply_event(SourceEvent {
        source: SourceId("source.camera".to_owned()),
        sequence: TaskSequence(0),
        kind: SourceEventKind::Item("old".into()),
    });
    state.apply_event(SourceEvent {
        source: SourceId("source.camera".to_owned()),
        sequence: TaskSequence(1),
        kind: SourceEventKind::Item("new".into()),
    });

    assert_eq!(
        state
            .queue
            .into_iter()
            .map(|item| item.label())
            .collect::<Vec<_>>(),
        vec!["new"]
    );
}

#[test]
fn engine_records_source_events_without_running_adapters() {
    let source = SourcePlan {
        id: SourceId("source.camera".to_owned()),
        item_ty: "Frame".to_owned(),
        error_ty: "CaptureError".to_owned(),
        from: RuntimeExpr::Local("camera".to_owned()),
        policy: SourcePolicy::default(),
        handlers: Vec::new(),
    };
    let plan = RuntimePlan::new(None, Vec::new(), Vec::new())
        .expect("empty plan is valid")
        .with_generation_plans(Vec::new(), vec![source]);
    let mut engine = Engine::new(plan);

    let output = super::runtime_step(
        &mut engine,
        RuntimeStepInput {
            source_events: vec![SourceEvent {
                source: SourceId("source.camera".to_owned()),
                sequence: TaskSequence(0),
                kind: SourceEventKind::Item("frame0".into()),
            }],
            ..RuntimeStepInput::default()
        },
    );

    assert_eq!(output.effects.source_events.len(), 1);
    assert_eq!(
        engine
            .fiber()
            .source_states
            .get(&SourceId("source.camera".to_owned()))
            .expect("source state exists")
            .queue
            .iter()
            .map(RuntimePayload::label)
            .collect::<Vec<_>>(),
        vec!["frame0".to_owned()]
    );
}

#[test]
fn source_handler_yield_controls_source_queue() {
    let source_id = SourceId("source.camera".to_owned());
    let source = SourcePlan {
        id: source_id.clone(),
        item_ty: "Frame".to_owned(),
        error_ty: "CaptureError".to_owned(),
        from: RuntimeExpr::Local("camera".to_owned()),
        policy: SourcePolicy {
            backpressure: BackpressurePolicy::LatestOnly,
            replay: ReplayPolicy::HashOnly,
            privacy: PrivacyPolicy::Transient,
            max_queue: 1,
        },
        handlers: vec![SourceHandlerPlan::Item {
            pattern: RuntimePattern::Ident("frame".to_owned()),
            ops: vec![SourceOp::Yield(RuntimeExpr::Local("frame".to_owned()))],
        }],
    };
    let plan = RuntimePlan::new(None, Vec::new(), Vec::new())
        .expect("empty plan is valid")
        .with_generation_plans(Vec::new(), vec![source]);
    let mut engine = Engine::new(plan);

    super::runtime_step(
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
        engine
            .fiber()
            .source_states
            .get(&source_id)
            .expect("source state exists")
            .queue
            .iter()
            .map(RuntimePayload::label)
            .collect::<Vec<_>>(),
        vec!["frame0".to_owned()]
    );
}
