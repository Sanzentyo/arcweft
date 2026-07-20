use std::{path::PathBuf, sync::Arc};

use arcweft_compiler::error::CompileSourceError;
use arcweft_compiler::project::{ProjectCompilationContext, compile_project};
use arcweft_compiler::source::compile_source;
use arcweft_compiler::style::ViewStyleLowerError;
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
    let facts = ProjectRegistrationFacts::try_new(world, documents, Vec::new(), Vec::new())
        .expect("registration facts");
    ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        None,
        None,
        Vec::new(),
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
            .style { opacity = 900milli }
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
fn style_compiler_lowers_flattened_environment_guard_with_exact_sources() {
    let source = r"pub style adaptive {
    when environment(text-scale >= 125.5%) {
        when environment(color-scheme == dark) {
            Button { opacity = 900milli }
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
        "\n        when environment(color-scheme == dark) {\n            Button { opacity = 900milli }\n        }\n    "
    );
    assert_eq!(
        authored_range(outer.scope_source()),
        "when environment(text-scale >= 125.5%) {\n        when environment(color-scheme == dark) {\n            Button { opacity = 900milli }\n        }\n    }"
    );
    assert_eq!(
        authored_range(inner.scope_source()),
        "when environment(color-scheme == dark) {\n            Button { opacity = 900milli }\n        }"
    );
    assert_eq!(
        authored_range(inner.body_source()),
        "\n            Button { opacity = 900milli }\n        "
    );
    assert_eq!(
        authored_range(rule.source()),
        "Button { opacity = 900milli }"
    );
    assert_eq!(
        authored_range(rule.declarations()[0].source()),
        "opacity = 900milli"
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
    assert!(matches!(
        error,
        CompileSourceError::Style(ViewStyleLowerError::UnknownSheetApplication { sheet, .. })
            if sheet == "style.missing"
    ));
}

#[test]
fn style_compiler_qualifies_equal_local_patch_ranges_and_uses_checked_ordinals() {
    let (project, child) = project_with_equal_local_style_patch_ranges();
    let context = project_context(&project);
    let compiled = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
        .expect("linked Style project compiles");

    let root_patch_range = compiled
        .hir_project()
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR")
        .style_patches()[0]
        .range();
    let child_patch_range = compiled
        .hir_project()
        .module(&child)
        .expect("child HIR")
        .style_patches()[0]
        .range();
    assert_eq!(root_patch_range, child_patch_range);

    let patches = compiled.style().resource().program.patches();
    assert_eq!(patches.len(), 2);
    assert_eq!(patches[0].id(), ViewStylePatchId::new(0));
    assert_eq!(patches[1].id(), ViewStylePatchId::new(1));

    let applications = compiled.style().applications();
    for (view, expected_patch) in [
        (PublicId::try_new("view.Root").unwrap(), 0),
        (PublicId::try_new("view.child.Child").unwrap(), 1),
    ] {
        let ranges = styled_producer_ranges(compiled.linked_hir(), &view);
        assert_eq!(ranges.len(), 1);
        assert_eq!(
            applications.applications_for(&view, ranges[0]),
            &[ViewStyleApplicationTarget::inline(ViewStylePatchId::new(
                expected_patch
            ))]
        );
    }
}

fn project_with_equal_local_style_patch_ranges() -> (ProjectSources, CanonicalModulePath) {
    let child = CanonicalModulePath::from_segments([
        ModuleSegment::new("child").expect("valid module segment")
    ]);
    let root_body = r#"pub view Root() {
    Button("Root").style { opacity = 800milli }
}
"#;
    let child_source = r#"mod child

pub view Child() {
    Button("Child").style { opacity = 700milli }
}
"#;
    let root_style_offset = root_body.find(".style").expect("root inline Style site");
    let child_style_offset = child_source
        .find(".style")
        .expect("child inline Style site");
    let root_source = format!(
        "{}{}",
        " ".repeat(child_style_offset - root_style_offset),
        root_body
    );
    assert_eq!(
        root_source.find(".style"),
        child_source.find(".style"),
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
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-project://style-project/arcw.toml")
                    .expect("manifest document ID"),
                SourceName::path("arcw.toml"),
                "schema = 1\n[package]\nid = \"org.arcweft.style-project\"\nversion = \"0.1.0\"\n",
            )
            .expect("manifest document"),
        ),
        [
            ProjectSourceFile::new(
                CanonicalModulePath::crate_root(),
                PathBuf::from("src/main.arcw"),
                Arc::new(
                    SourceDocument::try_new(
                        SourceDocumentId::try_new("src/main.arcw").expect("root document id"),
                        SourceName::path("src/main.arcw"),
                        root_source,
                    )
                    .expect("root document"),
                ),
                [ModuleDependency::new(child.clone())],
            ),
            ProjectSourceFile::new(
                child.clone(),
                PathBuf::from("src/child.arcw"),
                Arc::new(
                    SourceDocument::try_new(
                        SourceDocumentId::try_new("src/child.arcw").expect("child document id"),
                        SourceName::path("src/child.arcw"),
                        child_source,
                    )
                    .expect("child document"),
                ),
                [],
            ),
        ],
    )
    .expect("valid source inventory");
    (project, child)
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
        .expect("View body is retained in HIR");
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
