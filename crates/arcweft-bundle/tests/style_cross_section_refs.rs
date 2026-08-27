use arcweft_bundle::{
    ArcweftBundle, BundleCodecError, BundleFormat, BundleManifest, BundleRuntimeSummary,
    container::{
        BundleDigest, BundleSectionKind, BundleView, ReadBudget, SectionId, SectionInput,
        encode_bundle,
    },
    resource_codec::{
        CrossSectionRef, ProductResourceEnvelope, ProductSectionCodecKind, PublicIdRef,
        SectionCodecBudget, SourceMapSection, ViewStyleResource,
    },
};
use arcweft_core::{
    awbc::schema::{
        AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
        AwbcFlowBinding, AwbcFlowExecutable, AwbcFrameLayout, AwbcFrameLayoutId, AwbcFunction,
        AwbcFunctionFlag, AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind, AwbcProgram,
        AwbcSafePointKind, AwbcSignature, AwbcSignatureId, AwbcStringId, AwbcTableRange,
        AwbcTerminator,
    },
    effect::RuntimeArtifactFingerprint,
    entry::{FlowContractHash, RuntimeFlowExecutable},
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_text_model::DialogueContentCatalog;

type ReferenceSelector = for<'a> fn(&'a mut ViewStyleResource) -> &'a mut CrossSectionRef;
type ReferenceMutation = fn(&mut CrossSectionRef);

#[test]
fn style_cross_section_references_round_trip_from_every_supported_location() {
    let bundle = referenced_bundle();
    let expected_style = bundle.view_style.clone();

    let bytes = bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("valid Style cross-section references encode");
    let decoded = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &bytes)
        .expect("valid Style cross-section references decode");

    assert_eq!(decoded.view_style, expected_style);
}

#[test]
fn style_encode_rejects_missing_target_from_every_supported_location() {
    for (label, select) in reference_locations() {
        let mut bundle = referenced_bundle();
        missing_target(select(style_mut(&mut bundle)));

        let error = bundle
            .to_format_bytes(BundleFormat::Awfb)
            .expect_err("missing Style target must fail encode");
        assert_awfb_error(error, true, label, "targets missing section");
    }
}

#[test]
fn style_decode_rejects_missing_target_from_every_supported_location() {
    let bytes = referenced_bundle()
        .to_format_bytes(BundleFormat::Awfb)
        .expect("valid Style references encode");

    for (label, select) in reference_locations() {
        let tampered = tamper_style_reference(&bytes, select, missing_target);
        let error = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &tampered)
            .expect_err("missing Style target must fail decode");
        assert_awfb_error(error, false, label, "targets missing section");
    }
}

#[test]
fn style_encode_rejects_target_identity_and_public_id_mismatches() {
    for (label, mutate, expected) in mismatch_cases() {
        let mut bundle = referenced_bundle();
        mutate(adapter_reference(style_mut(&mut bundle)));

        let error = bundle
            .to_format_bytes(BundleFormat::Awfb)
            .expect_err("mismatched Style target must fail encode");
        assert_awfb_error(error, true, label, expected);
    }
}

#[test]
fn style_decode_rejects_target_identity_and_public_id_mismatches() {
    let bytes = referenced_bundle()
        .to_format_bytes(BundleFormat::Awfb)
        .expect("valid Style references encode");

    for (label, mutate, expected) in mismatch_cases() {
        let tampered = tamper_style_reference(&bytes, adapter_reference, mutate);
        let error = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &tampered)
            .expect_err("mismatched Style target must fail decode");
        assert_awfb_error(error, false, label, expected);
    }
}

#[test]
fn style_encode_rejects_public_id_on_target_without_public_id_table() {
    let mut bundle = referenced_bundle();
    let bytes = bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("valid Style references encode");
    let unsupported = section_reference_from_awfb(
        &bytes,
        BundleSectionKind::ProgramBytecode,
        Some(PublicIdRef(0)),
    );
    *adapter_reference(style_mut(&mut bundle)) = unsupported;

    let error = bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect_err("public ID on non-compact target must fail encode");
    assert_awfb_error(error, true, "unsupported target", "has no public-ID table");
}

#[test]
fn style_decode_rejects_public_id_on_target_without_public_id_table() {
    let bytes = referenced_bundle()
        .to_format_bytes(BundleFormat::Awfb)
        .expect("valid Style references encode");
    let unsupported = section_reference_from_awfb(
        &bytes,
        BundleSectionKind::ProgramBytecode,
        Some(PublicIdRef(0)),
    );
    let tampered = tamper_style_reference(&bytes, adapter_reference, |reference| {
        *reference = unsupported;
    });

    let error = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &tampered)
        .expect_err("public ID on non-compact target must fail decode");
    assert_awfb_error(error, false, "unsupported target", "has no public-ID table");
}

fn reference_locations() -> [(&'static str, ReferenceSelector); 1] {
    [("adapter requirement", adapter_reference)]
}

fn mismatch_cases() -> [(&'static str, ReferenceMutation, &'static str); 3] {
    [
        ("section kind", mismatched_kind, "expects kind"),
        ("content digest", mismatched_digest, "expects kind"),
        (
            "public ID",
            out_of_bounds_public_id,
            "out-of-bounds public ID",
        ),
    ]
}

fn referenced_bundle() -> ArcweftBundle {
    let mut bundle = minimal_bundle();
    let reference = valid_view_text_reference(&bundle);
    style_mut(&mut bundle).adapter_requirements.push(reference);
    bundle
}

fn valid_view_text_reference(bundle: &ArcweftBundle) -> CrossSectionRef {
    let bytes = bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("base bundle encodes");
    let view = BundleView::parse(&bytes, ReadBudget::default()).expect("base AWFB parses");
    let descriptor = view
        .sections()
        .iter()
        .find(|descriptor| descriptor.known_kind() == Some(BundleSectionKind::ViewText))
        .expect("standard bundle has ViewText");
    let payload = view
        .decoded_section(descriptor.id())
        .expect("ViewText payload decodes")
        .expect("ViewText payload is embedded");
    let envelope = ProductResourceEnvelope::decode_all_fields(
        &payload,
        ProductSectionCodecKind::ViewText,
        SectionCodecBudget::default(),
    )
    .expect("ViewText compact envelope decodes");
    assert!(!envelope.public_ids.is_empty());

    section_reference_from_awfb(&bytes, BundleSectionKind::ViewText, Some(PublicIdRef(0)))
}

fn section_reference_from_awfb(
    bytes: &[u8],
    kind: BundleSectionKind,
    public_id: Option<PublicIdRef>,
) -> CrossSectionRef {
    let view = BundleView::parse(bytes, ReadBudget::default()).expect("valid AWFB parses");
    let descriptor = view
        .sections()
        .iter()
        .find(|descriptor| descriptor.known_kind() == Some(kind))
        .expect("target section exists");
    CrossSectionRef {
        section_kind: descriptor.kind_code(),
        section_id: descriptor.id(),
        content_digest: descriptor.content_digest(),
        public_id,
    }
}

fn tamper_style_reference(
    bytes: &[u8],
    select: ReferenceSelector,
    mutate: impl Fn(&mut CrossSectionRef),
) -> Vec<u8> {
    let view = BundleView::parse(bytes, ReadBudget::default()).expect("valid AWFB parses");
    let sections = view
        .sections()
        .iter()
        .map(|descriptor| {
            let mut payload = view
                .decoded_section(descriptor.id())
                .expect("section payload decodes")
                .expect("test AWFB uses embedded sections");
            if descriptor.known_kind() == Some(BundleSectionKind::ViewStyle) {
                let mut style = ViewStyleResource::decode_canonical_section(&payload)
                    .expect("ViewStyle payload decodes");
                mutate(select(&mut style));
                payload = style
                    .encode_canonical_section()
                    .expect("locally valid tampered ViewStyle re-encodes");
            }
            SectionInput::embedded(
                descriptor.id(),
                descriptor.kind(),
                descriptor.schema_version(),
                descriptor.residency(),
                descriptor.required(),
                payload,
            )
        })
        .collect::<Vec<_>>();
    encode_bundle(view.kind(), view.manifest(), sections).expect("tampered AWFB re-encodes")
}

fn style_mut(bundle: &mut ArcweftBundle) -> &mut ViewStyleResource {
    bundle
        .view_style
        .as_mut()
        .expect("standard bundle has ViewStyle")
}

fn adapter_reference(style: &mut ViewStyleResource) -> &mut CrossSectionRef {
    style
        .adapter_requirements
        .first_mut()
        .expect("adapter reference exists")
}

fn missing_target(reference: &mut CrossSectionRef) {
    reference.section_id = SectionId::from_bytes([0xa5; 16]);
}

fn mismatched_kind(reference: &mut CrossSectionRef) {
    reference.section_kind = BundleSectionKind::ViewProgram.into();
}

fn mismatched_digest(reference: &mut CrossSectionRef) {
    reference.content_digest = BundleDigest::of(b"tampered Style target");
}

fn out_of_bounds_public_id(reference: &mut CrossSectionRef) {
    reference.public_id = Some(PublicIdRef(u32::MAX));
}

fn assert_awfb_error(error: BundleCodecError, encode: bool, label: &str, expected_message: &str) {
    let message = match (encode, error) {
        (true, BundleCodecError::EncodeAwfb { message })
        | (false, BundleCodecError::DecodeAwfb { message }) => message,
        (_, other) => panic!("{label}: unexpected AWFB error: {other:?}"),
    };
    assert!(
        message.contains(expected_message),
        "{label}: `{message}` does not contain `{expected_message}`"
    );
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
        source_map("style-cross-section.arcw", ""),
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
    let flow =
        arcweft_core::plan::FlowRuntimeId::from_checked_declaration_digest([0xa2; 32], "flow.main")
            .expect("test checked Flow identity");
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
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        }],
        flow_bindings: vec![AwbcFlowBinding {
            flow: flow.clone(),
            function: AwbcFunctionId(0),
        }],
        flow_executables: vec![AwbcFlowExecutable {
            metadata: RuntimeFlowExecutable {
                flow,
                contract: FlowContractHash::from_bytes([0xb2; 32]),
                controller: None,
            },
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
            target: AwbcEntryTarget::Function {
                function: AwbcFunctionId(0),
            },
            roles: arcweft_core::entry::RuntimeEntryRoles::None,
        }],
        ..AwbcProgram::default()
    }
}
