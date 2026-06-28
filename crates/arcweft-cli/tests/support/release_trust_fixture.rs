use arcweft_bundle::{
    container::{
        BundleDigest, BundleKind, BundleSectionKind, BundleView, ContentResidency, ReadBudget,
        SectionId, SectionInput, append_signature_block, encode_bundle,
    },
    patch::{BundlePatchArtifact, encode_patch_bundle},
    release::{
        RELEASE_MANIFEST_SCHEMA_VERSION, RELEASE_SIGNATURE_ALGORITHM_ED25519_V1, ReleaseBundleRef,
        ReleaseFetchPolicy, ReleaseManifest, ReleaseMirror, ReleaseSignatureEnvelope,
        ReleaseSignaturePolicy, ReleaseTrustedPublicKey,
        archive::{
            AwfrArchiveManifest, AwfrArchiveSignatureRef, AwfrPatchArtifactRef,
            AwfrPublicationMetadata, ExternalPayloadCarrier, ExternalPayloadMediaType,
            ReleaseChannel,
        },
        signing_policy::SigningDigestTranscript,
    },
};
use arcweft_project_loader::cache::store::FilesystemCacheStore;
use ed25519_dalek::Signer as _;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub const SIGNER_ID: &str = "seq02-9-test-fixture-key-do-not-use";
pub const CHANNEL: &str = "seq02-9-fixture";
pub const KEY_EPOCH: u64 = 4;
pub const SIGNING_KEY_BYTES: [u8; 32] = [7; 32];
const EXTERNAL_PAYLOAD: &[u8] = b"seq02.9 deterministic external payload";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseTrustCase {
    SuccessCacheHit,
    MissingPatchSignature,
    ExternalPayloadMissing,
    DetachedSignatureTranscriptMismatch,
}

pub struct BuiltReleaseTrustFixture {
    pub root: PathBuf,
    pub archive_path: PathBuf,
    pub cache_root: PathBuf,
}

pub fn build_release_trust_fixture(case: ReleaseTrustCase) -> BuiltReleaseTrustFixture {
    let root = temp_root(case.label());
    let artifacts = root.join("artifacts");
    let cache_root = root.join("cache");
    fs::create_dir_all(&artifacts).expect("fixture artifact dir");

    let external_required = true;
    let base_unsigned = content_pack(
        b"base embedded payload",
        EXTERNAL_PAYLOAD,
        external_required,
    );
    let target_unsigned = content_pack(
        b"target embedded payload",
        EXTERNAL_PAYLOAD,
        external_required,
    );

    let base_bytes = sign_awfb_bytes(&base_unsigned);
    let target_bytes = sign_awfb_bytes(&target_unsigned);

    let base_unsigned_view =
        BundleView::parse(&base_unsigned, ReadBudget::default()).expect("base unsigned parses");
    let target_unsigned_view =
        BundleView::parse(&target_unsigned, ReadBudget::default()).expect("target unsigned parses");
    let patch_artifact =
        BundlePatchArtifact::from_views(&base_unsigned_view, &target_unsigned_view)
            .expect("patch artifact");
    let patch_unsigned = encode_patch_bundle(&patch_artifact).expect("patch encodes");
    let patch_bytes = if case == ReleaseTrustCase::MissingPatchSignature {
        patch_unsigned.clone()
    } else {
        sign_awfb_bytes(&patch_unsigned)
    };

    fs::write(artifacts.join("base.awfb"), &base_bytes).expect("base writes");
    fs::write(artifacts.join("patch.awfb"), &patch_bytes).expect("patch writes");
    fs::write(artifacts.join("target.awfb"), &target_bytes).expect("target writes");

    let payload_path = artifacts.join("payload.bin");
    match case {
        ReleaseTrustCase::ExternalPayloadMissing => {}
        _ => fs::write(&payload_path, EXTERNAL_PAYLOAD).expect("payload writes"),
    }

    let target_view_for_carrier =
        BundleView::parse(&target_unsigned, ReadBudget::default()).expect("target carrier view");
    let carrier = external_payload_carrier(&target_view_for_carrier, payload_mirrors(case));
    if case == ReleaseTrustCase::SuccessCacheHit {
        FilesystemCacheStore::new(&cache_root)
            .put_object(EXTERNAL_PAYLOAD)
            .expect("cache object seeded");
    }

    let base_ref = ReleaseBundleRef::from_awfb_bytes(
        &base_bytes,
        [ReleaseMirror::new("file:artifacts/base.awfb").expect("base mirror")],
    )
    .expect("base ref");
    let target_ref = ReleaseBundleRef::from_awfb_bytes(
        &target_bytes,
        [ReleaseMirror::new("file:artifacts/target.awfb").expect("target mirror")],
    )
    .expect("target ref");
    let release_manifest = ReleaseManifest {
        schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
        fetch_policy: ReleaseFetchPolicy::default(),
        signature_policy: ReleaseSignaturePolicy::require_trusted_public_keys(
            Some(64),
            [trusted_public_key()],
        )
        .expect("signature policy"),
        bundles: vec![base_ref, target_ref],
    };
    release_manifest
        .validate()
        .expect("release manifest validates");

    let patch_view = BundleView::parse(&patch_bytes, ReadBudget::default()).expect("patch view");
    let patch_ref = AwfrPatchArtifactRef {
        patch_artifact: patch_view.artifact_identity(),
        target_artifact: target_unsigned_view.artifact_identity(),
        file_digest: BundleDigest::of(&patch_bytes),
        byte_len: patch_bytes.len() as u64,
        mirrors: vec![ReleaseMirror::new("file:artifacts/patch.awfb").expect("patch mirror")],
    };

    let mut archive = AwfrArchiveManifest::new(
        ReleaseChannel::new(CHANNEL).expect("channel"),
        release_manifest,
        [carrier],
    )
    .expect("archive");
    archive.publication = Some(AwfrPublicationMetadata {
        release_name: case.label().to_owned(),
        sequence: 1,
        published_at_epoch_millis: Some(0),
    });
    archive.patches.push(patch_ref);
    sign_awfr_archive(&mut archive);

    if case == ReleaseTrustCase::DetachedSignatureTranscriptMismatch {
        archive.signatures[0].signing_digest = BundleDigest::of(b"wrong detached transcript");
    }

    let archive_path = write_archive(&root, &archive);

    BuiltReleaseTrustFixture {
        root,
        archive_path,
        cache_root,
    }
}

impl ReleaseTrustCase {
    fn label(self) -> &'static str {
        match self {
            Self::SuccessCacheHit => "success-cache-hit",
            Self::MissingPatchSignature => "missing-patch-signature",
            Self::ExternalPayloadMissing => "external-payload-missing",
            Self::DetachedSignatureTranscriptMismatch => "detached-signature-transcript-mismatch",
        }
    }
}

fn write_archive(root: &std::path::Path, archive: &AwfrArchiveManifest) -> PathBuf {
    let archive_path = root.join("game.awfr");
    fs::write(
        &archive_path,
        archive.to_json_bytes().expect("archive JSON"),
    )
    .expect("archive writes");
    archive_path
}

fn content_pack(embedded: &[u8], external_payload: &[u8], external_required: bool) -> Vec<u8> {
    encode_bundle(
        BundleKind::ContentPack,
        br#"{"kind":"seq02.9-fixture-content"}"#,
        vec![
            SectionInput::embedded(
                SectionId::from_bytes([1; 16]),
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::Startup,
                true,
                embedded,
            ),
            SectionInput::external_ref(
                SectionId::from_bytes([2; 16]),
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::Startup,
                external_required,
                external_payload.len() as u64,
                BundleDigest::of(external_payload),
            ),
        ],
    )
    .expect("content pack encodes")
}

fn external_payload_carrier(
    target_view: &BundleView<'_>,
    mirrors: Vec<ReleaseMirror>,
) -> ExternalPayloadCarrier {
    let descriptor = target_view
        .sections()
        .iter()
        .find(|descriptor| descriptor.id() == SectionId::from_bytes([2; 16]))
        .expect("external descriptor");
    ExternalPayloadCarrier::from_descriptor(
        descriptor,
        target_view.artifact_identity(),
        ExternalPayloadMediaType::default(),
        EXTERNAL_PAYLOAD.len() as u64,
        BundleDigest::of(EXTERNAL_PAYLOAD),
        mirrors,
    )
    .expect("external payload carrier")
}

fn payload_mirrors(case: ReleaseTrustCase) -> Vec<ReleaseMirror> {
    match case {
        ReleaseTrustCase::SuccessCacheHit => {
            vec![ReleaseMirror::new("arcweft-cache:seq02-9-payload").expect("cache mirror")]
        }
        ReleaseTrustCase::ExternalPayloadMissing => vec![
            ReleaseMirror::new("file:artifacts/missing-payload.bin").expect("missing file mirror"),
        ],
        _ => vec![ReleaseMirror::new("file:artifacts/payload.bin").expect("payload mirror")],
    }
}

fn sign_awfb_bytes(bytes: &[u8]) -> Vec<u8> {
    let view = BundleView::parse(bytes, ReadBudget::default()).expect("AWFB parses for signing");
    assert!(
        view.signature().is_none(),
        "fixture signer expects unsigned input"
    );
    let signing_digest = view.signing_digest().expect("AWFB signing digest");
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&SIGNING_KEY_BYTES);
    let mut envelope = ReleaseSignatureEnvelope::new(
        SIGNER_ID,
        RELEASE_SIGNATURE_ALGORITHM_ED25519_V1,
        view.content_root(),
        view.kind(),
        signing_digest,
        encode_hex(&[0; 64]),
    )
    .expect("signature envelope");
    envelope.key_epoch = KEY_EPOCH;
    let signature = signing_key.sign(&envelope.signing_message());
    envelope.signature = encode_hex(&signature.to_bytes());
    let envelope_bytes = envelope.to_json_bytes().expect("signature envelope bytes");
    append_signature_block(bytes, &envelope_bytes).expect("signature appended")
}

fn sign_awfr_archive(archive: &mut AwfrArchiveManifest) {
    archive.signatures.clear();
    let unsigned_digest = archive
        .unsigned_whole_file_digest()
        .expect("unsigned AWFR digest");
    let transcript = SigningDigestTranscript::awfr_release_archive(
        archive,
        unsigned_digest,
        SIGNER_ID,
        KEY_EPOCH,
    )
    .expect("AWFR signing transcript");
    archive.signatures.push(AwfrArchiveSignatureRef {
        signer_id: SIGNER_ID.to_owned(),
        algorithm: RELEASE_SIGNATURE_ALGORITHM_ED25519_V1.to_owned(),
        key_epoch: KEY_EPOCH,
        signing_digest: transcript.digest().expect("AWFR transcript digest"),
        signature: "fixture-detached-signature-not-production".to_owned(),
    });
}

fn trusted_public_key() -> ReleaseTrustedPublicKey {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&SIGNING_KEY_BYTES);
    ReleaseTrustedPublicKey::ed25519_v1(
        SIGNER_ID,
        encode_hex(&signing_key.verifying_key().to_bytes()),
    )
    .expect("trusted fixture public key")
    .with_key_epoch_validity(KEY_EPOCH, Some(KEY_EPOCH + 1))
    .expect("trusted fixture key epoch")
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut hex, byte| {
            use std::fmt::Write as _;
            write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
            hex
        })
}

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "arcweft-seq02-9-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos(),
    ))
}

pub fn cleanup_fixture(fixture: &BuiltReleaseTrustFixture) {
    let _ = fs::remove_dir_all(&fixture.root);
}
