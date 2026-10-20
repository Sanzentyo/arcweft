//! Rust declarations, accepted callables and codec defaults share one producer.

use super::*;
use std::{cell::RefCell, collections::BTreeMap};

use arcweft_adapter_context::manifest::AdapterNominalPathPrefix;
use arcweft_core::{
    awbc::{codec::AwbcDecodeBudget, schema::AwbcProgram},
    entry::{RuntimeCallableId, RuntimeSchemaLimits},
    pattern::RuntimeSemanticTypeId,
    plan::RuntimePlan,
    program_types::{RuntimeProgramDataShapes, RuntimeProgramTypes},
    pure::{RuntimeExternalCallBackend, RuntimeExternalCallContext, VmRuntimePureCallBackend},
    task::RuntimeProgramOwner,
    value::{RuntimeCallTarget, RuntimeEvalError, RuntimeExprKind, RuntimeValue},
};
use arcweft_data::{
    DecodeShapeAccess, FieldDefaultProvider, FieldDefaultRequest, RawValue, Value,
    decode_with_shape_ref,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_rust_abi::{
    ArcweftRustManifest, ArcweftRustPackage, ArcweftRustPackageId, ArcweftRustPurity,
    ArcweftRustTypeRef,
};
use arcweft_rust_abi_macros::{ArcweftType, arcweft_export, arcweft_export_default};

arcweft_export_default!(pure, pub fn default_bool() -> bool);
arcweft_export_default!(pure, pub fn default_string() -> String);

#[derive(ArcweftType)]
pub struct PrimitiveDefaults {
    #[arcweft(default)]
    flag: bool,
    #[arcweft(default)]
    text: String,
}

#[derive(ArcweftType)]
pub struct GenericDefault<T> {
    #[arcweft(default)]
    value: T,
}

#[arcweft_export(pure)]
fn primitive_defaults() -> PrimitiveDefaults {
    PrimitiveDefaults {
        flag: default_bool(),
        text: default_string(),
    }
}

#[arcweft_export(pure)]
fn generic_default() -> GenericDefault<bool> {
    GenericDefault {
        value: default_bool(),
    }
}

#[derive(ArcweftType)]
pub struct Marker(bool);

#[arcweft_export(pure, name = "default_marker")]
impl Default for Marker {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(ArcweftType)]
pub struct Settings {
    #[arcweft(default = "default_flag")]
    flag: bool,
    #[arcweft(default)]
    marker: Marker,
    #[arcweft(skip)]
    cache: Marker,
}

#[arcweft_export(pure)]
fn default_flag() -> bool {
    true
}

#[arcweft_export(pure)]
fn settings() -> Settings {
    Settings {
        flag: default_flag(),
        marker: default_marker(),
        cache: default_marker(),
    }
}

fn metadata() -> ArcweftRustManifest {
    ArcweftRustManifest::builder(ArcweftRustPackage {
        id: ArcweftRustPackageId::try_new(env!("CARGO_PKG_NAME")).unwrap(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        metadata_hash: None,
    })
    .with_type_metadata::<Marker>()
    .with_type_metadata::<Settings>()
    .with_type_metadata::<PrimitiveDefaults>()
    .with_type_metadata::<GenericDefault<bool>>()
    .with_function(__arcweft_export_default_bool_metadata())
    .with_function(__arcweft_export_default_string_metadata())
    .with_function(__arcweft_export_primitive_defaults_metadata())
    .with_function(__arcweft_export_generic_default_metadata())
    .with_function(__arcweft_export_default_marker_metadata())
    .with_function(__arcweft_export_default_flag_metadata())
    .with_function(__arcweft_export_settings_metadata())
    .build()
}

fn compile_defaults(
    rust: &ArcweftRustManifest,
) -> Result<(Arc<RuntimePlan>, Arc<AwbcProgram>), String> {
    compile_rust(
        rust,
        "flow main { let value = settings(); }\nentry cli @entry.cli.main { goto @flow.main }\n",
    )
}

fn compile_rust_project(
    rust: &ArcweftRustManifest,
    source: &str,
    fixture_name: &str,
) -> Result<CompiledProject, String> {
    let manifest = AdapterManifest::new("rust-defaults", "Rust default integration")
        .try_with_rust_package_mount(
            rust.package.id.clone(),
            AdapterNominalPathPrefix::try_new([]).unwrap(),
        )
        .unwrap()
        .try_with_rust_manifest(rust)
        .map_err(|error| error.to_string())?;
    let (project, registration, env) = fixture_with_manifest(source, fixture_name, &manifest);
    let context = context(env, registration).with_command_policy(
        arcweft_core::entry::RuntimeCommandPolicy::deny_all(
            arcweft_core::entry::RootExecutionLimits::engine_default(),
        ),
    );
    AttachedCompiler::new(&project)
        .compile(&project, &context, &mut RecordingCache::default())
        .map_err(|error| format!("{error:?}"))
}

pub(super) fn compile_rust(
    rust: &ArcweftRustManifest,
    source: &str,
) -> Result<(Arc<RuntimePlan>, Arc<AwbcProgram>), String> {
    let compiled = compile_rust_project(rust, source, "rust-default-runtime")?;
    let lowered = compiled.runtime_plan();
    let program = AwbcLowerer::new(
        &lowered.plan,
        &lowered.dialogue_content_catalog,
        "rust-defaults",
    )
    .lower()
    .map_err(|error| format!("{error:?}"))?
    .program;
    let wire = program
        .encode_canonical()
        .map_err(|error| error.to_string())?;
    let program = AwbcProgram::decode_canonical(&wire, AwbcDecodeBudget::default())
        .map_err(|error| error.to_string())?;
    Ok((Arc::new(lowered.plan.clone()), Arc::new(program)))
}

#[derive(Clone, Copy)]
enum Factory {
    Flag,
    Marker,
    BoolDefault,
    StringDefault,
}

struct RustDefaults {
    owner: RuntimeProgramOwner,
    factories: BTreeMap<RuntimeCallableId, Factory>,
    wrong_result: bool,
}

struct ViewLocaleProbe {
    owner: RuntimeProgramOwner,
    observations: std::rc::Rc<RefCell<ViewLocaleObservations>>,
}

#[derive(Default)]
struct ViewLocaleObservations {
    locales: Vec<arcweft_id::LocaleTag>,
    targets: Vec<RuntimeCallTarget>,
}

impl RuntimeExternalCallBackend for ViewLocaleProbe {
    fn call_external(
        &mut self,
        context: &RuntimeExternalCallContext,
        target: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        assert!(target.callable_identity().is_some());
        assert!(args.is_empty());
        let execution = context.execution_context().unwrap();
        assert!(execution.program_owner().same_program(&self.owner));
        assert!(matches!(
            execution
                .types()
                .checked_type(context.result_type().unwrap())
                .unwrap(),
            arcweft_core::pattern::RuntimeCheckedType::String,
        ));
        let locale = execution.format_context().active_locale().clone();
        let result = RuntimeValue::String(locale.as_str().to_owned());
        let mut observations = self.observations.borrow_mut();
        observations.locales.push(locale);
        observations.targets.push(target.clone());
        Some(Ok(result))
    }
}

fn assert_view_locale(frame: &arcweft_runtime_driver::view_runtime::BundleViewFrame, tag: &str) {
    assert!(frame.diagnostics.is_empty(), "{frame:?}");
    assert_eq!(frame.mounts.len(), 1);
    assert_eq!(frame.mounts[0].text.len(), 1);
    assert_eq!(
        frame.mounts[0].text[0].value,
        arcweft_runtime_driver::view_runtime::BundleViewTextValue::Plain {
            value: tag.to_owned()
        },
    );
}

const ROOT_LOCALE_SOURCE: &str = r"
struct GameState { locale: String }
enum GameEvent { Start }
fn initial_game_state() -> GameState effects {} {
    let text = default_string();
    GameState { locale = text }
}
fn reduce_game(current: &GameState, event: GameEvent)
    -> Result<Reduction<GameState>, ReducerError> effects {} {
    Ok(Reduction.unchanged(current))
}
flow @flow.opening opening(current: GameState) {}
entry game @entry.game.main {
    state = GameState
    initializer = initial_game_state
    event = GameEvent
    reducer = reduce_game
    goto @flow.opening
}
";

#[test]
fn native_root_initializer_uses_budgeted_frame_and_selected_rust_locale() {
    use arcweft_core::{
        engine::{Engine, FlowFiberStatus},
        root::RootPhase,
        step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
        task::GenerationId,
        value::RuntimeFormatContext,
    };

    let (plan, _) = compile_rust(&metadata(), ROOT_LOCALE_SOURCE).unwrap();
    let entry = plan.entries()[0].id.clone();
    let mut engine = Engine::new_with_generation(Arc::clone(&plan), GenerationId::new(0));
    engine.start_entry(&entry).unwrap();
    assert!(matches!(
        engine.root().unwrap().active().phase,
        RootPhase::Initializing { .. }
    ));
    let observations = std::rc::Rc::new(RefCell::new(ViewLocaleObservations::default()));
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(ViewLocaleProbe {
        owner: RuntimeProgramOwner::Plan(Arc::clone(&plan)),
        observations: std::rc::Rc::clone(&observations),
    });
    engine.set_format_context(RuntimeFormatContext::new(
        arcweft_id::LocaleTag::try_new("ja-JP").unwrap(),
    ));
    let options = |max_ops| RuntimeStepOptions {
        mode: RuntimeStepMode::Drain,
        budget: RuntimeStepBudget { max_ops },
        ..RuntimeStepOptions::default()
    };
    let zero = engine.step_with_pure_backend(RuntimeStepInput::default(), options(0), &mut backend);
    assert_eq!(zero.stats.executed_ops, 0);
    assert!(zero.output.diagnostics.is_empty());
    assert!(zero.output.root_initializations.is_empty());
    assert!(observations.borrow().locales.is_empty());
    let mut completed = false;
    for _ in 0..64 {
        let result =
            engine.step_with_pure_backend(RuntimeStepInput::default(), options(1), &mut backend);
        assert!(result.output.diagnostics.is_empty(), "{:?}", result.output);
        assert!(result.stats.executed_ops <= 1);
        if let [outcome] = &result.output.root_initializations[..] {
            assert_eq!(outcome.entry, entry);
            assert_eq!(result.stats.executed_ops, 1);
            assert!(matches!(result.fiber_status, FlowFiberStatus::Running));
            completed = true;
            break;
        }
    }
    assert!(completed);
    assert_eq!(
        observations
            .borrow()
            .locales
            .iter()
            .map(arcweft_id::LocaleTag::as_str)
            .collect::<Vec<_>>(),
        ["ja-JP"]
    );
    let RuntimeValue::NominalRecord(state) = &engine
        .root()
        .unwrap()
        .active()
        .phase
        .state_value()
        .unwrap()
        .0
    else {
        panic!("nominal state")
    };
    assert_eq!(state.fields(), &[RuntimeValue::String("ja-JP".to_owned())]);
    engine.set_format_context(RuntimeFormatContext::new(
        arcweft_id::LocaleTag::try_new("fr-FR").unwrap(),
    ));
    let next = engine.step_with_pure_backend(RuntimeStepInput::default(), options(1), &mut backend);
    assert!(next.output.root_initializations.is_empty());
    assert_eq!(observations.borrow().locales.len(), 1);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one lifecycle test owns the real Rust call, cold restore, frame identity and exactly-once locale assertions"
)]
fn awbc_root_initializer_resumes_after_rust_call_without_repeating_it() {
    use arcweft_core::{
        awbc::{fiber::AwbcFiberRoot, product_step::AwbcProductStepExecutor, schema::AwbcEntryId},
        root::RootPhase,
        step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
        value::RuntimeFormatContext,
    };
    let (_, code) = compile_rust(&metadata(), ROOT_LOCALE_SOURCE).unwrap();
    let mut executor =
        AwbcProductStepExecutor::for_entry_arc(Arc::clone(&code), AwbcEntryId(0), 64).unwrap();
    let fiber_instance = executor.compact_fiber().instance;
    let initializer_frame = executor.compact_fiber().frames[0].instance;
    assert_eq!(
        executor.compact_fiber().root,
        AwbcFiberRoot::RootInitializer(AwbcEntryId(0))
    );
    assert!(matches!(
        executor.root().unwrap().active().phase,
        RootPhase::Initializing { .. }
    ));
    let observations = std::rc::Rc::new(RefCell::new(ViewLocaleObservations::default()));
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(ViewLocaleProbe {
        owner: RuntimeProgramOwner::Awbc(Arc::clone(&code)),
        observations: std::rc::Rc::clone(&observations),
    });
    let format = RuntimeFormatContext::new(arcweft_id::LocaleTag::try_new("ja-JP").unwrap());
    executor.set_format_context(format.clone());
    let options = |max_ops| RuntimeStepOptions {
        mode: RuntimeStepMode::Drain,
        budget: RuntimeStepBudget { max_ops },
        ..RuntimeStepOptions::default()
    };
    let zero =
        executor.step_with_pure_backend(RuntimeStepInput::default(), options(0), &mut backend);
    assert_eq!(zero.stats.executed_ops, 0);
    assert!(zero.output.diagnostics.is_empty());
    assert!(zero.output.root_initializations.is_empty());
    assert!(observations.borrow().locales.is_empty());
    let mut restored = false;
    let mut completed = false;
    for _ in 0..128 {
        let result =
            executor.step_with_pure_backend(RuntimeStepInput::default(), options(1), &mut backend);
        assert!(result.output.diagnostics.is_empty(), "{:?}", result.output);
        assert!(result.stats.executed_ops <= 1);
        if !restored
            && observations.borrow().locales.len() == 1
            && executor
                .root()
                .unwrap()
                .active()
                .phase
                .state_value()
                .is_none()
        {
            let image = executor.inert_rollback_image().unwrap();
            drop(executor);
            executor =
                AwbcProductStepExecutor::for_entry_arc(Arc::clone(&code), AwbcEntryId(0), 64)
                    .unwrap();
            assert_eq!(observations.borrow().locales.len(), 1);
            executor.set_format_context(format.clone());
            executor = executor.restore_rollback_image(image).unwrap();
            assert_eq!(executor.compact_fiber().instance, fiber_instance);
            restored = true;
        }
        if let [_] = &result.output.root_initializations[..] {
            completed = true;
            break;
        }
    }
    assert!(
        restored,
        "cold restore occurs after the actual Rust call and before initialization commits"
    );
    assert!(completed);
    assert_eq!(
        observations
            .borrow()
            .locales
            .iter()
            .map(arcweft_id::LocaleTag::as_str)
            .collect::<Vec<_>>(),
        ["ja-JP"]
    );
    assert_eq!(
        executor.compact_fiber().root,
        AwbcFiberRoot::Entry(AwbcEntryId(0))
    );
    assert_eq!(executor.compact_fiber().instance, fiber_instance);
    assert_ne!(
        executor.compact_fiber().frames[0].instance,
        initializer_frame
    );
    let RuntimeValue::NominalRecord(state) = &executor
        .root()
        .unwrap()
        .active()
        .phase
        .state_value()
        .unwrap()
        .0
    else {
        panic!("nominal state")
    };
    assert_eq!(state.fields(), &[RuntimeValue::String("ja-JP".to_owned())]);
    let next =
        executor.step_with_pure_backend(RuntimeStepInput::default(), options(1), &mut backend);
    assert!(next.output.root_initializations.is_empty());
    assert_eq!(observations.borrow().locales.len(), 1);
}

#[test]
fn awbc_root_initialization_uses_remaining_drain_allowance_for_initial_flow() {
    use arcweft_core::{
        awbc::{product_step::AwbcProductStepExecutor, schema::AwbcEntryId},
        engine::FlowFiberStatus,
        step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
    };
    let source = ROOT_LOCALE_SOURCE.replace("default_string()", "\"initial\"");
    let (_, code) = compile_rust(&metadata(), &source).unwrap();
    let mut executor = AwbcProductStepExecutor::for_entry_arc(code, AwbcEntryId(0), 64).unwrap();
    let step = executor.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 64 },
            ..RuntimeStepOptions::default()
        },
    );
    assert!(step.output.diagnostics.is_empty(), "{:?}", step.output);
    assert!(step.stats.executed_ops > 1);
    assert!(step.stats.executed_ops <= 64);
    assert_eq!(step.output.root_initializations.len(), 1);
    assert!(matches!(step.fiber_status, FlowFiberStatus::Done(_)));
}

fn root_replay_fixture(
    name: &str,
) -> (
    arcweft_bundle::ArcweftBundle,
    RuntimeValue,
    arcweft_runtime_driver::session::BundleSessionOptions,
) {
    use arcweft_bundle::{ArcweftBundle, BundleManifest, BundleRuntimeSummary};
    use arcweft_core::step::RuntimeStepMode;
    use arcweft_runtime_driver::session::BundleSessionOptions;
    let source = ROOT_LOCALE_SOURCE.replace("default_string()", "\"initial\"");
    let compiled = compile_rust_project(&metadata(), &source, name).unwrap();
    let lowered = compiled.runtime_plan();
    let program = AwbcLowerer::new(
        &lowered.plan,
        &lowered.dialogue_content_catalog,
        "main.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let runtime = BundleRuntimeSummary {
        artifact_fingerprint: arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes(
            *blake3::hash(&program.encode_canonical().unwrap()).as_bytes(),
        )
        .unwrap(),
        entry_flow: None,
        flows: program.flow_executables.len(),
        bytecode_instructions: program.instructions.len(),
        line_task_groups: program.line_task_groups.len(),
        stream_plans: program.stream_plans.len(),
    };
    let mut bundle = ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: Some("entry.game.main".to_owned()),
            adapter: None,
            locale: arcweft_manifest_model::ProjectLocaleSpec::default(),
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime,
        },
        compiled.view_product().product().source_map().clone(),
        program,
        lowered.dialogue_content_catalog.clone(),
    )
    .unwrap();
    bundle = bundle
        .try_with_validated_view_product(compiled.view_product().product())
        .unwrap();
    if let Some(text) = compiled.view_product().text() {
        bundle = bundle.with_view_text(text.clone());
    }
    if let Some(catalog) = &lowered.character_presentation_catalog {
        bundle = bundle.with_character_presentation_catalog(catalog.as_ref().clone());
    }
    if let Some(generation) = &lowered.character_dialogue_generation {
        bundle = bundle.with_character_dialogue_generation(generation.as_ref().clone());
    }
    let program = bundle.product_awbc_program();
    let roles = program.entries[0].roles.stateful().unwrap();
    let event = RuntimeProgramTypes::Awbc(program)
        .try_variant_value(
            roles.event.semantic_identity,
            0,
            None,
            roles.command_policy.root_limits.schema,
        )
        .unwrap();
    let options = BundleSessionOptions {
        mode: RuntimeStepMode::Drain,
        max_ops: 1,
        ..BundleSessionOptions::default()
    };
    (bundle, event, options)
}

#[test]
fn content_only_replacement_retains_the_root_generation_after_its_initial_flow_returns() {
    use arcweft_core::{
        root::{RootEventInput, RootTransitionOutcome},
        task::GenerationId,
    };
    use arcweft_runtime_driver::{
        clock::RuntimeClockStep,
        session::{BundleSession, BundleStepInput},
        session_save::BundleSessionSaveError,
        swap::SwapCompatibility,
    };
    let (first, event, options) = root_replay_fixture("root-content-only-generation");
    let mut second = first.clone();
    second.manifest.profile_id = Some("profile.root.content".to_owned());
    let mut session = BundleSession::new(&first, options).unwrap();
    let original_owner = session.program_owner();
    let mut tick = 0;
    let mut initialized = false;
    for _ in 0..128 {
        tick += 1;
        let step = session.step_with_clock(
            RuntimeClockStep::from_millis(tick, 1).unwrap(),
            BundleStepInput::default(),
        );
        assert!(step.diagnostics.is_empty(), "{:?}", step.diagnostics);
        assert!(step.stats.executed_ops <= 1);
        initialized |= !step.root_initializations.is_empty();
        if step.finished {
            break;
        }
    }
    assert!(initialized);
    assert!(session.is_finished());
    assert_eq!(
        session.current_fiber_generation(),
        Some(GenerationId::new(0))
    );
    let before = session.snapshot_session().unwrap();
    let mut missing_pin = session.snapshot_session().unwrap();
    missing_pin.runtime.runtime_generation_pin = None;
    let mut visited = false;
    session
        .restore_session_snapshot_with_value_validator(missing_pin, |_| {
            visited = true;
            Ok(())
        })
        .expect_err("a Root that can accept future events retains its generation pin");
    assert!(!visited);
    assert_eq!(session.snapshot_session().unwrap(), before);
    let report = session.hot_swap_bundle(&second).unwrap();
    assert_eq!(report.compatibility, SwapCompatibility::ContentOnly);
    assert_eq!(report.generation, GenerationId::new(1));
    assert_eq!(
        session.current_fiber_generation(),
        Some(GenerationId::new(0))
    );
    assert!(session.has_runtime_image(GenerationId::new(0)));
    assert!(matches!(
        session.snapshot_session(),
        Err(BundleSessionSaveError::GenerationMismatch {
            field: "runtime_generation_pin",
            ..
        })
    ));
    let mut input = BundleStepInput::default();
    input.root_events.push(RootEventInput::new(event.into()));
    let mut committed = false;
    for _ in 0..128 {
        tick += 1;
        let step = session.step_with_clock(
            RuntimeClockStep::from_millis(tick, 1).unwrap(),
            std::mem::take(&mut input),
        );
        assert!(step.diagnostics.is_empty(), "{:?}", step.diagnostics);
        assert!(step.stats.executed_ops <= 1);
        if let [outcome] = &step.root_transitions[..] {
            assert!(matches!(outcome, RootTransitionOutcome::Committed { .. }));
            assert_eq!(outcome.sequence().get(), 0);
            committed = true;
            break;
        }
    }
    assert!(committed);
    assert!(session.program_owner().same_program(&original_owner));
    assert_eq!(
        session.current_fiber_generation(),
        Some(GenerationId::new(0))
    );
    assert!(session.has_runtime_image(GenerationId::new(0)));
}

fn record_root_replay_session(
    session: &mut arcweft_runtime_driver::session::BundleSession,
    event: RuntimeValue,
) -> arcweft_runtime_driver::session::RootReplayTraceV1 {
    use arcweft_core::root::RootEventInput;
    use arcweft_runtime_driver::{
        clock::RuntimeClockStep,
        session::{BundleStepInput, RootReplayRecordingError},
    };
    let mut recorder = session.start_root_replay_recording().unwrap();
    assert_eq!(
        recorder.clone().finish(),
        Err(RootReplayRecordingError::InitializationPending)
    );
    let mut input = BundleStepInput::default();
    input.root_events.push(RootEventInput::new(event.into()));
    let mut initialized = None;
    let mut transitioned = false;
    for step_number in 1..=128 {
        let step = session
            .step_with_clock_recording(
                RuntimeClockStep::from_millis(step_number, 1).unwrap(),
                std::mem::take(&mut input),
                &mut recorder,
            )
            .unwrap();
        assert!(step.diagnostics.is_empty(), "{:?}", step.diagnostics);
        assert!(step.stats.executed_ops <= 1);
        if let [outcome] = &step.root_initializations[..] {
            assert!(initialized.replace(outcome.state_digest).is_none());
            assert!(step.root_transitions.is_empty());
        }
        if !step.root_transitions.is_empty() {
            assert!(initialized.is_some());
            assert_eq!(step.root_transitions.len(), 1);
            transitioned = true;
            break;
        }
    }
    assert!(transitioned);
    let trace = recorder.finish().unwrap();
    assert_eq!(Some(trace.initializer_state_digest), initialized);
    assert_eq!(trace.transitions.len(), 1);
    trace
}

#[test]
fn root_replay_records_observed_initialization_before_queued_event_transition() {
    use arcweft_core::root::RootEventInput;
    use arcweft_runtime_driver::{
        clock::RuntimeClockStep,
        session::{
            BundleSession, BundleStepInput, RecordedRootOutcomeV1, RootReplayError,
            RootReplayRecordingError, RootReplayTraceV1,
        },
    };
    let (bundle, event, options) = root_replay_fixture("root-replay-initialization");
    let mut unrecorded = BundleSession::new(&bundle, options.clone()).unwrap();
    let mut early_input = BundleStepInput::default();
    early_input
        .root_events
        .push(RootEventInput::new(event.clone().into()));
    let early =
        unrecorded.step_with_clock(RuntimeClockStep::from_millis(1, 1).unwrap(), early_input);
    assert!(early.diagnostics.is_empty(), "{:?}", early.diagnostics);
    assert!(matches!(
        unrecorded.start_root_replay_recording(),
        Err(RootReplayRecordingError::AlreadyAdvanced)
    ));
    let mut session = BundleSession::new(&bundle, options.clone()).unwrap();
    let trace = record_root_replay_session(&mut session, event);
    let wire = serde_json::to_value(&trace).unwrap();
    assert_eq!(wire["schema_version"], 1);
    assert!(wire.get("identity").is_none());
    let decoded: RootReplayTraceV1 = serde_json::from_value(wire).unwrap();
    assert_eq!(decoded, trace);
    let replay = BundleSession::replay_root_trace(&bundle, options.clone(), &decoded).unwrap();
    assert_eq!(replay.transitions_verified, 1);
    assert!(!replay.terminal_trap);
    let mut drain_options = options.clone();
    drain_options.max_ops = 128;
    assert_eq!(
        BundleSession::replay_root_trace(&bundle, drain_options, &decoded).unwrap(),
        replay,
        "the same semantic transcript survives different frame preemption boundaries"
    );
    let max_items = bundle.product_awbc_program().entries[0]
        .roles
        .stateful()
        .unwrap()
        .command_policy
        .root_limits
        .schema
        .max_sequence_items;
    let mut oversized = decoded;
    let RecordedRootOutcomeV1::Committed {
        command_digests, ..
    } = &mut oversized.transitions[0].outcome
    else {
        panic!("the recorded reducer committed");
    };
    command_digests.resize(
        usize::try_from(max_items).unwrap() + 1,
        oversized.initializer_state_digest,
    );
    let Err(RootReplayError::Snapshot(error)) =
        BundleSession::replay_root_trace(&bundle, options, &oversized)
    else {
        panic!("oversized transcript metadata must fail retained admission");
    };
    assert_eq!(
        *error,
        arcweft_core::value::AwbcRuntimeValueSnapshotError::Resource(
            arcweft_core::entry::RuntimeSchemaError::BudgetExceeded {
                budget: "sequence_items"
            }
        )
    );
}

#[test]
fn root_replay_recording_rejection_returns_unexecuted_input_and_clock() {
    use arcweft_core::root::RootEventInput;
    use arcweft_runtime_driver::{
        clock::RuntimeClockStep,
        session::{BundleSession, BundleStepInput, RootReplayStepError},
    };
    let (bundle, _, options) = root_replay_fixture("root-replay-unexecuted-input");
    let mut session = BundleSession::new(&bundle, options).unwrap();
    let mut recorder = session.start_root_replay_recording().unwrap();
    let before = recorder.clone().finish();
    let clock = RuntimeClockStep::from_millis(1, 1).unwrap();
    let mut input = BundleStepInput::default();
    input
        .root_events
        .push(RootEventInput::new(RuntimeValue::Unit.into()));
    let failure = session
        .step_with_clock_recording(clock, input, &mut recorder)
        .unwrap_err();
    let RootReplayStepError::BeforeStep {
        clock: returned_clock,
        input,
        error,
    } = failure
    else {
        panic!("invalid input must be returned before execution");
    };
    assert_eq!(returned_clock, clock);
    assert_eq!(input.root_events.len(), 1);
    assert!(matches!(input.root_events[0].payload.0, RuntimeValue::Unit));
    assert!(matches!(
        error,
        arcweft_runtime_driver::session::RootReplayRecordingError::Snapshot(_)
    ));
    assert_eq!(recorder.finish(), before);
    assert_eq!(
        session.start_root_replay_recording().unwrap().finish(),
        before
    );
}

#[test]
fn root_replay_trace_codec_bounds_wire_and_rejects_unknown_duplicate_and_trailing_data() {
    use arcweft_data::{DataErrorKind, DecodeLimits};
    use arcweft_runtime_driver::session::{
        BundleSession, ROOT_REPLAY_SAVE_SCHEMA_ID, ROOT_REPLAY_SCHEMA_VERSION, RootReplayError,
    };
    use arcweft_save::{SaveDecodeOptions, SaveEnvelope, SaveSchemaId, TYPED_JSON_CODEC_ID};
    let (bundle, event, options) = root_replay_fixture("root-replay-trace-wire");
    let mut session = BundleSession::new(&bundle, options.clone()).unwrap();
    let trace = record_root_replay_session(&mut session, event);
    let envelope = |payload: Vec<u8>| {
        SaveEnvelope::new(
            SaveSchemaId::new(ROOT_REPLAY_SAVE_SCHEMA_ID),
            ROOT_REPLAY_SCHEMA_VERSION,
            TYPED_JSON_CODEC_ID,
            payload,
        )
        .encode_bytes()
        .unwrap()
    };
    let json = serde_json::to_vec(&trace).unwrap();
    let bytes = envelope(json.clone());
    let decode = SaveDecodeOptions::default();
    let replay =
        BundleSession::replay_root_trace_from_save_bytes(&bundle, options.clone(), &bytes, &decode)
            .unwrap();
    assert_eq!(replay.transitions_verified, 1);
    let assert_rejected = |packet: &[u8], policy: &SaveDecodeOptions, expected: DataErrorKind| {
        // A codec failure must precede even inspecting the invalid AWFB input.
        let failure = BundleSession::replay_root_trace_from_awfb_and_trace_bytes(
            &[],
            options.clone(),
            packet,
            policy,
        )
        .unwrap_err();
        assert!(std::error::Error::source(&failure).is_some());
        let RootReplayError::Codec(error) = failure else {
            panic!("trace decoding must reject first");
        };
        assert_eq!(error.kind(), &expected);
    };
    for limits in [
        DecodeLimits {
            max_input_len: json.len() - 1,
            ..decode.codec.limits
        },
        DecodeLimits {
            max_depth: 0,
            ..decode.codec.limits
        },
        DecodeLimits {
            max_nodes: 0,
            ..decode.codec.limits
        },
        DecodeLimits {
            max_sequence_len: 0,
            ..decode.codec.limits
        },
        DecodeLimits {
            max_map_len: 0,
            ..decode.codec.limits
        },
        DecodeLimits {
            max_collection_items: 0,
            ..decode.codec.limits
        },
        DecodeLimits {
            max_string_len: 0,
            ..decode.codec.limits
        },
    ] {
        let mut policy = decode.clone();
        policy.codec.limits = limits;
        assert_rejected(&bytes, &policy, DataErrorKind::LimitExceeded);
    }
    for policy in [
        SaveDecodeOptions {
            envelope: arcweft_save::SaveEnvelopeLimits {
                max_envelope_bytes: u64::try_from(bytes.len() - 1).unwrap(),
                ..decode.envelope
            },
            ..decode.clone()
        },
        SaveDecodeOptions {
            envelope: arcweft_save::SaveEnvelopeLimits {
                max_payload_bytes: u64::try_from(json.len() - 1).unwrap(),
                ..decode.envelope
            },
            ..decode.clone()
        },
    ] {
        assert_rejected(&bytes, &policy, DataErrorKind::LimitExceeded);
    }
    let mut unknown_root = serde_json::to_value(&trace).unwrap();
    unknown_root["unrecognized"] = serde_json::json!(true);
    let mut unknown_transition = serde_json::to_value(&trace).unwrap();
    unknown_transition["transitions"][0]["unrecognized"] = serde_json::json!(true);
    for unknown in [unknown_root, unknown_transition] {
        assert_rejected(
            &envelope(serde_json::to_vec(&unknown).unwrap()),
            &decode,
            DataErrorKind::InvalidEncoding,
        );
    }
    let mut duplicate = json[..json.len() - 1].to_vec();
    duplicate.extend_from_slice(br#", "schema_\u0076ersion": 1}"#);
    assert_rejected(&envelope(duplicate), &decode, DataErrorKind::DuplicateField);
    let mut trailing_json = json.clone();
    trailing_json.extend_from_slice(b" null");
    assert_rejected(
        &envelope(trailing_json),
        &decode,
        DataErrorKind::TrailingData,
    );
    let mut trailing_envelope = bytes.clone();
    trailing_envelope.push(0);
    assert_rejected(
        &trailing_envelope,
        &SaveDecodeOptions {
            allow_trailing_data: true,
            ..decode.clone()
        },
        DataErrorKind::TrailingData,
    );
    for version in [0, 2] {
        let packet = SaveEnvelope::new(
            SaveSchemaId::new(ROOT_REPLAY_SAVE_SCHEMA_ID),
            version,
            TYPED_JSON_CODEC_ID,
            json.clone(),
        )
        .encode_bytes()
        .unwrap();
        assert_rejected(&packet, &decode, DataErrorKind::InvalidEncoding);
    }
    for (schema, codec) in [
        ("arcweft.other", TYPED_JSON_CODEC_ID),
        (ROOT_REPLAY_SAVE_SCHEMA_ID, "other"),
    ] {
        let packet = SaveEnvelope::new(SaveSchemaId::new(schema), 1, codec, json.clone())
            .encode_bytes()
            .unwrap();
        assert_rejected(&packet, &decode, DataErrorKind::InvalidEncoding);
    }
    let mut corrupt = bytes;
    *corrupt.last_mut().unwrap() ^= 1;
    assert_rejected(&corrupt, &decode, DataErrorKind::InvalidEncoding);
}

#[test]
fn root_replay_trace_bytes_preserve_the_observed_awfb_artifact_identity() {
    use arcweft_bundle::BundleFormat;
    use arcweft_runtime_driver::session::{
        BundleSession, ROOT_REPLAY_SAVE_SCHEMA_ID, ROOT_REPLAY_SCHEMA_VERSION, RootReplayError,
    };
    use arcweft_save::{SaveDecodeOptions, SaveSchemaId, encode_typed_json_save};
    let (bundle, event, options) = root_replay_fixture("root-replay-awfb-trace-wire");
    let awfb = bundle.to_format_bytes(BundleFormat::Awfb).unwrap();
    let mut session = BundleSession::from_awfb_bytes(&awfb, options.clone()).unwrap();
    let trace = record_root_replay_session(&mut session, event);
    let bytes = encode_typed_json_save(
        &trace,
        SaveSchemaId::new(ROOT_REPLAY_SAVE_SCHEMA_ID),
        ROOT_REPLAY_SCHEMA_VERSION,
    )
    .unwrap();
    let decode = SaveDecodeOptions::default();
    let replay = BundleSession::replay_root_trace_from_awfb_and_trace_bytes(
        &awfb,
        options.clone(),
        &bytes,
        &decode,
    )
    .unwrap();
    assert_eq!(replay.transitions_verified, 1);
    assert!(matches!(
        BundleSession::replay_root_trace_from_save_bytes(&bundle, options, &bytes, &decode,),
        Err(RootReplayError::ArtifactMismatch)
    ));
}

#[test]
fn root_replay_recording_after_step_failure_returns_the_actual_transition() {
    use arcweft_core::root::RootEventInput;
    use arcweft_runtime_driver::{
        clock::RuntimeClockStep,
        session::{BundleSession, BundleStepInput, RootReplayRecordingError, RootReplayStepError},
    };
    let (bundle, event, options) = root_replay_fixture("root-replay-executed-output");
    let mut session = BundleSession::new(&bundle, options).unwrap();
    let mut recorder = session.start_root_replay_recording().unwrap();
    let mut input = BundleStepInput::default();
    input
        .root_events
        .push(RootEventInput::new(event.clone().into()));
    let mut advanced = false;
    for number in 1..=128 {
        let step = session.step_with_clock(
            RuntimeClockStep::from_millis(number, 1).unwrap(),
            std::mem::take(&mut input),
        );
        assert!(step.diagnostics.is_empty(), "{:?}", step.diagnostics);
        if let [outcome] = &step.root_transitions[..] {
            assert_eq!(outcome.sequence().get(), 0);
            advanced = true;
            break;
        }
    }
    assert!(
        advanced,
        "the ordinary session must commit the unrecorded event"
    );
    input.root_events.push(RootEventInput::new(event.into()));
    for number in 129..=256 {
        let clock = RuntimeClockStep::from_millis(number, 1).unwrap();
        let before = recorder.clone().finish();
        match session.step_with_clock_recording(clock, std::mem::take(&mut input), &mut recorder) {
            Ok(step) => {
                assert!(step.diagnostics.is_empty(), "{:?}", step.diagnostics);
                assert!(step.root_transitions.is_empty());
            }
            Err(RootReplayStepError::BeforeStep { error, .. }) => {
                panic!("the event must reach ordinary execution: {error}");
            }
            Err(RootReplayStepError::AfterStep { step, error }) => {
                assert_eq!(
                    error,
                    RootReplayRecordingError::SequenceMismatch {
                        expected: 0,
                        actual: 1
                    }
                );
                assert_eq!(step.clock, clock);
                assert!(step.diagnostics.is_empty(), "{:?}", step.diagnostics);
                assert_eq!(step.root_transitions.len(), 1);
                assert_eq!(step.root_transitions[0].sequence().get(), 1);
                assert_eq!(recorder.finish(), before);
                return;
            }
        }
    }
    panic!("an executed sequence mismatch must return its actual step output");
}

#[test]
fn view_expression_locale_is_execution_input_and_invalidates_only_changed_cache() {
    use arcweft_runtime_driver::{
        presentation_handles::{
            PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
        },
        view_runtime::BundleViewRuntime,
    };
    let compiled = compile_rust_project(
        &metadata(),
        "entry cli @entry.main { goto @flow.main }\nflow main() -> String { return \"done\" }\nview Main() { Text(default_string()) }\n",
        "view-locale-execution",
    ).unwrap();
    let lowered = compiled.runtime_plan();
    let code = AwbcLowerer::new(
        &lowered.plan,
        &lowered.dialogue_content_catalog,
        "main.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let code = arcweft_bundle::standard_view::install_dialogue_handler_awbc(code).unwrap();
    let code = Arc::new(
        AwbcProgram::decode_canonical(
            &code.encode_canonical().unwrap(),
            AwbcDecodeBudget::default(),
        )
        .unwrap(),
    );
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().cloned();
    let mut runtime =
        BundleViewRuntime::try_new_with_awbc(product.clone(), text.clone(), Arc::clone(&code))
            .unwrap();
    let handles = [PresentationHandleRecord::new(
        arcweft_id::presentation::PresentationHandleId::try_new("view.locale").unwrap(),
        PresentationHandleKind::View,
        "view.Main".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    )];
    let observations = std::rc::Rc::new(RefCell::new(ViewLocaleObservations::default()));
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(ViewLocaleProbe {
        owner: RuntimeProgramOwner::Awbc(Arc::clone(&code)),
        observations: std::rc::Rc::clone(&observations),
    });
    for (tag, calls) in [("en-US", 1), ("en-US", 1), ("de-DE", 2), ("de-DE", 2)] {
        let format = arcweft_core::value::RuntimeFormatContext::new(
            arcweft_id::LocaleTag::try_new(tag).unwrap(),
        );
        let frame = runtime.evaluate_with_dialogue_and_backend(
            &handles,
            &[],
            &[],
            false,
            &format,
            &mut backend,
        );
        assert_view_locale(&frame, tag);
        assert_eq!(observations.borrow().locales.len(), calls);
    }
    assert_eq!(
        observations
            .borrow()
            .locales
            .iter()
            .map(arcweft_id::LocaleTag::as_str)
            .collect::<Vec<_>>(),
        ["en-US", "de-DE"]
    );
    assert!(
        observations
            .borrow()
            .targets
            .iter()
            .all(|target| target == &observations.borrow().targets[0])
    );
    let saved = runtime.snapshot().unwrap();
    let mut restored = BundleViewRuntime::try_new_with_awbc(product, text, code).unwrap();
    restored.restore(&saved, &handles).unwrap();
    assert_eq!(restored.snapshot().unwrap(), saved);
    let format = arcweft_core::value::RuntimeFormatContext::new(
        arcweft_id::LocaleTag::try_new("fr-FR").unwrap(),
    );
    let frame = restored.evaluate_with_dialogue_and_backend(
        &handles,
        &[],
        &[],
        false,
        &format,
        &mut backend,
    );
    assert_view_locale(&frame, "fr-FR");
    assert_eq!(observations.borrow().locales.len(), 3);
}

impl RuntimeExternalCallBackend for RustDefaults {
    fn call_external(
        &mut self,
        context: &RuntimeExternalCallContext,
        target: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        let factory = self.factories.get(target.callable_identity()?)?;
        assert!(args.is_empty());
        assert_eq!(context.argument_types(), Some([].as_slice()));
        assert!(match (context.program_owner().unwrap(), &self.owner) {
            (RuntimeProgramOwner::Plan(left), RuntimeProgramOwner::Plan(right)) =>
                Arc::ptr_eq(&left, right),
            (RuntimeProgramOwner::Awbc(left), RuntimeProgramOwner::Awbc(right)) =>
                Arc::ptr_eq(&left, right),
            _ => false,
        });
        if self.wrong_result {
            return Some(Ok(RuntimeValue::String("tampered result".to_owned())));
        }
        let types = selected_types(&self.owner);
        Some(Ok(match factory {
            Factory::Flag => RuntimeValue::Bool(default_flag()),
            Factory::BoolDefault => RuntimeValue::Bool(default_bool()),
            Factory::StringDefault => RuntimeValue::String(default_string()),
            Factory::Marker => types
                .try_record_value(
                    context.result_type().unwrap(),
                    vec![RuntimeValue::Bool(default_marker().0)],
                    RuntimeSchemaLimits::engine_default(),
                )
                .unwrap(),
        }))
    }
}

fn selected_types(owner: &RuntimeProgramOwner) -> RuntimeProgramTypes<'_> {
    match owner {
        RuntimeProgramOwner::Plan(plan) => RuntimeProgramTypes::Plan(plan),
        RuntimeProgramOwner::Awbc(program) => RuntimeProgramTypes::Awbc(program),
    }
}

struct Defaults<'a> {
    owner: &'a RuntimeProgramOwner,
    shapes: RuntimeProgramDataShapes<'a>,
    backend: RefCell<VmRuntimePureCallBackend<RustDefaults>>,
    calls: RefCell<Vec<usize>>,
}

impl FieldDefaultProvider for Defaults<'_> {
    fn default_value(&self, field: FieldDefaultRequest<'_>) -> arcweft_data::Result<Value> {
        self.calls.borrow_mut().push(field.field_ordinal());
        let request = self
            .shapes
            .field_default_request(field.record_id().unwrap(), field.field_ordinal())
            .unwrap()
            .unwrap();
        let mut backend = self.backend.borrow_mut();
        let value = match self.owner {
            RuntimeProgramOwner::Plan(plan) => {
                arcweft_core::pure::evaluate_pure_program_with_backend(
                    plan,
                    request.program(),
                    &[],
                    &mut *backend,
                )
                .map_err(|error| error.to_string())
            }
            RuntimeProgramOwner::Awbc(program) => {
                arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
                    program,
                    request.program(),
                    &[],
                    &mut *backend,
                )
                .map_err(|error| error.to_string())
            }
        }
        .map_err(|message| {
            arcweft_data::DataError::new(arcweft_data::DataErrorKind::InvalidType, message)
        })?;
        request
            .validate_result(&value, RuntimeSchemaLimits::engine_default())
            .unwrap();
        // These two source factories have exact Bool and Marker(Bool) result
        // contracts. The generic accelerator owns the exhaustive conversion.
        Ok(match value {
            RuntimeValue::Bool(value) => Value::Bool(value),
            RuntimeValue::String(value) => Value::String(value),
            RuntimeValue::NominalRecord(record) => {
                let RuntimeValue::Bool(value) = record.fields()[0] else {
                    panic!("Marker's admitted Bool field")
                };
                Value::Bool(value)
            }
            _ => panic!("the runtime result was admitted against the exact field type"),
        })
    }
}

fn default_contract(
    plan: &RuntimePlan,
) -> (RuntimeSemanticTypeId, BTreeMap<RuntimeCallableId, Factory>) {
    let domain = plan
        .nominal_record_domains()
        .domains()
        .find(|domain| domain.fields().len() == 3)
        .unwrap();
    let semantic = plan
        .type_table()
        .get(domain.owner())
        .unwrap()
        .semantic_identity();
    let shapes = RuntimeProgramDataShapes::new(RuntimeProgramTypes::Plan(plan));
    let root = shapes
        .root(semantic, RuntimeSchemaLimits::engine_default())
        .unwrap()
        .referenced_id()
        .unwrap();
    let mut factories = BTreeMap::new();
    for (ordinal, factory) in [
        (0, Factory::Flag),
        (1, Factory::Marker),
        (2, Factory::Marker),
    ] {
        let request = shapes
            .field_default_request(root, ordinal)
            .unwrap()
            .unwrap();
        let binding = plan
            .pure_programs()
            .iter()
            .find(|binding| binding.program() == request.program())
            .unwrap();
        let RuntimeExprKind::Call {
            callee: target,
            args,
        } = plan
            .function_sites()
            .get(binding.site())
            .unwrap()
            .body()
            .expression()
            .unwrap()
            .kind()
        else {
            panic!("accepted Rust callable wrapper")
        };
        assert!(args.is_empty());
        assert!(matches!(target, RuntimeCallTarget::External(_)));
        factories.insert(target.callable_identity().unwrap().clone(), factory);
    }
    (semantic, factories)
}

#[test]
fn rust_declared_defaults_reach_plan_awbc_and_execute_only_on_missing_fields() {
    let (plan, awbc) = compile_defaults(&metadata()).unwrap();
    let (semantic, factories) = default_contract(&plan);
    assert_eq!(
        plan.pure_programs().len(),
        2,
        "one binding per admitted producer"
    );
    let source_value = settings();
    assert!(source_value.flag && source_value.marker.0 && source_value.cache.0);
    for owner in [
        RuntimeProgramOwner::Plan(plan),
        RuntimeProgramOwner::Awbc(awbc),
    ] {
        let shapes = RuntimeProgramDataShapes::new(selected_types(&owner));
        let root = shapes
            .root(semantic, RuntimeSchemaLimits::engine_default())
            .unwrap();
        let provider = Defaults {
            owner: &owner,
            shapes: RuntimeProgramDataShapes::new(selected_types(&owner)),
            backend: RefCell::new(VmRuntimePureCallBackend::default().with_external_calls(
                RustDefaults {
                    owner: owner.clone(),
                    factories: factories.clone(),
                    wrong_result: false,
                },
            )),
            calls: RefCell::new(vec![]),
        };
        let absent = RawValue::Map(vec![]);
        assert!(decode_with_shape_ref(&absent, root, &shapes).is_err());
        let access = DecodeShapeAccess::new(&shapes, &provider);
        let value = decode_with_shape_ref(&absent, root, &access).unwrap();
        assert_eq!(*provider.calls.borrow(), [0, 1, 2]);
        assert!(
            value
                .as_record()
                .unwrap()
                .values()
                .all(|value| value == &Value::Bool(true))
        );
        provider.calls.borrow_mut().clear();
        let present = RawValue::Map(vec![
            (RawValue::String("flag".to_owned()), RawValue::Bool(false)),
            (RawValue::String("marker".to_owned()), RawValue::Bool(false)),
            (RawValue::String("cache".to_owned()), RawValue::Bool(false)),
        ]);
        let value = decode_with_shape_ref(&present, root, &access).unwrap();
        assert_eq!(
            *provider.calls.borrow(),
            [2],
            "skip ignores input and executes its declared producer"
        );
        assert_eq!(value.as_record().unwrap()["flag"], Value::Bool(false));
        let invalid = Defaults {
            owner: &owner,
            shapes: RuntimeProgramDataShapes::new(selected_types(&owner)),
            backend: RefCell::new(VmRuntimePureCallBackend::default().with_external_calls(
                RustDefaults {
                    owner: owner.clone(),
                    factories: factories.clone(),
                    wrong_result: true,
                },
            )),
            calls: RefCell::new(vec![]),
        };
        assert!(
            decode_with_shape_ref(&absent, root, &DecodeShapeAccess::new(&shapes, &invalid))
                .is_err(),
            "tampered factory results cannot publish a typed field"
        );
    }
}

#[test]
fn rust_default_producer_tampering_fails_before_runtime_publication() {
    for mutation in 0..6 {
        let mut rust = metadata();
        let factory = rust
            .functions
            .iter_mut()
            .find(|function| function.name == "default_flag")
            .unwrap();
        match mutation {
            0 => {
                rust.functions
                    .retain(|function| function.name != "default_flag");
            }
            1 => factory.purity = ArcweftRustPurity::External,
            2 => factory.return_type = ArcweftRustTypeRef::String,
            3 => factory.params.push(arcweft_rust_abi::ArcweftRustParam {
                name: "unexpected".to_owned(),
                ty: ArcweftRustTypeRef::Bool,
            }),
            4 => {
                let mut duplicate = factory.clone();
                duplicate.name = "other_flag".to_owned();
                rust.functions.push(duplicate);
            }
            5 => factory.effects.push("io.read".to_owned()),
            _ => unreachable!(),
        }
        assert!(
            compile_defaults(&rust).is_err(),
            "mutation {mutation} cannot issue a default program"
        );
    }
}

const PRIMITIVE_SOURCE: &str = "flow main { let a = primitive_defaults(); let b = generic_default(); }\nentry cli @entry.cli.main { goto @flow.main }\n";

#[test]
fn rust_defaults_for_primitive_foreign_and_generic_fields_share_checked_bindings() {
    let (plan, awbc) = compile_rust(&metadata(), PRIMITIVE_SOURCE).unwrap();
    assert_eq!(plan.pure_programs().len(), 2);
    assert!(!primitive_defaults().flag && primitive_defaults().text.is_empty());
    assert!(!generic_default().value);
    let mut records = Vec::new();
    let mut factories = BTreeMap::new();
    let shapes = RuntimeProgramDataShapes::new(RuntimeProgramTypes::Plan(&plan));
    for domain in plan.nominal_record_domains().domains() {
        let Some(codec) = domain.data_codec() else {
            continue;
        };
        let arcweft_core::entry::RuntimeCodecUse::Record { name, .. } = &codec.body else {
            continue;
        };
        if !matches!(name.as_str(), "PrimitiveDefaults" | "GenericDefault") {
            continue;
        }
        let semantic = plan
            .type_table()
            .get(domain.owner())
            .unwrap()
            .semantic_identity();
        let root = shapes
            .root(semantic, RuntimeSchemaLimits::engine_default())
            .unwrap()
            .referenced_id()
            .unwrap();
        for ordinal in 0..domain.fields().len() {
            let request = shapes
                .field_default_request(root, ordinal)
                .unwrap()
                .unwrap();
            let binding = plan
                .pure_programs()
                .iter()
                .find(|binding| binding.program() == request.program())
                .unwrap();
            let RuntimeExprKind::Call { callee: target, .. } = plan
                .function_sites()
                .get(binding.site())
                .unwrap()
                .body()
                .expression()
                .unwrap()
                .kind()
            else {
                panic!("declared Rust wrapper")
            };
            factories.insert(
                target.callable_identity().unwrap().clone(),
                if name == "PrimitiveDefaults" && ordinal == 1 {
                    Factory::StringDefault
                } else {
                    Factory::BoolDefault
                },
            );
        }
        records.push((name.clone(), semantic));
    }
    assert_eq!(records.len(), 2);
    for owner in [
        RuntimeProgramOwner::Plan(plan),
        RuntimeProgramOwner::Awbc(awbc),
    ] {
        let shapes = RuntimeProgramDataShapes::new(selected_types(&owner));
        let provider = Defaults {
            owner: &owner,
            shapes: RuntimeProgramDataShapes::new(selected_types(&owner)),
            backend: RefCell::new(VmRuntimePureCallBackend::default().with_external_calls(
                RustDefaults {
                    owner: owner.clone(),
                    factories: factories.clone(),
                    wrong_result: false,
                },
            )),
            calls: RefCell::new(vec![]),
        };
        for (name, semantic) in &records {
            let value = decode_with_shape_ref(
                &RawValue::Map(vec![]),
                shapes
                    .root(*semantic, RuntimeSchemaLimits::engine_default())
                    .unwrap(),
                &DecodeShapeAccess::new(&shapes, &provider),
            )
            .unwrap();
            let expected = if name == "PrimitiveDefaults" {
                BTreeMap::from([
                    ("flag".to_owned(), Value::Bool(false)),
                    ("text".to_owned(), Value::String(String::new())),
                ])
            } else {
                BTreeMap::from([("value".to_owned(), Value::Bool(false))])
            };
            assert_eq!(value, Value::Record(expected));
        }
    }
}

#[test]
fn rust_trait_default_bindings_reject_missing_wrong_and_duplicate_producers() {
    for mutation in 0..3 {
        let mut rust = metadata();
        let factory = rust
            .functions
            .iter_mut()
            .find(|function| function.name == "default_bool")
            .unwrap();
        match mutation {
            0 => rust
                .functions
                .retain(|function| function.name != "default_bool"),
            1 => factory.return_type = ArcweftRustTypeRef::String,
            2 => {
                let mut duplicate = factory.clone();
                duplicate.name = "other_default_bool".to_owned();
                rust.functions.push(duplicate);
            }
            _ => unreachable!(),
        }
        assert!(
            compile_rust(&rust, PRIMITIVE_SOURCE).is_err(),
            "trait witness mutation {mutation}"
        );
    }
}
