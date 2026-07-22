use arcweft_character::id::CharacterId;
use arcweft_lang_sema::types::CharacterNominalType;
use arcweft_lsp::profiles::LspProfileResolver;
use arcweft_runtime_host::RuntimeHostRunnerKind;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn profile_loads_character_manifest_into_completion_type_environment() {
    let project = TestProject::new("lsp-character-manifest");
    project.write(
        "arcw.toml",
        r#"schema = 1

[package]
id = "org.arcweft.tests.lsp-character-manifest"
version = "0.1.0"

[content-units.characters]
roots = ["@character.akane"]
visibility = "package"
demand = "required"

[profiles.game]
kind = "game"
entry = "@entry.game.main"
source = "src/main.arcw"

[profiles.game.content.characters]
residency = "startup"
placement = "embedded"
compression = "none"
"#,
    );
    project.write(
        "src/main.arcw",
        "flow @flow.main main() -> String { return \"ok\" }\n",
    );
    project.write(
        "assets/akane.awchar/character.awchar.json",
        r#"{
  "format": "arcweft.character",
  "version": 1,
  "character": "character.akane",
  "canvas": { "width": 96, "height": 128 },
  "anchor": { "x": 48, "y": 128 },
  "default_look": "normal",
  "parts": [{
    "id": "body",
    "z": 0,
    "variants": [{
      "id": "default",
      "asset": "layers/body.png",
      "rect": { "x": 0, "y": 0, "width": 96, "height": 128 },
      "opacity": 255,
      "blend": "normal",
      "clipping": false
    }]
  }],
  "looks": [
    { "id": "normal", "select": [{ "part": "body", "variant": "default" }] },
    { "id": "smile", "select": [{ "part": "body", "variant": "default" }] }
  ]
}"#,
    );
    project.write_bytes(
        "assets/akane.awchar/layers/body.png",
        include_bytes!(
            "../../arcweft-character/tests/fixtures/zundamon.awchar/layers/body--default.png"
        ),
    );

    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("game".to_owned()));
    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));

    assert!(
        profile.diagnostics().is_empty(),
        "{:?}",
        profile.diagnostics()
    );
    assert_eq!(profile.characters().len(), 1);
    let character = CharacterId::try_new("character.akane").expect("character");
    let accepted = profile
        .accepted_environment()
        .expect("source-backed project registration must be accepted");
    assert_eq!(
        accepted
            .world()
            .environment()
            .character_enum_variants(&CharacterNominalType::Look { character })
            .expect("registered look variants")
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["normal", "smile"]
    );
}

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("{label}-{unique}"));
        fs::create_dir_all(&root).expect("root");
        Self { root }
    }

    fn path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.root.join(path)
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.path(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, contents).expect("write");
    }

    fn write_bytes(&self, path: &str, contents: &[u8]) {
        let path = self.path(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, contents).expect("write");
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
