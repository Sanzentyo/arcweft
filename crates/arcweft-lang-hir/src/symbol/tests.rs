use std::{fmt::Write as _, sync::Arc};

use arcweft_lang_syntax::{
    ast::{
        common::Visibility,
        module_path::{CanonicalModulePath, ModulePathRoot, ModuleSegment},
        symbol_path::SymbolPath,
    },
    parser::parse_source,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use crate::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
};

use super::{
    CallablePackageId, ExternalDeclarationSeed, ProjectDirectBinding, ProjectExternalDeclarations,
    ProjectSymbolDiagnosticCode, ProjectSymbolLinkError, ProjectSymbolResolutionError,
    ProjectSymbolRevision, ProjectSymbolTable, ProjectSymbolTargetId, ProjectSymbolWorldId,
    ResolvedProjectSymbol,
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
        [HirProjectModule::new(
            CanonicalModulePath::crate_root(),
            document.identity().clone(),
            hir,
        )],
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
            let module = HirProjectModule::new(module_path(path), document.identity().clone(), hir);
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
    bindings: impl IntoIterator<Item = (String, bool)>,
) -> ExternalDeclarationSeed {
    let source = document
        .span(SourceRange::new(0, document.text().len().min(2)))
        .expect("declaration span");
    let bindings = bindings
        .into_iter()
        .map(|(name, authored_alias)| {
            ProjectDirectBinding::try_new(
                CanonicalModulePath::crate_root(),
                name,
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
            [(owner.to_owned(), false), ("akane".to_owned(), false)],
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
            external_seed(&document, "zeta", [("zeta".to_owned(), false)]),
            external_seed(&document, "alpha", [("alpha".to_owned(), false)]),
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
            [("character.akane".to_owned(), false)],
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
fn missing_import_preserves_callable_table_behavior() {
    let (document, project) = project("use crate.missing.symbol\nfn main() -> Unit { () }\n");
    let declarations = declarations(&document, Vec::new(), "missing-import");
    let table = ProjectSymbolTable::link(&project, &declarations)
        .expect("unknown imports are not link diagnostics")
        .into_table();
    let reference = SymbolPath::try_new(
        ModulePathRoot::ImplicitCrate,
        vec![ModuleSegment::new("missing").expect("segment")],
        "symbol",
    )
    .expect("reference");
    let source = document.span(SourceRange::new(0, 24)).expect("use span");

    assert!(matches!(
        table.resolve(&CanonicalModulePath::crate_root(), &reference, &source),
        Err(ProjectSymbolResolutionError::Unknown { source: actual, .. }) if actual == source
    ));
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
                [(canonical.clone(), false), (compact, false)],
            )
        })
        .collect();
    let declarations = declarations(&document, seeds, "generated-bindings");
    let link = ProjectSymbolTable::link(&project, &declarations)
        .expect("generated mandatory spellings are not authored aliases");
    assert_eq!(link.table().external_symbols().count(), 512);
}

fn aliased_missing_imports(count: usize) -> String {
    (0..count).fold(String::new(), |mut source, index| {
        writeln!(source, "use crate.missing.item{index} as alias{index}")
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
    let (document, exact_project) = project(&aliased_missing_imports(maximum));
    ProjectSymbolTable::link(
        &exact_project,
        &declarations(&document, Vec::new(), "alias-exact"),
    )
    .expect("exact per-module alias limit is accepted");

    let (document, project) = project(&aliased_missing_imports(maximum + 1));
    let report = ProjectSymbolTable::link(
        &project,
        &declarations(&document, Vec::new(), "alias-one-over"),
    )
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
            (path, aliased_missing_imports(per_module))
        })
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
    one_over_sources.push((format!("module{module_count}"), aliased_missing_imports(1)));
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
    let names = (0..count)
        .map(|index| format!("item{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("use crate.missing.{{{names}}}\n")
}

#[test]
fn group_imports_consume_terminal_import_budget() {
    let maximum = usize::try_from(super::ProjectSymbolLimits::PRODUCTION.imports())
        .expect("import limit fits usize");
    let (document, exact_project) = project(&grouped_missing_import(maximum));
    ProjectSymbolTable::link(
        &exact_project,
        &declarations(&document, Vec::new(), "imports-exact"),
    )
    .expect("exact terminal import limit is accepted");

    let (document, project) = project(&grouped_missing_import(maximum + 1));
    let report = ProjectSymbolTable::link(
        &project,
        &declarations(&document, Vec::new(), "imports-one-over"),
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
    assert_eq!(bindings[0].sites.len(), 2);
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
fn pure_import_cycle_has_no_binding_and_resolution_is_unknown() {
    let (documents, project) = project_modules(&[
        ("", "use crate.a.target\n"),
        ("a", "pub use crate.b.target\n"),
        ("b", "pub use crate.a.target\n"),
    ]);
    let table = ProjectSymbolTable::link(&project, &empty_declarations(&documents, "pure-cycle"))
        .expect("pure unresolved cycle terminates without link error")
        .into_table();
    let reference = SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "target")
        .expect("reference");
    let source = documents[0]
        .span(SourceRange::new(0, 3))
        .expect("reference source");
    assert!(matches!(
        table.resolve(&CanonicalModulePath::crate_root(), &reference, &source),
        Err(ProjectSymbolResolutionError::Unknown { .. })
    ));
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
