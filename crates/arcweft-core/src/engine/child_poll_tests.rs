use super::Engine;
use crate::{
    plan::{FlowOp, RuntimeFunctionSiteBodyKind},
    pure::{RuntimePureCallBackend, VmRuntimePureCallBackend},
    step::{RuntimeStepInput, RuntimeStepOutput},
    tests::function_application::returning_function_builder,
    value::RuntimeValue,
};

fn child_plan() -> crate::plan::RuntimePlan {
    use crate::plan::*;
    let mut builder = returning_function_builder(RuntimeFunctionSiteBodyKind::Executable);
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    for (index, (name, operations)) in [
        ("child.empty", vec![]),
        ("child.one", vec![RuntimeFlowOpSeed::Noop]),
        (
            "child.two",
            vec![RuntimeFlowOpSeed::Noop, RuntimeFlowOpSeed::Noop],
        ),
        (
            "child.return",
            vec![RuntimeFlowOpSeed::Return("child".into())],
        ),
        (
            "child.joined",
            vec![RuntimeFlowOpSeed::Return("joined".into())],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let flow = FlowRuntimeId::canonical(name).unwrap();
        builder
            .push_flow_schema(RuntimeFlowSchema {
                flow: flow.clone(),
                parameters: vec![],
            })
            .unwrap();
        builder
            .push_flow_seed(RuntimeFlowSeed::new(
                flow,
                RuntimeFunctionSiteDeclarationSeed::flow(
                    RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [100 + index as u8; 32],
                    ),
                    None,
                    Box::new([]),
                    unit,
                    RuntimeEffectSet::empty(),
                ),
                RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: operations.into_boxed_slice(),
                },
            ))
            .unwrap();
    }
    builder.finish().unwrap()
}

fn task_position(engine: &Engine, name: &str) -> super::NativeCodeCursor {
    let flow = crate::plan::FlowRuntimeId::canonical(name).unwrap();
    let site = engine
        .plan
        .validate_flow_invocation(&flow, &[])
        .unwrap()
        .site();
    super::NativeCodeCursor::root(
        &engine.plan,
        crate::plan::code::RuntimeCodeOwner::Function(site),
    )
    .unwrap()
}

fn task_work(
    engine: &Engine,
    name: &str,
) -> std::collections::VecDeque<super::work::NativePendingWork> {
    engine
        .prepare_task_root_work(task_position(engine, name), false)
        .unwrap()
}

fn main_with_unadmitted_value() -> Engine {
    let plan = child_plan();
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
    let (child, next) = engine
        .prepare_child_fiber(task_work(&engine, "child.one"), &[])
        .unwrap();
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
fn joined_return_keeps_its_position_across_steps_and_restore_without_poll_charges() {
    use super::{FlowExit, FlowFiberStatus};
    use crate::step::RuntimeStepOptions;

    let mut engine = Engine::new(child_plan());
    engine.main_started = true;
    let position = task_position(&engine, "child.joined");
    engine
        .fiber
        .frame
        .pending_ops
        .push_back(super::work::NativePendingWork::source(
            position,
            super::work::NativeSourceRangeRole::TaskRoot,
        ));
    let (child, next) = engine
        .prepare_child_fiber(task_work(&engine, "child.two"), &[])
        .unwrap();
    engine.next_fiber_id = next;
    engine.child_fibers.push_back(child);
    let owner = crate::task::RuntimeProgramOwner::Plan(engine.program_plan());
    let mut operations = 0;
    for _ in 0..5 {
        let joining = !engine.child_fibers.is_empty();
        if joining {
            assert!(!engine.main_fiber_can_attempt_runtime_op());
            assert!(matches!(
                engine.fiber.frame.pending_work().next(),
                Some(Ok(super::work::NativePendingWorkView::Source(FlowOp::Return(value)))) if value == "joined"
            ));
            assert_eq!(engine.fiber.frame.pending_ops.len(), 1);
        }
        let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        assert_eq!(result.stats.executed_ops, 1);
        operations += result.stats.executed_ops;
        if joining {
            assert_eq!(engine.fiber.frame.pending_ops.len(), 1);
            assert!(matches!(
                engine.fiber.frame.status,
                FlowFiberStatus::Running
            ));
        }
        engine =
            Engine::from_rollback_image(engine.inert_rollback_image().unwrap(), &owner).unwrap();
    }
    assert_eq!(
        operations, 5,
        "two body operations, scope enter/exit, and one return"
    );
    assert!(engine.child_fibers.is_empty());
    assert_eq!(
        engine.fiber.frame.status,
        FlowFiberStatus::Done(FlowExit::Return("joined".into()))
    );
}

#[test]
fn joined_original_function_return_expression_and_fallthrough_do_not_advance() {
    use super::{
        FlowControlStackEntry, FlowControlStackEntryKind, FunctionCallFrame,
        FunctionReturnContinuation,
    };
    use crate::plan::code::{RuntimeCodeBodyRef, RuntimeCodeOwner};

    let mut engine = Engine::new(child_plan());
    engine.main_started = true;
    let (child, next) = engine
        .prepare_child_fiber(task_work(&engine, "child.one"), &[])
        .unwrap();
    engine.next_fiber_id = next;
    engine.child_fibers.push_back(child);
    let sites = engine
        .plan
        .function_sites()
        .iter_with_ids()
        .map(|(site, _)| site)
        .collect::<Vec<_>>();
    let mut cases = 0;
    for site in sites {
        let positions = match engine
            .plan
            .inventory
            .code_body(RuntimeCodeOwner::Function(site))
            .unwrap()
        {
            RuntimeCodeBodyRef::Expression(_) => vec![0],
            RuntimeCodeBodyRef::Operations([FlowOp::ReturnExpr(_)]) => vec![0, 1],
            RuntimeCodeBodyRef::Operations(_) => continue,
        };
        for position in positions {
            let frame = FunctionCallFrame::new(site, FunctionReturnContinuation::Function { site });
            let mut cursor =
                super::NativeCodeCursor::root(&engine.plan, RuntimeCodeOwner::Function(site))
                    .unwrap();
            if cursor.operation_index().is_some() {
                assert!(cursor.set_operation(position));
            } else {
                assert_eq!(position, 0);
            }
            engine.fiber.frame.cursor = Some(cursor);
            engine.fiber.frame.control_stack = vec![FlowControlStackEntry {
                kind: FlowControlStackEntryKind::FunctionCall(frame),
            }];
            assert!(!engine.main_fiber_can_attempt_runtime_op());
            assert!(!engine.fiber_has_internal_work(&engine.fiber));
            let cursor = engine.fiber.frame.cursor.as_ref().unwrap();
            assert_eq!(cursor.operation_index().unwrap_or(0), position);
            assert!(engine.fiber.frame.pending_ops.is_empty());
            cases += 1;
        }
    }
    assert_eq!(
        cases, 3,
        "original expression, return operation, and exhausted body"
    );
}

#[test]
fn joined_child_return_keeps_its_queue_position_while_a_descendant_runs() {
    let mut engine = Engine::new(child_plan());
    let (mut parent, next) = engine
        .prepare_child_fiber(task_work(&engine, "child.return"), &[])
        .unwrap();
    engine.next_fiber_id = next;
    let parent_id = parent.id;
    std::mem::swap(&mut engine.fiber, &mut parent);
    let (descendant, next) = engine
        .prepare_child_fiber(task_work(&engine, "child.two"), &[])
        .unwrap();
    engine.next_fiber_id = next;
    std::mem::swap(&mut engine.fiber, &mut parent);
    engine.child_fibers.push_back(parent);
    engine.child_fibers.push_back(descendant);
    let mut input = RuntimeStepInput::default();
    let mut output = RuntimeStepOutput::default();
    let mut backend = VmRuntimePureCallBackend::default();
    // Enter the parent's scheduled scope, then the descendant's scope.
    assert!(engine.step_next_child_fiber(&mut input, &[], &mut output, &mut backend));
    assert!(engine.step_next_child_fiber(&mut input, &[], &mut output, &mut backend));
    assert_eq!(engine.child_fibers.front().unwrap().id, parent_id);
    let parent_before = engine.child_fibers.front().unwrap().frame.pending_ops.len();
    assert!(matches!(
        engine.child_fibers.front().unwrap().frame.pending_work().next(),
        Some(Ok(super::work::NativePendingWorkView::Source(FlowOp::Return(value)))) if value == "child"
    ));
    assert_eq!(engine.next_child_fiber_index(), Some(1));
    assert!(engine.step_next_child_fiber(&mut input, &[], &mut output, &mut backend));
    assert_eq!(engine.child_fibers.front().unwrap().id, parent_id);
    assert_eq!(
        engine.child_fibers.front().unwrap().frame.pending_ops.len(),
        parent_before
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let owner = crate::task::RuntimeProgramOwner::Plan(engine.program_plan());
    let restored =
        Engine::from_rollback_image(engine.inert_rollback_image().unwrap(), &owner).unwrap();
    assert_eq!(restored.next_child_fiber_index(), Some(1));
}

#[test]
fn child_identity_exhaustion_precedes_capture_admission_and_preserves_the_last_identity() {
    let plan = child_plan();
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
        engine.prepare_child_fiber(task_work(&engine, "child.one"), &[local]),
        Err(crate::value::RuntimeEvalError::FiberIdentityOverflow)
    ));
    assert_eq!(engine.next_fiber_id, u64::MAX);
    assert!(engine.child_fibers.is_empty());
    assert!(engine.fiber.frame.env.get(local).is_none());

    engine.next_fiber_id = u64::MAX - 1;
    let (child, next) = engine
        .prepare_child_fiber(task_work(&engine, "child.one"), &[])
        .unwrap();
    assert_eq!(child.id, super::FlowFiberId(u64::MAX - 1));
    assert_eq!(child.persistent_id.get(), u64::MAX);
    assert_eq!(child.execution.get().get(), u64::MAX);
    assert_eq!(next, u64::MAX);
    assert_eq!(engine.next_fiber_id, u64::MAX - 1);
    assert!(engine.child_fibers.is_empty());
}

#[test]
fn child_scope_joins_its_descendants_and_keeps_its_parent_until_they_finish() {
    let mut engine = Engine::new(child_plan());
    let (mut parent, next) = engine
        .prepare_child_fiber(task_work(&engine, "child.empty"), &[])
        .unwrap();
    engine.next_fiber_id = next;
    let parent_id = parent.id;
    let (sibling, next) = engine
        .prepare_child_fiber(task_work(&engine, "child.empty"), &[])
        .unwrap();
    engine.next_fiber_id = next;
    let retired_sibling = sibling.id;
    engine.child_fibers.push_back(sibling);

    std::mem::swap(&mut engine.fiber, &mut parent);
    assert!(
        !engine.has_joined_work(),
        "a sibling belongs to the outer task"
    );
    let (descendant, next) = engine
        .prepare_child_fiber(task_work(&engine, "child.two"), &[])
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
