use std::sync::Arc;

use crate::engine::Engine;
use crate::plan::RuntimeFunctionSiteBodyKind;
use crate::step::RuntimeStepOutput;
use crate::tests::function_application::{
    returning_callable_state, returning_function_plan, returning_function_result,
};
use crate::value::{RuntimeCallableValue, RuntimeEvalError, RuntimeValue};

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
    assert_eq!(engine.take_program_result(), None);
    engine.step_with_pure_backend(
        crate::step::RuntimeStepInput::default(),
        crate::step::RuntimeStepOptions::default(),
        &mut backend,
    );
    let saved = engine.inert_rollback_image().unwrap();
    assert_eq!(
        engine.take_program_result(),
        Some((program, RuntimeValue::Bool(true)))
    );
    assert_eq!(engine.take_program_result(), None);
    let mut restored = Engine::from_rollback_image(saved).unwrap();
    assert_eq!(
        restored.take_program_result(),
        Some((program, RuntimeValue::Bool(true)))
    );
    assert_eq!(restored.take_program_result(), None);
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
        source: RuntimeFunctionInputSource::Parameter { position: 0 },
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
    let value = RuntimeValue::Need(crate::task::NeedId("need.owned-program".to_owned()));
    assert!(!value.ownership().permits_copy());
    let mut engine = Engine::for_program_invocation(plan, program, vec![value]).unwrap();
    let output = engine.step(
        crate::step::RuntimeStepInput::default(),
        crate::step::RuntimeStepOptions::default(),
    );
    assert!(output.output.diagnostics.is_empty());
    let saved = engine.inert_rollback_image().unwrap();
    assert_eq!(
        engine.take_program_result(),
        Some((
            program,
            RuntimeValue::Need(crate::task::NeedId("need.owned-program".to_owned()))
        ))
    );
    assert_eq!(engine.take_program_result(), None);
    let mut restored = Engine::from_rollback_image(saved).unwrap();
    assert_eq!(
        restored.take_program_result(),
        Some((
            program,
            RuntimeValue::Need(crate::task::NeedId("need.owned-program".to_owned()))
        ))
    );
    assert_eq!(restored.take_program_result(), None);
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
