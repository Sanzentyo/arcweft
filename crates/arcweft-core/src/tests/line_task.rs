use super::call;
use crate::{effect::*, engine::*, line_task::*, plan::*, step::*, task::*};

#[test]
fn engine_steps_line_task_groups_as_sans_io_effects() {
    let group = LineTaskGroup {
        root: LineTaskScope {
            node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("line_start"))]),
            defer_stack: vec![vec![call("line_defer")]],
            completed_defer_stack: vec![vec![call("line_completed")]],
            ..LineTaskScope::default()
        },
        ..LineTaskGroup::default()
    };
    let mut engine = Engine::new(RuntimePlan::lines_only(vec![group]));

    let output = super::runtime_step(&mut engine, RuntimeStepInput::default());

    assert_eq!(
        output.effects.line,
        vec![
            call("line_start"),
            call("line_defer"),
            call("line_completed")
        ]
    );
    assert_eq!(engine.fiber().status, FlowFiberStatus::Done(FlowExit::Done));
}

#[test]
fn line_cancel_rule_replaces_normal_line_body() {
    let group = LineTaskGroup {
        root: LineTaskScope {
            node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("normal"))]),
            cancelled_defer_stack: vec![vec![call("cancel_cleanup")]],
            ..LineTaskScope::default()
        },
        cancel_rules: vec![LineCancelRuleRequest {
            trigger: "input .SkipLine".to_owned(),
            action: vec![LineEffectRequest::Out(LineOutRequest {
                label: None,
                value: ".Skipped".to_owned(),
            })],
        }],
        ..LineTaskGroup::default()
    };
    let output = run_line_task_group_for_input(
        &group,
        &RuntimeStepInput {
            input_events: vec![InputEvent {
                kind: "input".to_owned(),
                payload: Some(".SkipLine".to_owned()),
            }],
            ..RuntimeStepInput::default()
        },
    );

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::LineCancelled {
            trigger: "input .SkipLine".to_owned()
        }]
    );
    assert_eq!(
        output.effects.line,
        vec![
            LineEffectRequest::Out(LineOutRequest {
                label: None,
                value: ".Skipped".to_owned()
            }),
            call("cancel_cleanup")
        ]
    );
}

#[test]
fn child_task_triggers_emit_task_request_and_scoped_body() {
    let child = LineChildTask {
        id: TaskId("line.task.0.mark".to_owned()),
        key: Some(TaskKey("line.task.mark".to_owned())),
        name: Some("mark".to_owned()),
        trigger: LineTaskTrigger::Mark(".seen".to_owned()),
        priority: TaskPriority(7),
        join_policy: ChildJoinPolicy::Join,
        cancel_policy: ChildCancelPolicy::CancelAndJoin,
        scope: Box::new(LineTaskScope {
            node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("handler"))]),
            defer_stack: vec![vec![call("handler_defer")]],
            ..LineTaskScope::default()
        }),
    };
    let group = LineTaskGroup {
        root: LineTaskScope {
            node: LineTaskNode::Seq(vec![LineTaskNode::Child(child)]),
            ..LineTaskScope::default()
        },
        ..LineTaskGroup::default()
    };
    let input = RuntimeStepInput {
        input_events: vec![InputEvent {
            kind: "mark".to_owned(),
            payload: Some(".seen".to_owned()),
        }],
        ..RuntimeStepInput::default()
    };

    let output = run_line_task_group(&group, &input, ScopeExit::Completed);

    assert_eq!(output.requests.tasks.len(), 1);
    assert_eq!(output.requests.tasks[0].priority, TaskPriority(7));
    assert_eq!(output.requests.tasks[0].debug_label, "line_task.run_child");
    assert!(matches!(
        &output.requests.tasks[0].request,
        HostTaskRequest::Custom {
            capability,
            operation,
            args
        } if capability.0 == "line_task" && operation == "run_child" && args.len() == 1
    ));
    assert_eq!(
        output.effects.line,
        vec![call("handler"), call("handler_defer")]
    );
}
