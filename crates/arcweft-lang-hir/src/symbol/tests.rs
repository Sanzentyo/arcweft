use std::{
    collections::hash_map::DefaultHasher,
    fmt::Write as _,
    hash::{Hash, Hasher},
    sync::Arc,
};

use arcweft_lang_syntax::{
    ast::{
        common::Visibility,
        module_path::{CanonicalModulePath, ModulePathRoot, ModuleSegment},
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
    },
    parser::parse_source,
    types::{TypePath, TypeRef, TypeRefNodePath, parse_type_ref},
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange, SourceSpan};

use crate::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectError, HirProjectModule},
};

use super::{
    CallablePackageId, ExternalDeclarationSeed, ProjectDirectBinding, ProjectDirectBindingError,
    ProjectExternalDeclarations, ProjectSymbolDiagnosticCode, ProjectSymbolLinkError,
    ProjectSymbolResolutionError, ProjectSymbolRevision, ProjectSymbolTable, ProjectSymbolTargetId,
    ProjectSymbolWorldId, ProjectTypeLookupError, ProjectTypeTarget, ResolvedProjectSymbol,
    nominal::{
        ProjectNominalBody, ProjectNominalDeclaration, ProjectNominalDeclarationId,
        ProjectNominalDeclarationKind,
    },
};

const PACKAGE: &str = "project-symbol-tests";

fn project(source: &str) -> (Arc<SourceDocument>, HirProject) {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://project-symbol-tests/src/main.arcw")
                .expect("document id"),
            SourceName::path("src/main.arcw"),
            source,
        )
        .expect("source document"),
    );
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("lowered HIR");
    let project = HirProject::new(
        PACKAGE,
        [HirProjectModule::try_new(
            CanonicalModulePath::crate_root(),
            document.identity().clone(),
            hir,
        )
        .expect("root module binding")],
    )
    .expect("HIR project");
    (document, project)
}

fn module_path(path: &str) -> CanonicalModulePath {
    path.split('.').filter(|segment| !segment.is_empty()).fold(
        CanonicalModulePath::crate_root(),
        |module, segment| {
            module.join(ModuleSegment::new(segment).expect("valid test module segment"))
        },
    )
}

fn project_modules(sources: &[(&str, &str)]) -> (Vec<Arc<SourceDocument>>, HirProject) {
    let mut documents = Vec::with_capacity(sources.len());
    let modules = sources
        .iter()
        .map(|(path, source)| {
            let file = if path.is_empty() {
                "main".to_owned()
            } else {
                path.replace('.', "/")
            };
            let document = Arc::new(
                SourceDocument::try_new(
                    SourceDocumentId::try_new(format!(
                        "arcweft-project://project-symbol-tests/src/{file}.arcw"
                    ))
                    .expect("document id"),
                    SourceName::path(format!("src/{file}.arcw")),
                    *source,
                )
                .expect("source document"),
            );
            let parsed = parse_source(*source);
            assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
            let hir =
                lower_document_to_hir(&document, parsed.typed_tree()).expect("lowered module HIR");
            let module =
                HirProjectModule::try_new(module_path(path), document.identity().clone(), hir)
                    .expect("fixture module binding");
            documents.push(document);
            module
        })
        .collect::<Vec<_>>();
    let project = HirProject::new(PACKAGE, modules).expect("multi-module HIR project");
    (documents, project)
}

fn external_seed(
    document: &SourceDocument,
    canonical: &str,
    bindings: impl IntoIterator<Item = (ProjectSymbolPath, bool)>,
) -> ExternalDeclarationSeed {
    external_seed_in_module(
        document,
        canonical,
        &CanonicalModulePath::crate_root(),
        bindings,
    )
}

fn external_seed_in_module(
    document: &SourceDocument,
    canonical: &str,
    module: &CanonicalModulePath,
    bindings: impl IntoIterator<Item = (ProjectSymbolPath, bool)>,
) -> ExternalDeclarationSeed {
    let source = document
        .span(SourceRange::new(0, document.text().len().min(2)))
        .expect("declaration span");
    let bindings = bindings
        .into_iter()
        .map(|(path, authored_alias)| {
            ProjectDirectBinding::try_new(
                module.clone(),
                path,
                Some(Visibility::Public),
                source.clone(),
                authored_alias,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("direct bindings");
    ExternalDeclarationSeed::try_new(
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), canonical)
            .expect("canonical path"),
        Some(Visibility::Public),
        source,
        bindings,
    )
    .expect("external seed")
}

fn binding_path<const N: usize>(segments: [&str; N]) -> ProjectSymbolPath {
    ProjectSymbolPath::new(
        ModulePathRoot::ImplicitCrate,
        segments.map(|segment| {
            ProjectSymbolSegment::try_new(segment).expect("valid test project segment")
        }),
    )
    .expect("test project binding path is non-empty")
}

fn type_path(source: &str) -> TypePath {
    let authored = parse_type_ref(source).expect("valid test type path");
    let TypeRef::Path(path) = authored.value() else {
        panic!("`{source}` must parse as a plain type path");
    };
    path.clone()
}

fn assert_nominal_source_records(
    model_source: &str,
    model_document: &SourceDocument,
    record: &ProjectNominalDeclaration,
    choice: &ProjectNominalDeclaration,
    alias: &ProjectNominalDeclaration,
) {
    let alias_target = "Result<T, Missing>";
    let alias_target_start = model_source
        .rfind(alias_target)
        .expect("alias target source");
    let ProjectNominalBody::TypeAlias { target } = alias.body() else {
        panic!("Alias body must retain its parsed target")
    };
    assert_eq!(
        target
            .spans()
            .source_at(&TypeRefNodePath::root())
            .expect("alias root type source")
            .whole()
            .range(),
        SourceRange::new(alias_target_start, alias_target_start + alias_target.len())
    );

    let ProjectNominalBody::Enum { variants } = choice.body() else {
        panic!("Choice body must retain source-backed variants")
    };
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0].name().as_str(), "Value");
    assert!(variants[0].payload().is_some());
    assert_eq!(variants[1].name().as_str(), "Empty");
    assert!(variants[1].payload().is_none());

    let record_name = model_source.find("Record").expect("record name");
    assert_eq!(
        record.source().name().range(),
        SourceRange::new(record_name, record_name + "Record".len())
    );
    assert_eq!(record.source().name().source(), model_document.identity());
    assert_eq!(record.type_parameters().len(), 1);
    assert_eq!(record.where_predicates().len(), 1);
    let ProjectNominalBody::Struct { fields } = record.body() else {
        panic!("Record body must remain a source-backed struct")
    };
    assert_eq!(fields.len(), 1);
    let field_text = "value: Result<T, Missing>";
    let field_start = model_source.find(field_text).expect("field source");
    assert_eq!(
        fields[0].source().whole().range(),
        SourceRange::new(field_start, field_start + field_text.len())
    );
    assert_eq!(
        fields[0]
            .ty()
            .spans()
            .source_at(&TypeRefNodePath::root())
            .expect("field root type source")
            .whole()
            .range(),
        SourceRange::new(
            field_start + "value: ".len(),
            field_start + field_text.len(),
        )
    );
}

fn assert_visible_nominal_bindings(
    table: &ProjectSymbolTable,
    root: &CanonicalModulePath,
    record: &ProjectNominalDeclaration,
    choice: &ProjectNominalDeclaration,
    alias: &ProjectNominalDeclaration,
) {
    let visible = table
        .visible_type_bindings(root)
        .map(|binding| {
            (
                binding.spelling().to_string(),
                match binding.target() {
                    ProjectTypeTarget::Nominal(declaration) => declaration.id().clone(),
                    ProjectTypeTarget::External(_) => panic!("fixture has no external type"),
                },
                binding.binding_sites().len(),
            )
        })
        .collect::<Vec<_>>();
    assert!(
        visible
            .iter()
            .any(|(name, id, sites)| { name == "Record" && id == record.id() && *sites >= 2 })
    );
    assert!(
        visible
            .iter()
            .any(|(name, id, _)| { name == "Pick" && id == choice.id() })
    );
    assert!(
        visible
            .iter()
            .any(|(name, id, _)| { name == "Alias" && id == alias.id() })
    );
}

fn scope_rows(
    table: &ProjectSymbolTable,
    module: &CanonicalModulePath,
) -> Vec<(Vec<String>, ProjectSymbolTargetId)> {
    table
        .scope_bindings()
        .filter(|(candidate, _, _)| *candidate == module)
        .map(|(_, path, target)| {
            (
                path.segments()
                    .iter()
                    .map(|segment| segment.as_str().to_owned())
                    .collect(),
                target.clone(),
            )
        })
        .collect()
}

fn declarations(
    document: &SourceDocument,
    seeds: Vec<ExternalDeclarationSeed>,
    profile: &str,
) -> ProjectExternalDeclarations {
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(PACKAGE).expect("package"),
        document.identity().id().clone(),
        profile,
    )
    .expect("world");
    let revision =
        ProjectSymbolRevision::try_for_documents([document.identity()]).expect("project revision");
    ProjectExternalDeclarations::try_new(world, revision, seeds).expect("external declarations")
}

fn empty_declarations(
    documents: &[Arc<SourceDocument>],
    profile: &str,
) -> ProjectExternalDeclarations {
    let root = documents.first().expect("root document");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(PACKAGE).expect("package"),
        root.identity().id().clone(),
        profile,
    )
    .expect("world");
    let revision = ProjectSymbolRevision::try_for_documents(
        documents.iter().map(|document| document.identity()),
    )
    .expect("project revision");
    ProjectExternalDeclarations::try_new(world, revision, Vec::new())
        .expect("empty external declarations")
}

mod direct_bindings;
mod import_linking;
mod limits;
mod nominal_lookup;
mod nominal_publication;
mod symbol_projection;

fn aliased_target_imports(count: usize) -> String {
    (0..count).fold(String::new(), |mut source, index| {
        writeln!(source, "use crate.origin.target as alias{index}")
            .expect("writing to a String cannot fail");
        source
    })
}

fn assert_symbol_limit(
    report: &super::ProjectSymbolLinkReport,
    expected_kind: super::ProjectSymbolLimitKind,
    expected_observed: u64,
    expected_maximum: u64,
) {
    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic,
        ProjectSymbolLinkError::Limit {
            kind,
            observed,
            maximum,
            source: Some(_),
        } if *kind == expected_kind
            && *observed == expected_observed
            && *maximum == expected_maximum
    )));
}

fn nominal_alias_declarations(count: usize) -> String {
    (0..count).fold(String::new(), |mut source, index| {
        writeln!(source, "type Nominal{index} = i32").expect("writing to a String cannot fail");
        source
    })
}

fn nominal_struct_fields(count: usize) -> String {
    let mut source = String::from("struct FieldLimit {\n");
    for index in 0..count {
        writeln!(source, "    field{index}: i32,").expect("writing to a String cannot fail");
    }
    source.push_str("}\n");
    source
}

fn nominal_type_parameters(count: usize) -> String {
    let parameters = (0..count)
        .map(|index| format!("T{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("type ParameterLimit<{parameters}> = i32\n")
}

fn nominal_type_node_fields(one_over: bool) -> String {
    let maximum_per_reference = 4_096_usize;
    let tuple = format!(
        "({})",
        core::iter::repeat_n("i32", maximum_per_reference - 1)
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut source = String::from("struct TypeNodeLimit {\n");
    for index in 0..4 {
        writeln!(source, "    field{index}: {tuple},").expect("writing to a String cannot fail");
    }
    if one_over {
        source.push_str("    overflow: i32,\n");
    }
    source.push_str("}\n");
    source
}

fn grouped_missing_import(count: usize) -> String {
    let names = (0..count).map(|_| "target").collect::<Vec<_>>().join(", ");
    format!("use crate.origin.{{{names}}}\n")
}

type NominalLookupRow<'a> = (
    &'static str,
    &'a CanonicalModulePath,
    &'static str,
    ProjectNominalDeclarationKind,
);

fn local_nominal_lookup_rows<'a>(
    models: &'a CanonicalModulePath,
    child: &'a CanonicalModulePath,
) -> [NominalLookupRow<'a>; 9] {
    [
        (
            "ID-LOCAL-STRUCT",
            models,
            "Structure",
            ProjectNominalDeclarationKind::Struct,
        ),
        (
            "ID-LOCAL-ENUM",
            models,
            "Enumeration",
            ProjectNominalDeclarationKind::Enum,
        ),
        (
            "ID-LOCAL-ALIAS",
            models,
            "Alias",
            ProjectNominalDeclarationKind::TypeAlias,
        ),
        (
            "ID-CHILD-STRUCT",
            child,
            "Structure",
            ProjectNominalDeclarationKind::Struct,
        ),
        (
            "ID-CHILD-ENUM",
            child,
            "Enumeration",
            ProjectNominalDeclarationKind::Enum,
        ),
        (
            "ID-CHILD-ALIAS",
            child,
            "Alias",
            ProjectNominalDeclarationKind::TypeAlias,
        ),
        (
            "ID-PARENT-STRUCT",
            child,
            "super.Structure",
            ProjectNominalDeclarationKind::Struct,
        ),
        (
            "ID-PARENT-ENUM",
            child,
            "super.Enumeration",
            ProjectNominalDeclarationKind::Enum,
        ),
        (
            "ID-PARENT-ALIAS",
            child,
            "super.Alias",
            ProjectNominalDeclarationKind::TypeAlias,
        ),
    ]
}

fn root_nominal_lookup_rows(root: &CanonicalModulePath) -> [NominalLookupRow<'_>; 15] {
    [
        (
            "ID-QUAL-STRUCT",
            root,
            "crate.models.Structure",
            ProjectNominalDeclarationKind::Struct,
        ),
        (
            "ID-QUAL-ENUM",
            root,
            "crate.models.Enumeration",
            ProjectNominalDeclarationKind::Enum,
        ),
        (
            "ID-QUAL-ALIAS",
            root,
            "crate.models.Alias",
            ProjectNominalDeclarationKind::TypeAlias,
        ),
        (
            "ID-IMPORT-STRUCT",
            root,
            "Structure",
            ProjectNominalDeclarationKind::Struct,
        ),
        (
            "ID-IMPORT-ENUM",
            root,
            "Enumeration",
            ProjectNominalDeclarationKind::Enum,
        ),
        (
            "ID-IMPORT-ALIAS",
            root,
            "Alias",
            ProjectNominalDeclarationKind::TypeAlias,
        ),
        (
            "ID-AS-STRUCT",
            root,
            "Structure",
            ProjectNominalDeclarationKind::Struct,
        ),
        (
            "ID-AS-ENUM",
            root,
            "ImportedEnumeration",
            ProjectNominalDeclarationKind::Enum,
        ),
        (
            "ID-AS-ALIAS",
            root,
            "Alias",
            ProjectNominalDeclarationKind::TypeAlias,
        ),
        (
            "ID-GLOB-STRUCT",
            root,
            "Structure",
            ProjectNominalDeclarationKind::Struct,
        ),
        (
            "ID-GLOB-ENUM",
            root,
            "Enumeration",
            ProjectNominalDeclarationKind::Enum,
        ),
        (
            "ID-GLOB-ALIAS",
            root,
            "Alias",
            ProjectNominalDeclarationKind::TypeAlias,
        ),
        (
            "ID-REEXPORT-STRUCT",
            root,
            "crate.facade.Structure",
            ProjectNominalDeclarationKind::Struct,
        ),
        (
            "ID-REEXPORT-ENUM",
            root,
            "crate.facade.Enumeration",
            ProjectNominalDeclarationKind::Enum,
        ),
        (
            "ID-REEXPORT-ALIAS",
            root,
            "crate.facade.Alias",
            ProjectNominalDeclarationKind::TypeAlias,
        ),
    ]
}

fn resolve_nominal_lookup_rows<'a>(
    table: &ProjectSymbolTable,
    source: &SourceSpan,
    rows: impl IntoIterator<Item = NominalLookupRow<'a>>,
) -> Vec<(&'static str, ProjectNominalDeclarationId)> {
    rows.into_iter()
        .map(|(test_id, module, spelling, kind)| {
            let authored = parse_type_ref(spelling)
                .unwrap_or_else(|error| panic!("{test_id}: `{spelling}` must parse: {error:?}"));
            let TypeRef::Path(path) = authored.value() else {
                panic!("{test_id}: `{spelling}` must remain a typed path");
            };
            let target = table
                .resolve_type_target(module, path, source.clone())
                .unwrap_or_else(|error| panic!("{test_id}: `{spelling}` must resolve: {error:?}"));
            let ProjectTypeTarget::Nominal(declaration) = target else {
                panic!("{test_id}: `{spelling}` must resolve to a nominal declaration");
            };
            assert_eq!(declaration.id().kind(), kind, "{test_id}: declaration kind");
            (test_id, declaration.id().clone())
        })
        .collect()
}

fn assert_nominal_lookup_identities(resolved: &[(&str, ProjectNominalDeclarationId)]) {
    let id = |test_id: &str| {
        resolved
            .iter()
            .find_map(|(candidate, id)| (*candidate == test_id).then_some(id))
            .unwrap_or_else(|| panic!("{test_id}: matrix row must be present"))
    };
    assert_eq!(
        id("ID-LOCAL-STRUCT"),
        id("ID-QUAL-STRUCT"),
        "ID-ALIAS-IDENTITY-DISTINCT: qualified structure identity"
    );
    assert_eq!(
        id("ID-LOCAL-STRUCT"),
        id("ID-IMPORT-STRUCT"),
        "ID-ALIAS-IDENTITY-DISTINCT: imported structure identity"
    );
    assert_eq!(
        id("ID-LOCAL-ENUM"),
        id("ID-AS-ENUM"),
        "ID-ALIAS-IDENTITY-DISTINCT: alias identity"
    );
    assert_ne!(
        id("ID-LOCAL-STRUCT"),
        id("ID-LOCAL-ALIAS"),
        "ID-ALIAS-IDENTITY-DISTINCT: declarations remain distinct"
    );
    assert!(
        id("ID-LOCAL-STRUCT").owner_path().is_empty(),
        "ID-OWNER-PATH: top-level nominal has no owner path"
    );
}

fn nominal_identity_fingerprints(ids: &[ProjectNominalDeclarationId]) -> Vec<u64> {
    ids.iter()
        .map(|id| {
            let mut state = DefaultHasher::new();
            id.hash(&mut state);
            state.finish()
        })
        .collect()
}

fn assert_nominal_world_and_revision_variation(documents: &[Arc<SourceDocument>]) {
    let alternate_world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(PACKAGE)
            .unwrap_or_else(|error| panic!("ID-WORLD-DIFFERENT: package must be valid: {error:?}")),
        documents[0].identity().id().clone(),
        "p0-nominal-identity-lookup-alternate",
    )
    .unwrap_or_else(|error| {
        panic!("ID-WORLD-DIFFERENT: alternate world must construct: {error:?}")
    });
    assert_ne!(
        empty_declarations(documents, "p0-nominal-identity-lookup").world(),
        &alternate_world,
        "ID-WORLD-DIFFERENT: profile changes world identity",
    );
    let revised = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://project-symbol-tests/src/revised.arcw")
                .unwrap_or_else(|error| {
                    panic!("ID-REVISION-DIFFERENT: document id must be valid: {error:?}")
                }),
            SourceName::path("src/revised.arcw"),
            "pub struct Revised {}\n",
        )
        .unwrap_or_else(|error| {
            panic!("ID-REVISION-DIFFERENT: document must construct: {error:?}")
        }),
    );
    let original_revision = ProjectSymbolRevision::try_for_documents([documents[0].identity()])
        .unwrap_or_else(|error| {
            panic!("ID-REVISION-DIFFERENT: original revision must construct: {error:?}")
        });
    let revised_revision = ProjectSymbolRevision::try_for_documents([revised.identity()])
        .unwrap_or_else(|error| {
            panic!("ID-REVISION-DIFFERENT: revised revision must construct: {error:?}")
        });
    assert_ne!(
        original_revision, revised_revision,
        "ID-REVISION-DIFFERENT: source identity changes revision"
    );
}

fn assert_inaccessible_parent_import_rejected() {
    let (documents, project) = project_modules(&[
        (
            "",
            "use crate.left.*\nuse crate.right.*\nfn callable() -> Unit { () }\n",
        ),
        ("left", "pub struct Common {}\nstruct Hidden {}\n"),
        ("right", "pub enum Common { Value }\n"),
        ("left.child", "use super.Hidden\n"),
    ]);
    let declarations = declarations(
        &documents[0],
        vec![external_seed(
            &documents[0],
            "character.akane",
            [(binding_path(["character", "akane"]), false)],
        )],
        "p0-nominal-lookup-failures",
    );
    let report = ProjectSymbolTable::link(&project, &declarations)
        .expect_err("RES-INACCESS-SUPER: private parent import must reject linking");
    assert!(
        report.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic,
            ProjectSymbolLinkError::InaccessibleImport { module, .. } if module == &module_path("left.child")
        )),
        "RES-INACCESS-SUPER: child import reports typed inaccessible diagnostic: {report:?}",
    );
}
