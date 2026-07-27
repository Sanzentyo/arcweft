use std::sync::Arc;

use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{CallablePackageId, ProjectSymbolWorldId, nominal::ProjectNominalBody},
};
use arcweft_lang_sema::{
    env::TypeCheckEnv,
    nominal::{
        GenericTypeScope, NominalResolutionLimits, NominalTypeDiagnosticCode,
        ResolvedTypeRefOutcome, SelfTypeScope, TypeArityExpectation, TypeArityTarget,
        TypeNameResolution, TypeResolutionFailure, TypeResolutionInput, TypeResolutionReport,
        resolve_type_ref,
    },
    registration::{
        CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts,
        RegisteredSemanticWorld,
    },
    types::{ArrayLength, TypeKind},
};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath,
    parser::{ParseOptions, parse_document_with_source},
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

fn registered(id: &str, source: &str) -> RegisteredSemanticWorld {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("memory:///{id}.arcw"))
                .unwrap_or_else(|error| panic!("{id}: source ID: {error}")),
            SourceName::path(format!("memory:///{id}.arcw")),
            source,
        )
        .unwrap_or_else(|error| panic!("{id}: source document: {error}")),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    assert!(parsed.errors().is_empty(), "{id}: fixture parses");
    let hir = lower_document_to_hir(&document, parsed.typed_tree())
        .unwrap_or_else(|error| panic!("{id}: fixture lowers: {error:?}"));
    let package = CallablePackageId::try_new(format!("nominal-{id}"))
        .unwrap_or_else(|error| panic!("{id}: package ID: {error}"));
    let project = HirProject::new(
        package.as_str(),
        [HirProjectModule::try_new(
            CanonicalModulePath::crate_root(),
            document.identity().clone(),
            hir,
        )
        .unwrap_or_else(|error| panic!("{id}: root module: {error}"))],
    )
    .unwrap_or_else(|error| panic!("{id}: project: {error}"));
    let world = ProjectSymbolWorldId::try_new(package, document.identity().id().clone(), id)
        .unwrap_or_else(|error| panic!("{id}: world ID: {error}"));
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![document],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap_or_else(|error| panic!("{id}: registration facts: {error:?}"));
    CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::standard()),
        &project,
        &facts,
        None,
    ))
    .unwrap_or_else(|error| panic!("{id}: semantic registration: {error:?}"))
}

fn field_report(id: &str, source: &str) -> TypeResolutionReport {
    struct_field_report(id, source, None)
}

fn named_struct_field_report(id: &str, source: &str, owner: &str) -> TypeResolutionReport {
    struct_field_report(id, source, Some(owner))
}

fn struct_field_report(id: &str, source: &str, owner: Option<&str>) -> TypeResolutionReport {
    let world = registered(id, source);
    let declaration = world
        .symbols()
        .nominal_symbols()
        .find(|declaration| {
            matches!(declaration.body(), ProjectNominalBody::Struct { .. })
                && owner.is_none_or(|owner| declaration.id().name().as_str() == owner)
        })
        .unwrap_or_else(|| match owner {
            Some(owner) => panic!("{id}: fixture has struct declaration `{owner}`"),
            None => panic!("{id}: fixture has a struct declaration"),
        });
    let ProjectNominalBody::Struct { fields } = declaration.body() else {
        panic!("{id}: selected declaration is a struct")
    };
    let authored = fields
        .first()
        .unwrap_or_else(|| panic!("{id}: fixture struct has a field"))
        .ty();
    resolve_type_ref(
        &TypeResolutionInput::accepted(
            authored,
            &CanonicalModulePath::crate_root(),
            world.symbols(),
            world.environment().nominal_world(),
            &GenericTypeScope::empty(),
            SelfTypeScope::Absent,
            NominalResolutionLimits::PRODUCTION,
        )
        .unwrap_or_else(|error| panic!("{id}: accepted resolver input: {error:?}")),
    )
    .unwrap_or_else(|error| panic!("{id}: resolver executes: {error:?}"))
}

fn assert_codes(id: &str, report: &TypeResolutionReport, codes: &[NominalTypeDiagnosticCode]) {
    let actual = report
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.kind().code())
        .collect::<Vec<_>>();
    assert_eq!(actual, codes, "{id}: exact authoritative diagnostic codes");
}

#[test]
fn als_normal_alias_expansions_retain_typed_facts() {
    for (id, source, expected, aliases) in [
        (
            "ALS-ZERO",
            "pub type A = i32\npub struct Use { value: A }",
            TypeKind::I32,
            1,
        ),
        (
            "ALS-CHAIN",
            "pub type A<T> = B<T>\npub type B<U> = Result<U, ArcError>\npub struct Use { value: A<i32> }",
            TypeKind::Result {
                ok: Box::new(TypeKind::I32),
                error: Box::new(TypeKind::Named("ArcError".to_owned())),
            },
            2,
        ),
        (
            "ALS-CHAIN-NESTED",
            "pub type A<T> = Option<B<Vec<T>>>\npub type B<U> = Result<U, ArcError>\npub struct Use { value: A<i32> }",
            TypeKind::Option(Box::new(TypeKind::Result {
                ok: Box::new(TypeKind::Vec(Box::new(TypeKind::I32))),
                error: Box::new(TypeKind::Named("ArcError".to_owned())),
            })),
            2,
        ),
        (
            "ALS-CAPTURE",
            "pub type Outer<T> = Inner<T>\npub type Inner<T> = T\npub struct Use { value: Outer<i32> }",
            TypeKind::I32,
            2,
        ),
    ] {
        let report = field_report(id, source);
        assert!(
            matches!(report.outcome(), ResolvedTypeRefOutcome::Complete(_)),
            "{id}: valid alias expansion is complete"
        );
        assert_codes(id, &report, &[]);
        let product = report.outcome().product();
        assert_eq!(
            product.recovered(),
            &expected,
            "{id}: normalized semantic type"
        );
        assert_eq!(
            product.aliases().len(),
            aliases,
            "{id}: one fact per alias expansion"
        );
        let expects_substitution = id != "ALS-ZERO";
        assert!(
            product.aliases().iter().all(|fact| {
                fact.use_source().project().is_some()
                    && fact.substitution().is_empty() != expects_substitution
            }),
            "{id}: alias facts retain accepted source and substitutions"
        );
    }
}

#[test]
fn als_rejected_alias_applications_preserve_outer_recovery() {
    for (id, source, code) in [
        (
            "ALS-EXTRA-ARGS",
            "pub type One<A> = A\npub struct Use { value: One<i32, bool> }",
            NominalTypeDiagnosticCode::WrongArity,
        ),
        (
            "ALS-MISSING-ARGS",
            "pub type Pair<A, B> = (A, B)\npub struct Use { value: Pair<i32> }",
            NominalTypeDiagnosticCode::WrongArity,
        ),
        (
            "ALS-SELF-CYCLE",
            "pub type A = A\npub struct Use { value: A }",
            NominalTypeDiagnosticCode::CyclicAlias,
        ),
        (
            "ALS-TWO-CYCLE",
            "pub type A = B\npub type B = A\npub struct Use { value: A }",
            NominalTypeDiagnosticCode::CyclicAlias,
        ),
    ] {
        let report = field_report(id, source);
        assert!(
            matches!(report.outcome(), ResolvedTypeRefOutcome::Poisoned(_)),
            "{id}: rejected application is authoritative poison"
        );
        assert_codes(id, &report, &[code]);
        assert!(
            matches!(report.outcome().product().recovered(), TypeKind::Error(_)),
            "{id}: failing application recovers as typed error"
        );
    }
}

#[test]
fn alias_application_reports_child_failures_without_skipping_arity() {
    let argument_errors = field_report(
        "ALS-ARG-ERROR",
        "pub type Pair<A, B> = (A, B)\npub struct Use { value: Pair<Missing, AlsoMissing, bool> }",
    );
    assert!(
        matches!(
            argument_errors.outcome(),
            ResolvedTypeRefOutcome::Poisoned(_)
        ),
        "ALS-ARG-ERROR: invalid children keep the alias application poisoned"
    );
    assert_codes(
        "ALS-ARG-ERROR",
        &argument_errors,
        &[
            NominalTypeDiagnosticCode::WrongArity,
            NominalTypeDiagnosticCode::UnknownType,
            NominalTypeDiagnosticCode::UnknownType,
        ],
    );

    let enum_arity = field_report(
        "ALS-ENUM-ARITY",
        "pub enum Choice<T> { Value(T) }\npub struct Use { value: Choice<i32, bool> }",
    );
    assert!(
        matches!(enum_arity.outcome(), ResolvedTypeRefOutcome::Poisoned(_)),
        "ALS-ENUM-ARITY: wrong project enum arity is authoritative poison"
    );
    assert_codes(
        "ALS-ENUM-ARITY",
        &enum_arity,
        &[NominalTypeDiagnosticCode::WrongArity],
    );
}

#[test]
fn bare_generic_project_struct_reports_wrong_arity() {
    let report = named_struct_field_report(
        "ALS-STRUCT-ARITY",
        "pub struct Use { value: Boxed }\npub struct Boxed<T> { value: T }",
        "Use",
    );
    assert!(
        matches!(report.outcome(), ResolvedTypeRefOutcome::Poisoned(_)),
        "ALS-STRUCT-ARITY: a bare generic project struct is a zero-argument application"
    );
    assert_codes(
        "ALS-STRUCT-ARITY",
        &report,
        &[NominalTypeDiagnosticCode::WrongArity],
    );
    let [node] = report.outcome().product().nodes() else {
        panic!("ALS-STRUCT-ARITY: the bare application owns one root resolution fact")
    };
    assert!(
        matches!(
            node.outcome(),
            TypeNameResolution::Failed(TypeResolutionFailure::WrongArity {
                target: TypeArityTarget::Project(target),
                expected: TypeArityExpectation::Exact(1),
                actual: 0,
            }) if target.name().as_str() == "Boxed"
        ),
        "ALS-STRUCT-ARITY: failure retains the typed project constructor and exact arity"
    );
}

#[test]
fn unk_project_type_positions_are_authoritative_node_poison() {
    for (id, source, expected_nodes) in [
        ("UNK-FIELD", "pub struct Use { value: Missing }", 1),
        (
            "UNK-GENERIC-ARG",
            "pub struct Use { value: Vec<Missing> }",
            2,
        ),
        (
            "UNK-GENERIC-HEAD",
            "pub struct Use { value: Missing<i32> }",
            2,
        ),
        (
            "UNK-NESTED-GENERIC",
            "pub struct Use { value: Option<Vec<Missing>> }",
            3,
        ),
    ] {
        let report = field_report(id, source);
        assert!(
            matches!(report.outcome(), ResolvedTypeRefOutcome::Poisoned(_)),
            "{id}: unknown project type is authoritative poison"
        );
        assert_codes(id, &report, &[NominalTypeDiagnosticCode::UnknownType]);
        assert_eq!(
            report.outcome().product().nodes().len(),
            expected_nodes,
            "{id}: outer structural resolution facts remain available"
        );
        assert!(
            report
                .outcome()
                .product()
                .nodes()
                .iter()
                .any(|node| matches!(node.outcome(), TypeNameResolution::Failed(_))),
            "{id}: exact missing-name node retains a typed failure fact"
        );
    }
}

#[test]
fn builtin_paths_remain_unqualified_and_array_lengths_are_typed() {
    let array = field_report("ARRAY-CONST", "pub struct Use { value: Array<u8, 32> }");
    assert!(
        matches!(array.outcome(), ResolvedTypeRefOutcome::Complete(_)),
        "ARRAY-CONST: a numeric array length is a complete built-in application"
    );
    assert_codes("ARRAY-CONST", &array, &[]);
    assert!(matches!(
        array.outcome().product().recovered(),
        TypeKind::Array { item, len: ArrayLength::Const(32) } if item.as_ref() == &TypeKind::U8
    ));

    let builtin = field_report(
        "BUILTIN-RESERVED",
        "pub struct Use { value: Result<i32, ArcError> }",
    );
    assert!(
        matches!(builtin.outcome(), ResolvedTypeRefOutcome::Complete(_)),
        "BUILTIN-RESERVED: an unqualified Result selects the built-in constructor"
    );
    assert_codes("BUILTIN-RESERVED", &builtin, &[]);
    assert!(matches!(
        builtin.outcome().product().recovered(),
        TypeKind::Result { ok, error }
            if ok.as_ref() == &TypeKind::I32
                && error.as_ref() == &TypeKind::Named("ArcError".to_owned())
    ));

    let qualified = field_report(
        "BUILTIN-QUALIFIED",
        "pub struct Use { value: crate.Result<i32, ArcError> }",
    );
    assert!(
        matches!(qualified.outcome(), ResolvedTypeRefOutcome::Poisoned(_)),
        "BUILTIN-QUALIFIED: qualification prevents built-in selection"
    );
    assert_codes(
        "BUILTIN-QUALIFIED",
        &qualified,
        &[NominalTypeDiagnosticCode::UnknownType],
    );
    assert!(
        qualified
            .outcome()
            .product()
            .nodes()
            .iter()
            .any(|node| matches!(node.outcome(), TypeNameResolution::Failed(_)))
    );
}
