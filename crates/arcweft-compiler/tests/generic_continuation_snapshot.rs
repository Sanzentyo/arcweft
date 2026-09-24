use arcweft_bundle::{ArcweftBundle, BundleFormat, BundleManifest, BundleRuntimeSummary};
use arcweft_compiler::source::compile_source;
use arcweft_core::awbc::fiber::FiberState;
use arcweft_core::awbc::schema::{AwbcProgram, AwbcRuntimeTypeShape, AwbcSignedIntKind};
use arcweft_core::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use arcweft_core::engine::{Engine, FlowExit, FlowFiberStatus};
use arcweft_core::plan::{
    RuntimeCallableParameterCoordinate, RuntimeCallablePosition, RuntimeCallableRetainedRole,
    RuntimePlanTypeProjection,
};
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
};
use arcweft_core::task::{GenerationId, RuntimeProgramOwner};
use arcweft_core::value::{
    AwbcRuntimeCallableSnapshot, AwbcRuntimeValueSnapshot, RuntimeCallableValue,
    RuntimeSignedIntWidth, RuntimeValue,
};
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::session::{BundleSession, BundleSessionOptions, BundleStepInput};
use arcweft_runtime_driver::session_save::BundleSessionSaveError;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

const SOURCE: &str = r#"
entry cli @entry.main { goto @flow.main }
fn choose<A, B>(first: A)(second: B) -> B { second }
flow main() -> bool {
    let prefix = choose(7i64)
    let text = prefix("text")
    let integer = prefix(42i64)
    return text == "text" && integer == 42i64
}
"#;

#[test]
fn native_generic_continuation_keeps_its_scheme_origin_position_and_prefix() {
    let compiled = compile_source(SOURCE).expect("generic continuation source compiles");
    let [flow] = compiled.plan.flows() else {
        panic!("fixture has one flow");
    };
    let mut engine = Engine::for_flow(compiled.plan.clone(), &flow.id)
        .expect("generic continuation flow starts");
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
    };

    let mut observed_prefix = false;
    for _ in 0..1024 {
        let prefix = engine
            .fiber()
            .env
            .bindings_snapshot()
            .into_iter()
            .find_map(|binding| match binding.value {
                RuntimeValue::Callable(value) if is_i64_prefix(&value) => Some(value),
                _ => None,
            });
        if let Some(prefix) = prefix {
            assert_plan_prefix_scheme(&compiled.plan, &prefix);
            observed_prefix = true;
            break;
        }
        if matches!(&engine.fiber().status, FlowFiberStatus::Done(_)) {
            break;
        }
        let result = engine.step(RuntimeStepInput::default(), options);
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
    }
    assert!(
        observed_prefix,
        "native execution exposes the saved generic prefix"
    );

    for _ in 0..2048 {
        if matches!(&engine.fiber().status, FlowFiberStatus::Done(_)) {
            break;
        }
        let result = engine.step(RuntimeStepInput::default(), options);
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
    }
    assert_eq!(
        engine.fiber().status,
        FlowFiberStatus::Done(FlowExit::Return("true".to_owned()))
    );
}

#[test]
fn awfb_session_save_restores_the_generic_prefix_into_its_pinned_program() {
    let bundle_bytes = awfb_bytes([0x7d; 32]);
    let mut session = session_from_bytes(&bundle_bytes);
    let (snapshot, prefix) = advance_to_awbc_prefix(&mut session);
    let original_owner = session.program_owner();
    let RuntimeProgramOwner::Awbc(original_program) = &original_owner else {
        panic!("AWFB session is owned by Product AWBC");
    };
    assert_awbc_prefix_scheme(original_program, &prefix);
    assert_eq!(
        snapshot.runtime.runtime_generation_pin,
        Some(snapshot.generation.active_generation)
    );
    assert_eq!(
        snapshot.executor.generation,
        snapshot.generation.active_generation
    );

    let save = session
        .export_session_save_bytes()
        .expect("generic-prefix session save exports");
    let mut restored = session_from_bytes(&bundle_bytes);
    restored
        .import_session_save_bytes(&save, &Default::default())
        .expect("generic-prefix session save restores");

    let restored_snapshot = restored
        .snapshot_session()
        .expect("restored generic-prefix snapshot exports");
    let RuntimeProgramOwner::Awbc(restored_program) = restored.program_owner() else {
        panic!("AWFB session is owned by Product AWBC");
    };
    let restored_prefix =
        find_awbc_prefix(&restored_snapshot.executor.state.fiber, &restored_program)
            .expect("restored fiber retains the generic prefix");
    let RuntimeProgramOwner::Awbc(restored_program) = restored_prefix.owner() else {
        panic!("restored callable is AWBC-owned");
    };
    assert_awbc_prefix_scheme(restored_program, restored_prefix);
    assert_eq!(restored_prefix.state(), prefix.state());
    assert_eq!(restored_prefix.retained(), prefix.retained());
    let original_state = &original_program.callable_states[prefix.state().index()];
    let restored_state = &restored_program.callable_states[restored_prefix.state().index()];
    assert_eq!(restored_state.origin, original_state.origin);
    assert_eq!(restored_state.position, original_state.position);
    assert_eq!(restored_state.function_type, original_state.function_type);
    assert_eq!(restored_state.retained, original_state.retained);
    assert!(!restored_prefix.owner().same_program(prefix.owner()));
    assert!(
        restored_prefix
            .owner()
            .same_program(&restored.program_owner())
    );

    for _ in 0..2048 {
        if restored.is_finished() {
            break;
        }
        let step = restored.step_with_clock(
            RuntimeClockStep::from_millis(1, 16).expect("clock step"),
            BundleStepInput::default(),
        );
        assert!(step.diagnostics.is_empty(), "{:?}", step.diagnostics);
    }
    assert!(restored.is_finished(), "restored calls finish");
    let completed = restored
        .snapshot_session()
        .expect("completed session snapshot exports");
    assert!(matches!(
        completed.executor.state.fiber.terminal,
        Some(arcweft_core::awbc::fiber::FiberTerminalValue::Returned(
            Some(RuntimeValue::Bool(true))
        ))
    ));
}

#[test]
fn awbc_callable_snapshot_rejects_a_forged_retained_type() {
    let bundle_bytes = awfb_bytes([0x7d; 32]);
    let mut session = session_from_bytes(&bundle_bytes);
    let (snapshot, prefix) = advance_to_awbc_prefix(&mut session);
    let owner = session.program_owner();

    let forged = AwbcRuntimeValueSnapshot::Callable(AwbcRuntimeCallableSnapshot {
        state: prefix.state(),
        retained: vec![AwbcRuntimeValueSnapshot::String("wrong type".to_owned())],
    });
    let error = forged
        .into_runtime_value_for_program(&owner)
        .expect_err("restored retained prefix type is checked against the pinned AWBC state");
    assert!(error.to_string().contains("retained value 0"), "{error}");

    let before = session
        .snapshot_session()
        .expect("live session snapshot remains available");
    let mut wrong_generation = snapshot;
    wrong_generation.runtime.runtime_generation_pin = Some(GenerationId::new(
        wrong_generation
            .generation
            .active_generation
            .get()
            .saturating_add(1),
    ));
    let error = session
        .restore_session_snapshot(wrong_generation)
        .expect_err("a callable snapshot cannot move to a different pinned generation");
    assert!(matches!(
        error,
        BundleSessionSaveError::GenerationMismatch {
            field: "runtime_generation_pin",
            ..
        }
    ));
    assert_eq!(
        session
            .snapshot_session()
            .expect("rejected generation restore leaves live session unchanged"),
        before
    );

    let save = session
        .export_session_save_bytes()
        .expect("generic-prefix session save exports");
    let mut foreign_session = session_from_bytes(&awfb_bytes([0x7e; 32]));
    let error = foreign_session
        .import_session_save_bytes(&save, &Default::default())
        .expect_err("another AWFB artifact cannot adopt this generic continuation");
    assert!(matches!(
        error,
        BundleSessionSaveError::GenerationMismatch {
            field: "artifact",
            ..
        }
    ));
}

#[test]
fn awbc_verifier_rejects_a_generic_bound_type_at_a_forged_depth() {
    let mut program = lowered_program();
    let (index, ty) = program
        .runtime_types
        .iter()
        .enumerate()
        .find(|(_, ty)| matches!(ty.shape(), AwbcRuntimeTypeShape::BoundType(_)))
        .expect("generic scheme emits a bound type row");
    program.runtime_types[index] = ty
        .clone()
        .with_scope(arcweft_core::plan::RuntimeTypeScope::root());

    assert!(
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .is_err()
    );
}

fn advance_to_awbc_prefix(
    session: &mut BundleSession,
) -> (
    arcweft_runtime_driver::session_save::BundleSessionSnapshot,
    RuntimeCallableValue,
) {
    for _ in 0..2048 {
        let step = session.step_with_clock(
            RuntimeClockStep::from_millis(1, 16).expect("clock step"),
            BundleStepInput::default(),
        );
        assert!(step.diagnostics.is_empty(), "{:?}", step.diagnostics);
        let snapshot = session
            .snapshot_session()
            .expect("budget-yielded session is saveable");
        let RuntimeProgramOwner::Awbc(program) = session.program_owner() else {
            panic!("AWFB session is owned by Product AWBC");
        };
        if let Some(prefix) = find_awbc_prefix(&snapshot.executor.state.fiber, &program).cloned() {
            return (snapshot, prefix);
        }
    }
    panic!("AWFB execution did not retain the generic prefix");
}

fn find_awbc_prefix<'a>(
    fiber: &'a FiberState,
    program: &AwbcProgram,
) -> Option<&'a RuntimeCallableValue> {
    fiber
        .frames
        .iter()
        .flat_map(|frame| frame.registers.iter().flatten())
        .filter_map(|value| match value {
            RuntimeValue::Callable(callable) => Some(callable),
            _ => None,
        })
        .find(|callable| {
            is_i64_prefix(callable)
                && program
                    .callable_states
                    .get(callable.state().index())
                    .is_some_and(|state| {
                        state.position == (RuntimeCallablePosition::AfterGroup { completed: 0 })
                    })
        })
}

fn is_i64_prefix(callable: &RuntimeCallableValue) -> bool {
    callable.retained() == [RuntimeValue::i64(7)]
}

fn assert_plan_prefix_scheme(
    plan: &arcweft_core::plan::RuntimePlan,
    prefix: &RuntimeCallableValue,
) {
    let state = plan
        .callable_states()
        .get(prefix.state())
        .expect("native continuation state belongs to its plan");
    assert_eq!(
        state.position,
        RuntimeCallablePosition::AfterGroup { completed: 0 }
    );
    assert_eq!(state.retained.len(), 1);
    assert_eq!(
        state.retained[0].role,
        RuntimeCallableRetainedRole::Parameter(RuntimeCallableParameterCoordinate {
            group: 0,
            parameter: 0,
        })
    );
    assert!(matches!(
        plan.type_table()
            .get(state.retained[0].ty)
            .expect("native prefix type is admitted")
            .projection(),
        RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64)
    ));

    let origin = plan
        .callable_states()
        .get(state.origin)
        .expect("native generic scheme origin belongs to its plan");
    assert_eq!(origin.origin, state.origin);
    assert_eq!(origin.position, RuntimeCallablePosition::Unapplied);
    let root_type = plan
        .type_table()
        .get(origin.function_type)
        .expect("native origin function type is admitted");
    let RuntimePlanTypeProjection::Function { contract, .. } = root_type.projection() else {
        panic!("native origin keeps a function scheme");
    };
    assert_eq!(contract.binder().types(), 2);

    let residual_type = plan
        .type_table()
        .get(state.function_type)
        .expect("native residual function type is admitted");
    let RuntimePlanTypeProjection::Function {
        contract,
        parameters,
        ..
    } = residual_type.projection()
    else {
        panic!("native continuation keeps a residual function scheme");
    };
    assert_eq!(contract.binder().types(), 1);
    assert_eq!(parameters.len(), 1);
    let RuntimePlanTypeProjection::BoundType(bound) = plan
        .type_table()
        .get(parameters[0])
        .expect("native residual parameter type is admitted")
        .projection()
    else {
        panic!("native residual parameter remains generic");
    };
    assert_eq!((bound.depth(), bound.slot()), (0, 0));
}

fn assert_awbc_prefix_scheme(program: &AwbcProgram, prefix: &RuntimeCallableValue) {
    let state = program
        .callable_states
        .get(prefix.state().index())
        .expect("AWBC continuation state belongs to its program");
    assert_eq!(
        state.position,
        RuntimeCallablePosition::AfterGroup { completed: 0 }
    );
    assert_eq!(state.retained.len(), 1);
    assert_eq!(
        state.retained[0].role,
        RuntimeCallableRetainedRole::Parameter(RuntimeCallableParameterCoordinate {
            group: 0,
            parameter: 0,
        })
    );
    assert!(matches!(
        program.runtime_types[state.retained[0].ty.index()].shape(),
        AwbcRuntimeTypeShape::Int(AwbcSignedIntKind::I64)
    ));

    let origin = program
        .callable_states
        .get(state.origin.index())
        .expect("AWBC generic scheme origin belongs to its program");
    assert_eq!(origin.origin, state.origin);
    assert_eq!(origin.position, RuntimeCallablePosition::Unapplied);
    let AwbcRuntimeTypeShape::Function { contract, .. } =
        program.runtime_types[origin.function_type.index()].shape()
    else {
        panic!("AWBC origin keeps a function scheme");
    };
    assert_eq!(contract.binder().types(), 2);

    let AwbcRuntimeTypeShape::Function {
        contract,
        parameters,
        ..
    } = program.runtime_types[state.function_type.index()].shape()
    else {
        panic!("AWBC continuation keeps a residual function scheme");
    };
    assert_eq!(contract.binder().types(), 1);
    assert_eq!(parameters.len(), 1);
    let AwbcRuntimeTypeShape::BoundType(bound) =
        program.runtime_types[parameters[0].index()].shape()
    else {
        panic!("AWBC residual parameter remains generic");
    };
    assert_eq!((bound.depth(), bound.slot()), (0, 0));
}

fn lowered_program() -> AwbcProgram {
    let compiled = compile_source(SOURCE).expect("generic continuation source compiles");
    AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "generic_continuation_snapshot.arcw",
    )
    .lower()
    .expect("generic continuation lowers to verified AWBC")
    .program
}

fn awfb_bytes(artifact_fingerprint: [u8; 32]) -> Vec<u8> {
    let compiled = compile_source(SOURCE).expect("generic continuation source compiles");
    let character_dialogue_generation = compiled
        .character_dialogue_generation
        .as_ref()
        .expect("source compilation retains a CharacterDialogue generation")
        .as_ref()
        .clone();
    let program = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "generic_continuation_snapshot.arcw",
    )
    .lower()
    .expect("generic continuation lowers to verified AWBC")
    .program;
    let instruction_count = program.instructions.len();
    let accepted_product = compiled.dialogue_profile.product();
    let bundle = ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: Some("entry.main".to_owned()),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                artifact_fingerprint:
                    arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes(
                        artifact_fingerprint,
                    )
                    .expect("artifact fingerprint is non-zero"),
                entry_flow: Some("flow.main".to_owned()),
                flows: compiled.plan.flows().len(),
                bytecode_instructions: instruction_count,
                line_task_groups: 0,
                stream_plans: 0,
            },
        },
        accepted_product.source_map().clone(),
        program,
        compiled.dialogue_content,
    )
    .expect("compiled Product AWBC forms a bundle");
    let bundle = bundle
        .try_with_validated_view_product(accepted_product.as_ref())
        .expect("accepted source View and Style product joins Product AWBC");
    let bundle = bundle.with_character_dialogue_generation(character_dialogue_generation);
    bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("bundle encodes as AWFB")
}

fn session_from_bytes(bytes: &[u8]) -> BundleSession {
    BundleSession::from_awfb_bytes(
        bytes,
        BundleSessionOptions {
            max_ops: 1,
            ..BundleSessionOptions::default()
        },
    )
    .expect("compiled AWFB session starts")
}
