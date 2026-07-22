use std::sync::Arc;

use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{CallablePackageId, ProjectSymbolWorldId},
};
use arcweft_lang_sema::{
    checker::{TypeCheckReport, analyze_registered_project_types},
    env::TypeCheckEnv,
    registration::{CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts},
    types::TypeKind,
};
use arcweft_lang_syntax::{ast::module_path::CanonicalModulePath, parser::parse_source};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

fn registered_report(id: &str, source: &str) -> TypeCheckReport {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("memory:///{id}.arcw"))
                .unwrap_or_else(|error| panic!("{id}: source ID: {error}")),
            SourceName::path(format!("memory:///{id}.arcw")),
            source,
        )
        .unwrap_or_else(|error| panic!("{id}: source document: {error}")),
    );
    let parsed = parse_source(source);
    assert!(
        parsed.errors().is_empty(),
        "{id}: fixture parses: {:?}",
        parsed.errors()
    );
    let hir = lower_document_to_hir(&document, parsed.typed_tree())
        .unwrap_or_else(|error| panic!("{id}: fixture lowers: {error:?}"));
    let package = CallablePackageId::try_new(format!("unknown-position-{id}"))
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
    let registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::standard()),
        &project,
        &facts,
        None,
    ))
    .unwrap_or_else(|error| panic!("{id}: semantic registration: {error:?}"));
    analyze_registered_project_types(&project.linked_module(), &registered)
}

#[test]
fn unknown_nominal_types_are_poisoned_in_registered_project_positions() {
    for (id, source) in [
        ("UNK-FIELD", "struct Use { value: Missing }"),
        ("UNK-ENUM-PAYLOAD", "enum Use { Value(Missing) }"),
        ("UNK-ALIAS-TARGET", "type Use = Missing"),
        ("UNK-PARAM", "fn use_value(value: Missing) {}"),
        ("UNK-RETURN", "fn use_value() -> Missing { Unit }"),
        (
            "UNK-FLOW-PARAM",
            "flow @flow.use_value use_value(value: Missing) {}",
        ),
        (
            "UNK-FLOW-RETURN",
            "flow @flow.use_value use_value() -> Missing {}",
        ),
        ("UNK-TUPLE", "struct Use { value: (i32, Missing, bool) }"),
        (
            "UNK-FUNCTION-PARAM",
            "struct Use { value: (Missing, i32) -> i32 }",
        ),
        (
            "UNK-FUNCTION-RETURN",
            "struct Use { value: i32 -> Missing }",
        ),
        ("UNK-REFERENCE", "struct Use { value: &Missing }"),
        ("UNK-SLICE", "struct Use { value: [Missing] }"),
        (
            "UNK-PROJECTION-SUBJECT",
            "struct Use { value: Missing::Item }",
        ),
        ("UNK-GENERIC-ARG", "struct Use { value: Vec<Missing> }"),
        ("UNK-GENERIC-HEAD", "struct Use { value: Missing<i32> }"),
        (
            "UNK-NESTED-GENERIC",
            "struct Use { value: Option<Vec<Missing>> }",
        ),
        (
            "UNK-CLOSURE-RETURN",
            "fn use_value() { let value = |item| -> Missing { item } }",
        ),
        (
            "UNK-TRAIT-BOUND",
            "fn use_value<T: Iterator<Item = Missing>>() {}",
        ),
        (
            "UNK-ASSOC-BINDING",
            "fn use_value<T: Iterator<Item = Missing>>() {}",
        ),
        (
            "UNK-WHERE-BOUND",
            "fn use_value<T>() where T: Iterator<Item = Missing> {}",
        ),
        (
            "UNK-WHERE-SUBJECT",
            "fn use_value<T>() where Missing: Clone {}",
        ),
        (
            "UNK-TRAIT-ASSOC-DEFAULT",
            "trait Capability { type Item = Missing }",
        ),
        (
            "UNK-IMPL-TARGET",
            "trait Capability {}\nimpl Capability for Missing {}",
        ),
        (
            "UNK-IMPL-ASSOC",
            "trait Capability { type Item }\nstruct Owner {}\nimpl Capability for Owner { type Item = Missing }",
        ),
        (
            "UNK-METHOD-PARAM",
            "struct Owner {}\nimpl Owner { fn use_value(value: Missing) {} }",
        ),
        (
            "UNK-ENTRY-STATE",
            "enum GameEvent { Start }\nentry game @entry.game.main {\nstate = Missing\ninitializer = initialize\nevent = GameEvent\nreducer = reduce\ngoto @flow.main\n}",
        ),
        (
            "UNK-ENTRY-EVENT",
            "struct GameState {}\nentry game @entry.game.main {\nstate = GameState\ninitializer = initialize\nevent = Missing\nreducer = reduce\ngoto @flow.main\n}",
        ),
    ] {
        let report = registered_report(id, source);
        let unknowns = report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.stable_code() == "sema.nominal.unknown_type")
            .collect::<Vec<_>>();
        assert_eq!(
            unknowns.len(),
            1,
            "{id}: exactly one registered-project unknown nominal diagnostic: {unknowns:#?}"
        );
        assert!(
            report.nominal_resolutions.nodes().any(|(_, node)| {
                matches!(node.recovered(), Some(TypeKind::Error(_)))
                    && node.source().project().is_some()
            }),
            "{id}: the accepted-source node is typed nominal poison, never a Named fallback"
        );
    }
}
