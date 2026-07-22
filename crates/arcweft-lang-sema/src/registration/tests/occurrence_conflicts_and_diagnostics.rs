use super::*;

#[test]
fn external_equal_seed_contributions_are_not_hidden() {
    super::external_owners_and_atomicity::external_exact_duplicate_is_atomic();
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
        &character_binding_paths(&owner),
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
        &character_binding_paths(&owner),
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
        &character_binding_paths(&owner),
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
        &character_binding_paths(&owner),
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
        &character_binding_paths(&owner),
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
