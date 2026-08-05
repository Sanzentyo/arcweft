use std::{path::PathBuf, sync::Arc};

use arcweft_compiler::image::{ImageCompileError, lower_project_images};
use arcweft_id::PublicId;
use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath,
    parser::{ParseOptions, parse_document_with_source},
};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_presentation::image::ImageObjectId;
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSpan};

struct ImageProjectFixture {
    document: Arc<SourceDocument>,
    hir: HirProject,
    project: ProjectSources,
}

fn image_project(source: &str) -> ImageProjectFixture {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler-image-catalog").expect("source ID"),
            SourceName::path("main.arcw"),
            source,
        )
        .expect("source document"),
    );
    let module = CanonicalModulePath::crate_root();
    let syntax = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    assert_eq!(
        syntax.errors(),
        &[],
        "fixture must be current Arcweft syntax"
    );
    let lowered = lower_document_to_hir(document.as_ref(), syntax.typed_tree())
        .expect("fixture lowers to bound HIR");
    let hir = HirProject::new(
        "local.arcweft.image-catalog-test",
        vec![
            HirProjectModule::try_new(module.clone(), document.identity().clone(), lowered)
                .expect("HIR module"),
        ],
    )
    .expect("HIR project");
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::from("."),
        PackageSpec {
            id: PackageId::new("local.arcweft.image-catalog-test").expect("package ID"),
            version: PackageVersion::new("0.0.0").expect("package version"),
        },
        BuildSpec::default(),
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://compiler-image-manifest")
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
    ImageProjectFixture {
        document,
        hir,
        project,
    }
}

fn source_text<'a>(document: &'a SourceDocument, span: &SourceSpan) -> &'a str {
    span.validate_for(document)
        .expect("span belongs to fixture");
    let range = span.range();
    &document.text()[range.start()..range.end()]
}

fn absolute_image(extra_fields: &str) -> String {
    format!(
        r"
pub image poster {{
    asset = @asset.poster
    x = 0px
    y = 0px
    width = 1280px
    height = 720px
{extra_fields}
}}
"
    )
}

#[test]
fn compiler_image_catalog_preserves_typed_ids_and_negative_coordinates() {
    let source = absolute_image("    action = @action.inspect\n    layer = @layer.overlay")
        .replace("x = 0px", "x = -12px");
    let fixture = image_project(&source);
    let catalog = lower_project_images(&fixture.hir, &fixture.project).expect("image catalog");
    let [image] = catalog.objects() else {
        panic!("one image expected");
    };
    assert_eq!(image.id, "image.poster");
    assert_eq!(image.asset, "asset.poster");
    assert_eq!(image.bounds.x_milli, -12_000);
    assert_eq!(image.actions, vec!["action.inspect".to_owned()]);
    assert_eq!(image.layer.as_deref(), Some("layer.overlay"));
    assert_eq!(
        catalog
            .source(&ImageObjectId::new(
                PublicId::try_new("image.poster").expect("image ID"),
            ))
            .expect("declaration source")
            .source(),
        fixture.document.identity()
    );
}

#[test]
fn unknown_image_field_reports_the_exact_field_name() {
    let fixture = image_project(&absolute_image("    mystery = true"));
    let error = lower_project_images(&fixture.hir, &fixture.project)
        .expect_err("unknown field must not be ignored");
    let ImageCompileError::UnsupportedField { field, span, .. } = error else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(field, "mystery");
    assert_eq!(source_text(&fixture.document, &span), "mystery");
}

#[test]
fn invalid_optional_image_value_reports_the_exact_value() {
    let fixture = image_project(&absolute_image("    visible = maybe"));
    let error = lower_project_images(&fixture.hir, &fixture.project)
        .expect_err("invalid optional value must not silently use a default");
    let ImageCompileError::InvalidField { field, span, .. } = error else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(field, "visible");
    assert_eq!(source_text(&fixture.document, &span), "maybe");
}

#[test]
fn image_geometry_rejects_a_non_px_unit_at_the_value_span() {
    let source = absolute_image("").replace("width = 1280px", "width = 1280ms");
    let fixture = image_project(&source);
    let error = lower_project_images(&fixture.hir, &fixture.project)
        .expect_err("geometry must retain its px unit contract");
    let ImageCompileError::InvalidField { field, span, .. } = error else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(field, "width");
    assert_eq!(source_text(&fixture.document, &span), "1280ms");
}

#[test]
fn image_size_rejects_negative_px_without_losing_the_authored_span() {
    let source = absolute_image("").replace("height = 720px", "height = -1px");
    let fixture = image_project(&source);
    let error = lower_project_images(&fixture.hir, &fixture.project)
        .expect_err("negative image size must be rejected");
    let ImageCompileError::InvalidField { field, span, .. } = error else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(field, "height");
    assert_eq!(source_text(&fixture.document, &span), "-1px");
}

#[test]
fn retained_image_references_enforce_their_nominal_families() {
    for (field, value) in [
        ("asset", "@action.not_an_asset"),
        ("layer", "@action.not_a_layer"),
        ("action", "@layer.not_an_action"),
    ] {
        let source = if field == "asset" {
            absolute_image("").replace("@asset.poster", value)
        } else {
            absolute_image(&format!("    {field} = {value}"))
        };
        let fixture = image_project(&source);
        let error = lower_project_images(&fixture.hir, &fixture.project)
            .expect_err("wrong retained identity family must be rejected");
        let ImageCompileError::InvalidField {
            field: actual,
            span,
            ..
        } = error
        else {
            panic!("unexpected error: {error:?}");
        };
        assert_eq!(actual, field);
        assert_eq!(source_text(&fixture.document, &span), value);
    }
}

#[test]
fn unpublished_body_identity_and_enabled_fields_are_rejected_not_shimmed() {
    for field in ["id", "enabled"] {
        let fixture = image_project(&absolute_image(&format!("    {field} = true")));
        let error = lower_project_images(&fixture.hir, &fixture.project)
            .expect_err("unsupported provisional field must be rejected");
        let ImageCompileError::UnsupportedField {
            field: actual,
            span,
            ..
        } = error
        else {
            panic!("unexpected error: {error:?}");
        };
        assert_eq!(actual, field);
        assert_eq!(source_text(&fixture.document, &span), field);
    }
}

#[test]
fn image_lowering_rejects_a_hir_project_bound_to_another_source_revision() {
    let source = absolute_image("");
    let fixture = image_project(&source);
    let detached = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://detached-compiler-image-catalog")
                .expect("detached source ID"),
            SourceName::path("main.arcw"),
            source,
        )
        .expect("detached source document"),
    );
    let syntax = parse_document_with_source(Arc::clone(&detached), ParseOptions::default());
    assert_eq!(syntax.errors(), &[]);
    let lowered =
        lower_document_to_hir(detached.as_ref(), syntax.typed_tree()).expect("detached HIR");
    let detached_hir = HirProject::new(
        "local.arcweft.image-catalog-test",
        vec![
            HirProjectModule::try_new(
                CanonicalModulePath::crate_root(),
                detached.identity().clone(),
                lowered,
            )
            .expect("detached HIR module"),
        ],
    )
    .expect("detached HIR project");

    let error = lower_project_images(&detached_hir, &fixture.project)
        .expect_err("same text from another source identity must not be admitted");
    assert!(matches!(
        error,
        ImageCompileError::ProjectHirSourceMismatch { .. }
    ));
}
