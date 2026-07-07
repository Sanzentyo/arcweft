use arcweft_bundle::{
    ArcweftBundle, BundleFormat, BundleManifest, BundleRuntimeSummary, BundleSource,
};
use arcweft_core::{
    awbc::{
        fiber::{FiberScope, FiberScopeCleanup},
        schema::{
            AwbcBlock, AwbcBlockId, AwbcEffectKind, AwbcEffectPlan, AwbcEffectPlanId,
            AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget, AwbcFrameLayout,
            AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind,
            AwbcProgram, AwbcSafePointKind, AwbcScopeId, AwbcSignature, AwbcSignatureId,
            AwbcStringId, AwbcTableRange, AwbcTerminator,
        },
    },
    bytecode::BytecodeProgram,
    value::{RuntimeExpr, RuntimeFunctionValue, RuntimeValue},
};
use arcweft_interaction_model::input::{
    InputEpoch, InputEventKind, InputSequence, InteractionTarget, RoutedInputEvent,
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
        BundleSessionExecutorSnapshot, BundleSessionPendingBlocker, BundleSessionSaveError,
        BundleSessionSnapshot,
    },
};

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
    let BundleSessionExecutorSnapshot::ProductAwbc { state, .. } = &mut snapshot.executor else {
        panic!("test bundle uses Product AWBC")
    };
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
fn session_save_rejects_runtime_function_values() {
    let bytes = product_awfb_bytes("entry.main");
    let mut session = product_session_from_bytes(&bytes);
    let mut snapshot = session.snapshot_session().expect("snapshot exports");
    let BundleSessionExecutorSnapshot::ProductAwbc { state, .. } = &mut snapshot.executor else {
        panic!("test bundle uses Product AWBC")
    };
    let frame = state.fiber.active_frame_mut().expect("active frame");
    frame.root_cleanups.push(FiberScopeCleanup {
        key: "handle.function".to_owned(),
        effect: AwbcEffectPlanId(0),
        args: vec![runtime_function_value()],
    });

    let error = session
        .restore_session_snapshot(snapshot.clone())
        .expect_err("function values are not valid session-save payloads");
    assert!(matches!(
        error,
        BundleSessionSaveError::UnsupportedRuntimeValue { path, kind }
            if kind == "function"
                && path == "executor.product_awbc.fiber.frames[0].root_cleanups[0].args[0]"
    ));

    let save = encode_session_snapshot(&snapshot, BUNDLE_SESSION_SAVE_SCHEMA_VERSION);
    let mut restored = product_session_from_bytes(&bytes);
    let error = restored
        .import_session_save_bytes(&save, &arcweft_save::SaveDecodeOptions::default())
        .expect_err("encoded function values are rejected on import");
    assert!(matches!(
        error,
        BundleSessionSaveError::UnsupportedRuntimeValue { path, kind }
            if kind == "function"
                && path == "executor.product_awbc.fiber.frames[0].root_cleanups[0].args[0]"
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
fn save_decode_rejects_unsupported_executor_tier() {
    let bytes = product_awfb_bytes("entry.main");
    let session = product_session_from_bytes(&bytes);
    let mut snapshot = session.snapshot_session().expect("snapshot exports");
    snapshot.executor = BundleSessionExecutorSnapshot::StructuredVm;
    let save = encode_session_snapshot(&snapshot, BUNDLE_SESSION_SAVE_SCHEMA_VERSION);
    let mut target = product_session_from_bytes(&bytes);

    let error = target
        .import_session_save_bytes(&save, &arcweft_save::SaveDecodeOptions::default())
        .expect_err("Structured VM save restore is unsupported");

    assert!(matches!(
        error,
        BundleSessionSaveError::UnsupportedExecutorTier { tier } if tier == "structured_vm"
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

fn product_awfb_bytes(entry: &str) -> Vec<u8> {
    ArcweftBundle::new(
        BundleManifest {
            source_label: "awbc-session.arcw".to_owned(),
            profile_id: None,
            profile_kind: None,
            entry: None,
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
        BundleSource {
            label: "awbc-session.arcw".to_owned(),
            text: String::new(),
        },
        BytecodeProgram::default(),
        LineDisplayCatalog::default(),
    )
    .with_product_awbc(minimal_awbc_program(entry))
    .to_format_bytes(BundleFormat::Awfb)
    .expect("awfb encodes")
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
        Some("layer.ui".to_owned()),
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

fn runtime_function_value() -> RuntimeValue {
    RuntimeValue::Function(RuntimeFunctionValue::new(
        vec!["value".to_owned()],
        RuntimeExpr::Local("value".to_owned()),
        Vec::new(),
    ))
}

fn minimal_awbc_program(entry: &str) -> AwbcProgram {
    AwbcProgram {
        strings: vec![entry.to_owned()],
        signatures: vec![AwbcSignature {
            params: Vec::new(),
            result: None,
            effects: AwbcEffectSetId(0),
        }],
        frame_layouts: vec![AwbcFrameLayout {
            slots: Vec::new(),
            max_scope_depth: 1,
        }],
        functions: vec![AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
        }],
        blocks: vec![AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange::new(0, 0),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::FlowEntry,
            source_map: None,
        }],
        entries: vec![AwbcEntry {
            public_id: AwbcStringId(0),
            kind: AwbcEntryKind::Game,
            signature: AwbcSignatureId(0),
            target: AwbcEntryTarget::Function(AwbcFunctionId(0)),
        }],
        effect_plans: vec![AwbcEffectPlan {
            kind: AwbcEffectKind::DropHandle,
            signature: AwbcSignatureId(0),
            capability: None,
            audio: None,
            static_args: Vec::new(),
            resources: Vec::new(),
        }],
        ..AwbcProgram::default()
    }
}
