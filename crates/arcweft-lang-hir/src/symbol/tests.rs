use std::{fmt::Write as _, sync::Arc};

use arcweft_lang_syntax::{
    ast::{
        common::Visibility,
        module_path::{CanonicalModulePath, ModulePathRoot, ModuleSegment},
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
    },
    parser::parse_source,
    types::{TypePath, TypeRef, TypeRefNodePath, parse_type_ref},
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use crate::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
};

use super::{
    CallablePackageId, ExternalDeclarationSeed, ProjectDirectBinding, ProjectDirectBindingError,
    ProjectExternalDeclarations, ProjectSymbolDiagnosticCode, ProjectSymbolLinkError,
    ProjectSymbolResolutionError, ProjectSymbolRevision, ProjectSymbolTable, ProjectSymbolTargetId,
    ProjectSymbolWorldId, ProjectTypeLookupError, ProjectTypeTarget, ResolvedProjectSymbol,
    nominal::{ProjectNominalBody, ProjectNominalDeclaration, ProjectNominalDeclarationKind},
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

#[test]
fn project_direct_binding_retains_exact_typed_path_and_rejects_explicit_roots() {
    let (document, _) = project("fn main() -> Unit { () }\n");
    let source = document.span(SourceRange::new(0, 2)).expect("source span");
    let path = binding_path(["character", "akane"]);
    let binding = ProjectDirectBinding::try_new(
        CanonicalModulePath::crate_root(),
        path.clone(),
        Some(Visibility::Public),
        source.clone(),
        true,
    )
    .expect("implicit direct binding");

    assert_eq!(binding.path(), &path);
    assert_eq!(binding.visibility(), Some(Visibility::Public));
    assert_eq!(binding.source(), &source);
    assert!(binding.authored_alias());

    for root in [
        ModulePathRoot::Crate,
        ModulePathRoot::SelfModule,
        ModulePathRoot::Super(1),
    ] {
        let explicit = ProjectSymbolPath::new(
            root,
            [ProjectSymbolSegment::try_new("akane").expect("valid segment")],
        )
        .expect("explicit path");
        assert_eq!(
            ProjectDirectBinding::try_new(
                CanonicalModulePath::crate_root(),
                explicit,
                None,
                source.clone(),
                false,
            ),
            Err(ProjectDirectBindingError::ExplicitRoot { root })
        );
    }
}

#[test]
fn external_seed_keeps_canonical_identity_and_exact_binding_paths_distinct() {
    let (document, _) = project("fn main() -> Unit { () }\n");
    let source = document.span(SourceRange::new(0, 2)).expect("source span");
    let qualified = ProjectDirectBinding::try_new(
        CanonicalModulePath::crate_root(),
        binding_path(["character", "akane"]),
        Some(Visibility::Public),
        source.clone(),
        false,
    )
    .expect("qualified binding");
    let compact = ProjectDirectBinding::try_new(
        CanonicalModulePath::crate_root(),
        binding_path(["akane"]),
        Some(Visibility::Public),
        source.clone(),
        false,
    )
    .expect("compact binding");
    let alias = ProjectDirectBinding::try_new(
        CanonicalModulePath::crate_root(),
        binding_path(["hero"]),
        Some(Visibility::Public),
        source.clone(),
        true,
    )
    .expect("authored alias");
    let seed = ExternalDeclarationSeed::try_new(
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "character.akane")
            .expect("opaque canonical path"),
        Some(Visibility::Public),
        source,
        vec![qualified.clone(), compact, alias, qualified],
    )
    .expect("external seed");

    assert!(seed.canonical_path().qualifiers().is_empty());
    assert_eq!(seed.canonical_path().leaf(), "character.akane");
    assert_eq!(seed.direct_bindings().len(), 3);
    assert_eq!(
        seed.direct_bindings()
            .iter()
            .map(|binding| binding.path().to_string())
            .collect::<Vec<_>>(),
        ["akane", "character.akane", "hero"]
    );
}

#[test]
fn direct_external_paths_survive_linking_and_resolve_to_one_target() {
    let (document, project) = project("fn main() -> Unit { () }\n");
    let declarations = declarations(
        &document,
        vec![external_seed(
            &document,
            "character.akane",
            [
                (binding_path(["character", "akane"]), false),
                (binding_path(["akane"]), false),
                (binding_path(["hero"]), true),
            ],
        )],
        "typed-direct-paths",
    );
    let link = ProjectSymbolTable::link(&project, &declarations).expect("external paths link");
    let declaration = link
        .seed_declarations()
        .next()
        .expect("external declaration")
        .1;
    let expected = ProjectSymbolTargetId::External(declaration);
    let rows = scope_rows(link.table(), &CanonicalModulePath::crate_root())
        .into_iter()
        .filter(|(_, target)| target == &expected)
        .collect::<Vec<_>>();

    assert_eq!(
        rows.iter()
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>(),
        [
            vec!["akane".to_owned()],
            vec!["character".to_owned(), "akane".to_owned()],
            vec!["hero".to_owned()],
        ]
    );
    let source = document.span(SourceRange::new(0, 2)).expect("source span");
    for path in [
        binding_path(["character", "akane"]),
        binding_path(["akane"]),
        binding_path(["hero"]),
    ] {
        let reference = SymbolPath::try_from(&path).expect("typed resolution reference");
        assert!(matches!(
            link.table().resolve(
                &CanonicalModulePath::crate_root(),
                &reference,
                &source
            ),
            Ok(ResolvedProjectSymbol::External(symbol))
                if symbol.declaration() == declaration
        ));
    }
}

#[test]
fn unaliased_and_explicit_alias_imports_use_typed_destination_paths() {
    let (documents, project) = project_modules(&[
        ("", "fn main() -> Unit { () }\n"),
        (
            "consumer",
            "use character.akane\nuse character.akane as hero\n",
        ),
    ]);
    let declarations = declarations(
        &documents[0],
        vec![external_seed(
            &documents[0],
            "character.akane",
            [(binding_path(["character", "akane"]), false)],
        )],
        "typed-path-imports",
    );
    let link = ProjectSymbolTable::link(&project, &declarations).expect("imports link");
    let target = ProjectSymbolTargetId::External(
        link.seed_declarations()
            .next()
            .expect("external declaration")
            .1,
    );
    let rows = scope_rows(link.table(), &module_path("consumer"))
        .into_iter()
        .filter(|(_, candidate)| candidate == &target)
        .collect::<Vec<_>>();

    assert_eq!(
        rows.iter()
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>(),
        [vec!["akane".to_owned()], vec!["hero".to_owned()]]
    );
    assert!(
        scope_rows(link.table(), &CanonicalModulePath::crate_root())
            .iter()
            .any(|(path, candidate)| {
                path == &["character".to_owned(), "akane".to_owned()] && candidate == &target
            }),
        "the source qualified binding remains independent"
    );
}

#[test]
fn grouped_imports_use_selected_and_alias_segments() {
    let (documents, project) = project_modules(&[
        ("", "fn main() -> Unit { () }\n"),
        (
            "cast",
            "pub fn akane() -> Unit { () }\npub fn hero() -> Unit { () }\n",
        ),
        ("consumer", "use crate.cast.{akane, hero as lead}\n"),
    ]);
    let table = ProjectSymbolTable::link(
        &project,
        &empty_declarations(&documents, "typed-group-imports"),
    )
    .expect("grouped imports link")
    .into_table();
    let rows = scope_rows(&table, &module_path("consumer"));

    assert!(rows.iter().any(|(path, target)| {
        path == &["akane".to_owned()] && matches!(target, ProjectSymbolTargetId::Callable(_))
    }));
    assert!(rows.iter().any(|(path, target)| {
        path == &["lead".to_owned()] && matches!(target, ProjectSymbolTargetId::Callable(_))
    }));
}

#[test]
fn glob_and_fixed_point_reexport_preserve_qualified_external_segments() {
    let (documents, project) = project_modules(&[
        ("", "fn main() -> Unit { () }\n"),
        ("origin", ""),
        ("middle", "pub use crate.origin.*\n"),
        ("consumer", "use crate.middle.*\n"),
    ]);
    let declarations = declarations(
        &documents[0],
        vec![external_seed_in_module(
            &documents[0],
            "character.akane",
            &module_path("origin"),
            [(binding_path(["character", "akane"]), false)],
        )],
        "typed-glob-reexport",
    );
    let link = ProjectSymbolTable::link(&project, &declarations).expect("glob chain links");
    let target = ProjectSymbolTargetId::External(
        link.seed_declarations()
            .next()
            .expect("external declaration")
            .1,
    );

    for module in ["origin", "middle", "consumer"] {
        assert!(
            scope_rows(link.table(), &module_path(module))
                .iter()
                .any(|(path, candidate)| {
                    path == &["character".to_owned(), "akane".to_owned()] && candidate == &target
                }),
            "{module} must retain the exact qualified external path"
        );
    }
}

#[test]
fn external_only_qualifier_import_retains_the_full_typed_binding() {
    let (documents, project) = project_modules(&[
        ("", "fn main() -> Unit { () }\n"),
        ("consumer", "use character.hero-pack.2d\n"),
    ]);
    let declarations = declarations(
        &documents[0],
        vec![external_seed(
            &documents[0],
            "character.hero-pack.2d",
            [(binding_path(["character", "hero-pack", "2d"]), false)],
        )],
        "external-only-import",
    );
    let link =
        ProjectSymbolTable::link(&project, &declarations).expect("external-only import links");
    let target = ProjectSymbolTargetId::External(
        link.seed_declarations()
            .next()
            .expect("external declaration")
            .1,
    );

    assert!(
        scope_rows(link.table(), &module_path("consumer"))
            .iter()
            .any(|(path, candidate)| {
                path == &[
                    "character".to_owned(),
                    "hero-pack".to_owned(),
                    "2d".to_owned(),
                ] && candidate == &target
            })
    );
}

#[test]
fn typed_scope_iterator_is_insertion_order_independent_and_mixes_target_kinds() {
    let (documents, project) =
        project_modules(&[("", "fn main() -> Unit { () }\n"), ("child", "")]);
    let forward_seed = external_seed(
        &documents[0],
        "character.akane",
        [
            (binding_path(["character", "akane"]), false),
            (binding_path(["akane"]), false),
        ],
    );
    let reverse_seed = external_seed(
        &documents[0],
        "character.akane",
        [
            (binding_path(["akane"]), false),
            (binding_path(["character", "akane"]), false),
        ],
    );
    let forward = ProjectSymbolTable::link(
        &project,
        &declarations(
            &documents[0],
            vec![forward_seed],
            "typed-iterator-determinism",
        ),
    )
    .expect("forward facts link");
    let reverse = ProjectSymbolTable::link(
        &project,
        &declarations(
            &documents[0],
            vec![reverse_seed],
            "typed-iterator-determinism",
        ),
    )
    .expect("reverse facts link");
    let rows = |table: &ProjectSymbolTable| {
        table
            .scope_bindings()
            .map(|(module, path, target)| (module.clone(), path.clone(), target.clone()))
            .collect::<Vec<_>>()
    };

    assert_eq!(rows(forward.table()), rows(reverse.table()));
    let root_rows = scope_rows(forward.table(), &CanonicalModulePath::crate_root());
    assert!(
        root_rows
            .iter()
            .any(|(_, target)| matches!(target, ProjectSymbolTargetId::Callable(_)))
    );
    assert!(
        root_rows
            .iter()
            .any(|(_, target)| matches!(target, ProjectSymbolTargetId::Module(_)))
    );
    assert!(
        root_rows
            .iter()
            .any(|(_, target)| matches!(target, ProjectSymbolTargetId::External(_)))
    );
}

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

#[test]
fn limit_aliases_per_module_exact_and_one_over() {
    let maximum = usize::try_from(super::ProjectSymbolLimits::PRODUCTION.aliases_per_module())
        .expect("alias limit fits usize");
    let exact_source = aliased_target_imports(maximum);
    let (documents, exact_project) = project_modules(&[
        ("", exact_source.as_str()),
        ("origin", "pub fn target() -> Unit { () }\n"),
    ]);
    ProjectSymbolTable::link(
        &exact_project,
        &empty_declarations(&documents, "alias-exact"),
    )
    .expect("exact per-module alias limit is accepted");

    let one_over_source = aliased_target_imports(maximum + 1);
    let (documents, project) = project_modules(&[
        ("", one_over_source.as_str()),
        ("origin", "pub fn target() -> Unit { () }\n"),
    ]);
    let report =
        ProjectSymbolTable::link(&project, &empty_declarations(&documents, "alias-one-over"))
            .expect_err("one-over per-module alias limit is rejected");
    assert_symbol_limit(
        &report,
        super::ProjectSymbolLimitKind::AliasesPerModule,
        u64::try_from(maximum + 1).expect("observed aliases fit u64"),
        super::ProjectSymbolLimits::PRODUCTION.aliases_per_module(),
    );
}

#[test]
fn limit_aliases_world_exact_and_one_over() {
    let per_module = usize::try_from(super::ProjectSymbolLimits::PRODUCTION.aliases_per_module())
        .expect("per-module limit fits usize");
    let world = usize::try_from(super::ProjectSymbolLimits::PRODUCTION.aliases_per_world())
        .expect("world limit fits usize");
    let module_count = world / per_module;
    let exact_sources = (0..module_count)
        .map(|index| {
            let path = if index == 0 {
                String::new()
            } else {
                format!("module{index}")
            };
            (path, aliased_target_imports(per_module))
        })
        .chain([(
            "origin".to_owned(),
            "pub fn target() -> Unit { () }\n".to_owned(),
        )])
        .collect::<Vec<_>>();
    let exact_refs = exact_sources
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_str()))
        .collect::<Vec<_>>();
    let (documents, project) = project_modules(&exact_refs);
    ProjectSymbolTable::link(
        &project,
        &empty_declarations(&documents, "world-alias-exact"),
    )
    .expect("exact world alias limit is accepted");

    let mut one_over_sources = exact_sources;
    one_over_sources.push((format!("module{module_count}"), aliased_target_imports(1)));
    let one_over_refs = one_over_sources
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_str()))
        .collect::<Vec<_>>();
    let (documents, project) = project_modules(&one_over_refs);
    let report = ProjectSymbolTable::link(
        &project,
        &empty_declarations(&documents, "world-alias-one-over"),
    )
    .expect_err("one-over world alias limit is rejected");
    assert_symbol_limit(
        &report,
        super::ProjectSymbolLimitKind::AliasesPerWorld,
        super::ProjectSymbolLimits::PRODUCTION.aliases_per_world() + 1,
        super::ProjectSymbolLimits::PRODUCTION.aliases_per_world(),
    );
}

fn grouped_missing_import(count: usize) -> String {
    let names = (0..count).map(|_| "target").collect::<Vec<_>>().join(", ");
    format!("use crate.origin.{{{names}}}\n")
}

#[test]
fn group_imports_consume_terminal_import_budget() {
    let maximum = usize::try_from(super::ProjectSymbolLimits::PRODUCTION.imports())
        .expect("import limit fits usize");
    let exact_source = grouped_missing_import(maximum);
    let (documents, exact_project) = project_modules(&[
        ("", exact_source.as_str()),
        ("origin", "pub fn target() -> Unit { () }\n"),
    ]);
    ProjectSymbolTable::link(
        &exact_project,
        &empty_declarations(&documents, "imports-exact"),
    )
    .expect("exact terminal import limit is accepted");

    let one_over_source = grouped_missing_import(maximum + 1);
    let (documents, project) = project_modules(&[
        ("", one_over_source.as_str()),
        ("origin", "pub fn target() -> Unit { () }\n"),
    ]);
    let report = ProjectSymbolTable::link(
        &project,
        &empty_declarations(&documents, "imports-one-over"),
    )
    .expect_err("one-over terminal import limit is rejected");
    assert_symbol_limit(
        &report,
        super::ProjectSymbolLimitKind::Imports,
        u64::try_from(maximum + 1).expect("observed imports fit u64"),
        super::ProjectSymbolLimits::PRODUCTION.imports(),
    );
}

#[test]
fn project_symbol_error_codes_are_exhaustive() {
    assert_eq!(
        [
            ProjectSymbolDiagnosticCode::DuplicateDeclaration,
            ProjectSymbolDiagnosticCode::InaccessibleImport,
            ProjectSymbolDiagnosticCode::VisibilityEscalation,
            ProjectSymbolDiagnosticCode::AmbiguousImport,
            ProjectSymbolDiagnosticCode::InvalidImportPath,
            ProjectSymbolDiagnosticCode::InvalidDeclaration,
            ProjectSymbolDiagnosticCode::UnknownImport,
            ProjectSymbolDiagnosticCode::CyclicImport,
            ProjectSymbolDiagnosticCode::ReservedTypeName,
            ProjectSymbolDiagnosticCode::InvalidNominalDeclaration,
            ProjectSymbolDiagnosticCode::Limit,
            ProjectSymbolDiagnosticCode::WorkOverflow,
        ]
        .map(ProjectSymbolDiagnosticCode::as_str),
        [
            "aw.project.symbol.duplicate_declaration",
            "aw.project.symbol.inaccessible_import",
            "aw.project.symbol.visibility_escalation",
            "aw.project.symbol.ambiguous_import",
            "aw.project.symbol.invalid_import_path",
            "aw.project.symbol.invalid_declaration",
            "aw.project.symbol.unknown_import",
            "aw.project.symbol.cyclic_import",
            "aw.project.symbol.reserved_type_name",
            "aw.project.symbol.invalid_nominal_declaration",
            "aw.project.symbol.limit",
            "aw.project.symbol.work_overflow",
        ]
    );
}

#[test]
fn same_target_imports_coalesce() {
    let (documents, project) = project_modules(&[
        (
            "",
            "use crate.a.target\nuse crate.b.target\nfn main() -> Unit { () }\n",
        ),
        ("a", "pub fn target() -> Unit { () }\n"),
        ("b", "pub use crate.a.target\n"),
    ]);
    let table = ProjectSymbolTable::link(
        &project,
        &empty_declarations(&documents, "same-target-imports"),
    )
    .expect("same-target imports link")
    .into_table();
    let root = CanonicalModulePath::crate_root();
    let bindings = table
        .scopes
        .get(&root)
        .and_then(|scope| scope.get("target"))
        .expect("target binding");

    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].sites.len(), 4);
    assert_eq!(
        documents
            .iter()
            .map(|document| {
                bindings[0]
                    .sites
                    .iter()
                    .filter(|site| site.source() == document.identity())
                    .count()
            })
            .collect::<Vec<_>>(),
        vec![2, 1, 1],
        "both root import sites and every upstream declaration/re-export site survive coalescing"
    );
    let reference = SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "target")
        .expect("reference");
    let source = documents[0]
        .span(SourceRange::new(0, 3))
        .expect("reference source");
    assert!(matches!(
        table.resolve(&root, &reference, &source),
        Ok(ResolvedProjectSymbol::Callable(symbol)) if symbol.declaration().name() == "target"
    ));
}

#[test]
fn different_targets_are_deterministically_ambiguous() {
    let (documents, project) = project_modules(&[
        (
            "",
            "use crate.a.target\nuse crate.b.target\nfn main() -> Unit { () }\n",
        ),
        ("a", "pub fn target() -> Unit { () }\n"),
        ("b", "pub fn target() -> Unit { () }\n"),
    ]);
    let table = ProjectSymbolTable::link(
        &project,
        &empty_declarations(&documents, "different-targets"),
    )
    .expect("ordinary ambiguity remains a resolution outcome")
    .into_table();
    let reference = SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "target")
        .expect("reference");
    let source = documents[0]
        .span(SourceRange::new(0, 3))
        .expect("reference source");
    let first = table
        .resolve(&CanonicalModulePath::crate_root(), &reference, &source)
        .expect_err("ambiguous target");
    let second = table
        .resolve(&CanonicalModulePath::crate_root(), &reference, &source)
        .expect_err("deterministic ambiguity");

    assert_eq!(first, second);
    let ProjectSymbolResolutionError::Ambiguous { candidates, .. } = first else {
        panic!("expected typed ambiguity");
    };
    assert_eq!(candidates.len(), 2);
    assert!(candidates.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn visibility_and_reexport_parity() {
    let (documents, project) = project_modules(&[
        ("", "use crate.a.visible\nuse crate.b.exported\n"),
        (
            "a",
            "pub fn visible() -> Unit { () }\nfn hidden() -> Unit { () }\n",
        ),
        ("b", "pub use crate.a.visible as exported\n"),
    ]);
    let table = ProjectSymbolTable::link(
        &project,
        &empty_declarations(&documents, "visibility-reexport"),
    )
    .expect("public import and re-export link")
    .into_table();
    let root = CanonicalModulePath::crate_root();
    let source = documents[0]
        .span(SourceRange::new(0, 3))
        .expect("reference source");
    let resolves_callable = |name: &str| {
        let reference = SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), name)
            .expect("reference");
        matches!(
            table.resolve(&root, &reference, &source),
            Ok(ResolvedProjectSymbol::Callable(_))
        )
    };

    assert!(resolves_callable("visible"));
    assert!(resolves_callable("exported"));

    let (documents, project) = project_modules(&[
        ("", "use crate.a.hidden\n"),
        ("a", "fn hidden() -> Unit { () }\n"),
    ]);
    let report = ProjectSymbolTable::link(
        &project,
        &empty_declarations(&documents, "inaccessible-import"),
    )
    .expect_err("private cross-module import is inaccessible");
    assert!(matches!(
        report.diagnostics(),
        [ProjectSymbolLinkError::InaccessibleImport { .. }]
    ));
}

#[test]
fn reachable_import_cycle_resolves() {
    let (documents, project) = project_modules(&[
        ("", "use crate.a.target\n"),
        ("a", "pub use crate.b.target\n"),
        (
            "b",
            "pub fn target() -> Unit { () }\npub use crate.a.target\n",
        ),
    ]);
    let table =
        ProjectSymbolTable::link(&project, &empty_declarations(&documents, "reachable-cycle"))
            .expect("reachable cycle converges")
            .into_table();
    let reference = SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "target")
        .expect("reference");
    let source = documents[0]
        .span(SourceRange::new(0, 3))
        .expect("reference source");
    assert!(matches!(
        table.resolve(&CanonicalModulePath::crate_root(), &reference, &source),
        Ok(ResolvedProjectSymbol::Callable(symbol)) if symbol.declaration().name() == "target"
    ));
}

#[test]
fn pure_import_cycle_is_rejected_with_related_cycle_sources() {
    let (documents, project) = project_modules(&[
        ("", "use crate.a.target\n"),
        ("a", "pub use crate.b.target\n"),
        ("b", "pub use crate.a.target\n"),
    ]);
    let report = ProjectSymbolTable::link(&project, &empty_declarations(&documents, "pure-cycle"))
        .expect_err("unanchored import cycles cannot publish a partial symbol table");
    let cyclic = report
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            ProjectSymbolLinkError::CyclicImport {
                source, related, ..
            } => Some((source, related.as_ref())),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(cyclic.len(), 2);
    assert!(
        cyclic.iter().all(|(source, related)| {
            related.len() == 1 && related[0].source() != source.source()
        })
    );
    assert!(report.diagnostics().iter().all(|diagnostic| matches!(
        diagnostic.code(),
        ProjectSymbolDiagnosticCode::CyclicImport | ProjectSymbolDiagnosticCode::UnknownImport
    )));
}

#[test]
fn three_module_unanchored_cycle_reports_every_edge_with_related_sources() {
    let (documents, project) = project_modules(&[
        ("", ""),
        ("a", "pub use crate.b.target\n"),
        ("b", "pub use crate.c.target\n"),
        ("c", "pub use crate.a.target\n"),
    ]);
    let report = ProjectSymbolTable::link(
        &project,
        &empty_declarations(&documents, "three-module-pure-cycle"),
    )
    .expect_err("an unanchored three-node cycle is rejected");
    let cyclic = report
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            ProjectSymbolLinkError::CyclicImport {
                module,
                source,
                related,
                ..
            } => Some((module, source, related.as_ref())),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(cyclic.len(), 3);
    assert!(cyclic.iter().all(|(_, source, related)| {
        related.len() == 2
            && related.iter().all(|site| site.source() != source.source())
            && related.windows(2).all(|pair| pair[0] < pair[1])
    }));
    assert_eq!(
        cyclic
            .iter()
            .map(|(module, _, _)| module.to_string())
            .collect::<Vec<_>>(),
        ["crate.a", "crate.b", "crate.c"]
    );
    assert!(cyclic.iter().all(|(_, _, related)| {
        related.iter().all(|site| {
            documents[1..]
                .iter()
                .any(|document| site.source() == document.identity())
        })
    }));
}

#[test]
fn ambiguous_reexport_cycle() {
    let (documents, project) = project_modules(&[
        ("", ""),
        (
            "a",
            "pub fn left() -> Unit { () }\npub use crate.a.left as target\npub use crate.b.target\n",
        ),
        (
            "b",
            "pub fn right() -> Unit { () }\npub use crate.b.right as target\npub use crate.a.target\n",
        ),
    ]);
    let report =
        ProjectSymbolTable::link(&project, &empty_declarations(&documents, "ambiguous-cycle"))
            .expect_err("cycle exposes two distinct targets");

    assert!(report.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic,
        ProjectSymbolLinkError::AmbiguousImport { candidates, .. }
            if candidates.len() == 2 && candidates.windows(2).all(|pair| pair[0] < pair[1])
    )));
}

#[test]
fn link_report_cap_and_work_are_observable() {
    let mut source = String::new();
    for _ in 0..130 {
        source.push_str("fn repeated() -> Unit { () }\n");
    }
    let (document, project) = project(&source);
    let report = ProjectSymbolTable::link(
        &project,
        &declarations(&document, Vec::new(), "diagnostic-cap"),
    )
    .expect_err("duplicate declarations exceed diagnostic cap");
    assert_eq!(report.diagnostics().len(), 128);
    assert_eq!(report.omitted_diagnostics(), 1);
    assert_eq!(report.work_charged(), 131);

    let source = document.span(SourceRange::new(0, 1)).expect("work source");
    let mut work = super::ProjectSymbolLimits::PRODUCTION.work() - 1;
    ProjectSymbolTable::charge(&mut work, 1, Some(source.clone()))
        .expect("last permitted work unit succeeds");
    assert_eq!(work, super::ProjectSymbolLimits::PRODUCTION.work());
    assert!(matches!(
        ProjectSymbolTable::charge(&mut work, 1, Some(source)),
        Err(ProjectSymbolLinkError::WorkOverflow { attempted, maximum, .. })
            if attempted == maximum + 1
    ));
}
