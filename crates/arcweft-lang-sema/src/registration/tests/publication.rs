use super::*;

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
fn accepted_world_publishes_project_callables_and_non_callable_shadow_bindings() {
    let (root, project, world) = root_project("callable-catalog");
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("complete world registers");
    let catalog = registered.environment().callable_catalog().project();

    assert_eq!(catalog.modules().len(), 1);
    assert_eq!(
        catalog.modules()[0].module(),
        &CanonicalModulePath::crate_root()
    );
    let declaration = registered
        .symbols()
        .callable_symbols()
        .next()
        .expect("main callable symbol")
        .declaration()
        .clone();
    assert!(catalog.record(&declaration).is_some());

    let path = |segments: &[&str]| {
        ProjectCallablePath::new(
            registered.symbols().world().package().clone(),
            CanonicalModulePath::crate_root(),
            CallablePath::try_new(
                segments
                    .iter()
                    .map(|segment| CallableName::try_new(*segment).unwrap()),
            )
            .unwrap(),
        )
    };
    assert_eq!(
        catalog.binding(&path(&["main"])),
        Some(&ProjectNameBinding::Callable(declaration))
    );
    assert!(matches!(
        catalog.binding(&path(&["akane"])),
        Some(ProjectNameBinding::NonCallable {
            ty: TypeKind::Ref(entity),
            ..
        }) if entity.kind() == &EntityKind::Character
    ));
    assert!(matches!(
        catalog.binding(&path(&["character", "akane"])),
        Some(ProjectNameBinding::NonCallable {
            ty: TypeKind::Ref(entity),
            ..
        }) if entity.kind() == &EntityKind::Character
    ));
}

#[test]
fn accepted_callable_schema_uses_exact_project_nominal_identity() {
    let (root, project, world) = root_project_source(
        "callable-project-nominal",
        "struct Score { value: i32 }\nfn identity(value: Score) -> Score { value }\n",
    );
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("project nominal callable registers");
    let declaration = registered
        .symbols()
        .callable_symbols()
        .find(|symbol| symbol.declaration().name() == "identity")
        .expect("identity callable symbol")
        .declaration();
    let record = registered
        .environment()
        .callable_catalog()
        .project()
        .record(declaration)
        .expect("identity callable record");
    let parameter = record.schema().groups()[0].parameters()[0].ty();
    let CallableParameterType::Exact(TypeKind::ProjectNominal(parameter)) = parameter else {
        panic!("expected project nominal parameter, found {parameter:?}");
    };
    let TypeKind::ProjectNominal(result) = record.schema().result() else {
        panic!(
            "expected project nominal result, found {:?}",
            record.schema().result()
        );
    };

    assert_eq!(parameter.declaration(), result.declaration());
    assert!(parameter.arguments().is_empty());
    assert!(result.arguments().is_empty());
    assert_eq!(
        registered
            .environment()
            .callable_catalog()
            .nominal_resolutions()
            .roots()
            .len(),
        2,
        "parameter and result source roots must both retain accepted nominal reports"
    );
}

#[test]
fn poisoned_callable_schema_registers_recovery_and_retains_diagnostics() {
    let (root, project, world) = root_project_source(
        "callable-poisoned-nominal",
        "fn identity(value: MissingInput) -> MissingOutput { value }\n",
    );
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("poisoned callable types remain a recoverable accepted registration");
    let declaration = registered
        .symbols()
        .callable_symbols()
        .find(|symbol| symbol.declaration().name() == "identity")
        .expect("identity callable symbol")
        .declaration();
    let record = registered
        .environment()
        .callable_catalog()
        .project()
        .record(declaration)
        .expect("identity callable record");

    assert!(matches!(
        record.schema().groups()[0].parameters()[0].ty(),
        CallableParameterType::Exact(TypeKind::Error(_))
    ));
    assert!(matches!(record.schema().result(), TypeKind::Error(_)));
    let resolutions = registered
        .environment()
        .callable_catalog()
        .nominal_resolutions();
    assert_eq!(resolutions.roots().len(), 2);
    assert_eq!(resolutions.diagnostics().len(), 2);
}

#[test]
fn accepted_world_publishes_qualified_compact_and_authored_character_paths() {
    let (root, project, world) = root_project("typed-character-binding-paths");
    let manifest = sample_manifest("layers/body.png");
    let (manifest_document, backed) = backed_manifest(
        "arcweft-project://registration-tests/characters/typed-paths.awchar.json",
        &manifest,
    );
    let owner = manifest.character().clone();
    let declaration = declaration_span(&backed);
    let mut binding_paths = character_binding_paths(&owner);
    binding_paths.push(project_path(["hero"]));
    let fact = external_fact(
        owner.as_str(),
        &binding_paths,
        RegisteredExternalOwner::Character(owner.clone()),
        declaration,
    );
    let catalog = SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![backed])
        .expect("source-backed character catalog");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, manifest_document],
        vec![fact],
        vec![catalog],
    )
    .expect("typed character binding facts");
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("typed character binding paths register");

    let accepted_owner = AcceptedNominalOwnerId::Character(owner.clone());
    let accepted_records = registered
        .environment()
        .nominal_catalog()
        .exact_records_for_owner(&accepted_owner)
        .collect::<Vec<_>>();
    assert_eq!(accepted_records.len(), 3);
    assert!(accepted_records.iter().all(|record| {
        record.origin() == AcceptedNominalOrigin::Character
            && matches!(
                record.semantics(),
                AcceptedNominalSemantics::Exact(TypeKind::Ref(entity))
                    if entity.kind() == &EntityKind::Character
            )
    }));

    let expected_paths = [
        project_path(["akane"]),
        project_path(["character", "akane"]),
        project_path(["hero"]),
    ];
    let targets = registered
        .symbols()
        .scope_bindings()
        .filter(|(module, path, _)| {
            *module == &CanonicalModulePath::crate_root() && expected_paths.contains(path)
        })
        .map(|(_, _, target)| target.clone())
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 3);
    assert!(matches!(
        targets.first(),
        Some(ProjectSymbolTargetId::External(_))
    ));
    assert!(targets.windows(2).all(|pair| pair[0] == pair[1]));

    let callable_path = |path: &arcweft_lang_syntax::ast::symbol_path::ProjectSymbolPath| {
        ProjectCallablePath::new(
            registered.symbols().world().package().clone(),
            CanonicalModulePath::crate_root(),
            CallablePath::try_new(
                path.segments()
                    .iter()
                    .map(|segment| CallableName::try_new(segment.as_str()).unwrap()),
            )
            .unwrap(),
        )
    };
    for path in &expected_paths {
        assert!(matches!(
            registered
                .environment()
                .callable_catalog()
                .project()
                .binding(&callable_path(path)),
            Some(ProjectNameBinding::NonCallable {
                ty: TypeKind::Ref(entity),
                ..
            }) if entity.kind() == &EntityKind::Character
        ));
    }
}

#[test]
fn character_external_segments_do_not_require_module_identifier_grammar() {
    let (root, project, world) = root_project("external-character-segments");
    let manifest = sample_manifest_for("character.hero-pack.2d", "layers/hero-pack.png");
    let facts = one_character_facts(&root, world, &manifest);
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("external character segments register");
    let paths = registered
        .symbols()
        .scope_bindings()
        .filter(|(_, _, target)| matches!(target, ProjectSymbolTargetId::External(_)))
        .map(|(_, path, _)| {
            path.segments()
                .iter()
                .map(|segment| segment.as_str().to_owned())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    assert_eq!(
        paths,
        [
            vec![
                "character".to_owned(),
                "hero-pack".to_owned(),
                "2d".to_owned()
            ],
            vec!["hero-pack".to_owned(), "2d".to_owned()],
        ]
    );
}

#[test]
fn accepted_world_catalogues_qualified_adapter_non_callable_path() {
    let (registered, environment) =
        registered_character_and_environment("typed-adapter-binding-path");
    let path = ProjectCallablePath::new(
        registered.symbols().world().package().clone(),
        CanonicalModulePath::crate_root(),
        CallablePath::try_new([
            CallableName::try_new("adapter").unwrap(),
            CallableName::try_new("viewport").unwrap(),
        ])
        .unwrap(),
    );
    assert!(matches!(
        registered
            .environment()
            .callable_catalog()
            .project()
            .binding(&path),
        Some(ProjectNameBinding::NonCallable {
            ty: TypeKind::I32,
            ..
        })
    ));
    let declaration = registered
        .symbols()
        .scope_bindings()
        .find_map(|(_, binding, target)| {
            (binding == &project_path(["adapter", "viewport"]))
                .then_some(target)
                .and_then(|target| match target {
                    ProjectSymbolTargetId::External(declaration) => Some(*declaration),
                    ProjectSymbolTargetId::Callable(_)
                    | ProjectSymbolTargetId::Nominal(_)
                    | ProjectSymbolTargetId::Module(_) => None,
                })
        })
        .expect("qualified adapter external target");
    assert_eq!(
        registered.environment().external_owner(
            registered.symbols(),
            declaration,
            RegisteredExternalOwnerKind::Environment,
        ),
        Ok(&RegisteredExternalOwner::Environment(environment))
    );
}

#[test]
fn same_rank_callable_collision_rejects_candidate_world_before_publication() {
    use crate::{
        callable::{
            AdapterPackageId, CallableArgumentPolicy, CallableDocumentation, CallableEffectSchema,
            CallableGroupIndex, CallableGroupKind, CallableLookupKey, CallableOverloadIndex,
            CallableParameterGroup, CallableSignatureSchema, CallableValidator,
            EnvironmentCallableKind, EnvironmentCallableOwner, EnvironmentCallablePublication,
            EnvironmentCallablePublicationRecord, EnvironmentDeclarationOrdinal,
            PRODUCTION_CALLABLE_LIMITS, SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
        },
        effect_row::EffectRow,
    };

    let publication = |owner: &str, result: TypeKind| {
        let schema = CallableSignatureSchema::try_new(
            vec![
                CallableParameterGroup::try_new(
                    CallableGroupIndex::try_from_usize(0).unwrap(),
                    CallableGroupKind::Initial,
                    Vec::new(),
                    &PRODUCTION_CALLABLE_LIMITS,
                )
                .unwrap(),
            ],
            result,
            CallableEffectSchema::fixed(EffectRow::closed(crate::effects::EffectSet::default())),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::Reject,
            ),
            CallableValidator::Ordinary,
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .unwrap();
        EnvironmentCallablePublication::try_new(
            EnvironmentCallableOwner::Adapter(AdapterPackageId::try_new(owner).unwrap()),
            vec![
                EnvironmentCallablePublicationRecord::try_new(
                    EnvironmentCallableKind::Function,
                    CallableLookupKey::Free(
                        CallablePath::try_new([CallableName::try_new("collision").unwrap()])
                            .unwrap(),
                    ),
                    CallableOverloadIndex::try_from_usize(0).unwrap(),
                    schema,
                    CallableDocumentation::missing(),
                    None,
                    None,
                    EnvironmentDeclarationOrdinal::try_from_usize(0).unwrap(),
                )
                .unwrap(),
            ],
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .unwrap()
    };

    let (root, project, world) = root_project("callable-collision");
    let facts = one_character_facts(&root, world, &sample_manifest("layers/body.png"));
    let previous = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("baseline world registers");
    let (_, previous_environment, _) = previous.into_parts();
    let previous_revision = previous_environment.character_revision();
    let report = CharacterRegistrar::register(
        CharacterRegistrationRequest::new(
            Arc::new(TypeCheckEnv::standard()),
            &project,
            &facts,
            Some(&previous_environment),
        )
        .with_callable_publication(publication("adapter-a", TypeKind::I32))
        .with_callable_publication(publication("adapter-b", TypeKind::I64)),
    )
    .expect_err("same-rank providers for one key reject the candidate world");

    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind(),
        CharacterRegistrationDiagnosticKind::CallableCatalog {
            code: crate::callable::CallableDiagnosticCode::CorruptCallableCatalog,
        }
    )));
    assert_eq!(previous_environment.character_revision(), previous_revision);
}

#[test]
fn project_world_package_mismatch_rejects_registration_as_a_corrupt_catalog() {
    let (root, project, _) = root_project("callable-package-mismatch");
    let actual = CallablePackageId::try_new("registration-tests-other-package")
        .expect("different callable package");
    assert_ne!(project.package(), &actual);
    let world = ProjectSymbolWorldId::try_new(
        actual,
        root.identity().id().clone(),
        "callable-package-mismatch",
    )
    .expect("mismatched symbol world");
    let facts =
        ProjectRegistrationFacts::try_new(world, vec![Arc::clone(&root)], Vec::new(), Vec::new())
            .expect("registration facts retain their own typed world");

    let report = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect_err("registration cannot combine project and symbol-world packages");
    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind(),
        CharacterRegistrationDiagnosticKind::CallableCatalog {
            code: crate::callable::CallableDiagnosticCode::CorruptCallableCatalog,
        }
    )));
}
