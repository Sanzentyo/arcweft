use arcweft_character::id::CharacterId;
use arcweft_lang_sema::types::TypeKind;
use arcweft_lsp::features::character_metadata::character_hover_markdown;
use arcweft_lsp::features::completion::completions;
use arcweft_lsp::profiles::{LspProfileDiagnosticKind, LspProfileResolver};
use arcweft_runtime_host::RuntimeHostRunnerKind;
use std::fs::{create_dir_all, write};
use std::path::{Component, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn completions_include_loaded_character_manifest_data() {
    let project = TestProject::new("lsp-character-completions");
    project.write(
        "arcw.toml",
        r#"
[package]
name = "lsp-character-completions"

[profiles.dev]
kind = "game"
entry = "entry.game.main"
source = "src/main.arcw"
adapter = "sans-io"
character_manifests = ["assets/zundamon.awchar"]
"#,
    );
    project.write("src/main.arcw", "flow @flow.main main {}\n");
    project.write(
        "assets/zundamon.awchar/character.awchar.json",
        include_str!("fixtures/zundamon.awchar/character.awchar.json"),
    );

    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));
    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
    assert!(
        profile.diagnostics().is_empty(),
        "{:?}",
        profile.diagnostics()
    );

    let labels = completions(&profile, None)
        .into_iter()
        .map(|item| item.label)
        .collect::<Vec<_>>();
    assert!(labels.iter().any(|label| label == "@character.zundamon"));
    assert!(labels.iter().any(|label| label == ".normal"));
    assert!(labels.iter().any(|label| label == ".smile"));
    assert!(labels.iter().any(|label| label == ".eyes"));
}

#[test]
fn hover_includes_psd_source_layer_names() {
    let project = TestProject::new("lsp-character-hover");
    project.write(
        "arcw.toml",
        r#"
[package]
name = "lsp-character-hover"

[profiles.dev]
kind = "game"
entry = "entry.game.main"
source = "src/main.arcw"
adapter = "sans-io"
character_manifests = ["assets/zundamon.awchar"]
"#,
    );
    project.write("src/main.arcw", "flow @flow.main main {}\n");
    project.write(
        "assets/zundamon.awchar/character.awchar.json",
        include_str!("fixtures/zundamon.awchar/character.awchar.json"),
    );

    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));
    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
    let expected =
        TypeKind::character_look(CharacterId::try_new("character.zundamon").expect("character id"));
    let hover = character_hover_markdown(&profile, ".smile", Some(&expected)).expect("typed hover");

    assert!(hover.contains("character look") || hover.contains("character variant"));
    assert!(hover.contains("source PSD layer") || hover.contains("mouth = smile"));
}

#[test]
fn missing_character_manifest_uses_typed_profile_diagnostic() {
    let project = TestProject::new("lsp-character-missing");
    project.write(
        "arcw.toml",
        r#"
[package]
name = "lsp-character-missing"

[profiles.dev]
kind = "game"
entry = "entry.game.main"
source = "src/main.arcw"
adapter = "sans-io"
character_manifests = ["assets/missing.awchar"]
"#,
    );
    project.write("src/main.arcw", "flow @flow.main main {}\n");

    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("dev".into()));
    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));

    assert!(
        profile
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.kind() == LspProfileDiagnosticKind::CharacterManifestRead)
    );
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
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}
