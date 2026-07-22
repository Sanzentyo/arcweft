use super::*;

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
        "use crate.akane as hero\nuse crate.akane as hero\n",
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
            &character_binding_paths(&owner),
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
        Vec::new(),
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
        &character_binding_paths(&character),
        RegisteredExternalOwner::Character(character.clone()),
        declaration_span(&backed),
    );
    let environment = EnvironmentBindingId::try_new("adapter.viewport").expect("environment id");
    let environment_fact = external_fact(
        environment.as_str(),
        &[project_path(["character", "akane"])],
        environment_external_owner(environment.clone()),
        declaration_span(&backed),
    );
    let catalog = SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![backed])
        .expect("catalog");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, document],
        vec![character_fact, environment_fact],
        vec![catalog],
        Vec::new(),
    )
    .expect("collision facts");
    let base = TypeCheckEnv::standard().with_symbol(environment.as_str(), TypeKind::I32);

    let report = register(&project, &facts, base, None)
        .expect_err("canonical character spelling must remain unambiguous");

    assert!(
        report.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic.kind(),
            CharacterRegistrationDiagnosticKind::ProjectSymbol {
                error: ProjectSymbolLinkError::DuplicateDeclaration { name, .. },
            } if name == "akane"
        )),
        "{report:#?}"
    );
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
        CharacterRegistrationDiagnosticKind::ProjectSymbol {
            error: ProjectSymbolLinkError::DuplicateDeclaration { name, .. },
        } if name == "akane"
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
    tampered_registry.external_owners_mut_for_test().insert(
        declaration,
        environment_external_owner(
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
        .external_owners_mut_for_test()
        .remove(&character)
        .expect("character registry entry");
    mismatched
        .external_owners_mut_for_test()
        .insert(environment, owner);

    assert!(matches!(
        mismatched.verify_character_inventory(registered.symbols()),
        Err(CharacterInventoryIntegrityError::DescriptorTamper { .. })
    ));
}
