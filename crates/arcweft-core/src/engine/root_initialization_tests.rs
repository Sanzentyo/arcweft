use super::*;
use crate::entry::{RootExecutionLimits, RuntimeTypeSchema};
use crate::root::RootPhase;

fn fixture() -> (Arc<RuntimePlan>, Engine) {
    let plan = Arc::new(crate::plan::body_semantic::stateful_root_fixture(
        RuntimeTypeSchema::Bool,
        RootExecutionLimits::engine_default(),
    ));
    let mut engine = Engine::new_with_generation(Arc::clone(&plan), GenerationId::new(0));
    engine.start_entry(&plan.entries()[0].id).unwrap();
    (plan, engine)
}

fn options(max_ops: usize) -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: RuntimeStepMode::Drain,
        budget: crate::step::RuntimeStepBudget { max_ops },
        ..RuntimeStepOptions::default()
    }
}

fn finish_initializer(engine: &mut Engine) -> crate::root::RootInitializationOutcome {
    for _ in 0..32 {
        let result = engine.step(RuntimeStepInput::default(), options(1));
        assert!(result.output.diagnostics.is_empty(), "{:?}", result.output);
        assert!(result.stats.executed_ops <= 1);
        assert!(result.output.root_transitions.is_empty());
        if let [outcome] = &result.output.root_initializations[..] {
            assert_eq!(result.stats.executed_ops, 1);
            assert!(matches!(result.fiber_status, FlowFiberStatus::Running));
            return outcome.clone();
        }
        assert!(result.output.root_initializations.is_empty());
    }
    panic!("initializer did not complete within its unchanged one-op steps")
}

#[test]
fn native_root_constructor_and_zero_budget_do_not_initialize_or_lose_ingress() {
    let (plan, mut engine) = fixture();
    assert!(matches!(
        engine.root().unwrap().active().phase,
        RootPhase::Initializing { .. }
    ));
    let roles = engine.root().unwrap().roles();
    let event = crate::program_types::RuntimeProgramTypes::Plan(&plan)
        .try_record_value(
            roles.event.semantic_identity,
            vec![],
            roles.command_policy.root_limits.schema,
        )
        .unwrap();
    let mut input = RuntimeStepInput::default();
    input
        .root_events
        .push(RootEventInput::new(RuntimePayload(event)));
    let result = engine.step(input, options(0));
    assert_eq!(result.stats.executed_ops, 0);
    assert!(result.output.diagnostics.is_empty());
    assert!(result.output.root_initializations.is_empty());
    assert!(
        engine
            .root()
            .unwrap()
            .active()
            .phase
            .state_value()
            .is_none()
    );
    assert_eq!(engine.root().unwrap().active().pending_event_count(), 1);
    let outcome = finish_initializer(&mut engine);
    assert_eq!(outcome.entry, plan.entries()[0].id);
    assert_eq!(
        outcome.binding,
        plan.entries()[0].roles.stateful().unwrap().binding
    );
    assert_eq!(engine.root().unwrap().active().pending_event_count(), 1);
    assert!(
        engine
            .root()
            .unwrap()
            .active()
            .phase
            .state_value()
            .is_some()
    );
    engine.inert_rollback_image().unwrap();
}

#[test]
fn native_root_initialization_uses_remaining_drain_allowance_for_initial_flow() {
    let (_, mut engine) = fixture();
    let result = engine.step(RuntimeStepInput::default(), options(64));
    assert!(result.output.diagnostics.is_empty(), "{:?}", result.output);
    assert!(result.stats.executed_ops > 1);
    assert!(result.stats.executed_ops <= 64);
    assert_eq!(result.output.root_initializations.len(), 1);
    assert!(matches!(result.fiber_status, FlowFiberStatus::Done(_)));
}

#[test]
fn native_pending_initializer_cold_rollback_keeps_its_authenticated_frame() {
    let (plan, engine) = fixture();
    let owner = crate::task::RuntimeProgramOwner::Plan(Arc::clone(&plan));
    let image = engine.inert_rollback_image().unwrap();
    drop(engine);
    let mut restored = Engine::from_rollback_image(image.clone(), &owner).unwrap();
    assert_eq!(restored.inert_rollback_image().unwrap(), image);
    let zero = restored.step(RuntimeStepInput::default(), options(0));
    assert_eq!(zero.stats.executed_ops, 0);
    assert!(zero.output.root_initializations.is_empty());
    let outcome = finish_initializer(&mut restored);
    let value = restored
        .root()
        .unwrap()
        .active()
        .phase
        .state_value()
        .unwrap();
    let roles = restored.root().unwrap().roles();
    assert_eq!(
        roles
            .state
            .accepts_payload(
                owner.types(),
                value,
                roles.command_policy.root_limits.schema
            )
            .unwrap(),
        outcome.state_digest
    );
}

#[test]
fn native_restore_rejects_ready_root_with_pending_initializer_frame() {
    let (plan, engine) = fixture();
    let owner = crate::task::RuntimeProgramOwner::Plan(Arc::clone(&plan));
    let original = engine.inert_rollback_image().unwrap();
    let contract = RootStartupContract::from_runtime_plan(&plan, &plan.entries()[0].id).unwrap();
    let value = owner
        .types()
        .try_record_value(
            contract.roles.state.semantic_identity,
            vec![],
            contract.roles.command_policy.root_limits.schema,
        )
        .unwrap();
    let startup = RootRuntime::prepare(contract, owner.types())
        .unwrap()
        .complete_initialization(value, owner.types())
        .unwrap();
    let mut invalid = original.clone();
    invalid.root = Some(startup.root.inert_rollback_image(&owner).unwrap());
    assert!(Engine::from_rollback_image(invalid, &owner).is_err());
    assert_eq!(engine.inert_rollback_image().unwrap(), original);
}
