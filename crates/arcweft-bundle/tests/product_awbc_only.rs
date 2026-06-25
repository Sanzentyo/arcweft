use arcweft_bundle::{
    ArcweftBundle, BundleAwbcProgram, BundleCodecError, BundleFormat, BundleManifest,
    BundleRuntimeSummary, BundleSource,
    container::{BundleSectionKind, BundleView, ReadBudget, SectionInput, encode_bundle},
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

#[test]
fn product_awbc_decode_rejects_old_structured_product_bytecode_tag() {
    let bytes = minimal_bundle()
        .with_product_awbc(minimal_awbc_program())
        .to_format_bytes(BundleFormat::Awfb)
        .expect("AWBC-only AWFB encodes");
    let view = BundleView::parse(&bytes, ReadBudget::default()).expect("AWFB parses");
    let mut old_payload = Vec::new();
    old_payload.extend_from_slice(b"AWBC\r\n\x1a\n");
    old_payload.extend_from_slice(&1_u32.to_le_bytes());
    old_payload.extend_from_slice(&2_u32.to_le_bytes());
    old_payload.extend_from_slice(&0_u32.to_le_bytes());
    old_payload.extend_from_slice(&0_u32.to_le_bytes());
    let sections = view
        .sections()
        .iter()
        .map(|descriptor| {
            let bytes = if descriptor.kind() == BundleSectionKind::ProgramBytecode {
                old_payload.clone()
            } else {
                view.decoded_section(descriptor.id())
                    .expect("section decodes")
                    .expect("test AWFB uses embedded sections")
            };
            SectionInput::embedded(
                descriptor.id(),
                descriptor.kind(),
                descriptor.schema_version(),
                descriptor.residency(),
                descriptor.required(),
                bytes,
            )
        })
        .collect::<Vec<_>>();
    let old_structured =
        encode_bundle(view.kind(), view.manifest(), sections).expect("AWFB re-encodes");

    let error = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &old_structured)
        .expect_err("old structured product bytecode is rejected");

    assert!(matches!(
        error,
        BundleCodecError::StructuredProductBytecodeUnsupported { encoding_tag: 2 }
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
