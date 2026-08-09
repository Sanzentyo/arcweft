use super::*;

fn absolute_entity_reference(value: &str) -> HirIdRef {
    HirIdRef::absolute(
        HirEntityReference::try_new(value.into()).expect("valid absolute entity reference"),
    )
}

#[test]
fn flow_uses_one_structural_symbol_without_entering_the_value_namespace() {
    let (document, project) = project("flow opening {}\n");
    let table = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(std::slice::from_ref(&document), "flow-structural-symbol"),
    )
    .expect("Flow structural symbol links")
    .into_table();
    let symbol = table
        .callable_symbols()
        .find(|symbol| symbol.owner() == CallableDeclarationOwner::Flow)
        .expect("Flow has one structural callable record owner");
    let CallableDeclarationKey::Flow(flow) = symbol.declaration() else {
        panic!("Flow symbol must retain the structural Flow key")
    };
    assert_eq!(flow.public_id().as_str(), "flow.opening");
    assert_eq!(flow.publication(), FlowPublicationKind::ModuleScoped);
    assert_eq!(
        table.flow_symbol_for_item(symbol.source_item()),
        Some(symbol)
    );

    let root = CanonicalModulePath::crate_root();
    let source = document
        .span(SourceRange::new(0, "flow opening".len()))
        .expect("reference source");
    let reference = absolute_entity_reference("flow.opening");
    assert!(matches!(
        table.resolve_entity_reference(&root, &reference, source.clone()),
        Ok(ResolvedProjectSymbol::StructuralCallable(resolved))
            if resolved.declaration() == symbol.declaration()
    ));
    assert!(matches!(
        table.resolve_value_target(&root, &symbol_path("flow.opening"), source.clone()),
        Ok(ProjectValueLookup::Absent)
    ));
    assert!(matches!(
        table.resolve_callable(&root, &symbol_path("flow.opening"), &source),
        Err(ProjectSymbolResolutionError::NotCallable {
            actual: ProjectSymbolTargetId::StructuralCallable(_),
            ..
        })
    ));
}

#[test]
fn authored_absolute_parameterized_flow_resolves_through_its_structural_symbol() {
    let source = concat!(
        "struct GameState { score: i32 }\n",
        "flow @flow.opening opening(current: GameState) {}\n",
    );
    let (document, project) = project(source);
    let table = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(std::slice::from_ref(&document), "parameterized-flow-symbol"),
    )
    .expect("parameterized authored Flow structural symbol links")
    .into_table();
    let symbol = table
        .callable_symbols()
        .find(|symbol| symbol.owner() == CallableDeclarationOwner::Flow)
        .expect("parameterized authored Flow has one structural symbol");
    let CallableDeclarationKey::Flow(flow) = symbol.declaration() else {
        panic!("Flow symbol must retain the structural Flow key")
    };
    assert_eq!(flow.public_id().as_str(), "flow.opening");
    assert_eq!(flow.publication(), FlowPublicationKind::AuthoredAbsolute);

    let source = document
        .span(SourceRange::new(
            source.find("@flow.opening").expect("Flow ID") + 1,
            source.find("@flow.opening").expect("Flow ID") + "@flow.opening".len(),
        ))
        .expect("entity-reference source");
    let reference = absolute_entity_reference("flow.opening");
    assert!(matches!(
        table.resolve_entity_reference(
            &CanonicalModulePath::crate_root(),
            &reference,
            source,
        ),
        Ok(ResolvedProjectSymbol::StructuralCallable(resolved))
            if resolved.declaration() == symbol.declaration()
    ));
}

#[test]
fn name_derived_flow_identity_remains_module_preserving() {
    let (documents, project) = project_modules(&[
        ("", "fn root() -> Unit { () }\n"),
        ("left", "flow opening {}\n"),
        ("right", "flow opening {}\n"),
    ]);
    let table = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "module-preserving-flow"),
    )
    .expect("same name-derived Flow identity remains legal in distinct modules")
    .into_table();
    let flows = table
        .callable_symbols()
        .filter_map(|symbol| match symbol.declaration() {
            CallableDeclarationKey::Flow(flow) => Some((symbol, flow)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(flows.len(), 2);
    assert_eq!(flows[0].1.public_id().as_str(), "flow.opening");
    assert_eq!(flows[1].1.public_id().as_str(), "flow.opening");
    assert_ne!(flows[0].1.module(), flows[1].1.module());

    let reference = absolute_entity_reference("flow.opening");
    for (index, module_name) in [(1, "left"), (2, "right")] {
        let module = module_path(module_name);
        let source = documents[index]
            .span(SourceRange::new(0, "flow opening".len()))
            .expect("reference source");
        let resolved = table.resolve_entity_reference(&module, &reference, source);
        assert!(
            matches!(
                &resolved,
                Ok(ResolvedProjectSymbol::StructuralCallable(symbol))
                    if symbol.declaration().module() == &module
            ),
            "module-scoped Flow resolution failed: {resolved:#?}"
        );
    }
}

#[test]
fn duplicate_authored_absolute_flow_id_is_rejected_project_wide() {
    let (documents, project) = project_modules(&[
        ("", "fn root() -> Unit { () }\n"),
        ("left", "flow @flow.shared {}\n"),
        ("right", "flow @flow.shared {}\n"),
    ]);
    let report = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "duplicate-absolute-flow"),
    )
    .expect_err("authored absolute Flow IDs are project-global");
    assert!(matches!(
        report.diagnostics(),
        [ProjectSymbolLinkError::DuplicatePublicId { public_id, .. }]
            if public_id.as_str() == "flow.shared"
    ));
}

#[test]
fn one_symbol_table_registers_all_callable_kinds_and_character() {
    let source = concat!(
        "fn work() -> Unit { () }\n",
        "predicate is_ready() = true\n",
        "proof readiness() = ()\n",
    );
    let (document, project) = project(source);
    let character = external_seed(
        &document,
        "character.akane",
        [
            (binding_path(["character", "akane"]), false),
            (binding_path(["akane"]), false),
        ],
    );
    let table = ProjectSymbolTable::link(
        project.view(),
        &declarations(&document, vec![character], "all-callable-families"),
    )
    .expect("one typed symbol table")
    .into_table();

    let callables = table
        .callable_symbols()
        .map(|symbol| {
            (
                symbol.declaration().name().to_owned(),
                symbol.owner(),
                symbol.source_snapshot(),
                symbol.source_item(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(callables.len(), 3);
    assert_eq!(
        callables
            .iter()
            .map(|(name, owner, _, _)| (name.as_str(), *owner))
            .collect::<Vec<_>>(),
        [
            ("work", CallableDeclarationOwner::Function),
            ("is_ready", CallableDeclarationOwner::Predicate),
            ("readiness", CallableDeclarationOwner::Proof),
        ]
    );

    let root = CanonicalModulePath::crate_root();
    let module = project.module(&root).expect("root project module").module();
    for (_, _, snapshot, item) in &callables {
        assert_eq!(*snapshot, module.snapshot_id());
        assert!(module.resolve_item(*item).is_ok());
    }

    let external = table
        .external_symbols()
        .next()
        .expect("Character external declaration");
    assert_eq!(table.external_symbols().count(), 1);
    assert_eq!(external.canonical_path().to_string(), "character.akane");
    let expected = ProjectSymbolTargetId::External(external.declaration());
    let bindings = scope_rows(&table, &root);
    assert!(bindings.contains(&(
        vec!["character".to_owned(), "akane".to_owned()],
        expected.clone(),
    )));
    assert!(bindings.contains(&(vec!["akane".to_owned()], expected)));
}

#[test]
fn ordinary_callable_duplicate_names_are_reported_together() {
    let source = concat!(
        "fn repeated() -> Unit { () }\n",
        "predicate repeated() = true\n",
        "proof repeated() = ()\n",
    );
    let (document, project) = project(source);
    let report = ProjectSymbolTable::link(
        project.view(),
        &declarations(&document, Vec::new(), "ordinary-duplicate-families"),
    )
    .expect_err("ordinary callable names do not form overload sets");

    let [
        ProjectSymbolLinkError::DuplicateDeclaration {
            module,
            name,
            sites,
        },
    ] = report.diagnostics()
    else {
        panic!(
            "one grouped duplicate-name diagnostic must own every source site: {:?}",
            report.diagnostics()
        )
    };
    assert_eq!(module, &CanonicalModulePath::crate_root());
    assert_eq!(name, "repeated");
    assert_eq!(sites.len(), 3);
    assert!(
        sites
            .windows(2)
            .all(|pair| pair[0].range() < pair[1].range())
    );
    assert!(
        sites.iter().all(|site| {
            &document.text()[site.range().start()..site.range().end()] == "repeated"
        })
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one session-identity matrix proves snapshot binding across project republication"
)]
fn proof_artifact_id_is_session_only_and_snapshot_bound() {
    fn publish_project(
        database: &mut HirDatabase,
        parsed: &ParsedSource,
        package: &CallablePackageId,
        path: &CanonicalModulePath,
        profile: &str,
    ) -> HirProject {
        let key = HirModuleKey::new(
            package.clone(),
            path.clone(),
            parsed.document().identity().clone(),
        );
        let world = ProjectSymbolWorldId::try_new(
            package.clone(),
            parsed.document().identity().id().clone(),
            profile,
        )
        .expect("symbol world");
        let revision = ProjectSymbolRevision::try_for_documents([parsed.document().identity()])
            .expect("symbol revision");
        let transaction = database
            .stage_proof_return_project(
                [LoweringRequest::try_new(key, parsed).expect("lowering request")],
                world,
                revision,
                [parsed.document().identity()],
                crate::lowering::HirLoweringControl::new(),
            )
            .expect("staged HIR project");
        let facts = HirProofReturnSemanticFactSet::try_new(
            Arc::clone(transaction.generation()),
            transaction.headers().cloned(),
            [],
        )
        .expect("fixture has no authored Proof returns");
        let mut outputs = transaction
            .publish_with_semantic_facts(database, facts)
            .expect("published HIR project");
        let hir = outputs.pop().expect("one fixture module").into_module();
        assert!(outputs.is_empty());
        let module =
            HirProjectModule::try_new(database, package, path, parsed.document().identity(), hir)
                .expect("root module binding");
        let mut builder = HirProjectBuilder::new(database, package.clone());
        builder.insert_module(module).expect("module insertion");
        builder.finish().expect("HIR project")
    }

    fn registered_artifact(
        project: &HirProject,
        document: &SourceDocument,
        profile: &str,
    ) -> (CallableDeclarationKey, ProofArtifactId) {
        let table =
            ProjectSymbolTable::link(project.view(), &declarations(document, Vec::new(), profile))
                .expect("proof symbol publication")
                .into_table();
        let symbol = table
            .callable_symbols()
            .find(|symbol| symbol.owner() == CallableDeclarationOwner::Proof)
            .expect("registered Proof");
        let declaration = symbol.declaration().clone();
        let CallableDeclarationKey::Existing(id) = &declaration else {
            panic!("authored top-level Proof uses the existing declaration key")
        };
        let artifact = table
            .proof_artifact(project.view(), id)
            .expect("snapshot-bound Proof artifact");
        (declaration, artifact)
    }

    let profile = "proof-artifact-snapshot-bound";
    let source_name = SourceName::path("src/proof-artifact.arcw");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://project-symbol-tests/proof-artifact")
                .expect("document id"),
            source_name.clone(),
            "proof stable() = ()\n",
        )
        .expect("source document"),
    );
    let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(source_name),
            Arc::clone(&document),
            ParseOptions::default(),
        )
        .expect("initial attached source");
    let package = CallablePackageId::try_new(PACKAGE).expect("package");
    let path = CanonicalModulePath::crate_root();
    let mut database = HirDatabase::try_new().expect("HIR database");
    let first_project = publish_project(&mut database, &initial, &package, &path, profile);
    let (first_declaration, first) = registered_artifact(&first_project, &document, profile);

    let body_start = document.text().rfind("()").expect("Proof body");
    let changed_source = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                document
                    .span(SourceRange::new(body_start, body_start + 2))
                    .expect("Proof body span"),
                "{ let value: Unit = (); value }",
            )],
            ParseOptions::default(),
        )
        .expect("changed attached source");
    let changed_project = publish_project(&mut database, &changed_source, &package, &path, profile);
    let (changed_declaration, changed) =
        registered_artifact(&changed_project, changed_source.document(), profile);

    assert_eq!(first_declaration, changed_declaration);
    assert_eq!(first.declaration(), changed.declaration());
    assert_ne!(first.snapshot(), changed.snapshot());
    assert_eq!(
        first.item(),
        changed.item(),
        "a body-only reparse preserves the source-backed Proof item identity",
    );
    assert_ne!(first, changed);
    assert_eq!(first.item().module(), first.snapshot().module());
    assert_eq!(changed.item().module(), changed.snapshot().module());

    let (foreign_document, foreign_project) = project("proof stable() = ()\n");
    let (foreign_declaration, foreign) = registered_artifact(
        &foreign_project,
        &foreign_document,
        "proof-artifact-foreign-session",
    );
    assert_eq!(first_declaration, foreign_declaration);
    assert_ne!(first, foreign, "Proof artifacts never cross HIR sessions");
}

#[test]
fn ordinary_projection_matches_callable_golden() {
    let (document, project) =
        project("pub fn alpha() -> Unit { () }\n#[fx]\nfn beta(value: i32) -> i32 { value }\n");
    let table = ProjectSymbolTable::link(
        project.view(),
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
fn final_hir_symbol_projection_preserves_the_resolved_family() {
    let source = concat!(
        "pub fn work() -> Unit { () }\n",
        "pub struct Record { value: i32 }\n",
    );
    let (document, project) = project(source);
    let table = ProjectSymbolTable::link(
        project.view(),
        &declarations(&document, Vec::new(), "final-hir-symbol-family"),
    )
    .expect("symbol families link")
    .into_table();
    let root = CanonicalModulePath::crate_root();
    let site = document
        .span(SourceRange::new(0, source.len()))
        .expect("reference site");

    assert!(matches!(
        table.resolve_hir_symbol_target(&root, &type_path("work"), site.clone()),
        Ok(ResolvedProjectSymbol::Callable(_))
    ));
    assert!(matches!(
        table.resolve_hir_symbol_target(&root, &type_path("Record"), site),
        Ok(ResolvedProjectSymbol::Nominal(_))
    ));
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
        project.view(),
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
            .resolve_hir_type_target(module, &type_path(spelling), reference_source.clone())
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
    assert_nominal_source_records(
        &project,
        model_source,
        &documents[1],
        local_record,
        choice,
        alias,
    );
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
    let (document, reserved_project) = project("struct Ref {\n    value: i32,\n}\n");
    let report = ProjectSymbolTable::link(
        reserved_project.view(),
        &declarations(&document, Vec::new(), "reserved-type-name"),
    )
    .expect_err("reserved built-in type names cannot be shadowed");
    assert!(matches!(
        report.diagnostics(),
        [ProjectSymbolLinkError::ReservedTypeName { module, name, source }]
            if module == &CanonicalModulePath::crate_root()
                && name == "Ref"
                && &document.text()[source.range().start()..source.range().end()] == "Ref"
    ));

    let (document, project) = project(concat!(
        "fn Widget() -> Unit { () }\n",
        "struct Widget {\n    value: i32,\n}\n",
    ));
    let report = ProjectSymbolTable::link(
        project.view(),
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
        project.view(),
        &empty_declarations(&documents, "typed-type-lookup-errors"),
    )
    .expect("ordinary same-spelling ambiguity remains a lookup result")
    .into_table();
    let root = CanonicalModulePath::crate_root();
    let source = documents[0]
        .span(SourceRange::new(0, 3))
        .expect("reference source");

    assert!(matches!(
        table.resolve_hir_type_target(&root, &type_path("work"), source.clone()),
        Err(ProjectTypeLookupError::WrongKind { actual, .. })
            if matches!(actual.target(), ProjectSymbolTargetId::Callable(_))
                && actual.declaration().is_some()
                && !actual.binding_sites().is_empty()
    ));
    assert!(matches!(
        table.resolve_hir_type_target(
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
        table.resolve_hir_type_target(&root, &type_path("Both"), source),
        Err(ProjectTypeLookupError::Ambiguous { candidates, .. })
            if candidates.len() == 2
                && candidates.windows(2).all(|pair| pair[0].target() < pair[1].target())
                && candidates.iter().all(|candidate| {
                    candidate.declaration().is_some() && !candidate.binding_sites().is_empty()
                })
    ));
}

#[test]
fn value_lookup_selects_callable_before_same_spelling_type() {
    let (documents, project) = project_modules(&[
        (
            "",
            concat!(
                "use crate.values.Item as Shared\n",
                "use crate.types.Item as Shared\n",
            ),
        ),
        ("values", "pub fn Item() -> Unit { () }\n"),
        ("types", "pub struct Item {\n    value: i32,\n}\n"),
    ]);
    let table = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "value-type-namespace-collision"),
    )
    .expect("cross-namespace collision remains a lookup decision")
    .into_table();
    let reference = SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "Shared")
        .expect("structured reference");
    let source = documents[0]
        .span(SourceRange::new(0, 3))
        .expect("reference source");

    assert!(matches!(
        table.resolve_value_target(
            &CanonicalModulePath::crate_root(),
            &reference,
            source.clone(),
        ),
        Ok(ProjectValueLookup::Present(callable))
            if callable.declaration().name() == "Item"
                && callable.declaration().module() == &module_path("values")
    ));
    assert!(matches!(
        table.resolve_hir_value_target(
            &CanonicalModulePath::crate_root(),
            &type_path("Shared"),
            source,
        ),
        Ok(ProjectValueLookup::Present(callable))
            if callable.declaration().name() == "Item"
                && callable.declaration().module() == &module_path("values")
    ));
}

#[test]
fn value_lookup_retains_inaccessible_callable_before_same_spelling_type() {
    let (documents, project) = project_modules(&[
        ("", "fn main() -> Unit { () }\n"),
        (
            "values",
            "pub use crate.types.Item\nfn Item() -> Unit { () }\n",
        ),
        ("types", "pub struct Item {\n    value: i32,\n}\n"),
    ]);
    let table = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "inaccessible-value-type-collision"),
    )
    .expect("cross-namespace collision remains a lookup decision")
    .into_table();
    let root = CanonicalModulePath::crate_root();
    let reference = symbol_path("crate.values.Item");
    let source = documents[0]
        .span(SourceRange::new(0, 3))
        .expect("reference source");

    assert!(matches!(
        table.resolve_value_target(&root, &reference, source),
        Err(ProjectValueLookupError::Inaccessible { candidates, .. })
            if matches!(candidates.as_ref(), [ProjectSymbolTargetId::Callable(_)])
    ));
}

#[test]
fn value_lookup_retains_ambiguous_and_inaccessible_callable_failures() {
    let (documents, project) = project_modules(&[
        (
            "",
            concat!(
                "use crate.left.run as selected\n",
                "use crate.right.run as selected\n",
            ),
        ),
        (
            "left",
            "pub fn run() -> Unit { () }\nfn hidden() -> Unit { () }\n",
        ),
        ("right", "pub fn run() -> Unit { () }\n"),
    ]);
    let table = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "typed-value-lookup-errors"),
    )
    .expect("value ambiguity remains a lookup result")
    .into_table();
    let root = CanonicalModulePath::crate_root();
    let source = documents[0]
        .span(SourceRange::new(0, 3))
        .expect("reference source");
    let ambiguous = SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "selected")
        .expect("ambiguous reference");
    let inaccessible = symbol_path("crate.left.hidden");

    assert!(matches!(
        table.resolve_value_target(&root, &ambiguous, source.clone()),
        Err(ProjectValueLookupError::Ambiguous { candidates, .. })
            if candidates.len() == 2
                && candidates.windows(2).all(|pair| pair[0] < pair[1])
                && candidates
                    .iter()
                    .all(|candidate| matches!(candidate, ProjectSymbolTargetId::Callable(_)))
    ));
    assert!(matches!(
        table.resolve_value_target(&root, &inaccessible, source),
        Err(ProjectValueLookupError::Inaccessible { candidates, .. })
            if matches!(candidates.as_ref(), [ProjectSymbolTargetId::Callable(_)])
    ));
}

#[test]
fn value_lookup_reports_nominal_and_unknown_paths_as_absent() {
    let (documents, project) = project_modules(&[
        ("", "fn anchor() -> Unit { () }\n"),
        ("types", "pub struct Item {\n    value: i32,\n}\n"),
    ]);
    let table = ProjectSymbolTable::link(
        project.view(),
        &empty_declarations(&documents, "absent-value-lookup"),
    )
    .expect("ordinary project")
    .into_table();
    let root = CanonicalModulePath::crate_root();
    let source = documents[0]
        .span(SourceRange::new(0, 2))
        .expect("reference source");
    let nominal = symbol_path("crate.types.Item");
    let unknown = SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "Missing")
        .expect("unknown reference");

    assert!(matches!(
        table.resolve_value_target(&root, &nominal, source.clone()),
        Ok(ProjectValueLookup::Absent)
    ));
    assert!(matches!(
        table.resolve_value_target(&root, &unknown, source),
        Ok(ProjectValueLookup::Absent)
    ));
}

#[test]
fn table_retains_source_identity_for_every_module() {
    let (documents, project) = project_modules(&[("", ""), ("empty", "")]);
    let table = ProjectSymbolTable::link(
        project.view(),
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
    let ordinary = ProjectSymbolTable::link(project.view(), &empty).expect("ordinary table");
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
    let extended =
        ProjectSymbolTable::link(project.view(), &with_character).expect("extended table");

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
    let link = ProjectSymbolTable::link(project.view(), &declarations).expect("linked externals");

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
    let table = ProjectSymbolTable::link(project.view(), &declarations)
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
    let report = ProjectSymbolTable::link(project.view(), &declarations)
        .expect_err("unknown imports are rejected during atomic publication");

    let [
        ProjectSymbolLinkError::UnknownImport {
            module,
            import,
            source,
        },
    ] = report.diagnostics()
    else {
        panic!(
            "one typed unknown-import diagnostic: {:#?}",
            report.diagnostics()
        )
    };
    assert_eq!(module, &CanonicalModulePath::crate_root());
    assert_eq!(import.to_string(), "crate.missing.symbol");
    assert_eq!(source.range(), SourceRange::new(0, 25));
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
    let link = ProjectSymbolTable::link(project.view(), &declarations)
        .expect("generated mandatory spellings are not authored aliases");
    assert_eq!(link.table().external_symbols().count(), 512);
}
