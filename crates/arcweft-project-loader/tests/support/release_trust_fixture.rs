use arcweft_bundle::{
    container::{
        ArtifactIdentity, BundleDigest, BundleKind, BundleSectionKind, BundleView,
        ContentResidency, ReadBudget, SectionId, SectionInput, append_signature_block,
        encode_bundle,
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
        signing_policy::{KeyEpochPolicy, SigningDigestTranscript, SigningPolicy},
    },
};
use arcweft_project_loader::cache::store::FilesystemCacheStore;
use ed25519_dalek::Signer as _;
use std::{
    fs,
    io::{Read, Write},
    net::{Shutdown, TcpListener},
    path::{Path, PathBuf},
    thread,
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
    SuccessMetadataOnly,
    SuccessFileMirror,
    MissingBaseSignature,
    MissingPatchSignature,
    PatchTargetIdentityMismatch,
    MaterializedTargetDigestMismatch,
    MissingTargetSignature,
    ExternalPayloadDigestMismatch,
    ExternalPayloadSizeMismatch,
    ExternalPayloadMissing,
    AwfrManifestTamper,
    DetachedSignatureTranscriptMismatch,
}

pub struct BuiltReleaseTrustFixture {
    pub root: PathBuf,
    pub archive_path: PathBuf,
    pub cache_root: PathBuf,
    pub expected_code: Option<&'static str>,
}

struct SignedReleaseTrustArtifacts {
    target_unsigned: Vec<u8>,
    base_bytes: Vec<u8>,
    target_bytes: Vec<u8>,
    patch_bytes: Vec<u8>,
    target_content_root: BundleDigest,
    target_artifact: ArtifactIdentity,
    wrong_target_artifact: ArtifactIdentity,
}

pub fn release_consume_policy() -> SigningPolicy {
    SigningPolicy::release_consume(
        ReleaseChannel::new(CHANNEL).expect("fixture channel"),
        KeyEpochPolicy {
            min: KEY_EPOCH,
            max: Some(KEY_EPOCH + 1),
        },
    )
}

pub fn wrong_channel_policy() -> SigningPolicy {
    SigningPolicy::release_consume(
        ReleaseChannel::new("seq02-9-wrong-channel").expect("wrong fixture channel"),
        KeyEpochPolicy {
            min: KEY_EPOCH,
            max: Some(KEY_EPOCH + 1),
        },
    )
}

pub fn build_release_trust_fixture(case: ReleaseTrustCase) -> BuiltReleaseTrustFixture {
    let root = temp_root(case.label());
    let artifacts = root.join("artifacts");
    let cache_root = root.join("cache");
    fs::create_dir_all(&artifacts).expect("fixture artifact dir");

    let signed = signed_fixture_artifacts(case);
    write_awfb_artifacts(&artifacts, &signed);
    write_external_payload_fixture(&artifacts, case);
    seed_cache_hit_fixture(&cache_root, case);

    let target_view_for_carrier = BundleView::parse(&signed.target_unsigned, ReadBudget::default())
        .expect("target carrier view");
    let carrier = external_payload_carrier(&target_view_for_carrier, payload_mirrors(case));
    let release_manifest = release_manifest_for_case(case, &signed);
    let patch_ref = patch_ref_for_case(case, &signed);
    let mut archive = release_archive(case, release_manifest, carrier, patch_ref);
    apply_archive_mutation(case, &mut archive);

    let archive_path = root.join("game.awfr");
    fs::write(
        &archive_path,
        archive.to_json_bytes().expect("archive JSON"),
    )
    .expect("archive writes");

    BuiltReleaseTrustFixture {
        root,
        archive_path,
        cache_root,
        expected_code: case.expected_code(),
    }
}

fn signed_fixture_artifacts(case: ReleaseTrustCase) -> SignedReleaseTrustArtifacts {
    let base_unsigned = content_pack(b"base embedded payload", EXTERNAL_PAYLOAD, true);
    let target_unsigned = content_pack(b"target embedded payload", EXTERNAL_PAYLOAD, true);
    let wrong_target_unsigned =
        content_pack(b"wrong target embedded payload", EXTERNAL_PAYLOAD, true);

    let base_bytes = if case == ReleaseTrustCase::MissingBaseSignature {
        base_unsigned.clone()
    } else {
        sign_awfb_bytes(&base_unsigned)
    };
    let target_bytes = if case == ReleaseTrustCase::MissingTargetSignature {
        target_unsigned.clone()
    } else if case == ReleaseTrustCase::MaterializedTargetDigestMismatch {
        sign_awfb_bytes(&wrong_target_unsigned)
    } else {
        sign_awfb_bytes(&target_unsigned)
    };

    let base_unsigned_view =
        BundleView::parse(&base_unsigned, ReadBudget::default()).expect("base unsigned parses");
    let target_unsigned_view =
        BundleView::parse(&target_unsigned, ReadBudget::default()).expect("target unsigned parses");
    let wrong_target_view = BundleView::parse(&wrong_target_unsigned, ReadBudget::default())
        .expect("wrong target parses");
    let patch_artifact =
        BundlePatchArtifact::from_views(&base_unsigned_view, &target_unsigned_view)
            .expect("patch artifact");
    let patch_unsigned = encode_patch_bundle(&patch_artifact).expect("patch encodes");
    let patch_bytes = if case == ReleaseTrustCase::MissingPatchSignature {
        patch_unsigned.clone()
    } else {
        sign_awfb_bytes(&patch_unsigned)
    };
    let target_content_root = target_unsigned_view.content_root();
    let target_artifact = target_unsigned_view.artifact_identity();
    let wrong_target_artifact = wrong_target_view.artifact_identity();

    SignedReleaseTrustArtifacts {
        target_unsigned,
        base_bytes,
        target_bytes,
        patch_bytes,
        target_content_root,
        target_artifact,
        wrong_target_artifact,
    }
}

fn write_awfb_artifacts(artifacts: &Path, signed: &SignedReleaseTrustArtifacts) {
    fs::write(artifacts.join("base.awfb"), &signed.base_bytes).expect("base writes");
    fs::write(artifacts.join("patch.awfb"), &signed.patch_bytes).expect("patch writes");
    fs::write(artifacts.join("target.awfb"), &signed.target_bytes).expect("target writes");
}

fn write_external_payload_fixture(artifacts: &Path, case: ReleaseTrustCase) {
    let payload_path = artifacts.join("payload.bin");
    match case {
        ReleaseTrustCase::ExternalPayloadDigestMismatch => {
            fs::write(&payload_path, b"seq02.9 deterministic external paylord")
                .expect("bad payload writes");
        }
        ReleaseTrustCase::ExternalPayloadSizeMismatch => {
            fs::write(&payload_path, b"short").expect("short payload writes");
        }
        ReleaseTrustCase::ExternalPayloadMissing => {}
        _ => fs::write(&payload_path, EXTERNAL_PAYLOAD).expect("payload writes"),
    }
}

fn seed_cache_hit_fixture(cache_root: &Path, case: ReleaseTrustCase) {
    if case == ReleaseTrustCase::SuccessCacheHit {
        FilesystemCacheStore::new(cache_root)
            .put_object(EXTERNAL_PAYLOAD)
            .expect("cache object seeded");
    }
}

fn release_manifest_for_case(
    case: ReleaseTrustCase,
    signed: &SignedReleaseTrustArtifacts,
) -> ReleaseManifest {
    let base_ref = ReleaseBundleRef::from_awfb_bytes(
        &signed.base_bytes,
        [ReleaseMirror::new("file:artifacts/base.awfb").expect("base mirror")],
    )
    .expect("base ref");
    let target_ref = if case == ReleaseTrustCase::MaterializedTargetDigestMismatch {
        ReleaseBundleRef::new(
            signed.target_content_root,
            BundleDigest::of(&signed.target_bytes),
            signed.target_bytes.len() as u64,
            BundleKind::ContentPack,
            [ReleaseMirror::new("file:artifacts/target.awfb").expect("target mirror")],
        )
        .expect("tampered target ref")
    } else {
        ReleaseBundleRef::from_awfb_bytes(
            &signed.target_bytes,
            [ReleaseMirror::new("file:artifacts/target.awfb").expect("target mirror")],
        )
        .expect("target ref")
    };
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
    release_manifest
}

fn patch_ref_for_case(
    case: ReleaseTrustCase,
    signed: &SignedReleaseTrustArtifacts,
) -> AwfrPatchArtifactRef {
    let patch_view =
        BundleView::parse(&signed.patch_bytes, ReadBudget::default()).expect("patch view");
    let patch_target_artifact = if case == ReleaseTrustCase::PatchTargetIdentityMismatch {
        signed.wrong_target_artifact
    } else {
        signed.target_artifact
    };
    AwfrPatchArtifactRef {
        patch_artifact: patch_view.artifact_identity(),
        target_artifact: patch_target_artifact,
        file_digest: BundleDigest::of(&signed.patch_bytes),
        byte_len: signed.patch_bytes.len() as u64,
        mirrors: vec![ReleaseMirror::new("file:artifacts/patch.awfb").expect("patch mirror")],
    }
}

fn release_archive(
    case: ReleaseTrustCase,
    release_manifest: ReleaseManifest,
    carrier: ExternalPayloadCarrier,
    patch_ref: AwfrPatchArtifactRef,
) -> AwfrArchiveManifest {
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
    archive
}

fn apply_archive_mutation(case: ReleaseTrustCase, archive: &mut AwfrArchiveManifest) {
    if case == ReleaseTrustCase::AwfrManifestTamper {
        archive.publication.as_mut().expect("publication").sequence += 1;
    }
    if case == ReleaseTrustCase::DetachedSignatureTranscriptMismatch {
        archive.signatures[0].signing_digest = BundleDigest::of(b"wrong detached transcript");
    }
}

pub fn spawn_http_payload_server(body: Vec<u8>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture HTTP listener binds");
    let addr = listener.local_addr().expect("fixture HTTP local addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("fixture HTTP accepts request");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 256];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream
                .read(&mut buffer)
                .expect("fixture HTTP request reads");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
        }
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .expect("fixture HTTP headers write");
        stream.write_all(&body).expect("fixture HTTP body writes");
        stream.flush().expect("fixture HTTP response flushes");
        stream
            .shutdown(Shutdown::Write)
            .expect("fixture HTTP response shuts down");
    });
    (format!("http://{addr}/payload.bin"), handle)
}

impl ReleaseTrustCase {
    fn label(self) -> &'static str {
        match self {
            Self::SuccessCacheHit => "success-cache-hit",
            Self::SuccessMetadataOnly => "success-metadata-only",
            Self::SuccessFileMirror => "success-file-mirror",
            Self::MissingBaseSignature => "missing-base-signature",
            Self::MissingPatchSignature => "missing-patch-signature",
            Self::PatchTargetIdentityMismatch => "patch-target-identity-mismatch",
            Self::MaterializedTargetDigestMismatch => "materialized-target-digest-mismatch",
            Self::MissingTargetSignature => "missing-target-signature",
            Self::ExternalPayloadDigestMismatch => "external-payload-digest-mismatch",
            Self::ExternalPayloadSizeMismatch => "external-payload-size-mismatch",
            Self::ExternalPayloadMissing => "external-payload-missing",
            Self::AwfrManifestTamper => "awfr-manifest-tamper",
            Self::DetachedSignatureTranscriptMismatch => "detached-signature-transcript-mismatch",
        }
    }

    fn expected_code(self) -> Option<&'static str> {
        match self {
            Self::SuccessCacheHit | Self::SuccessMetadataOnly | Self::SuccessFileMirror => None,
            Self::MissingBaseSignature => Some("missing_base_signature"),
            Self::MissingPatchSignature => Some("missing_patch_signature"),
            Self::PatchTargetIdentityMismatch => Some("patch_target_identity_mismatch"),
            Self::MaterializedTargetDigestMismatch => Some("materialized_target_digest_mismatch"),
            Self::MissingTargetSignature => Some("missing_materialized_target_signature"),
            Self::ExternalPayloadDigestMismatch => Some("external_payload_digest_mismatch"),
            Self::ExternalPayloadSizeMismatch => Some("external_payload_size_mismatch"),
            Self::ExternalPayloadMissing => Some("external_payload_missing"),
            Self::AwfrManifestTamper | Self::DetachedSignatureTranscriptMismatch => {
                Some("detached_signature_transcript_mismatch")
            }
        }
    }
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

pub fn fixture_payload() -> &'static [u8] {
    EXTERNAL_PAYLOAD
}

pub fn replace_payload_mirror_with_http(archive_path: &Path, uri: String) {
    let bytes = fs::read(archive_path).expect("archive reads for HTTP rewrite");
    let mut archive =
        AwfrArchiveManifest::from_json_slice(&bytes).expect("archive decodes for HTTP rewrite");
    archive.external_payloads[0].mirrors = vec![ReleaseMirror::new(uri).expect("HTTP mirror")];
    archive.signatures.clear();
    sign_awfr_archive(&mut archive);
    fs::write(
        archive_path,
        archive.to_json_bytes().expect("archive rewrites"),
    )
    .expect("archive writes for HTTP rewrite");
}
