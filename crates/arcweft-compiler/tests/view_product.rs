use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use arcweft_compiler::project::{
    CompiledProject, ProjectCompilationContext, ProjectCompilationSession, ProjectCompileError,
    ProjectCompileStage, compile_project,
};
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::{env::TypeCheckEnv, registration::ProjectRegistrationFacts};
use arcweft_lang_syntax::{
    ast::module_path::{CanonicalModulePath, ModuleSegment},
    incremental::{ParsedSource, SyntaxDatabase},
    parser::ParseOptions,
};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::graph::ModuleDependency;
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{
    DiagnosticLabelStyle, SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId,
};
use arcweft_view::{ViewId, style::ViewStyleSheetId};

fn compile_attached_project(
    project: &ProjectSources,
    context: &ProjectCompilationContext,
) -> Result<CompiledProject, ProjectCompileError> {
    let mut syntax = SyntaxDatabase::try_new().expect("View test syntax database");
    let parsed_sources: BTreeMap<CanonicalModulePath, ParsedSource> = project
        .modules()
        .map(|source| {
            let parsed = syntax
                .parse_initial(
                    SourceSnapshotId::initial(source.document().display_name().clone()),
                    Arc::clone(source.document()),
                    ParseOptions::default(),
                )
                .expect("View test attached source");
            (source.module().clone(), parsed)
        })
        .collect();
    let mut compiler = ProjectCompilationSession::try_new().expect("View test HIR database");
    compile_project(&mut compiler, project, &parsed_sources, context)
}

#[test]
fn compiler_lowers_every_typed_view_into_one_validated_product() {
    let source = r#"
view First() {
  Text("first")
}

view Second() {
  Text("second")
}

style Primary {
  Button { color = rgba(10, 20, 30, 255) }
}
"#;
    let fixture = project_view_fixture(source, "arcweft-test://compiler-view-product");
    let compiled = fixture.compile().expect("validated View product");
    let product = compiled.view_product();

    let program = product.product().program().expect("program");
    let first = ViewId::try_new("view.First").expect("first View ID");
    let second = ViewId::try_new("view.Second").expect("second View ID");
    assert!(program.definition(&first).is_some());
    assert!(program.definition(&second).is_some());
    assert!(
        program.definition(&ViewId::standard_dialogue()).is_some(),
        "the standard View is linked by the compiler"
    );
    assert_eq!(
        product.view_source(&first).expect("first source").source(),
        fixture.document.identity()
    );
    let style_id = ViewStyleSheetId::try_new("style.Primary").expect("Style ID");
    assert_eq!(
        product
            .style_source(&style_id)
            .expect("Style source")
            .source(),
        fixture.document.identity()
    );
    assert_eq!(
        product
            .view_source(&ViewId::standard_dialogue())
            .expect("standard View source")
            .source()
            .id()
            .as_str(),
        arcweft_bundle::standard_view::DIALOGUE_VIEW_SOURCE_ID
    );
    assert_ne!(
        product.authored_source_revision(),
        product.product_source_revision(),
        "engine-generated standard View/Style sources extend the complete product source set"
    );
    assert_eq!(
        product.resource_type_registry_digest(),
        ResourceTypeRegistry::empty().digest()
    );
}

#[test]
fn compiler_lowers_project_views_in_canonical_module_and_source_order() {
    let (project, context, root_document, a_document, z_document) =
        canonical_view_project_fixture();
    let compiled =
        compile_attached_project(&project, &context).expect("canonical multi-module View project");
    let program = compiled
        .view_product()
        .product()
        .program()
        .expect("project View program");
    let standard = ViewId::standard_dialogue();
    let authored = program
        .definitions()
        .filter(|definition| definition.public_id.view_id() != &standard)
        .collect::<Vec<_>>();
    assert_eq!(
        authored
            .iter()
            .map(|definition| definition.public_id.view_id().as_str())
            .collect::<Vec<_>>(),
        ["view.RootFirst", "view.RootSecond", "view.a.A", "view.z.Z",]
    );
    assert_eq!(program.program_id().as_str(), "view.program.view.RootFirst");
    for pair in authored.windows(2) {
        assert!(
            pair[0].body.end_instruction <= pair[1].body.start_instruction,
            "View instruction spans must advance across module boundaries"
        );
    }

    for (view, document) in [
        ("view.RootFirst", &root_document),
        ("view.a.A", &a_document),
        ("view.z.Z", &z_document),
    ] {
        let view = ViewId::try_new(view).expect("View ID");
        let span = compiled
            .view_product()
            .view_source(&view)
            .expect("module-bound View source");
        assert_eq!(span.source(), document.identity());
        assert!(source_text(document, span).starts_with("pub view"));
    }
}

#[test]
fn compiler_rejects_nested_view_recovery_before_product_acceptance() {
    for source in [
        "view Broken() {\n  Text(@@@)\n}\n",
        "view Broken() {\n  Panel(width = @@@)\n}\n",
        "view Broken() {\n  Scroll(axis = @@@) { Text(\"x\") }\n}\n",
        "view Broken() {\n  if @@@ { Text(\"x\") }\n}\n",
        "view Broken(value: i32) {\n  match value {\n    ??? => Text(\"x\")\n  }\n}\n",
        "view Broken(value: i32) {\n  match value {\n    .MissingArrow Text(\"x\")\n  }\n}\n",
        "view Broken() {\n  Button(\"x\").unknown_modifier(@@@)\n}\n",
        "view Broken() {\n  Button(\"x\").on_focus { wait(@@@) }\n}\n",
        "view Broken(items: Vec<Item>) {\n  for item in items key item.id {\n    Text(\"x\")\n  }\n}\n",
        "view Broken() {\n  Button(\"x\").nav(sideways: auto)\n}\n",
        "view Broken() {\n  Button(\"x\").nav(right: nowhere)\n}\n",
    ] {
        let fixture = project_view_fixture(source, "arcweft-test://compiler-view-recovery");
        let error = fixture
            .compile()
            .expect_err("malformed View must not enter an accepted product");
        let diagnostics = error.diagnostics();
        for diagnostic in diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.stage() == ProjectCompileStage::Parse)
        {
            assert!(diagnostic.syntax_diagnostic().is_some());
            assert!(diagnostic.diagnostic().labels().iter().any(|label| {
                label.style() == DiagnosticLabelStyle::Primary
                    && label.span().validate_for(&fixture.document).is_ok()
            }));
        }
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.stage() == ProjectCompileStage::Readiness),
            "a recovered HIR module must fail execution readiness: {source}\n{error:?}"
        );
    }
}

#[test]
fn compiler_rejects_well_formed_view_values_without_a_typed_runtime_contract() {
    let cases = [
        (
            "view Good() { Text(\"ok\") }\n\nview Broken(label: String) {\n  Button(label)\n}\n",
            "compiler.view.literal_text",
        ),
        (
            "view Good() { Text(\"ok\") }\n\nview Broken(enabled: bool) {\n  Button(\"x\", enabled = enabled)\n}\n",
            "compiler.view.static_boolean",
        ),
    ];

    for (source, _previous_leaf_diagnostic) in cases {
        let fixture = project_view_fixture(source, "arcweft-test://compiler-view-typed-rejection");
        let error = fixture
            .compile()
            .expect_err("unimplemented runtime semantics must not be accepted or defaulted");
        assert_eq!(
            error.diagnostics().len(),
            1,
            "unexpected diagnostics: {error:?}"
        );
        let diagnostic = &error.diagnostics()[0];
        assert_eq!(
            diagnostic.stage(),
            ProjectCompileStage::ViewLower,
            "unexpected rejection stage for {source}: {error:?}"
        );
        assert_eq!(
            diagnostic
                .diagnostic()
                .code()
                .map(arcweft_source::DiagnosticCode::as_str),
            Some("compiler.view.lower"),
            "unexpected structured rejection for {source}: {error:?}"
        );
        assert!(diagnostic.source().is_none());
        assert!(diagnostic.diagnostic().labels().is_empty());
    }
}

#[test]
fn compiler_retains_authored_owner_for_signature_and_default_failures() {
    let cases = [
        (
            "view Good() { Text(\"ok\") }\n\nview Broken(value: String = \"x\") { Text(\"x\") }\n",
            "compiler.view.lower",
        ),
        (
            "fn make_default() -> i32 { 1 }\n\nview Good() { Text(\"ok\") }\n\nview Broken(value: i32 = make_default()) { Text(\"x\") }\n",
            "compiler.view.value_program",
        ),
    ];

    for (source, _previous_leaf_diagnostic) in cases {
        let fixture = project_view_fixture(
            source,
            "arcweft-test://compiler-view-schema-default-rejection",
        );
        let error = fixture
            .compile()
            .expect_err("schema/default rejection must retain its authored owner");
        assert_eq!(
            error.diagnostics().len(),
            1,
            "unexpected diagnostics: {error:?}"
        );
        let diagnostic = &error.diagnostics()[0];
        assert_eq!(diagnostic.stage(), ProjectCompileStage::ViewLower);
        assert_eq!(
            diagnostic
                .diagnostic()
                .code()
                .map(arcweft_source::DiagnosticCode::as_str),
            Some("compiler.view.lower")
        );
        assert!(diagnostic.source().is_none());
        assert!(diagnostic.diagnostic().labels().is_empty());
    }
}

#[test]
fn compiler_rejects_every_unknown_text_control_and_scroll_policy_symbol() {
    for (_policy, value, authored) in unknown_policy_cases() {
        let source = format!("view Broken() {{\n  {authored}\n}}\n");
        let fixture = project_view_fixture(
            &source,
            &format!("arcweft-test://compiler-view-unknown-{value}"),
        );
        let error = fixture
            .compile()
            .expect_err("an explicitly authored typo must not lower as a default policy");
        assert!(matches!(
            error.diagnostics()[0].stage(),
            ProjectCompileStage::Parse | ProjectCompileStage::TypeCheck
        ));
        let diagnostic = error.diagnostics()[0].diagnostic();
        assert!(diagnostic.code().is_some());
    }
}

fn unknown_policy_cases() -> [(&'static str, &'static str, &'static str); 12] {
    [
        (
            "text input purpose",
            "serch",
            "TextField(\"value\")\n    .purpose(\"serch\")",
        ),
        (
            "enter-key hint",
            "snd",
            "TextField(\"value\")\n    .enter_key(\"snd\")",
        ),
        (
            "text selection",
            "enabeld",
            "TextField(\"value\", selection = enabeld)",
        ),
        (
            "text shortcut",
            "enabeld",
            "TextField(\"value\", shortcuts = enabeld)",
        ),
        (
            "Tab-key",
            "insert_tba",
            "TextField(\"value\", tab = insert_tba)",
        ),
        (
            "vertical navigation",
            "visaul",
            "TextField(\"value\", vertical_navigation = visaul)",
        ),
        (
            "secure input",
            "pasword",
            "SecureField(\"value\", secure_policy = pasword)",
        ),
        (
            "scroll overflow",
            "scrol",
            "Scroll(overflow = \"scrol\") {\n    Text(\"x\")\n  }",
        ),
        (
            "scroll axis",
            "vertcial",
            "Scroll(axis = \"vertcial\") {\n    Text(\"x\")\n  }",
        ),
        (
            "scroll indicators",
            "visble",
            "Scroll(indicators = \"visble\") {\n    Text(\"x\")\n  }",
        ),
        (
            "scroll overscroll",
            "elstic",
            "Scroll(overscroll = \"elstic\") {\n    Text(\"x\")\n  }",
        ),
        (
            "scroll focus",
            "nerest",
            "Scroll(auto_scroll_focus = \"nerest\") {\n    Text(\"x\")\n  }",
        ),
    ]
}

struct ProjectViewFixture {
    project: ProjectSources,
    document: Arc<SourceDocument>,
    context: ProjectCompilationContext,
}

impl ProjectViewFixture {
    fn compile(&self) -> Result<CompiledProject, ProjectCompileError> {
        compile_attached_project(&self.project, &self.context)
    }
}

fn project_view_fixture(source: &str, source_id: &str) -> ProjectViewFixture {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(source_id).expect("source ID"),
            SourceName::path("main.arcw"),
            source,
        )
        .expect("source document"),
    );
    let module = CanonicalModulePath::crate_root();
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        PackageSpec {
            id: PackageId::new("local.arcweft.view-diagnostic").expect("package ID"),
            version: PackageVersion::new("0.0.0").expect("package version"),
        },
        BuildSpec::default(),
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://compile-project-view-manifest")
                    .expect("manifest source ID"),
                SourceName::path("arcw.toml"),
                "",
            )
            .expect("manifest document"),
        ),
        vec![ProjectSourceFile::new(
            module,
            PathBuf::from("main.arcw"),
            Arc::clone(&document),
            [],
        )],
    )
    .expect("project sources");
    let package =
        CallablePackageId::try_new(project.package().id.as_str()).expect("callable package ID");
    let world = ProjectSymbolWorldId::try_new(
        package,
        document.identity().id().clone(),
        "compiler-view-product-test",
    )
    .expect("symbol world");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(ResourceTypeRegistry::empty()),
        None,
        None,
    );
    ProjectViewFixture {
        project,
        document,
        context,
    }
}

fn canonical_view_project_fixture() -> (
    ProjectSources,
    ProjectCompilationContext,
    Arc<SourceDocument>,
    Arc<SourceDocument>,
    Arc<SourceDocument>,
) {
    let a_module =
        CanonicalModulePath::from_segments(
            [ModuleSegment::new("a").expect("valid module segment")],
        );
    let z_module =
        CanonicalModulePath::from_segments(
            [ModuleSegment::new("z").expect("valid module segment")],
        );
    let root_document = canonical_view_document(
        "arcweft-test://canonical-view/root",
        "src/main.arcw",
        "pub view RootFirst() { Text(\"root first\") }\n\
         pub view RootSecond() { Text(\"root second\") }\n",
    );
    let a_document = canonical_view_document(
        "arcweft-test://canonical-view/a",
        "src/a.arcw",
        "mod a\n\npub view A() { Text(\"a\") }\n",
    );
    let z_document = canonical_view_document(
        "arcweft-test://canonical-view/z",
        "src/z.arcw",
        "mod z\n\npub view Z() { Text(\"z\") }\n",
    );
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        PackageSpec {
            id: PackageId::new("local.arcweft.canonical-view").expect("package ID"),
            version: PackageVersion::new("0.0.0").expect("package version"),
        },
        BuildSpec::default(),
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://canonical-view/manifest")
                    .expect("manifest source ID"),
                SourceName::path("arcw.toml"),
                "",
            )
            .expect("manifest document"),
        ),
        [
            ProjectSourceFile::new(
                z_module.clone(),
                PathBuf::from("src/z.arcw"),
                Arc::clone(&z_document),
                [],
            ),
            ProjectSourceFile::new(
                CanonicalModulePath::crate_root(),
                PathBuf::from("src/main.arcw"),
                Arc::clone(&root_document),
                [
                    ModuleDependency::new(z_module),
                    ModuleDependency::new(a_module.clone()),
                ],
            ),
            ProjectSourceFile::new(
                a_module,
                PathBuf::from("src/a.arcw"),
                Arc::clone(&a_document),
                [],
            ),
        ],
    )
    .expect("canonical View project sources");
    let package =
        CallablePackageId::try_new(project.package().id.as_str()).expect("callable package ID");
    let world = ProjectSymbolWorldId::try_new(
        package,
        root_document.identity().id().clone(),
        "canonical-view-project-test",
    )
    .expect("symbol world");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        project
            .modules()
            .map(|source| Arc::clone(source.document()))
            .collect::<Vec<_>>(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(ResourceTypeRegistry::empty()),
        None,
        None,
    );
    (project, context, root_document, a_document, z_document)
}

fn canonical_view_document(id: &str, path: &str, source: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).expect("source ID"),
            SourceName::path(path),
            source,
        )
        .expect("canonical View source document"),
    )
}

fn source_text<'a>(document: &'a SourceDocument, span: &arcweft_source::SourceSpan) -> &'a str {
    span.validate_for(document)
        .expect("span belongs to fixture");
    let range = span.range();
    &document.text()[range.start()..range.end()]
}
