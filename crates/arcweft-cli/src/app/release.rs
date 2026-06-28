use super::shared::print_json;
use arcweft_bundle::release::{
    archive::{ExternalPayloadMaterializationMode, ReleaseChannel},
    signing_policy::{KeyEpochPolicy, SigningPolicy},
};
use arcweft_project_loader::release_adapter::{
    consume::verify_release_archive,
    publish::{
        ReleasePublishArtifactBytes, ReleasePublishArtifactKind, ReleasePublishPlan,
        publish_release_atomically,
    },
};
use clap::{Args, Subcommand, ValueEnum};
use std::{fs, path::PathBuf, process::ExitCode};

#[derive(Debug, Subcommand)]
pub(in crate::app) enum ReleaseCommand {
    /// Atomically stages release artifacts into a local destination root.
    Publish(ReleasePublishOptions),
    /// Verifies AWFR release metadata and policy-derived payload states.
    Verify(ReleaseVerifyOptions),
}

#[derive(Clone, Debug, Args)]
pub(in crate::app) struct ReleasePublishOptions {
    /// Destination root for committed release files.
    #[arg(long)]
    destination_root: PathBuf,
    /// Optional staging root. Defaults under destination root.
    #[arg(long)]
    staging_root: Option<PathBuf>,
    /// Artifact spec in the form `kind:source_path:relative_publish_path`.
    #[arg(long = "artifact", value_parser = parse_publish_artifact_arg)]
    artifacts: Vec<ReleasePublishArtifactArg>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Debug, Args)]
pub(in crate::app) struct ReleaseVerifyOptions {
    /// AWFR archive path.
    #[arg(long)]
    archive: PathBuf,
    /// Cache root used when payload mode requires external bytes.
    #[arg(long, default_value = "target/arcweft/cache/v1")]
    cache_root: PathBuf,
    /// Signing policy mode.
    #[arg(long, value_enum, default_value_t = CliSigningPolicyMode::OfflineInspection)]
    policy: CliSigningPolicyMode,
    /// Expected release channel.
    #[arg(long, default_value = "local-dev")]
    channel: String,
    /// Inclusive minimum accepted key epoch.
    #[arg(long, default_value_t = 0)]
    key_epoch_min: u64,
    /// Exclusive maximum accepted key epoch.
    #[arg(long)]
    key_epoch_max: Option<u64>,
    /// External payload materialization mode.
    #[arg(long, value_enum, default_value_t = CliExternalPayloadMaterializationMode::MetadataOnly)]
    payload_mode: CliExternalPayloadMaterializationMode,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Debug)]
struct ReleasePublishArtifactArg {
    kind: ReleasePublishArtifactKind,
    source_path: PathBuf,
    relative_path: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CliSigningPolicyMode {
    LocalDev,
    Ci,
    ReleasePublish,
    ReleaseConsume,
    OfflineInspection,
    TestFixture,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CliExternalPayloadMaterializationMode {
    MetadataOnly,
    RequiredResidency,
    AllPayloads,
}

pub(super) fn release_command(command: ReleaseCommand) -> Result<(), ExitCode> {
    match command {
        ReleaseCommand::Publish(options) => release_publish_command(&options),
        ReleaseCommand::Verify(options) => release_verify_command(&options),
    }
}

fn release_publish_command(options: &ReleasePublishOptions) -> Result<(), ExitCode> {
    let artifacts = options
        .artifacts
        .iter()
        .map(|artifact| {
            let bytes = fs::read(&artifact.source_path).map_err(|error| {
                eprintln!(
                    "error: failed to read release artifact {}: {error}",
                    artifact.source_path.display()
                );
                ExitCode::FAILURE
            })?;
            Ok(ReleasePublishArtifactBytes {
                kind: artifact.kind,
                relative_path: artifact.relative_path.clone(),
                bytes,
            })
        })
        .collect::<Result<Vec<_>, ExitCode>>()?;
    let report = publish_release_atomically(&ReleasePublishPlan {
        destination_root: options.destination_root.clone(),
        staging_root: options.staging_root.clone(),
        artifacts,
    })
    .map_err(|error| {
        eprintln!("error: failed to publish release artifacts: {error}");
        ExitCode::FAILURE
    })?;

    if options.json {
        print_json(&report)
    } else {
        println!(
            "ok: published {} release artifact(s) under {}",
            report.artifacts.len(),
            options.destination_root.display()
        );
        Ok(())
    }
}

fn release_verify_command(options: &ReleaseVerifyOptions) -> Result<(), ExitCode> {
    let policy = signing_policy_for_options(options).map_err(|message| {
        eprintln!("error: {message}");
        ExitCode::from(2)
    })?;
    let report = verify_release_archive(
        &options.archive,
        &policy,
        &options.cache_root,
        options.payload_mode.into(),
    )
    .map_err(|error| {
        eprintln!("error: failed to verify release archive: {error}");
        ExitCode::FAILURE
    })?;

    if options.json {
        print_json(&report)
    } else {
        println!("policy: {:?}", report.policy_mode);
        println!("channel: {}", report.channel);
        println!("signing states: {}", report.signing.len());
        println!("payload states: {}", report.payloads.len());
        Ok(())
    }
}

fn signing_policy_for_options(options: &ReleaseVerifyOptions) -> Result<SigningPolicy, String> {
    let channel = ReleaseChannel::new(&options.channel).map_err(|error| error.to_string())?;
    let key_epoch = KeyEpochPolicy {
        min: options.key_epoch_min,
        max: options.key_epoch_max,
    };
    let policy = match options.policy {
        CliSigningPolicyMode::LocalDev => SigningPolicy::local_dev(channel),
        CliSigningPolicyMode::Ci => SigningPolicy::ci(channel, key_epoch),
        CliSigningPolicyMode::ReleasePublish => SigningPolicy::release_publish(channel, key_epoch),
        CliSigningPolicyMode::ReleaseConsume => SigningPolicy::release_consume(channel, key_epoch),
        CliSigningPolicyMode::OfflineInspection => SigningPolicy::offline_inspection(channel),
        CliSigningPolicyMode::TestFixture => SigningPolicy::test_fixture(channel),
    };
    policy.validate().map_err(|error| error.to_string())?;
    Ok(policy)
}

fn parse_publish_artifact_arg(value: &str) -> Result<ReleasePublishArtifactArg, String> {
    let mut parts = value.splitn(3, ':');
    let kind = parse_publish_artifact_kind(parts.next().unwrap_or_default())?;
    let source_path = parts
        .next()
        .ok_or_else(|| "artifact spec must be kind:source_path:relative_publish_path".to_owned())?;
    let relative_path = parts
        .next()
        .ok_or_else(|| "artifact spec must be kind:source_path:relative_publish_path".to_owned())?;
    Ok(ReleasePublishArtifactArg {
        kind,
        source_path: PathBuf::from(source_path),
        relative_path: PathBuf::from(relative_path),
    })
}

fn parse_publish_artifact_kind(value: &str) -> Result<ReleasePublishArtifactKind, String> {
    match value {
        "awfb" | "awfb_bundle" => Ok(ReleasePublishArtifactKind::AwfbBundle),
        "patch" | "patch_artifact" => Ok(ReleasePublishArtifactKind::PatchArtifact),
        "external_payload" | "payload" => Ok(ReleasePublishArtifactKind::ExternalPayload),
        "signature" | "sig" => Ok(ReleasePublishArtifactKind::Signature),
        "awfr" | "awfr_archive" => Ok(ReleasePublishArtifactKind::AwfrArchive),
        _ => Err(format!("unknown release artifact kind `{value}`")),
    }
}

impl From<CliExternalPayloadMaterializationMode> for ExternalPayloadMaterializationMode {
    fn from(value: CliExternalPayloadMaterializationMode) -> Self {
        match value {
            CliExternalPayloadMaterializationMode::MetadataOnly => Self::MetadataOnly,
            CliExternalPayloadMaterializationMode::RequiredResidency => Self::RequiredResidency,
            CliExternalPayloadMaterializationMode::AllPayloads => Self::AllPayloads,
        }
    }
}
