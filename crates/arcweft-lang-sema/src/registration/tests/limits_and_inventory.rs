use super::super::descriptor;
use super::*;

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
    assert_eq!(
        first.environment().callable_catalog(),
        second.environment().callable_catalog(),
        "unordered environment storage must publish one deterministic catalog",
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
        Vec::new(),
    )
    .expect("exact catalog facts");
    register(&project, &exact, TypeCheckEnv::standard(), None)
        .expect("exact catalog limit is accepted");

    let mut one_over = catalogs;
    one_over.push(
        SourceBackedCharacterCatalog::try_new(root.identity().clone(), Vec::new())
            .expect("one-over empty catalog"),
    );
    let facts =
        ProjectRegistrationFacts::try_new(world, vec![root], Vec::new(), one_over, Vec::new())
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
    let exact = ProjectRegistrationFacts::try_new(
        world.clone(),
        documents.clone(),
        Vec::new(),
        catalogs,
        Vec::new(),
    )
    .expect("exact occurrence facts");
    register(&project, &exact, TypeCheckEnv::standard(), None)
        .expect("exact occurrence limit is accepted");

    let mut one_over_counts = vec![16; 63];
    one_over_counts.push(17);
    let (manifest_documents, one_over_catalogs) = build_catalogs(&one_over_counts);
    let mut one_over_documents = vec![root];
    one_over_documents.extend(manifest_documents);
    let facts = ProjectRegistrationFacts::try_new(
        world,
        one_over_documents,
        Vec::new(),
        one_over_catalogs,
        Vec::new(),
    )
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
        Vec::new(),
    )
    .expect("exact owner facts");
    register(&project, &exact, TypeCheckEnv::standard(), None)
        .expect("exact owner limit is accepted");

    let one_over_catalog =
        SourceBackedCharacterCatalog::try_new(root.identity().clone(), manifests)
            .expect("one-over owner catalog");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        documents,
        Vec::new(),
        vec![one_over_catalog],
        Vec::new(),
    )
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
    let exact = ProjectRegistrationFacts::try_new(
        world.clone(),
        documents.clone(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("exact document facts");
    register(&project, &exact, TypeCheckEnv::standard(), None)
        .expect("exact document limit is accepted");

    documents.push(source_document(
        "arcweft-generated://registration-tests/document-one-over",
        "",
    ));
    let report =
        ProjectRegistrationFacts::try_new(world, documents, Vec::new(), Vec::new(), Vec::new())
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
        Vec::new(),
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
        Vec::new(),
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
        descriptor::descriptor_canonical_len(&registered.environment().character_descriptor),
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
        &character_binding_paths(&owner)
            .into_iter()
            .chain([project_path(["hero"])])
            .collect::<Vec<_>>(),
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
        Vec::new(),
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
        &character_binding_paths(&owner),
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
        Vec::new(),
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
