//! Actual compiler Source and Core retirement cross the host publication fence.

use super::*;
use arcweft_compiler::source::compile_source;
use arcweft_core::{
    effect::LineEffectRequest,
    engine::{FlowExit, FlowFiberStatus},
    executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor},
    plan::RuntimeEntryTarget,
    step::{RuntimeStepInput, RuntimeStepOptions},
    task::ProgramTaskTerminal,
};

fn runtime() -> ArcweftRuntimeExecutor {
    let compiled = compile_source(
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String effects { control.spawn, log.write } {
    thread worker {
        thread descendant { log.info("descendant") }
        log.info("worker")
    }
    return "done"
}
"#,
    )
    .expect("actual nested Thread source compiles");
    let [entry] = compiled.plan.entries() else {
        panic!("one entry")
    };
    let RuntimeEntryTarget::Flow(flow) = &entry.target else {
        panic!("Flow entry")
    };
    let flow = flow.clone();
    ArcweftRuntimeExecutor::from_runtime_flow_invocation(
        compiled.plan.seal_flow_invocation(flow, []).unwrap(),
        ArcweftExecutionTier::RuntimePlanVm,
    )
    .unwrap()
}

fn retire(runtime: &mut ArcweftRuntimeExecutor) -> Vec<arcweft_core::task::ProgramTaskCompletion> {
    let mut reports = Vec::new();
    for _ in 0..128 {
        let step = runtime.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(step.stats.executed_ops <= 1);
        assert!(step.output.diagnostics.is_empty(), "{:?}", step.output);
        reports.extend(step.output.requests.program_task_completions);
        if matches!(runtime.fiber().frame.status, FlowFiberStatus::Done(_)) {
            assert_eq!(reports.len(), 2);
            return reports;
        }
    }
    panic!("foreign original executable did not retire its bodies");
}

#[test]
fn actual_program_body_retirement_is_required_and_duplicate_batch_is_atomic() {
    let mut runtime = runtime();
    let owner = runtime.program_owner();
    let mut foreign = self::runtime();
    let foreign_owner = foreign.program_owner();
    let foreign_reports = retire(&mut foreign);
    let mut bridge =
        NativeTaskBridge::try_with_registry(HostCallPolicy::new(), HostAdapterRegistry::new())
            .unwrap();
    let mut events = Vec::new();
    let mut submissions = 0;
    let mut terminals = Vec::new();
    let mut logs = Vec::new();
    for step in 0..128 {
        let expected_diagnostics = events
            .iter()
            .map(|event: &TaskEvent| {
                arcweft_core::step::RuntimeDiagnostic::new(format!(
                    "task {} sequence {} delivered",
                    event.correlation.task_id, event.cursor.sequence.0,
                ))
            })
            .collect::<Vec<_>>();
        let result = runtime.step(
            RuntimeStepInput {
                task_events: std::mem::take(&mut events),
                ..RuntimeStepInput::default()
            },
            RuntimeStepOptions::default(),
        );
        assert!(result.stats.executed_ops <= 1);
        assert_eq!(
            result.output.diagnostics, expected_diagnostics,
            "{:?}",
            result.output
        );
        assert_eq!(result.stats.task_events_in, expected_diagnostics.len());
        logs.extend(
            result
                .output
                .effects
                .line
                .iter()
                .filter_map(|effect| match effect {
                    LineEffectRequest::Log(log) => Some(log.message.clone()),
                    _ => None,
                }),
        );
        let tasks = result.output.requests.tasks;
        if !tasks.is_empty() {
            let before = bridge.stats();
            assert!(
                bridge
                    .complete_tasks(foreign_owner.clone(), tasks.clone())
                    .is_err()
            );
            assert_eq!(bridge.stats(), before);
            assert!(tasks.iter().all(|task| matches!(
                task.spec().request,
                arcweft_core::task::TaskRequest::Program(_)
            )));
            let count = tasks.len();
            let foreign_report = foreign_reports
                .iter()
                .find(|report| report.handle() == tasks[0].handle())
                .expect(
                    "equal source independently issues the same deterministic task coordinates",
                );
            let task_id = tasks[0].task_id();
            let before_terminal_count = bridge.stats().completed_tasks;
            events.extend(
                bridge
                    .complete_tasks_with_generation(
                        owner.clone(),
                        runtime.generation(),
                        LogicalEpoch(step),
                        tasks,
                    )
                    .unwrap(),
            );
            submissions += count;
            assert!(events.is_empty(), "dispatch cannot finish a program body");
            assert_eq!(bridge.stats().completed_tasks, before_terminal_count);
            let before = bridge.stats();
            assert!(matches!(
                bridge.complete_program_tasks(std::slice::from_ref(foreign_report)),
                Err(NativeTaskBridgeError::TaskCompletion(TaskCompletionError::DispatchMismatch { task_id: rejected })) if rejected == task_id
            ));
            assert_eq!(bridge.stats(), before);
        }
        let reports = result.output.requests.program_task_completions;
        if let Some(first) = reports.first() {
            let before = bridge.stats();
            let dispatches = bridge.dispatches.clone();
            assert!(
                bridge
                    .complete_program_tasks(&[first.clone(), first.clone()])
                    .is_err()
            );
            assert_eq!(bridge.stats(), before);
            assert_eq!(bridge.dispatches, dispatches);
            events.extend(bridge.complete_program_tasks(&reports).unwrap());
            assert!(events.iter().all(|event| matches!(&event.kind, TaskEventKind::Ready(payload) if payload.value() == &RuntimeValue::Unit)));
            assert!(
                reports
                    .iter()
                    .all(|report| matches!(report.terminal(), ProgramTaskTerminal::Completed))
            );
            terminals.extend(reports);
        }
        if let FlowFiberStatus::Done(exit) = &runtime.fiber().frame.status {
            assert_eq!(exit, &FlowExit::Return("done".into()));
            assert_eq!(submissions, 2);
            assert_eq!(terminals.len(), 2);
            assert_eq!(bridge.stats().scheduler.in_flight, 0);
            assert_eq!(bridge.stats().scheduler.completed, 2);
            assert_eq!(bridge.stats().program_task_terminals, 2);
            assert!(logs.iter().any(|log| log == "descendant"));
            assert!(logs.iter().any(|log| log == "worker"));
            assert_eq!(result.stats.child_fibers, 0);
            let before = bridge.stats();
            assert!(bridge.complete_program_tasks(&terminals).is_err());
            assert_eq!(bridge.stats(), before);
            return;
        }
    }
    panic!("actual nested Thread bodies did not retire");
}
