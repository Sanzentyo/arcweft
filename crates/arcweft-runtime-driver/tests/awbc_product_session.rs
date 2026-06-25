use arcweft_bundle::{
    ArcweftBundle, BundleFormat, BundleManifest, BundleRuntimeSummary, BundleSource,
};
use arcweft_core::{
    awbc::schema::{
        AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
        AwbcFrameLayout, AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId,
        AwbcFunctionKind, AwbcProgram, AwbcSafePointKind, AwbcSignature, AwbcSignatureId,
        AwbcStringId, AwbcTableRange, AwbcTerminator,
    },
    bytecode::BytecodeProgram,
};
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_driver::{
    clock::RuntimeClockStep,
    session::{BundleSession, BundleSessionOptions, BundleStepInput},
};

#[test]
fn awbc_product_bundle_session_from_awfb_requires_and_uses_product_awbc() {
    let bundle = ArcweftBundle::new(
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
    .with_product_awbc(minimal_awbc_program());
    let bytes = bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("awfb encodes");
    let mut session = BundleSession::from_awfb_bytes(&bytes, BundleSessionOptions::default())
        .expect("AWBC product session starts");
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

fn minimal_awbc_program() -> AwbcProgram {
    AwbcProgram {
        strings: vec!["entry.main".to_owned()],
        signatures: vec![AwbcSignature {
            params: Vec::new(),
            result: None,
            effects: AwbcEffectSetId(0),
        }],
        frame_layouts: vec![AwbcFrameLayout {
            slots: Vec::new(),
            max_scope_depth: 0,
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
        ..AwbcProgram::default()
    }
}
