use std::{path::PathBuf, sync::Arc};

use arcweft_bundle::resource_codec::ProductSourceId;
use arcweft_compiler::project::{ProjectCompilationContext, ProjectCompileStage, compile_project};
use arcweft_compiler::source::compile_source;
use arcweft_id::PublicId;
use arcweft_lang_hir::model::{HirModule, HirTopLevelDecl};
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::{env::TypeCheckEnv, registration::ProjectRegistrationFacts};
use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModuleSegment};
use arcweft_lang_syntax::ast::view::{ViewExpr, ViewModifier};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::graph::ModuleDependency;
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_runtime_plan::flow::RuntimePlanLowerOptions;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_view::style::{
    ViewEnvironmentTest, ViewStyleApplicationTarget, ViewStylePatchId, ViewStyleSheetId,
    ViewTextScaleComparison,
};

fn project_context(project: &ProjectSources) -> ProjectCompilationContext {
    let documents = project
        .modules()
        .map(|source| Arc::clone(source.document()))
        .collect::<Vec<_>>();
    let root = project.root_module().document();
    let package = CallablePackageId::try_new(project.package().id.as_str()).expect("package id");
    let world =
        ProjectSymbolWorldId::try_new(package, root.identity().id().clone(), "compiler-style-test")
            .expect("symbol world");
    let facts =
        ProjectRegistrationFacts::try_new(world, documents, Vec::new(), Vec::new(), Vec::new())
            .expect("registration facts");
    ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(arcweft_resource_model::registry::ResourceTypeRegistry::empty()),
        None,
        None,
    )
}

#[test]
fn style_compiler_lowers_owned_sheets_patch_and_ordered_applications() {
    let source = r#"pub style primary {
    token color.shared: Color = rgba(10, 20, 30, 255)
    Button { color = token(color.shared) }
}

pub style secondary {
    token color.shared: Color = rgba(40, 50, 60, 255)
    Button { color = token(color.shared) }
}

pub view Example() {
    Column {
        Button("First").style(@style.primary)
        Button("Second")
            .style(@style.secondary)
            .style { opacity = 90% }
    }
}
"#;
    let compiled = compile_source(source).expect("typed Style source compiles");
    let resource = compiled.style.resource();

    assert_eq!(resource.program.sheets().len(), 2);
    assert_eq!(resource.program.patches().len(), 1);
    assert_eq!(resource.program.sheets()[0].tokens().len(), 1);
    assert_eq!(resource.program.sheets()[1].tokens().len(), 1);
    assert_eq!(
        resource.program.sheets()[0].tokens()[0].id(),
        resource.program.sheets()[1].tokens()[0].id(),
        "same token tail remains valid under different sheet owners"
    );
    for source_range in &resource.source_map_refs {
        resource
            .source_refs
            .get(source_range.source().value() as usize)
            .expect("compiler source ref uses the final product-source table");
    }
    let encoded = resource
        .encode_canonical_section()
        .expect("native compiler resource encodes");
    assert_eq!(
        arcweft_bundle::resource_codec::ViewStyleResource::decode_canonical_section(&encoded)
            .expect("native compiler resource decodes"),
        resource.clone()
    );

    let view = PublicId::try_new("view.Example").unwrap();
    let ranges = styled_producer_ranges(&compiled.hir, &view);
    assert_eq!(ranges.len(), 2);
    assert_eq!(
        compiled
            .style
            .applications()
            .applications_for(&view, ranges[0]),
        &[ViewStyleApplicationTarget::named(
            ViewStyleSheetId::try_new("style.primary").unwrap()
        )]
    );
    assert_eq!(
        compiled
            .style
            .applications()
            .applications_for(&view, ranges[1]),
        &[
            ViewStyleApplicationTarget::named(
                ViewStyleSheetId::try_new("style.secondary").unwrap()
            ),
            ViewStyleApplicationTarget::inline(ViewStylePatchId::new(0)),
        ]
    );
}

#[test]
fn style_compiler_keeps_same_shaped_authored_nodes_as_distinct_application_sites() {
    let source = r#"pub style shared {
    Button { opacity = 90% }
}

pub view SameShape() {
    Column {
        Button("Same").style(@style.shared)
        Button("Same").style(@style.shared)
    }
}
"#;
    let compiled = compile_source(source).expect("same-shaped style sites compile");
    let view = PublicId::try_new("view.SameShape").expect("View ID");
    let ranges = styled_producer_ranges(&compiled.hir, &view);
    assert_eq!(ranges.len(), 2);
    assert_ne!(ranges[0], ranges[1]);
    for range in ranges {
        assert_eq!(
            compiled.style.applications().applications_for(&view, range),
            &[ViewStyleApplicationTarget::named(
                ViewStyleSheetId::try_new("style.shared").expect("Style ID")
            )]
        );
    }
}

#[test]
fn style_compiler_lowers_flattened_environment_guard_with_exact_sources() {
    let source = r"pub style adaptive {
    when environment(text-scale >= 125.5%) {
        when environment(color-scheme == dark) {
            Button { opacity = 90% }
        }
    }
}
";
    let compiled = compile_source(source).expect("typed environment Style source compiles");
    let resource = compiled.style.resource();
    let rule = &resource.program.sheets()[0].rules()[0];
    let environment = rule.environment().expect("lowered environment guard");
    assert_eq!(environment.wrappers().len(), 2);
    assert_eq!(environment.clauses().len(), 2);
    assert!(matches!(
        environment.clauses()[0].test(),
        ViewEnvironmentTest::ColorScheme(_)
    ));
    assert!(matches!(
        environment.clauses()[1].test(),
        ViewEnvironmentTest::TextScale {
            comparison: ViewTextScaleComparison::GreaterOrEqual,
            value,
        } if value.value() == 1_255
    ));

    let authored_range = |id: arcweft_view::ViewStyleSourceId| {
        let range = resource.source_map_refs[id.value() as usize];
        &source[range.start_byte() as usize..range.end_byte() as usize]
    };
    let outer = environment.wrappers()[0];
    let inner = environment.wrappers()[1];
    assert_eq!(
        authored_range(outer.predicate_source()),
        "(text-scale >= 125.5%)"
    );
    assert_eq!(
        authored_range(inner.predicate_source()),
        "(color-scheme == dark)"
    );
    assert_eq!(
        authored_range(outer.body_source()),
        "\n        when environment(color-scheme == dark) {\n            Button { opacity = 90% }\n        }\n    "
    );
    assert_eq!(
        authored_range(outer.scope_source()),
        "when environment(text-scale >= 125.5%) {\n        when environment(color-scheme == dark) {\n            Button { opacity = 90% }\n        }\n    }"
    );
    assert_eq!(
        authored_range(inner.scope_source()),
        "when environment(color-scheme == dark) {\n            Button { opacity = 90% }\n        }"
    );
    assert_eq!(
        authored_range(inner.body_source()),
        "\n            Button { opacity = 90% }\n        "
    );
    assert_eq!(authored_range(rule.source()), "Button { opacity = 90% }");
    assert_eq!(
        authored_range(rule.declarations()[0].source()),
        "opacity = 90%"
    );
    assert_eq!(
        environment.clauses()[0].wrapper().value(),
        1,
        "canonical color-scheme clause retains inner ownership"
    );
    assert_eq!(
        environment.clauses()[1].wrapper().value(),
        0,
        "canonical text-scale clause retains outer ownership"
    );
    for clause in environment.clauses() {
        let authored = authored_range(clause.source());
        assert!(
            authored == "text-scale >= 125.5%" || authored == "color-scheme == dark",
            "unexpected clause source: {authored}"
        );
    }
    assert_eq!(
        resource.source_map_refs.len(),
        10,
        "two P/B/S triples, two clauses, one rule, and one declaration are retained separately"
    );
}

#[test]
fn style_compiler_rejects_an_application_to_a_missing_sheet() {
    let error = compile_source(
        r#"pub view Example() {
    Button("OK").style(@style.missing)
}
"#,
    )
    .expect_err("missing named sheet is not product data");
    let project = error.project();
    assert_eq!(project.stage(), ProjectCompileStage::StyleLower.as_str());
    let diagnostic = project
        .diagnostics()
        .first()
        .expect("style rejection emits a diagnostic");
    assert_eq!(
        diagnostic
            .diagnostic()
            .code()
            .expect("style diagnostic code")
            .as_str(),
        "style.lower"
    );
    assert!(diagnostic.diagnostic().message().contains("style.missing"));
}

#[test]
fn style_compiler_uses_canonical_project_patch_ordinals_and_exact_sources() {
    let (project, a, z) = project_with_shuffled_equal_local_style_patch_ranges();
    let context = project_context(&project);
    let compiled = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
        .expect("module-preserving Style project compiles");

    let root = CanonicalModulePath::crate_root();
    let first_patch_ranges = [&root, &a, &z].map(|module| {
        compiled
            .hir_project()
            .module(module)
            .expect("module HIR")
            .style_patches()[0]
            .range()
    });
    assert_eq!(first_patch_ranges[0], first_patch_ranges[1]);
    assert_eq!(first_patch_ranges[1], first_patch_ranges[2]);

    let patches = compiled.style().resource().program.patches();
    assert_eq!(patches.len(), 4);
    assert_eq!(
        patches
            .iter()
            .map(arcweft_view::style::ViewStylePatch::id)
            .collect::<Vec<_>>(),
        (0..4).map(ViewStylePatchId::new).collect::<Vec<_>>()
    );

    let applications = compiled.style().applications();
    for (module, view, expected_patches) in [
        (
            root.clone(),
            PublicId::try_new("view.Root").unwrap(),
            vec![0, 1],
        ),
        (a.clone(), PublicId::try_new("view.a.A").unwrap(), vec![2]),
        (z.clone(), PublicId::try_new("view.z.Z").unwrap(), vec![3]),
    ] {
        let ranges = styled_producer_ranges(
            compiled
                .hir_project()
                .module(&module)
                .expect("View owner module HIR"),
            &view,
        );
        assert_eq!(ranges.len(), expected_patches.len());
        for (range, expected_patch) in ranges.into_iter().zip(expected_patches) {
            assert_eq!(
                applications.applications_for(&view, range),
                &[ViewStyleApplicationTarget::inline(ViewStylePatchId::new(
                    expected_patch
                ))]
            );
        }
    }

    let resource = compiled.style().resource();
    for (project_ordinal, module, local_ordinal) in
        [(0, &root, 0), (1, &root, 1), (2, &a, 0), (3, &z, 0)]
    {
        let declaration = &patches[project_ordinal].declarations()[0];
        let source_range = resource.source_map_refs[declaration.source().value() as usize];
        let source_ref = &resource.source_refs[source_range.source().value() as usize];
        let source = project.module(module).expect("project source module");
        let expected_source =
            ProductSourceId::try_for_document_id(source.document().identity().id())
                .expect("product source identity");
        assert_eq!(source_ref.id(), &expected_source);

        let hir_range = compiled
            .hir_project()
            .module(module)
            .expect("module HIR")
            .style_patches()[local_ordinal]
            .declarations()[0]
            .range();
        assert_eq!(source_range.start_byte() as usize, hir_range.start());
        assert_eq!(source_range.end_byte() as usize, hir_range.end());
    }
}

fn project_with_shuffled_equal_local_style_patch_ranges()
-> (ProjectSources, CanonicalModulePath, CanonicalModulePath) {
    let a =
        CanonicalModulePath::from_segments(
            [ModuleSegment::new("a").expect("valid module segment")],
        );
    let z =
        CanonicalModulePath::from_segments(
            [ModuleSegment::new("z").expect("valid module segment")],
        );
    let root_body = r#"pub view Root() {
    Column {
        Button("Root 0").style { opacity = 80% }
        Button("Root 1").style { opacity = 81% }
    }
}
"#;
    let a_body = r#"mod a

pub view A() {
    Button("A").style { opacity = 70% }
}
"#;
    let z_body = r#"mod z

pub view Z() {
    Button("Z").style { opacity = 60% }
}
"#;
    let first_style_offset = [root_body, a_body, z_body]
        .into_iter()
        .map(|source| source.find(".style").expect("inline Style site"))
        .max()
        .expect("source inventory");
    let align = |source: &str| {
        let current = source.find(".style").expect("inline Style site");
        format!("{}{}", " ".repeat(first_style_offset - current), source)
    };
    let root_source = align(root_body);
    let a_source = align(a_body);
    let z_source = align(z_body);
    assert_eq!(
        root_source.find(".style"),
        a_source.find(".style"),
        "fixture must exercise equal module-local Style ranges"
    );
    assert_eq!(
        a_source.find(".style"),
        z_source.find(".style"),
        "fixture must exercise equal module-local Style ranges"
    );
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        PackageSpec {
            id: PackageId::new("org.arcweft.style-project").expect("package ID"),
            version: PackageVersion::new("0.1.0").expect("package version"),
        },
        BuildSpec::default(),
        style_fixture_document(
            "arcweft-project://style-project/arcw.toml",
            "arcw.toml",
            "schema = 1\n[package]\nid = \"org.arcweft.style-project\"\nversion = \"0.1.0\"\n",
        ),
        [
            ProjectSourceFile::new(
                z.clone(),
                PathBuf::from("src/z.arcw"),
                style_fixture_document("src/z.arcw", "src/z.arcw", z_source),
                [],
            ),
            ProjectSourceFile::new(
                CanonicalModulePath::crate_root(),
                PathBuf::from("src/main.arcw"),
                style_fixture_document("src/main.arcw", "src/main.arcw", root_source),
                [
                    ModuleDependency::new(z.clone()),
                    ModuleDependency::new(a.clone()),
                ],
            ),
            ProjectSourceFile::new(
                a.clone(),
                PathBuf::from("src/a.arcw"),
                style_fixture_document("src/a.arcw", "src/a.arcw", a_source),
                [],
            ),
        ],
    )
    .expect("valid source inventory");
    (project, a, z)
}

fn style_fixture_document(
    id: &str,
    display_name: &str,
    source: impl Into<String>,
) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).expect("fixture document ID"),
            SourceName::path(display_name),
            source.into(),
        )
        .expect("Style fixture document"),
    )
}

fn styled_producer_ranges(hir: &HirModule, view: &PublicId) -> Vec<TextRange> {
    let body = hir
        .declarations()
        .iter()
        .find_map(|declaration| match declaration {
            HirTopLevelDecl::EntityDecl(entity) if entity.id().body() == view.as_str() => {
                entity.view_body()?.view()
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            let candidates = hir
                .declarations()
                .iter()
                .filter_map(|declaration| match declaration {
                    HirTopLevelDecl::EntityDecl(entity) => Some(entity.id().body()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            panic!("View `{view}` is retained in HIR; candidates: {candidates:?}")
        });
    let mut ranges = Vec::new();
    collect_styled_producer_ranges(body.value(), &mut ranges);
    ranges
}

fn collect_styled_producer_ranges(expr: &ViewExpr, output: &mut Vec<TextRange>) {
    let style_range = match expr {
        ViewExpr::Element(node) => Some((node.range(), node.modifiers(), node.children())),
        ViewExpr::ViewCall(node) => Some((node.range(), node.modifiers(), &[][..])),
        ViewExpr::Text(node) => Some((node.range(), node.modifiers(), &[][..])),
        ViewExpr::Image(node) => Some((node.range(), node.modifiers(), &[][..])),
        ViewExpr::TextField(node) => Some((node.range(), node.modifiers(), &[][..])),
        ViewExpr::Button(node) => Some((node.range(), node.modifiers(), &[][..])),
        _ => None,
    };
    if let Some((range, modifiers, children)) = style_range {
        if modifiers
            .iter()
            .any(|modifier| matches!(modifier, ViewModifier::Style(_)))
        {
            output.push(range);
        }
        for child in children {
            collect_styled_producer_ranges(child, output);
        }
    }
    match expr {
        ViewExpr::Fragment(children) => children
            .iter()
            .for_each(|child| collect_styled_producer_ranges(child, output)),
        ViewExpr::If(branch) => {
            collect_styled_producer_ranges(branch.then_branch(), output);
            if let Some(branch) = branch.else_branch() {
                collect_styled_producer_ranges(branch, output);
            }
        }
        ViewExpr::Match(branch) => branch
            .arms()
            .iter()
            .for_each(|arm| collect_styled_producer_ranges(arm.value(), output)),
        ViewExpr::ForEach(loop_expr) => collect_styled_producer_ranges(loop_expr.body(), output),
        ViewExpr::Await(await_expr) => await_expr
            .branches()
            .iter()
            .for_each(|branch| collect_styled_producer_ranges(branch.value(), output)),
        ViewExpr::Element(_)
        | ViewExpr::ViewCall(_)
        | ViewExpr::Text(_)
        | ViewExpr::Image(_)
        | ViewExpr::TextField(_)
        | ViewExpr::Button(_)
        | ViewExpr::Let(_)
        | ViewExpr::Expr(_)
        | ViewExpr::Raw(_) => {}
    }
}
