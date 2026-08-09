use super::*;

#[test]
fn nominal_declaration_limits_are_inclusive_and_report_typed_one_over_counts() {
    let per_module = usize::try_from(
        super::super::ProjectSymbolLimits::PRODUCTION.nominal_declarations_per_module(),
    )
    .expect("per-module nominal declaration limit fits usize");
    let exact_source = nominal_alias_declarations(per_module);
    let (documents, project) = project_modules(&[("", &exact_source)]);
    ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "nominal-declarations-per-module-exact"),
    )
    .expect("the exact per-module nominal declaration limit is accepted");

    let one_over_source = nominal_alias_declarations(per_module + 1);
    let (documents, project) = project_modules(&[("", &one_over_source)]);
    let report = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "nominal-declarations-per-module-one-over"),
    )
    .expect_err("one over the per-module nominal declaration limit is rejected");
    assert_symbol_limit(
        &report,
        super::super::ProjectSymbolLimitKind::NominalDeclarationsPerModule,
        u64::try_from(per_module + 1).expect("observed declaration count fits u64"),
        super::super::ProjectSymbolLimits::PRODUCTION.nominal_declarations_per_module(),
    );

    let per_world = usize::try_from(
        super::super::ProjectSymbolLimits::PRODUCTION.nominal_declarations_per_world(),
    )
    .expect("world nominal declaration limit fits usize");
    let module_count = per_world / per_module;
    let exact_sources = (0..module_count)
        .map(|index| {
            (
                (index == 0)
                    .then(String::new)
                    .unwrap_or_else(|| format!("m{index}")),
                nominal_alias_declarations(per_module),
            )
        })
        .collect::<Vec<_>>();
    let exact_refs = exact_sources
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_str()))
        .collect::<Vec<_>>();
    let (documents, project) = project_modules(&exact_refs);
    ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "nominal-declarations-per-world-exact"),
    )
    .expect("the exact world nominal declaration limit is accepted");

    let mut one_over_sources = exact_sources;
    one_over_sources.push(("overflow".to_owned(), nominal_alias_declarations(1)));
    let one_over_refs = one_over_sources
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_str()))
        .collect::<Vec<_>>();
    let (documents, project) = project_modules(&one_over_refs);
    let report = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "nominal-declarations-per-world-one-over"),
    )
    .expect_err("one over the world nominal declaration limit is rejected");
    assert_symbol_limit(
        &report,
        super::super::ProjectSymbolLimitKind::NominalDeclarationsPerWorld,
        super::super::ProjectSymbolLimits::PRODUCTION.nominal_declarations_per_world() + 1,
        super::super::ProjectSymbolLimits::PRODUCTION.nominal_declarations_per_world(),
    );
}

#[test]
fn nominal_shape_limits_are_inclusive_and_report_typed_one_over_counts() {
    let parameters =
        usize::try_from(super::super::ProjectSymbolLimits::PRODUCTION.nominal_type_parameters())
            .expect("parameter limit fits usize");
    let exact_source = nominal_type_parameters(parameters);
    let (documents, project) = project_modules(&[("", &exact_source)]);
    ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "nominal-parameters-exact"),
    )
    .expect("the exact nominal type parameter limit is accepted");

    let one_over_source = nominal_type_parameters(parameters + 1);
    let (documents, project) = project_modules(&[("", &one_over_source)]);
    let report = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "nominal-parameters-one-over"),
    )
    .expect_err("one over the nominal type parameter limit is rejected");
    assert_symbol_limit(
        &report,
        super::super::ProjectSymbolLimitKind::NominalTypeParameters,
        u64::try_from(parameters + 1).expect("observed parameter count fits u64"),
        super::super::ProjectSymbolLimits::PRODUCTION.nominal_type_parameters(),
    );

    let type_nodes =
        super::super::ProjectSymbolLimits::PRODUCTION.nominal_type_nodes_per_declaration();
    let exact_source = nominal_type_node_fields(false);
    let (documents, project) = project_modules(&[("", &exact_source)]);
    ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "nominal-type-nodes-exact"),
    )
    .expect("the exact nominal declaration type-node limit is accepted");

    let one_over_source = nominal_type_node_fields(true);
    let (documents, project) = project_modules(&[("", &one_over_source)]);
    let report = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "nominal-type-nodes-one-over"),
    )
    .expect_err("one over the nominal declaration type-node limit is rejected");
    assert_symbol_limit(
        &report,
        super::super::ProjectSymbolLimitKind::NominalTypeNodesPerDeclaration,
        type_nodes + 1,
        type_nodes,
    );
}

#[test]
fn limit_aliases_per_module_exact_and_one_over() {
    let maximum =
        usize::try_from(super::super::ProjectSymbolLimits::PRODUCTION.aliases_per_module())
            .expect("alias limit fits usize");
    let exact_source = aliased_target_imports(maximum);
    let (documents, exact_project) = project_modules(&[
        ("", exact_source.as_str()),
        ("origin", "pub fn target() -> Unit { () }\n"),
    ]);
    ProjectSymbolTable::link(
        exact_project.view(),
        &empty_declarations(&documents, "alias-exact"),
    )
    .expect("exact per-module alias limit is accepted");

    let one_over_source = aliased_target_imports(maximum + 1);
    let (documents, project) = project_modules(&[
        ("", one_over_source.as_str()),
        ("origin", "pub fn target() -> Unit { () }\n"),
    ]);
    let report = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "alias-one-over"),
    )
    .expect_err("one-over per-module alias limit is rejected");
    assert_symbol_limit(
        &report,
        super::super::ProjectSymbolLimitKind::AliasesPerModule,
        u64::try_from(maximum + 1).expect("observed aliases fit u64"),
        super::super::ProjectSymbolLimits::PRODUCTION.aliases_per_module(),
    );
}

#[test]
fn limit_aliases_world_exact_and_one_over() {
    let per_module =
        usize::try_from(super::super::ProjectSymbolLimits::PRODUCTION.aliases_per_module())
            .expect("per-module limit fits usize");
    let world = usize::try_from(super::super::ProjectSymbolLimits::PRODUCTION.aliases_per_world())
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
        project.view(),
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
        project.view(),
        &empty_declarations(&documents, "world-alias-one-over"),
    )
    .expect_err("one-over world alias limit is rejected");
    assert_symbol_limit(
        &report,
        super::super::ProjectSymbolLimitKind::AliasesPerWorld,
        super::super::ProjectSymbolLimits::PRODUCTION.aliases_per_world() + 1,
        super::super::ProjectSymbolLimits::PRODUCTION.aliases_per_world(),
    );
}
