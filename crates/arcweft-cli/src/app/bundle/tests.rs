use super::*;
use arcweft_character::{
    id::CharacterId,
    presentation_name::{
        CharacterPresentationCatalogGeneration, CharacterPresentationCatalogRevision,
        CharacterPresentationLocalePolicyDigest, CharacterPresentationSemanticDigest,
    },
};
use arcweft_compiler::project::{
    CompiledProject, ProjectCompilationContext, ProjectCompilationSession, compile_project,
};
use arcweft_core::plan::RuntimeLineId;
use arcweft_dialogue::{
    DialoguePresentationProfile, DialogueProfileRevision,
    character_presentation::{
        CharacterPresentationTargetEvidence, CheckedCharacterPresentationPlan,
    },
};
use arcweft_id::TextKey;
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::{env::TypeCheckEnv, registration::ProjectRegistrationFacts};
use arcweft_lang_syntax::{incremental::SyntaxDatabase, parser::ParseOptions};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{
    ProductSourceRef, SourceDocument, SourceDocumentId, SourceName, SourceSetRevision,
    identity::SourceSnapshotId,
};
use arcweft_text_model::{
    DialogueContentCatalog, DialogueContentSpec, RichTextDocument, RichTextNode,
};
use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

mod view_part_recovery;

fn test_character_plan() -> CheckedCharacterPresentationPlan {
    CheckedCharacterPresentationPlan::try_new(
        CharacterPresentationTargetEvidence::Exact(
            CharacterId::try_new("character.fixture").unwrap(),
        ),
        CharacterPresentationCatalogGeneration::new(
            CharacterPresentationCatalogRevision::INITIAL,
            CharacterPresentationSemanticDigest::from_bytes([1; 32]),
            CharacterPresentationLocalePolicyDigest::from_bytes([2; 32]),
        ),
    )
    .unwrap()
}

fn test_dialogue_profile_revision() -> DialogueProfileRevision {
    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("cli-bundle-dialogue-profile-fixture").unwrap(),
        SourceName::Memory,
        "schema = 1\n",
    )
    .unwrap();
    let sources = SourceSetRevision::try_for_identities([source.identity()]).unwrap();
    DialogueProfileRevision::from_admitted_parts(
        source.identity().clone(),
        sources,
        sources,
        ViewProgramId::try_new("view_program.cli_bundle_dialogue").unwrap(),
        AcceptedViewProgramRevision::try_from_bytes([0x48; 32]).unwrap(),
        ResourceTypeRegistry::empty().digest(),
    )
}

fn collect_bundle_dsl_view_resources(document: &Arc<SourceDocument>) -> Result<(), ExitCode> {
    collect_bundle_dsl_view_resources_for_package(document, "local.test-package")
}

fn collect_bundle_dsl_view_resources_for_package(
    document: &Arc<SourceDocument>,
    package: &str,
) -> Result<(), ExitCode> {
    compile_bundle_fixture_project(document, package).map(|_| ())
}

#[expect(
    clippy::too_many_lines,
    reason = "the fixture constructs one complete typed project authority for bundle tests"
)]
fn compile_bundle_fixture_project(
    document: &Arc<SourceDocument>,
    package: &str,
) -> Result<CompiledProject, ExitCode> {
    let mut syntax = SyntaxDatabase::try_new().map_err(|error| {
        eprintln!("error: failed to create test syntax session: {error}");
        ExitCode::FAILURE
    })?;
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(document.display_name().clone()),
            Arc::clone(document),
            ParseOptions::default(),
        )
        .map_err(|error| {
            eprintln!("error: failed to bind test View source: {error}");
            ExitCode::FAILURE
        })?;
    let parsed_sources = std::collections::BTreeMap::from([(
        arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root(),
        parsed,
    )]);
    let package_spec = PackageSpec {
        id: PackageId::new(package).map_err(|error| {
            eprintln!("error: invalid test package ID: {error}");
            ExitCode::FAILURE
        })?,
        version: PackageVersion::new("0.0.0").map_err(|error| {
            eprintln!("error: invalid test package version: {error}");
            ExitCode::FAILURE
        })?,
    };
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        package_spec,
        BuildSpec::default(),
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!("arcweft-test://{package}/manifest")).map_err(
                    |error| {
                        eprintln!("error: invalid test manifest source ID: {error}");
                        ExitCode::FAILURE
                    },
                )?,
                SourceName::path("arcw.toml"),
                "",
            )
            .map_err(|error| {
                eprintln!("error: invalid test manifest source: {error}");
                ExitCode::FAILURE
            })?,
        ),
        [ProjectSourceFile::new(
            arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root(),
            PathBuf::from("main.arcw"),
            Arc::clone(document),
            [],
        )],
    )
    .map_err(|error| {
        eprintln!("error: failed to build test project sources: {error}");
        ExitCode::FAILURE
    })?;
    let package = CallablePackageId::try_new(project.package().id.as_str()).map_err(|error| {
        eprintln!("error: invalid callable package ID: {error}");
        ExitCode::FAILURE
    })?;
    let world = ProjectSymbolWorldId::try_new(
        package,
        document.identity().id().clone(),
        "cli-view-product-test",
    )
    .map_err(|error| {
        eprintln!("error: invalid test semantic world: {error}");
        ExitCode::FAILURE
    })?;
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(document)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| {
        eprintln!("error: failed to build test registration facts: {error:?}");
        ExitCode::FAILURE
    })?;
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(ResourceTypeRegistry::empty()),
        None,
        None,
    );
    let mut session = ProjectCompilationSession::try_new().map_err(|error| {
        eprintln!("error: failed to create test compiler session: {error}");
        ExitCode::FAILURE
    })?;
    let compiled =
        compile_project(&mut session, &project, &parsed_sources, &context).map_err(|error| {
            eprintln!("error: failed to compile the test View project: {error:?}");
            ExitCode::FAILURE
        })?;
    Ok(compiled)
}

#[test]
fn launch_profile_compiles_without_enumerating_default_source_root() {
    let unique = format!(
        "arcweft-bundle-package-identity-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock follows epoch")
            .as_nanos()
    );
    let root = std::env::temp_dir().join(unique);
    fs::create_dir_all(&root).expect("fixture root creates");
    let manifest_path = root.join("arcw.toml");
    fs::write(
        &manifest_path,
        r#"
schema = 1
default-profile = "main"

[package]
id = "org.arcweft.test.launch-only"
version = "0.1.0"

[profiles.main]
kind = "cli"
entry = "@entry.main"
source = "demo.arcw"
"#,
    )
    .expect("fixture manifest writes");
    fs::write(
        root.join("demo.arcw"),
        "entry cli @entry.main { goto @flow.main }\nflow main { return () }",
    )
    .expect("profile source writes");

    let selection = resolve_source_selection(
        None,
        &ProfileOptions {
            profile: Some("main".to_owned()),
            manifest: manifest_path,
        },
    )
    .expect("profile resolves");
    assert_eq!(
        selection
            .package_identity()
            .expect("package identity resolves"),
        "org.arcweft.test.launch-only"
    );
    super::super::project::load_and_check_selection(&selection, None)
        .expect("launch profile compiles its selected source directly");

    fs::remove_dir_all(root).expect("fixture root removes");
}

#[test]
fn view_scroll_rejects_both_axis_authoring() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(
                "arcweft-test://cli/bundle/view-scroll-rejects-both-axis-authoring",
            )
            .expect("fixture document ID"),
            SourceName::path("view-scroll-rejects-both-axis-authoring.arcw"),
            r#"
view BothAxisScroll() {
  Scroll(axis = .both, width = 120px, height = 72px) {
    Text("One")
  }
}

flow test {
  view(@view:.BothAxisScroll)
}
"#,
        )
        .expect("fixture source document"),
    );

    assert!(collect_bundle_dsl_view_resources(&document).is_err());
}

#[test]
fn view_style_rule_rejects_interactive_overflow_on_non_scroll_element() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://cli/bundle/view-style-rule-rejects-interactive-overflow-on-non-scroll-element")
                .expect("fixture document ID"),
            SourceName::path("view-style-rule-rejects-interactive-overflow-on-non-scroll-element.arcw"),
        r#"
style invalid_button_scroll {
  Button {
    overflow-x = .Auto
  }
}

view Actions() {
  Button(@button:.send, label = "Send")
}

flow test {
  view(@view:.Actions)
}
"#,
        )
        .expect("fixture source document"),
    );

    assert!(collect_bundle_dsl_view_resources(&document).is_err());
}

#[test]
fn bundle_hydrates_default_view_localization_from_matching_content_text_key() {
    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-test://cli/bundle/dialogue-localization")
            .expect("source ID"),
        SourceName::Memory,
        "夢",
    )
    .expect("source document");
    let document = RichTextDocument::new(vec![RichTextNode::Ruby {
        base: "夢".to_owned(),
        ruby: "ゆめ".to_owned(),
    }]);
    let dialogue_content =
        DialogueContentCatalog::try_from_records(vec![DialogueContentSpec::new(
            RuntimeLineId::from_runtime_line_value("say.localization.display").unwrap(),
            TextKey::try_new("text.opening.dream").expect("text key"),
            document.clone(),
            test_character_plan(),
            arcweft_text_model::DialoguePresentationSnapshot::new(
                DialoguePresentationProfile::engine_default(),
                test_dialogue_profile_revision(),
            ),
            Vec::new(),
            ProductSourceRef::try_for_identity(source.identity()).expect("product source"),
        )])
        .expect("dialogue content catalog is canonical");
    let mut text = ViewTextResource {
        sources: vec![arcweft_bundle::resource_codec::view::ViewTextSourceRecord {
            public_id: "text.view.dream".to_owned(),
            kind: arcweft_bundle::resource_codec::view::ViewTextSourceKind::Localized {
                key: "text.opening.dream".to_owned(),
                locale: None,
            },
            source: None,
        }],
        ..ViewTextResource::default()
    };

    hydrate_default_view_localization(&mut text, &dialogue_content);

    assert_eq!(
        text.localized_document("text.opening.dream", None),
        Some(&document)
    );
}

fn sample_image_virtual_file(path: &str) -> BundleVirtualFile {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("samples")
            .join("assets")
            .join(path),
    )
    .expect("sample image asset is readable");
    BundleVirtualFile {
        space: BundleVirtualFileSpace::Asset,
        path: path.to_owned(),
        bytes,
    }
}

#[test]
fn project_bundle_uses_schema_one_asset_root_and_project_local_state() {
    let unique = format!(
        "arcweft-project-resource-roots-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after UNIX epoch")
            .as_nanos()
    );
    let root = std::env::temp_dir().join(unique);
    let source_root = root.join("src");
    let asset_root = root.join("assets").join("bg");
    let state_root = root.join(".arcweft").join("save");
    fs::create_dir_all(&source_root).expect("temporary project source directory");
    fs::create_dir_all(&asset_root).expect("project asset directory");
    fs::create_dir_all(&state_root).expect("project state directory");
    fs::create_dir_all(source_root.join(".arcweft/save")).expect("source-local legacy state");
    let manifest_path = root.join("arcw.toml");
    let source_path = source_root.join("main.arcw");
    fs::write(
        &manifest_path,
        r#"
schema = 1

[package]
id = "org.arcweft.test.resource-root-builder"
version = "0.1.0"

[build]
source-dir = "src"
"#,
    )
    .expect("temporary manifest writes");
    fs::write(
        &source_path,
        r#"
entry cli @entry.main { goto @flow.main }

flow main() -> String { return "done" }
"#,
    )
    .expect("temporary project source writes");
    fs::write(
        asset_root.join("room.png"),
        sample_image_virtual_file("bg/room.png").bytes,
    )
    .expect("custom asset writes");
    fs::write(state_root.join("slot.txt"), "project-state").expect("project state writes");
    fs::write(source_root.join(".arcweft/save/legacy.txt"), "legacy-state")
        .expect("legacy state writes");
    let selection = crate::app::project::resolve_project_root_source_selection(&manifest_path)
        .expect("project selection loads");
    let mut phases = Vec::new();

    let artifact = compile_bundle_for_selection(
        &selection,
        vec![BundleVirtualFileSpace::Asset, BundleVirtualFileSpace::Save],
        &mut phases,
    )
    .expect("project bundle uses schema-one roots");

    assert!(
        artifact
            .bundle
            .virtual_file(&BundleVirtualFileRef {
                space: BundleVirtualFileSpace::Asset,
                path: "bg/room.png".to_owned(),
            })
            .is_some()
    );
    assert!(
        artifact
            .bundle
            .virtual_file(&BundleVirtualFileRef {
                space: BundleVirtualFileSpace::Save,
                path: "slot.txt".to_owned(),
            })
            .is_some()
    );
    assert!(
        artifact
            .bundle
            .virtual_file(&BundleVirtualFileRef {
                space: BundleVirtualFileSpace::Save,
                path: "legacy.txt".to_owned(),
            })
            .is_none()
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn collect_bundle_image_assets_decodes_static_and_animated_webp_metadata() {
    let assets = collect_bundle_image_assets(&[
        sample_image_virtual_file("bg/poster.webp"),
        sample_image_virtual_file("bg/loop.webp"),
    ])
    .expect("sample image assets decode");

    let poster = assets
        .iter()
        .find(|asset| asset.id == "asset.bg.poster")
        .expect("static webp asset is collected");
    assert_eq!(poster.format, BundleImageFormat::WebP);
    assert_eq!(poster.animation, BundleImageAnimation::Static);
    assert!(poster.dimensions.is_some());

    let loop_asset = assets
        .iter()
        .find(|asset| asset.id == "asset.bg.loop")
        .expect("animated webp asset is collected");
    assert_eq!(loop_asset.format, BundleImageFormat::WebP);
    assert_eq!(loop_asset.animation, BundleImageAnimation::Animated);
    assert!(loop_asset.dimensions.is_some());
}

#[test]
fn collect_bundle_image_assets_rejects_invalid_stable_identity_components() {
    let mut file = sample_image_virtual_file("bg/poster.webp");
    file.path = "bg/main menu.webp".to_owned();
    assert!(collect_bundle_image_assets(&[file]).is_err());
}

#[test]
fn collect_bundle_image_assets_rejects_normalized_identity_collisions() {
    let mut dashed = sample_image_virtual_file("bg/poster.webp");
    dashed.path = "ui/main-menu.webp".to_owned();
    let mut underscored = sample_image_virtual_file("bg/room.png");
    underscored.path = "ui/main_menu.png".to_owned();
    assert!(collect_bundle_image_assets(&[dashed, underscored]).is_err());

    let mut uppercase = sample_image_virtual_file("bg/poster.webp");
    uppercase.path = "images/Hero.webp".to_owned();
    let mut lowercase = sample_image_virtual_file("bg/room.png");
    lowercase.path = "images/hero.png".to_owned();
    assert!(collect_bundle_image_assets(&[uppercase, lowercase]).is_err());
}
