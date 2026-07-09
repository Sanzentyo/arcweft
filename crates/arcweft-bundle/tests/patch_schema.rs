use arcweft_bundle::container::{
    BundleDigest, BundleKind, BundleSectionKind, BundleView, ContentPlacement, ContentResidency,
    ReadBudget, SectionId, SectionInput, SectionKindCode, encode_bundle,
};
use arcweft_bundle::patch::{
    BundlePatchArtifact, PATCH_PLAN_SCHEMA_VERSION, PatchBundleError, PatchCompatibility,
    PatchManifestRewrite, PatchMaterializationState, SectionChangeDerivation,
    SectionChangeOperation, apply_patch_bundle, apply_patch_bundle_bytes, decode_patch_bundle,
    encode_patch_bundle,
};

#[derive(Clone, Copy, Debug)]
enum FingerprintTamper {
    Compatibility,
    Operation,
    RawKind,
    KnownKind,
    Required,
    Derivation,
    BaseDescriptor,
    TargetDescriptor,
    BaseContent,
    TargetContent,
}

impl FingerprintTamper {
    const ALL: [Self; 10] = [
        Self::Compatibility,
        Self::Operation,
        Self::RawKind,
        Self::KnownKind,
        Self::Required,
        Self::Derivation,
        Self::BaseDescriptor,
        Self::TargetDescriptor,
        Self::BaseContent,
        Self::TargetContent,
    ];

    fn apply(self, artifact: &mut BundlePatchArtifact) {
        let fingerprint = artifact
            .manifest
            .compatibility_fingerprints
            .first_mut()
            .expect("fixture has one changed section");
        match self {
            Self::Compatibility => {
                fingerprint.compatibility = PatchCompatibility::CodeCompatible;
                artifact.manifest.compatibility = PatchCompatibility::CodeCompatible;
            }
            Self::Operation => fingerprint.operation = SectionChangeOperation::Add,
            Self::RawKind => fingerprint.raw_kind_code ^= 0x0100_0000,
            Self::KnownKind => fingerprint.known_kind = Some(BundleSectionKind::AudioGraph),
            Self::Required => fingerprint.required = !fingerprint.required,
            Self::Derivation => {
                fingerprint.derivation = SectionChangeDerivation::ExternalDescriptor;
            }
            Self::BaseDescriptor => {
                fingerprint.base_descriptor_fingerprint =
                    Some(BundleDigest::of(b"tampered base descriptor"));
            }
            Self::TargetDescriptor => {
                fingerprint.target_descriptor_fingerprint =
                    Some(BundleDigest::of(b"tampered target descriptor"));
            }
            Self::BaseContent => {
                fingerprint.base_content_fingerprint =
                    Some(BundleDigest::of(b"tampered base content"));
            }
            Self::TargetContent => {
                fingerprint.target_content_fingerprint =
                    Some(BundleDigest::of(b"tampered target content"));
            }
        }
    }
}

fn content_pack(
    manifest: &'static [u8],
    asset_blob: &'static [u8],
    include_catalog: bool,
) -> Vec<u8> {
    let mut sections = vec![SectionInput::embedded(
        SectionId::from_bytes([1; 16]),
        BundleSectionKind::AssetBlob,
        1,
        ContentResidency::OnDemand,
        false,
        asset_blob,
    )];
    if include_catalog {
        sections.push(SectionInput::embedded(
            SectionId::from_bytes([2; 16]),
            BundleSectionKind::ContentCatalog,
            1,
            ContentResidency::Startup,
            false,
            b"catalog",
        ));
    }
    encode_bundle(BundleKind::ContentPack, manifest, sections).expect("content pack encodes")
}

fn external_content_pack(manifest: &'static [u8], asset_blob: &'static [u8]) -> Vec<u8> {
    encode_bundle(
        BundleKind::ContentPack,
        manifest,
        vec![SectionInput::external_ref(
            SectionId::from_bytes([1; 16]),
            BundleSectionKind::AssetBlob,
            1,
            ContentResidency::OnDemand,
            false,
            u64::try_from(asset_blob.len()).expect("fixture length fits u64"),
            BundleDigest::of(asset_blob),
        )],
    )
    .expect("external content pack encodes")
}

fn unknown_optional_pack(manifest: &'static [u8], bytes: &'static [u8]) -> Vec<u8> {
    encode_bundle(
        BundleKind::ContentPack,
        manifest,
        vec![
            SectionInput::embedded_raw_optional(
                SectionId::from_bytes([9; 16]),
                SectionKindCode::new(0xfeed_beef),
                1,
                ContentResidency::OnDemand,
                false,
                bytes,
            )
            .expect("unknown optional section encodes"),
        ],
    )
    .expect("unknown optional content pack encodes")
}

fn patch_artifact(base: &[u8], target: &[u8]) -> BundlePatchArtifact {
    let base = BundleView::parse(base, ReadBudget::default()).expect("base parses");
    let target = BundleView::parse(target, ReadBudget::default()).expect("target parses");
    BundlePatchArtifact::from_views(&base, &target).expect("patch artifact")
}

#[test]
fn patch_schema_two_is_the_only_decoded_schema() {
    let base = content_pack(br#"{"kind":"content","rev":1}"#, b"old", true);
    let target = content_pack(br#"{"kind":"content","rev":2}"#, b"new", true);
    let mut artifact = patch_artifact(&base, &target);

    assert_eq!(artifact.manifest.schema_version, PATCH_PLAN_SCHEMA_VERSION);
    artifact.manifest.schema_version = 1;

    let error = encode_patch_bundle(&artifact).expect_err("schema 1 is not accepted");
    assert!(matches!(
        error,
        PatchBundleError::UnsupportedSchema { actual: 1, .. }
    ));
}

#[test]
fn patch_bytes_are_deterministic_and_round_trip_schema_two() {
    let base = content_pack(br#"{"kind":"content","rev":1}"#, b"old", true);
    let target = content_pack(br#"{"kind":"content","rev":1}"#, b"new", false);
    let artifact = patch_artifact(&base, &target);

    let first = encode_patch_bundle(&artifact).expect("patch encodes");
    let second = encode_patch_bundle(&artifact).expect("patch encodes deterministically");
    let decoded = decode_patch_bundle(&first).expect("patch decodes");

    assert_eq!(first, second);
    assert_eq!(decoded, artifact);
    assert_eq!(
        decoded.manifest.compatibility,
        PatchCompatibility::RestartRequired
    );
}

#[test]
fn materialization_rewrites_manifest_and_reports_unsigned_target_identity() {
    let base = content_pack(br#"{"kind":"content","rev":1}"#, b"old", true);
    let target = content_pack(br#"{"kind":"content","rev":2}"#, b"new", true);
    let target_view = BundleView::parse(&target, ReadBudget::default()).expect("target parses");
    let artifact = patch_artifact(&base, &target);

    assert_eq!(
        artifact.manifest.materialization.manifest_rewrite,
        PatchManifestRewrite::ReplaceWithTargetManifestBytes
    );

    let materialized = apply_patch_bundle(&base, &artifact).expect("patch materializes");
    let materialized_view =
        BundleView::parse(materialized.bytes(), ReadBudget::default()).expect("target parses");

    assert_eq!(materialized_view.manifest(), target_view.manifest());
    assert_eq!(materialized_view.content_root(), target_view.content_root());
    assert_eq!(
        materialized.report().target_artifact,
        target_view.artifact_identity()
    );
    assert_eq!(
        materialized.report().completed_states.last(),
        Some(&PatchMaterializationState::Materialized)
    );
    assert_eq!(
        materialized.compatibility(),
        artifact.manifest.compatibility
    );
    assert_eq!(materialized.into_bytes(), target);
}

#[test]
fn patch_plan_rejects_duplicate_operation_ids() {
    let base = content_pack(br#"{"kind":"content"}"#, b"old", false);
    let target = content_pack(br#"{"kind":"content"}"#, b"new", false);
    let mut artifact = patch_artifact(&base, &target);
    let duplicate_id = match &artifact.plan.operations[0] {
        arcweft_bundle::patch::SectionOperation::Add(descriptor) => descriptor.id(),
        arcweft_bundle::patch::SectionOperation::Replace { next, .. } => next.id(),
        arcweft_bundle::patch::SectionOperation::Remove { id, .. } => *id,
    };
    artifact
        .plan
        .operations
        .push(artifact.plan.operations[0].clone());

    let error = artifact
        .validate()
        .expect_err("duplicate operation id must reject");

    assert!(matches!(
        error,
        PatchBundleError::DuplicateSectionOperation(id) if id == duplicate_id
    ));
}

#[test]
fn materialization_rejects_every_tampered_compatibility_fingerprint_field() {
    let base = content_pack(br#"{"kind":"content"}"#, b"old", false);
    let target = content_pack(br#"{"kind":"content"}"#, b"new", false);

    for tamper in FingerprintTamper::ALL {
        let mut artifact = patch_artifact(&base, &target);
        tamper.apply(&mut artifact);
        let patch_bytes = encode_patch_bundle(&artifact).unwrap_or_else(|error| {
            panic!("{tamper:?} must pass manifest self-validation: {error}")
        });

        let error = apply_patch_bundle_bytes(&base, &patch_bytes)
            .expect_err("materialized target must reject the tampered fingerprint");
        match error {
            PatchBundleError::MaterializedFingerprintMismatch {
                declared, derived, ..
            } => assert_ne!(declared, derived, "{tamper:?} must change a verified field"),
            error => panic!("{tamper:?} returned the wrong error: {error}"),
        }
    }
}

#[test]
fn missing_target_manifest_rolls_back_before_target_encoding() {
    let base = content_pack(br#"{"kind":"content","rev":1}"#, b"old", true);
    let target = content_pack(br#"{"kind":"content","rev":2}"#, b"new", true);
    let mut artifact = patch_artifact(&base, &target);
    artifact.target_manifest_bytes = None;

    let error = apply_patch_bundle(&base, &artifact).expect_err("missing target manifest rejects");

    assert!(matches!(
        error,
        PatchBundleError::MissingTargetManifest { .. }
    ));
}

#[test]
fn external_descriptor_change_is_metadata_only_and_preserved() {
    let base = external_content_pack(br#"{"kind":"content"}"#, b"old external bytes");
    let target = external_content_pack(br#"{"kind":"content"}"#, b"new external bytes");
    let target_view = BundleView::parse(&target, ReadBudget::default()).expect("target parses");
    let artifact = patch_artifact(&base, &target);

    assert!(artifact.changed_sections.is_empty());
    assert_eq!(
        artifact.manifest.compatibility_fingerprints[0].derivation,
        SectionChangeDerivation::ExternalDescriptor
    );

    let materialized = apply_patch_bundle(&base, &artifact).expect("patch applies");
    let view =
        BundleView::parse(materialized.bytes(), ReadBudget::default()).expect("patched parses");
    let section = view
        .sections()
        .iter()
        .find(|section| section.id() == SectionId::from_bytes([1; 16]))
        .expect("section exists");

    assert_eq!(view.content_root(), target_view.content_root());
    assert_eq!(section.placement(), ContentPlacement::External);
    assert_eq!(
        section.content_digest(),
        BundleDigest::of(b"new external bytes")
    );
}

#[test]
fn unknown_optional_section_kind_is_preserved_through_patch() {
    let base = unknown_optional_pack(br#"{"kind":"content"}"#, b"old opaque");
    let target = unknown_optional_pack(br#"{"kind":"content"}"#, b"new opaque");
    let artifact = patch_artifact(&base, &target);

    assert_eq!(
        artifact.manifest.compatibility_fingerprints[0].derivation,
        SectionChangeDerivation::UnknownOptionalSectionKind
    );

    let materialized = apply_patch_bundle(&base, &artifact).expect("patch applies");
    let view =
        BundleView::parse(materialized.bytes(), ReadBudget::default()).expect("patched parses");
    let section = view.sections().first().expect("opaque section exists");

    assert_eq!(section.kind_code().encoded(), 0xfeed_beef);
    assert_eq!(section.known_kind(), None);
}
