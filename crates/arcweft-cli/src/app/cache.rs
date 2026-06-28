use arcweft_bundle::container::{BundleDigest, SectionId};
use arcweft_project_loader::cache::external_payload::fetch_external_payload_to_cache;
use arcweft_project_loader::cache::inspect::{
    CacheExplainStatus, CacheVerifyStatus, cache_stats, explain_cache,
    explain_cache_by_logical_item, prune_cache, verify_cache,
};
use arcweft_project_loader::cache::release::fetch_release_bundle_to_cache;
use clap::{Args, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Subcommand)]
pub(super) enum CacheCommand {
    /// Reports filesystem cache inventory counts.
    Stats(CacheOptions),
    /// Verifies object digests and cache record references.
    Verify(CacheOptions),
    /// Explains cache entries for one artifact key, object digest, or logical item.
    Explain(CacheExplainOptions),
    /// Removes safe cache garbage; dry-run unless --apply is provided.
    Prune(CachePruneOptions),
    /// Fetches one release-manifest bundle into the local cache.
    Fetch(CacheFetchOptions),
    /// Fetches one AWFR external payload into the local cache.
    FetchExternal(CacheFetchExternalOptions),
}

#[derive(Clone, Debug, Args)]
pub(super) struct CacheOptions {
    /// Cache root directory.
    #[arg(long, default_value = "target/arcweft/cache/v1")]
    root: PathBuf,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Debug, Args)]
pub(super) struct CacheExplainOptions {
    /// Artifact key digest, object digest, or logical item when --logical is set.
    query: String,
    /// Interpret the query as a logical cache item label.
    #[arg(long)]
    logical: bool,
    /// Cache root directory.
    #[arg(long, default_value = "target/arcweft/cache/v1")]
    root: PathBuf,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Debug, Args)]
pub(super) struct CachePruneOptions {
    /// Cache root directory.
    #[arg(long, default_value = "target/arcweft/cache/v1")]
    root: PathBuf,
    /// Apply removals. Without this flag, prune only reports candidates.
    #[arg(long)]
    apply: bool,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Debug, Args)]
pub(super) struct CacheFetchOptions {
    /// Release manifest (.awfr) path.
    #[arg(long)]
    manifest: PathBuf,
    /// Content root digest to fetch.
    #[arg(long)]
    content_root: String,
    /// Cache root directory.
    #[arg(long, default_value = "target/arcweft/cache/v1")]
    root: PathBuf,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Debug, Args)]
pub(super) struct CacheFetchExternalOptions {
    /// AWFR archive path.
    #[arg(long)]
    archive: PathBuf,
    /// Bundle content root digest that owns the external section descriptor.
    #[arg(long)]
    bundle_content_root: String,
    /// External section descriptor id as 32 lowercase hexadecimal characters.
    #[arg(long)]
    descriptor_id: String,
    /// Cache root directory.
    #[arg(long, default_value = "target/arcweft/cache/v1")]
    root: PathBuf,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

pub(super) fn cache_command(command: CacheCommand) -> Result<(), ExitCode> {
    match command {
        CacheCommand::Stats(options) => cache_stats_command(&options),
        CacheCommand::Verify(options) => cache_verify_command(&options),
        CacheCommand::Explain(options) => cache_explain_command(&options),
        CacheCommand::Prune(options) => cache_prune_command(&options),
        CacheCommand::Fetch(options) => cache_fetch_command(&options),
        CacheCommand::FetchExternal(options) => cache_fetch_external_command(&options),
    }
}

fn cache_stats_command(options: &CacheOptions) -> Result<(), ExitCode> {
    let stats = cache_stats(&options.root).map_err(|error| {
        eprintln!("error: failed to inspect cache: {error}");
        ExitCode::FAILURE
    })?;
    if options.json {
        serde_json::to_writer_pretty(std::io::stdout(), &stats).map_err(|error| {
            eprintln!("error: failed to write cache stats JSON: {error}");
            ExitCode::FAILURE
        })?;
        println!();
    } else {
        println!("root: {}", stats.root);
        println!(
            "objects: {} file(s), {} byte(s)",
            stats.object_files, stats.object_bytes
        );
        println!(
            "records: {} file(s), {} byte(s)",
            stats.record_files, stats.record_bytes
        );
        println!("locks: {}", stats.lock_files);
        println!("temp files: {}", stats.temp_files);
        println!("other files: {}", stats.other_files);
    }
    Ok(())
}

fn cache_verify_command(options: &CacheOptions) -> Result<(), ExitCode> {
    let report = verify_cache(&options.root).map_err(|error| {
        eprintln!("error: failed to verify cache: {error}");
        ExitCode::FAILURE
    })?;
    if options.json {
        serde_json::to_writer_pretty(std::io::stdout(), &report).map_err(|error| {
            eprintln!("error: failed to write cache verify JSON: {error}");
            ExitCode::FAILURE
        })?;
        println!();
    } else {
        println!("status: {:?}", report.status);
        println!(
            "objects: {} file(s), records: {} file(s), issues: {}",
            report.stats.object_files,
            report.stats.record_files,
            report.issues.len()
        );
        for issue in &report.issues {
            println!("- {:?}: {} ({})", issue.kind, issue.path, issue.message);
        }
    }
    if report.status == CacheVerifyStatus::Ok {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

fn cache_explain_command(options: &CacheExplainOptions) -> Result<(), ExitCode> {
    let report = if options.logical {
        explain_cache_by_logical_item(&options.root, &options.query)
    } else {
        explain_cache(&options.root, &options.query)
    }
    .map_err(|error| {
        eprintln!("error: failed to explain cache entry: {error}");
        ExitCode::FAILURE
    })?;
    if options.json {
        serde_json::to_writer_pretty(std::io::stdout(), &report).map_err(|error| {
            eprintln!("error: failed to write cache explain JSON: {error}");
            ExitCode::FAILURE
        })?;
        println!();
    } else {
        println!("status: {:?}", report.status);
        println!("query: {}", report.query);
        println!("matches: {}", report.matches.len());
        for item in &report.matches {
            println!("- {:?}: {}", item.kind, item.path);
            if let Some(artifact_kind) = &item.artifact_kind {
                println!("  artifact: {artifact_kind}");
            }
            if let Some(logical_item) = &item.logical_item {
                println!("  logical: {logical_item}");
            }
            if let Some(object_digest) = &item.object_digest {
                println!("  object: {object_digest}");
            }
            if let Some(object_status) = item.object_status {
                println!("  object status: {object_status:?}");
            }
        }
        for issue in &report.issues {
            println!("- {:?}: {} ({})", issue.kind, issue.path, issue.message);
        }
    }
    match report.status {
        CacheExplainStatus::Found => Ok(()),
        CacheExplainStatus::Missing | CacheExplainStatus::InvalidQuery => Err(ExitCode::FAILURE),
    }
}

fn cache_prune_command(options: &CachePruneOptions) -> Result<(), ExitCode> {
    let report = prune_cache(&options.root, options.apply).map_err(|error| {
        eprintln!("error: failed to prune cache: {error}");
        ExitCode::FAILURE
    })?;
    if options.json {
        serde_json::to_writer_pretty(std::io::stdout(), &report).map_err(|error| {
            eprintln!("error: failed to write cache prune JSON: {error}");
            ExitCode::FAILURE
        })?;
        println!();
    } else {
        println!("mode: {}", if report.applied { "apply" } else { "dry-run" });
        println!("candidates: {}", report.candidates.len());
        println!(
            "removed: {} file(s), {} dir(s), {} byte(s)",
            report.removed_files, report.removed_directories, report.removed_bytes
        );
        for candidate in &report.candidates {
            println!(
                "- {:?}: {} ({} byte(s))",
                candidate.kind, candidate.path, candidate.bytes
            );
        }
        for issue in &report.issues {
            println!("- {:?}: {} ({})", issue.kind, issue.path, issue.message);
        }
    }
    if report.issues.is_empty() {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

fn cache_fetch_command(options: &CacheFetchOptions) -> Result<(), ExitCode> {
    let content_root = parse_bundle_digest(&options.content_root).map_err(|message| {
        eprintln!("error: {message}");
        ExitCode::from(2)
    })?;
    let report = fetch_release_bundle_to_cache(&options.manifest, content_root, &options.root)
        .map_err(|error| {
            eprintln!("error: failed to fetch release bundle into cache: {error}");
            ExitCode::FAILURE
        })?;
    if options.json {
        serde_json::to_writer_pretty(std::io::stdout(), &report).map_err(|error| {
            eprintln!("error: failed to write cache fetch JSON: {error}");
            ExitCode::FAILURE
        })?;
        println!();
    } else {
        println!("status: {:?}", report.status);
        println!("content root: {}", report.content_root);
        println!("file digest: {}", report.file_digest);
        if let Some(uri) = &report.source_uri {
            println!("source: {uri}");
        }
        if let Some(key) = &report.record_key {
            println!("record key: {key}");
        }
    }
    Ok(())
}

fn cache_fetch_external_command(options: &CacheFetchExternalOptions) -> Result<(), ExitCode> {
    let bundle_content_root =
        parse_bundle_digest(&options.bundle_content_root).map_err(|message| {
            eprintln!("error: {message}");
            ExitCode::from(2)
        })?;
    let descriptor_id = parse_section_id(&options.descriptor_id).map_err(|message| {
        eprintln!("error: {message}");
        ExitCode::from(2)
    })?;
    let report = fetch_external_payload_to_cache(
        &options.archive,
        bundle_content_root,
        descriptor_id,
        &options.root,
    )
    .map_err(|error| {
        eprintln!("error: failed to fetch external payload into cache: {error}");
        ExitCode::FAILURE
    })?;
    if options.json {
        serde_json::to_writer_pretty(std::io::stdout(), &report).map_err(|error| {
            eprintln!("error: failed to write external payload fetch JSON: {error}");
            ExitCode::FAILURE
        })?;
        println!();
    } else {
        println!("status: {:?}", report.status);
        println!("bundle content root: {}", report.bundle_content_root);
        println!("descriptor id: {}", report.descriptor_id);
        println!("decoded digest: {}", report.decoded_digest);
        if let Some(uri) = &report.source_uri {
            println!("source: {uri}");
        }
        if let Some(key) = &report.record_key {
            println!("record key: {key}");
        }
    }
    Ok(())
}

fn parse_bundle_digest(value: &str) -> Result<BundleDigest, String> {
    parse_hex_array::<32>(value, "content root").map(BundleDigest::from_bytes)
}

fn parse_section_id(value: &str) -> Result<SectionId, String> {
    parse_hex_array::<16>(value, "descriptor id").map(SectionId::from_bytes)
}

fn parse_hex_array<const N: usize>(value: &str, label: &str) -> Result<[u8; N], String> {
    if value.len() != N * 2 {
        return Err(format!(
            "{label} must be a {}-character lowercase hexadecimal digest",
            N * 2
        ));
    }
    let mut bytes = [0_u8; N];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(chunk)
            .map_err(|_| format!("{label} digest must be valid UTF-8 hex"))?;
        bytes[index] = u8::from_str_radix(text, 16)
            .map_err(|_| format!("{label} digest must contain only hexadecimal digits"))?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::{
        container::{
            BundleKind, BundleSectionKind, BundleView, ContentResidency, ReadBudget, SectionInput,
            encode_bundle,
        },
        release::{
            ReleaseBundleRef, ReleaseManifest, ReleaseMirror,
            archive::{AwfrArchiveManifest, ExternalPayloadMediaType, ReleaseChannel},
        },
    };
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn missing_cache_root_has_empty_stats_and_verifies_ok() {
        let root = std::env::temp_dir().join(format!(
            "arcweft-missing-cache-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));

        cache_stats_command(&CacheOptions {
            root: root.clone(),
            json: true,
        })
        .expect("missing cache stats are empty");
        cache_verify_command(&CacheOptions { root, json: true })
            .expect("missing cache verifies as empty");
    }

    #[test]
    fn explain_rejects_invalid_digest_query() {
        let root = std::env::temp_dir().join(format!(
            "arcweft-cache-explain-invalid-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));

        let result = cache_explain_command(&CacheExplainOptions {
            query: "not-a-digest".to_owned(),
            logical: false,
            root,
            json: true,
        });

        assert_eq!(result, Err(ExitCode::FAILURE));
    }

    #[test]
    fn prune_missing_cache_root_is_empty_dry_run() {
        let root = std::env::temp_dir().join(format!(
            "arcweft-cache-prune-missing-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));

        cache_prune_command(&CachePruneOptions {
            root,
            apply: false,
            json: true,
        })
        .expect("missing cache prune dry-run succeeds");
    }

    #[test]
    fn fetch_rejects_invalid_content_root_digest() {
        let result = cache_fetch_command(&CacheFetchOptions {
            manifest: PathBuf::from("missing.awfr"),
            content_root: "not-a-digest".to_owned(),
            root: PathBuf::from("target/arcweft/cache/v1"),
            json: true,
        });

        assert_eq!(result, Err(ExitCode::from(2)));
    }

    #[test]
    fn fetch_external_rejects_invalid_descriptor_id() {
        let result = cache_fetch_external_command(&CacheFetchExternalOptions {
            archive: PathBuf::from("missing.awfr"),
            bundle_content_root: "00".repeat(32),
            descriptor_id: "not-a-section-id".to_owned(),
            root: PathBuf::from("target/arcweft/cache/v1"),
            json: true,
        });

        assert_eq!(result, Err(ExitCode::from(2)));
    }

    #[test]
    fn fetch_populates_cache_from_file_release_manifest() {
        let root = temp_root("cli-fetch-file");
        let cache = root.join("cache");
        let bundle = encode_bundle(
            BundleKind::ContentPack,
            br#"{"kind":"content"}"#,
            vec![SectionInput::embedded(
                SectionId::from_bytes([2; 16]),
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                b"voice",
            )],
        )
        .expect("content pack encodes");
        fs::create_dir_all(&root).expect("root creates");
        fs::write(root.join("content.awfb"), &bundle).expect("bundle writes");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("file:content.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let content_root = bundle_ref.content_root.to_string();
        let manifest = ReleaseManifest::new([bundle_ref]).expect("manifest");
        let manifest_path = root.join("game.awfr");
        fs::write(
            &manifest_path,
            manifest.to_json_bytes().expect("manifest encodes"),
        )
        .expect("manifest writes");

        cache_fetch_command(&CacheFetchOptions {
            manifest: manifest_path,
            content_root: content_root.clone(),
            root: cache.clone(),
            json: true,
        })
        .expect("fetch succeeds");

        let verify = verify_cache(&cache).expect("cache verifies");
        assert_eq!(verify.status, CacheVerifyStatus::Ok);
        assert_eq!(verify.stats.object_files, 1);
        assert_eq!(verify.stats.record_files, 1);
        cache_explain_command(&CacheExplainOptions {
            query: format!("content-root:{content_root}"),
            logical: true,
            root: cache,
            json: true,
        })
        .expect("logical explain finds release record");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fetch_external_populates_cache_from_file_awfr_archive() {
        let root = temp_root("cli-fetch-external-file");
        let cache = root.join("cache");
        let payload = b"external-voice";
        let section_id = SectionId::from_bytes([7; 16]);
        let bundle = encode_bundle(
            BundleKind::ContentPack,
            br#"{"kind":"content"}"#,
            vec![SectionInput::external_ref(
                section_id,
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                payload.len() as u64,
                BundleDigest::of(payload),
            )],
        )
        .expect("content pack encodes");
        let view = BundleView::parse(&bundle, ReadBudget::default()).expect("bundle parses");
        let carrier = arcweft_bundle::release::archive::ExternalPayloadCarrier::from_descriptor(
            &view.sections()[0],
            view.artifact_identity(),
            ExternalPayloadMediaType::default(),
            payload.len() as u64,
            BundleDigest::of(payload),
            [ReleaseMirror::new("file:payload.bin").expect("payload mirror")],
        )
        .expect("carrier");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("file:content.awfb").expect("bundle mirror")],
        )
        .expect("bundle ref");
        let archive = AwfrArchiveManifest::new(
            ReleaseChannel::new("dev").expect("channel"),
            ReleaseManifest::new([bundle_ref]).expect("manifest"),
            [carrier.clone()],
        )
        .expect("archive");
        fs::create_dir_all(&root).expect("root creates");
        fs::write(root.join("payload.bin"), payload).expect("payload writes");
        let archive_path = root.join("game.awfr");
        fs::write(
            &archive_path,
            archive.to_json_bytes().expect("archive encodes"),
        )
        .expect("archive writes");

        cache_fetch_external_command(&CacheFetchExternalOptions {
            archive: archive_path,
            bundle_content_root: carrier.bundle_content_root.to_string(),
            descriptor_id: carrier.descriptor_id.to_string(),
            root: cache.clone(),
            json: true,
        })
        .expect("external fetch succeeds");

        let verify = verify_cache(&cache).expect("cache verifies");
        assert_eq!(verify.status, CacheVerifyStatus::Ok);
        assert_eq!(verify.stats.object_files, 1);
        assert_eq!(verify.stats.record_files, 1);
        cache_explain_command(&CacheExplainOptions {
            query: carrier.cache_key.logical_item(),
            logical: true,
            root: cache,
            json: true,
        })
        .expect("logical explain finds external payload record");
        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "arcweft-cache-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }
}
