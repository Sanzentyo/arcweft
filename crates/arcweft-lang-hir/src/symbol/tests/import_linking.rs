use super::*;

#[test]
fn group_imports_consume_terminal_import_budget() {
    let maximum = usize::try_from(super::super::ProjectSymbolLimits::PRODUCTION.imports())
        .expect("import limit fits usize");
    let per_declaration = crate::identity::HirLimit::DeclarationMembers.maximum();
    let exact_source = grouped_missing_import(maximum);
    let (documents, exact_project) = project_modules(&[
        ("", exact_source.as_str()),
        ("origin", "pub predicate target() = true\n"),
    ]);
    let exact = ProjectSymbolTable::link(
        exact_project.view(),
        &empty_declarations(&documents, "imports-exact"),
    )
    .expect("exact terminal import limit is accepted");
    let maximum_work = u64::try_from(maximum).expect("import limit fits work accounting");
    assert_eq!(
        exact.work_charged(),
        2 + maximum_work * 2,
        "one transaction admission, one target declaration, and every terminal import on both fixed-point passes are charged exactly once",
    );
    let exact = exact.into_table();
    let binding = exact
        .scopes
        .get(&CanonicalModulePath::crate_root())
        .and_then(|scope| scope.get("target"))
        .and_then(|bindings| bindings.first())
        .expect("exact-limit target binding");
    assert_eq!(binding.sites.len(), maximum.div_ceil(per_declaration) + 1);
    assert_eq!(binding.reference_sites.len(), maximum);
    assert!(binding.sites.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(
        binding
            .reference_sites
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );

    let one_over_source = grouped_missing_import(maximum + 1);
    let (documents, project) = project_modules(&[
        ("", one_over_source.as_str()),
        ("origin", "pub predicate target() = true\n"),
    ]);
    let report = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "imports-one-over"),
    )
    .expect_err("one-over terminal import limit is rejected");
    assert_symbol_limit(
        &report,
        super::super::ProjectSymbolLimitKind::Imports,
        u64::try_from(maximum + 1).expect("observed imports fit u64"),
        super::super::ProjectSymbolLimits::PRODUCTION.imports(),
    );
    assert_eq!(
        report.work_charged(),
        2,
        "one-over import inventory is rejected after transaction and target-declaration accounting but before any fixed-point import work",
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
        project.view(),
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
fn reference_only_growth_reopens_the_fixed_point_for_an_earlier_consumer() {
    // Canonical module order is intentional. The direct target reaches
    // `z_facade` first, while the second same-site group member advances one
    // module per pass through the later modules. On the decisive pass,
    // `a_consumer` has already run and the only new evidence is the second
    // terminal-reference span in `z_facade`; that reference-only growth must
    // therefore keep the fixed point open for one more consumer pass.
    let (documents, project) = project_modules(&[
        ("", ""),
        ("a_consumer", "use crate.z_facade.target\n"),
        (
            "z_facade",
            "pub use crate.zz_origin.{target, alias as target}\n",
        ),
        (
            "zz_origin",
            concat!(
                "pub use crate.zzz_middle.alias\n",
                "pub predicate target() = true\n",
            ),
        ),
        ("zzz_middle", "pub use crate.zzzz_leaf.target as alias\n"),
        ("zzzz_leaf", "pub use crate.zz_origin.target\n"),
    ]);
    let table = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "reference-only-transitive-fixed-point"),
    )
    .expect("anchored adverse-order re-export chain converges")
    .into_table();
    let consumer = module_path("a_consumer");
    let binding = table
        .scopes
        .get(&consumer)
        .and_then(|scope| scope.get("target"))
        .and_then(|bindings| bindings.first())
        .expect("consumer receives the transitive target binding");

    assert_eq!(binding.reference_sites.len(), 6);
    assert!(
        binding
            .reference_sites
            .windows(2)
            .all(|pair| pair[0] < pair[1]),
        "the complete transitive terminal-reference closure is canonical",
    );
    assert_eq!(
        documents
            .iter()
            .map(|document| {
                binding
                    .reference_sites
                    .iter()
                    .filter(|site| site.source() == document.identity())
                    .count()
            })
            .collect::<Vec<_>>(),
        [0, 1, 2, 1, 1, 1],
        "the earlier consumer receives both same-site facade references and every upstream reference exactly once",
    );
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
        project.view(),
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
        project.view(),
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
        project.view(),
        &empty_declarations(&documents, "inaccessible-import"),
    )
    .expect_err("private cross-module import is inaccessible");
    assert!(matches!(
        report.diagnostics(),
        [ProjectSymbolLinkError::InaccessibleImport { .. }]
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one visibility matrix proves direct, aliased, qualified, and inaccessible resolution parity"
)]
fn visibility_import_alias_and_qualification_are_uniform() {
    let origin = concat!(
        "pub fn public_function() -> Unit { () }\n",
        "pub predicate public_predicate() = true\n",
        "pub proof public_proof() = ()\n",
        "pub(crate) fn crate_function() -> Unit { () }\n",
        "pub(crate) predicate crate_predicate() = true\n",
        "pub(crate) proof crate_proof() = ()\n",
        "fn private_function() -> Unit { () }\n",
        "predicate private_predicate() = true\n",
        "proof private_proof() = ()\n",
    );
    let owner = concat!(
        "pub(super) fn super_function() -> Unit { () }\n",
        "pub(super) predicate super_predicate() = true\n",
        "pub(super) proof super_proof() = ()\n",
    );
    let consumer = concat!(
        "use crate.origin.public_function as direct_function\n",
        "use crate.origin.{public_predicate as grouped_predicate}\n",
        "use crate.facade.*\n",
        "use crate.origin.crate_function\n",
        "use crate.origin.crate_predicate\n",
        "use crate.origin.crate_proof\n",
    );
    let child = concat!(
        "use super.super_function\n",
        "use super.super_predicate\n",
        "use super.super_proof\n",
    );
    let (documents, project) = project_modules(&[
        ("", ""),
        ("origin", origin),
        ("facade", "pub use crate.origin.*\n"),
        ("consumer", consumer),
        ("owner", owner),
        ("owner.child", child),
    ]);
    let table = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "all-callable-import-parity"),
    )
    .expect("all callable families share one visibility/import authority")
    .into_table();
    let source = documents[0]
        .span(SourceRange::new(0, 0))
        .expect("root reference span");
    let resolve = |module: &str, spelling: &str| {
        table
            .resolve_callable(&module_path(module), &symbol_path(spelling), &source)
            .unwrap_or_else(|error| {
                panic!("{module}:{spelling} must resolve through the shared table: {error:?}")
            })
    };

    let routes = [
        (
            "origin",
            "public_function",
            "consumer",
            "direct_function",
            CallableDeclarationOwner::Function,
        ),
        (
            "origin",
            "public_predicate",
            "consumer",
            "grouped_predicate",
            CallableDeclarationOwner::Predicate,
        ),
        (
            "origin",
            "public_proof",
            "consumer",
            "public_proof",
            CallableDeclarationOwner::Proof,
        ),
        (
            "origin",
            "crate_function",
            "consumer",
            "crate_function",
            CallableDeclarationOwner::Function,
        ),
        (
            "origin",
            "crate_predicate",
            "consumer",
            "crate_predicate",
            CallableDeclarationOwner::Predicate,
        ),
        (
            "origin",
            "crate_proof",
            "consumer",
            "crate_proof",
            CallableDeclarationOwner::Proof,
        ),
        (
            "owner",
            "super_function",
            "owner.child",
            "super_function",
            CallableDeclarationOwner::Function,
        ),
        (
            "owner",
            "super_predicate",
            "owner.child",
            "super_predicate",
            CallableDeclarationOwner::Predicate,
        ),
        (
            "owner",
            "super_proof",
            "owner.child",
            "super_proof",
            CallableDeclarationOwner::Proof,
        ),
        (
            "origin",
            "private_function",
            "origin",
            "private_function",
            CallableDeclarationOwner::Function,
        ),
        (
            "origin",
            "private_predicate",
            "origin",
            "private_predicate",
            CallableDeclarationOwner::Predicate,
        ),
        (
            "origin",
            "private_proof",
            "origin",
            "private_proof",
            CallableDeclarationOwner::Proof,
        ),
    ];
    for (origin_module, origin_name, route_module, route_name, owner) in routes {
        let original = resolve(origin_module, origin_name);
        let resolved_route = resolve(route_module, route_name);
        assert_eq!(original.owner(), owner);
        assert_eq!(resolved_route.owner(), owner);
        assert_eq!(resolved_route.declaration(), original.declaration());
        assert_eq!(resolved_route.source_snapshot(), original.source_snapshot());
        assert_eq!(resolved_route.source_item(), original.source_item());
    }
    assert_eq!(
        resolve("consumer", "crate.origin.public_function").declaration(),
        resolve("origin", "public_function").declaration(),
        "qualified lookup preserves the same Function identity",
    );

    let (documents, project) = project_modules(&[
        ("", ""),
        (
            "origin",
            concat!(
                "fn hidden_function() -> Unit { () }\n",
                "predicate hidden_predicate() = true\n",
                "proof hidden_proof() = ()\n",
            ),
        ),
        (
            "consumer",
            concat!(
                "use crate.origin.hidden_function\n",
                "use crate.origin.hidden_predicate\n",
                "use crate.origin.hidden_proof\n",
            ),
        ),
    ]);
    let report = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "all-callable-inaccessible"),
    )
    .expect_err("private callable imports reject one atomic publication");
    assert_eq!(report.diagnostics().len(), 3);
    assert!(
        report.diagnostics().iter().all(|diagnostic| matches!(
            diagnostic,
            ProjectSymbolLinkError::InaccessibleImport { source, .. }
                if source.source() == documents[2].identity()
        )),
        "every inaccessible import diagnostic belongs to the exact consumer revision"
    );

    let (documents, project) = project_modules(&[
        ("", ""),
        (
            "origin",
            concat!(
                "pub(crate) fn crate_function() -> Unit { () }\n",
                "pub(crate) predicate crate_predicate() = true\n",
                "pub(crate) proof crate_proof() = ()\n",
            ),
        ),
        (
            "facade",
            concat!(
                "pub use crate.origin.crate_function\n",
                "pub use crate.origin.crate_predicate\n",
                "pub use crate.origin.crate_proof\n",
            ),
        ),
    ]);
    let report = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "all-callable-escalation"),
    )
    .expect_err("public re-export cannot widen crate visibility");
    assert_eq!(report.diagnostics().len(), 3);
    assert!(
        report.diagnostics().iter().all(|diagnostic| matches!(
            diagnostic,
            ProjectSymbolLinkError::VisibilityEscalation { source, .. }
                if source.source() == documents[2].identity()
        )),
        "every escalation diagnostic belongs to the exact facade revision"
    );

    let (documents, project) = project_modules(&[
        (
            "",
            concat!(
                "use crate.function.same as ambiguous\n",
                "use crate.predicate.same as ambiguous\n",
                "use crate.proof.same as ambiguous\n",
            ),
        ),
        ("function", "pub fn same() -> Unit { () }\n"),
        ("predicate", "pub predicate same() = true\n"),
        ("proof", "pub proof same() = ()\n"),
    ]);
    let table = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "all-callable-ambiguity"),
    )
    .expect("ordinary ambiguity is a typed resolution outcome")
    .into_table();
    let source = documents[0]
        .span(SourceRange::new(0, 3))
        .expect("ambiguity reference span");
    let error = table
        .resolve_callable(
            &CanonicalModulePath::crate_root(),
            &symbol_path("ambiguous"),
            &source,
        )
        .expect_err("three imported callable families are ambiguous");
    let ProjectSymbolResolutionError::Ambiguous {
        source: reference_source,
        candidates,
        ..
    } = error
    else {
        panic!("shared lookup must retain a typed ambiguity: {error:?}")
    };
    assert_eq!(reference_source.source(), documents[0].identity());
    assert_eq!(candidates.len(), 3);
    assert!(candidates.windows(2).all(|pair| pair[0] < pair[1]));
    let mut owners = candidates
        .iter()
        .map(|candidate| match candidate {
            ProjectSymbolTargetId::Callable(id) => table
                .callable(id)
                .expect("ambiguity candidate remains in the sole symbol table")
                .owner(),
            other => panic!("ordinary callable ambiguity cannot contain {other:?}"),
        })
        .collect::<Vec<_>>();
    owners.sort_unstable();
    assert_eq!(
        owners,
        [
            CallableDeclarationOwner::Function,
            CallableDeclarationOwner::Predicate,
            CallableDeclarationOwner::Proof,
        ]
    );
}

#[test]
fn reachable_import_cycle_resolves() {
    let (documents, project) = project_modules(&[
        ("", "use crate.a.target\n"),
        ("a", "pub use crate.b.target\n"),
        (
            "b",
            "pub use crate.a.target\npub predicate target() = true\n",
        ),
    ]);
    let table = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "reachable-cycle"),
    )
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
    let report = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "pure-cycle"),
    )
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
        project.view(),
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
            "pub use crate.a.left as target\npub use crate.b.target\npub predicate left() = true\n",
        ),
        (
            "b",
            "pub use crate.b.right as target\npub use crate.a.target\npub predicate right() = true\n",
        ),
    ]);
    let report = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "ambiguous-cycle"),
    )
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
    for index in 0..130 {
        writeln!(source, "predicate repeated{index}() = true")
            .expect("writing to a String cannot fail");
        writeln!(source, "predicate repeated{index}() = true")
            .expect("writing to a String cannot fail");
    }
    let (document, project) = project(&source);
    let report = ProjectSymbolTable::link(
        project.view(),
        &declarations(&document, Vec::new(), "diagnostic-cap"),
    )
    .expect_err("duplicate declarations exceed diagnostic cap");
    assert_eq!(report.diagnostics().len(), 128);
    assert_eq!(report.omitted_diagnostics(), 2);
    assert_eq!(report.work_charged(), 261);

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
