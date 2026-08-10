use arcweft_bundle::{
    container::{BundleSectionKind, SectionId},
    resource_type_manifests::{
        RESOURCE_TYPE_MANIFESTS_SECTION_MAGIC, RESOURCE_TYPE_MANIFESTS_SECTION_SCHEMA,
        ResourceTypeManifestSectionError, decode_resource_type_manifest_section_v1,
        encode_resource_type_manifest_section_v1, resource_type_manifest_section_input_v1,
    },
};
use arcweft_manifest_model::{PackageId, PackageVersion, RawDigest};
use arcweft_resource_manifest::{
    PackageCoordinateFile, ResourceManifestDecodeLimits, ResourceManifestDiagnosticCode,
    ResourceManifestPublicationLimits, decode_resource_type_manifest,
    publish_resource_type_manifests_v1,
};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::sync::Arc;

const ENTRY_LEN_OFFSET: usize = 48;
const ENTRY_DIGEST_OFFSET: usize = 56;
const ENTRY_BYTES_OFFSET: usize = 88;
const HEADER_LEN: usize = 48;
const ENTRY_HEADER_LEN: usize = 40;

fn published() -> arcweft_resource_manifest::PublishedResourceTypeManifestSetV1 {
    let source = include_str!("../../arcweft-resource-manifest/tests/fixtures/minimal.input.json");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("bundle-resource-manifest").unwrap(),
            SourceName::Memory,
            source,
        )
        .unwrap(),
    );
    let coordinate = PackageCoordinateFile::new(
        PackageId::new("org.example.weather").unwrap(),
        PackageVersion::new("1.0.0").unwrap(),
    );
    let manifest = decode_resource_type_manifest(
        document,
        &coordinate,
        ResourceManifestDecodeLimits::PRODUCTION,
    )
    .unwrap();
    publish_resource_type_manifests_v1(
        &ResourceTypeRegistry::empty(),
        [manifest],
        ResourceManifestPublicationLimits::PRODUCTION,
    )
    .unwrap()
}

fn published_empty_packages(
    packages: &[&str],
) -> arcweft_resource_manifest::PublishedResourceTypeManifestSetV1 {
    let manifests = packages.iter().map(|package| {
        let source = format!(
            "{{\"format\":\"arcweft.resource-type-manifest\",\"schema\":1,\"package\":{{\"id\":\"{package}\",\"version\":\"1.0.0\"}},\"schemas\":[],\"resource_types\":[],\"codecs\":[]}}"
        );
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!("bundle-resource-manifest:{package}"))
                    .unwrap(),
                SourceName::Memory,
                source,
            )
            .unwrap(),
        );
        let coordinate = PackageCoordinateFile::new(
            PackageId::new(*package).unwrap(),
            PackageVersion::new("1.0.0").unwrap(),
        );
        decode_resource_type_manifest(
            document,
            &coordinate,
            ResourceManifestDecodeLimits::PRODUCTION,
        )
        .unwrap()
    });
    publish_resource_type_manifests_v1(
        &ResourceTypeRegistry::empty(),
        manifests,
        ResourceManifestPublicationLimits::PRODUCTION,
    )
    .unwrap()
}

fn encoded() -> Vec<u8> {
    encode_resource_type_manifest_section_v1(&published())
        .unwrap()
        .unwrap()
}

#[test]
fn code_22_required_section_round_trips_against_the_engine_base() {
    assert_eq!(BundleSectionKind::ResourceTypeManifests.encoded(), 22);
    assert_eq!(
        BundleSectionKind::from_encoded(22),
        Some(BundleSectionKind::ResourceTypeManifests)
    );
    let original = published();
    let bytes = encode_resource_type_manifest_section_v1(&original)
        .unwrap()
        .unwrap();
    assert_eq!(&bytes[..8], &RESOURCE_TYPE_MANIFESTS_SECTION_MAGIC);
    assert_eq!(
        u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
        RESOURCE_TYPE_MANIFESTS_SECTION_SCHEMA
    );
    let section =
        resource_type_manifest_section_input_v1(SectionId::from_bytes([22; 16]), &original)
            .unwrap()
            .unwrap();
    assert_eq!(section.kind(), BundleSectionKind::ResourceTypeManifests);
    let decoded = decode_resource_type_manifest_section_v1(
        &bytes,
        &ResourceTypeRegistry::empty(),
        ResourceManifestDecodeLimits::PRODUCTION,
        ResourceManifestPublicationLimits::PRODUCTION,
    )
    .unwrap();
    assert_eq!(decoded.registry_digest(), original.registry_digest());
    assert_eq!(decoded.manifests().len(), 1);
    assert_eq!(
        decoded.manifests()[0].canonical_bytes(),
        original.manifests()[0].canonical_bytes()
    );
}

#[test]
fn empty_extension_set_omits_the_section() {
    let empty = publish_resource_type_manifests_v1(
        &ResourceTypeRegistry::empty(),
        [],
        ResourceManifestPublicationLimits::PRODUCTION,
    )
    .unwrap();
    assert_eq!(
        encode_resource_type_manifest_section_v1(&empty).unwrap(),
        None
    );
    assert!(
        resource_type_manifest_section_input_v1(SectionId::from_bytes([22; 16]), &empty)
            .unwrap()
            .is_none()
    );
}

#[test]
fn entry_digest_is_checked_before_json_decode() {
    let mut bytes = encoded();
    bytes[ENTRY_DIGEST_OFFSET] ^= 1;
    let error = decode(&bytes).unwrap_err();
    assert!(matches!(
        error,
        ResourceTypeManifestSectionError::ArtifactDigestMismatch { entry: 0 }
    ));
}

#[test]
fn valid_noncanonical_json_is_rejected_after_the_sole_manifest_decode() {
    let mut bytes = encoded();
    bytes.insert(ENTRY_BYTES_OFFSET, b' ');
    let manifest_len = bytes.len() - ENTRY_BYTES_OFFSET;
    let digest = *RawDigest::for_bytes(&bytes[ENTRY_BYTES_OFFSET..]).as_bytes();
    bytes[ENTRY_LEN_OFFSET..ENTRY_DIGEST_OFFSET]
        .copy_from_slice(&u64::try_from(manifest_len).unwrap().to_le_bytes());
    bytes[ENTRY_DIGEST_OFFSET..ENTRY_BYTES_OFFSET].copy_from_slice(&digest);
    let error = decode(&bytes).unwrap_err();
    assert!(matches!(
        error,
        ResourceTypeManifestSectionError::ArtifactNonCanonicalManifest { entry: 0 }
    ));
}

#[test]
fn final_registry_digest_and_exact_section_end_are_authoritative() {
    let mut wrong_registry = encoded();
    wrong_registry[16] ^= 1;
    assert_eq!(
        decode(&wrong_registry).unwrap_err().code(),
        ResourceManifestDiagnosticCode::RegistryDigestMismatch
    );

    let mut trailing = encoded();
    trailing.push(0);
    assert_eq!(
        decode(&trailing).unwrap_err().code(),
        ResourceManifestDiagnosticCode::ArtifactMalformed
    );
}

#[test]
fn reordered_entries_are_rejected_before_publication() {
    let published = published_empty_packages(&["org.example.alpha", "org.example.beta"]);
    let bytes = encode_resource_type_manifest_section_v1(&published)
        .unwrap()
        .unwrap();
    let first_len = usize::try_from(u64::from_le_bytes(
        bytes[HEADER_LEN..HEADER_LEN + 8].try_into().unwrap(),
    ))
    .unwrap();
    let second_start = HEADER_LEN + ENTRY_HEADER_LEN + first_len;
    let second_len = usize::try_from(u64::from_le_bytes(
        bytes[second_start..second_start + 8].try_into().unwrap(),
    ))
    .unwrap();
    let second_end = second_start + ENTRY_HEADER_LEN + second_len;
    let mut reordered = bytes[..HEADER_LEN].to_vec();
    reordered.extend_from_slice(&bytes[second_start..second_end]);
    reordered.extend_from_slice(&bytes[HEADER_LEN..second_start]);

    let error = decode(&reordered).unwrap_err();
    assert_eq!(
        error.code(),
        ResourceManifestDiagnosticCode::ArtifactMalformed
    );
}

#[test]
fn truncated_entry_and_count_overflow_are_rejected_as_malformed() {
    let mut truncated = encoded();
    truncated.pop();
    assert_eq!(
        decode(&truncated).unwrap_err().code(),
        ResourceManifestDiagnosticCode::ArtifactMalformed
    );

    let mut count_overflow = encoded();
    count_overflow[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        decode(&count_overflow).unwrap_err().code(),
        ResourceManifestDiagnosticCode::ArtifactMalformed
    );
}

fn decode(
    bytes: &[u8],
) -> Result<
    arcweft_resource_manifest::PublishedResourceTypeManifestSetV1,
    ResourceTypeManifestSectionError,
> {
    decode_resource_type_manifest_section_v1(
        bytes,
        &ResourceTypeRegistry::empty(),
        ResourceManifestDecodeLimits::PRODUCTION,
        ResourceManifestPublicationLimits::PRODUCTION,
    )
}
