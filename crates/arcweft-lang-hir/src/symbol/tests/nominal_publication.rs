use super::*;

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
        let report =
            ProjectSymbolTable::link(&project, &declarations(&document, Vec::new(), test_id))
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
        &project,
        &empty_declarations(&documents, "PUB-nominal-visibility"),
    )
    .unwrap_or_else(|error| panic!("PUB-VIS-PUBLIC-POSITIVE: fixture must link: {error:?}"))
    .into_table();
    let source = documents[0]
        .span(SourceRange::new(0, 0))
        .unwrap_or_else(|_| panic!("PUB-VIS-PRIVATE-OWNER: reference span must exist"));
    let resolve = |test_id: &str, module: &str, name: &str| {
        table
            .resolve_type_target(&module_path(module), &type_path(name), source.clone())
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
            table.resolve_type_target(
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
        let report = ProjectSymbolTable::link(&project, &empty_declarations(&documents, test_id))
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
    let original = ProjectSymbolTable::link(&project, &empty_declarations(&documents, TEST_ID))
        .expect_err("both missing imports reject publication");

    let reversed_sources = [sources[2], sources[1], sources[0]];
    let (documents, project) = project_modules(&reversed_sources);
    let reordered = ProjectSymbolTable::link(&project, &empty_declarations(&documents, TEST_ID))
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
        &project,
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
        &project,
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

    let report = ProjectSymbolTable::link(&project, &declarations(&document, Vec::new(), TEST_ID))
        .expect_err("a callable and nominal cannot share the direct project symbol namespace");

    assert!(
        report.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic,
            ProjectSymbolLinkError::DuplicateDeclaration {
                name,
                first,
                duplicate,
                ..
            } if name == "Thing" && first != duplicate
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

    let report = ProjectSymbolTable::link(&project, &empty_declarations(&documents, TEST_ID))
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
