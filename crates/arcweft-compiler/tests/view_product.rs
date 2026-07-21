use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use arcweft_bundle::{
    BundleImageObject, BundleImageObjectAlignment, BundleImageObjectBounds, BundleImageObjectFit,
    BundleImageObjectPlayback, BundleImageObjectTransform,
};
use arcweft_compiler::{
    hir::validate_hir_with_env,
    image::lower_project_images,
    project::{ProjectCompilationContext, ProjectCompileStage, compile_project},
    style::{CompiledViewStyleArtifact, lower_project_view_styles, lower_source_view_styles},
    view::{ViewProjectLowerError, ViewProjectLowerer, ViewSidecarError, ViewValueCompileError},
};
use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    model::HirModule,
    project::{HirProject, HirProjectModule},
    symbol::{CallablePackageId, ProjectSymbolWorldId},
};
use arcweft_lang_sema::{
    check::TypeCheckReport, env::TypeCheckEnv, registration::ProjectRegistrationFacts,
};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath,
    parser::{ParseOptions, parse_document_with_source},
};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_runtime_plan::flow::RuntimePlanLowerOptions;
use arcweft_source::{
    DiagnosticLabelStyle, SourceDocument, SourceDocumentId, SourceName, SourceSpan,
};
use arcweft_view::{ViewId, style::ViewStyleSheetId};

struct SourceViewFixture {
    document: Arc<SourceDocument>,
    hir: HirModule,
    typecheck: TypeCheckReport,
    style: CompiledViewStyleArtifact,
}

fn source_view_fixture(source: &str, id: &str) -> SourceViewFixture {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).expect("source ID"),
            SourceName::path("views.arcw"),
            source,
        )
        .expect("source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    assert_eq!(parsed.errors(), &[], "source should parse:\n{source}");
    let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("HIR");
    let typecheck = validate_hir_with_env(&hir, &TypeCheckEnv::standard()).expect("typecheck");
    let style =
        lower_source_view_styles(&hir, &typecheck.style_catalog, &document).expect("Style product");
    SourceViewFixture {
        document,
        hir,
        typecheck,
        style,
    }
}

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
    let fixture = source_view_fixture(source, "arcweft-test://compiler-view-product");
    let resource_types = ResourceTypeRegistry::empty();

    let compiled = ViewProjectLowerer::for_source(
        &fixture.hir,
        &fixture.typecheck,
        &fixture.style,
        &fixture.document,
        &[],
        &[],
        &resource_types,
    )
    .and_then(ViewProjectLowerer::lower)
    .expect("validated View product");

    let program = compiled.product().program().expect("program");
    let first = ViewId::try_new("view.First").expect("first View ID");
    let second = ViewId::try_new("view.Second").expect("second View ID");
    assert!(program.definition(&first).is_some());
    assert!(program.definition(&second).is_some());
    assert!(
        program.definition(&ViewId::standard_dialogue()).is_some(),
        "the standard View is linked by the compiler"
    );
    assert_eq!(
        compiled.view_source(&first).expect("first source").source(),
        fixture.document.identity()
    );
    let style_id = ViewStyleSheetId::try_new("style.Primary").expect("Style ID");
    assert_eq!(
        compiled
            .style_source(&style_id)
            .expect("Style source")
            .source(),
        fixture.document.identity()
    );
    assert_eq!(
        compiled
            .view_source(&ViewId::standard_dialogue())
            .expect("standard View source")
            .source()
            .id()
            .as_str(),
        arcweft_bundle::standard_view::DIALOGUE_VIEW_SOURCE_ID
    );
    assert_ne!(
        compiled.authored_source_revision(),
        compiled.product_source_revision(),
        "engine-generated standard View/Style sources extend the complete product source set"
    );
    assert_eq!(
        compiled.resource_type_registry_digest(),
        resource_types.digest()
    );
}

#[test]
fn standalone_view_lowering_rejects_image_collisions_and_detached_sources() {
    let source = r#"
view First() {
  Text("first")
  Image(@image.view.First.0)
}
"#;
    let fixture = source_view_fixture(source, "arcweft-test://compiler-view-collision");
    let resource_types = ResourceTypeRegistry::empty();
    let source_images = vec![BundleImageObject {
        id: "image.view.First.0".to_owned(),
        asset: "asset.test".to_owned(),
        target: None,
        layer: None,
        view: None,
        containing_scroll_region: None,
        bounds: BundleImageObjectBounds::from_px(0, 0, 16, 16),
        placement: None,
        fit: BundleImageObjectFit::default(),
        alignment: BundleImageObjectAlignment::default(),
        playback: BundleImageObjectPlayback::default(),
        transform: BundleImageObjectTransform::default(),
        depth_milli: 0,
        opacity_milli: 1_000,
        actions: Vec::new(),
        params: BTreeMap::new(),
        proxies: Vec::new(),
        visible: true,
    }];
    let collision = ViewProjectLowerer::for_source(
        &fixture.hir,
        &fixture.typecheck,
        &fixture.style,
        &fixture.document,
        &source_images,
        &[],
        &resource_types,
    )
    .and_then(ViewProjectLowerer::lower);
    assert!(
        matches!(
            &collision,
            Err(ViewProjectLowerError::DuplicateImageObject {
                image,
                top_level: None,
                ..
            }) if image == "image.view.First.0"
        ),
        "unexpected collision result: {collision:?}"
    );

    let detached = SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-test://detached-view-product").expect("source ID"),
        SourceName::path("detached.arcw"),
        source,
    )
    .expect("detached document");
    assert!(matches!(
        ViewProjectLowerer::for_source(
            &fixture.hir,
            &fixture.typecheck,
            &fixture.style,
            &detached,
            &[],
            &[],
            &resource_types,
        ),
        Err(ViewProjectLowerError::HirSourceMismatch { .. })
    ));
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
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://compiler-view-recovery")
                    .expect("source ID"),
                SourceName::path("recovery.arcw"),
                source,
            )
            .expect("source document"),
        );
        let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
        let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("HIR");
        let typecheck =
            validate_hir_with_env(&hir, &TypeCheckEnv::standard()).expect("typecheck report");
        let style = lower_source_view_styles(&hir, &typecheck.style_catalog, &document)
            .expect("Style product");
        let resource_types = ResourceTypeRegistry::empty();
        let result = ViewProjectLowerer::for_source(
            &hir,
            &typecheck,
            &style,
            &document,
            &[],
            &[],
            &resource_types,
        )
        .and_then(ViewProjectLowerer::lower);
        assert!(
            matches!(
                result,
                Err(ViewProjectLowerError::Lower(
                    ViewSidecarError::RecoveredViewSyntax { .. }
                ))
            ),
            "malformed View entered or bypassed the structured compiler boundary: {source}"
        );
    }
}

#[test]
fn compiler_rejects_well_formed_view_values_without_a_typed_runtime_contract() {
    let cases = [
        (
            "view Broken(label: String) {\n  Button(label)\n}\n",
            ViewSidecarError::UnsupportedLiteralText {
                context: "button label",
            },
        ),
        (
            "view Broken(enabled: bool) {\n  Button(\"x\", enabled = enabled)\n}\n",
            ViewSidecarError::UnsupportedStaticBoolean {
                context: "button enabled policy",
            },
        ),
        (
            "view Broken(width: i32) {\n  Panel(width = width) {\n    Text(\"x\")\n  }\n}\n",
            ViewSidecarError::UnsupportedLayoutValue {
                property: "width".to_owned(),
            },
        ),
        (
            r#"
pub action feedback.focus(value: String)

view Broken() {
  Button("x")
    .on_focus {
      action.invoke(@action:.feedback.focus, value = "focused")
    }
}
"#,
            ViewSidecarError::UnsupportedEventHandler {
                event: "focus".to_owned(),
            },
        ),
        (
            "view Broken() {\n  AwaitView(load_avatar(user)) {\n    pending _ => Text(\"Loading\")\n  }\n}\n",
            ViewSidecarError::ValueProgram(ViewValueCompileError::UnsupportedAwaitSource),
        ),
    ];

    for (source, expected) in cases {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://compiler-view-typed-rejection")
                    .expect("source ID"),
                SourceName::path("typed-rejection.arcw"),
                source,
            )
            .expect("source document"),
        );
        let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
        assert_eq!(parsed.errors(), &[], "source should be structurally valid");
        let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("HIR");
        let typecheck =
            validate_hir_with_env(&hir, &TypeCheckEnv::standard()).expect("typecheck report");
        let style = lower_source_view_styles(&hir, &typecheck.style_catalog, &document)
            .expect("Style product");
        let resource_types = ResourceTypeRegistry::empty();
        let error = ViewProjectLowerer::for_source(
            &hir,
            &typecheck,
            &style,
            &document,
            &[],
            &[],
            &resource_types,
        )
        .and_then(ViewProjectLowerer::lower)
        .expect_err("unimplemented runtime semantics must not be accepted or defaulted");
        let ViewProjectLowerError::Lower(actual) = error else {
            panic!("unexpected typed rejection for {source}: {error:?}");
        };
        assert_eq!(actual, expected, "unexpected typed rejection for {source}");
    }
}

#[test]
fn compiler_rejects_every_unknown_text_control_and_scroll_policy_symbol() {
    for (policy, value, authored) in unknown_policy_cases() {
        let source = format!("view Broken() {{\n  {authored}\n}}\n");
        let fixture = source_view_fixture(
            &source,
            &format!("arcweft-test://compiler-view-unknown-{value}"),
        );
        let resource_types = ResourceTypeRegistry::empty();
        let error = ViewProjectLowerer::for_source(
            &fixture.hir,
            &fixture.typecheck,
            &fixture.style,
            &fixture.document,
            &[],
            &[],
            &resource_types,
        )
        .and_then(ViewProjectLowerer::lower)
        .expect_err("an explicitly authored typo must not lower as a default policy");
        let diagnostic = error.diagnostic();
        assert_eq!(diagnostic.labels().len(), 1);
        diagnostic.labels()[0]
            .span()
            .validate_for(&fixture.document)
            .expect("the rejection remains attached to its authored View document");
        let ViewProjectLowerError::AuthoredViewLower {
            view,
            error:
                ViewSidecarError::UnknownPolicySymbol {
                    view: error_view,
                    policy: actual_policy,
                    value: actual_value,
                },
            ..
        } = error
        else {
            panic!("unexpected policy rejection for {authored}: {error:?}");
        };
        assert_eq!(view.as_str(), "view.Broken");
        assert_eq!(error_view, "view.Broken");
        assert_eq!(actual_policy, policy);
        assert_eq!(actual_value, value);
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

fn project_view_error(source: &str) -> (Arc<SourceDocument>, ViewProjectLowerError) {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler-view-image-collision")
                .expect("source ID"),
            SourceName::path("main.arcw"),
            source,
        )
        .expect("source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    assert_eq!(parsed.errors(), &[]);
    let module = CanonicalModulePath::crate_root();
    let lowered = lower_document_to_hir(&document, parsed.typed_tree()).expect("HIR");
    let hir_project = HirProject::new(
        "local.arcweft.view-image-collision",
        vec![
            HirProjectModule::try_new(module.clone(), document.identity().clone(), lowered)
                .expect("HIR module"),
        ],
    )
    .expect("HIR project");
    let linked_hir = hir_project.linked_module();
    let typecheck =
        validate_hir_with_env(&linked_hir, &TypeCheckEnv::standard()).expect("typecheck");
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        PackageSpec {
            id: PackageId::new("local.arcweft.view-image-collision").expect("package ID"),
            version: PackageVersion::new("0.0.0").expect("package version"),
        },
        BuildSpec::default(),
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://compiler-view-image-manifest")
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
    let style = lower_project_view_styles(
        &hir_project,
        &linked_hir,
        &typecheck.style_catalog,
        &project,
    )
    .expect("Style product");
    let images = lower_project_images(&hir_project, &project).expect("image catalog");
    let resource_types = ResourceTypeRegistry::empty();
    let error = ViewProjectLowerer::for_project(
        &hir_project,
        &linked_hir,
        &typecheck,
        &style,
        &project,
        &images,
        &[],
        &resource_types,
    )
    .and_then(ViewProjectLowerer::lower)
    .expect_err("top-level and View-generated image identities must be disjoint");
    (document, error)
}

#[test]
fn project_image_collision_retains_both_exact_declaration_owners() {
    let (document, error) = project_view_error(VIEW_IMAGE_COLLISION_SOURCE);
    let diagnostic = error.diagnostic();
    assert_eq!(
        diagnostic
            .code()
            .map(arcweft_source::DiagnosticCode::as_str),
        Some("compiler.view.duplicate_image_object")
    );
    assert_eq!(diagnostic.labels().len(), 2);
    assert_eq!(
        diagnostic.labels()[0].style(),
        DiagnosticLabelStyle::Primary
    );
    assert_eq!(
        diagnostic.labels()[1].style(),
        DiagnosticLabelStyle::Secondary
    );
    let ViewProjectLowerError::DuplicateImageObject {
        image,
        view,
        top_level: Some(top_level),
        generated,
    } = error
    else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(image, "image.view.First.0");
    assert_eq!(view, ViewId::try_new("view.First").expect("View ID"));
    assert!(source_text(&document, &top_level).starts_with("pub image @image.view.First.0"));
    assert!(source_text(&document, &generated).starts_with("view First()"));
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
    let (project, document, context) = compile_project_collision_fixture();
    let error = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
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
        document.identity()
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
            .validate_for(&document)
            .expect("both collision labels stay attached to the project source");
    }
}

fn compile_project_collision_fixture() -> (
    ProjectSources,
    Arc<SourceDocument>,
    ProjectCompilationContext,
) {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compile-project-view-diagnostic")
                .expect("source ID"),
            SourceName::path("main.arcw"),
            VIEW_IMAGE_COLLISION_SOURCE,
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
        "compiler-view-diagnostic-test",
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
    (project, document, context)
}

fn source_text<'a>(document: &'a SourceDocument, span: &SourceSpan) -> &'a str {
    span.validate_for(document)
        .expect("span belongs to fixture");
    let range = span.range();
    &document.text()[range.start()..range.end()]
}
