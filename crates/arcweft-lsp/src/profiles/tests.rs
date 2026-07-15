use super::cache::LspProfileState;
use super::*;
use arcweft_runtime_host::RuntimeHostRunnerKind;
use arcweft_rust_abi::{
    ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage, ArcweftRustParam,
    ArcweftRustPurity, ArcweftRustTypeRef,
};
use std::{
    fs::{self, create_dir_all, write},
    path::{Component, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn resolves_project_profile_adapter_and_rust_metadata() {
    let project = TestProject::new("lsp-profile-resolve");
    project.write(
        "arcw.toml",
        r#"
[package]
name = "lsp-profile-resolve"

[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "custom-echo"
adapter_manifests = ["adapters/custom-echo.toml"]
rust_metadata = ["target/arcweft/custom.json"]
"#,
    );
    project.write("src/main.arcw", "flow @.main main {}\n");
    project.write(
        "adapters/custom-echo.toml",
        r#"
schema_version = 1
id = "custom-echo"
display_name = "Custom Echo"

[[functions]]
name = "custom.echo"
return_type = "String"
params = [{ name = "value", ty = "String" }]

[[host_calls]]
id = "custom.echo"
return_type = "Unit"
"#,
    );
    let rust_manifest = ArcweftRustManifest::new(ArcweftRustPackage {
        name: "custom_adapter".to_owned(),
        version: "0.1.0".to_owned(),
        metadata_hash: None,
    })
    .with_function(ArcweftRustFunction {
        name: "custom.score".to_owned(),
        rust_path: "custom_adapter::score".to_owned(),
        params: vec![ArcweftRustParam {
            name: "value".to_owned(),
            ty: ArcweftRustTypeRef::I32,
        }],
        return_type: ArcweftRustTypeRef::I64,
        purity: ArcweftRustPurity::Pure,
        effects: Vec::new(),
    });
    project.write(
        "target/arcweft/custom.json",
        &rust_manifest.to_json_pretty().expect("metadata json"),
    );

    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));
    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));

    assert!(
        profile.diagnostics().is_empty(),
        "unexpected diagnostics: {:?}",
        profile.diagnostics()
    );
    assert_eq!(profile.adapter().id().as_str(), "custom-echo");
    assert!(
        profile
            .adapter()
            .functions()
            .iter()
            .any(|function| function.name() == "custom.echo")
    );
    assert!(
        profile
            .adapter()
            .rust_functions()
            .iter()
            .any(|function| function.name() == "custom.score")
    );
}

#[test]
fn failed_rebuild_preserves_generation_and_cache() {
    let project = TestProject::new("lsp-profile-failed-rebuild");
    project.write(
        "arcw.toml",
        r#"
[package]
name = "lsp-profile-failed-rebuild"

[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "sans-io"
"#,
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
fn adapter_manifest_diagnostic_keeps_profile_relative_resource() {
    let project = TestProject::new("lsp-profile-adapter-diagnostic");
    project.write(
        "arcw.toml",
        r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "missing"
adapter_manifests = ["adapters/missing.toml"]
"#,
    );
    project.write("src/main.arcw", "flow @.main main {}\n");
    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));

    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
    let diagnostic = profile
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.kind() == LspProfileDiagnosticKind::AdapterManifestRead)
        .expect("adapter manifest diagnostic");

    assert_eq!(diagnostic.profile_id(), Some("dev"));
    assert_eq!(diagnostic.resource(), Some("adapters/missing.toml"));
    assert!(!diagnostic.message().contains(":/"));
    assert!(!diagnostic.message().contains(":\\"));
}

#[test]
fn invalid_adapter_manifest_diagnostic_keeps_profile_relative_resource() {
    let project = TestProject::new("lsp-profile-adapter-invalid");
    project.write(
        "arcw.toml",
        r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "missing"
adapter_manifests = ["adapters/bad.toml"]
"#,
    );
    project.write("src/main.arcw", "flow @.main main {}\n");
    project.write("adapters/bad.toml", "schema_version = ");
    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));

    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
    let diagnostic = profile
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.kind() == LspProfileDiagnosticKind::AdapterManifestParse)
        .expect("adapter manifest parse diagnostic");

    assert_eq!(diagnostic.profile_id(), Some("dev"));
    assert_eq!(diagnostic.resource(), Some("adapters/bad.toml"));
    assert!(!diagnostic.message().contains(":/"));
    assert!(!diagnostic.message().contains(":\\"));
}

#[test]
fn use_path_range_comes_from_token_map() {
    let project = TestProject::new("lsp-profile-token-map-range");
    let manifest = r#"
[package]
name = "lsp-profile-token-map-range"

[profiles.other]
kind = "game"
source = "src/main.arcw"
adapter = "sans-io"
character_manifests = ["assets/missing.awchar"]

[profiles.dev]
kind = "game"
source = "src/main.arcw"
adapter = "sans-io"
character_manifests = ["assets/missing.awchar"]
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
    let source = diagnostic.source().expect("structural launch token");
    let token = &manifest[source.range().as_range()];

    assert_eq!(token, "\"assets/missing.awchar\"");
    assert_eq!(
        source.range().start(),
        manifest.rfind(token).expect("selected profile occurrence")
    );
    assert_ne!(
        source.range().start(),
        manifest.find(token).expect("unselected earlier occurrence")
    );
}

#[test]
fn missing_rust_metadata_diagnostic_keeps_profile_relative_resource() {
    let project = TestProject::new("lsp-profile-rust-missing");
    project.write(
        "arcw.toml",
        r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "sans-io"
rust_metadata = ["target/arcweft/missing.json"]
"#,
    );
    project.write("src/main.arcw", "flow @.main main {}\n");
    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));

    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
    let diagnostic = profile
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.kind() == LspProfileDiagnosticKind::RustMetadataRead)
        .expect("rust metadata read diagnostic");

    assert_eq!(diagnostic.profile_id(), Some("dev"));
    assert_eq!(diagnostic.resource(), Some("target/arcweft/missing.json"));
    assert!(!diagnostic.message().contains(":/"));
    assert!(!diagnostic.message().contains(":\\"));
}

#[test]
fn invalid_rust_metadata_diagnostic_keeps_profile_relative_resource() {
    let project = TestProject::new("lsp-profile-rust-invalid");
    project.write(
        "arcw.toml",
        r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "sans-io"
rust_metadata = ["target/arcweft/bad.json"]
"#,
    );
    project.write("src/main.arcw", "flow @.main main {}\n");
    project.write("target/arcweft/bad.json", "{ not json");
    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));

    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
    let diagnostic = profile
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.kind() == LspProfileDiagnosticKind::RustMetadataParse)
        .expect("rust metadata parse diagnostic");

    assert_eq!(diagnostic.profile_id(), Some("dev"));
    assert_eq!(diagnostic.resource(), Some("target/arcweft/bad.json"));
    assert!(!diagnostic.message().contains(":/"));
    assert!(!diagnostic.message().contains(":\\"));
}

#[test]
fn resolves_dialogue_defaults_selection_source_range() {
    let project = TestProject::new("lsp-profile-dialogue-defaults-selection");
    let manifest = r#"
[profiles.dev]
kind = "game"
source = "src/main.arcw"
adapter = "sans-io"
dialogue_defaults = "dialogue.mobile"

[profiles.other]
kind = "game"
source = "src/main.arcw"
adapter = "sans-io"
dialogue_defaults = "dialogue.debug"
"#;
    project.write("arcw.toml", manifest);
    project.write("src/main.arcw", "flow @.main main {}\n");
    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));

    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
    let selection = profile
        .dialogue_defaults_selection()
        .expect("dialogue defaults source selection");
    let range = selection.value_range();

    assert_eq!(&selection.source()[range.clone()], "dialogue.mobile");
    assert_eq!(selection.path(), project.path("arcw.toml").as_path());
    assert!(selection.uri().is_some());
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
