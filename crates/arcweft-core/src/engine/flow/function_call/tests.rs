use std::sync::Arc;

use crate::engine::Engine;
use crate::plan::RuntimeFunctionSiteBodyKind;
use crate::step::RuntimeStepOutput;
use crate::tests::function_application::{
    returning_callable_state, returning_function_plan, returning_function_result,
};
use crate::value::{RuntimeCallableValue, RuntimeEvalError, RuntimeValue};

#[test]
fn active_effect_frame_rollback_rejects_a_missing_frame_binding() {
    use crate::effect_row::{DecisionControl, DecisionWork, EffectFormula};
    use crate::pattern::RuntimeSemanticTypeId;
    use crate::plan::{
        RuntimeEffectSet, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFunctionSiteDeclarationSeed,
        RuntimeFunctionTypeContract, RuntimePlanBuilder, RuntimePlanTypeProjection,
        RuntimePlanTypeSeed, RuntimeTypeBinder, RuntimeTypeScope,
    };
    struct Work;
    impl DecisionControl for Work {
        type Error = std::convert::Infallible;
        fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
            Ok(())
        }
    }
    let unit = RuntimeSemanticTypeId::from_bytes([0xd1; 32]);
    let header = RuntimeSemanticTypeId::from_bytes([0xd2; 32]);
    let binder = RuntimeTypeBinder::new(0, 0, 1);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    let io = EffectFormula::literal(
        crate::effect_row::EffectSet::from_labels(["io.read"]).unwrap(),
        None,
    );
    let variable =
        EffectFormula::literal(Default::default(), Some(scope.bound_effect(0, 0).unwrap()));
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    header,
                    RuntimePlanTypeProjection::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            binder,
                            io.subset(&variable, &mut Work).unwrap(),
                            EffectFormula::empty(),
                        ),
                        parameters: Box::new([]),
                        result: unit,
                    },
                ),
            ],
            [],
        )
        .unwrap();
    let site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                [41; 32],
            ),
            role: crate::plan::RuntimeFunctionSemanticRole::Closure,
            function_type: Some(header),
            inputs: Box::new([]),
            result: unit,
            body_kind: RuntimeFunctionSiteBodyKind::Expression,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    builder
        .define_function_site_seed(
            &site,
            RuntimeExprSeed::new(unit, RuntimeExprSeedKind::Value(RuntimeValue::Unit)),
        )
        .unwrap();
    let program =
        arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest([0xd3; 32]);
    builder
        .push_pure_program_binding_seed(&crate::plan::RuntimePureProgramBindingSeed {
            program,
            site,
        })
        .unwrap();
    let plan = Arc::new(builder.finish().unwrap());
    let engine = Engine::for_program_invocation(plan, program, vec![]).unwrap();
    let image = engine.inert_rollback_image().unwrap();
    let restored = Engine::from_rollback_image(image.clone()).unwrap();
    assert_eq!(
        restored
            .fiber
            .env
            .function_instantiation()
            .unwrap()
            .context(),
        header
    );
    let mut missing = image;
    let Some(crate::engine::FlowControlStackEntryRollbackImage::FunctionCall {
        type_instantiation,
        ..
    }) = missing.fiber.control_stack.last_mut()
    else {
        panic!("program did not retain its active function frame")
    };
    *type_instantiation = None;
    assert!(Engine::from_rollback_image(missing).is_err());
}

fn completed_handle_program() -> (
    Engine,
    arcweft_id::runtime_program::RuntimePureProgramId,
    crate::runtime_id::RuntimeLineHandleToken,
) {
    completed_handle_program_with_prefix(false)
}

fn completed_handle_program_with_prefix(
    prefix: bool,
) -> (
    Engine,
    arcweft_id::runtime_program::RuntimePureProgramId,
    crate::runtime_id::RuntimeLineHandleToken,
) {
    let fixture = crate::tests::program_custody::issued_program_handle_with_prefix(prefix);
    let mut engine = Engine::new_with_shared_plan(fixture.plan, crate::task::GenerationId::new(37));
    let destination = crate::value::ownership::RuntimeOwnedSlotId::ProgramResult {
        execution: engine.fiber.execution,
        fiber: engine.fiber.persistent_id,
    };
    let (value, custody) = crate::tests::program_custody::publish_program_handle(
        fixture.ledger,
        fixture.value,
        destination,
    );
    engine.dialogue_activations =
        crate::engine::dialogue::DialogueActivationStore::from_published(custody);
    engine.program_result = Some((fixture.program, value));
    engine.fiber.status = crate::engine::FlowFiberStatus::Done(crate::engine::FlowExit::Done);
    engine.main_started = true;
    engine.need_producers = crate::tests::program_custody::pending_program_need(
        engine.generation,
        engine.fiber.persistent_id,
    )
    .0;
    (engine, fixture.program, fixture.token)
}

#[test]
fn program_continuation_accepts_detached_prefix_and_retains_native_need_result_context() {
    use crate::program_invocation::RuntimeProgramInput;
    let (engine, id, token) = completed_handle_program_with_prefix(true);
    let mut engine = engine
        .continue_program(
            id,
            vec![
                RuntimeProgramInput::Detached(RuntimeValue::Bool(true)),
                RuntimeProgramInput::PreviousResult,
            ],
        )
        .unwrap();
    let output = engine.step(Default::default(), Default::default());
    assert!(output.output.diagnostics.is_empty());
    assert_eq!(
        engine
            .program_result
            .as_ref()
            .unwrap()
            .1
            .affine_line_handles()
            .unwrap()[0]
            .token(),
        &token
    );

    let plan = crate::tests::program_custody::native_need_program(id);
    let (registry, need) = crate::tests::program_custody::pending_program_need(
        crate::task::GenerationId::new(0),
        crate::runtime_id::RuntimePersistentFiberId::from_allocated(1),
    );
    // Begin from a completed owner retaining its real accepted producer context.
    // Detached activation correctly rejects an accepted Need without that owner.
    let mut engine = Engine::new_with_shared_plan(plan, crate::task::GenerationId::new(0));
    engine.need_producers = registry;
    engine.program_result = Some((id, RuntimeValue::NeedHandle(need.clone())));
    engine.fiber.status = crate::engine::FlowFiberStatus::Done(crate::engine::FlowExit::Done);
    engine.main_started = true;
    let mut engine = engine
        .continue_program(id, vec![RuntimeProgramInput::PreviousResult])
        .unwrap();
    assert!(
        engine
            .step(Default::default(), Default::default())
            .output
            .diagnostics
            .is_empty()
    );
    let before = engine.inert_rollback_image().unwrap();
    assert!(matches!(
        engine.take_program_result(),
        Err(crate::value::ownership::RuntimeDetachedValueError::NeedProducerCustodyRequired { .. })
    ));
    assert_eq!(engine.inert_rollback_image().unwrap(), before);
    let mut engine = engine
        .continue_program(id, vec![RuntimeProgramInput::PreviousResult])
        .unwrap();
    assert!(
        engine
            .step(Default::default(), Default::default())
            .output
            .diagnostics
            .is_empty()
    );
    assert_eq!(
        &engine.program_result.as_ref().unwrap().1,
        &RuntimeValue::NeedHandle(need.clone())
    );
    assert!(engine.need_producers.launch_for_handle(&need).is_some());
}

#[test]
fn program_continuation_moves_live_handle_with_need_context_and_returns_it_again() {
    use crate::program_invocation::RuntimeProgramInput;
    let (engine, id, token) = completed_handle_program();
    let owner = crate::task::RuntimeProgramOwner::Plan(Arc::clone(&engine.plan));
    let needs = engine.need_producers.inert_rollback_image(&owner).unwrap();
    let execution = engine.fiber.execution;
    let mut engine = engine
        .continue_program(id, vec![RuntimeProgramInput::PreviousResult])
        .unwrap();
    assert_eq!(engine.fiber.execution, execution);
    assert_eq!(engine.generation, crate::task::GenerationId::new(37));
    assert_eq!(
        engine.need_producers.inert_rollback_image(&owner).unwrap(),
        needs
    );
    assert!(engine.program_result.is_none());
    let output = engine.step_with_pure_backend(
        Default::default(),
        Default::default(),
        &mut crate::pure::VmRuntimePureCallBackend::default(),
    );
    assert!(
        output.output.diagnostics.is_empty(),
        "{:?}",
        output.output.diagnostics
    );
    assert!(output.output.requests.line_commands.is_empty());
    assert_eq!(
        engine
            .program_result
            .as_ref()
            .unwrap()
            .1
            .affine_line_handles()
            .unwrap()[0]
            .token(),
        &token
    );
    assert!(engine.take_program_result().is_err());
    let image = engine.inert_rollback_image().unwrap();
    drop(engine);
    let engine = Engine::from_rollback_image(image).unwrap();
    assert_eq!(
        engine.need_producers.inert_rollback_image(&owner).unwrap(),
        needs
    );
    let mut engine = engine
        .continue_program(id, vec![RuntimeProgramInput::PreviousResult])
        .unwrap();
    let output = engine.step_with_pure_backend(
        Default::default(),
        Default::default(),
        &mut crate::pure::VmRuntimePureCallBackend::default(),
    );
    assert!(output.output.diagnostics.is_empty());
    assert_eq!(
        engine
            .program_result
            .as_ref()
            .unwrap()
            .1
            .affine_line_handles()
            .unwrap()[0]
            .token(),
        &token
    );
}

#[test]
fn program_continuation_rejects_duplicate_or_invalid_inputs_with_original_native_owner() {
    use crate::program_invocation::{RuntimeProgramContinuationFailure, RuntimeProgramInput};
    let (engine, id, _) = completed_handle_program();
    let before = engine.inert_rollback_image().unwrap();
    let (reason, engine, inputs) = engine
        .continue_program(
            id,
            vec![
                RuntimeProgramInput::PreviousResult,
                RuntimeProgramInput::PreviousResult,
            ],
        )
        .unwrap_err()
        .into_parts();
    assert!(matches!(
        reason,
        RuntimeProgramContinuationFailure::ResultUseCount { count: 2 }
    ));
    assert_eq!(inputs.len(), 2);
    assert_eq!(engine.inert_rollback_image().unwrap(), before);
    let (reason, engine, inputs) = engine
        .continue_program(
            id,
            vec![
                RuntimeProgramInput::Detached(RuntimeValue::Bool(true)),
                RuntimeProgramInput::PreviousResult,
            ],
        )
        .unwrap_err()
        .into_parts();
    assert!(matches!(
        reason,
        RuntimeProgramContinuationFailure::Native(RuntimeEvalError::TooManyPureArgs { .. })
    ));
    assert_eq!(
        inputs[0],
        RuntimeProgramInput::Detached(RuntimeValue::Bool(true))
    );
    assert_eq!(engine.inert_rollback_image().unwrap(), before);
}

#[test]
fn program_continuation_rolls_back_native_context_and_inputs_after_custody_revision_failure() {
    use crate::program_invocation::{RuntimeProgramContinuationFailure, RuntimeProgramInput};
    let (mut engine, id, _) = completed_handle_program_with_prefix(true);
    let custody = std::mem::take(&mut engine.dialogue_activations)
        .into_published()
        .unwrap();
    let custody = crate::tests::program_custody::published_at_final_revision(custody);
    engine.dialogue_activations =
        crate::engine::dialogue::DialogueActivationStore::from_published(custody);
    let before = engine.inert_rollback_image().unwrap();
    let (reason, engine, inputs) = engine
        .continue_program(
            id,
            vec![
                RuntimeProgramInput::Detached(RuntimeValue::Bool(false)),
                RuntimeProgramInput::PreviousResult,
            ],
        )
        .unwrap_err()
        .into_parts();
    assert!(matches!(
        reason,
        RuntimeProgramContinuationFailure::Custody(
            crate::line_task::LineRuntimeError::ActivationTransactionRevisionOverflow
        )
    ));
    assert_eq!(
        inputs,
        vec![
            RuntimeProgramInput::Detached(RuntimeValue::Bool(false)),
            RuntimeProgramInput::PreviousResult
        ]
    );
    assert_eq!(engine.inert_rollback_image().unwrap(), before);
}

#[test]
fn detached_program_result_rejection_retains_native_value() {
    let fixture = crate::tests::program_custody::issued_program_handle();
    let mut engine = Engine::new_with_shared_plan(fixture.plan, crate::task::GenerationId::new(0));
    let destination = crate::value::ownership::RuntimeOwnedSlotId::ProgramResult {
        execution: engine.fiber.execution,
        fiber: engine.fiber.persistent_id,
    };
    let (value, custody) = crate::tests::program_custody::publish_program_handle(
        fixture.ledger,
        fixture.value,
        destination,
    );
    engine.dialogue_activations =
        crate::engine::dialogue::DialogueActivationStore::from_published(custody);
    engine.program_result = Some((fixture.program, value));
    let before = engine.inert_rollback_image().unwrap();
    assert!(engine.take_program_result().is_err());
    assert!(engine.take_program_result().is_err());
    let (_, value) = engine.program_result.as_ref().unwrap();
    assert_eq!(
        value.affine_line_handles().unwrap()[0].token(),
        &fixture.token
    );
    assert_eq!(engine.inert_rollback_image().unwrap(), before);
}

#[test]
fn program_return_preserves_typed_result_and_rollback_custody() {
    use crate::engine::FunctionReturnContinuation;
    use crate::plan::{
        RuntimeExprSeed, RuntimeExprSeedKind, RuntimePlanBuilder, RuntimePlanTypeProjection,
        RuntimePlanTypeSeed, RuntimePureProgramBindingSeed,
    };
    let ty = crate::pattern::RuntimeSemanticTypeId::from_bytes([81; 32]);
    let program = arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest([82; 32]);
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                ty,
                RuntimePlanTypeProjection::Bool,
            )],
            [],
        )
        .unwrap();
    let site = builder
        .push_function_site_seed(
            crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([41; 32]),
            crate::plan::RuntimeFunctionSemanticRole::Ordinary,
            [],
            RuntimeExprSeed::new(ty, RuntimeExprSeedKind::Value(RuntimeValue::Bool(true))),
        )
        .unwrap();
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed { program, site })
        .unwrap();
    let plan = Arc::new(builder.finish().unwrap());
    let site = plan.pure_programs()[0].site();
    let mut backend = crate::pure::VmRuntimePureCallBackend::default();
    let mut engine =
        Engine::for_program_invocation(Arc::clone(&plan), program, Vec::new()).unwrap();
    let rejection =
        Engine::for_program_invocation(Arc::clone(&plan), program, vec![RuntimeValue::Bool(false)])
            .unwrap_err();
    let (reason, retained) = rejection.into_parts();
    assert!(matches!(
        reason,
        RuntimeEvalError::TooManyPureArgs {
            found: 1,
            max: 0,
            ..
        }
    ));
    assert_eq!(retained, [RuntimeValue::Bool(false)]);
    assert_eq!(engine.take_program_result().unwrap(), None);
    engine.step_with_pure_backend(
        crate::step::RuntimeStepInput::default(),
        crate::step::RuntimeStepOptions::default(),
        &mut backend,
    );
    let saved = engine.inert_rollback_image().unwrap();
    assert_eq!(
        engine.take_program_result().unwrap(),
        Some((program, RuntimeValue::Bool(true)))
    );
    assert_eq!(engine.take_program_result().unwrap(), None);
    let mut restored = Engine::from_rollback_image(saved).unwrap();
    assert_eq!(
        restored.take_program_result().unwrap(),
        Some((program, RuntimeValue::Bool(true)))
    );
    assert_eq!(restored.take_program_result().unwrap(), None);
    let owner = crate::task::RuntimeProgramOwner::Plan(plan);
    assert!(
        FunctionReturnContinuation::from_rollback_image(
            crate::engine::FunctionReturnContinuationRollbackImage::Program {
                program: arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest(
                    [83; 32]
                ),
            },
            site,
            &owner,
        )
        .is_err()
    );
}

#[test]
fn owned_program_moves_affine_input_and_returns_it_once() {
    use crate::pattern::RuntimeSemanticTypeId;
    use crate::plan::{
        RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFunctionInputBindingSeed,
        RuntimeFunctionInputOwnershipRequirement, RuntimeFunctionInputSource,
        RuntimeLocalDeclarationSeed, RuntimeLocalReadSeed, RuntimePatternSeed,
        RuntimePatternSeedKind, RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
        RuntimePureProgramBindingSeed,
    };
    let boolean = RuntimeSemanticTypeId::from_bytes([84; 32]);
    let need = RuntimeSemanticTypeId::from_bytes([85; 32]);
    let program = arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest([86; 32]);
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(need, RuntimePlanTypeProjection::Need(boolean)),
            ],
            [RuntimeLocalDeclarationSeed::new(need)],
        )
        .unwrap();
    let local = admitted.local_ids()[0].clone();
    let input = RuntimeFunctionInputBindingSeed {
        origin: crate::plan::RuntimeFunctionInputOrigin::Parameter([81; 32]),
        source: RuntimeFunctionInputSource::Parameter {
            position: 0,
            passing: crate::plan::RuntimeFunctionParameterPassing::Affine,
        },
        input_local: local.clone(),
        pattern: RuntimePatternSeed::new(
            need,
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local: local.clone(),
            },
        ),
        ownership: RuntimeFunctionInputOwnershipRequirement::Owned,
        unrestricted_bindings: Box::new([]),
    };
    let site = builder
        .push_function_site_seed(
            crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([41; 32]),
            crate::plan::RuntimeFunctionSemanticRole::Ordinary,
            [input],
            RuntimeExprSeed::new(
                need,
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    local,
                    crate::value::RuntimeLocalReadMode::Move,
                )),
            ),
        )
        .unwrap();
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed { program, site })
        .unwrap();
    let plan = Arc::new(builder.finish().unwrap());
    let value = RuntimeValue::NeedHandle(crate::tests::reusable_need("need.owned-program"));
    assert!(!value.ownership().permits_copy());
    let mut engine = Engine::for_program_invocation(plan, program, vec![value]).unwrap();
    let output = engine.step(
        crate::step::RuntimeStepInput::default(),
        crate::step::RuntimeStepOptions::default(),
    );
    assert!(output.output.diagnostics.is_empty());
    let saved = engine.inert_rollback_image().unwrap();
    assert_eq!(
        engine.take_program_result().unwrap(),
        Some((
            program,
            RuntimeValue::NeedHandle(crate::tests::reusable_need("need.owned-program"))
        ))
    );
    assert_eq!(engine.take_program_result().unwrap(), None);
    let mut restored = Engine::from_rollback_image(saved).unwrap();
    assert_eq!(
        restored.take_program_result().unwrap(),
        Some((
            program,
            RuntimeValue::NeedHandle(crate::tests::reusable_need("need.owned-program"))
        ))
    );
    assert_eq!(restored.take_program_result().unwrap(), None);
}

#[test]
fn surplus_arguments_reject_before_creating_an_executable_frame() {
    let mut engine = Engine::new(returning_function_plan(
        RuntimeFunctionSiteBodyKind::Executable,
    ));
    let function = RuntimeCallableValue::try_new(
        crate::task::RuntimeProgramOwner::Plan(Arc::clone(&engine.plan)),
        returning_callable_state(&engine.plan),
        [],
    )
    .unwrap();
    let result = returning_function_result(&engine.plan);
    let owner = crate::task::RuntimeProgramOwner::Plan(Arc::clone(&engine.plan));
    let before = engine.fiber().inert_rollback_image(&owner).unwrap();
    let mut backend = crate::pure::VmRuntimePureCallBackend::default();
    let mut output = RuntimeStepOutput::default();
    assert!(matches!(
        engine.start_function_value_call(
            RuntimeValue::Callable(function),
            vec![RuntimeValue::Unit],
            result,
            None,
            &mut output,
            &mut backend,
        ),
        Err(RuntimeEvalError::Callable(
            crate::value::RuntimeCallableValueError::ArgumentCount {
                expected: 0,
                actual: 1,
                ..
            }
        ))
    ));
    assert_eq!(engine.fiber().inert_rollback_image(&owner).unwrap(), before);
    assert_eq!(output, RuntimeStepOutput::default());
}
