use super::Engine;
use crate::{
    plan::{FlowOp, RuntimeFunctionSiteBodyKind},
    pure::{RuntimePureCallBackend, VmRuntimePureCallBackend},
    step::{RuntimeStepInput, RuntimeStepOutput},
    tests::function_application::returning_function_plan,
    value::RuntimeValue,
};

fn main_with_unadmitted_value() -> Engine {
    let plan = returning_function_plan(RuntimeFunctionSiteBodyKind::Executable);
    let local = plan
        .function_sites()
        .iter_with_ids()
        .flat_map(|(_, function)| function.inputs())
        .next()
        .unwrap()
        .input_local();
    let mut engine = Engine::new(plan);
    engine
        .fiber
        .frame
        .env
        .set_root(local, RuntimeValue::String("unadmitted main value".into()));
    assert!(engine.inert_rollback_image().is_err());
    engine
}

#[test]
fn empty_child_poll_does_not_admit_the_main_frame() {
    let mut engine = main_with_unadmitted_value();
    assert!(engine.child_fibers.is_empty());
    let mut input = RuntimeStepInput::default();
    let mut output = RuntimeStepOutput::default();
    let mut backend = VmRuntimePureCallBackend::default();
    assert!(!engine.step_next_child_fiber(&mut input, &[], &mut output, &mut backend));
    assert_eq!(input, RuntimeStepInput::default());
    assert_eq!(output, RuntimeStepOutput::default());
    assert!(engine.inert_rollback_image().is_err());
    assert!(matches!(
        engine.fiber.frame.status,
        super::FlowFiberStatus::Running
    ));
    assert_eq!(backend.stats().pure_calls, 0);
}

#[test]
fn nonempty_child_poll_still_requires_atomic_admission() {
    let mut engine = main_with_unadmitted_value();
    let (child, next) = engine.prepare_child_fiber(&[FlowOp::Noop], &[]).unwrap();
    engine.next_fiber_id = next;
    engine.child_fibers.push_back(child);
    let mut input = RuntimeStepInput::default();
    let mut output = RuntimeStepOutput::default();
    let mut backend = VmRuntimePureCallBackend::default();
    assert!(engine.step_next_child_fiber(&mut input, &[], &mut output, &mut backend));
    assert!(!output.diagnostics.is_empty());
    assert!(matches!(
        engine.fiber.frame.status,
        super::FlowFiberStatus::Failed(_)
    ));
    assert_eq!(backend.stats().pure_calls, 0);
}

#[test]
fn child_identity_exhaustion_precedes_capture_admission_and_preserves_the_last_identity() {
    let plan = returning_function_plan(RuntimeFunctionSiteBodyKind::Executable);
    let local = plan
        .function_sites()
        .iter_with_ids()
        .flat_map(|(_, function)| function.inputs())
        .next()
        .unwrap()
        .input_local();
    let mut engine = Engine::new(plan);
    engine.next_fiber_id = u64::MAX;
    assert!(matches!(
        engine.prepare_child_fiber(&[FlowOp::Noop], &[local]),
        Err(crate::value::RuntimeEvalError::FiberIdentityOverflow)
    ));
    assert_eq!(engine.next_fiber_id, u64::MAX);
    assert!(engine.child_fibers.is_empty());
    assert!(engine.fiber.frame.env.get(local).is_none());

    engine.next_fiber_id = u64::MAX - 1;
    let (child, next) = engine.prepare_child_fiber(&[FlowOp::Noop], &[]).unwrap();
    assert_eq!(child.id, super::FlowFiberId(u64::MAX - 1));
    assert_eq!(child.persistent_id.get(), u64::MAX);
    assert_eq!(child.execution.get().get(), u64::MAX);
    assert_eq!(next, u64::MAX);
    assert_eq!(engine.next_fiber_id, u64::MAX - 1);
    assert!(engine.child_fibers.is_empty());
}
