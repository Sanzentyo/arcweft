use super::*;
use crate::entry::{RootExecutionLimits, RuntimeTypeSchema};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed, RuntimePatternSeed,
    RuntimePatternSeedKind,
};
use crate::pure::{RuntimeExternalCallBackend, RuntimeExternalCallContext};
use crate::task::RuntimeProgramOwner;
use crate::value::{RuntimeCallTarget, RuntimeFormatContext};
use std::cell::Cell;
use std::rc::Rc;

fn options(max_ops: usize) -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: RuntimeStepMode::Drain,
        budget: crate::step::RuntimeStepBudget { max_ops },
        ..RuntimeStepOptions::default()
    }
}

fn target() -> RuntimeCallTarget {
    RuntimeCallTarget::callable(crate::entry::RuntimeCallableId::from_checked_digest(
        [98; 32],
    ))
}

struct ReducerBackend {
    plan: Arc<RuntimePlan>,
    calls: Rc<Cell<usize>>,
    fail: bool,
}

impl RuntimeExternalCallBackend for ReducerBackend {
    fn call_external(
        &mut self,
        context: &RuntimeExternalCallContext,
        callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        assert_eq!(callee, &target());
        assert!(args.is_empty());
        let Some(RuntimeProgramOwner::Plan(plan)) = context.program_owner() else {
            panic!("reducer call lost its original executable owner")
        };
        assert!(Arc::ptr_eq(&plan, &self.plan));
        assert_eq!(
            context
                .execution_context()
                .unwrap()
                .format_context()
                .active_locale()
                .as_str(),
            "de-DE"
        );
        assert_eq!(
            context.result_type(),
            Some(RuntimeSemanticTypeId::from_bytes([9; 32]))
        );
        self.calls.set(self.calls.get() + 1);
        Some(if self.fail {
            Err(RuntimeEvalError::UnsupportedPure {
                name: "test.reducer".to_owned(),
                reason: "selected backend failure".to_owned(),
            })
        } else {
            Ok(RuntimeValue::Unit)
        })
    }
}

fn fixture() -> (Arc<RuntimePlan>, Engine) {
    let unit = RuntimeSemanticTypeId::from_bytes([9; 32]);
    let plan = Arc::new(
        crate::plan::body_semantic::stateful_builder_with_reducer_prefix(
            RuntimeTypeSchema::Bool,
            RootExecutionLimits::engine_default(),
            vec![
                RuntimeFlowOpSeed::Noop,
                RuntimeFlowOpSeed::Let {
                    pattern: RuntimePatternSeed::new(unit, RuntimePatternSeedKind::Discard),
                    expr: RuntimeExprSeed::new(
                        unit,
                        RuntimeExprSeedKind::Call {
                            callee: target(),
                            args: Box::new([]),
                        },
                    ),
                },
                RuntimeFlowOpSeed::Noop,
            ],
        )
        .finish()
        .unwrap(),
    );
    let mut engine = Engine::new_with_generation(Arc::clone(&plan), GenerationId::new(0));
    engine.set_format_context(RuntimeFormatContext::new(
        arcweft_id::LocaleTag::try_new("de-DE").unwrap(),
    ));
    engine.start_entry(&plan.entries()[0].id).unwrap();
    let initialized = engine.step(RuntimeStepInput::default(), options(64));
    assert!(
        initialized.output.diagnostics.is_empty(),
        "{:?}",
        initialized.output
    );
    assert_eq!(initialized.output.root_initializations.len(), 1);
    assert!(matches!(initialized.fiber_status, FlowFiberStatus::Done(_)));
    (plan, engine)
}

fn input(plan: &RuntimePlan, count: usize) -> RuntimeStepInput {
    let roles = plan.entries()[0].roles.stateful().unwrap();
    let mut input = RuntimeStepInput::default();
    for _ in 0..count {
        let event = crate::program_types::RuntimeProgramTypes::Plan(plan)
            .try_record_value(
                roles.event.semantic_identity,
                vec![],
                roles.command_policy.root_limits.schema,
            )
            .unwrap();
        input
            .root_events
            .push(RootEventInput::new(RuntimePayload(event)));
    }
    input
}

#[test]
fn native_reducer_budget_and_cold_rollback_keep_actual_backend_and_completed_calls() {
    let (plan, mut engine) = fixture();
    let owner = RuntimeProgramOwner::Plan(Arc::clone(&plan));
    let calls = Rc::new(Cell::new(0));
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(ReducerBackend {
        plan: Arc::clone(&plan),
        calls: Rc::clone(&calls),
        fail: false,
    });
    let committed = engine
        .root()
        .unwrap()
        .active()
        .phase
        .state_value()
        .unwrap()
        .clone();
    let zero = engine.step_with_pure_backend(input(&plan, 1), options(0), &mut backend);
    assert_eq!(zero.stats.executed_ops, 0);
    assert_eq!(zero.stop_reason, RuntimeStepStopReason::BudgetExhausted);
    assert_eq!(calls.get(), 0);
    assert!(!engine.root().unwrap().active().reducer_active());
    assert_eq!(engine.root().unwrap().active().pending_event_count(), 1);
    for expected_calls in [0, 1] {
        let step =
            engine.step_with_pure_backend(RuntimeStepInput::default(), options(1), &mut backend);
        assert_eq!(step.stats.executed_ops, 1);
        assert!(step.output.diagnostics.is_empty(), "{:?}", step.output);
        assert!(step.output.root_transitions.is_empty());
        assert_eq!(calls.get(), expected_calls);
        assert!(engine.root().unwrap().active().reducer_active());
        assert_eq!(
            engine.root().unwrap().active().phase.state_value(),
            Some(&committed)
        );
    }
    let image = engine.inert_rollback_image().unwrap();
    drop(engine);
    let mut restored = Engine::from_rollback_image(image.clone(), &owner).unwrap();
    assert_eq!(restored.inert_rollback_image().unwrap(), image);
    let zero =
        restored.step_with_pure_backend(RuntimeStepInput::default(), options(0), &mut backend);
    assert_eq!(zero.stats.executed_ops, 0);
    assert_eq!(calls.get(), 1);
    let done =
        restored.step_with_pure_backend(RuntimeStepInput::default(), options(64), &mut backend);
    assert!(done.output.diagnostics.is_empty(), "{:?}", done.output);
    assert!(
        matches!(&done.output.root_transitions[..], [RootTransitionOutcome::Committed { sequence, .. }] if sequence.get() == 0)
    );
    assert_eq!(calls.get(), 1);
    assert!(!restored.root().unwrap().active().reducer_active());
    assert!(restored.child_fibers.is_empty());
    let next = restored.step_with_pure_backend(input(&plan, 2), options(64), &mut backend);
    assert!(next.output.diagnostics.is_empty(), "{:?}", next.output);
    assert_eq!(next.output.root_transitions.len(), 2);
    for (outcome, expected) in next.output.root_transitions.iter().zip([1, 2]) {
        assert!(
            matches!(outcome, RootTransitionOutcome::Committed { sequence, .. } if sequence.get() == expected)
        );
    }
    assert_eq!(calls.get(), 3);
    restored.inert_rollback_image().unwrap();
}

#[test]
fn native_reducer_backend_trap_keeps_committed_state_and_removes_active_frame() {
    let (plan, mut engine) = fixture();
    let committed = engine
        .root()
        .unwrap()
        .active()
        .phase
        .state_value()
        .unwrap()
        .clone();
    let calls = Rc::new(Cell::new(0));
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(ReducerBackend {
        plan: Arc::clone(&plan),
        calls: Rc::clone(&calls),
        fail: true,
    });
    let trapped = engine.step_with_pure_backend(input(&plan, 2), options(64), &mut backend);
    assert!(matches!(trapped.fiber_status, FlowFiberStatus::Failed(_)));
    assert_eq!(trapped.stop_reason, RuntimeStepStopReason::Failed);
    assert!(
        matches!(&trapped.output.root_transitions[..], [RootTransitionOutcome::Trapped { sequence, message, .. }] if sequence.get() == 0 && message.contains("selected backend failure"))
    );
    assert!(trapped.output.root_commands.is_empty());
    assert_eq!(calls.get(), 1);
    let root = engine.root().unwrap();
    assert_eq!(root.active().phase.state_value(), Some(&committed));
    assert!(!root.active().reducer_active());
    assert_eq!(root.active().pending_event_count(), 0);
    assert!(root.failure().is_some());
    assert!(engine.child_fibers.is_empty());
    engine.inert_rollback_image().unwrap();
}

#[test]
fn native_reducer_checkpoint_rejection_traps_without_running_or_stranding_its_frame() {
    let (plan, mut engine) = fixture();
    let state_identity = engine.root().unwrap().roles().state.semantic_identity;
    let local = plan
        .function_sites()
        .iter()
        .flat_map(|function| function.parameter_inputs())
        .find(|input| {
            let declaration = plan.local_declarations().get(input.input_local()).unwrap();
            plan.type_table()
                .get(declaration.ty())
                .unwrap()
                .semantic_identity()
                == state_identity
        })
        .unwrap()
        .input_local();
    engine.fiber.frame.env.set_root(
        local,
        RuntimeValue::String("invalid retained foreground value".to_owned()),
    );
    assert!(engine.inert_rollback_image().is_err());
    let committed = engine
        .root()
        .unwrap()
        .active()
        .phase
        .state_value()
        .unwrap()
        .clone();
    let calls = Rc::new(Cell::new(0));
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(ReducerBackend {
        plan: Arc::clone(&plan),
        calls: Rc::clone(&calls),
        fail: false,
    });
    let step = engine.step_with_pure_backend(input(&plan, 2), options(64), &mut backend);
    assert_eq!(step.stop_reason, RuntimeStepStopReason::Failed);
    assert!(
        matches!(&step.output.root_transitions[..], [RootTransitionOutcome::Trapped { sequence, .. }] if sequence.get() == 0)
    );
    assert!(step.output.root_commands.is_empty());
    assert_eq!(calls.get(), 0);
    assert!(engine.child_fibers.is_empty());
    let root = engine.root().unwrap();
    assert_eq!(root.active().phase.state_value(), Some(&committed));
    assert!(!root.active().reducer_active());
    assert_eq!(root.active().pending_event_count(), 0);
    assert!(root.failure().is_some());
    root.inert_rollback_image(&RuntimeProgramOwner::Plan(Arc::clone(&plan)))
        .unwrap();
    let failed =
        engine.step_with_pure_backend(RuntimeStepInput::default(), options(64), &mut backend);
    assert_eq!(failed.stop_reason, RuntimeStepStopReason::Failed);
    assert_eq!(failed.stats.executed_ops, 0);
    assert!(failed.output.root_transitions.is_empty());
    assert_eq!(calls.get(), 0);
}

#[test]
fn native_reducer_frame_allocation_failure_traps_before_execution() {
    let (plan, mut engine) = fixture();
    let committed = engine
        .root()
        .unwrap()
        .active()
        .phase
        .state_value()
        .unwrap()
        .clone();
    engine.next_fiber_id = u64::MAX;
    let step = engine.step(input(&plan, 2), options(64));
    assert_eq!(step.stats.executed_ops, 0);
    assert_eq!(step.stop_reason, RuntimeStepStopReason::Failed);
    assert!(
        matches!(&step.output.root_transitions[..], [RootTransitionOutcome::Trapped { sequence, .. }] if sequence.get() == 0)
    );
    assert!(step.output.root_commands.is_empty());
    assert!(engine.child_fibers.is_empty());
    let root = engine.root().unwrap();
    assert_eq!(root.active().phase.state_value(), Some(&committed));
    assert!(!root.active().reducer_active());
    assert_eq!(root.active().pending_event_count(), 0);
    assert!(root.failure().is_some());
    engine.inert_rollback_image().unwrap();
}

#[test]
fn native_reducer_restore_rejects_missing_duplicate_and_forged_role_owners() {
    let (plan, mut engine) = fixture();
    let owner = RuntimeProgramOwner::Plan(Arc::clone(&plan));
    let step = engine.step(input(&plan, 1), options(1));
    assert!(step.output.diagnostics.is_empty(), "{:?}", step.output);
    let original = engine.inert_rollback_image().unwrap();
    let mut missing = original.clone();
    missing.child_fibers.clear();
    assert!(Engine::from_rollback_image(missing, &owner).is_err());
    let mut duplicate = original.clone();
    duplicate
        .child_fibers
        .push_back(duplicate.child_fibers[0].clone());
    assert!(Engine::from_rollback_image(duplicate, &owner).is_err());
    let mut forged = original.clone();
    forged.child_fibers[0].owner = FlowFiberOwner::Executor;
    assert!(Engine::from_rollback_image(forged, &owner).is_err());
    let mut ready = original.clone();
    let contract = RootStartupContract::from_runtime_plan(&plan, &plan.entries()[0].id).unwrap();
    let startup = RootRuntime::prepare(contract, owner.types())
        .unwrap()
        .complete_initialization(
            engine
                .root()
                .unwrap()
                .active()
                .phase
                .state_value()
                .unwrap()
                .0
                .clone(),
            owner.types(),
        )
        .unwrap();
    ready.root = Some(startup.root.inert_rollback_image(&owner).unwrap());
    assert!(Engine::from_rollback_image(ready, &owner).is_err());
    let mut wrong_role = original.clone();
    let FlowControlStackEntryKind::FunctionCall(frame) =
        &mut wrong_role.child_fibers[0].frame.control_stack[0]
    else {
        panic!("reducer frame must retain its exact function continuation")
    };
    frame.continuation = FunctionReturnContinuationRollbackImage::RootInitializer {
        entry: plan.entries()[0].id.clone(),
    };
    assert!(Engine::from_rollback_image(wrong_role, &owner).is_err());
    assert_eq!(engine.inert_rollback_image().unwrap(), original);
}
