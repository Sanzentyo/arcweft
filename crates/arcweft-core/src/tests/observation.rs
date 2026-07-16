use crate::{effect::*, plan::*, step::*};

#[test]
fn runtime_records_log_signal_metric_and_event_observations() {
    let effects = vec![
        LineEffectRequest::Log(RuntimeLog {
            level: "info".to_owned(),
            message: "entered".to_owned(),
            fields: Vec::new(),
        }),
        LineEffectRequest::SignalWrite(RuntimeAssignment {
            target: "signal.current_flow".to_owned(),
            value: "flow.opening".to_owned(),
        }),
        LineEffectRequest::MetricWrite(RuntimeAssignment {
            target: "metric.frame_time_ms".to_owned(),
            value: "16".to_owned(),
        }),
        LineEffectRequest::EmitEvent(RuntimeEvent {
            event: "event.flow_entered".to_owned(),
            fields: Vec::new(),
        }),
    ];
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.opening")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.opening"),
            ops: effects.into_iter().map(FlowOp::Effect).collect(),
        }],
        Vec::new(),
    )
    .expect("flow plan is valid");
    let mut engine = super::engine_for_test_plan(plan);

    for _ in 0..4 {
        super::runtime_step(&mut engine, RuntimeStepInput::default());
    }

    let observations = &engine.fiber().observations;
    assert_eq!(observations.logs.len(), 1);
    assert_eq!(
        observations.signals.get("signal.current_flow"),
        Some(&"flow.opening".to_owned())
    );
    assert_eq!(
        observations.metrics.get("metric.frame_time_ms"),
        Some(&"16".to_owned())
    );
    assert_eq!(observations.events.len(), 1);
}
