use std::{path::PathBuf, sync::Arc};

use arcweft_compiler::project::{
    CompiledProject, ProjectCompilationContext, ProjectCompileError, ProjectCompileStage,
    compile_project,
};
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::{env::TypeCheckEnv, registration::ProjectRegistrationFacts};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_runtime_plan::flow::RuntimePlanLowerOptions;
use arcweft_source::{
    DiagnosticLabelStyle, SourceDocument, SourceDocumentId, SourceName, SourceRange,
};
use arcweft_view::{ViewId, style::ViewStyleSheetId};

#[test]
fn compiler_lowers_every_typed_view_into_one_validated_product() {
    let source = r#"
view First() {
  Text("first")
  Image(@image.view.First.0)
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
        "view Broken() {\n  Button(\"x\").nav(@button:.next)\n}\n",
    ] {
        let fixture = project_view_fixture(source, "arcweft-test://compiler-view-recovery");
        let error = fixture
            .compile()
            .expect_err("malformed View must not enter an accepted product");
        assert!(
            error
                .diagnostics()
                .iter()
                .all(|diagnostic| match diagnostic.stage() {
                    ProjectCompileStage::Parse => diagnostic.parse_error().is_some(),
                    ProjectCompileStage::ViewLower => {
                        diagnostic
                            .diagnostic()
                            .code()
                            .is_some_and(|code| code.as_str() == "compiler.view.recovered_syntax")
                            && diagnostic.diagnostic().labels().iter().any(|label| {
                                label.style() == DiagnosticLabelStyle::Primary
                                    && label.span().validate_for(&fixture.document).is_ok()
                            })
                    }
                    _ => false,
                }),
            "malformed View bypassed ordinary parser rejection: {source}\n{error:?}"
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
        (
            "view Good() { Text(\"ok\") }\n\nview Broken(width: i32) {\n  Panel(width = width) {\n    Text(\"x\")\n  }\n}\n",
            "compiler.view.layout_value",
        ),
        (
            r#"
pub action feedback.focus(value: String)

view Good() { Text("ok") }

view Broken() {
  Button("x")
    .on_focus {
      action.invoke(@action:.feedback.focus, value = "focused")
    }
}
"#,
            "compiler.view.event_handler",
        ),
        (
            "view Good() { Text(\"ok\") }\n\nview Broken() {\n  AwaitView(load_avatar(user)) {\n    pending _ => Text(\"Loading\")\n  }\n}\n",
            "compiler.view.value_program",
        ),
    ];

    for (source, expected) in cases {
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
            Some(expected),
            "unexpected structured rejection for {source}: {error:?}"
        );
        assert_eq!(
            diagnostic
                .source()
                .expect("authored View rejection source")
                .document()
                .identity(),
            fixture.document.identity()
        );
        let labels = diagnostic.diagnostic().labels();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].style(), DiagnosticLabelStyle::Primary);
        assert_eq!(
            labels[0].message(),
            Some("this authored View contains the rejected value")
        );
        assert_eq!(labels[0].span().source(), fixture.document.identity());
        assert_eq!(labels[0].span().range(), authored_broken_view_range(source));
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

    for (source, expected_code) in cases {
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
            Some(expected_code)
        );
        assert_eq!(
            diagnostic
                .source()
                .expect("authored View rejection source")
                .document()
                .identity(),
            fixture.document.identity()
        );
        let labels = diagnostic.diagnostic().labels();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].style(), DiagnosticLabelStyle::Primary);
        assert_eq!(
            labels[0].message(),
            Some("this authored View contains the rejected value")
        );
        assert_eq!(labels[0].span().source(), fixture.document.identity());
        assert_eq!(labels[0].span().range(), authored_broken_view_range(source));
    }
}

fn authored_broken_view_range(source: &str) -> SourceRange {
    SourceRange::new(
        source.find("view Broken").expect("Broken View start"),
        source.rfind('}').expect("Broken View end") + 1,
    )
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
        assert_eq!(
            error.diagnostics()[0].stage(),
            ProjectCompileStage::ViewLower
        );
        let diagnostic = error.diagnostics()[0].diagnostic();
        assert_eq!(diagnostic.labels().len(), 1);
        diagnostic.labels()[0]
            .span()
            .validate_for(&fixture.document)
            .expect("the rejection remains attached to its authored View document");
        assert_eq!(
            diagnostic
                .code()
                .map(arcweft_source::DiagnosticCode::as_str),
            Some("compiler.view.policy_symbol")
        );
        assert_eq!(
            diagnostic.labels()[0].style(),
            DiagnosticLabelStyle::Primary
        );
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

const VIEW_IMAGE_COLLISION_SOURCE: &str = r"
pub image @image.poster {
  asset = @asset.poster
  x = 0px
  y = 0px
  width = 16px
  height = 16px
}

pub image @image.view.First.0 {
  asset = @asset.collision
  x = 0px
  y = 0px
  width = 16px
  height = 16px
}

view First() {
  Image(@image.poster)
}
";

#[test]
fn compile_project_retains_view_diagnostic_source_and_both_collision_spans() {
    let fixture = project_view_fixture(
        VIEW_IMAGE_COLLISION_SOURCE,
        "arcweft-test://compile-project-view-diagnostic",
    );
    let error = fixture
        .compile()
        .expect_err("the image identity collision must fail in View lowering");
    assert_eq!(error.diagnostics().len(), 1);
    let diagnostic = &error.diagnostics()[0];
    assert_eq!(diagnostic.stage(), ProjectCompileStage::ViewLower);
    assert_eq!(
        diagnostic
            .source()
            .expect("View diagnostic source")
            .document()
            .identity(),
        fixture.document.identity()
    );
    assert_eq!(
        diagnostic
            .diagnostic()
            .code()
            .map(arcweft_source::DiagnosticCode::as_str),
        Some("compiler.view.duplicate_image_object")
    );
    assert_eq!(diagnostic.diagnostic().labels().len(), 2);
    assert_eq!(
        diagnostic.diagnostic().labels()[0].style(),
        DiagnosticLabelStyle::Primary
    );
    assert_eq!(
        diagnostic.diagnostic().labels()[1].style(),
        DiagnosticLabelStyle::Secondary
    );
    for label in diagnostic.diagnostic().labels() {
        label
            .span()
            .validate_for(&fixture.document)
            .expect("both collision labels stay attached to the project source");
    }
    let labels = diagnostic.diagnostic().labels();
    assert!(source_text(&fixture.document, labels[0].span()).starts_with("view First()"));
    assert!(
        source_text(&fixture.document, labels[1].span())
            .starts_with("pub image @image.view.First.0")
    );
}

struct ProjectViewFixture {
    project: ProjectSources,
    document: Arc<SourceDocument>,
    context: ProjectCompilationContext,
}

impl ProjectViewFixture {
    fn compile(&self) -> Result<CompiledProject, ProjectCompileError> {
        compile_project(
            &self.project,
            &self.context,
            &RuntimePlanLowerOptions::default(),
        )
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
    )
    .expect("registration facts");
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(ResourceTypeRegistry::empty()),
        None,
        None,
        Vec::new(),
    );
    ProjectViewFixture {
        project,
        document,
        context,
    }
}

fn source_text<'a>(document: &'a SourceDocument, span: &arcweft_source::SourceSpan) -> &'a str {
    span.validate_for(document)
        .expect("span belongs to fixture");
    let range = span.range();
    &document.text()[range.start()..range.end()]
}
