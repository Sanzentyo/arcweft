use super::*;

#[test]
fn action_payload_publishes_one_nominal_record_from_its_original_declaration() {
    let (document, project) =
        project("pub action @action:.feedback.submit submit(value: String, count: i64)\n");
    let table = ProjectSymbolTable::link(
        project.view(),
        &declarations(&document, Vec::new(), "action-payload-record"),
    )
    .expect("Action declaration links")
    .into_table();
    let action = table
        .retained_symbols()
        .find(|symbol| symbol.public_id().as_str() == "action.feedback.submit")
        .expect("original Action declaration");
    let payload = table
        .nominal_symbols()
        .find(|symbol| symbol.owner() == action.owner())
        .expect("Action payload retains its defining item instead of a detached schema");
    assert_eq!(payload.id().module(), action.module());
    assert_eq!(action.action_payload(), Some(payload.id()));
    let ProjectTypeTarget::Nominal(resolved) = table
        .resolve_hir_type_target(
            action.module(),
            &type_path("submit"),
            payload.source().name().clone(),
        )
        .expect("the retained binding also resolves in a type context")
    else {
        panic!("Action payload type");
    };
    assert_eq!(resolved.id(), payload.id());
    assert_eq!(table.visible_type_bindings(action.module()).count(), 1);
    let ProjectNominalBody::Struct { fields } = payload.body() else {
        panic!("Action payload has a declaration-ordered record shape");
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| field.name().as_str())
            .collect::<Vec<_>>(),
        ["value", "count"],
    );
    assert!(
        fields
            .iter()
            .all(|field| field.ty().module() == action.owner().module())
    );
    assert_eq!(
        &document.text()[fields[0].source().whole().range().as_range()],
        "value: String"
    );
    assert_eq!(
        &document.text()[fields[1].source().name().range().as_range()],
        "count"
    );
}

#[test]
fn action_payload_empty_record_has_one_retained_binding_and_no_fake_callable() {
    let (document, project) = project("pub action clicked()\n");
    let table = ProjectSymbolTable::link(
        project.view(),
        &declarations(&document, Vec::new(), "action-empty"),
    )
    .unwrap()
    .into_table();
    let action = table.retained_symbols().next().unwrap();
    let payload = table.nominal(action.action_payload().unwrap()).unwrap();
    assert!(matches!(payload.body(), ProjectNominalBody::Struct { fields } if fields.is_empty()));
    assert_eq!(table.callable_symbols().count(), 0);
    assert_eq!(table.nominal_symbols().count(), 1);
}

#[test]
fn action_payload_imports_preserve_the_original_channel_and_type_owner() {
    let (documents, project) = project_modules(&[
        ("", "mod feedback\nuse crate.feedback.submit as send\n"),
        ("feedback", "pub action submit(value: String)\n"),
    ]);
    let table = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "action-import"),
    )
    .unwrap()
    .into_table();
    let action = table.retained_symbols().next().unwrap();
    assert_eq!(action.public_id().as_str(), "action.feedback.submit");
    for spelling in ["send", "crate.feedback.submit"] {
        let target = table
            .resolve_hir_type_target(
                &CanonicalModulePath::crate_root(),
                &type_path(spelling),
                action.declaration_span().clone(),
            )
            .unwrap();
        let ProjectTypeTarget::Nominal(payload) = target else {
            panic!("payload type");
        };
        assert_eq!(Some(payload.id()), action.action_payload());
    }
    assert_eq!(table.nominal_symbols().count(), 1);
}

#[test]
fn action_payload_rejects_patterns_without_one_immutable_named_field() {
    for source in [
        "pub action submit((value, other): (i64, i64))\n",
        "pub action submit(mut value: i64)\n",
    ] {
        let name = SourceName::path("invalid-action.arcw");
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://invalid-action").unwrap(),
                name.clone(),
                source,
            )
            .unwrap(),
        );
        let mut syntax = SyntaxDatabase::try_new().unwrap();
        let parsed = syntax
            .parse_initial(
                SourceSnapshotId::initial(name),
                document,
                ParseOptions::default(),
            )
            .unwrap();
        assert!(
            parsed
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == "syntax.action.invalid_parameter"),
            "invalid Action field patterns must be rejected at their original syntax boundary"
        );
    }
}

#[test]
fn nominal_publication_matrix_rejects_family_specific_duplicates() {
    let cases = [
        ("PUB-DUP-ALIAS", "type Thing = i32\ntype Thing = i32\n"),
        ("PUB-DUP-ENUM", "enum Thing { One }\nenum Thing { Two }\n"),
        ("PUB-DUP-STRUCT", "struct Thing {}\nstruct Thing {}\n"),
        (
            "PUB-CROSS-ENUM-ALIAS",
            "enum Thing { One }\ntype Thing = i32\n",
        ),
        (
            "PUB-CROSS-STRUCT-ALIAS",
            "struct Thing {}\ntype Thing = i32\n",
        ),
        (
            "PUB-CROSS-STRUCT-ENUM",
            "struct Thing {}\nenum Thing { One }\n",
        ),
    ];

    for (test_id, source) in cases {
        let (document, project) = project(source);
        let report = ProjectSymbolTable::link(
            project.view(),
            &declarations(&document, Vec::new(), test_id),
        )
        .expect_err(&format!(
            "{test_id}: conflicting nominal declarations must not publish"
        ));
        assert!(
            report.diagnostics().iter().any(|diagnostic| matches!(
                diagnostic,
                ProjectSymbolLinkError::DuplicateDeclaration { name, .. } if name == "Thing"
            )),
            "{test_id}: duplicate declaration diagnostic must retain the conflicting name: {report:?}",
        );
    }
}

#[test]
fn nominal_publication_matrix_preserves_nominal_visibility_boundaries() {
    let (documents, project) = project_modules(&[
        ("", ""),
        ("crate_visible", "pub(crate) enum CrateVisible { One }\n"),
        ("crate_consumer", "use crate.crate_visible.CrateVisible\n"),
        (
            "owner",
            "struct PrivateOwner {}\npub(super) struct SuperVisible {}\n",
        ),
        ("owner.child", "use super.SuperVisible\n"),
        ("public_origin", "pub type PublicThroughFacade = i32\n"),
        (
            "facade",
            "pub use crate.public_origin.PublicThroughFacade\n",
        ),
        ("consumer", "use crate.facade.PublicThroughFacade\n"),
        (
            "glob_origin",
            "pub struct PublicGlob {}\nstruct PrivateGlob {}\n",
        ),
        ("glob_consumer", "use crate.glob_origin.*\n"),
    ]);
    let table = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "PUB-nominal-visibility"),
    )
    .unwrap_or_else(|error| panic!("PUB-VIS-PUBLIC-POSITIVE: fixture must link: {error:?}"))
    .into_table();
    let source = documents[0]
        .span(SourceRange::new(0, 0))
        .unwrap_or_else(|_| panic!("PUB-VIS-PRIVATE-OWNER: reference span must exist"));
    let resolve = |test_id: &str, module: &str, name: &str| {
        table
            .resolve_hir_type_target(&module_path(module), &type_path(name), source.clone())
            .unwrap_or_else(|error| panic!("{test_id}: `{name}` must resolve: {error:?}"))
    };

    assert!(
        matches!(
            resolve("PUB-VIS-CRATE-POSITIVE", "crate_consumer", "CrateVisible"),
            ProjectTypeTarget::Nominal(_)
        ),
        "PUB-VIS-CRATE-POSITIVE: crate-visible nominal resolves from another project module",
    );
    assert!(
        matches!(
            resolve("PUB-VIS-PRIVATE-OWNER", "owner", "PrivateOwner"),
            ProjectTypeTarget::Nominal(_)
        ),
        "PUB-VIS-PRIVATE-OWNER: private nominal resolves in its owner module",
    );
    assert!(
        matches!(
            resolve("PUB-VIS-SUPER-CHILD", "owner.child", "SuperVisible"),
            ProjectTypeTarget::Nominal(_)
        ),
        "PUB-VIS-SUPER-CHILD: super-visible nominal resolves in its child module",
    );
    let ProjectTypeTarget::Nominal(original) = resolve(
        "PUB-VIS-PUBLIC-POSITIVE",
        "public_origin",
        "PublicThroughFacade",
    ) else {
        panic!("PUB-VIS-PUBLIC-POSITIVE: origin must be nominal");
    };
    let ProjectTypeTarget::Nominal(reexported) =
        resolve("PUB-VIS-PUBLIC-POSITIVE", "consumer", "PublicThroughFacade")
    else {
        panic!("PUB-VIS-PUBLIC-POSITIVE: public re-export must be nominal");
    };
    assert_eq!(
        original.id(),
        reexported.id(),
        "PUB-VIS-PUBLIC-POSITIVE: public re-export retains the origin declaration identity",
    );
    assert!(
        matches!(
            resolve("PUB-GLOB-SKIPS-PRIVATE", "glob_consumer", "PublicGlob"),
            ProjectTypeTarget::Nominal(_)
        ),
        "PUB-GLOB-SKIPS-PRIVATE: glob retains public nominal",
    );
    assert!(
        matches!(
            table.resolve_hir_type_target(
                &module_path("glob_consumer"),
                &type_path("PrivateGlob"),
                source,
            ),
            Err(ProjectTypeLookupError::Unknown { .. })
        ),
        "PUB-GLOB-SKIPS-PRIVATE: glob does not leak a private nominal",
    );
}

#[test]
fn nominal_publication_matrix_rejects_missing_names_and_invalid_publication() {
    let cases = [
        (
            "PUB-UNKNOWN-NAME",
            vec![
                ("", "use crate.origin.Missing\n"),
                ("origin", "pub struct Present {}\n"),
            ],
            ProjectSymbolDiagnosticCode::UnknownImport,
        ),
        (
            "PUB-LIFETIME-PARAM",
            vec![("", "struct Ref<'a> { value: &'a Value }\n")],
            ProjectSymbolDiagnosticCode::InvalidNominalDeclaration,
        ),
        (
            "PUB-VIS-ESC-PUB",
            vec![
                ("", ""),
                ("origin", "pub(crate) struct CrateOnly {}\n"),
                ("facade", "pub use crate.origin.CrateOnly\n"),
            ],
            ProjectSymbolDiagnosticCode::VisibilityEscalation,
        ),
        (
            "PUB-VIS-ESC-CRATE",
            vec![
                ("", ""),
                ("owner", "pub use crate.owner.child.ParentOnly\n"),
                ("owner.child", "pub(super) struct ParentOnly {}\n"),
            ],
            ProjectSymbolDiagnosticCode::VisibilityEscalation,
        ),
    ];

    for (test_id, sources, code) in cases {
        let (documents, project) = project_modules(&sources);
        let report =
            ProjectSymbolTable::link(project.view(), &empty_declarations(&documents, test_id))
                .expect_err(&format!(
                    "{test_id}: invalid publication must not produce a symbol table"
                ));
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == code),
            "{test_id}: expected {} diagnostic, got {report:?}",
            code.as_str(),
        );
    }
}

#[test]
fn nominal_publication_diagnostics_are_independent_of_module_construction_order() {
    const TEST_ID: &str = "PUB-INSERTION-ORDER";
    let sources = [
        ("", "use crate.left.Missing\nuse crate.right.Absent\n"),
        ("left", ""),
        ("right", ""),
    ];
    let (documents, project) = project_modules(&sources);
    let original =
        ProjectSymbolTable::link(project.view(), &empty_declarations(&documents, TEST_ID))
            .expect_err("both missing imports reject publication");

    let reversed_sources = [sources[2], sources[1], sources[0]];
    let (documents, project) = project_modules(&reversed_sources);
    let reordered =
        ProjectSymbolTable::link(project.view(), &empty_declarations(&documents, TEST_ID))
            .expect_err("module construction order cannot change typed publication errors");

    assert_eq!(
        original, reordered,
        "{TEST_ID}: typed diagnostics, their source spans, and ordering are canonical",
    );
}

#[test]
fn nominal_publication_matrix_rejects_external_and_module_name_collisions() {
    let (document, project) = project("enum Thing { One }\n");
    let report = ProjectSymbolTable::link(
        project.view(),
        &declarations(
            &document,
            vec![external_seed(
                &document,
                "external.thing",
                [(binding_path(["Thing"]), false)],
            )],
            "PUB-CROSS-EXTERNAL",
        ),
    )
    .expect_err("PUB-CROSS-EXTERNAL: external and enum collision must not publish");
    assert!(
        report.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic,
            ProjectSymbolLinkError::DuplicateDeclaration { name, .. } if name == "Thing"
        )),
        "PUB-CROSS-EXTERNAL: collision reports the direct symbol name: {report:?}",
    );

    let (documents, project) = project_modules(&[("", "type Thing = i32\n"), ("Thing", "")]);
    let report = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "PUB-CROSS-MODULE"),
    )
    .expect_err("PUB-CROSS-MODULE: child module and nominal collision must not publish");
    assert!(
        report.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic,
            ProjectSymbolLinkError::DuplicateDeclaration { name, .. } if name == "Thing"
        )),
        "PUB-CROSS-MODULE: collision reports the direct symbol name: {report:?}",
    );
}

#[test]
fn nominal_publication_rejects_callable_collisions_atomically() {
    const TEST_ID: &str = "PUB-CROSS-CALLABLE";
    let (document, project) = project(concat!("fn Thing() -> Unit { () }\n", "struct Thing {}\n",));

    let report = ProjectSymbolTable::link(
        project.view(),
        &declarations(&document, Vec::new(), TEST_ID),
    )
    .expect_err("a callable and nominal cannot share the direct project symbol namespace");

    assert!(
        report.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic,
            ProjectSymbolLinkError::DuplicateDeclaration {
                name,
                sites,
                ..
            } if name == "Thing" && sites.len() == 2 && sites[0] != sites[1]
        )),
        "{TEST_ID}: collision is a source-related typed diagnostic: {report:?}",
    );
    assert_eq!(
        report.diagnostics().len(),
        1,
        "PUB-ATOMIC: link failure returns only diagnostics, never a partial table",
    );
}

#[test]
fn nominal_publication_rejects_named_import_of_an_ambiguous_glob_binding() {
    const TEST_ID: &str = "PUB-NAMED-AMBIG";
    let (documents, project) = project_modules(&[
        ("", "use crate.middle.Thing\n"),
        ("left", "pub struct Thing {}\n"),
        ("right", "pub enum Thing { Value }\n"),
        ("middle", "pub use crate.left.*\npub use crate.right.*\n"),
    ]);

    let report = ProjectSymbolTable::link(project.view(), &empty_declarations(&documents, TEST_ID))
        .expect_err("a named import cannot select an ambiguous glob binding");

    assert!(
        report.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic,
            ProjectSymbolLinkError::AmbiguousImport { candidates, .. }
                if candidates.len() == 2 && candidates.windows(2).all(|pair| pair[0] < pair[1])
        )),
        "{TEST_ID}: named import retains deterministically ordered candidate sources: {report:?}",
    );
}
