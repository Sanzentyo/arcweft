use arcweft_bundle::container::{BundleView, ReadBudget};
use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_bundle::resource_codec::SourceMapSection;
use arcweft_bundle::{ArcweftBundle, BundleFormat, BundleManifest, BundleRuntimeSummary};
use arcweft_core::{
    awbc::{
        fiber::{FiberScope, FiberScopeCleanup},
        schema::{
            AwbcBlock, AwbcBlockId, AwbcConstant, AwbcConstantId, AwbcEffectKind, AwbcEffectPlan,
            AwbcEffectPlanId, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
            AwbcFrameLayout, AwbcFrameLayoutId, AwbcFrameSlot, AwbcFrameSlotRole, AwbcFunction,
            AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind, AwbcProgram, AwbcRuntimeType,
            AwbcSafePointKind, AwbcScopeId, AwbcSignature, AwbcSignatureId, AwbcStringId,
            AwbcTableRange, AwbcTerminator, AwbcTypeId,
        },
    },
    bytecode::BytecodeProgram,
    entry::{EntryBindingIdentity, RuntimeEntryRoles},
    plan::EntryRuntimeId,
    value::{RuntimeBinding, RuntimeExpr, RuntimeFunctionValue, RuntimeValue},
};
use arcweft_interaction_model::input::{
    InputEpoch, InputEventKind, InputSequence, InteractionTarget, RoutedInputEvent,
};
use arcweft_presentation::fx::{
    FiniteF32, FxAbiHash, FxDefinition, FxDiagnosticCode, FxGraph, FxGraphChildPath, FxId,
    FxInstanceId, FxParameter, FxRuntimeType, FxRuntimeValue,
};
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_driver::{
    clock::RuntimeClockStep,
    presentation_handles::{
        PresentationHandleId, PresentationHandleKind, PresentationHandleRecord,
        PresentationResourceState,
    },
    session::{BundleSession, BundleSessionOptions, BundleStepInput},
    session_save::{
        BUNDLE_SESSION_SAVE_SCHEMA_ID, BUNDLE_SESSION_SAVE_SCHEMA_VERSION,
        BundleSessionArtifactIdentity, BundleSessionPendingBlocker, BundleSessionSaveError,
        BundleSessionSnapshot,
    },
    swap::GenerationId,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

#[test]
fn awbc_product_bundle_session_from_awfb_requires_and_uses_product_awbc() {
    let bytes = product_awfb_bytes("entry.main");
    let mut session = product_session_from_bytes(&bytes);
    let step = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert!(
        step.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        step.diagnostics
    );
}

#[test]
fn awbc_product_bundle_session_save_bytes_round_trip_restore() {
    let bytes = product_awfb_bytes("entry.main");
    let mut session = product_session_from_bytes(&bytes);
    session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );

    let identity = BundleView::parse(&bytes, ReadBudget::default())
        .expect("source AWFB parses")
        .artifact_identity();
    assert_eq!(
        session
            .snapshot_session()
            .expect("session snapshot exports")
            .generation
            .artifact,
        BundleSessionArtifactIdentity::AwfbContainer { identity }
    );

    let save = session
        .export_session_save_bytes()
        .expect("session save exports");
    let mut restored = BundleSession::from_awfb_bytes(&bytes, BundleSessionOptions::default())
        .expect("AWBC product session restarts");
    restored
        .import_session_save_bytes(&save, &arcweft_save::SaveDecodeOptions::default())
        .expect("session save imports");

    let restored_save = restored
        .export_session_save_bytes()
        .expect("restored session save exports");
    assert_eq!(save, restored_save);
}

#[test]
fn fx_instances_and_logical_time_restore_atomically_with_the_session() {
    let definition = fx_definition();
    let bundle = product_bundle_with_label("entry.main", "fx-save.arcw")
        .with_fx_definitions(FxDefinitions::try_new([definition.clone()]).expect("Fx inventory"));
    let bytes = bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("AWFB encodes");
    let mut session = product_session_from_bytes(&bytes);
    session.step_with_clock(
        RuntimeClockStep::from_millis(1, 250).expect("clock"),
        BundleStepInput::default(),
    );
    let instance = FxInstanceId::derive(definition.id(), ["view.hud", "node.1", "fx.0"]);
    session
        .retain_fx_instance(
            definition.id(),
            instance,
            vec![FxRuntimeValue::F32(
                FiniteF32::try_new(1.5).expect("finite"),
            )],
            FxGraphChildPath::try_new(vec![2, 4]).expect("child path"),
            Some(b"authored-seed"),
        )
        .expect("Fx activates");
    session.step_with_clock(
        RuntimeClockStep::from_millis(2, 750).expect("clock"),
        BundleStepInput::default(),
    );
    let expected = session.snapshot_session().expect("snapshot exports");
    assert_eq!(
        expected.presentation.fx.logical_time.seconds().value(),
        FiniteF32::ONE
    );
    assert_eq!(expected.presentation.fx.instances.len(), 1);

    let save = session
        .export_session_save_bytes()
        .expect("Fx session save exports");
    let mut restored = product_session_from_bytes(&bytes);
    restored
        .import_session_save_bytes(&save, &arcweft_save::SaveDecodeOptions::default())
        .expect("Fx session save imports");
    assert_eq!(restored.fx_runtime(), &expected.presentation.fx);

    let before_rejection = restored
        .snapshot_session()
        .expect("restored snapshot exports");
    let mut invalid = before_rejection.clone();
    invalid.runtime.source_label = "must not leak".to_owned();
    invalid.presentation.fx.instances[0].abi_hash = FxAbiHash::derive(["wrong"]);
    let error = restored
        .restore_session_snapshot(invalid)
        .expect_err("ABI mismatch is rejected");
    assert!(matches!(
        error,
        BundleSessionSaveError::Fx { diagnostic }
            if diagnostic.code == FxDiagnosticCode::AbiMismatch
    ));
    assert_eq!(
        restored
            .snapshot_session()
            .expect("rejected restore leaves state unchanged"),
        before_rejection
    );
}

#[test]
fn rejected_session_restore_does_not_partially_mutate_the_live_session() {
    let bytes = product_awfb_bytes("entry.main");
    let mut session = product_session_from_bytes(&bytes);
    session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    let before = session.snapshot_session().expect("live snapshot exports");
    let mut invalid = before.clone();
    invalid.runtime.source_label = "invalid restore must not leak".to_owned();
    invalid.runtime.next_step_index = invalid.runtime.next_step_index.saturating_add(99);
    invalid.runtime.runtime_generation_pin = Some(GenerationId(
        invalid.generation.active_generation.0.saturating_add(1),
    ));

    let error = session
        .restore_session_snapshot(invalid)
        .expect_err("mismatched generation pin is rejected");
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
            .expect("live session remains valid"),
        before
    );
}

#[test]
fn presentation_handle_save_load_restores_lifecycle_table() {
    let bytes = product_awfb_bytes("entry.main");
    let mut session = product_session_from_bytes(&bytes);
    session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    let mut snapshot = session.snapshot_session().expect("snapshot exports");
    snapshot.presentation.revision = 7;
    snapshot.presentation.presentation_handle_epoch = 2;
    snapshot.presentation.presentation_handles = vec![view_handle(
        "handle.flow.feedback.panel",
        PresentationResourceState::Hidden,
        1,
        2,
    )];

    session
        .restore_session_snapshot(snapshot.clone())
        .expect("snapshot restores");
    assert_eq!(session.presentation(), &snapshot.presentation);

    let save = session
        .export_session_save_bytes()
        .expect("session save exports presentation handles");
    let mut restored = product_session_from_bytes(&bytes);
    restored
        .import_session_save_bytes(&save, &arcweft_save::SaveDecodeOptions::default())
        .expect("session save imports presentation handles");

    assert_eq!(restored.presentation(), &snapshot.presentation);
}

#[test]
fn presentation_handle_rollback_restores_tombstones() {
    let bytes = product_awfb_bytes("entry.main");
    let mut session = product_session_from_bytes(&bytes);
    let mut live = session.snapshot_session().expect("live snapshot exports");
    live.presentation.presentation_handle_epoch = 1;
    live.presentation.presentation_handles = vec![view_handle(
        "handle.flow.feedback.panel",
        PresentationResourceState::Mounted,
        1,
        1,
    )];
    let mut released = live.clone();
    released.presentation.presentation_handle_epoch = 2;
    released.presentation.presentation_handles[0] = view_handle(
        "handle.flow.feedback.panel",
        PresentationResourceState::Released,
        1,
        2,
    );

    session
        .restore_session_snapshot(live)
        .expect("live snapshot restores");
    assert_eq!(
        session.presentation().presentation_handles[0].state,
        PresentationResourceState::Mounted
    );

    session
        .restore_session_snapshot(released.clone())
        .expect("released snapshot restores");
    assert_eq!(
        session.presentation().presentation_handles[0].state,
        PresentationResourceState::Released
    );
    assert_eq!(
        session.presentation().presentation_handles[0].updated_epoch,
        2
    );

    let save = session
        .export_session_save_bytes()
        .expect("released handle save exports");
    let mut restored = product_session_from_bytes(&bytes);
    restored
        .import_session_save_bytes(&save, &arcweft_save::SaveDecodeOptions::default())
        .expect("released handle save imports");
    assert_eq!(restored.presentation(), &released.presentation);
}

#[test]
fn awbc_save_load_preserves_cleanup_stacks() {
    let bytes = product_awfb_bytes("entry.main");
    let mut session = product_session_from_bytes(&bytes);
    let mut snapshot = session.snapshot_session().expect("snapshot exports");
    let state = &mut snapshot.executor.state;
    let frame = state.fiber.active_frame_mut().expect("active frame");
    frame
        .root_cleanups
        .push(cleanup("handle.root", "root cleanup"));
    frame.scopes.push(FiberScope {
        id: AwbcScopeId(0),
        depth: 1,
        cleanups: vec![cleanup("handle.scope", "scope cleanup")],
    });

    session
        .restore_session_snapshot(snapshot.clone())
        .expect("snapshot with cleanup stacks restores");
    let save = session
        .export_session_save_bytes()
        .expect("cleanup stack save exports");
    let mut restored = product_session_from_bytes(&bytes);
    restored
        .import_session_save_bytes(&save, &arcweft_save::SaveDecodeOptions::default())
        .expect("cleanup stack save imports");

    assert_eq!(
        restored
            .snapshot_session()
            .expect("restored snapshot")
            .executor,
        snapshot.executor
    );
}

#[test]
fn session_save_rejects_structured_function_bodies() {
    let bytes = product_awfb_bytes("entry.main");
    let mut session = product_session_from_bytes(&bytes);
    let mut snapshot = session.snapshot_session().expect("snapshot exports");
    let state = &mut snapshot.executor.state;
    let frame = state.fiber.active_frame_mut().expect("active frame");
    frame.root_cleanups.push(FiberScopeCleanup {
        key: "handle.function".to_owned(),
        effect: AwbcEffectPlanId(0),
        args: vec![structured_runtime_function_value()],
    });

    let error = session
        .restore_session_snapshot(snapshot.clone())
        .expect_err("structured function bodies are not valid Product AWBC state");
    assert!(matches!(
        error,
        BundleSessionSaveError::InvalidRuntimeValue { path, message }
            if path == "executor.product_awbc.fiber.frames[0].root_cleanups[0].args[0]"
                && message.contains("structured expression function bodies")
    ));

    let save = encode_session_snapshot(&snapshot, BUNDLE_SESSION_SAVE_SCHEMA_VERSION);
    let mut restored = product_session_from_bytes(&bytes);
    let error = restored
        .import_session_save_bytes(&save, &arcweft_save::SaveDecodeOptions::default())
        .expect_err("encoded structured functions are rejected on import");
    assert!(matches!(
        error,
        BundleSessionSaveError::InvalidRuntimeValue { path, message }
            if path == "executor.product_awbc.fiber.frames[0].root_cleanups[0].args[0]"
                && message.contains("structured expression function bodies")
    ));
}

#[test]
fn session_save_round_trips_awbc_function_values() {
    let bytes = product_awfb_bytes("entry.main");
    let mut session = product_session_from_bytes(&bytes);
    let mut snapshot = session.snapshot_session().expect("snapshot exports");
    snapshot
        .executor
        .state
        .fiber
        .active_frame_mut()
        .expect("active frame")
        .root_cleanups
        .push(FiberScopeCleanup {
            key: "handle.function".to_owned(),
            effect: AwbcEffectPlanId(0),
            args: vec![captured_awbc_runtime_function_value()],
        });

    session
        .restore_session_snapshot(snapshot.clone())
        .expect("AWBC-backed function state restores");
    let encoded = session
        .export_session_save_bytes()
        .expect("AWBC-backed function state encodes");
    let mut restored = product_session_from_bytes(&bytes);
    restored
        .import_session_save_bytes(&encoded, &arcweft_save::SaveDecodeOptions::default())
        .expect("AWBC-backed function state imports");

    assert_eq!(
        restored
            .snapshot_session()
            .expect("restored snapshot exports")
            .executor,
        snapshot.executor
    );
}

#[test]
fn session_save_rejects_stale_awbc_function_ids() {
    let bytes = product_awfb_bytes("entry.main");
    let mut session = product_session_from_bytes(&bytes);
    let mut snapshot = session.snapshot_session().expect("snapshot exports");
    snapshot
        .executor
        .state
        .fiber
        .active_frame_mut()
        .expect("active frame")
        .root_cleanups
        .push(FiberScopeCleanup {
            key: "handle.function".to_owned(),
            effect: AwbcEffectPlanId(0),
            args: vec![awbc_runtime_function_value(AwbcFunctionId(u32::MAX))],
        });

    let error = session
        .restore_session_snapshot(snapshot)
        .expect_err("stale function table ids reject");
    assert!(matches!(
        error,
        BundleSessionSaveError::InvalidRuntimeValue { path, message }
            if path == "executor.product_awbc.fiber.frames[0].root_cleanups[0].args[0]"
                && message.contains("does not exist")
    ));
}

#[test]
fn session_restore_rebuilds_facade_fiber() {
    let bytes = product_awfb_bytes("entry.main");
    let mut session = product_session_from_bytes(&bytes);
    session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert!(session.is_finished());
    let save = session
        .export_session_save_bytes()
        .expect("finished session save exports");

    let mut restored = product_session_from_bytes(&bytes);
    assert!(!restored.is_finished());
    restored
        .import_session_save_bytes(&save, &arcweft_save::SaveDecodeOptions::default())
        .expect("finished session save imports");

    assert!(restored.is_finished());
}

#[test]
fn save_decode_rejects_mismatched_bundle_generation() {
    let source_bytes = product_awfb_bytes("entry.main");
    let target_bytes = product_awfb_bytes("entry.other");
    let session = product_session_from_bytes(&source_bytes);
    let save = session
        .export_session_save_bytes()
        .expect("source save exports");
    let mut target = product_session_from_bytes(&target_bytes);

    let error = target
        .import_session_save_bytes(&save, &arcweft_save::SaveDecodeOptions::default())
        .expect_err("generation mismatch rejects restore");

    assert!(matches!(
        error,
        BundleSessionSaveError::GenerationMismatch { .. }
    ));
}

#[test]
fn save_decode_rejects_same_root_from_a_different_bundle_manifest() {
    let source_bytes = product_awfb_bytes_with_profile("entry.main", "profile.source");
    let target_bytes = product_awfb_bytes_with_profile("entry.main", "profile.target");
    let source_view =
        BundleView::parse(&source_bytes, ReadBudget::default()).expect("source AWFB parses");
    let target_view =
        BundleView::parse(&target_bytes, ReadBudget::default()).expect("target AWFB parses");
    assert_eq!(source_view.content_root(), target_view.content_root());
    assert_ne!(
        source_view.artifact_identity(),
        target_view.artifact_identity()
    );
    let session = product_session_from_bytes(&source_bytes);
    let save = session
        .export_session_save_bytes()
        .expect("source save exports");
    let mut target = product_session_from_bytes(&target_bytes);

    let error = target
        .import_session_save_bytes(&save, &arcweft_save::SaveDecodeOptions::default())
        .expect_err("manifest identity mismatch rejects restore");

    assert!(matches!(
        error,
        BundleSessionSaveError::GenerationMismatch {
            field: "artifact",
            ..
        }
    ));
}

#[test]
fn logical_session_save_rejects_a_different_bundle_manifest() {
    let source_bundle = product_bundle_with_label("entry.main", "source-session.arcw");
    let target_bundle = product_bundle_with_label("entry.main", "target-session.arcw");
    let source = BundleSession::new(&source_bundle, BundleSessionOptions::default())
        .expect("logical source session starts");
    let save = source
        .export_session_save_bytes()
        .expect("logical source save exports");
    let mut target = BundleSession::new(&target_bundle, BundleSessionOptions::default())
        .expect("logical target session starts");

    let error = target
        .import_session_save_bytes(&save, &arcweft_save::SaveDecodeOptions::default())
        .expect_err("logical manifest identity mismatch rejects restore");

    assert!(matches!(
        error,
        BundleSessionSaveError::GenerationMismatch {
            field: "artifact",
            ..
        }
    ));
}

#[test]
fn save_envelope_strict_decode_rejects_future_or_trailing_payloads() {
    let bytes = product_awfb_bytes("entry.main");
    let session = product_session_from_bytes(&bytes);
    let snapshot = session.snapshot_session().expect("snapshot exports");
    let future = encode_session_snapshot(&snapshot, BUNDLE_SESSION_SAVE_SCHEMA_VERSION + 1);
    let mut target = product_session_from_bytes(&bytes);

    let future_error = target
        .import_session_save_bytes(&future, &arcweft_save::SaveDecodeOptions::default())
        .expect_err("future session save version rejects");

    assert!(matches!(
        future_error,
        BundleSessionSaveError::Decode { message } if message.contains("newer than supported")
    ));

    let mut trailing = session
        .export_session_save_bytes()
        .expect("session save exports");
    trailing.push(0);
    let trailing_error = target
        .import_session_save_bytes(&trailing, &arcweft_save::SaveDecodeOptions::default())
        .expect_err("trailing session save data rejects");

    assert!(matches!(
        trailing_error,
        BundleSessionSaveError::Decode { message } if message.contains("trailing data")
    ));
}

#[test]
fn save_007_predecessor_v1_missing_runtime_generation_pin_is_rejected() {
    let bytes = product_awfb_bytes("entry.main");
    let session = product_session_from_bytes(&bytes);
    let snapshot = session.snapshot_session().expect("snapshot exports");
    let mut predecessor = serde_json::to_value(snapshot).expect("snapshot becomes JSON");
    predecessor
        .get_mut("runtime")
        .and_then(serde_json::Value::as_object_mut)
        .expect("runtime object")
        .remove("runtime_generation_pin");
    let encoded = encode_session_json_value(&predecessor);
    let mut target = product_session_from_bytes(&bytes);
    let before = target.snapshot_session().expect("live snapshot exports");

    let error = target
        .import_session_save_bytes(&encoded, &arcweft_save::SaveDecodeOptions::default())
        .expect_err("missing required generation pin rejects");

    assert!(matches!(
        error,
        BundleSessionSaveError::Decode { message }
            if message.contains("missing field `runtime_generation_pin`")
    ));
    assert_eq!(
        target
            .snapshot_session()
            .expect("rejected decode leaves session valid"),
        before
    );
}

#[test]
fn save_007_unknown_nested_session_field_is_rejected() {
    let bytes = product_awfb_bytes("entry.main");
    let session = product_session_from_bytes(&bytes);
    let snapshot = session.snapshot_session().expect("snapshot exports");
    let mut future = serde_json::to_value(snapshot).expect("snapshot becomes JSON");
    future
        .get_mut("runtime")
        .and_then(serde_json::Value::as_object_mut)
        .expect("runtime object")
        .insert(
            "predecessor_extension".to_owned(),
            serde_json::Value::Bool(true),
        );
    let encoded = encode_session_json_value(&future);
    let mut target = product_session_from_bytes(&bytes);
    let before = target.snapshot_session().expect("live snapshot exports");

    let error = target
        .import_session_save_bytes(&encoded, &arcweft_save::SaveDecodeOptions::default())
        .expect_err("unknown nested field rejects");

    assert!(matches!(
        error,
        BundleSessionSaveError::Decode { message }
            if message.contains("runtime.predecessor_extension")
    ));
    assert_eq!(
        target
            .snapshot_session()
            .expect("rejected decode leaves session valid"),
        before
    );
}

#[test]
fn session_save_rejects_pending_input_events() {
    let bytes = product_awfb_bytes("entry.main");
    let mut session = product_session_from_bytes(&bytes);
    session.queue_input(RoutedInputEvent::new(
        InputEpoch::new(1),
        InputSequence::new(1),
        InteractionTarget::new("runtime").expect("runtime target"),
        InputEventKind::FocusGained,
    ));

    let error = session
        .export_session_save_bytes()
        .expect_err("pending input blocks save");

    assert!(matches!(
        error,
        BundleSessionSaveError::NonQuiescent {
            blockers
        } if blockers == vec![BundleSessionPendingBlocker::PendingInputEvents { count: 1 }]
    ));
}

fn product_session_from_bytes(bytes: &[u8]) -> BundleSession {
    BundleSession::from_awfb_bytes(bytes, BundleSessionOptions::default())
        .expect("AWBC product session starts")
}

fn encode_session_snapshot(snapshot: &BundleSessionSnapshot, schema_version: u32) -> Vec<u8> {
    arcweft_save::encode_typed_json_save(
        snapshot,
        arcweft_save::SaveSchemaId::new(BUNDLE_SESSION_SAVE_SCHEMA_ID),
        schema_version,
    )
    .expect("session snapshot encodes")
}

fn encode_session_json_value(value: &serde_json::Value) -> Vec<u8> {
    arcweft_save::SaveEnvelope::new(
        arcweft_save::SaveSchemaId::new(BUNDLE_SESSION_SAVE_SCHEMA_ID),
        BUNDLE_SESSION_SAVE_SCHEMA_VERSION,
        arcweft_save::TYPED_JSON_CODEC_ID,
        serde_json::to_vec(value).expect("JSON payload encodes"),
    )
    .encode_bytes()
    .expect("save envelope encodes")
}

fn product_awfb_bytes(entry: &str) -> Vec<u8> {
    product_awfb_bytes_with_label(entry, "awbc-session.arcw")
}

fn product_awfb_bytes_with_label(entry: &str, source_label: &str) -> Vec<u8> {
    product_bundle_with_label(entry, source_label)
        .to_format_bytes(BundleFormat::Awfb)
        .expect("awfb encodes")
}

fn product_awfb_bytes_with_profile(entry: &str, profile_id: &str) -> Vec<u8> {
    let mut bundle = product_bundle_with_label(entry, "awbc-session.arcw");
    bundle.manifest.profile_id = Some(profile_id.to_owned());
    bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("awfb encodes")
}

fn product_bundle_with_label(entry: &str, source_label: &str) -> ArcweftBundle {
    ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: Some(entry.to_owned()),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                entry_flow: None,
                flows: 0,
                bytecode_instructions: 0,
                line_task_groups: 0,
                stream_plans: 0,
                source_plans: 0,
            },
        },
        source_map(source_label, ""),
        BytecodeProgram::default(),
        LineDisplayCatalog::default(),
    )
    .expect("standard dialogue source joins source map")
    .with_product_awbc(minimal_awbc_program(entry))
}

fn source_map(label: &str, text: &str) -> SourceMapSection {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new(label).expect("source ID"),
        SourceName::path(label),
        text,
    )
    .expect("source document");
    SourceMapSection::try_from_documents(&[&document]).expect("source map")
}

fn fx_definition() -> FxDefinition {
    FxDefinition::new(
        FxId::try_new("test", "pulse").expect("Fx identity"),
        vec![FxParameter::try_new("speed", FxRuntimeType::F32, None).expect("Fx parameter")],
        FxGraph::default(),
    )
    .expect("Fx definition")
}

fn view_handle(
    id: &str,
    state: PresentationResourceState,
    created_epoch: u64,
    updated_epoch: u64,
) -> PresentationHandleRecord {
    PresentationHandleRecord::new(
        PresentationHandleId::try_new(id).expect("valid handle id"),
        PresentationHandleKind::View,
        "view.FeedbackPanel".to_owned(),
        Some("flow.feedback".to_owned()),
        state,
        Some("layer.view".to_owned()),
        1_500,
    )
    .with_epochs(created_epoch, updated_epoch)
}

fn cleanup(key: &str, value: &str) -> FiberScopeCleanup {
    FiberScopeCleanup {
        key: key.to_owned(),
        effect: AwbcEffectPlanId(0),
        args: vec![RuntimeValue::String(value.to_owned())],
    }
}

fn structured_runtime_function_value() -> RuntimeValue {
    RuntimeValue::Function(RuntimeFunctionValue::new(
        vec!["value".to_owned()],
        RuntimeExpr::Local("value".to_owned()),
        Vec::new(),
    ))
}

fn awbc_runtime_function_value(function: AwbcFunctionId) -> RuntimeValue {
    RuntimeValue::Function(RuntimeFunctionValue::new_awbc(
        Vec::new(),
        function,
        Vec::new(),
    ))
}

fn captured_awbc_runtime_function_value() -> RuntimeValue {
    RuntimeValue::Function(RuntimeFunctionValue::new_awbc(
        Vec::new(),
        AwbcFunctionId(1),
        vec![RuntimeBinding {
            name: "captured".to_owned(),
            value: RuntimeValue::String("saved value".to_owned()),
        }],
    ))
}

fn minimal_awbc_program(entry: &str) -> AwbcProgram {
    AwbcProgram {
        strings: vec![
            "captured".to_owned(),
            entry.to_owned(),
            "zz.debug".to_owned(),
            "zz.message".to_owned(),
        ],
        constants: vec![
            AwbcConstant::String(AwbcStringId(2)),
            AwbcConstant::String(AwbcStringId(3)),
        ],
        runtime_types: vec![AwbcRuntimeType::String, AwbcRuntimeType::Dynamic],
        signatures: vec![
            AwbcSignature {
                params: Vec::new(),
                result: None,
                effects: AwbcEffectSetId(0),
            },
            AwbcSignature {
                params: vec![AwbcTypeId(0)],
                result: Some(AwbcTypeId(0)),
                effects: AwbcEffectSetId(0),
            },
            AwbcSignature {
                params: vec![AwbcTypeId(1)],
                result: None,
                effects: AwbcEffectSetId(0),
            },
        ],
        frame_layouts: vec![
            AwbcFrameLayout {
                slots: Vec::new(),
                max_scope_depth: 1,
            },
            AwbcFrameLayout {
                slots: vec![AwbcFrameSlot {
                    name: Some(AwbcStringId(0)),
                    ty: AwbcTypeId(0),
                    role: AwbcFrameSlotRole::Parameter,
                    scope_depth: 0,
                }],
                max_scope_depth: 0,
            },
        ],
        functions: vec![
            AwbcFunction {
                public_id: Some(AwbcStringId(1)),
                kind: AwbcFunctionKind::Flow,
                signature: AwbcSignatureId(0),
                frame_layout: AwbcFrameLayoutId(0),
                blocks: AwbcTableRange::new(0, 1),
                entry_block: AwbcBlockId(0),
                flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
            },
            AwbcFunction {
                public_id: None,
                kind: AwbcFunctionKind::Synthetic,
                signature: AwbcSignatureId(1),
                frame_layout: AwbcFrameLayoutId(1),
                blocks: AwbcTableRange::new(1, 1),
                entry_block: AwbcBlockId(1),
                flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
            },
        ],
        blocks: vec![
            AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Return { value: None },
                safe_point: AwbcSafePointKind::FlowEntry,
                source_map: None,
            },
            AwbcBlock {
                owner: AwbcFunctionId(1),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Return {
                    value: Some(arcweft_core::awbc::schema::AwbcRegisterId(0)),
                },
                safe_point: AwbcSafePointKind::CallableBoundary,
                source_map: None,
            },
        ],
        entries: vec![minimal_awbc_entry(entry)],
        effect_plans: vec![AwbcEffectPlan {
            kind: AwbcEffectKind::Log,
            signature: AwbcSignatureId(2),
            capability: None,
            audio: None,
            static_args: vec![AwbcConstantId(0), AwbcConstantId(1)],
            resources: Vec::new(),
        }],
        ..AwbcProgram::default()
    }
}

fn minimal_awbc_entry(entry: &str) -> AwbcEntry {
    AwbcEntry {
        runtime_id: EntryRuntimeId::from_source_entity_body(entry).expect("test entry ID is valid"),
        binding: EntryBindingIdentity::from_bytes([1; 32]),
        public_id: AwbcStringId(1),
        kind: AwbcEntryKind::Cli,
        signature: AwbcSignatureId(0),
        target: AwbcEntryTarget::Function(AwbcFunctionId(0)),
        roles: RuntimeEntryRoles::None,
    }
}
