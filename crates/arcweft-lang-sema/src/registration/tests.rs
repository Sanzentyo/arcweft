use std::{collections::BTreeMap, fmt::Write as _, sync::Arc};

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
    symbol::CharacterSymbolDescriptor,
};
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_syntax::ast::{
    module_path::{CanonicalModulePath, ModulePathRoot, ModuleSegment},
    symbol_path::SymbolPath,
};
use arcweft_source::{SourceDocument, SourceRange};

use crate::test_support::character_project::{
    PACKAGE, backed_manifest, declaration_span, external_fact, one_character_facts,
    one_character_facts_with_documents, project_modules, register, root_project,
    root_project_source, sample_manifest, sample_manifest_for, source_document,
};
use crate::{env::TypeCheckEnv, types::TypeKind};

use super::model::{RegistrationDocumentView, registration_document_diagnostics};
use super::registrar::{charge, merge_manifest_occurrence};
use super::{
    CharacterInventoryDigest, CharacterInventoryIntegrityError, CharacterInventoryRevision,
    CharacterRegistrationCode, CharacterRegistrationDiagnostic,
    CharacterRegistrationDiagnosticKind, CharacterRegistrationLimitKind,
    CharacterRegistrationLimits, CharacterRegistrationReport, EnvironmentBindingId,
    ExternalOwnerLookupError, ExternalRegistrationFact, ProjectRegistrationFacts,
    RegisteredExternalOwner, RegisteredExternalOwnerKind, RegisteredSemanticWorld,
};

fn reordered_manifest(reverse: bool) -> CharacterManifest {
    let owner = CharacterId::try_new("character.akane").expect("character");
    let body = CharacterPartId::try_new("body").expect("body part");
    let face = CharacterPartId::try_new("face").expect("face part");
    let default = CharacterVariantId::try_new("default").expect("default variant");
    let alternate = CharacterVariantId::try_new("alternate").expect("alternate variant");
    let normal = CharacterLookId::try_new("normal").expect("normal look");
    let happy = CharacterLookId::try_new("happy").expect("happy look");
    let variant = |id: CharacterVariantId, asset: &str| {
        CharacterVariant::new(
            id,
            CharacterAssetPath::try_new(asset).expect("asset"),
            CharacterRect::new(0, 0, 64, 128),
            u8::MAX,
            CharacterBlendMode::Normal,
            false,
        )
    };
    let mut body_variants = vec![
        variant(default.clone(), "layers/body-default.png"),
        variant(alternate.clone(), "layers/body-alternate.png"),
    ];
    let mut face_variants = vec![
        variant(default.clone(), "layers/face-default.png"),
        variant(alternate.clone(), "layers/face-alternate.png"),
    ];
    let mut normal_selections = vec![
        CharacterPartSelection::new(body.clone(), default.clone()),
        CharacterPartSelection::new(face.clone(), default.clone()),
    ];
    let mut happy_selections = vec![
        CharacterPartSelection::new(body.clone(), alternate.clone()),
        CharacterPartSelection::new(face.clone(), alternate),
    ];
    if reverse {
        body_variants.reverse();
        face_variants.reverse();
        normal_selections.reverse();
        happy_selections.reverse();
    }
    let mut parts = vec![
        CharacterPart::new(body, 0, body_variants),
        CharacterPart::new(face, 1, face_variants),
    ];
    let mut looks = vec![
        CharacterLook::new(normal.clone(), normal_selections),
        CharacterLook::new(happy, happy_selections),
    ];
    if reverse {
        parts.reverse();
        looks.reverse();
    }
    CharacterManifest::new(
        owner,
        CharacterCanvas::new(64, 128),
        CharacterPoint::new(32, 128),
        normal,
        parts,
        looks,
        None,
    )
    .expect("reordered manifest")
}

fn registered_character_and_environment(
    profile: &str,
) -> (RegisteredSemanticWorld, EnvironmentBindingId) {
    let (root, project, world) = root_project(profile);
    let manifest = sample_manifest("layers/body.png");
    let (manifest_document, backed) = backed_manifest(
        "arcweft-project://registration-tests/characters/mixed-owner.awchar.json",
        &manifest,
    );
    let character = manifest.character().clone();
    let character_fact = external_fact(
        character.as_str(),
        &[character.as_str(), character.compact_str()],
        RegisteredExternalOwner::Character(character.clone()),
        declaration_span(&backed),
    );
    let generated = source_document(
        "arcweft-generated://registration-tests/mixed-adapter",
        "adapter.viewport",
    );
    let environment = EnvironmentBindingId::try_new("adapter.viewport").expect("environment id");
    let environment_fact = external_fact(
        environment.as_str(),
        &[environment.as_str()],
        RegisteredExternalOwner::Environment(environment.clone()),
        generated
            .span(SourceRange::new(0, "adapter.viewport".len()))
            .expect("environment declaration"),
    );
    let catalog = SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![backed])
        .expect("catalog");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, manifest_document, generated],
        vec![character_fact, environment_fact],
        vec![catalog],
    )
    .expect("mixed registration facts");
    let base = TypeCheckEnv::standard().with_symbol(environment.as_str(), TypeKind::I32);
    (
        register(&project, &facts, base, None).expect("mixed world registers"),
        environment,
    )
}

fn registration_snapshot(
    registered: &RegisteredSemanticWorld,
) -> (
    ProjectSymbolWorldId,
    arcweft_lang_hir::symbol::ProjectSymbolRevision,
    Vec<arcweft_lang_hir::symbol::CallableDeclarationId>,
    Vec<String>,
    Vec<CharacterId>,
    CharacterInventoryDigest,
    CharacterInventoryRevision,
) {
    (
        registered.symbols().world().clone(),
        *registered.symbols().revision(),
        registered
            .symbols()
            .callable_symbols()
            .map(|symbol| symbol.declaration().clone())
            .collect(),
        registered
            .symbols()
            .external_symbols()
            .map(|symbol| symbol.canonical_path().canonical_string())
            .collect(),
        registered
            .environment()
            .characters()
            .map(|(owner, _)| owner.clone())
            .collect(),
        registered.environment().character_digest(),
        registered.environment().character_revision(),
    )
}

#[test]
fn complete_world_commits_once() {
    let (root, project, world) = root_project("complete");
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("complete world registers");

    assert_eq!(
        registered.symbols().world(),
        registered.environment().world()
    );
    assert_eq!(
        registered.symbols().revision(),
        registered.environment().symbol_revision()
    );
    assert_eq!(registered.environment().characters().len(), 1);
    let definitions = registered.character_definition_index();
    assert_eq!(definitions.world(), registered.symbols().world());
    assert_eq!(
        definitions.symbol_revision(),
        registered.symbols().revision()
    );
    assert_eq!(definitions.manifest_count(), 1);
    assert_eq!(definitions.len(), 4);
    assert_eq!(definitions.documents().len(), 1);
    let (_, _, consumed_definitions) = registered.clone().into_parts();
    assert_eq!(
        consumed_definitions.source_revision(),
        definitions.source_revision()
    );
    registered
        .environment()
        .verify_character_inventory(registered.symbols())
        .expect("registered descriptor verifies");
}

#[test]
fn same_facts_same_inventory() {
    let (root, project, world) = root_project("same-facts-inventory");
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));

    let first =
        register(&project, &facts, TypeCheckEnv::standard(), None).expect("first registration");
    let second = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("second registration from identical facts");

    assert_eq!(
        registration_snapshot(&first),
        registration_snapshot(&second)
    );
    assert_eq!(
        first.environment().character_descriptor,
        second.environment().character_descriptor
    );
}

fn sourced_manifest_inventory(
    count: usize,
) -> (Vec<Arc<SourceDocument>>, Vec<SourceBackedCharacterManifest>) {
    (0..count)
        .map(|index| {
            backed_manifest(
                &format!(
                    "arcweft-project://registration-tests/characters/owner{index:04}.awchar.json"
                ),
                &sample_manifest_for(
                    &format!("character.owner{index:04}"),
                    &format!("layers/owner{index:04}.png"),
                ),
            )
        })
        .unzip()
}

fn assert_registration_limit(
    report: &CharacterRegistrationReport,
    expected_kind: CharacterRegistrationLimitKind,
    expected_observed: u64,
    expected_maximum: u64,
) {
    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind(),
        CharacterRegistrationDiagnosticKind::Limit {
            kind,
            observed,
            maximum,
        } if *kind == expected_kind
            && *observed == expected_observed
            && *maximum == expected_maximum
    )));
}

#[test]
fn limit_catalogs_exact_and_one_over() {
    let (root, project, world) = root_project("catalog-limit");
    let maximum = usize::try_from(CharacterRegistrationLimits::PRODUCTION.catalogs())
        .expect("catalog limit fits usize");
    let catalogs = (0..maximum)
        .map(|_| {
            SourceBackedCharacterCatalog::try_new(root.identity().clone(), Vec::new())
                .expect("empty source-backed catalog")
        })
        .collect::<Vec<_>>();
    let exact = ProjectRegistrationFacts::try_new(
        world.clone(),
        vec![Arc::clone(&root)],
        Vec::new(),
        catalogs.clone(),
    )
    .expect("exact catalog facts");
    register(&project, &exact, TypeCheckEnv::standard(), None)
        .expect("exact catalog limit is accepted");

    let mut one_over = catalogs;
    one_over.push(
        SourceBackedCharacterCatalog::try_new(root.identity().clone(), Vec::new())
            .expect("one-over empty catalog"),
    );
    let facts = ProjectRegistrationFacts::try_new(world, vec![root], Vec::new(), one_over)
        .expect("catalog count is enforced by the registrar");
    let report = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect_err("one-over catalog limit is rejected");
    assert_registration_limit(
        &report,
        CharacterRegistrationLimitKind::Catalogs,
        CharacterRegistrationLimits::PRODUCTION.catalogs() + 1,
        CharacterRegistrationLimits::PRODUCTION.catalogs(),
    );
}

#[test]
fn limit_occurrences_exact_and_one_over() {
    let (root, project, world) = root_project("occurrence-limit");
    let manifests = (0..17)
        .map(|index| {
            sample_manifest_for(
                &format!("character.owner{index:04}"),
                &format!("layers/owner{index:04}.png"),
            )
        })
        .collect::<Vec<_>>();
    let build_catalogs = |counts: &[usize]| {
        let mut documents = Vec::new();
        let catalogs = counts
            .iter()
            .enumerate()
            .map(|(catalog, &count)| {
                let backed = manifests[..count]
                    .iter()
                    .enumerate()
                    .map(|(owner, manifest)| {
                        let (document, backed) = backed_manifest(
                            &format!(
                                "arcweft-project://registration-tests/characters/occurrence-{catalog:02}-owner-{owner:02}.awchar.json"
                            ),
                            manifest,
                        );
                        documents.push(document);
                        backed
                    })
                    .collect();
                SourceBackedCharacterCatalog::try_new(root.identity().clone(), backed)
                    .expect("catalog with distinct owners and source occurrences")
            })
            .collect::<Vec<_>>();
        (documents, catalogs)
    };
    let (manifest_documents, catalogs) = build_catalogs(&vec![16; 64]);
    let mut documents = vec![Arc::clone(&root)];
    documents.extend(manifest_documents);
    let exact =
        ProjectRegistrationFacts::try_new(world.clone(), documents.clone(), Vec::new(), catalogs)
            .expect("exact occurrence facts");
    register(&project, &exact, TypeCheckEnv::standard(), None)
        .expect("exact occurrence limit is accepted");

    let mut one_over_counts = vec![16; 63];
    one_over_counts.push(17);
    let (manifest_documents, one_over_catalogs) = build_catalogs(&one_over_counts);
    let mut one_over_documents = vec![root];
    one_over_documents.extend(manifest_documents);
    let facts =
        ProjectRegistrationFacts::try_new(world, one_over_documents, Vec::new(), one_over_catalogs)
            .expect("occurrence count is enforced by registrar");
    let report = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect_err("one-over occurrence limit is rejected");
    assert_registration_limit(
        &report,
        CharacterRegistrationLimitKind::ManifestOccurrences,
        CharacterRegistrationLimits::PRODUCTION.manifest_occurrences() + 1,
        CharacterRegistrationLimits::PRODUCTION.manifest_occurrences(),
    );
}

#[test]
fn limit_owners_exact_and_one_over() {
    let (root, project, world) = root_project("owner-limit");
    let maximum = usize::try_from(CharacterRegistrationLimits::PRODUCTION.owners())
        .expect("owner limit fits usize");
    let (manifest_documents, manifests) = sourced_manifest_inventory(maximum + 1);
    let mut documents = vec![Arc::clone(&root)];
    documents.extend(manifest_documents);
    let exact_catalog = SourceBackedCharacterCatalog::try_new(
        root.identity().clone(),
        manifests[..maximum].to_vec(),
    )
    .expect("exact owner catalog");
    let exact = ProjectRegistrationFacts::try_new(
        world.clone(),
        documents.clone(),
        Vec::new(),
        vec![exact_catalog],
    )
    .expect("exact owner facts");
    register(&project, &exact, TypeCheckEnv::standard(), None)
        .expect("exact owner limit is accepted");

    let one_over_catalog =
        SourceBackedCharacterCatalog::try_new(root.identity().clone(), manifests)
            .expect("one-over owner catalog");
    let facts =
        ProjectRegistrationFacts::try_new(world, documents, Vec::new(), vec![one_over_catalog])
            .expect("owner count is enforced by registrar");
    let report = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect_err("one-over owner limit is rejected");
    assert_registration_limit(
        &report,
        CharacterRegistrationLimitKind::Owners,
        CharacterRegistrationLimits::PRODUCTION.owners() + 1,
        CharacterRegistrationLimits::PRODUCTION.owners(),
    );
}

#[test]
fn limit_documents_exact_and_one_over() {
    let (root, project, world) = root_project("document-limit");
    let maximum = usize::try_from(CharacterRegistrationLimits::PRODUCTION.documents())
        .expect("document limit fits usize");
    let mut documents = vec![Arc::clone(&root)];
    documents.extend((1..maximum).map(|index| {
        source_document(
            &format!("arcweft-generated://registration-tests/document-{index}"),
            "",
        )
    }));
    let exact =
        ProjectRegistrationFacts::try_new(world.clone(), documents.clone(), Vec::new(), Vec::new())
            .expect("exact document facts");
    register(&project, &exact, TypeCheckEnv::standard(), None)
        .expect("exact document limit is accepted");

    documents.push(source_document(
        "arcweft-generated://registration-tests/document-one-over",
        "",
    ));
    let report = ProjectRegistrationFacts::try_new(world, documents, Vec::new(), Vec::new())
        .expect_err("one-over document limit is rejected");
    assert_registration_limit(
        &report,
        CharacterRegistrationLimitKind::Documents,
        CharacterRegistrationLimits::PRODUCTION.documents() + 1,
        CharacterRegistrationLimits::PRODUCTION.documents(),
    );
}

#[test]
fn registration_source_bytes_exact_and_one_over() {
    let (root, project, world) = root_project("source-byte-limit");
    let base = sample_manifest("layers/body.png")
        .to_json_pretty()
        .expect("base manifest JSON");
    let maximum = usize::try_from(CharacterRegistrationLimits::PRODUCTION.source_bytes())
        .expect("source-byte limit fits usize");
    let manifest_length = maximum - root.text().len();
    let mut exact_source = base.clone();
    exact_source.push_str(&" ".repeat(manifest_length - exact_source.len()));
    let exact_document = source_document(
        "arcweft-project://registration-tests/characters/source-bytes-exact.json",
        exact_source,
    );
    let exact_manifest = SourceBackedCharacterManifest::decode_registration_json(&exact_document)
        .expect("exact source-byte manifest decodes");
    let exact_catalog =
        SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![exact_manifest])
            .expect("exact source-byte catalog");
    let exact = ProjectRegistrationFacts::try_new(
        world.clone(),
        vec![Arc::clone(&root), exact_document],
        Vec::new(),
        vec![exact_catalog],
    )
    .expect("exact source-byte facts");
    register(&project, &exact, TypeCheckEnv::standard(), None)
        .expect("exact source-byte limit is accepted");

    let mut one_over_source = base;
    one_over_source.push_str(&" ".repeat(manifest_length + 1 - one_over_source.len()));
    let one_over_document = source_document(
        "arcweft-project://registration-tests/characters/source-bytes-one-over.json",
        one_over_source,
    );
    let one_over_manifest =
        SourceBackedCharacterManifest::decode_registration_json(&one_over_document)
            .expect("per-document byte limit is not exceeded");
    let one_over_catalog =
        SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![one_over_manifest])
            .expect("one-over source-byte catalog");
    let report = ProjectRegistrationFacts::try_new(
        world,
        vec![root, one_over_document],
        Vec::new(),
        vec![one_over_catalog],
    )
    .expect_err("one-over aggregate source-byte limit is rejected");
    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind(),
        CharacterRegistrationDiagnosticKind::ManifestBytesLimit { observed, maximum }
            if *observed == CharacterRegistrationLimits::PRODUCTION.source_bytes() + 1
                && *maximum == CharacterRegistrationLimits::PRODUCTION.source_bytes()
    )));
}

#[test]
fn hard_limit_values_have_one_public_owner() {
    let limits = CharacterRegistrationLimits::PRODUCTION;
    let manifest_limits = arcweft_character::manifest::limits::CharacterManifestLimits::PRODUCTION;
    let symbol_limits = arcweft_lang_hir::symbol::ProjectSymbolLimits::PRODUCTION;

    assert_eq!(
        limits.source_bytes(),
        arcweft_source::MAX_REGISTRATION_SOURCE_BYTES
    );
    assert_eq!(limits.parts(), manifest_limits.parts());
    assert_eq!(
        limits.variants_per_part(),
        manifest_limits.variants_per_part()
    );
    assert_eq!(
        limits.variants_per_manifest(),
        manifest_limits.variants_per_manifest()
    );
    assert_eq!(limits.looks(), manifest_limits.looks());
    assert_eq!(limits.selections(), manifest_limits.selections());
    assert_eq!(limits.diagnostics(), symbol_limits.diagnostics());
    assert_eq!(limits.work(), symbol_limits.work());
}

#[test]
fn source_digest_collision_rejects_equal_identity_unequal_bytes() {
    let document = source_document(
        "arcweft-project://registration-tests/source-digest-collision.arcw",
        "abc",
    );
    let primary = document
        .span(SourceRange::new(0, document.text().len()))
        .expect("full span");
    let views = [
        RegistrationDocumentView::new(&document),
        RegistrationDocumentView::with_injected_text(document.identity(), "abd", primary),
    ];

    let diagnostics = registration_document_diagnostics(&views);

    assert!(matches!(
        diagnostics.as_slice(),
        [diagnostic]
            if diagnostic.kind()
                == &CharacterRegistrationDiagnosticKind::SourceDigestCollision {
                    id: document.identity().id().clone(),
                    revision: document.identity().revision(),
                }
    ));
}

#[test]
fn source_digest_collision_preserves_previous_registered_world() {
    let (root, project, world) = root_project("source-collision-preserves-world");
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let accepted = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("initial world registers");
    let before = registration_snapshot(&accepted);
    let primary = root
        .span(SourceRange::new(0, root.text().len()))
        .expect("full root span");
    let views = [
        RegistrationDocumentView::new(&root),
        RegistrationDocumentView::with_injected_text(
            root.identity(),
            "fn main() -> Unit { [] }\n",
            primary,
        ),
    ];

    let diagnostics = registration_document_diagnostics(&views);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code() == CharacterRegistrationCode::SourceDigestCollision));
    assert_eq!(registration_snapshot(&accepted), before);
}

#[test]
fn inventory_descriptor_v1_fixed_vector() {
    let (root, project, world) = root_project("digest");
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let registered =
        register(&project, &facts, TypeCheckEnv::standard(), None).expect("world registers");
    assert_eq!(
        hex(registered.environment().character_digest().as_bytes()),
        "ca1d4a15f15454289fd0334310088513c5f884ab19684c768e519b98ccd0f0f3"
    );
    assert_eq!(
        super::descriptor::descriptor_canonical_len(&registered.environment().character_descriptor),
        143
    );
}

#[test]
fn inventory_descriptor_excludes_aliases_base_and_world() {
    let (root, project, first_world) =
        root_project_source("descriptor-exclusions-a", "fn main() -> Unit { () }\n");
    let first_facts = one_character_facts(&root, first_world, &sample_manifest("layers/body.png"));
    let first =
        register(&project, &first_facts, TypeCheckEnv::standard(), None).expect("first descriptor");

    let second_world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(PACKAGE).expect("package"),
        root.identity().id().clone(),
        "descriptor-exclusions-b",
    )
    .expect("second world");
    let manifest = sample_manifest("layers/body.png");
    let (document, backed) = backed_manifest(
        "arcweft-project://registration-tests/characters/descriptor-exclusions.awchar.json",
        &manifest,
    );
    let owner = manifest.character().clone();
    let fact = external_fact(
        owner.as_str(),
        &[owner.as_str(), owner.compact_str(), "hero"],
        RegisteredExternalOwner::Character(owner.clone()),
        declaration_span(&backed),
    );
    let catalog = SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![backed])
        .expect("catalog");
    let second_facts = ProjectRegistrationFacts::try_new(
        second_world,
        vec![Arc::clone(&root), document],
        vec![fact],
        vec![catalog],
    )
    .expect("second facts");
    let changed_base = TypeCheckEnv::standard().with_symbol("unrelated", TypeKind::String);
    let second = register(&project, &second_facts, changed_base, None).expect("second descriptor");

    assert_eq!(
        first.environment().character_digest(),
        second.environment().character_digest()
    );
    assert_eq!(
        first.environment().character_descriptor,
        second.environment().character_descriptor
    );
}

#[test]
fn inventory_descriptor_observes_character_owner_path() {
    let (root, project, world) = root_project("descriptor-path");
    let manifest = sample_manifest("layers/body.png");
    let first_facts = one_character_facts(&root, world.clone(), &manifest);
    let first = register(&project, &first_facts, TypeCheckEnv::standard(), None)
        .expect("canonical descriptor");

    let (document, backed) = backed_manifest(
        "arcweft-project://registration-tests/characters/descriptor-path.awchar.json",
        &manifest,
    );
    let owner = manifest.character().clone();
    let changed_fact = external_fact(
        "cast.akane",
        &[owner.as_str(), owner.compact_str()],
        RegisteredExternalOwner::Character(owner.clone()),
        declaration_span(&backed),
    );
    let catalog = SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![backed])
        .expect("catalog");
    let changed_facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, document],
        vec![changed_fact],
        vec![catalog],
    )
    .expect("changed path facts");
    let changed = register(&project, &changed_facts, TypeCheckEnv::standard(), None)
        .expect("changed path descriptor");

    assert_ne!(
        first.environment().character_digest(),
        changed.environment().character_digest()
    );
    assert_ne!(
        first.environment().character_descriptor,
        changed.environment().character_descriptor
    );
}

#[test]
fn character_external_owner_lookup() {
    let (root, project, world) = root_project("character-owner");
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let registered =
        register(&project, &facts, TypeCheckEnv::standard(), None).expect("world registers");
    let declaration = registered
        .symbols()
        .external_symbols()
        .next()
        .expect("character external")
        .declaration();

    assert!(matches!(
        registered.environment().external_owner(
            registered.symbols(),
            declaration,
            RegisteredExternalOwnerKind::Character,
        ),
        Ok(RegisteredExternalOwner::Character(owner)) if owner.as_str() == "character.akane"
    ));
    assert!(matches!(
        registered.environment().external_owner(
            registered.symbols(),
            declaration,
            RegisteredExternalOwnerKind::Environment,
        ),
        Err(ExternalOwnerLookupError::WrongKind {
            expected: RegisteredExternalOwnerKind::Environment,
            actual: RegisteredExternalOwnerKind::Character,
            ..
        })
    ));
}

#[test]
fn external_lookup_unknown_id() {
    let (registered, _) = registered_character_and_environment("external-unknown-id");
    let declaration = registered
        .symbols()
        .external_symbols()
        .find(|symbol| symbol.canonical_path().leaf() == "character.akane")
        .expect("character external")
        .declaration();
    let mut missing = registered.environment().clone();
    assert!(
        missing
            .external_owners
            .owners
            .remove(&declaration)
            .is_some()
    );

    assert_eq!(
        missing.external_owner(
            registered.symbols(),
            declaration,
            RegisteredExternalOwnerKind::Character,
        ),
        Err(ExternalOwnerLookupError::Unknown { declaration })
    );
}

#[test]
fn external_lookup_wrong_kind() {
    let (registered, _) = registered_character_and_environment("external-wrong-kind");
    let character = registered
        .symbols()
        .external_symbols()
        .find(|symbol| symbol.canonical_path().leaf() == "character.akane")
        .expect("character external")
        .declaration();
    let environment = registered
        .symbols()
        .external_symbols()
        .find(|symbol| symbol.canonical_path().leaf() == "adapter.viewport")
        .expect("environment external")
        .declaration();

    assert!(matches!(
        registered.environment().external_owner(
            registered.symbols(),
            character,
            RegisteredExternalOwnerKind::Environment,
        ),
        Err(ExternalOwnerLookupError::WrongKind {
            declaration,
            expected: RegisteredExternalOwnerKind::Environment,
            actual: RegisteredExternalOwnerKind::Character,
        }) if declaration == character
    ));
    assert!(matches!(
        registered.environment().external_owner(
            registered.symbols(),
            environment,
            RegisteredExternalOwnerKind::Character,
        ),
        Err(ExternalOwnerLookupError::WrongKind {
            declaration,
            expected: RegisteredExternalOwnerKind::Character,
            actual: RegisteredExternalOwnerKind::Environment,
        }) if declaration == environment
    ));
}

#[test]
fn external_typed_key_swap() {
    let (registered, _) = registered_character_and_environment("external-key-swap");
    let environment = registered
        .symbols()
        .external_symbols()
        .find(|symbol| symbol.canonical_path().leaf() == "adapter.viewport")
        .expect("environment external")
        .declaration();
    let mut missing = registered.environment().clone();
    assert!(
        missing
            .external_owners
            .owners
            .remove(&environment)
            .is_some()
    );

    assert_eq!(
        missing.external_owner(
            registered.symbols(),
            environment,
            RegisteredExternalOwnerKind::Environment,
        ),
        Err(ExternalOwnerLookupError::Unknown {
            declaration: environment,
        }),
        "a linked typed key is never recovered by reparsing its canonical spelling",
    );
}

#[test]
fn registration_facts_and_registry_share_one_owner_enum() {
    let (registered, environment) = registered_character_and_environment("shared-owner-enum");
    let declaration = registered
        .symbols()
        .external_symbols()
        .find(|symbol| symbol.canonical_path().leaf() == environment.as_str())
        .expect("environment external")
        .declaration();
    let expected = RegisteredExternalOwner::Environment(environment);

    assert_eq!(
        registered.environment().external_owner(
            registered.symbols(),
            declaration,
            RegisteredExternalOwnerKind::Environment,
        ),
        Ok(&expected)
    );
}

#[test]
fn external_lookup_stale_revision() {
    let (first_root, first_project, first_world) =
        root_project_source("same-world-stale", "fn main() -> Unit { () }\n");
    let first_facts = one_character_facts(
        &first_root,
        first_world,
        &sample_manifest("layers/body.png"),
    );
    let first = register(&first_project, &first_facts, TypeCheckEnv::standard(), None)
        .expect("first world registers");
    let declaration = first
        .symbols()
        .external_symbols()
        .next()
        .expect("character external")
        .declaration();

    let (changed_root, changed_project, changed_world) = root_project_source(
        "same-world-stale",
        "fn main() -> Unit { let changed = true; () }\n",
    );
    let changed_facts = one_character_facts(
        &changed_root,
        changed_world,
        &sample_manifest("layers/body.png"),
    );
    let changed = register(
        &changed_project,
        &changed_facts,
        TypeCheckEnv::standard(),
        None,
    )
    .expect("changed world registers");

    assert_eq!(first.symbols().world(), changed.symbols().world());
    assert_ne!(first.symbols().revision(), changed.symbols().revision());
    assert!(matches!(
        first.environment().external_owner(
            changed.symbols(),
            declaration,
            RegisteredExternalOwnerKind::Character,
        ),
        Err(ExternalOwnerLookupError::Stale {
            expected_world,
            actual_world,
            expected_revision,
            actual_revision,
        }) if expected_world == actual_world && expected_revision != actual_revision
    ));
}

#[test]
fn same_character_digest_preserves_character_revision() {
    let (root, project, world) = root_project("same-revision");
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let first =
        register(&project, &facts, TypeCheckEnv::standard(), None).expect("first registration");
    let changed_base = TypeCheckEnv::standard().with_symbol("unrelated", TypeKind::String);
    let second = register(&project, &facts, changed_base, Some(first.environment()))
        .expect("identical character inventory registers");

    assert_eq!(
        first.environment().character_digest(),
        second.environment().character_digest()
    );
    assert_eq!(
        first.environment().character_revision(),
        second.environment().character_revision()
    );
}

#[test]
fn changed_character_digest_increments_revision() {
    let (root, project, world) = root_project("changed-revision");
    let first_facts =
        one_character_facts(&root, world.clone(), &sample_manifest("layers/body.png"));
    let first = register(&project, &first_facts, TypeCheckEnv::standard(), None)
        .expect("first registration");
    let changed_facts =
        one_character_facts(&root, world, &sample_manifest("layers/body-changed.png"));
    let changed = register(
        &project,
        &changed_facts,
        TypeCheckEnv::standard(),
        Some(first.environment()),
    )
    .expect("changed registration");

    assert_ne!(
        first.environment().character_digest(),
        changed.environment().character_digest()
    );
    assert_eq!(
        first.environment().character_revision(),
        CharacterInventoryRevision(1)
    );
    assert_eq!(
        changed.environment().character_revision(),
        CharacterInventoryRevision(2)
    );
}

#[test]
fn environment_external_owner_lookup() {
    let (root, project, world) = root_project("environment-owner");
    let generated = source_document(
        "arcweft-generated://registration-tests/adapter",
        "adapter.viewport",
    );
    let declaration = generated
        .span(SourceRange::new(0, "adapter.viewport".len()))
        .expect("environment declaration span");
    let id = EnvironmentBindingId::try_new("adapter.viewport").expect("environment id");
    let fact = external_fact(
        id.as_str(),
        &[id.as_str()],
        RegisteredExternalOwner::Environment(id.clone()),
        declaration,
    );
    let facts =
        ProjectRegistrationFacts::try_new(world, vec![root, generated], vec![fact], Vec::new())
            .expect("environment facts");
    let base = TypeCheckEnv::standard().with_symbol(id.as_str(), TypeKind::I32);
    let registered = register(&project, &facts, base, None).expect("environment registers");
    let declaration = registered
        .symbols()
        .external_symbols()
        .next()
        .expect("environment external")
        .declaration();

    assert_eq!(
        registered.environment().environment_binding(&id),
        Some(&TypeKind::I32)
    );
    assert_eq!(
        registered.environment().external_owner(
            registered.symbols(),
            declaration,
            RegisteredExternalOwnerKind::Environment,
        ),
        Ok(&RegisteredExternalOwner::Environment(id))
    );
    assert!(matches!(
        registered.environment().external_owner(
            registered.symbols(),
            declaration,
            RegisteredExternalOwnerKind::Character,
        ),
        Err(ExternalOwnerLookupError::WrongKind {
            expected: RegisteredExternalOwnerKind::Character,
            actual: RegisteredExternalOwnerKind::Environment,
            ..
        })
    ));
}

#[test]
fn environment_owner_uses_exact_base_symbol_key() {
    assert_eq!(
        EnvironmentBindingId::try_new(""),
        Err(super::EnvironmentBindingIdError::Empty)
    );
    assert_eq!(
        EnvironmentBindingId::try_new("adapter\nviewport"),
        Err(super::EnvironmentBindingIdError::Control { byte: 7 })
    );

    let (root, project, world) = root_project("environment-exact-base-key");
    let generated = source_document(
        "arcweft-generated://registration-tests/exact-adapter",
        "adapter.viewport",
    );
    let declaration = generated
        .span(SourceRange::new(0, "adapter.viewport".len()))
        .expect("environment declaration span");
    let id = EnvironmentBindingId::try_new("adapter.viewport").expect("environment id");
    let fact = external_fact(
        id.as_str(),
        &[id.as_str()],
        RegisteredExternalOwner::Environment(id.clone()),
        declaration,
    );
    let facts =
        ProjectRegistrationFacts::try_new(world, vec![root, generated], vec![fact], Vec::new())
            .expect("environment facts");
    let altered = EnvironmentBindingId::try_new("adapter.viewporT").expect("altered key");
    let wrong_base = TypeCheckEnv::standard().with_symbol(altered.as_str(), TypeKind::I32);

    let report = register(&project, &facts, wrong_base, None)
        .expect_err("altered environment key is not an exact owner match");
    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind(),
        CharacterRegistrationDiagnosticKind::UnknownOwner {
            owner: RegisteredExternalOwner::Environment(owner),
        } if owner == &id
    )));

    let exact_base = TypeCheckEnv::standard().with_symbol(id.as_str(), TypeKind::I32);
    let registered =
        register(&project, &facts, exact_base, None).expect("exact environment key registers");
    assert_eq!(
        registered.environment().environment_binding(&id),
        Some(&TypeKind::I32)
    );
    assert_eq!(registered.environment().environment_binding(&altered), None);
}

#[test]
fn unknown_owner_is_atomic() {
    let (root, project, world) = root_project("unknown-owner");
    let declaration = root.span(SourceRange::new(0, 2)).expect("declaration span");
    let owner = CharacterId::try_new("character.missing").expect("owner");
    let fact = external_fact(
        owner.as_str(),
        &[owner.as_str(), owner.compact_str()],
        RegisteredExternalOwner::Character(owner.clone()),
        declaration,
    );
    let facts = ProjectRegistrationFacts::try_new(world, vec![root], vec![fact], Vec::new())
        .expect("source facts");
    let report = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect_err("unknown owner rejects transaction");

    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == CharacterRegistrationCode::UnknownOwner
            && matches!(
                diagnostic.kind(),
                CharacterRegistrationDiagnosticKind::UnknownOwner {
                    owner: RegisteredExternalOwner::Character(actual),
                } if actual == &owner
            )
    }));
}

#[test]
fn registration_missing_provenance_is_atomic() {
    let (root, project, world) = root_project("missing-provenance");
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let accepted =
        register(&project, &facts, TypeCheckEnv::standard(), None).expect("baseline registration");
    let before = registration_snapshot(&accepted);
    let mut missing = facts.clone();
    missing.remove_first_manifest_owner_source_for_test();

    let report = register(
        &project,
        &missing,
        TypeCheckEnv::standard(),
        Some(accepted.environment()),
    )
    .expect_err("missing manifest token provenance rejects transaction");
    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind(),
        CharacterRegistrationDiagnosticKind::MissingProvenance {
            token: super::RequiredCharacterToken::Manifest(CharacterManifestTokenPath::Root(
                CharacterManifestRootField::Character
            ))
        }
    )));
    assert_eq!(registration_snapshot(&accepted), before);
}

#[test]
fn registration_wrong_document_is_atomic() {
    let (root, project, world) = root_project("wrong-document");
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let accepted =
        register(&project, &facts, TypeCheckEnv::standard(), None).expect("baseline registration");
    let before = registration_snapshot(&accepted);
    let mut wrong = facts.clone();
    let wrong_span = root
        .span(SourceRange::new(0, 2))
        .expect("wrong-document span");
    wrong.replace_first_manifest_owner_source_for_test(wrong_span.clone());

    let report = register(
        &project,
        &wrong,
        TypeCheckEnv::standard(),
        Some(accepted.environment()),
    )
    .expect_err("wrong document rejects transaction");
    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind(),
        CharacterRegistrationDiagnosticKind::WrongDocument { expected, actual }
            if expected.as_str().ends_with("characters/akane.awchar.json")
                && actual == wrong_span.source().id()
    )));
    assert_eq!(registration_snapshot(&accepted), before);
}

#[test]
fn registration_wrong_revision_is_atomic() {
    let (root, project, world) = root_project("wrong-revision");
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let accepted =
        register(&project, &facts, TypeCheckEnv::standard(), None).expect("baseline registration");
    let before = registration_snapshot(&accepted);
    let current_manifest_document = facts
        .documents()
        .find(|document| {
            document
                .identity()
                .id()
                .as_str()
                .ends_with("characters/akane.awchar.json")
        })
        .expect("manifest document");
    let stale_document = source_document(current_manifest_document.identity().id().as_str(), "x");
    let stale_span = stale_document
        .span(SourceRange::new(0, 1))
        .expect("stale span");
    let mut wrong = facts.clone();
    wrong.replace_first_manifest_owner_source_for_test(stale_span.clone());

    let report = register(
        &project,
        &wrong,
        TypeCheckEnv::standard(),
        Some(accepted.environment()),
    )
    .expect_err("wrong revision rejects transaction");
    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind(),
        CharacterRegistrationDiagnosticKind::WrongRevision { expected, actual }
            if *expected == current_manifest_document.identity().revision()
                && *actual == stale_span.source().revision()
    )));
    assert_eq!(registration_snapshot(&accepted), before);
}

#[test]
fn registration_stale_source_set_is_atomic() {
    let (root, project, world) = root_project("stale-source-set");
    let facts = one_character_facts(&root, world.clone(), &sample_manifest("layers/body.png"));
    let accepted =
        register(&project, &facts, TypeCheckEnv::standard(), None).expect("baseline registration");
    let before = registration_snapshot(&accepted);
    let changed_root = source_document(
        root.identity().id().as_str(),
        "fn changed() -> Unit { () }\n",
    );
    let changed_facts =
        one_character_facts(&changed_root, world, &sample_manifest("layers/body.png"));
    let mut stale = facts.clone();
    stale.replace_symbol_revision_for_test(*changed_facts.symbol_revision());

    let report = register(
        &project,
        &stale,
        TypeCheckEnv::standard(),
        Some(accepted.environment()),
    )
    .expect_err("facts/table source revision mismatch rejects transaction");
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == CharacterRegistrationCode::StaleSource
            && matches!(
                diagnostic.kind(),
                CharacterRegistrationDiagnosticKind::StaleSource { expected, actual }
                    if expected == stale.symbol_revision()
                        && actual == stale.external_declarations().revision()
            )
    }));
    assert_eq!(registration_snapshot(&accepted), before);
}

#[test]
fn external_unknown_registration_is_atomic() {
    let (root, project, world) = root_project("external-unknown");
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let accepted =
        register(&project, &facts, TypeCheckEnv::standard(), None).expect("baseline registration");
    let before = registration_snapshot(&accepted);
    let mut missing = facts.clone();
    missing.clear_external_owner_contributions_for_test();

    let report = register(
        &project,
        &missing,
        TypeCheckEnv::standard(),
        Some(accepted.environment()),
    )
    .expect_err("linked external without owner contribution rejects transaction");
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == CharacterRegistrationCode::ExternalUnknown })
    );
    assert_eq!(registration_snapshot(&accepted), before);
}

#[test]
fn external_exact_duplicate_is_atomic() {
    let (root, project, world) = root_project("external-duplicate");
    let manifest = sample_manifest("layers/body.png");
    let (document, backed) = backed_manifest(
        "arcweft-project://registration-tests/characters/duplicate.awchar.json",
        &manifest,
    );
    let declaration = declaration_span(&backed);
    let owner = manifest.character().clone();
    let fact = external_fact(
        owner.as_str(),
        &[owner.as_str(), owner.compact_str()],
        RegisteredExternalOwner::Character(owner.clone()),
        declaration,
    );
    let catalog = SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![backed])
        .expect("catalog");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, document],
        vec![fact.clone(), fact],
        vec![catalog],
    )
    .expect("duplicate contributions remain observable");
    let report = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect_err("duplicate registry contribution rejects transaction");

    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == CharacterRegistrationCode::ExternalDuplicate)
    );
}

#[test]
fn external_equal_seed_contributions_are_not_hidden() {
    external_exact_duplicate_is_atomic();
}

#[test]
fn equal_cross_catalog_occurrences_coalesce() {
    let (root, project, world) = root_project("equal-catalogs");
    let manifest = sample_manifest("layers/body.png");
    let (first_document, first) = backed_manifest(
        "arcweft-project://registration-tests/characters/first.awchar.json",
        &manifest,
    );
    let (second_document, second) = backed_manifest(
        "arcweft-project://registration-tests/characters/second.awchar.json",
        &manifest,
    );
    let owner = manifest.character().clone();
    let fact = external_fact(
        owner.as_str(),
        &[owner.as_str(), owner.compact_str()],
        RegisteredExternalOwner::Character(owner.clone()),
        declaration_span(&first),
    );
    let first_catalog = SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![first])
        .expect("first catalog");
    let second_catalog =
        SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![second])
            .expect("second catalog");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, first_document, second_document],
        vec![fact],
        vec![first_catalog, second_catalog],
    )
    .expect("equal cross-catalog facts");
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("equal occurrences coalesce");

    assert_eq!(registered.environment().characters().len(), 1);
    assert_eq!(registered.symbols().external_symbols().count(), 1);
    let declarations = registered
        .character_definition_index()
        .declaration(&CharacterSymbolDescriptor::Owner { character: owner })
        .expect("owner declarations");
    assert_eq!(declarations.sources().len(), 2);
    assert_eq!(registered.character_definition_index().documents().len(), 2);
}

#[test]
fn reordered_equal_manifest_coalesces() {
    let (root, project, world) = root_project("reordered-equal-catalogs");
    let first_manifest = reordered_manifest(false);
    let reordered_manifest = reordered_manifest(true);
    assert_eq!(
        first_manifest.semantic_fingerprint_v1(),
        reordered_manifest.semantic_fingerprint_v1(),
    );
    let (first_document, first) = backed_manifest(
        "arcweft-project://registration-tests/characters/ordered.awchar.json",
        &first_manifest,
    );
    let (reordered_document, reordered) = backed_manifest(
        "arcweft-project://registration-tests/characters/reordered.awchar.json",
        &reordered_manifest,
    );
    let owner = first_manifest.character().clone();
    let fact = external_fact(
        owner.as_str(),
        &[owner.as_str(), owner.compact_str()],
        RegisteredExternalOwner::Character(owner.clone()),
        declaration_span(&first),
    );
    let catalogs = [first, reordered]
        .into_iter()
        .map(|manifest| {
            SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![manifest])
                .expect("catalog")
        })
        .collect();
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, first_document, reordered_document],
        vec![fact],
        catalogs,
    )
    .expect("reordered facts");

    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("semantic ordering does not create a conflict");

    assert_eq!(registered.environment().characters().len(), 1);
    assert_eq!(registered.symbols().external_symbols().count(), 1);
}

#[test]
fn equal_digest_unequal_manifest_is_collision() {
    let document = source_document(
        "arcweft-project://registration-tests/forced-manifest-collision.arcw",
        "ab",
    );
    let first = sample_manifest("layers/body.png");
    let unequal = sample_manifest("layers/body-changed.png");
    let forced = first.semantic_fingerprint_v1();
    let owner = first.character().clone();
    let mut records = BTreeMap::new();
    let mut diagnostics = Vec::new();

    merge_manifest_occurrence(
        &mut records,
        owner.clone(),
        &first,
        forced,
        document.span(SourceRange::new(0, 1)).expect("first span"),
        &mut diagnostics,
    );
    merge_manifest_occurrence(
        &mut records,
        owner.clone(),
        &unequal,
        forced,
        document
            .span(SourceRange::new(1, 2))
            .expect("collision span"),
        &mut diagnostics,
    );

    assert_eq!(records.len(), 1, "a digest collision never coalesces");
    assert!(matches!(
        diagnostics.as_slice(),
        [diagnostic]
            if diagnostic.kind()
                == &CharacterRegistrationDiagnosticKind::DigestCollision {
                    owner,
                    digest: forced,
                }
    ));
}

#[test]
fn equal_cross_catalog_occurrences_retain_all_conflict_provenance() {
    let (root, project, world) = root_project("equal-catalog-provenance");
    let equal_manifest = sample_manifest("layers/body.png");
    let changed_manifest = sample_manifest("layers/body-changed.png");
    let (first_document, first) = backed_manifest(
        "arcweft-project://registration-tests/characters/first-equal.awchar.json",
        &equal_manifest,
    );
    let (second_document, second) = backed_manifest(
        "arcweft-project://registration-tests/characters/second-equal.awchar.json",
        &equal_manifest,
    );
    let (changed_document, changed) = backed_manifest(
        "arcweft-project://registration-tests/characters/changed-after-equal.awchar.json",
        &changed_manifest,
    );
    let owner = equal_manifest.character().clone();
    let fact = external_fact(
        owner.as_str(),
        &[owner.as_str(), owner.compact_str()],
        RegisteredExternalOwner::Character(owner.clone()),
        declaration_span(&first),
    );
    let catalogs = [first, second, changed]
        .into_iter()
        .map(|manifest| {
            SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![manifest])
                .expect("catalog")
        })
        .collect();
    let expected_sources = [
        first_document.identity().id().clone(),
        second_document.identity().id().clone(),
    ];
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, first_document, second_document, changed_document],
        vec![fact],
        catalogs,
    )
    .expect("source-backed facts");
    let report = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect_err("changed third occurrence conflicts");
    let conflict = report
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == CharacterRegistrationCode::ConflictingManifest)
        .expect("conflict diagnostic");
    let retained_sources = conflict
        .secondary()
        .iter()
        .map(|span| span.source().id().clone())
        .collect::<Vec<_>>();
    assert_eq!(retained_sources, expected_sources);
}

#[test]
fn diagnostics_are_deterministically_sorted() {
    let document = source_document(
        "arcweft-project://registration-tests/diagnostic-order.arcw",
        "ab",
    );
    let span = document.span(SourceRange::new(0, 1)).expect("span");
    let diagnostic = |owner: &str| {
        CharacterRegistrationDiagnostic::new(
            CharacterRegistrationDiagnosticKind::UnknownOwner {
                owner: RegisteredExternalOwner::Environment(
                    EnvironmentBindingId::try_new(owner).expect("environment id"),
                ),
            },
            span.clone(),
            [],
        )
    };
    let forward = CharacterRegistrationReport::from_diagnostics(vec![
        diagnostic("environment.a"),
        diagnostic("environment.b"),
    ]);
    let reversed = CharacterRegistrationReport::from_diagnostics(vec![
        diagnostic("environment.b"),
        diagnostic("environment.a"),
    ]);

    assert_eq!(forward, reversed);
    assert!(matches!(
        forward.diagnostics()[0].kind(),
        CharacterRegistrationDiagnosticKind::UnknownOwner {
            owner: RegisteredExternalOwner::Environment(owner)
        } if owner.as_str() == "environment.a"
    ));
}

#[test]
fn diagnostic_code_is_derived_from_kind() {
    let document = source_document(
        "arcweft-project://registration-tests/diagnostic-code.arcw",
        "x",
    );
    let kind = CharacterRegistrationDiagnosticKind::UnknownOwner {
        owner: RegisteredExternalOwner::Environment(
            EnvironmentBindingId::try_new("adapter.viewport").expect("environment id"),
        ),
    };
    let diagnostic = CharacterRegistrationDiagnostic::new(
        kind.clone(),
        document.span(SourceRange::new(0, 1)).expect("span"),
        [],
    );

    assert_eq!(diagnostic.code(), kind.code());
    assert_eq!(diagnostic.code(), CharacterRegistrationCode::UnknownOwner);
}

#[test]
fn diagnostic_cap_128_and_129() {
    let document = source_document(
        "arcweft-project://registration-tests/diagnostic-cap.arcw",
        "x",
    );
    let span = document.span(SourceRange::new(0, 1)).expect("span");
    let diagnostics = |count: usize| {
        (0..count)
            .rev()
            .map(|index| {
                CharacterRegistrationDiagnostic::new(
                    CharacterRegistrationDiagnosticKind::UnknownOwner {
                        owner: RegisteredExternalOwner::Environment(
                            EnvironmentBindingId::try_new(format!("environment.{index:03}"))
                                .expect("environment id"),
                        ),
                    },
                    span.clone(),
                    [],
                )
            })
            .collect()
    };

    let exact = CharacterRegistrationReport::from_diagnostics(diagnostics(128));
    assert_eq!(exact.diagnostics().len(), 128);
    assert_eq!(exact.omitted_diagnostics(), 0);

    let one_over = CharacterRegistrationReport::from_diagnostics(diagnostics(129));
    assert_eq!(one_over.diagnostics().len(), 128);
    assert_eq!(one_over.omitted_diagnostics(), 1);
    assert!(matches!(
        one_over.diagnostics().last().expect("last retained").kind(),
        CharacterRegistrationDiagnosticKind::UnknownOwner {
            owner: RegisteredExternalOwner::Environment(owner)
        } if owner.as_str() == "environment.127"
    ));
}

#[test]
fn work_exact_one_over_and_arithmetic_overflow() {
    let document = source_document(
        "arcweft-project://registration-tests/work-overflow.arcw",
        "x",
    );
    let span = document.span(SourceRange::new(0, 1)).expect("span");
    let maximum = CharacterRegistrationLimits::PRODUCTION.work();
    let mut work = maximum - 1;
    let mut diagnostics = Vec::new();

    charge(&mut work, 1, &span, &mut diagnostics);

    assert_eq!(work, maximum);
    assert!(diagnostics.is_empty());

    charge(&mut work, 1, &span, &mut diagnostics);

    assert_eq!(work, maximum);
    assert!(matches!(
        diagnostics.as_slice(),
        [diagnostic]
            if diagnostic.kind()
                == &CharacterRegistrationDiagnosticKind::WorkOverflow {
                    attempted: maximum + 1,
                    maximum,
                }
    ));

    work = u64::MAX;
    diagnostics.clear();
    charge(&mut work, 1, &span, &mut diagnostics);

    assert_eq!(work, u64::MAX);
    assert!(matches!(
        diagnostics.as_slice(),
        [diagnostic]
            if diagnostic.kind()
                == &CharacterRegistrationDiagnosticKind::ArithmeticOverflow {
                    counter: super::CharacterRegistrationLimitKind::Work,
                }
    ));
}

#[test]
fn unequal_cross_catalog_occurrences_conflict_atomically() {
    let (root, project, world) = root_project("conflicting-catalogs");
    let first_manifest = sample_manifest("layers/body.png");
    let changed_manifest = sample_manifest("layers/body-changed.png");
    let (first_document, first) = backed_manifest(
        "arcweft-project://registration-tests/characters/first-conflict.awchar.json",
        &first_manifest,
    );
    let (changed_document, changed) = backed_manifest(
        "arcweft-project://registration-tests/characters/changed-conflict.awchar.json",
        &changed_manifest,
    );
    let owner = first_manifest.character().clone();
    let fact = external_fact(
        owner.as_str(),
        &[owner.as_str(), owner.compact_str()],
        RegisteredExternalOwner::Character(owner.clone()),
        declaration_span(&first),
    );
    let first_catalog = SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![first])
        .expect("first catalog");
    let changed_catalog =
        SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![changed])
            .expect("changed catalog");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, first_document, changed_document],
        vec![fact],
        vec![first_catalog, changed_catalog],
    )
    .expect("conflicting source facts remain constructible");
    let report = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect_err("conflicting manifests reject registration");

    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == CharacterRegistrationCode::ConflictingManifest)
    );
}

#[test]
fn external_conflict_is_atomic() {
    let (root, project, world) = root_project("external-conflict");
    let manifest = sample_manifest("layers/body.png");
    let (document, backed) = backed_manifest(
        "arcweft-project://registration-tests/characters/external-conflict.awchar.json",
        &manifest,
    );
    let declaration = declaration_span(&backed);
    let owner = manifest.character().clone();
    let character_fact = external_fact(
        owner.as_str(),
        &[owner.as_str(), owner.compact_str()],
        RegisteredExternalOwner::Character(owner.clone()),
        declaration.clone(),
    );
    let environment_id = EnvironmentBindingId::try_new("adapter.viewport").expect("environment id");
    let environment_fact = ExternalRegistrationFact::new(
        character_fact.declaration().clone(),
        RegisteredExternalOwner::Environment(environment_id.clone()),
        declaration,
    );
    let catalog = SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![backed])
        .expect("catalog");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, document],
        vec![environment_fact, character_fact],
        vec![catalog],
    )
    .expect("conflicting contributions remain observable");
    let base = TypeCheckEnv::standard().with_symbol(environment_id.as_str(), TypeKind::I32);
    let report = register(&project, &facts, base, None)
        .expect_err("conflicting registry contributions reject transaction");

    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == CharacterRegistrationCode::ExternalConflict)
    );
}

#[test]
fn same_owner_two_worlds_isolated() {
    let (first_root, first_project, first_world) = root_project("isolated-a");
    let first_facts = one_character_facts(
        &first_root,
        first_world,
        &sample_manifest("layers/body.png"),
    );
    let first = register(&first_project, &first_facts, TypeCheckEnv::standard(), None)
        .expect("first world");
    let (second_root, second_project, second_world) = root_project("isolated-b");
    let second_facts = one_character_facts(
        &second_root,
        second_world,
        &sample_manifest("layers/body.png"),
    );
    let second = register(
        &second_project,
        &second_facts,
        TypeCheckEnv::standard(),
        None,
    )
    .expect("second world");
    let first_declaration = first
        .symbols()
        .external_symbols()
        .next()
        .expect("first external")
        .declaration();

    assert_ne!(first.environment().world(), second.environment().world());
    assert_eq!(
        first.environment().character_digest(),
        second.environment().character_digest(),
        "world identity is excluded from the character descriptor"
    );
    assert!(matches!(
        first.environment().external_owner(
            second.symbols(),
            first_declaration,
            RegisteredExternalOwnerKind::Character,
        ),
        Err(ExternalOwnerLookupError::Stale { .. })
    ));
}

#[test]
fn external_lookup_stale_world() {
    let (first, _) = registered_character_and_environment("external-stale-world-a");
    let (second, _) = registered_character_and_environment("external-stale-world-b");
    let declaration = first
        .symbols()
        .external_symbols()
        .find(|symbol| symbol.canonical_path().leaf() == "character.akane")
        .expect("first character external")
        .declaration();

    assert!(matches!(
        first.environment().external_owner(
            second.symbols(),
            declaration,
            RegisteredExternalOwnerKind::Character,
        ),
        Err(ExternalOwnerLookupError::Stale {
            expected_world,
            actual_world,
            ..
        }) if expected_world != actual_world
    ));
}

#[test]
fn character_spelling_variants_one_target() {
    let (documents, project, world) = project_modules(
        "spellings",
        &[
            (
                "",
                "use crate.akane as hero\nuse crate.cast.akane as speaker\nfn main() -> Unit { () }\n",
            ),
            ("cast", "pub use crate.akane\n"),
        ],
    );
    let root = Arc::clone(documents.first().expect("root document"));
    let facts = one_character_facts_with_documents(
        &root,
        documents,
        world,
        &sample_manifest("layers/body.png"),
    );
    let registered =
        register(&project, &facts, TypeCheckEnv::standard(), None).expect("world registers");
    let source = root.span(SourceRange::new(0, 2)).expect("reference span");
    let canonical =
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "character.akane")
            .expect("canonical path");
    let compact = SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "akane")
        .expect("compact path");
    let qualified = SymbolPath::try_new(
        ModulePathRoot::Crate,
        vec![ModuleSegment::new("cast").expect("qualifier")],
        "akane",
    )
    .expect("qualified path");
    let arbitrary = SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "hero")
        .expect("arbitrary alias");
    let second_alias = SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "speaker")
        .expect("second alias");

    let canonical_owner = registered
        .environment()
        .resolve_character_owner(
            registered.symbols(),
            &CanonicalModulePath::crate_root(),
            &canonical,
            &source,
        )
        .expect("canonical owner");
    let compact_owner = registered
        .environment()
        .resolve_character_owner(
            registered.symbols(),
            &CanonicalModulePath::crate_root(),
            &compact,
            &source,
        )
        .expect("compact owner");
    let qualified_owner = registered
        .environment()
        .resolve_character_owner(
            registered.symbols(),
            &CanonicalModulePath::crate_root(),
            &qualified,
            &source,
        )
        .expect("qualified owner");
    let arbitrary_owner = registered
        .environment()
        .resolve_character_owner(
            registered.symbols(),
            &CanonicalModulePath::crate_root(),
            &arbitrary,
            &source,
        )
        .expect("arbitrary alias owner");
    let second_alias_owner = registered
        .environment()
        .resolve_character_owner(
            registered.symbols(),
            &CanonicalModulePath::crate_root(),
            &second_alias,
            &source,
        )
        .expect("second alias owner");
    assert_eq!(canonical_owner, compact_owner);
    assert_eq!(canonical_owner, qualified_owner);
    assert_eq!(canonical_owner, arbitrary_owner);
    assert_eq!(canonical_owner, second_alias_owner);
}

#[test]
fn repeated_character_import_same_target_succeeds() {
    let (root, project, world) = root_project_source(
        "repeated-character-import",
        "use crate.akane as hero\nuse crate.character.akane as hero\n",
    );
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("same-target imports coalesce");
    let source = root.span(SourceRange::new(0, 3)).expect("reference span");
    let hero =
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "hero").expect("hero alias");
    assert_eq!(
        registered
            .environment()
            .resolve_character_owner(
                registered.symbols(),
                &CanonicalModulePath::crate_root(),
                &hero,
                &source,
            )
            .expect("hero owner")
            .as_str(),
        "character.akane"
    );
}

#[test]
fn two_aliases_one_character_succeed() {
    let (root, project, world) = root_project_source(
        "two-character-aliases",
        "use crate.akane as hero\nuse crate.akane as speaker\n",
    );
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("two aliases for one character register");
}

#[test]
fn character_alias_two_owners_fails() {
    let (root, project, world) = root_project_source(
        "character-alias-two-owners",
        "use crate.akane as hero\nuse crate.ren as hero\n",
    );
    let akane = sample_manifest_for("character.akane", "layers/akane.png");
    let ren = sample_manifest_for("character.ren", "layers/ren.png");
    let (akane_document, akane_backed) = backed_manifest(
        "arcweft-project://registration-tests/characters/alias-akane.awchar.json",
        &akane,
    );
    let (ren_document, ren_backed) = backed_manifest(
        "arcweft-project://registration-tests/characters/alias-ren.awchar.json",
        &ren,
    );
    let facts = [(&akane, &akane_backed), (&ren, &ren_backed)].map(|(manifest, backed)| {
        let owner = manifest.character().clone();
        external_fact(
            owner.as_str(),
            &[owner.as_str(), owner.compact_str()],
            RegisteredExternalOwner::Character(owner.clone()),
            declaration_span(backed),
        )
    });
    let catalog = SourceBackedCharacterCatalog::try_new(
        root.identity().clone(),
        vec![akane_backed, ren_backed],
    )
    .expect("two-owner catalog");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, akane_document, ren_document],
        facts.into(),
        vec![catalog],
    )
    .expect("two-owner facts");

    let report = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect_err("one alias cannot resolve to two character owners");

    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind(),
        CharacterRegistrationDiagnosticKind::AliasCollision {
            spelling,
            conflicting,
            ..
        } if spelling.leaf() == "hero" && !conflicting.is_empty()
    )));
}

#[test]
fn character_alias_local_collision_fails() {
    let (root, project, world) = root_project_source(
        "character-alias-local-collision",
        "use crate.akane as hero\nfn hero() -> Unit { () }\n",
    );
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let report = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect_err("character alias collides with local callable");

    assert!(report.diagnostics().iter().any(|diagnostic| {
        matches!(
            diagnostic.kind(),
            CharacterRegistrationDiagnosticKind::AliasCollision {
                spelling,
                conflicting,
                ..
            } if spelling.leaf() == "hero"
                && conflicting
                    .iter()
                    .any(|target| matches!(target, arcweft_lang_hir::symbol::ProjectSymbolTargetId::Callable(_)))
        )
    }));
}

#[test]
fn canonical_spelling_collision_fails() {
    let (root, project, world) = root_project("canonical-spelling-collision");
    let manifest = sample_manifest("layers/body.png");
    let (document, backed) = backed_manifest(
        "arcweft-project://registration-tests/characters/canonical-collision.awchar.json",
        &manifest,
    );
    let character = manifest.character().clone();
    let character_fact = external_fact(
        character.as_str(),
        &[character.as_str(), character.compact_str()],
        RegisteredExternalOwner::Character(character.clone()),
        declaration_span(&backed),
    );
    let environment = EnvironmentBindingId::try_new("adapter.viewport").expect("environment id");
    let environment_fact = external_fact(
        environment.as_str(),
        &["character.akane"],
        RegisteredExternalOwner::Environment(environment.clone()),
        declaration_span(&backed),
    );
    let catalog = SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![backed])
        .expect("catalog");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, document],
        vec![character_fact, environment_fact],
        vec![catalog],
    )
    .expect("collision facts");
    let base = TypeCheckEnv::standard().with_symbol(environment.as_str(), TypeKind::I32);

    let report = register(&project, &facts, base, None)
        .expect_err("canonical character spelling must remain unambiguous");

    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind(),
        CharacterRegistrationDiagnosticKind::AliasCollision {
            spelling,
            conflicting,
            ..
        } if spelling.leaf() == "character.akane" && !conflicting.is_empty()
    )));
}

#[test]
fn compact_spelling_collision_fails() {
    let (root, project, world) =
        root_project_source("compact-spelling-collision", "fn akane() -> Unit { () }\n");
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let report = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect_err("compact spelling collides with local callable");

    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind(),
        CharacterRegistrationDiagnosticKind::AliasCollision { spelling, .. }
            if spelling.leaf() == "akane"
    )));
}

#[test]
fn qualified_spelling_collision_fails() {
    let (documents, project, world) = project_modules(
        "qualified-spelling-collision",
        &[
            ("", "fn main() -> Unit { () }\n"),
            (
                "cast",
                "pub use crate.akane\npub fn akane() -> Unit { () }\n",
            ),
        ],
    );
    let root = Arc::clone(documents.first().expect("root document"));
    let facts = one_character_facts_with_documents(
        &root,
        documents,
        world,
        &sample_manifest("layers/body.png"),
    );

    let report = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect_err("qualified character spelling must remain unambiguous");

    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind(),
        CharacterRegistrationDiagnosticKind::AliasCollision {
            spelling,
            conflicting,
            ..
        } if spelling.canonical_string() == "crate.cast.akane" && !conflicting.is_empty()
    )));
}

#[test]
fn unchanged_digest_at_max_revision_succeeds() {
    let (root, project, world) = root_project("max-unchanged");
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let first =
        register(&project, &facts, TypeCheckEnv::standard(), None).expect("first registration");
    let mut previous = first.environment().clone();
    previous.character_revision = CharacterInventoryRevision(u64::MAX);

    let unchanged = register(&project, &facts, TypeCheckEnv::standard(), Some(&previous))
        .expect("unchanged digest preserves max revision");
    assert_eq!(unchanged.environment().character_revision().get(), u64::MAX);
}

#[test]
fn changed_digest_at_max_revision_fails_atomically() {
    let (root, project, world) = root_project("max-changed");
    let first_facts =
        one_character_facts(&root, world.clone(), &sample_manifest("layers/body.png"));
    let first = register(&project, &first_facts, TypeCheckEnv::standard(), None)
        .expect("first registration");
    let mut previous = first.environment().clone();
    previous.character_revision = CharacterInventoryRevision(u64::MAX);
    let changed_facts =
        one_character_facts(&root, world, &sample_manifest("layers/body-changed.png"));

    let report = register(
        &project,
        &changed_facts,
        TypeCheckEnv::standard(),
        Some(&previous),
    )
    .expect_err("changed digest cannot overflow revision");
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == CharacterRegistrationCode::RevisionOverflow)
    );
    assert_eq!(previous.character_revision().get(), u64::MAX);
}

#[test]
fn verify_character_inventory_detects_tamper() {
    let (root, project, world) = root_project("tamper");
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let registered =
        register(&project, &facts, TypeCheckEnv::standard(), None).expect("world registers");
    let mut tampered = registered.environment().clone();
    tampered.character_digest = CharacterInventoryDigest([0; 32]);

    assert!(matches!(
        tampered.verify_character_inventory(registered.symbols()),
        Err(CharacterInventoryIntegrityError::DescriptorTamper { .. })
    ));

    let mut tampered_registry = registered.environment().clone();
    let declaration = registered
        .symbols()
        .external_symbols()
        .next()
        .expect("character external")
        .declaration();
    tampered_registry.external_owners.owners.insert(
        declaration,
        RegisteredExternalOwner::Environment(
            EnvironmentBindingId::try_new("adapter.tampered").expect("environment binding"),
        ),
    );
    assert!(matches!(
        tampered_registry.verify_character_inventory(registered.symbols()),
        Err(CharacterInventoryIntegrityError::DescriptorTamper { .. })
    ));
}

#[test]
fn registry_descriptor_table_mismatch() {
    let (registered, _) = registered_character_and_environment("registry-table-mismatch");
    let character = registered
        .symbols()
        .external_symbols()
        .find(|symbol| symbol.canonical_path().leaf() == "character.akane")
        .expect("character external")
        .declaration();
    let environment = registered
        .symbols()
        .external_symbols()
        .find(|symbol| symbol.canonical_path().leaf() == "adapter.viewport")
        .expect("environment external")
        .declaration();
    let mut mismatched = registered.environment().clone();
    let owner = mismatched
        .external_owners
        .owners
        .remove(&character)
        .expect("character registry entry");
    mismatched.external_owners.owners.insert(environment, owner);

    assert!(matches!(
        mismatched.verify_character_inventory(registered.symbols()),
        Err(CharacterInventoryIntegrityError::DescriptorTamper { .. })
    ));
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
        output
    })
}
