use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use arcweft_character::{
    manifest::registration::SourceBackedCharacterManifest,
    registration_catalog::SourceBackedCharacterCatalog,
};
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::{env::TypeCheckEnv, registration::ProjectRegistrationFacts};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath, incremental::SyntaxDatabase, parser::ParseOptions,
};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId};

use crate::project::{
    CompiledProject, ProjectCompilationContext, ProjectCompilationSession, compile_project,
};

const CHARACTER_MANIFEST: &str = r#"{
  "format": "arcweft.character",
  "version": 1,
  "character": "character.alice",
  "canvas": { "width": 64, "height": 128 },
  "anchor": { "x": 32, "y": 128 },
  "default_look": "normal",
  "parts": [{
    "id": "body",
    "z": 0,
    "variants": [{
      "id": "default",
      "asset": "layers/body.png",
      "rect": { "x": 0, "y": 0, "width": 64, "height": 128 },
      "opacity": 255,
      "blend": "normal",
      "clipping": false
    }]
  }],
  "looks": [
    { "id": "normal", "select": [{ "part": "body", "variant": "default" }] },
    { "id": "bright", "select": [{ "part": "body", "variant": "default" }] }
  ]
}"#;

fn document(path: &str, text: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("arcweft-test://character-dialogue/{path}")).unwrap(),
            SourceName::path(path),
            text,
        )
        .unwrap(),
    )
}

pub(super) fn compile_with_character_manifest(source: &str) -> CompiledProject {
    let source = document("src/main.arcw", source);
    let manifest = document("arcw.toml", "");
    let character_source = document("alice.json", CHARACTER_MANIFEST);
    let character = SourceBackedCharacterManifest::decode_registration_json(&character_source)
        .expect("source-backed Character manifest");
    let characters =
        SourceBackedCharacterCatalog::try_new(character_source.identity().clone(), vec![character])
            .expect("accepted Character catalog");
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        PackageSpec {
            id: PackageId::new("local.arcweft.character-dialogue-test").unwrap(),
            version: PackageVersion::new("0.0.0").unwrap(),
        },
        BuildSpec::default(),
        Arc::clone(&manifest),
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            PathBuf::from("src/main.arcw"),
            Arc::clone(&source),
            [],
        )],
    )
    .unwrap();
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(project.package().id.as_str()).unwrap(),
        source.identity().id().clone(),
        "character-dialogue-test",
    )
    .unwrap();
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&source), manifest, character_source],
        vec![],
        vec![characters],
        vec![],
    )
    .unwrap();
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(ResourceTypeRegistry::empty()),
        None,
        None,
    );
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(source.display_name().clone()),
            source,
            ParseOptions::default(),
        )
        .unwrap();
    let mut session = ProjectCompilationSession::try_new().unwrap();
    compile_project(
        &mut session,
        &project,
        &BTreeMap::from([(CanonicalModulePath::crate_root(), parsed)]),
        &context,
    )
    .expect("CharacterDialogue Look values compile from their accepted manifest")
}
