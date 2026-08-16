use arcweft_bundle::resource_codec::SourceMapSection;
use arcweft_bundle::{
    ArcweftBundle, BundleAwbcProgram, BundleCodecError, BundleFormat, BundleManifest,
    BundleRuntimeSummary,
};
use arcweft_core::awbc::schema::{
    AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
    AwbcFlowBinding, AwbcFrameLayout, AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags,
    AwbcFunctionId, AwbcFunctionKind, AwbcProgram, AwbcSafePointKind, AwbcSignature,
    AwbcSignatureId, AwbcStringId, AwbcTableRange, AwbcTerminator,
};
use arcweft_core::effect::RuntimeArtifactFingerprint;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_text_model::DialogueContentCatalog;

#[test]
fn product_awbc_embeds_awbc_only_executable_section() {
    let bundle = minimal_bundle();
    let bytes = bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("AWBC-only AWFB encodes");
    let decoded = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &bytes)
        .expect("AWBC-only AWFB decodes");
    assert_eq!(
        decoded.product_awbc().encoding,
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
    ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: None,
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                artifact_fingerprint: RuntimeArtifactFingerprint::try_from_bytes([0x6a; 32])
                    .expect("non-zero runtime artifact fingerprint"),
                entry_flow: None,
                flows: 0,
                bytecode_instructions: 0,
                line_task_groups: 0,
                stream_plans: 0,
            },
        },
        source_map("awbc-only.arcw", ""),
        minimal_awbc_program(),
        DialogueContentCatalog::new(),
    )
    .expect("standard dialogue source joins source map")
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
        flow_bindings: vec![AwbcFlowBinding {
            flow: arcweft_core::plan::FlowRuntimeId::from_checked_declaration_digest(
                [0xa1; 32],
                "flow.main",
            )
            .expect("test checked Flow identity"),
            function: AwbcFunctionId(0),
        }],
        blocks: vec![AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange::new(0, 0),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::FlowEntry,
            source_map: None,
        }],
        entries: vec![AwbcEntry {
            runtime_id: arcweft_core::plan::EntryRuntimeId::from_source_entity_body("entry.main")
                .expect("test entry ID is valid"),
            binding: arcweft_core::entry::EntryBindingIdentity::from_bytes([1; 32]),
            public_id: AwbcStringId(0),
            kind: AwbcEntryKind::Cli,
            signature: AwbcSignatureId(0),
            target: AwbcEntryTarget::Function(AwbcFunctionId(0)),
            roles: arcweft_core::entry::RuntimeEntryRoles::None,
        }],
        ..AwbcProgram::default()
    }
}
