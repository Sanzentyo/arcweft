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
        &character_project_manifest("lsp-character-completions", "zundamon"),
    );
    project.write("src/main.arcw", "flow @flow.main main {}\n");
    write_character_fixture(&project);

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
        &character_project_manifest("lsp-character-hover", "zundamon"),
    );
    project.write("src/main.arcw", "flow @flow.main main {}\n");
    write_character_fixture(&project);

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
        &character_project_manifest("lsp-character-missing", "missing"),
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

fn character_project_manifest(package: &str, character: &str) -> String {
    format!(
        r#"schema = 1

[package]
id = "org.arcweft.tests.{package}"
version = "0.1.0"

[content-units.characters]
roots = ["@character.{character}"]
visibility = "package"
demand = "required"

[profiles.dev]
kind = "game"
entry = "@entry.game.main"
source = "src/main.arcw"

[profiles.dev.content.characters]
residency = "startup"
placement = "embedded"
compression = "none"
"#
    )
}

fn write_character_fixture(project: &TestProject) {
    project.write(
        "assets/zundamon.awchar/character.awchar.json",
        include_str!("fixtures/zundamon.awchar/character.awchar.json"),
    );
    for (path, bytes) in [
        (
            "body--default.png",
            include_bytes!("fixtures/zundamon.awchar/layers/body--default.png").as_slice(),
        ),
        (
            "eyes--normal.png",
            include_bytes!("fixtures/zundamon.awchar/layers/eyes--normal.png").as_slice(),
        ),
        (
            "eyes--smile.png",
            include_bytes!("fixtures/zundamon.awchar/layers/eyes--smile.png").as_slice(),
        ),
        (
            "mouth--neutral.png",
            include_bytes!("fixtures/zundamon.awchar/layers/mouth--neutral.png").as_slice(),
        ),
        (
            "mouth--smile.png",
            include_bytes!("fixtures/zundamon.awchar/layers/mouth--smile.png").as_slice(),
        ),
    ] {
        project.write_bytes(&format!("assets/zundamon.awchar/layers/{path}"), bytes);
    }
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

    fn write_bytes(&self, path: &str, contents: &[u8]) {
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
