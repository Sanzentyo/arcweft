use super::*;

#[test]
fn nominal_identity_and_lookup_p0_matrix() {
    const TEST_ID: &str = "ID-ALIAS-IDENTITY-DISTINCT";
    let root_source = concat!(
        "use crate.models.Structure\n",
        "use crate.models.Enumeration as ImportedEnumeration\n",
        "use crate.facade.*\n",
        "fn callable() -> Unit { () }\n",
    );
    let model_source = concat!(
        "pub struct Structure { value: i32 }\n",
        "pub enum Enumeration { Value }\n",
        "pub type Alias = String\n",
    );
    let (documents, project) = project_modules(&[
        ("", root_source),
        ("models", model_source),
        (
            "models.child",
            "use super.Structure\nuse super.Enumeration\nuse super.Alias\n",
        ),
        ("facade", "pub use crate.models.*\n"),
    ]);
    let table = ProjectSymbolTable::link(
        &project,
        &empty_declarations(&documents, "p0-nominal-identity-lookup"),
    )
    .unwrap_or_else(|error| panic!("{TEST_ID}: fixture must link: {error:?}"))
    .into_table();
    let root = CanonicalModulePath::crate_root();
    let models = module_path("models");
    let child = module_path("models.child");
    let source = documents[0]
        .span(SourceRange::new(0, root_source.len()))
        .unwrap_or_else(|_| panic!("{TEST_ID}: root reference span must exist"));

    let matrix = local_nominal_lookup_rows(&models, &child)
        .into_iter()
        .chain(root_nominal_lookup_rows(&root))
        .collect::<Vec<_>>();
    let resolved = resolve_nominal_lookup_rows(&table, &source, matrix);
    assert_nominal_lookup_identities(&resolved);

    let first_ids = table
        .nominal_symbols()
        .map(|declaration| declaration.id().clone())
        .collect::<Vec<_>>();
    let (_, reordered_project) = project_modules(&[
        ("facade", "pub use crate.models.*\n"),
        (
            "models.child",
            "use super.Structure\nuse super.Enumeration\nuse super.Alias\n",
        ),
        ("models", model_source),
        ("", root_source),
    ]);
    let reordered = ProjectSymbolTable::link(
        &reordered_project,
        &empty_declarations(&documents, "p0-nominal-identity-lookup"),
    )
    .unwrap_or_else(|error| panic!("ID-HASH-ORDER: reordered fixture must link: {error:?}"))
    .into_table()
    .nominal_symbols()
    .map(|declaration| declaration.id().clone())
    .collect::<Vec<_>>();
    assert_eq!(
        first_ids, reordered,
        "ID-HASH-ORDER: declaration identity order is input-order independent"
    );
    assert!(
        first_ids.windows(2).all(|pair| pair[0] < pair[1]),
        "ID-HASH-ORDER: nominal identities retain a strict canonical order"
    );
    assert_eq!(
        nominal_identity_fingerprints(&first_ids),
        nominal_identity_fingerprints(&reordered),
        "ID-HASH-ORDER: equal canonical identities retain equal hashes"
    );

    assert_nominal_world_and_revision_variation(&documents);
}

#[test]
fn nominal_lookup_failure_p0_matrix() {
    assert_inaccessible_parent_import_rejected();

    let (documents, project) = project_modules(&[
        (
            "",
            "use crate.left.*\nuse crate.right.*\nfn callable() -> Unit { () }\n",
        ),
        ("left", "pub struct Common {}\nstruct Hidden {}\n"),
        ("right", "pub enum Common { Value }\n"),
    ]);
    let declarations = declarations(
        &documents[0],
        vec![external_seed(
            &documents[0],
            "character.akane",
            [(binding_path(["character", "akane"]), false)],
        )],
        "p0-nominal-lookup-failures-resolvable",
    );
    let table = ProjectSymbolTable::link(&project, &declarations)
        .unwrap_or_else(|error| panic!("RES-AMBIG-GLOB: fixture must link: {error:?}"))
        .into_table();
    let root = CanonicalModulePath::crate_root();
    let source = documents[0]
        .span(SourceRange::new(0, 3))
        .unwrap_or_else(|_| panic!("RES-AMBIG-GLOB: root reference span must exist"));
    let lookup = |test_id: &'static str, spelling: &str| {
        let authored = parse_type_ref(spelling)
            .unwrap_or_else(|error| panic!("{test_id}: `{spelling}` must parse: {error:?}"));
        let TypeRef::Path(path) = authored.value() else {
            panic!("{test_id}: `{spelling}` must remain a typed path");
        };
        table.resolve_type_target(&root, path, source.clone())
    };

    assert!(
        matches!(
        lookup("RES-AMBIG-GLOB", "Common"),
            Err(ProjectTypeLookupError::Ambiguous { candidates, .. }) if candidates.len() == 2
        ),
        "RES-AMBIG-GLOB: competing glob targets remain ambiguous"
    );
    assert!(
        matches!(
            lookup("RES-INACCESS-QUAL", "crate.left.Hidden"),
            Err(ProjectTypeLookupError::Inaccessible { candidates, .. }) if candidates.len() == 1
        ),
        "RES-INACCESS-QUAL: qualified private nominal remains inaccessible"
    );
    assert!(
        matches!(
            lookup("RES-WRONG-CALLABLE", "callable"),
            Err(ProjectTypeLookupError::WrongKind { actual, .. })
                if matches!(actual.target(), ProjectSymbolTargetId::Callable(_))
        ),
        "RES-WRONG-CALLABLE: callable cannot occupy type position"
    );

    let external_reference =
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "character.akane")
            .unwrap_or_else(|error| {
                panic!("RES-WRONG-EXTERNAL: external reference must construct: {error:?}")
            });
    assert!(
        matches!(
            table.resolve_callable(&root, &external_reference, &source),
            Err(ProjectSymbolResolutionError::NotCallable {
                actual: ProjectSymbolTargetId::External(_),
                ..
            })
        ),
        "RES-WRONG-EXTERNAL: external cannot occupy callable position"
    );
    assert!(
        matches!(
            lookup("RES-WRONG-MODULE", "crate.left"),
            Err(ProjectTypeLookupError::WrongKind { .. })
        ),
        "RES-WRONG-MODULE: a module cannot occupy type position"
    );
}

#[test]
fn duplicate_module_document_is_rejected_by_project_publication() {
    const TEST_ID: &str = "SRC-DUPLICATE-MODULE-DOCUMENT";
    let (document, project) = project("fn main() -> Unit { () }\n");
    let root = CanonicalModulePath::crate_root();
    let module = HirProjectModule::try_new(
        root.clone(),
        document.identity().clone(),
        project.linked_module(),
    )
    .unwrap_or_else(|error| panic!("{TEST_ID}: duplicate fixture module must bind: {error:?}"));
    assert_eq!(
        HirProject::new("duplicate-module-project", [module.clone(), module]),
        Err(HirProjectError::DuplicateModule { module: root }),
        "{TEST_ID}: duplicate canonical module is rejected even with the same document identity",
    );
}
