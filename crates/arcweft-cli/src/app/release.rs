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
        remote::{
            ReleaseObjectDirectoryBackend, ReleaseRemoteAwfrFinalization, ReleaseRemoteCredentials,
            ReleaseRemotePublishArtifact, ReleaseRemotePublishFailure, ReleaseRemotePublishPlan,
            ReleaseRemotePublishPolicy, ReleaseRemotePublishTarget,
            ReleaseRemoteSigningRequirements, dry_run_release_remote_publication,
            publish_release_to_remote,
        },
    },
};
use clap::{Args, Subcommand, ValueEnum};
use std::{env, fs, path::PathBuf, process::ExitCode};

#[derive(Debug, Subcommand)]
pub(in crate::app) enum ReleaseCommand {
    /// Publishes release artifacts into a local or remote-like destination.
    Publish(ReleasePublishOptions),
    /// Verifies AWFR release metadata and policy-derived payload states.
    Verify(ReleaseVerifyOptions),
}

#[derive(Clone, Debug, Args)]
pub(in crate::app) struct ReleasePublishOptions {
    /// Destination root for committed release files or object-directory objects.
    #[arg(long)]
    destination_root: PathBuf,
    /// Optional local staging root. Used only by the local backend.
    #[arg(long)]
    staging_root: Option<PathBuf>,
    /// Publication backend.
    #[arg(long, value_enum, default_value_t = CliReleasePublishBackend::Local)]
    backend: CliReleasePublishBackend,
    /// Build a remote publication plan without writing remote objects.
    #[arg(long)]
    dry_run: bool,
    /// Optional remote object key prefix for object-directory publication.
    #[arg(long)]
    remote_prefix: Option<String>,
    /// Maximum attempts for each remote backend operation.
    #[arg(long, default_value_t = 1)]
    retry_attempts: u8,
    /// Maximum total artifact bytes allowed in one remote publication.
    #[arg(long, default_value_t = u64::MAX)]
    byte_budget: u64,
    /// Optional remote publish timeout budget in milliseconds.
    #[arg(long)]
    timeout_millis: Option<u64>,
    /// Optional credential profile identifier recorded in redacted reports.
    #[arg(long)]
    credential_profile: Option<String>,
    /// Environment variable holding a backend secret; only redacted presence is reported.
    #[arg(long)]
    credential_secret_env: Option<String>,
    /// Require at least one signature artifact in the remote plan.
    #[arg(long)]
    require_signature_artifact: bool,
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
enum CliReleasePublishBackend {
    Local,
    ObjectDirectory,
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
    match options.backend {
        CliReleasePublishBackend::Local => release_publish_local_command(options),
        CliReleasePublishBackend::ObjectDirectory => release_publish_remote_command(options),
    }
}

fn release_publish_local_command(options: &ReleasePublishOptions) -> Result<(), ExitCode> {
    if options.dry_run {
        eprintln!("error: --dry-run is supported only with --backend object-directory");
        return Err(ExitCode::from(2));
    }
    let artifacts = read_publish_artifacts(options)?;
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

fn release_publish_remote_command(options: &ReleasePublishOptions) -> Result<(), ExitCode> {
    let plan = remote_publish_plan(options)?;
    if options.dry_run {
        let mut report = dry_run_release_remote_publication(&plan).map_err(|failure| {
            print_remote_publish_failure(options, &failure);
            ExitCode::FAILURE
        })?;
        "object_directory".clone_into(&mut report.backend);
        return print_remote_publish_report(options, &report);
    }

    let mut backend = ReleaseObjectDirectoryBackend::new(&options.destination_root);
    let report = publish_release_to_remote(&mut backend, &plan).map_err(|failure| {
        print_remote_publish_failure(options, &failure);
        ExitCode::FAILURE
    })?;
    print_remote_publish_report(options, &report)
}

fn print_remote_publish_report(
    options: &ReleasePublishOptions,
    report: &arcweft_project_loader::release_adapter::publish::remote::ReleaseRemotePublishReport,
) -> Result<(), ExitCode> {
    if options.json {
        print_json(report)
    } else {
        println!(
            "ok: {} {} release artifact(s) under {}",
            if options.dry_run {
                "planned"
            } else {
                "published"
            },
            report.artifacts.len(),
            options.destination_root.display()
        );
        Ok(())
    }
}

fn print_remote_publish_failure(
    options: &ReleasePublishOptions,
    failure: &ReleaseRemotePublishFailure,
) {
    if options.json {
        if let Err(code) = print_json(&failure.report) {
            eprintln!("error: failed to print remote publish report: {code:?}");
        }
    } else {
        eprintln!(
            "error: failed to publish release artifacts remotely: {}",
            failure.error
        );
    }
}

fn remote_publish_plan(
    options: &ReleasePublishOptions,
) -> Result<ReleaseRemotePublishPlan, ExitCode> {
    let artifacts = read_publish_artifacts(options)?
        .into_iter()
        .map(ReleaseRemotePublishArtifact::from_local_artifact)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            eprintln!("error: failed to build remote publish artifact: {error}");
            ExitCode::from(2)
        })?;
    let target =
        ReleaseRemotePublishTarget::new(options.remote_prefix.clone()).map_err(|error| {
            eprintln!("error: failed to build remote publish target: {error}");
            ExitCode::from(2)
        })?;
    let policy = ReleaseRemotePublishPolicy::new(
        options.retry_attempts,
        options.byte_budget,
        options.timeout_millis,
    )
    .map_err(|error| {
        eprintln!("error: failed to build remote publish policy: {error}");
        ExitCode::from(2)
    })?;
    let credentials = remote_credentials(options)?;
    Ok(ReleaseRemotePublishPlan::new(target, artifacts)
        .with_policy(policy)
        .with_signing_requirements(ReleaseRemoteSigningRequirements {
            require_signature_artifact: options.require_signature_artifact,
            require_awfr_signature_reference: false,
        })
        .with_awfr_finalization(ReleaseRemoteAwfrFinalization::default())
        .with_credentials(credentials))
}

fn remote_credentials(
    options: &ReleasePublishOptions,
) -> Result<ReleaseRemoteCredentials, ExitCode> {
    let secret = options
        .credential_secret_env
        .as_ref()
        .map(|variable| {
            env::var(variable).map_err(|error| {
                eprintln!("error: failed to read credential secret env {variable}: {error}");
                ExitCode::from(2)
            })
        })
        .transpose()?;
    Ok(ReleaseRemoteCredentials::new(
        options.credential_profile.clone(),
        secret,
    ))
}

fn read_publish_artifacts(
    options: &ReleasePublishOptions,
) -> Result<Vec<ReleasePublishArtifactBytes>, ExitCode> {
    options
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
        .collect::<Result<Vec<_>, ExitCode>>()
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
        print_json(&report)?;
        if report.success {
            Ok(())
        } else {
            Err(ExitCode::FAILURE)
        }
    } else {
        println!("policy: {:?}", report.policy_mode);
        println!("channel: {}", report.channel);
        println!("success: {}", report.success);
        println!("signing states: {}", report.signing.len());
        println!("payload states: {}", report.payloads.len());
        if report.success {
            Ok(())
        } else {
            Err(ExitCode::FAILURE)
        }
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
    let (kind, source_path, relative_path) = split_publish_artifact_spec(value)?;
    let kind = parse_publish_artifact_kind(kind)?;
    Ok(ReleasePublishArtifactArg {
        kind,
        source_path: PathBuf::from(source_path),
        relative_path: PathBuf::from(relative_path),
    })
}

fn split_publish_artifact_spec(value: &str) -> Result<(&str, &str, &str), String> {
    let (kind, artifact_paths) = value
        .split_once(':')
        .ok_or_else(|| "artifact spec must be kind:source_path:relative_publish_path".to_owned())?;
    let (source_path, relative_path) = artifact_paths
        .rsplit_once(':')
        .ok_or_else(|| "artifact spec must be kind:source_path:relative_publish_path".to_owned())?;
    if source_path.is_empty() || relative_path.is_empty() {
        return Err("artifact spec must be kind:source_path:relative_publish_path".to_owned());
    }
    Ok((kind, source_path, relative_path))
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

#[cfg(test)]
mod tests {
    use super::{
        ReleasePublishArtifactKind, parse_publish_artifact_arg, split_publish_artifact_spec,
    };
    use std::path::PathBuf;

    #[test]
    fn publish_artifact_parser_accepts_windows_absolute_source_path() {
        let artifact =
            parse_publish_artifact_arg(r"awfb:C:\fixtures\base.awfb:artifacts/base.awfb")
                .expect("artifact parses");

        assert_eq!(artifact.kind, ReleasePublishArtifactKind::AwfbBundle);
        assert_eq!(
            artifact.source_path,
            PathBuf::from(r"C:\fixtures\base.awfb")
        );
        assert_eq!(artifact.relative_path, PathBuf::from("artifacts/base.awfb"));
    }

    #[test]
    fn publish_artifact_spec_requires_source_and_destination() {
        assert!(split_publish_artifact_spec("awfb").is_err());
        assert!(split_publish_artifact_spec("awfb:").is_err());
        assert!(split_publish_artifact_spec("awfb:source.awfb:").is_err());
    }
}
