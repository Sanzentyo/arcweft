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
    engine.invocation_result = Some((
        crate::engine::NativeInvocationRoot::Program(fixture.program),
        value,
    ));
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
            .invocation_result
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
    engine.invocation_result = Some((
        crate::engine::NativeInvocationRoot::Program(id),
        RuntimeValue::NeedHandle(need.clone()),
    ));
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
        &engine.invocation_result.as_ref().unwrap().1,
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
    assert!(engine.invocation_result.is_none());
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
            .invocation_result
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
            .invocation_result
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
    engine.invocation_result = Some((
        crate::engine::NativeInvocationRoot::Program(fixture.program),
        value,
    ));
    let before = engine.inert_rollback_image().unwrap();
    assert!(engine.take_program_result().is_err());
    assert!(engine.take_program_result().is_err());
    let (_, value) = engine.invocation_result.as_ref().unwrap();
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
            [RuntimeLocalDeclarationSeed::new(manual_local_source("arcweft-core.fixture.engine.flow.function_call.tests.owned_program_moves_affine_input_and_returns_it_once.binding_a"), need)],
        )
        .unwrap();
    let local = admitted.local_ids()[0].clone();
    let input = RuntimeFunctionInputBindingSeed {
        transfer: crate::plan::RuntimeFunctionInputTransfer::Formal,
        origin: crate::plan::RuntimeFunctionInputOrigin::Parameter(
            crate::plan::RuntimeFunctionParameterIdentity::from_accepted_identity([81; 32]),
        ),
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

fn manual_local_source(declaration: &str) -> crate::plan::RuntimeLocalDeclarationSource {
    // This fixture declares a semantic binding name independent of its value,
    // type, source offset, and builder-issued local ordinal.
    let mut identity = blake3::Hasher::new();
    identity.update(b"arcweft.manual-fixture-binding.v1\0");
    identity.update(declaration.as_bytes());
    crate::plan::RuntimeLocalDeclarationSource::Binding {
        identity: *identity.finalize().as_bytes(),
        declaration: crate::plan::RuntimeLocalBindingDeclaration::new(
            crate::plan::RuntimeLocalBindingKind::PatternBinding,
            false,
            crate::plan::RuntimeLocalBindingStorage::Derived,
        ),
    }
}

fn scalar_executable_invocation_fixture(
    divide: bool,
) -> (
    Arc<crate::plan::RuntimePlan>,
    crate::runtime_id::RuntimeFunctionSiteId,
) {
    use crate::plan::*;
    use crate::value::{RuntimeBinaryOp, RuntimeLocalReadMode, RuntimeSignedIntWidth};
    let ty = crate::pattern::RuntimeSemanticTypeId::from_bytes([0xf1; 32]);
    let parameters = [
        RuntimeFunctionParameterIdentity::from_accepted_identity([0xf2; 32]),
        RuntimeFunctionParameterIdentity::from_accepted_identity([0xf3; 32]),
    ];
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                ty,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
            )],
            parameters.map(|parameter| {
                RuntimeLocalDeclarationSeed::new(
                    RuntimeLocalDeclarationSource::Parameter(parameter),
                    ty,
                )
            }),
        )
        .unwrap();
    let inputs = parameters
        .into_iter()
        .zip(admission.local_ids().iter().cloned())
        .enumerate()
        .map(
            |(position, (parameter, local))| RuntimeFunctionInputBindingSeed {
                transfer: RuntimeFunctionInputTransfer::Formal,
                origin: RuntimeFunctionInputOrigin::Parameter(parameter),
                source: RuntimeFunctionInputSource::Parameter {
                    position: u32::try_from(position).unwrap(),
                    passing: RuntimeFunctionParameterPassing::Value,
                },
                input_local: local.clone(),
                pattern: RuntimePatternSeed::new(
                    ty,
                    RuntimePatternSeedKind::Bind {
                        mutable: false,
                        local,
                    },
                ),
                ownership: Default::default(),
                unrestricted_bindings: Box::new([]),
            },
        )
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xf4; 32]),
            role: RuntimeFunctionSemanticRole::Ordinary,
            function_type: None,
            inputs,
            result: ty,
            body_kind: RuntimeFunctionSiteBodyKind::Executable,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    let read = |index: usize| {
        RuntimeExprSeed::new(
            ty,
            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                admission.local_ids()[index].clone(),
                RuntimeLocalReadMode::Copy,
            )),
        )
    };
    let value = if divide {
        RuntimeExprSeed::new(
            ty,
            RuntimeExprSeedKind::Binary {
                op: RuntimeBinaryOp::Div,
                lhs: Box::new(read(0)),
                rhs: Box::new(read(1)),
            },
        )
    } else {
        RuntimeExprSeed::new(
            ty,
            RuntimeExprSeedKind::Binary {
                op: RuntimeBinaryOp::Add,
                lhs: Box::new(read(0)),
                rhs: Box::new(RuntimeExprSeed::new(
                    ty,
                    RuntimeExprSeedKind::Value(RuntimeValue::i64(3)),
                )),
            },
        )
    };
    builder
        .define_function_site_seed(
            &site,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([
                    RuntimeFlowOpSeed::Noop,
                    RuntimeFlowOpSeed::Scope {
                        identity: crate::scope::RuntimeScopeIdentity::Named(
                            "scalar_body".parse().unwrap(),
                        ),
                        body: vec![
                            RuntimeFlowOpSeed::Noop,
                            RuntimeFlowOpSeed::ReturnExpr(value),
                        ],
                    },
                ]),
            }),
        )
        .unwrap();
    let plan = Arc::new(builder.finish().unwrap());
    let site = plan.function_sites().iter_with_ids().next().unwrap().0;
    (plan, site)
}

#[test]
fn scalar_executable_function_root_preserves_full_formals_budget_rollback_and_result_custody() {
    use crate::pure::{
        PureFunctionBackend, PureFunctionRequest, RuntimePureFunctionRef, VmPureFunctionBackend,
    };
    let (plan, site) = scalar_executable_invocation_fixture(false);
    assert!(plan.pure_helpers().is_empty());
    assert_eq!(plan.pure_function_candidate_count(), 1);
    let function = RuntimePureFunctionRef::resolve(&plan, site).unwrap();
    assert!(function.body.is_executable());
    assert_eq!(
        function.inputs.len(),
        2,
        "unused original formal must remain"
    );
    assert!(Engine::for_function_invocation(function, vec![RuntimeValue::i64(4)]).is_err());
    let request = PureFunctionRequest::try_new(
        Arc::clone(&plan),
        site,
        [RuntimeValue::i64(4), RuntimeValue::i64(999)],
    )
    .unwrap();
    assert!(matches!(
        VmPureFunctionBackend.evaluate(&request),
        Err(RuntimeEvalError::UnsupportedPure { .. })
    ));
    assert!(
        VmPureFunctionBackend
            .evaluate_invocation(&request, crate::step::RuntimeStepBudget { max_ops: 1 })
            .is_err()
    );
    assert_eq!(
        request.bindings().len(),
        2,
        "a bounded standalone failure retains every borrowed input"
    );
    let evaluated = VmPureFunctionBackend
        .evaluate_invocation(&request, crate::step::RuntimeStepBudget { max_ops: 16 })
        .unwrap();
    assert_eq!(evaluated.value, RuntimeValue::i64(7));
    assert!(evaluated.stats.evaluated_exprs > 0);
    let mut engine = Engine::for_function_invocation(
        function,
        vec![RuntimeValue::i64(4), RuntimeValue::i64(999)],
    )
    .unwrap();
    assert!(Arc::ptr_eq(&engine.plan, &plan));
    let options = crate::step::RuntimeStepOptions {
        mode: crate::step::RuntimeStepMode::OneOp,
        budget: crate::step::RuntimeStepBudget { max_ops: 1 },
        ..Default::default()
    };
    let first = engine.step(Default::default(), options);
    assert_eq!(first.stats.executed_ops, 1);
    assert!(matches!(
        first.fiber_status,
        crate::engine::FlowFiberStatus::Running
    ));
    assert!(engine.take_function_result(site).unwrap().is_none());
    let saved = engine.inert_rollback_image().unwrap();
    let mut restored = Engine::from_rollback_image(saved).unwrap();
    assert!(Arc::ptr_eq(&restored.plan, &plan));
    for _ in 0..16 {
        let output = restored.step(Default::default(), options);
        assert!(output.output.diagnostics.is_empty());
        assert!(output.stats.executed_ops <= 1);
        if matches!(output.fiber_status, crate::engine::FlowFiberStatus::Done(_)) {
            break;
        }
    }
    assert!(matches!(
        restored.fiber.status,
        crate::engine::FlowFiberStatus::Done(_)
    ));
    assert_eq!(
        restored.take_program_result().unwrap(),
        None,
        "program API must retain the function result"
    );
    let saved_result = restored.inert_rollback_image().unwrap();
    let mut forged_result = saved_result.clone();
    let owner = crate::task::RuntimeProgramOwner::Plan(Arc::clone(&plan));
    forged_result.invocation_result.as_mut().unwrap().1 =
        crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
            &RuntimeValue::Bool(true),
            &owner,
        )
        .unwrap();
    assert!(
        Engine::from_rollback_image(forged_result).is_err(),
        "the retained function result must have its original type"
    );
    assert_eq!(
        restored.take_function_result(site).unwrap(),
        Some(RuntimeValue::i64(7))
    );
    assert_eq!(restored.take_function_result(site).unwrap(), None);
    let mut restored_result = Engine::from_rollback_image(saved_result).unwrap();
    assert_eq!(
        restored_result.take_function_result(site).unwrap(),
        Some(RuntimeValue::i64(7))
    );
    assert_eq!(restored_result.take_function_result(site).unwrap(), None);
}

#[test]
fn scalar_executable_declined_division_retains_the_engine_failure_transaction() {
    let (plan, site) = scalar_executable_invocation_fixture(true);
    let function = crate::pure::RuntimePureFunctionRef::resolve(&plan, site).unwrap();
    let mut engine = Engine::for_function_invocation(
        function,
        vec![RuntimeValue::i64(12), RuntimeValue::i64(0)],
    )
    .unwrap();
    let options = crate::step::RuntimeStepOptions {
        mode: crate::step::RuntimeStepMode::OneOp,
        budget: crate::step::RuntimeStepBudget { max_ops: 1 },
        ..Default::default()
    };
    let expected = RuntimeEvalError::RecoverableExpression(
        crate::value::RuntimeExpressionFailure::DivisionByZero,
    )
    .to_string();
    let mut failed = false;
    for _ in 0..16 {
        let result = engine.step(Default::default(), options);
        assert!(result.stats.executed_ops <= 1);
        if let crate::engine::FlowFiberStatus::Failed(message) = result.fiber_status {
            assert_eq!(message, expected);
            assert!(
                result
                    .output
                    .diagnostics
                    .iter()
                    .any(|row| row.message == expected)
            );
            failed = true;
            break;
        }
    }
    assert!(failed);
    assert!(engine.invocation_result.is_none());
    let before = engine.inert_rollback_image().unwrap();
    let result = engine.step(Default::default(), options);
    assert!(matches!(
        result.fiber_status,
        crate::engine::FlowFiberStatus::Failed(_)
    ));
    assert!(engine.take_function_result(site).unwrap().is_none());
    assert_eq!(engine.inert_rollback_image().unwrap(), before);
}
#[test]
fn aot_exhausted_executable_frame_retains_native_fallthrough_rejection_and_cleanup() {
    use crate::plan::{
        RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeFlowOpSeed,
        RuntimeFunctionSiteBodySeed, RuntimeFunctionSiteDeclarationSeed, RuntimePlanBuilder,
        RuntimePlanTypeProjection, RuntimePlanTypeSeed,
    };
    let ty = crate::pattern::RuntimeSemanticTypeId::from_bytes([0xee; 32]);
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
    let reserved = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                [0xef; 32],
            ),
            role: crate::plan::RuntimeFunctionSemanticRole::Ordinary,
            function_type: None,
            inputs: Box::new([]),
            result: ty,
            body_kind: RuntimeFunctionSiteBodyKind::Executable,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    builder
        .define_function_site_seed(
            &reserved,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([RuntimeFlowOpSeed::Noop]),
            }),
        )
        .unwrap();
    let plan = Arc::new(builder.finish().unwrap());
    let site = plan.function_sites().iter_with_ids().next().unwrap().0;
    let function = crate::pure::RuntimePureFunctionRef::resolve(&plan, site).unwrap();
    let aot = crate::aot::AotProgram::from_runtime_plan(&plan);
    let options = crate::step::RuntimeStepOptions {
        mode: crate::step::RuntimeStepMode::Drain,
        budget: crate::step::RuntimeStepBudget { max_ops: 8 },
        ..Default::default()
    };
    let expected = RuntimeEvalError::FunctionFallthrough { site }.to_string();
    for linear in [false, true] {
        let mut engine = Engine::for_function_invocation(function, vec![]).unwrap();
        let mut backend = crate::pure::VmRuntimePureCallBackend::default();
        let result = if linear {
            engine
                .step_prechecked_aot_linear_with_pure_backend(&aot, options, &mut backend)
                .0
        } else {
            engine.step_with_pure_backend(Default::default(), options, &mut backend)
        };
        assert_eq!(
            result.stats.executed_ops, 2,
            "the Noop and owning fallthrough dispatch are both charged"
        );
        assert!(
            matches!(result.fiber_status, crate::engine::FlowFiberStatus::Failed(ref message) if message == &expected)
        );
        assert!(
            result
                .output
                .diagnostics
                .iter()
                .any(|row| row.message == expected)
        );
        assert!(
            engine.fiber.control_stack.is_empty(),
            "the original function frame is unwound"
        );
        assert!(engine.fiber.pending_ops.is_empty());
        assert!(engine.take_function_result(site).unwrap().is_none());
        let failed = engine.inert_rollback_image().unwrap();
        let repeated = engine.step_with_pure_backend(Default::default(), options, &mut backend);
        assert!(matches!(
            repeated.fiber_status,
            crate::engine::FlowFiberStatus::Failed(_)
        ));
        assert_eq!(engine.inert_rollback_image().unwrap(), failed);
    }
}

#[cfg(test)]
mod synchronous_executable_boundary_tests {
    use super::*;
    use crate::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
    use crate::pure::{
        RuntimeExternalCallContext, RuntimeI32Args, RuntimeI64Args, RuntimePureFunctionRef,
        RuntimePureScalarInteger,
    };
    use crate::step::RuntimePureCallStats;
    use crate::value::RuntimeCallTarget;

    struct NoPhysicalCalls;
    impl crate::pure::RuntimeMathCallBackend for NoPhysicalCalls {
        fn call_math_matmul_f32(
            &mut self,
            _lhs: &DenseMatrixF32,
            _rhs: &DenseMatrixF32,
        ) -> Result<DenseMatrixF32, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_math_matrix_add_f32(
            &mut self,
            _lhs: &DenseMatrixF32,
            _rhs: &DenseMatrixF32,
        ) -> Result<DenseMatrixF32, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_math_tensor_add_f32(
            &mut self,
            _lhs: &DenseTensorF32,
            _rhs: &DenseTensorF32,
        ) -> Result<DenseTensorF32, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_math_matmul_f64(
            &mut self,
            _lhs: &DenseMatrixF64,
            _rhs: &DenseMatrixF64,
        ) -> Result<DenseMatrixF64, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_math_matrix_add_f64(
            &mut self,
            _lhs: &DenseMatrixF64,
            _rhs: &DenseMatrixF64,
        ) -> Result<DenseMatrixF64, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_math_tensor_add_f64(
            &mut self,
            _lhs: &DenseTensorF64,
            _rhs: &DenseTensorF64,
        ) -> Result<DenseTensorF64, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
    }
    impl crate::pure::RuntimePureCallBackend for NoPhysicalCalls {
        fn record_awbc_pure_program_call(&mut self) {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i8_slice(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _args: &[i8],
        ) -> Result<Option<i8>, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i8_flat_batch(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[i8],
            _arity: usize,
            _out: &mut [i8],
        ) -> Result<(), RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i8_flat_batch_sum(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[i8],
            _arity: usize,
            _rows: usize,
        ) -> Result<i64, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i16_slice(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _args: &[i16],
        ) -> Result<Option<i16>, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i16_flat_batch(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[i16],
            _arity: usize,
            _out: &mut [i16],
        ) -> Result<(), RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i16_flat_batch_sum(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[i16],
            _arity: usize,
            _rows: usize,
        ) -> Result<i64, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i128_flat_batch(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[i128],
            _arity: usize,
            _out: &mut [i128],
        ) -> Result<(), RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i128_flat_batch_sum(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[i128],
            _arity: usize,
            _rows: usize,
        ) -> Result<i64, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i32(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _args: RuntimeI32Args,
        ) -> Result<Option<i32>, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i32_slice(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _args: &[i32],
        ) -> Result<Option<i32>, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i32_flat_batch(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[i32],
            _arity: usize,
            _out: &mut [i32],
        ) -> Result<(), RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i32_flat_batch_sum(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[i32],
            _arity: usize,
            _rows: usize,
        ) -> Result<i64, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_u32_slice(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _args: &[u32],
        ) -> Result<Option<u32>, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_u8_slice(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _args: &[u8],
        ) -> Result<Option<u8>, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_u8_flat_batch(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[u8],
            _arity: usize,
            _out: &mut [u8],
        ) -> Result<(), RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_u8_flat_batch_sum(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[u8],
            _arity: usize,
            _rows: usize,
        ) -> Result<i64, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_u16_slice(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _args: &[u16],
        ) -> Result<Option<u16>, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_u16_flat_batch(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[u16],
            _arity: usize,
            _out: &mut [u16],
        ) -> Result<(), RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_u16_flat_batch_sum(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[u16],
            _arity: usize,
            _rows: usize,
        ) -> Result<i64, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_u128_flat_batch(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[u128],
            _arity: usize,
            _out: &mut [u128],
        ) -> Result<(), RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_u128_flat_batch_sum(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[u128],
            _arity: usize,
            _rows: usize,
        ) -> Result<i64, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_u32_flat_batch(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[u32],
            _arity: usize,
            _out: &mut [u32],
        ) -> Result<(), RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_u32_flat_batch_sum(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[u32],
            _arity: usize,
            _rows: usize,
        ) -> Result<i64, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_u64_slice(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _args: &[u64],
        ) -> Result<Option<u64>, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_u64_flat_batch(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[u64],
            _arity: usize,
            _out: &mut [u64],
        ) -> Result<(), RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_u64_flat_batch_sum(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[u64],
            _arity: usize,
            _rows: usize,
        ) -> Result<i64, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_exact_int_flat_batch_sum<T: RuntimePureScalarInteger>(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[T],
            _arity: usize,
            _rows: usize,
        ) -> Result<i64, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_exact_int_slice<T: RuntimePureScalarInteger>(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _args: &[T],
        ) -> Result<Option<T>, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_exact_int_flat_batch<T: RuntimePureScalarInteger>(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[T],
            _arity: usize,
            _out: &mut [T],
        ) -> Result<(), RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i64(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _args: RuntimeI64Args,
        ) -> Result<Option<i64>, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i64_slice(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _args: &[i64],
        ) -> Result<Option<i64>, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i64_batch(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _rows: &[RuntimeI64Args],
            _out: &mut [i64],
        ) -> Result<(), RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i64_flat_batch(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[i64],
            _arity: usize,
            _out: &mut [i64],
        ) -> Result<(), RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i64_flat_batch_sum(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[i64],
            _arity: usize,
            _rows: usize,
        ) -> Result<i64, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_i64_repeated_flat_batch_sum(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _row: &[i64],
            _rows: usize,
        ) -> Result<i64, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_f32_slice(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _args: &[f32],
        ) -> Result<Option<f32>, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_f32_flat_batch(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[f32],
            _arity: usize,
            _out: &mut [f32],
        ) -> Result<(), RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_f64_slice(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _args: &[f64],
        ) -> Result<Option<f64>, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_f64_flat_batch(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _flat_inputs: &[f64],
            _arity: usize,
            _out: &mut [f64],
        ) -> Result<(), RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn call_values(
            &mut self,
            _helper: RuntimePureFunctionRef<'_>,
            _args: Vec<RuntimeValue>,
        ) -> Result<RuntimeValue, RuntimeEvalError> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
        fn stats(&self) -> RuntimePureCallStats {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
    }
    impl crate::pure::RuntimeExternalCallBackend for NoPhysicalCalls {
        fn call_external(
            &mut self,
            _context: &RuntimeExternalCallContext,
            _callee: &RuntimeCallTarget,
            _args: &[RuntimeValue],
        ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
            panic!(
                "synchronous executable ingress must not invoke a physical backend without a caller budget"
            )
        }
    }
    #[test]
    fn synchronous_executable_ingress_without_a_driver_retains_control_transfer_and_environment() {
        let (plan, site) = scalar_executable_invocation_fixture(false);
        let mut engine = Engine::new_with_shared_plan(plan, crate::task::GenerationId::new(0));
        let before = engine.inert_rollback_image().unwrap();
        let error = engine
            .evaluate_function_site(
                site,
                vec![],
                vec![RuntimeValue::i64(4), RuntimeValue::i64(999)],
                &mut NoPhysicalCalls,
            )
            .unwrap_err();
        assert!(
            matches!(error, RuntimeEvalError::UnsupportedPure { ref name, ref reason }
            if name == "structured.function" && reason == "an executable runtime function requires function-call control transfer")
        );
        assert_eq!(
            engine.inert_rollback_image().unwrap(),
            before,
            "declining synchronous ingress must retain the original environment, owner and return state"
        );
    }
}
