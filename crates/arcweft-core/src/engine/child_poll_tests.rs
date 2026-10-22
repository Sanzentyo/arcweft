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

#[test]
fn child_scope_joins_its_descendants_and_keeps_its_parent_until_they_finish() {
    let mut engine = Engine::new(returning_function_plan(
        RuntimeFunctionSiteBodyKind::Executable,
    ));
    let (mut parent, next) = engine.prepare_child_fiber(&[], &[]).unwrap();
    engine.next_fiber_id = next;
    let parent_id = parent.id;
    let (sibling, next) = engine.prepare_child_fiber(&[], &[]).unwrap();
    engine.next_fiber_id = next;
    let retired_sibling = sibling.id;
    engine.child_fibers.push_back(sibling);

    std::mem::swap(&mut engine.fiber, &mut parent);
    assert!(
        !engine.has_joined_work(),
        "a sibling belongs to the outer task"
    );
    let (descendant, next) = engine
        .prepare_child_fiber(&[FlowOp::Noop, FlowOp::Noop], &[])
        .unwrap();
    engine.next_fiber_id = next;
    assert_eq!(descendant.parent, Some(parent_id));
    std::mem::swap(&mut engine.fiber, &mut parent);
    engine.child_fibers.push_back(parent);
    engine.child_fibers.push_back(descendant);

    let owner = crate::task::RuntimeProgramOwner::Plan(engine.program_plan());
    let mut input = RuntimeStepInput::default();
    let mut output = RuntimeStepOutput::default();
    let mut backend = VmRuntimePureCallBackend::default();
    assert!(engine.step_next_child_fiber(&mut input, &[], &mut output, &mut backend));
    assert!(
        engine
            .child_fibers
            .iter()
            .all(|fiber| fiber.id != retired_sibling)
    );

    let saved = engine.inert_rollback_image().unwrap();
    let mut orphan = saved.clone();
    orphan.child_fibers.back_mut().unwrap().parent = Some(retired_sibling);
    assert!(Engine::from_rollback_image(orphan, &owner).is_err());
    let mut cyclic = saved.clone();
    let descendant = cyclic.child_fibers.back_mut().unwrap();
    descendant.parent = Some(descendant.id);
    assert!(Engine::from_rollback_image(cyclic, &owner).is_err());
    let mut unowned = saved.clone();
    unowned.child_fibers.back_mut().unwrap().parent = None;
    assert!(Engine::from_rollback_image(unowned, &owner).is_err());
    let mut failed_parent = Engine::from_rollback_image(saved.clone(), &owner).unwrap();
    failed_parent.child_fibers.front_mut().unwrap().frame.status =
        super::FlowFiberStatus::Failed("parent failed".into());
    assert!(failed_parent.step_next_child_fiber(&mut input, &[], &mut output, &mut backend));
    assert!(
        failed_parent
            .child_fibers
            .iter()
            .any(|fiber| fiber.id == parent_id)
    );
    assert!(failed_parent.inert_rollback_image().is_ok());
    let mut engine = Engine::from_rollback_image(saved, &owner).unwrap();
    assert!(engine.step_next_child_fiber(&mut input, &[], &mut output, &mut backend));
    assert!(
        engine
            .child_fibers
            .iter()
            .any(|fiber| fiber.id == parent_id)
    );
    for _ in 0..32 {
        if engine.child_fibers.is_empty() {
            break;
        }
        assert!(engine.step_next_child_fiber(&mut input, &[], &mut output, &mut backend));
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        engine =
            Engine::from_rollback_image(engine.inert_rollback_image().unwrap(), &owner).unwrap();
    }
    assert!(engine.child_fibers.is_empty());
}
