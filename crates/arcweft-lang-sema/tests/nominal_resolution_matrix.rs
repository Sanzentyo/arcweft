use std::sync::Arc;

use arcweft_character::id::CharacterId;
use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{CallablePackageId, ProjectSymbolWorldId, nominal::ProjectNominalBody},
};
use arcweft_lang_sema::{
    env::{
        TypeCheckEnv,
        identity::EnvironmentBindingId,
        nominal::{
            AcceptedNominalId, AcceptedNominalOrigin, AcceptedNominalOwnerId,
            AcceptedNominalRecord, AcceptedNominalSemantics, OpenNominalArity, OpenNominalPattern,
            OpenNominalRule, OpenNominalRuleId, OpenNominalScope, RustPackageId,
        },
    },
    nominal::{
        CheckedTypeReferenceCache, GenericTypeBinding, GenericTypeScope, NominalResolutionLimits,
        ResolvedTypeRefOutcome, SelfTypeScope, TypeNameResolution, TypeResolutionInput,
        TypeResolutionInputError, TypeSourceEvidence, resolve_type_ref,
    },
    registration::{
        CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts,
        RegisteredSemanticWorld,
    },
    types::{
        CharacterNominalType, DetachedTypeOwnerId, GenericTypeOwnerId, GenericTypeParameterId,
        TypeKind,
    },
};
use arcweft_lang_syntax::{
    ast::{
        common::TextRange,
        module_path::{CanonicalModulePath, ModuleSegment},
    },
    parser::parse_source,
    types::{TypePath, TypeRef, parse_type_ref},
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

fn type_path(source: &str) -> TypePath {
    let authored = parse_type_ref(source).expect("matrix type path parses");
    let TypeRef::Path(path) = authored.value() else {
        panic!("matrix path is direct")
    };
    path.clone()
}

fn resolve_detached(
    source: &str,
    environment: &TypeCheckEnv,
    module: Option<&CanonicalModulePath>,
    generics: &GenericTypeScope,
    self_scope: SelfTypeScope,
) -> arcweft_lang_sema::nominal::TypeResolutionReport {
    let authored = parse_type_ref(source).expect("matrix type parses");
    resolve_type_ref(&TypeResolutionInput::detached(
        &authored,
        module,
        environment,
        generics,
        self_scope,
        NominalResolutionLimits::PRODUCTION,
    ))
    .expect("detached matrix input is valid")
}

fn generic(name: &str, owner: u64) -> (GenericTypeScope, GenericTypeParameterId) {
    let parameter = GenericTypeParameterId::new(
        GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(owner)),
        0,
    );
    let scope = GenericTypeScope::try_new([GenericTypeBinding::new(
        parameter.clone(),
        ModuleSegment::new(name).expect("matrix generic name"),
        TypeSourceEvidence::detached(TextRange::new(0, name.len())),
    )])
    .expect("matrix generic scope");
    (scope, parameter)
}

fn registered(source: &str, profile: &str) -> RegisteredSemanticWorld {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("memory:///{profile}.arcw"))
                .expect("matrix source ID"),
            SourceName::path(format!("memory:///{profile}.arcw")),
            source,
        )
        .expect("matrix source document"),
    );
    let parsed = parse_source(source);
    assert!(
        parsed.errors().is_empty(),
        "matrix fixture parses: {profile}"
    );
    let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("matrix fixture lowers");
    let package = CallablePackageId::try_new(format!("matrix-{profile}")).expect("matrix package");
    let project = HirProject::new(
        package.as_str(),
        [HirProjectModule::try_new(
            CanonicalModulePath::crate_root(),
            document.identity().clone(),
            hir,
        )
        .expect("matrix root module")],
    )
    .expect("matrix project");
    let world = ProjectSymbolWorldId::try_new(package, document.identity().id().clone(), profile)
        .expect("matrix world");
    let facts = ProjectRegistrationFacts::try_new(world, vec![document], Vec::new(), Vec::new())
        .expect("matrix registration facts");
    CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::standard()),
        &project,
        &facts,
        None,
    ))
    .expect("matrix semantic world")
}

fn field_type(
    world: &RegisteredSemanticWorld,
) -> &arcweft_lang_hir::symbol::nominal::SourceBackedTypeRef {
    let declaration = world
        .symbols()
        .nominal_symbols()
        .next()
        .expect("matrix struct");
    let ProjectNominalBody::Struct { fields } = declaration.body() else {
        panic!("matrix declaration is struct")
    };
    fields.first().expect("matrix field").ty()
}

fn accepted_input<'a>(
    authored: &'a arcweft_lang_hir::symbol::nominal::SourceBackedTypeRef,
    module: &'a CanonicalModulePath,
    world: &'a RegisteredSemanticWorld,
    generics: &'a GenericTypeScope,
    self_scope: SelfTypeScope,
) -> Result<TypeResolutionInput<'a>, TypeResolutionInputError> {
    TypeResolutionInput::accepted(
        authored,
        module,
        world.symbols(),
        world.environment().nominal_world(),
        generics,
        self_scope,
        NominalResolutionLimits::PRODUCTION,
    )
}

fn external_catalog_environment(
    owner: &EnvironmentBindingId,
    character: &CharacterId,
) -> TypeCheckEnv {
    let records = [
        (
            "EXT-ADAPTER",
            "adapter.Context",
            AcceptedNominalOwnerId::RustPackage(
                RustPackageId::try_new("matrix-adapter").expect("package"),
            ),
            AcceptedNominalSemantics::Opaque,
            AcceptedNominalOrigin::Adapter,
        ),
        (
            "EXT-CHARACTER",
            "CharacterMatrix",
            AcceptedNominalOwnerId::Character(character.clone()),
            AcceptedNominalSemantics::Character(CharacterNominalType::Look {
                character: character.clone(),
            }),
            AcceptedNominalOrigin::Character,
        ),
        (
            "EXT-ENUM-INVENTORY",
            "InputEvent",
            AcceptedNominalOwnerId::Environment(owner.clone()),
            AcceptedNominalSemantics::Opaque,
            AcceptedNominalOrigin::EnumInventory,
        ),
        (
            "EXT-TEST",
            "FixtureState",
            AcceptedNominalOwnerId::Environment(owner.clone()),
            AcceptedNominalSemantics::Exact(TypeKind::Bool),
            AcceptedNominalOrigin::Test,
        ),
    ];
    records.into_iter().fold(
        TypeCheckEnv::standard(),
        |environment, (id, path, record_owner, semantics, origin)| {
            environment
                .try_with_nominal_record(
                    AcceptedNominalRecord::try_new(
                        AcceptedNominalId::new(record_owner, type_path(path)),
                        0,
                        semantics,
                        origin,
                        None,
                    )
                    .unwrap_or_else(|error| panic!("{id}: record is valid: {error}")),
                )
                .unwrap_or_else(|error| panic!("{id}: record registers: {error}"))
        },
    )
}

fn environment_with_open_nominal_rules(
    environment: TypeCheckEnv,
    owner: EnvironmentBindingId,
    child: &CanonicalModulePath,
) -> TypeCheckEnv {
    let environment = environment
        .try_with_open_nominal_rule(
            OpenNominalRule::try_new(
                OpenNominalRuleId::new(owner.clone(), 0),
                OpenNominalScope::DetachedOnly,
                OpenNominalPattern::Exact(type_path("ScratchType")),
                OpenNominalArity::Exact(0),
                None,
            )
            .expect("OPEN-DETACHED: rule"),
        )
        .expect("OPEN-DETACHED: registration");
    let environment = environment
        .try_with_open_nominal_rule(
            OpenNominalRule::try_new(
                OpenNominalRuleId::new(owner.clone(), 1),
                OpenNominalScope::DetachedOnly,
                OpenNominalPattern::Exact(type_path("third_party.Handle")),
                OpenNominalArity::Exact(0),
                None,
            )
            .expect("OPEN-EXACT: rule"),
        )
        .expect("OPEN-EXACT: registration");
    let environment = environment
        .try_with_open_nominal_rule(
            OpenNominalRule::try_new(
                OpenNominalRuleId::new(owner.clone(), 2),
                OpenNominalScope::DetachedOnly,
                OpenNominalPattern::Namespace {
                    prefix: type_path("adapter.generated"),
                    min_tail_segments: 1,
                    max_tail_segments: 1,
                },
                OpenNominalArity::Exact(0),
                None,
            )
            .expect("OPEN-NAMESPACE: rule"),
        )
        .expect("OPEN-NAMESPACE: registration");
    environment
        .try_with_open_nominal_rule(
            OpenNominalRule::try_new(
                OpenNominalRuleId::new(owner, 3),
                OpenNominalScope::ModuleSubtree(child.clone()),
                OpenNominalPattern::Exact(type_path("PluginType")),
                OpenNominalArity::Exact(0),
                None,
            )
            .expect("OPEN-SUBTREE: rule"),
        )
        .expect("OPEN-SUBTREE: registration")
}

fn assert_external_and_open_catalog_cases(environment: &TypeCheckEnv, child: &CanonicalModulePath) {
    for (id, source) in [
        ("EXT-ADAPTER", "adapter.Context"),
        ("EXT-CHARACTER", "CharacterMatrix"),
        ("EXT-ENUM-INVENTORY", "InputEvent"),
        ("EXT-STANDARD", "ArcError"),
        ("EXT-TEST", "FixtureState"),
        ("OPEN-DETACHED", "ScratchType"),
        ("OPEN-EXACT", "third_party.Handle"),
        ("OPEN-NAMESPACE", "adapter.generated.Widget"),
    ] {
        let report = resolve_detached(
            source,
            environment,
            None,
            &GenericTypeScope::empty(),
            SelfTypeScope::Absent,
        );
        assert!(
            matches!(report.outcome(), ResolvedTypeRefOutcome::Complete(_)),
            "{id}: typed catalog entry resolves completely"
        );
        assert!(
            report.diagnostics().is_empty(),
            "{id}: no nominal diagnostics"
        );
    }
    let subtree = resolve_detached(
        "PluginType",
        environment,
        Some(child),
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
    );
    assert!(
        matches!(
            subtree.outcome().product().nodes()[0].outcome(),
            TypeNameResolution::Open(_)
        ),
        "OPEN-SUBTREE: subtree rule is selected"
    );
}

#[test]
fn matrix_external_and_open_catalog_cases_are_direct() {
    let owner = EnvironmentBindingId::try_new("matrix-open").expect("matrix owner");
    let character = CharacterId::try_new("character.matrix").expect("matrix character");
    let child =
        CanonicalModulePath::crate_root().join(ModuleSegment::new("plugin").expect("child"));
    let environment = environment_with_open_nominal_rules(
        external_catalog_environment(&owner, &character),
        owner,
        &child,
    );
    assert_external_and_open_catalog_cases(&environment, &child);
}

#[test]
fn matrix_generic_and_self_cases_are_direct() {
    let environment = TypeCheckEnv::standard();
    let (outer, outer_id) = generic("T", 1);
    let (inner, inner_id) = generic("T", 2);
    let (environment_shadow, environment_shadow_id) = generic("ArcError", 3);
    for (id, source, scope, expected) in [
        ("GEN-NESTED", "T", &inner, &inner_id),
        (
            "GEN-SHADOW-ENV",
            "ArcError",
            &environment_shadow,
            &environment_shadow_id,
        ),
    ] {
        let report = resolve_detached(source, &environment, None, scope, SelfTypeScope::Absent);
        assert!(
            matches!(report.outcome().product().recovered(), TypeKind::GenericParam(actual) if actual == expected),
            "{id}: nearest generic identity wins"
        );
    }
    let outer_report = resolve_detached("T", &environment, None, &outer, SelfTypeScope::Absent);
    assert!(
        matches!(outer_report.outcome().product().recovered(), TypeKind::GenericParam(actual) if actual == &outer_id),
        "GEN-NESTED: outer binding remains observable outside inner scope"
    );
    let (project_shadow, project_shadow_id) = generic("T", 4);
    let project_report = resolve_detached(
        "T",
        &environment,
        None,
        &project_shadow,
        SelfTypeScope::Absent,
    );
    assert!(
        matches!(project_report.outcome().product().recovered(), TypeKind::GenericParam(actual) if actual == &project_shadow_id),
        "GEN-SHADOW-PROJECT: unqualified generic uses typed generic identity"
    );

    for (id, scope, expected) in [
        (
            "SELF-IMPL",
            SelfTypeScope::Known(TypeKind::Bool),
            TypeKind::Bool,
        ),
        (
            "SELF-TRAIT",
            SelfTypeScope::Known(TypeKind::String),
            TypeKind::String,
        ),
    ] {
        let report = resolve_detached(
            "Self",
            &environment,
            None,
            &GenericTypeScope::empty(),
            scope,
        );
        assert!(
            matches!(report.outcome().product().recovered(), actual if actual == &expected),
            "{id}: supplied typed Self resolves without diagnostic"
        );
        assert!(
            report.diagnostics().is_empty(),
            "{id}: no duplicate Self diagnostic"
        );
    }
    let unavailable = resolve_detached(
        "Self",
        &environment,
        None,
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
    );
    let poison = unavailable.poisons()[0].id();
    let poisoned = resolve_detached(
        "Self",
        &environment,
        None,
        &GenericTypeScope::empty(),
        SelfTypeScope::Poisoned(poison),
    );
    assert!(
        poisoned.diagnostics().is_empty(),
        "SELF-POISON: inherited poison emits no second diagnostic"
    );
    assert!(
        matches!(poisoned.outcome().product().nodes()[0].outcome(), TypeNameResolution::Poisoned(actual) if *actual == poison),
        "SELF-POISON: inherited typed poison is reused"
    );
}

fn assert_stale_input_rejections(
    world: &RegisteredSemanticWorld,
    authored: &arcweft_lang_hir::symbol::nominal::SourceBackedTypeRef,
    module: &CanonicalModulePath,
    empty: &GenericTypeScope,
) {
    let stale_source = accepted_input(authored, module, world, empty, SelfTypeScope::Absent);
    assert!(
        stale_source.is_ok(),
        "STALE-SOURCE: same source identity remains valid before mismatch fixture"
    );
    let foreign_document = SourceDocument::try_new(
        SourceDocumentId::try_new("memory:///matrix-foreign.arcw").expect("STALE-SOURCE: ID"),
        SourceName::path("memory:///matrix-foreign.arcw"),
        "T",
    )
    .expect("STALE-SOURCE: document");
    let foreign_authored = arcweft_lang_hir::symbol::nominal::SourceBackedTypeRef::try_bind(
        parse_type_ref("T").expect("STALE-SOURCE: parsed type"),
        &foreign_document,
        foreign_document.identity(),
    )
    .expect("STALE-SOURCE: source binding");
    let Err(stale_source) = accepted_input(
        &foreign_authored,
        module,
        world,
        empty,
        SelfTypeScope::Absent,
    ) else {
        panic!("STALE-SOURCE: foreign typed source rejects input");
    };
    assert!(
        matches!(
            stale_source,
            TypeResolutionInputError::SourceMismatch { .. }
        ),
        "STALE-SOURCE: typed document mismatch is retained"
    );
    let other = registered("pub struct Boxed<T> { value: T }", "matrix-other");
    let Err(stale_world) = TypeResolutionInput::accepted(
        authored,
        module,
        world.symbols(),
        other.environment().nominal_world(),
        empty,
        SelfTypeScope::Absent,
        NominalResolutionLimits::PRODUCTION,
    ) else {
        panic!("STALE-WORLD: distinct world rejects input");
    };
    assert!(
        matches!(stale_world, TypeResolutionInputError::StaleWorld { .. }),
        "STALE-WORLD: typed world mismatch is retained"
    );
}

#[test]
fn matrix_cache_and_stale_input_cases_are_direct() {
    let world = registered("pub struct Boxed<T> { value: T }", "matrix-cache");
    let authored = field_type(&world);
    let module = CanonicalModulePath::crate_root();
    let (first_scope, _) = generic("T", 10);
    let (second_scope, _) = generic("T", 11);
    let empty = GenericTypeScope::empty();
    let mut cache = CheckedTypeReferenceCache::default();
    let first = cache
        .resolve(
            &accepted_input(
                authored,
                &module,
                &world,
                &first_scope,
                SelfTypeScope::Absent,
            )
            .expect("CACHE-HIT: input"),
        )
        .expect("CACHE-HIT: first");
    let second = cache
        .resolve(
            &accepted_input(
                authored,
                &module,
                &world,
                &first_scope,
                SelfTypeScope::Absent,
            )
            .expect("CACHE-HIT: repeated input"),
        )
        .expect("CACHE-HIT: second");
    assert!(
        Arc::ptr_eq(&first, &second) && cache.hits() == 1 && cache.misses() == 1,
        "CACHE-HIT: exact accepted key reuses one report"
    );
    cache
        .resolve(
            &accepted_input(
                authored,
                &module,
                &world,
                &second_scope,
                SelfTypeScope::Absent,
            )
            .expect("CACHE-GENERIC-MISS: input"),
        )
        .expect("CACHE-GENERIC-MISS: resolve");
    cache
        .resolve(
            &accepted_input(
                authored,
                &module,
                &world,
                &GenericTypeScope::empty(),
                SelfTypeScope::Known(TypeKind::Bool),
            )
            .expect("CACHE-SELF-MISS: first input"),
        )
        .expect("CACHE-SELF-MISS: first resolve");
    cache
        .resolve(
            &accepted_input(
                authored,
                &module,
                &world,
                &GenericTypeScope::empty(),
                SelfTypeScope::Known(TypeKind::String),
            )
            .expect("CACHE-SELF-MISS: second input"),
        )
        .expect("CACHE-SELF-MISS: second resolve");
    assert_eq!(
        cache.len(),
        4,
        "CACHE-GENERIC-MISS/CACHE-SELF-MISS: generic and Self fingerprints separate entries"
    );

    let revised = registered(
        "pub struct Boxed<T> { value: T }\npub struct RevisionMarker { value: i32 }",
        "matrix-cache",
    );
    let revised_authored = field_type(&revised);
    cache
        .resolve(
            &accepted_input(
                revised_authored,
                &module,
                &revised,
                &empty,
                SelfTypeScope::Absent,
            )
            .expect("CACHE-REV-MISS: revised input"),
        )
        .expect("CACHE-REV-MISS: revised resolution");
    assert_eq!(
        cache.len(),
        5,
        "CACHE-REV-MISS: revised accepted input creates a distinct entry"
    );
    assert_stale_input_rejections(&world, authored, &module, &empty);
}
