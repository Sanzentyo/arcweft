use super::*;

#[test]
fn group_imports_consume_terminal_import_budget() {
    let maximum = usize::try_from(super::super::ProjectSymbolLimits::PRODUCTION.imports())
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
        super::super::ProjectSymbolLimitKind::Imports,
        u64::try_from(maximum + 1).expect("observed imports fit u64"),
        super::super::ProjectSymbolLimits::PRODUCTION.imports(),
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
    let mut work = super::super::ProjectSymbolLimits::PRODUCTION.work() - 1;
    ProjectSymbolTable::charge(&mut work, 1, Some(source.clone()))
        .expect("last permitted work unit succeeds");
    assert_eq!(work, super::super::ProjectSymbolLimits::PRODUCTION.work());
    assert!(matches!(
        ProjectSymbolTable::charge(&mut work, 1, Some(source)),
        Err(ProjectSymbolLinkError::WorkOverflow { attempted, maximum, .. })
            if attempted == maximum + 1
    ));
}
