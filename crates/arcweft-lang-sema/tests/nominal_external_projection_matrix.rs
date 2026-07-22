use std::sync::Arc;

use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{
        CallablePackageId, ExternalDeclarationSeed, ProjectDirectBinding, ProjectSymbolWorldId,
    },
};
use arcweft_lang_sema::{
    env::{TypeCheckEnv, identity::EnvironmentBindingId, nominal::RustPackageId},
    nominal::{
        ExternalNominalResolution, GenericTypeBinding, GenericTypeScope, NominalResolutionLimits,
        NominalTypeDiagnosticCode, ResolvedTypeRefOutcome, SelfTypeScope, TypeNameResolution,
        TypeResolutionInput, TypeSourceEvidence, resolve_type_ref,
    },
    registration::{
        CharacterRegistrar, CharacterRegistrationRequest, ExternalRegistrationFact,
        ProjectRegistrationFacts,
    },
    types::{DetachedTypeOwnerId, GenericTypeOwnerId, GenericTypeParameterId, TypeKind},
};
use arcweft_lang_syntax::{
    ast::{
        common::{TextRange, Visibility},
        module_path::{CanonicalModulePath, ModulePathRoot, ModuleSegment},
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
    },
    parser::parse_source,
    types::{TypePath, TypeRef, parse_type_ref},
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

fn path(source: &str) -> TypePath {
    let authored = parse_type_ref(source).expect("matrix path parses");
    let TypeRef::Path(path) = authored.value() else {
        panic!("matrix fixture is a direct path");
    };
    path.clone()
}

fn detached(
    source: &str,
    environment: &TypeCheckEnv,
    generics: &GenericTypeScope,
    self_scope: SelfTypeScope,
) -> arcweft_lang_sema::nominal::TypeResolutionReport {
    let authored = parse_type_ref(source).expect("matrix type parses");
    resolve_type_ref(&TypeResolutionInput::detached(
        &authored,
        None,
        environment,
        generics,
        self_scope,
        NominalResolutionLimits::PRODUCTION,
    ))
    .expect("matrix detached resolution is valid")
}

fn generic(name: &str) -> (GenericTypeScope, GenericTypeParameterId) {
    let id = GenericTypeParameterId::new(
        GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(901)),
        0,
    );
    let scope = GenericTypeScope::try_new([GenericTypeBinding::new(
        id.clone(),
        ModuleSegment::new(name).expect("matrix generic name"),
        TypeSourceEvidence::detached(TextRange::new(0, name.len())),
    )])
    .expect("matrix generic scope");
    (scope, id)
}

fn external_rust_project(
    document: &Arc<SourceDocument>,
    package: &CallablePackageId,
    module: &CanonicalModulePath,
) -> HirProject {
    const SOURCE: &str = "pub struct Use { value: rust.Packet }";
    let parsed = parse_source(SOURCE);
    assert!(parsed.errors().is_empty(), "matrix fixture parses");
    let hir = lower_document_to_hir(document, parsed.typed_tree()).expect("matrix fixture lowers");
    HirProject::new(
        package.as_str(),
        [
            HirProjectModule::try_new(module.clone(), document.identity().clone(), hir)
                .expect("root module"),
        ],
    )
    .expect("project")
}

fn external_rust_registration_fact(
    document: &SourceDocument,
    module: CanonicalModulePath,
    owner: EnvironmentBindingId,
) -> ExternalRegistrationFact {
    const SOURCE: &str = "pub struct Use { value: rust.Packet }";
    let declaration = document
        .span(arcweft_source::SourceRange::new(0, SOURCE.len()))
        .expect("span");
    let binding = ProjectDirectBinding::try_new(
        module,
        ProjectSymbolPath::new(
            ModulePathRoot::ImplicitCrate,
            [
                ProjectSymbolSegment::try_new("rust").expect("namespace segment"),
                ProjectSymbolSegment::try_new("Packet").expect("leaf segment"),
            ],
        )
        .expect("binding path"),
        Some(Visibility::Public),
        declaration.clone(),
        false,
    )
    .expect("external binding");
    let seed = ExternalDeclarationSeed::try_new(
        SymbolPath::try_new(
            ModulePathRoot::ImplicitCrate,
            vec![ModuleSegment::new("rust").expect("namespace segment")],
            "Packet",
        )
        .expect("external path"),
        Some(Visibility::Public),
        declaration.clone(),
        vec![binding],
    )
    .expect("external declaration");
    ExternalRegistrationFact::new(
        seed,
        arcweft_lang_sema::registration::RegisteredExternalOwner::Environment(owner),
        declaration,
    )
}

fn use_field_type(
    world: &arcweft_lang_sema::registration::RegisteredSemanticWorld,
) -> arcweft_lang_hir::symbol::nominal::SourceBackedTypeRef {
    let declaration = world
        .symbols()
        .nominal_symbols()
        .next()
        .expect("Use declaration");
    let arcweft_lang_hir::symbol::nominal::ProjectNominalBody::Struct { fields } =
        declaration.body()
    else {
        panic!("Use is a struct");
    };
    fields.first().expect("Use field").ty().clone()
}

fn external_rust_fixture() -> (
    arcweft_lang_sema::registration::RegisteredSemanticWorld,
    CanonicalModulePath,
    arcweft_lang_hir::symbol::nominal::SourceBackedTypeRef,
    TypeKind,
) {
    const SOURCE: &str = "pub struct Use { value: rust.Packet }";
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("memory:///external-rust-matrix.arcw").expect("source ID"),
            SourceName::path("memory:///external-rust-matrix.arcw"),
            SOURCE,
        )
        .expect("source document"),
    );
    let package = CallablePackageId::try_new("external-rust-matrix").expect("package");
    let module = CanonicalModulePath::crate_root();
    let project = external_rust_project(&document, &package, &module);
    let owner = EnvironmentBindingId::try_new("adapter.packet").expect("environment owner");
    let package_id = RustPackageId::try_new("packet-rust").expect("Rust package");
    let environment = TypeCheckEnv::standard()
        .try_with_rust_type_export(package_id, path("Packet"))
        .expect("Rust type export");
    let packet = detached(
        "Packet",
        &environment,
        &GenericTypeScope::empty(),
        SelfTypeScope::Absent,
    )
    .outcome()
    .product()
    .recovered()
    .clone();
    let environment = environment.with_symbol(owner.as_str(), packet.clone());
    let world_id = ProjectSymbolWorldId::try_new(
        package,
        document.identity().id().clone(),
        "external-rust-matrix",
    )
    .expect("world");
    let facts = ProjectRegistrationFacts::try_new(
        world_id,
        vec![Arc::clone(&document)],
        vec![external_rust_registration_fact(
            document.as_ref(),
            module.clone(),
            owner,
        )],
        Vec::new(),
    )
    .expect("registration facts");
    let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(environment),
        &project,
        &facts,
        None,
    ))
    .expect("registered semantic world");
    let authored = use_field_type(&world);
    (world, module, authored, packet)
}

fn accepted_field_report(source: &str) -> arcweft_lang_sema::nominal::TypeResolutionReport {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("memory:///projection-unknown-matrix.arcw")
                .expect("source ID"),
            SourceName::path("memory:///projection-unknown-matrix.arcw"),
            source,
        )
        .expect("source document"),
    );
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "projection fixture parses");
    let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("projection lowers");
    let package = CallablePackageId::try_new("projection-unknown-matrix").expect("package");
    let module = CanonicalModulePath::crate_root();
    let project = HirProject::new(
        package.as_str(),
        [
            HirProjectModule::try_new(module.clone(), document.identity().clone(), hir)
                .expect("root module"),
        ],
    )
    .expect("project");
    let facts = ProjectRegistrationFacts::try_new(
        ProjectSymbolWorldId::try_new(
            package,
            document.identity().id().clone(),
            "projection-unknown-matrix",
        )
        .expect("world"),
        vec![Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::standard()),
        &project,
        &facts,
        None,
    ))
    .expect("registered semantic world");
    let authored = {
        let declaration = world
            .symbols()
            .nominal_symbols()
            .next()
            .expect("Use declaration");
        let arcweft_lang_hir::symbol::nominal::ProjectNominalBody::Struct { fields } =
            declaration.body()
        else {
            panic!("Use is a struct");
        };
        fields.first().expect("Use field").ty().clone()
    };
    resolve_type_ref(
        &TypeResolutionInput::accepted(
            &authored,
            &module,
            world.symbols(),
            world.environment().nominal_world(),
            &GenericTypeScope::empty(),
            SelfTypeScope::Absent,
            NominalResolutionLimits::PRODUCTION,
        )
        .expect("accepted projection input"),
    )
    .expect("accepted projection resolution")
}

#[test]
fn external_catalog_rows_resolve_to_typed_nominal_facts() {
    let environment = TypeCheckEnv::standard();
    for (id, source, expected) in [
        ("EXT-DOMAIN", "Duration", TypeKind::Duration),
        (
            "EXT-NOMINAL-RECORD",
            "DialogueContent",
            TypeKind::Named("DialogueContent".to_owned()),
        ),
    ] {
        let report = detached(
            source,
            &environment,
            &GenericTypeScope::empty(),
            SelfTypeScope::Absent,
        );
        assert!(
            matches!(report.outcome(), ResolvedTypeRefOutcome::Complete(_)),
            "{id}: complete"
        );
        assert_eq!(
            report.outcome().product().recovered(),
            &expected,
            "{id}: exact type"
        );
        assert!(
            matches!(
                report.outcome().product().nodes()[0].outcome(),
                TypeNameResolution::Accepted(_)
            ),
            "{id}: typed accepted origin"
        );
        assert!(report.diagnostics().is_empty(), "{id}: no diagnostics");
    }
}

#[test]
fn external_rust_row_uses_external_identity_and_accepted_rust_type() {
    let (world, module, authored, packet) = external_rust_fixture();
    let report = resolve_type_ref(
        &TypeResolutionInput::accepted(
            &authored,
            &module,
            world.symbols(),
            world.environment().nominal_world(),
            &GenericTypeScope::empty(),
            SelfTypeScope::Absent,
            NominalResolutionLimits::PRODUCTION,
        )
        .expect("EXT-RUST: accepted input"),
    )
    .expect("EXT-RUST: resolution");
    assert!(
        matches!(report.outcome(), ResolvedTypeRefOutcome::Complete(_)),
        "EXT-RUST: complete"
    );
    assert_eq!(
        report.outcome().product().recovered(),
        &packet,
        "EXT-RUST: preserves Rust nominal identity"
    );
    assert!(
        matches!(report.outcome().product().nodes()[0].outcome(), TypeNameResolution::External(ExternalNominalResolution::Exact { ty, .. }) if ty == &packet),
        "EXT-RUST: project external owns the typed projection"
    );
    assert!(report.diagnostics().is_empty(), "EXT-RUST: no diagnostics");
}

#[test]
fn projection_rows_preserve_valid_subjects_and_suppress_unknown_follow_ons() {
    let environment = TypeCheckEnv::standard();
    let (generics, generic_id) = generic("T");
    let generic = detached("T::Item", &environment, &generics, SelfTypeScope::Absent);
    assert!(
        matches!(generic.outcome(), ResolvedTypeRefOutcome::Complete(_)),
        "PROJ-GENERIC: complete"
    );
    assert!(
        matches!(generic.outcome().product().recovered(), TypeKind::Projection { subject, assoc, .. } if subject.as_ref() == &TypeKind::GenericParam(generic_id) && assoc == "Item"),
        "PROJ-GENERIC: preserves generic subject"
    );
    assert!(
        generic
            .outcome()
            .product()
            .nodes()
            .iter()
            .any(|node| matches!(node.outcome(), TypeNameResolution::Projection)),
        "PROJ-GENERIC: projection fact"
    );
    assert!(
        generic.diagnostics().is_empty(),
        "PROJ-GENERIC: no nominal diagnostic"
    );

    let self_projection = detached(
        "Self::Item",
        &environment,
        &GenericTypeScope::empty(),
        SelfTypeScope::Known(TypeKind::String),
    );
    assert!(
        matches!(
            self_projection.outcome(),
            ResolvedTypeRefOutcome::Complete(_)
        ),
        "PROJ-SELF: complete"
    );
    assert!(
        matches!(self_projection.outcome().product().recovered(), TypeKind::Projection { subject, assoc, .. } if subject.as_ref() == &TypeKind::String && assoc == "Item"),
        "PROJ-SELF: preserves Self subject"
    );
    assert!(
        self_projection.diagnostics().is_empty(),
        "PROJ-SELF: no nominal diagnostic"
    );

    let unknown = accepted_field_report("pub struct Use { value: Missing::Item }");
    assert!(
        matches!(unknown.outcome(), ResolvedTypeRefOutcome::Poisoned(_)),
        "PROJ-UNKNOWN: subject poison is authoritative"
    );
    assert_eq!(
        unknown.diagnostics().len(),
        1,
        "PROJ-UNKNOWN: no projection follow-on"
    );
    assert_eq!(
        unknown.diagnostics()[0].kind().code(),
        NominalTypeDiagnosticCode::UnknownType,
        "PROJ-UNKNOWN: subject diagnostic"
    );
}

#[test]
fn diagnostic_order_is_stable_across_catalog_insertion_order() {
    let first = TypeCheckEnv::new()
        .try_with_rust_type_export(
            RustPackageId::try_new("zeta").expect("package"),
            path("Zeta"),
        )
        .expect("first Rust export")
        .try_with_rust_type_export(
            RustPackageId::try_new("alpha").expect("package"),
            path("Alpha"),
        )
        .expect("second Rust export");
    let second = TypeCheckEnv::new()
        .try_with_rust_type_export(
            RustPackageId::try_new("alpha").expect("package"),
            path("Alpha"),
        )
        .expect("first Rust export")
        .try_with_rust_type_export(
            RustPackageId::try_new("zeta").expect("package"),
            path("Zeta"),
        )
        .expect("second Rust export");
    let reports = [&first, &second].map(|environment| {
        detached(
            "(Self, Self, Self)",
            environment,
            &GenericTypeScope::empty(),
            SelfTypeScope::Absent,
        )
    });
    let diagnostics = reports.each_ref().map(|report| {
        report
            .diagnostics()
            .iter()
            .map(|diagnostic| (diagnostic.kind().code(), diagnostic.primary().local()))
            .collect::<Vec<_>>()
    });
    assert_eq!(
        diagnostics[0], diagnostics[1],
        "ORDER-DIAGNOSTICS: catalog insertion cannot change diagnostics"
    );
    assert_eq!(
        diagnostics[0],
        vec![
            (
                NominalTypeDiagnosticCode::SelfUnavailable,
                TextRange::new(1, 5)
            ),
            (
                NominalTypeDiagnosticCode::SelfUnavailable,
                TextRange::new(7, 11)
            ),
            (
                NominalTypeDiagnosticCode::SelfUnavailable,
                TextRange::new(13, 17)
            ),
        ],
        "ORDER-DIAGNOSTICS: source range and diagnostic code define the stable sequence",
    );
    assert_eq!(
        reports[0].omitted_diagnostics(),
        reports[1].omitted_diagnostics()
    );
}
