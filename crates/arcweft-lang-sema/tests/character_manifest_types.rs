use std::{collections::HashSet, sync::Arc};

use arcweft_character::{
    id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId},
    manifest::{
        CharacterAssetPath, CharacterBlendMode, CharacterCanvas, CharacterLook, CharacterManifest,
        CharacterPart, CharacterPartSelection, CharacterPoint, CharacterRect, CharacterVariant,
        registration::{
            CharacterManifestRootField, CharacterManifestTokenPath, SourceBackedCharacterManifest,
        },
    },
    registration_catalog::SourceBackedCharacterCatalog,
};
use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{
        CallablePackageId, ExternalDeclarationSeed, ProjectDirectBinding, ProjectSymbolWorldId,
    },
};
use arcweft_lang_sema::{
    env::TypeCheckEnv,
    registration::{
        CharacterRegistrar, CharacterRegistrationRequest, ExternalRegistrationFact,
        ProjectRegistrationFacts, RegisteredExternalOwner, RegisteredSemanticWorld,
    },
    types::{CharacterNominalFamily, CharacterNominalType, TypeKind},
};
use arcweft_lang_syntax::{
    ast::{
        common::Visibility,
        module_path::{CanonicalModulePath, ModulePathRoot},
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
    },
    parser::parse_source,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSpan};

fn manifest(character: &str) -> CharacterManifest {
    let body = CharacterPartId::try_new("body").expect("part");
    let uniform = CharacterVariantId::try_new("uniform").expect("variant");
    let normal = CharacterLookId::try_new("normal").expect("look");
    let smile = CharacterLookId::try_new("smile").expect("look");
    CharacterManifest::new(
        CharacterId::try_new(character).expect("character"),
        CharacterCanvas::new(32, 64),
        CharacterPoint::new(16, 64),
        normal.clone(),
        vec![CharacterPart::new(
            body.clone(),
            0,
            vec![CharacterVariant::new(
                uniform.clone(),
                CharacterAssetPath::try_new(format!(
                    "layers/{}-body.png",
                    character.trim_start_matches("character.")
                ))
                .expect("path"),
                CharacterRect::new(0, 0, 32, 64),
                u8::MAX,
                CharacterBlendMode::Normal,
                false,
            )],
        )],
        vec![
            CharacterLook::new(
                normal,
                vec![CharacterPartSelection::new(body.clone(), uniform.clone())],
            ),
            CharacterLook::new(smile, vec![CharacterPartSelection::new(body, uniform)]),
        ],
        None,
    )
    .expect("manifest")
}

fn character_direct_bindings(
    owner: &CharacterId,
    declaration: &SourceSpan,
) -> Vec<ProjectDirectBinding> {
    let compact_segments = owner
        .compact_segments()
        .map(|segment| {
            ProjectSymbolSegment::try_new(segment).expect("character compact segment is valid")
        })
        .collect::<Vec<_>>();
    [
        ProjectSymbolPath::new(
            ModulePathRoot::ImplicitCrate,
            std::iter::once(
                ProjectSymbolSegment::try_new("character")
                    .expect("character namespace segment is valid"),
            )
            .chain(compact_segments.iter().cloned()),
        )
        .expect("qualified character binding path"),
        ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, compact_segments)
            .expect("compact character binding path"),
    ]
    .into_iter()
    .map(|path| {
        ProjectDirectBinding::try_new(
            CanonicalModulePath::crate_root(),
            path,
            Some(Visibility::Public),
            declaration.clone(),
            false,
        )
        .expect("direct binding")
    })
    .collect()
}

fn register(manifests: &[CharacterManifest]) -> RegisteredSemanticWorld {
    let package = CallablePackageId::try_new("character-tests").expect("package");
    let source = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("memory:///main.arcw").expect("source id"),
            SourceName::path("memory:///main.arcw"),
            "",
        )
        .expect("source document"),
    );
    let parsed = parse_source("");
    let hir = lower_document_to_hir(&source, parsed.typed_tree()).expect("source lowers");
    let project = HirProject::new(
        package.as_str(),
        [HirProjectModule::try_new(
            CanonicalModulePath::crate_root(),
            source.identity().clone(),
            hir,
        )
        .expect("character manifest fixture module binding")],
    )
    .expect("HIR project");
    let world = ProjectSymbolWorldId::try_new(package, source.identity().id().clone(), "default")
        .expect("symbol world");

    let mut documents = vec![source.clone()];
    let mut source_backed = Vec::new();
    let mut externals = Vec::new();
    for (index, manifest) in manifests.iter().enumerate() {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!("memory:///character-{index}.json"))
                    .expect("manifest id"),
                SourceName::path(format!("memory:///character-{index}.json")),
                manifest.to_json_pretty().expect("manifest JSON"),
            )
            .expect("manifest document"),
        );
        let backed = SourceBackedCharacterManifest::decode_registration_json(&document)
            .expect("source-backed manifest");
        let owner = backed.manifest().character().clone();
        let declaration = backed
            .source_map()
            .token(&CharacterManifestTokenPath::Root(
                CharacterManifestRootField::Character,
            ))
            .expect("character token")
            .value()
            .clone();
        let seed = ExternalDeclarationSeed::try_new(
            SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), owner.as_str())
                .expect("character symbol path"),
            Some(Visibility::Public),
            declaration.clone(),
            character_direct_bindings(&owner, &declaration),
        )
        .expect("external declaration");
        externals.push(ExternalRegistrationFact::new(
            seed,
            RegisteredExternalOwner::Character(owner),
            declaration,
        ));
        documents.push(document);
        source_backed.push(backed);
    }
    let catalogs = vec![
        SourceBackedCharacterCatalog::try_new(source.identity().clone(), source_backed)
            .expect("source-backed catalog"),
    ];
    let facts =
        ProjectRegistrationFacts::try_new(world, documents, externals, catalogs, Vec::new())
            .expect("registration facts");
    CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::default()),
        &project,
        &facts,
        None,
    ))
    .expect("character registration")
}

#[test]
fn character_nominal_family_is_the_only_classifier() {
    let character = CharacterId::try_new("character.akane").expect("character");
    let part = CharacterPartId::try_new("body").expect("part");
    let cases = [
        (
            CharacterNominalType::Look {
                character: character.clone(),
            },
            CharacterNominalFamily::Look,
            None,
        ),
        (
            CharacterNominalType::Part {
                character: character.clone(),
            },
            CharacterNominalFamily::Part,
            None,
        ),
        (
            CharacterNominalType::Variant {
                character: character.clone(),
                part: part.clone(),
            },
            CharacterNominalFamily::Variant,
            Some(&part),
        ),
    ];

    for (nominal, family, expected_part) in cases {
        assert_eq!(nominal.family(), family);
        assert_eq!(nominal.character(), &character);
        assert_eq!(nominal.part(), expected_part);
    }
}

#[test]
fn structural_identity_ignores_display_labels_and_provenance() {
    let owner = CharacterId::try_new("character.akane").expect("owner");
    let nominal = TypeKind::character_look(owner);
    let identical = nominal.clone();

    assert_eq!(nominal, identical);
    assert_eq!(HashSet::from([nominal.clone(), identical.clone()]).len(), 1);
    assert_ne!(nominal, TypeKind::Named(identical.source_label()));
}

#[test]
fn aliases_do_not_change_nominal_identity() {
    let manifest = manifest("character.akane");
    let owner = manifest.character().clone();
    let registered = register(&[manifest]);
    let module = CanonicalModulePath::crate_root();
    let source = registered
        .symbols()
        .external_symbols()
        .next()
        .expect("external")
        .declaration_span()
        .clone();
    let canonical = SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), owner.as_str())
        .expect("canonical path");
    let compact = SymbolPath::try_new(
        ModulePathRoot::ImplicitCrate,
        Vec::new(),
        owner.compact_str(),
    )
    .expect("compact path");

    let canonical_owner = registered
        .environment()
        .resolve_character_owner(registered.symbols(), &module, &canonical, &source)
        .expect("canonical owner");
    let compact_owner = registered
        .environment()
        .resolve_character_owner(registered.symbols(), &module, &compact, &source)
        .expect("compact owner");
    assert_eq!(canonical_owner, compact_owner);
    assert_eq!(
        TypeKind::character_look(canonical_owner),
        TypeKind::character_look(compact_owner)
    );
}

#[test]
fn registers_manifest_enums_on_structural_nominal_types() {
    let manifest = manifest("character.akane");
    let character = manifest.character().clone();
    let registered = register(core::slice::from_ref(&manifest));
    let environment = registered.environment();

    assert_eq!(
        environment
            .character_manifest(&character)
            .expect("registered character")
            .default_look()
            .as_str(),
        "normal"
    );
    let look = TypeKind::character_look(character.clone());
    assert_eq!(look.source_label(), "CharacterLook<character.akane>");
    assert_eq!(look.to_string(), "CharacterLook<character.akane>");
    assert!(matches!(
        look.character_nominal(),
        Some(CharacterNominalType::Look { character: owner }) if owner == &character
    ));
    assert_eq!(
        look.character_nominal().map(CharacterNominalType::family),
        Some(CharacterNominalFamily::Look)
    );
    assert_eq!(
        environment
            .character_enum_variants(look.character_nominal().expect("nominal look"))
            .expect("look variants")
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["normal", "smile"]
    );
}

#[test]
fn equal_member_spellings_preserve_character_family_and_part_identity() {
    let akane_manifest = manifest("character.akane");
    let aoi_manifest = manifest("character.aoi");
    let akane = akane_manifest.character().clone();
    let aoi = aoi_manifest.character().clone();
    let body = CharacterPartId::try_new("body").expect("part");
    let registered = register(&[akane_manifest, aoi_manifest]);

    let akane_look = TypeKind::character_look(akane.clone());
    let aoi_look = TypeKind::character_look(aoi.clone());
    let akane_part = TypeKind::character_part(akane.clone());
    let akane_body = TypeKind::character_variant(akane.clone(), body.clone());
    let aoi_body = TypeKind::character_variant(aoi.clone(), body);

    assert_ne!(akane_look, aoi_look);
    assert_ne!(akane_look, akane_part);
    assert_ne!(akane_body, aoi_body);
    assert!(
        registered
            .environment()
            .character_manifest(&akane)
            .is_some()
    );
    assert!(registered.environment().character_manifest(&aoi).is_some());
    assert_eq!(
        HashSet::from([akane_look.clone(), akane_look.clone()]).len(),
        1
    );
    assert_ne!(
        TypeKind::function([akane_look], akane_body),
        TypeKind::function([aoi_look], aoi_body)
    );
}
