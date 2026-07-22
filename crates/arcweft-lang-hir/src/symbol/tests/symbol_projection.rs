use super::*;

#[test]
fn ordinary_projection_matches_callable_golden() {
    let (document, project) =
        project("pub fn alpha() -> Unit { () }\n#[fx]\nfn beta(value: i32) -> i32 { value }\n");
    let table = ProjectSymbolTable::link(
        &project,
        &declarations(&document, Vec::new(), "ordinary-golden"),
    )
    .expect("ordinary link")
    .into_table();
    let actual = table
        .callable_symbols()
        .map(|symbol| {
            (
                symbol.declaration().module().to_string(),
                symbol.declaration().name().to_owned(),
                symbol.visibility(),
                symbol.is_fx(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        vec![
            (
                "crate".to_owned(),
                "alpha".to_owned(),
                Some(Visibility::Public),
                false,
            ),
            ("crate".to_owned(), "beta".to_owned(), None, true),
        ]
    );
}

#[test]
fn nominal_records_publish_once_and_resolve_through_every_import_form() {
    let root_source = concat!(
        "use crate.models.Record\n",
        "use crate.models.Choice as Pick\n",
        "use crate.facade.*\n",
    );
    let model_source = concat!(
        "pub struct Record<T: Bound> where T: Bound {\n",
        "    value: Result<T, Missing>,\n",
        "}\n",
        "pub enum Choice<T> where T: Bound {\n",
        "    Value Result<T, Missing>,\n",
        "    Empty,\n",
        "}\n",
        "pub type Alias<T> = Result<T, Missing>\n",
        "where T: Bound\n",
    );
    let (documents, project) = project_modules(&[
        ("", root_source),
        ("models", model_source),
        ("facade", "pub use crate.models.Alias\n"),
    ]);
    let table = ProjectSymbolTable::link(
        &project,
        &empty_declarations(&documents, "nominal-import-publication"),
    )
    .expect("nominals and imports publish atomically")
    .into_table();
    let models = module_path("models");
    let root = CanonicalModulePath::crate_root();
    let reference_source = documents[0]
        .span(SourceRange::new(0, root_source.len()))
        .expect("reference source");
    let resolve_nominal = |module: &CanonicalModulePath, spelling: &str| {
        let ProjectTypeTarget::Nominal(declaration) = table
            .resolve_type_target(module, &type_path(spelling), reference_source.clone())
            .expect("nominal type target")
        else {
            panic!("`{spelling}` must resolve to a project nominal")
        };
        declaration
    };

    let local_record = resolve_nominal(&models, "Record");
    let qualified_record = resolve_nominal(&root, "crate.models.Record");
    let imported_record = resolve_nominal(&root, "Record");
    assert_eq!(local_record.id(), qualified_record.id());
    assert_eq!(local_record.id(), imported_record.id());
    assert_eq!(
        local_record.id().kind(),
        ProjectNominalDeclarationKind::Struct
    );
    assert_eq!(local_record.id().module(), &models);
    assert!(local_record.id().owner_path().is_empty());
    assert_eq!(local_record.id().name().as_str(), "Record");

    let choice = resolve_nominal(&root, "Pick");
    assert_eq!(choice.id().kind(), ProjectNominalDeclarationKind::Enum);
    assert_eq!(
        choice.id(),
        resolve_nominal(&root, "crate.models.Choice").id()
    );

    let alias = resolve_nominal(&root, "Alias");
    assert_eq!(alias.id().kind(), ProjectNominalDeclarationKind::TypeAlias);
    assert_eq!(
        alias.id(),
        resolve_nominal(&root, "crate.facade.Alias").id()
    );
    assert_eq!(alias.id(), resolve_nominal(&models, "Alias").id());

    assert_eq!(table.nominal_symbols().count(), 3);
    assert_eq!(table.nominal(local_record.id()), Some(local_record));
    assert_nominal_source_records(model_source, &documents[1], local_record, choice, alias);
    assert_visible_nominal_bindings(&table, &root, local_record, choice, alias);
    let visible = table.visible_type_bindings(&root).collect::<Vec<_>>();
    let record_binding = visible
        .iter()
        .find(|binding| binding.spelling().to_string() == "Record")
        .expect("unaliased record import remains visible");
    assert!(record_binding.reference_sites().iter().any(|site| {
        site.source() == documents[0].identity()
            && &root_source[site.range().start()..site.range().end()] == "Record"
    }));
    let choice_binding = visible
        .iter()
        .find(|binding| binding.spelling().to_string() == "Pick")
        .expect("aliased choice import remains visible");
    assert!(choice_binding.reference_sites().iter().any(|site| {
        site.source() == documents[0].identity()
            && &root_source[site.range().start()..site.range().end()] == "Choice"
    }));
}

#[test]
fn reserved_type_names_and_cross_family_duplicates_block_publication() {
    let (document, reserved_project) = project("struct Result {\n    value: i32,\n}\n");
    let report = ProjectSymbolTable::link(
        &reserved_project,
        &declarations(&document, Vec::new(), "reserved-type-name"),
    )
    .expect_err("reserved built-in type names cannot be shadowed");
    assert!(matches!(
        report.diagnostics(),
        [ProjectSymbolLinkError::ReservedTypeName { module, name, source }]
            if module == &CanonicalModulePath::crate_root()
                && name == "Result"
                && &document.text()[source.range().start()..source.range().end()] == "Result"
    ));

    let (document, project) = project(concat!(
        "fn Widget() -> Unit { () }\n",
        "struct Widget {\n    value: i32,\n}\n",
    ));
    let report = ProjectSymbolTable::link(
        &project,
        &declarations(&document, Vec::new(), "cross-family-duplicate"),
    )
    .expect_err("callable and nominal cannot publish the same direct name");
    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic,
        ProjectSymbolLinkError::DuplicateDeclaration { name, .. } if name == "Widget"
    )));
}

#[test]
fn type_lookup_reports_wrong_kind_inaccessible_and_ambiguous_candidates() {
    let (documents, project) = project_modules(&[
        (
            "",
            concat!(
                "use crate.a.ProjectRecord as Both\n",
                "use crate.b.ProjectRecord as Both\n",
                "fn work() -> Unit { () }\n",
            ),
        ),
        (
            "a",
            "pub struct ProjectRecord {\n    left: i32,\n}\nstruct Hidden {\n    value: i32,\n}\n",
        ),
        ("b", "pub enum ProjectRecord {\n    Right,\n}\n"),
    ]);
    let table = ProjectSymbolTable::link(
        &project,
        &empty_declarations(&documents, "typed-type-lookup-errors"),
    )
    .expect("ordinary same-spelling ambiguity remains a lookup result")
    .into_table();
    let root = CanonicalModulePath::crate_root();
    let source = documents[0]
        .span(SourceRange::new(0, 3))
        .expect("reference source");

    assert!(matches!(
        table.resolve_type_target(&root, &type_path("work"), source.clone()),
        Err(ProjectTypeLookupError::WrongKind { actual, .. })
            if matches!(actual.target(), ProjectSymbolTargetId::Callable(_))
                && actual.declaration().is_some()
                && !actual.binding_sites().is_empty()
    ));
    assert!(matches!(
        table.resolve_type_target(
            &root,
            &type_path("crate.a.Hidden"),
            source.clone(),
        ),
        Err(ProjectTypeLookupError::Inaccessible { candidates, .. })
            if candidates.len() == 1
                && matches!(candidates[0].target(), ProjectSymbolTargetId::Nominal(_))
                && candidates[0].declaration().is_some()
    ));
    assert!(matches!(
        table.resolve_type_target(&root, &type_path("Both"), source),
        Err(ProjectTypeLookupError::Ambiguous { candidates, .. })
            if candidates.len() == 2
                && candidates.windows(2).all(|pair| pair[0].target() < pair[1].target())
                && candidates.iter().all(|candidate| {
                    candidate.declaration().is_some() && !candidate.binding_sites().is_empty()
                })
    ));
}

#[test]
fn table_retains_source_identity_for_every_module() {
    let (documents, project) = project_modules(&[("", ""), ("empty", "")]);
    let table = ProjectSymbolTable::link(
        &project,
        &empty_declarations(&documents, "module-source-identities"),
    )
    .expect("empty modules link")
    .into_table();
    let root = CanonicalModulePath::crate_root();
    let child = module_path("empty");

    assert_eq!(table.source_identity(&root), Some(documents[0].identity()));
    assert_eq!(table.source_identity(&child), Some(documents[1].identity()));
}

#[test]
fn ordinary_projection_unchanged_by_character_externals() {
    let (document, project) =
        project("pub fn alpha() -> Unit { () }\nfn beta(value: i32) -> i32 { value }\n");
    let empty = declarations(&document, Vec::new(), "ordinary-empty");
    let ordinary = ProjectSymbolTable::link(&project, &empty).expect("ordinary table");
    let owner = "character.akane";
    let with_character = declarations(
        &document,
        vec![external_seed(
            &document,
            owner,
            [
                (binding_path(["character", "akane"]), false),
                (binding_path(["akane"]), false),
            ],
        )],
        "ordinary-character",
    );
    let extended = ProjectSymbolTable::link(&project, &with_character).expect("extended table");

    assert_eq!(
        ordinary
            .table()
            .callable_symbols()
            .cloned()
            .collect::<Vec<_>>(),
        extended
            .table()
            .callable_symbols()
            .cloned()
            .collect::<Vec<_>>()
    );
}

#[test]
fn external_seed_assignment_is_sorted_and_opaque() {
    let (document, project) = project("fn main() -> Unit { () }\n");
    let declarations = declarations(
        &document,
        vec![
            external_seed(&document, "zeta", [(binding_path(["zeta"]), false)]),
            external_seed(&document, "alpha", [(binding_path(["alpha"]), false)]),
        ],
        "sorted-seeds",
    );
    let link = ProjectSymbolTable::link(&project, &declarations).expect("linked externals");

    assert_eq!(
        declarations
            .declarations()
            .map(|(_, seed)| seed.canonical_path().canonical_string())
            .collect::<Vec<_>>(),
        vec!["alpha", "zeta"]
    );
    assert_eq!(link.seed_declarations().len(), 2);
    assert_eq!(
        link.table()
            .external_symbols()
            .map(|symbol| symbol.canonical_path().canonical_string())
            .collect::<Vec<_>>(),
        vec!["alpha", "zeta"]
    );
}

#[test]
fn callable_filter_rejects_external() {
    let (document, project) = project("fn main() -> Unit { () }\n");
    let declarations = declarations(
        &document,
        vec![external_seed(
            &document,
            "character.akane",
            [(binding_path(["character", "akane"]), false)],
        )],
        "not-callable",
    );
    let table = ProjectSymbolTable::link(&project, &declarations)
        .expect("linked table")
        .into_table();
    let reference =
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "character.akane")
            .expect("reference");
    let source = document.span(SourceRange::new(0, 2)).expect("source span");

    assert!(matches!(
        table.resolve_callable(&CanonicalModulePath::crate_root(), &reference, &source,),
        Err(ProjectSymbolResolutionError::NotCallable {
            actual: ProjectSymbolTargetId::External(_),
            ..
        })
    ));
}

#[test]
fn missing_import_is_a_typed_link_diagnostic() {
    let (document, project) = project("use crate.missing.symbol\nfn main() -> Unit { () }\n");
    let declarations = declarations(&document, Vec::new(), "missing-import");
    let report = ProjectSymbolTable::link(&project, &declarations)
        .expect_err("unknown imports are rejected during atomic publication");

    assert!(matches!(
        report.diagnostics(),
        [ProjectSymbolLinkError::UnknownImport { module, import, source }]
            if module == &CanonicalModulePath::crate_root()
                && import.to_string() == "crate.missing.symbol"
                && source.range() == SourceRange::new(0, 24)
    ));
    assert_eq!(
        report.diagnostics()[0].code().as_str(),
        "aw.project.symbol.unknown_import"
    );
}

#[test]
fn generated_character_spellings_do_not_consume_alias_limit() {
    let (document, project) = project("fn main() -> Unit { () }\n");
    let seeds = (0..512)
        .map(|index| {
            let canonical = format!("character.owner{index:03}");
            let compact = format!("owner{index:03}");
            external_seed(
                &document,
                &canonical,
                [
                    (binding_path(["character", &compact]), false),
                    (binding_path([&compact]), false),
                ],
            )
        })
        .collect();
    let declarations = declarations(&document, seeds, "generated-bindings");
    let link = ProjectSymbolTable::link(&project, &declarations)
        .expect("generated mandatory spellings are not authored aliases");
    assert_eq!(link.table().external_symbols().count(), 512);
}
