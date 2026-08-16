use arcweft_bundle::container::{
    BundleDigest, BundleKind as ContainerBundleKind, BundleSectionKind, BundleView,
    ContentResidency, ReadBudget, SectionId, SectionInput, encode_bundle,
};
use arcweft_bundle::patch::{BundlePatchArtifact, PatchCompatibility};
use arcweft_bundle::resource_codec::runtime::{
    AdapterRequirementsSection, EntrypointDeclaration, EntrypointsSection,
    FunctionInterfaceFingerprint, InitialStateRequirement, ProductVisibility, RuntimeFunctionKind,
    RuntimeResourceBudget, RuntimeResourceCompatibility, RuntimeTypeDeclaration,
    RuntimeTypesSection, RuntimeValueKind, TypeCompatibilityLabel,
    migrated_runtime_section_compatibility,
};
use arcweft_bundle::resource_codec::{
    FieldId, ResourceField, ResourceWireType, SectionCodecBudget, SectionCodecError,
};
use arcweft_core::awbc::schema::{AwbcDigest, AwbcProgram};

#[test]
fn runtime_types_compact_bytes_are_deterministic_and_round_trip() {
    let left = runtime_types_section();
    let right = RuntimeTypesSection::new(
        left.abi_version,
        left.runtime_layout_digest,
        left.declarations.iter().cloned().rev(),
        left.function_interfaces.iter().cloned().rev(),
    );

    let left_bytes = left
        .encode_canonical_section()
        .expect("runtime types encode");
    let right_bytes = right
        .encode_canonical_section()
        .expect("runtime types encode");

    assert_eq!(left_bytes, right_bytes);
    assert_eq!(
        RuntimeTypesSection::decode_canonical_section(&left_bytes).expect("runtime types decode"),
        left
    );
}

#[test]
fn runtime_types_must_match_the_awbc_header_exactly() {
    let program = AwbcProgram::default();
    let section = RuntimeTypesSection::new(
        program.header.abi_version,
        program.header.runtime_layout_digest,
        [],
        [],
    );
    assert!(section.validate_awbc(&program).is_ok());

    let abi_mismatch = RuntimeTypesSection::new(
        program.header.abi_version + 1,
        program.header.runtime_layout_digest,
        [],
        [],
    );
    assert_eq!(
        abi_mismatch.validate_awbc(&program),
        Err(SectionCodecError::RuntimeLayoutMismatch)
    );

    let digest_mismatch =
        RuntimeTypesSection::new(program.header.abi_version, AwbcDigest([0xa9; 32]), [], []);
    assert_eq!(
        digest_mismatch.validate_awbc(&program),
        Err(SectionCodecError::RuntimeLayoutMismatch)
    );
}

#[test]
fn entrypoints_compact_bytes_are_deterministic_and_round_trip() {
    let section = EntrypointsSection::new([
        EntrypointDeclaration {
            public_id: "entry.zeta".to_owned(),
            exported_name: Some("zeta".to_owned()),
            awbc_function_index: Some(2),
            initial_state: InitialStateRequirement::None,
            source_anchor: None,
            visibility: ProductVisibility::Public,
        },
        EntrypointDeclaration {
            public_id: "entry.alpha".to_owned(),
            exported_name: Some("alpha".to_owned()),
            awbc_function_index: Some(1),
            initial_state: InitialStateRequirement::RootBindings,
            source_anchor: None,
            visibility: ProductVisibility::Hidden,
        },
    ]);

    let bytes = section
        .encode_canonical_section()
        .expect("entrypoints encode");
    let decoded = EntrypointsSection::decode_canonical_section(&bytes).expect("entrypoints decode");

    assert_eq!(decoded.entries[0].public_id, "entry.alpha");
    assert_eq!(decoded, section);
}

#[test]
fn adapter_requirements_compact_bytes_round_trip_without_json_fallback() {
    let section = AdapterRequirementsSection::new(
        Some("native-file".to_owned()),
        ["native-file".to_owned()],
        ["fs.read_text".to_owned()],
        [arcweft_bundle::BundleAdapterManifest {
            id: "native-file".to_owned(),
            display_name: "Native File".to_owned(),
            effects: vec!["fs.read".to_owned()],
            host_calls: vec![arcweft_bundle::BundleAdapterHostCall {
                id: "fs.read_text".to_owned(),
                effects: vec!["fs.read".to_owned()],
            }],
        }],
    );

    let bytes = section
        .encode_canonical_section()
        .expect("adapter requirements encode");
    let decoded = AdapterRequirementsSection::decode_canonical_section(&bytes)
        .expect("adapter requirements decode");

    assert_eq!(decoded.default_adapter.as_deref(), Some("native-file"));
    assert_eq!(decoded.adapter_manifest_ids, vec!["native-file".to_owned()]);
    assert_eq!(decoded.required_host_calls, vec!["fs.read_text".to_owned()]);
    assert_eq!(decoded.adapter_manifests, section.adapter_manifests);
}

#[test]
fn runtime_resource_budget_failures_are_reported_by_family() {
    let bytes = runtime_types_section()
        .encode_canonical_section()
        .expect("runtime types encode");

    let error = RuntimeTypesSection::decode_canonical_section_with_budget(
        &bytes,
        RuntimeResourceBudget {
            runtime_types: 0,
            ..RuntimeResourceBudget::default()
        },
    )
    .expect_err("runtime type budget rejects");

    assert_eq!(error, SectionCodecError::BudgetExceeded("runtime_types"));
}

#[test]
fn unknown_optional_fields_are_skipped_and_unknown_required_fields_reject() {
    let section = runtime_types_section();
    let envelope = section.envelope_for_test().expect("test envelope builds");
    let mut fields = envelope.fields.clone();
    fields.push(ResourceField::optional(
        FieldId(60_000),
        ResourceWireType::Bytes,
        b"future optional".to_vec(),
    ));
    let record_count = envelope.header.record_count;
    let envelope = arcweft_bundle::resource_codec::ProductResourceEnvelope::new(
        arcweft_bundle::resource_codec::ProductSectionCodecKind::RuntimeTypes,
        envelope.strings,
        envelope.public_ids,
        envelope.enums,
        fields,
        record_count,
    )
    .expect("optional envelope rebuilds");
    let bytes = envelope.encode_canonical().expect("envelope encodes");
    RuntimeTypesSection::decode_canonical_section(&bytes).expect("unknown optional skips");

    let envelope = section.envelope_for_test().expect("test envelope builds");
    let mut fields = envelope.fields.clone();
    fields.push(ResourceField::required(
        FieldId(60_001),
        ResourceWireType::Bytes,
        b"future required".to_vec(),
    ));
    let record_count = envelope.header.record_count;
    let envelope = arcweft_bundle::resource_codec::ProductResourceEnvelope::new(
        arcweft_bundle::resource_codec::ProductSectionCodecKind::RuntimeTypes,
        envelope.strings,
        envelope.public_ids,
        envelope.enums,
        fields,
        record_count,
    )
    .expect("required envelope rebuilds");
    let bytes = envelope.encode_canonical().expect("envelope encodes");
    assert!(matches!(
        RuntimeTypesSection::decode_canonical_section(&bytes),
        Err(SectionCodecError::UnknownRequiredField(FieldId(60_001)))
    ));
}

#[test]
fn patch_compatibility_fingerprints_classify_runtime_resource_changes() {
    let base = runtime_types_section();
    let added = RuntimeTypesSection::new(
        base.abi_version,
        base.runtime_layout_digest,
        base.declarations
            .iter()
            .cloned()
            .chain([RuntimeTypeDeclaration {
                public_id: Some("type.extra".to_owned()),
                value_kind: RuntimeValueKind::Record,
                layout_digest: digest(b"extra"),
                compatibility: TypeCompatibilityLabel::CodeCompatible,
            }]),
        base.function_interfaces.clone(),
    );
    assert_eq!(
        base.compatibility_with(&added),
        RuntimeResourceCompatibility::CodeCompatible
    );

    let mut broken = base.clone();
    broken.abi_version += 1;
    assert_eq!(
        base.compatibility_with(&broken),
        RuntimeResourceCompatibility::RestartRequired
    );

    assert_eq!(
        RuntimeResourceCompatibility::CodeGenerational.patch_compatibility(),
        PatchCompatibility::CodeGenerational
    );
}

#[test]
fn patch_artifact_from_views_uses_compact_runtime_compatibility() {
    let base = runtime_types_section();
    let target = RuntimeTypesSection::new(
        base.abi_version,
        base.runtime_layout_digest,
        base.declarations
            .iter()
            .cloned()
            .chain([RuntimeTypeDeclaration {
                public_id: Some("type.extra".to_owned()),
                value_kind: RuntimeValueKind::Record,
                layout_digest: digest(b"extra"),
                compatibility: TypeCompatibilityLabel::CodeCompatible,
            }]),
        base.function_interfaces.clone(),
    );
    let base_bytes = compact_program_with_runtime_types(&base);
    let target_bytes = compact_program_with_runtime_types(&target);
    let base_view = BundleView::parse(&base_bytes, ReadBudget::default()).expect("base parses");
    let target_view =
        BundleView::parse(&target_bytes, ReadBudget::default()).expect("target parses");

    let artifact = BundlePatchArtifact::from_views(&base_view, &target_view)
        .expect("patch artifact classifies compact runtime sections");

    assert_eq!(
        artifact.manifest.compatibility,
        PatchCompatibility::CodeCompatible
    );
}

#[test]
fn migrated_runtime_section_compatibility_decodes_compact_bytes() {
    let base = runtime_types_section();
    let target = RuntimeTypesSection::new(
        base.abi_version,
        base.runtime_layout_digest,
        base.declarations.clone(),
        base.function_interfaces
            .iter()
            .cloned()
            .map(|mut fingerprint| {
                fingerprint.signature_digest = digest(b"new signature");
                fingerprint.compatibility = TypeCompatibilityLabel::CodeGenerational;
                fingerprint
            }),
    );

    let compatibility = migrated_runtime_section_compatibility(
        arcweft_bundle::container::BundleSectionKind::RuntimeTypes,
        &base.encode_canonical_section().expect("base encodes"),
        &target.encode_canonical_section().expect("target encodes"),
    )
    .expect("compatibility decodes")
    .expect("runtime section recognized");

    assert_eq!(
        compatibility,
        RuntimeResourceCompatibility::CodeGenerational
    );
}

trait RuntimeTypesSectionTestExt {
    fn envelope_for_test(
        &self,
    ) -> Result<arcweft_bundle::resource_codec::ProductResourceEnvelope, SectionCodecError>;
}

impl RuntimeTypesSectionTestExt for RuntimeTypesSection {
    fn envelope_for_test(
        &self,
    ) -> Result<arcweft_bundle::resource_codec::ProductResourceEnvelope, SectionCodecError> {
        let bytes = self.encode_canonical_section()?;
        arcweft_bundle::resource_codec::ProductResourceEnvelope::decode_all_fields(
            &bytes,
            arcweft_bundle::resource_codec::ProductSectionCodecKind::RuntimeTypes,
            SectionCodecBudget::default(),
        )
    }
}

fn runtime_types_section() -> RuntimeTypesSection {
    RuntimeTypesSection::new(
        1,
        AwbcDigest([0xa8; 32]),
        [RuntimeTypeDeclaration {
            public_id: Some("type.actor".to_owned()),
            value_kind: RuntimeValueKind::Record,
            layout_digest: digest(b"actor layout"),
            compatibility: TypeCompatibilityLabel::RestartRequired,
        }],
        [FunctionInterfaceFingerprint {
            public_id: Some("flow.main".to_owned()),
            awbc_function_index: 0,
            kind: RuntimeFunctionKind::Flow,
            signature_digest: digest(b"signature"),
            frame_layout_digest: digest(b"frame"),
            flags: 0,
            compatibility: TypeCompatibilityLabel::CodeCompatible,
        }],
    )
}

fn compact_program_with_runtime_types(runtime_types: &RuntimeTypesSection) -> Vec<u8> {
    let entrypoints = EntrypointsSection::new([EntrypointDeclaration {
        public_id: "entry.main".to_owned(),
        exported_name: Some("main".to_owned()),
        awbc_function_index: Some(0),
        initial_state: InitialStateRequirement::None,
        source_anchor: None,
        visibility: ProductVisibility::Public,
    }]);
    let adapters = AdapterRequirementsSection::new(
        None,
        Vec::<String>::new(),
        Vec::<String>::new(),
        Vec::<arcweft_bundle::BundleAdapterManifest>::new(),
    );
    encode_bundle(
        ContainerBundleKind::Program,
        br#"{"kind":"program"}"#,
        vec![
            SectionInput::embedded(
                test_section_id(BundleSectionKind::ProgramBytecode),
                BundleSectionKind::ProgramBytecode,
                1,
                ContentResidency::Startup,
                true,
                b"test-awbc",
            ),
            SectionInput::embedded(
                test_section_id(BundleSectionKind::RuntimeTypes),
                BundleSectionKind::RuntimeTypes,
                1,
                ContentResidency::Startup,
                true,
                runtime_types
                    .encode_canonical_section()
                    .expect("runtime types encode"),
            ),
            SectionInput::embedded(
                test_section_id(BundleSectionKind::Entrypoints),
                BundleSectionKind::Entrypoints,
                1,
                ContentResidency::Startup,
                true,
                entrypoints
                    .encode_canonical_section()
                    .expect("entrypoints encode"),
            ),
            SectionInput::embedded(
                test_section_id(BundleSectionKind::AdapterRequirements),
                BundleSectionKind::AdapterRequirements,
                1,
                ContentResidency::Startup,
                true,
                adapters
                    .encode_canonical_section()
                    .expect("adapters encode"),
            ),
            SectionInput::embedded(
                test_section_id(BundleSectionKind::ContentCatalog),
                BundleSectionKind::ContentCatalog,
                1,
                ContentResidency::Startup,
                true,
                b"{}",
            ),
        ],
    )
    .expect("program bundle encodes")
}

fn test_section_id(kind: BundleSectionKind) -> SectionId {
    let mut id = [0_u8; 16];
    id[..4].copy_from_slice(&kind.encoded().to_le_bytes());
    id[4..].copy_from_slice(&BundleDigest::of(b"seq-02.2-runtime-test").as_bytes()[..12]);
    SectionId::from_bytes(id)
}

fn digest(bytes: &[u8]) -> BundleDigest {
    BundleDigest::of(bytes)
}
