use arcweft_bundle::{
    ArcweftBundle, BundleAwbcProgram, BundleCodecError, BundleFormat, BundleManifest,
    BundleRuntimeSummary, BundleSource,
};
use arcweft_core::awbc::schema::{
    AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
    AwbcFrameLayout, AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId,
    AwbcFunctionKind, AwbcProgram, AwbcSafePointKind, AwbcSignature, AwbcSignatureId, AwbcStringId,
    AwbcTableRange, AwbcTerminator,
};
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_render_text::LineDisplayCatalog;

#[test]
fn product_awbc_requires_executable() {
    let bundle = minimal_bundle();
    let error = bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect_err("product encode must require AWBC");
    assert!(matches!(
        error,
        BundleCodecError::MissingProductAwbcExecutable
    ));
}

#[test]
fn product_awbc_embeds_awbc_only_executable_section() {
    let bundle = minimal_bundle().with_product_awbc(minimal_awbc_program());
    let bytes = bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("AWBC-only AWFB encodes");
    let decoded = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &bytes)
        .expect("AWBC-only AWFB decodes");
    assert_eq!(
        decoded
            .product_awbc()
            .expect("product awbc exists")
            .encoding,
        arcweft_bundle::BundleAwbcEncoding::AwbcV1
    );
}

#[test]
fn product_awbc_malformed_reports_typed_diagnostic() {
    let error = BundleAwbcProgram::decode_product_section(b"not-awbc")
        .expect_err("malformed AWBC must be rejected");
    assert!(matches!(
        error,
        BundleCodecError::MalformedProductAwbcExecutable { .. }
    ));
}

fn minimal_bundle() -> ArcweftBundle {
    ArcweftBundle::new(
        BundleManifest {
            source_label: "awbc-only.arcw".to_owned(),
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
            label: "awbc-only.arcw".to_owned(),
            text: String::new(),
        },
        BytecodeProgram::default(),
        LineDisplayCatalog::default(),
    )
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
