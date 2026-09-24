use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
};

use arcweft_character::{
    manifest::registration::{
        CharacterManifestRootField, CharacterManifestTokenPath, SourceBackedCharacterManifest,
    },
    registration_catalog::SourceBackedCharacterCatalog,
};
use arcweft_lang_hir::symbol::{
    CallablePackageId, ExternalDeclarationSeed, ProjectDirectBinding, ProjectSymbolWorldId,
};
use arcweft_lang_sema::{
    callable::{AdapterPackageId, EnvironmentCallableOwner},
    character_dialogue::CharacterDialogueCustomFieldBinding,
    env::TypeCheckEnv,
    registration::{
        CharacterDialogueCustomFieldInput, EnvironmentManifestDigest, EnvironmentPublicationItemId,
        EnvironmentTypeProjectionKind, EnvironmentTypeProjectionNode, ExternalRegistrationFact,
        ProjectRegistrationFacts, RegisteredExternalOwner,
        SourceBackedEnvironmentRegistrationInput,
    },
};
use arcweft_lang_syntax::{
    ast::{
        common::Visibility,
        module_path::{CanonicalModulePath, ModulePathRoot},
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
    },
    incremental::SyntaxDatabase,
    parser::ParseOptions,
};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceName, SourceRange, identity::SourceSnapshotId,
};

use crate::project::{
    CompiledProject, ProjectCompilationContext, ProjectCompilationSession, ProjectCompileError,
    compile_project,
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
    compile_character_project(source, false, None)
        .expect("CharacterDialogue Look values compile from their accepted manifest")
}

pub(super) fn compile_with_external_character(
    source: &str,
) -> Result<CompiledProject, ProjectCompileError> {
    compile_character_project(source, true, None)
}

pub(super) fn compile_with_custom_field(source: &str, clearable: bool) -> CompiledProject {
    compile_character_project(source, false, Some(clearable))
        .expect("source-backed custom fields are accepted generation inputs")
}

fn compile_character_project(
    source: &str,
    external_character: bool,
    custom_clearable: Option<bool>,
) -> Result<CompiledProject, ProjectCompileError> {
    let source = document("src/main.arcw", source);
    let manifest = document("arcw.toml", "");
    let character_source = document("alice.json", CHARACTER_MANIFEST);
    let character = SourceBackedCharacterManifest::decode_registration_json(&character_source)
        .expect("source-backed Character manifest");
    let externals = external_character
        .then(|| external_fact(&character))
        .into_iter()
        .collect();
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
    let mut documents = vec![Arc::clone(&source), manifest, character_source];
    let mut environment_inputs = Vec::new();
    if let Some(clearable) = custom_clearable {
        let (document, input) = custom_field_input(clearable);
        documents.push(document);
        environment_inputs.push(input);
    }
    let facts = ProjectRegistrationFacts::try_new(
        world,
        documents,
        externals,
        vec![characters],
        environment_inputs,
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
}

fn custom_field_input(
    clearable: bool,
) -> (
    Arc<SourceDocument>,
    SourceBackedEnvironmentRegistrationInput,
) {
    let source = document("dialogue-custom-field.environment", "mood");
    let declaration = source
        .span(SourceRange::new(0, source.text().len()))
        .unwrap();
    let owner = EnvironmentCallableOwner::Adapter(
        AdapterPackageId::try_new("dialogue-custom-field").unwrap(),
    );
    let item = EnvironmentPublicationItemId::AdapterSymbol {
        owner: owner.clone(),
        path: ProjectSymbolPath::new(
            ModulePathRoot::ImplicitCrate,
            [ProjectSymbolSegment::try_new("mood").unwrap()],
        )
        .unwrap(),
    };
    let input = SourceBackedEnvironmentRegistrationInput::new(
        owner,
        source.identity().clone(),
        EnvironmentManifestDigest::from_bytes([91; 32]),
        [],
        [],
        [],
        [],
    )
    .with_character_dialogue_fields([CharacterDialogueCustomFieldInput::new(
        item,
        arcweft_interaction_model::dialogue::CharacterDialogueCustomFieldId::try_new(
            "character_dialogue_field.mood",
        )
        .unwrap(),
        [CharacterDialogueCustomFieldBinding::global("mood")],
        EnvironmentTypeProjectionNode::new(
            declaration.clone(),
            EnvironmentTypeProjectionKind::String,
        ),
        None,
        arcweft_core::entry::TypeLayoutHash::from_bytes([9; 32]),
        clearable,
        BTreeSet::new(),
        declaration,
    )]);
    (source, input)
}

fn external_fact(manifest: &SourceBackedCharacterManifest) -> ExternalRegistrationFact {
    let owner = manifest.manifest().character().clone();
    let declaration = manifest
        .source_map()
        .token(&CharacterManifestTokenPath::Root(
            CharacterManifestRootField::Character,
        ))
        .expect("accepted Character owner source")
        .value()
        .clone();
    let compact = owner
        .compact_segments()
        .map(|segment| ProjectSymbolSegment::try_new(segment).unwrap())
        .collect::<Vec<_>>();
    let qualified = std::iter::once(ProjectSymbolSegment::try_new("character").unwrap())
        .chain(compact.iter().cloned())
        .collect::<Vec<_>>();
    let bindings = [qualified, compact]
        .into_iter()
        .map(|segments| {
            ProjectDirectBinding::try_new(
                CanonicalModulePath::crate_root(),
                ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, segments).unwrap(),
                Some(Visibility::Public),
                declaration.clone(),
                false,
            )
            .unwrap()
        })
        .collect();
    let seed = ExternalDeclarationSeed::try_new(
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), owner.as_str()).unwrap(),
        Some(Visibility::Public),
        declaration.clone(),
        bindings,
    )
    .unwrap();
    ExternalRegistrationFact::new(seed, RegisteredExternalOwner::Character(owner), declaration)
}
