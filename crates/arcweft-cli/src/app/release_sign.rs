use super::shared::print_json;
use arcweft_bundle::{
    container::{BundleView, ReadBudget, append_signature_block},
    release::{
        RELEASE_SIGNATURE_ALGORITHM_ED25519_V1, ReleaseSignatureEnvelope, ReleaseSignaturePolicy,
        ReleaseTrustedPublicKey,
    },
};
use clap::Args;
use ed25519_dalek::Signer as _;
use std::{fs, path::PathBuf, process::ExitCode};

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct SignBundleOptions {
    /// Unsigned AWFB bundle to sign.
    #[arg(long)]
    input: PathBuf,
    /// Signed AWFB output path.
    #[arg(short, long)]
    output: PathBuf,
    /// Release signer id recorded in the signature envelope.
    #[arg(long)]
    signer_id: String,
    /// 32-byte Ed25519 signing key as hex. Prefer --signing-key-file outside tests.
    #[arg(
        long,
        conflicts_with = "signing_key_file",
        required_unless_present = "signing_key_file"
    )]
    signing_key_hex: Option<String>,
    /// File containing the 32-byte Ed25519 signing key as hex.
    #[arg(long, conflicts_with = "signing_key_hex")]
    signing_key_file: Option<PathBuf>,
    /// Deterministic release key epoch used for rotation policy.
    #[arg(long, default_value_t = 0)]
    key_epoch: u64,
    /// Print a JSON report.
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
struct SignBundleReport {
    input: String,
    output: String,
    signer_id: String,
    algorithm: String,
    key_epoch: u64,
    kind: String,
    content_root: String,
    signing_digest: String,
    signature_bytes: u64,
    public_key: String,
}

struct SignedAwfb {
    bytes: Vec<u8>,
    report: SignBundleReport,
}

pub(super) fn sign_bundle_command(options: &SignBundleOptions) -> Result<(), ExitCode> {
    let bytes = fs::read(&options.input).map_err(|error| {
        eprintln!(
            "error: failed to read AWFB bundle {}: {error}",
            options.input.display()
        );
        ExitCode::FAILURE
    })?;
    let signing_key_bytes = read_signing_key(options)?;
    let signed = sign_awfb_bytes(
        &bytes,
        &options.input.display().to_string(),
        &options.output.display().to_string(),
        &options.signer_id,
        options.key_epoch,
        signing_key_bytes,
    )
    .map_err(|message| {
        eprintln!("error: failed to sign AWFB bundle: {message}");
        ExitCode::FAILURE
    })?;
    if let Some(parent) = options.output.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            eprintln!(
                "error: failed to create output directory {}: {error}",
                parent.display()
            );
            ExitCode::FAILURE
        })?;
    }
    fs::write(&options.output, &signed.bytes).map_err(|error| {
        eprintln!(
            "error: failed to write signed AWFB bundle {}: {error}",
            options.output.display()
        );
        ExitCode::FAILURE
    })?;
    if options.json {
        print_json(&signed.report)
    } else {
        println!(
            "ok: signed {} -> {} (signer={}, key_epoch={}, content_root={})",
            options.input.display(),
            options.output.display(),
            signed.report.signer_id,
            signed.report.key_epoch,
            signed.report.content_root
        );
        Ok(())
    }
}

fn read_signing_key(options: &SignBundleOptions) -> Result<[u8; 32], ExitCode> {
    let key_hex = if let Some(key_hex) = &options.signing_key_hex {
        key_hex.clone()
    } else if let Some(path) = &options.signing_key_file {
        fs::read_to_string(path).map_err(|error| {
            eprintln!(
                "error: failed to read signing key file {}: {error}",
                path.display()
            );
            ExitCode::FAILURE
        })?
    } else {
        eprintln!("error: either --signing-key-hex or --signing-key-file is required");
        return Err(ExitCode::FAILURE);
    };
    decode_hex_array::<32>(&key_hex).map_err(|message| {
        eprintln!("error: invalid Ed25519 signing key: {message}");
        ExitCode::FAILURE
    })
}

fn sign_awfb_bytes(
    bytes: &[u8],
    input: &str,
    output: &str,
    signer_id: &str,
    key_epoch: u64,
    signing_key_bytes: [u8; 32],
) -> Result<SignedAwfb, String> {
    let view =
        BundleView::parse(bytes, ReadBudget::default()).map_err(|error| error.to_string())?;
    if view.signature().is_some() {
        return Err("input AWFB already has a signature block".to_owned());
    }
    let signing_digest = view.signing_digest().map_err(|error| error.to_string())?;
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&signing_key_bytes);
    let mut envelope = ReleaseSignatureEnvelope::new(
        signer_id,
        RELEASE_SIGNATURE_ALGORITHM_ED25519_V1,
        view.content_root(),
        view.kind(),
        signing_digest,
        encode_hex(&[0; 64]),
    )
    .map_err(|error| error.to_string())?;
    envelope.key_epoch = key_epoch;
    let signature = signing_key.sign(&envelope.signing_message());
    envelope.signature = encode_hex(&signature.to_bytes());
    let envelope_bytes = envelope
        .to_json_bytes()
        .map_err(|error| error.to_string())?;
    let signed_bytes =
        append_signature_block(bytes, &envelope_bytes).map_err(|error| error.to_string())?;

    let trusted_key = ReleaseTrustedPublicKey::ed25519_v1(
        signer_id,
        encode_hex(&signing_key.verifying_key().to_bytes()),
    )
    .map_err(|error| error.to_string())?
    .with_key_epoch_validity(key_epoch, None)
    .map_err(|error| error.to_string())?;
    ReleaseSignaturePolicy::require_trusted_public_keys(Some(64), [trusted_key])
        .map_err(|error| error.to_string())?
        .verify_awfb_bytes(view.content_root(), &signed_bytes)
        .map_err(|error| error.to_string())?;

    Ok(SignedAwfb {
        bytes: signed_bytes,
        report: SignBundleReport {
            input: input.to_owned(),
            output: output.to_owned(),
            signer_id: signer_id.to_owned(),
            algorithm: RELEASE_SIGNATURE_ALGORITHM_ED25519_V1.to_owned(),
            key_epoch,
            kind: format!("{:?}", view.kind()),
            content_root: view.content_root().to_string(),
            signing_digest: signing_digest.to_string(),
            signature_bytes: u64::try_from(envelope_bytes.len()).unwrap_or(u64::MAX),
            public_key: encode_hex(&signing_key.verifying_key().to_bytes()),
        },
    })
}

fn decode_hex_array<const N: usize>(value: &str) -> Result<[u8; N], String> {
    let value = value.strip_prefix("ed25519:").unwrap_or(value).trim();
    if value.len() != N * 2 {
        return Err(format!(
            "expected {} hex characters, got {}",
            N * 2,
            value.len()
        ));
    }
    let mut bytes = [0_u8; N];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let hex = std::str::from_utf8(chunk).map_err(|error| error.to_string())?;
        bytes[index] = u8::from_str_radix(hex, 16)
            .map_err(|_| format!("invalid hex byte `{hex}` at offset {}", index * 2))?;
    }
    Ok(bytes)
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

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::{
        container::{
            BundleKind, BundleSectionKind, ContentResidency, SectionId, SectionInput, encode_bundle,
        },
        release::ReleaseSignaturePolicy,
    };

    #[test]
    fn sign_awfb_bytes_appends_verifiable_release_signature() {
        let bytes = content_pack(b"voice");
        let signing_key_bytes = [7; 32];
        let signed = sign_awfb_bytes(
            &bytes,
            "content.awfb",
            "content.signed.awfb",
            "release-key-main",
            4,
            signing_key_bytes,
        )
        .expect("bundle signs");
        let view = BundleView::parse(&signed.bytes, ReadBudget::default()).expect("signed parses");
        assert!(view.signature().is_some());
        assert_eq!(signed.report.key_epoch, 4);

        let signing_key = ed25519_dalek::SigningKey::from_bytes(&signing_key_bytes);
        let trusted_key = ReleaseTrustedPublicKey::ed25519_v1(
            "release-key-main",
            encode_hex(&signing_key.verifying_key().to_bytes()),
        )
        .expect("trusted public key")
        .with_key_epoch_validity(4, None)
        .expect("trusted validity");
        ReleaseSignaturePolicy::require_trusted_public_keys(Some(64), [trusted_key])
            .expect("policy")
            .verify_awfb_bytes(view.content_root(), &signed.bytes)
            .expect("signature verifies");
    }

    fn content_pack(bytes: &'static [u8]) -> Vec<u8> {
        encode_bundle(
            BundleKind::ContentPack,
            br#"{"kind":"content"}"#,
            vec![SectionInput::embedded(
                SectionId::from_bytes([9; 16]),
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                bytes,
            )],
        )
        .expect("content pack encodes")
    }
}
