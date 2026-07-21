use arcweft_character::id::CharacterId;
use arcweft_lang_sema::types::TypeKind;
use arcweft_lsp::{
    features::{character_metadata::character_hover_markdown, completion::completions},
    profiles::LspProfileResolver,
};
use arcweft_runtime_host::RuntimeHostRunnerKind;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn tooling_preserves_cross_character_nominal_provenance() {
    let project = TestProject::new("lsp-character-nominal-identity");
    project.write(
        "arcw.toml",
        r#"schema = 1

[package]
id = "org.arcweft.tests.lsp-character-nominal-identity"
version = "0.1.0"

[content-units.characters]
roots = ["@character.akane", "@character.aoi"]
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
    project.write("src/main.arcw", "flow @flow.main main { return \"ok\" }\n");
    project.write_manifest("akane");
    project.write_manifest("aoi");

    let resolver = LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("game".to_owned()));
    let profile = resolver.resolve_for_document_path(&project.path("src/main.arcw"));
    assert!(
        profile.diagnostics().is_empty(),
        "{:?}",
        profile.diagnostics()
    );

    let ambiguous = character_hover_markdown(&profile, ".smile", None).expect("ambiguous hover");
    assert!(ambiguous.contains("ambiguous character member"));
    assert!(ambiguous.contains("CharacterLook<character.akane>.smile"));
    assert!(ambiguous.contains("CharacterLook<character.aoi>.smile"));

    let expected =
        TypeKind::character_look(CharacterId::try_new("character.aoi").expect("character"));
    let scoped_hover =
        character_hover_markdown(&profile, ".smile", Some(&expected)).expect("expected-type hover");
    assert!(scoped_hover.contains("for `character.aoi`"));
    assert!(!scoped_hover.contains("for `character.akane`"));

    let smile_items = completions(&profile, None)
        .into_iter()
        .filter(|item| item.label == ".smile")
        .collect::<Vec<_>>();
    assert!(
        smile_items
            .iter()
            .any(|item| { item.detail.as_deref() == Some("CharacterLook<character.akane>.smile") })
    );
    assert!(
        smile_items
            .iter()
            .any(|item| { item.detail.as_deref() == Some("CharacterLook<character.aoi>.smile") })
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

    fn write_manifest(&self, local: &str) {
        self.write(
            &format!("assets/{local}.awchar/character.awchar.json"),
            &format!(
                r#"{{
  "format": "arcweft.character",
  "version": 1,
  "character": "character.{local}",
  "canvas": {{ "width": 96, "height": 128 }},
  "anchor": {{ "x": 48, "y": 128 }},
  "default_look": "normal",
  "parts": [{{
    "id": "body",
    "z": 0,
    "variants": [{{
      "id": "smile",
      "asset": "layers/{local}-body.png",
      "rect": {{ "x": 0, "y": 0, "width": 96, "height": 128 }},
      "opacity": 255,
      "blend": "normal",
      "clipping": false
    }}]
  }}],
  "looks": [
    {{ "id": "normal", "select": [{{ "part": "body", "variant": "smile" }}] }},
    {{ "id": "smile", "select": [{{ "part": "body", "variant": "smile" }}] }}
  ]
}}"#
            ),
        );
        self.write_bytes(
            &format!("assets/{local}.awchar/layers/{local}-body.png"),
            include_bytes!(
                "../../arcweft-character/tests/fixtures/zundamon.awchar/layers/body--default.png"
            ),
        );
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
