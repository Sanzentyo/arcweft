use super::state::LspProfileState;
use super::*;
use arcweft_manifest_model::RawDigest;
use arcweft_runtime_host::RuntimeHostRunnerKind;
use std::{
    fs::{self, create_dir_all, write},
    path::{Component, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

const TRUCK_METADATA: &str =
    include_str!("../../../arcweft-adapter-metadata/tests/fixtures/truck-rust.adapter.json");

#[test]
fn resolves_project_profile_and_external_module_metadata() {
    let project = TestProject::new("lsp-profile-resolve");
    project.write(
        "arcw.toml",
        &external_module_manifest(TRUCK_METADATA, "generated/truck.adapter.json"),
    );
    project.write("src/main.arcw", "flow @.main main {}\n");
    project.write("generated/truck.adapter.json", TRUCK_METADATA);

    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));
    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));

    assert!(
        profile.diagnostics().is_empty(),
        "unexpected diagnostics: {:?}",
        profile.diagnostics()
    );
    assert_eq!(profile.adapter().id().as_str(), "sans-io");
    assert!(profile.adapter().symbols().iter().any(|symbol| {
        symbol
            .path()
            .segments()
            .iter()
            .map(arcweft_adapter_context::manifest::AdapterSymbolSegment::as_str)
            .eq(["mini_games", "truck", "TruckResult"])
    }));
}

#[test]
fn failed_rebuild_preserves_generation_and_cache() {
    let project = TestProject::new("lsp-profile-failed-rebuild");
    project.write(
        "arcw.toml",
        &minimal_manifest("lsp-profile-failed-rebuild", "server", ""),
    );
    project.write("src/main.arcw", "fn main() -> Unit { () }\n");
    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));
    let state = Arc::new(LspProfileState::new());
    let first = resolver
        .resolve_for_document_path_with_state(&project.path("src/main.arcw"), Arc::clone(&state));
    assert!(first.diagnostics().is_empty(), "{:?}", first.diagnostics());
    let accepted = state.current().expect("first accepted environment");
    accepted.insert_cache_for_test("analysis", "accepted");
    let generation = accepted.generation();
    let cache = accepted.cache_snapshot_for_test();

    project.write("src/main.arcw", "fn main( {\n");
    let failed = resolver
        .resolve_for_document_path_with_state(&project.path("src/main.arcw"), Arc::clone(&state));

    assert!(
        failed
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.kind() == LspProfileDiagnosticKind::CharacterCatalog })
    );
    let retained = state.current().expect("accepted environment is retained");
    assert!(Arc::ptr_eq(&retained, &accepted));
    assert_eq!(retained.generation(), generation);
    assert_eq!(retained.cache_snapshot_for_test(), cache);
}

#[test]
fn closed_profile_state_reports_typed_publication_failure_without_mutation() {
    let project = TestProject::new("lsp-profile-closed-publication");
    project.write(
        "arcw.toml",
        &minimal_manifest("lsp-profile-closed-publication", "server", ""),
    );
    project.write("src/main.arcw", "fn main() -> Unit { () }\n");
    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));
    let state = Arc::new(LspProfileState::new());
    state.shutdown();

    let profile = resolver
        .resolve_for_document_path_with_state(&project.path("src/main.arcw"), Arc::clone(&state));

    assert!(state.current().is_none());
    assert!(
        profile
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.kind() == LspProfileDiagnosticKind::ProfilePublication),
        "{:?}",
        profile.diagnostics(),
    );
}

#[test]
fn invalid_external_metadata_preserves_the_real_accepted_profile_state() {
    let project = TestProject::new("lsp-profile-malformed-adapter-path");
    project.write(
        "arcw.toml",
        &external_module_manifest(TRUCK_METADATA, "generated/truck.adapter.json"),
    );
    project.write("src/main.arcw", "fn main() -> Unit { () }\n");
    project.write("generated/truck.adapter.json", TRUCK_METADATA);
    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));
    let state = Arc::new(LspProfileState::new());
    let first = resolver
        .resolve_for_document_path_with_state(&project.path("src/main.arcw"), Arc::clone(&state));
    assert!(first.diagnostics().is_empty(), "{:?}", first.diagnostics());
    let accepted = state.current().expect("first accepted environment");
    accepted.insert_cache_for_test("analysis", "accepted");
    let generation = accepted.generation();
    let cache = accepted.cache_snapshot_for_test();

    project.write("generated/truck.adapter.json", "{ not valid metadata");
    let failed = resolver
        .resolve_for_document_path_with_state(&project.path("src/main.arcw"), Arc::clone(&state));

    assert!(
        failed.diagnostics().iter().any(|diagnostic| {
            diagnostic.kind() == LspProfileDiagnosticKind::ExternalModuleMetadataParse
        }),
        "{:?}",
        failed.diagnostics()
    );
    let retained = state.current().expect("accepted environment is retained");
    assert!(Arc::ptr_eq(&retained, &accepted));
    assert!(Arc::ptr_eq(retained.world(), accepted.world()));
    assert_eq!(retained.generation(), generation);
    assert_eq!(retained.cache_snapshot_for_test(), cache);
}

#[test]
fn missing_manifest_is_reported_without_absolute_path() {
    let project = TestProject::new("lsp-profile-missing");
    project.write("src/main.arcw", "flow @.main main {}\n");
    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, None);

    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));

    assert_eq!(
        profile.diagnostics()[0].kind(),
        LspProfileDiagnosticKind::WorkspaceManifestNotFound
    );
    assert!(!profile.diagnostics()[0].message().contains(":/"));
    assert!(!profile.diagnostics()[0].message().contains(":\\"));
}

#[test]
fn failed_topology_resource_uses_owner_relative_identity() {
    let project = TestProject::new("lsp-profile-token-map-range");
    let manifest = r#"schema = 1

[package]
id = "org.arcweft.lsp-profile-token-map-range"
version = "0.1.0"

[content-units.characters]
roots = ["@character.missing"]
visibility = "package"
demand = "required"

[profiles.other]
kind = "game"
entry = "@entry.game.main"
source = "src/main.arcw"

[profiles.other.content.characters]
residency = "startup"
placement = "embedded"
compression = "none"

[profiles.dev]
kind = "game"
entry = "@entry.game.main"
source = "src/main.arcw"

[profiles.dev.content.characters]
residency = "startup"
placement = "embedded"
compression = "none"
"#;
    project.write("arcw.toml", manifest);
    project.write("src/main.arcw", "fn main() -> Unit { () }\n");
    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));

    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
    let diagnostic = profile
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.kind() == LspProfileDiagnosticKind::CharacterManifestRead)
        .expect("missing character manifest diagnostic");
    assert_eq!(diagnostic.profile_id(), Some("dev"));
    assert_eq!(
        diagnostic.resource(),
        Some("assets/missing.awchar/character.awchar.json")
    );
    assert!(diagnostic.source().is_none());
}

#[test]
fn missing_external_module_metadata_diagnostic_keeps_profile_relative_resource() {
    let project = TestProject::new("lsp-profile-rust-missing");
    project.write(
        "arcw.toml",
        &external_module_manifest(TRUCK_METADATA, "generated/missing.adapter.json"),
    );
    project.write("src/main.arcw", "flow @.main main {}\n");
    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));

    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
    let diagnostic = profile
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.kind() == LspProfileDiagnosticKind::ExternalModuleMetadataRead
        })
        .expect("external module metadata read diagnostic");

    assert_eq!(diagnostic.profile_id(), Some("dev"));
    assert_eq!(
        diagnostic.resource(),
        Some("generated/missing.adapter.json")
    );
    assert!(!diagnostic.message().contains(":/"));
    assert!(!diagnostic.message().contains(":\\"));
}

#[test]
fn invalid_external_module_metadata_diagnostic_keeps_profile_relative_resource() {
    let project = TestProject::new("lsp-profile-rust-invalid");
    let invalid = "{ not json";
    project.write(
        "arcw.toml",
        &external_module_manifest(invalid, "generated/bad.adapter.json"),
    );
    project.write("src/main.arcw", "flow @.main main {}\n");
    project.write("generated/bad.adapter.json", invalid);
    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));

    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
    let diagnostic = profile
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.kind() == LspProfileDiagnosticKind::ExternalModuleMetadataParse
        })
        .expect("external module metadata parse diagnostic");

    assert_eq!(diagnostic.profile_id(), Some("dev"));
    assert_eq!(diagnostic.resource(), Some("generated/bad.adapter.json"));
    assert!(!diagnostic.message().contains(":/"));
    assert!(!diagnostic.message().contains(":\\"));
}

fn minimal_manifest(project: &str, kind: &str, profile_extra: &str) -> String {
    format!(
        r#"schema = 1

[package]
id = "org.arcweft.{project}"
version = "0.1.0"

[profiles.dev]
kind = "{kind}"
entry = "@entry.server.main"
source = "src/main.arcw"
{profile_extra}
"#
    )
}

fn external_module_manifest(metadata: &str, path: &str) -> String {
    let decoded = serde_json::from_str::<serde_json::Value>(metadata).ok();
    let package = decoded
        .as_ref()
        .and_then(|value| value["package"]["id"].as_str())
        .unwrap_or("com.example.truck");
    let version = decoded
        .as_ref()
        .and_then(|value| value["package"]["version"].as_str())
        .unwrap_or("1.2.3");
    let module = decoded
        .as_ref()
        .and_then(|value| value["module"]["id"].as_str())
        .unwrap_or("truck");
    let family = decoded
        .as_ref()
        .and_then(|value| value["target"]["family"].as_str())
        .unwrap_or("rust");
    let abi_hash = decoded
        .as_ref()
        .and_then(|value| value["abi_hash"].as_str())
        .unwrap_or("blake3:0000000000000000000000000000000000000000000000000000000000000000");
    let raw_hash = RawDigest::for_bytes(metadata.as_bytes());
    format!(
        r#"schema = 1

[package]
id = "org.arcweft.lsp-profile-external"
version = "0.1.0"

[external-modules.truck]
mount = "mini_games.truck"
metadata = "{path}"
metadata-hash = "{raw_hash}"
expected-package = "{package}"
expected-version = "{version}"
expected-module = "{module}"
expected-family = "{family}"
expected-abi-hash = "{abi_hash}"
visibility = "package"
demand = "required"

[profiles.dev]
kind = "server"
entry = "@entry.server.main"
source = "src/main.arcw"
external-modules = ["truck"]
"#
    )
}

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("{name}-{unique}"));
        create_dir_all(&root).expect("create test project root");
        Self { root }
    }

    fn path(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.path(path);
        if let Some(parent) = path.parent() {
            create_dir_all(parent).expect("create parent");
        }
        write(path, contents).expect("write fixture");
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        if self
            .root
            .components()
            .any(|component| matches!(component, Component::Normal(_)))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
