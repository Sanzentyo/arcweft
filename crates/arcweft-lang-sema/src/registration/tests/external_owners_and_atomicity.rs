use super::super::RequiredCharacterToken;
use super::*;

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
            .external_owners_mut_for_test()
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
            .external_owners_mut_for_test()
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
    let expected = environment_external_owner(environment);

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
        "fn main() -> Unit {\n    let changed = true\n    ()\n}\n",
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
        &[project_path(["adapter", "viewport"])],
        environment_external_owner(id.clone()),
        declaration,
    );
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, generated],
        vec![fact],
        Vec::new(),
        Vec::new(),
    )
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
    let accepted_owner = AcceptedNominalOwnerId::Environment(id.clone());
    let accepted_records = registered
        .environment()
        .nominal_catalog()
        .exact_records_for_owner(&accepted_owner)
        .collect::<Vec<_>>();
    assert_eq!(accepted_records.len(), 1);
    assert_eq!(accepted_records[0].origin(), AcceptedNominalOrigin::Adapter);
    assert_eq!(
        accepted_records[0].semantics(),
        &AcceptedNominalSemantics::Exact(TypeKind::I32)
    );
    assert_eq!(
        registered.environment().external_owner(
            registered.symbols(),
            declaration,
            RegisteredExternalOwnerKind::Environment,
        ),
        Ok(&environment_external_owner(id))
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
        Err(EnvironmentBindingIdError::Empty)
    );
    assert_eq!(
        EnvironmentBindingId::try_new("adapter\nviewport"),
        Err(EnvironmentBindingIdError::Control { byte: 7 })
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
        &[project_path(["adapter", "viewport"])],
        environment_external_owner(id.clone()),
        declaration,
    );
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, generated],
        vec![fact],
        Vec::new(),
        Vec::new(),
    )
    .expect("environment facts");
    let altered = EnvironmentBindingId::try_new("adapter.viewporT").expect("altered key");
    let wrong_base = TypeCheckEnv::standard().with_symbol(altered.as_str(), TypeKind::I32);

    let report = register(&project, &facts, wrong_base, None)
        .expect_err("altered environment key is not an exact owner match");
    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind(),
        CharacterRegistrationDiagnosticKind::UnknownOwner {
            owner: RegisteredExternalOwner::Environment(owner),
        } if owner.value_binding() == &id
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
        &character_binding_paths(&owner),
        RegisteredExternalOwner::Character(owner.clone()),
        declaration,
    );
    let facts =
        ProjectRegistrationFacts::try_new(world, vec![root], vec![fact], Vec::new(), Vec::new())
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
            token: RequiredCharacterToken::Manifest(CharacterManifestTokenPath::Root(
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
pub(super) fn external_exact_duplicate_is_atomic() {
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
        &character_binding_paths(&owner),
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
        Vec::new(),
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
