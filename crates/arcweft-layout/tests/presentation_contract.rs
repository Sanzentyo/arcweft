// Focused presentation contract tests for layout, units, text fitting, and capture metadata.

use arcweft_layout::{
    CaptureMetadata, CaptureRendererKind, ContentRect, HitTestInputSpace, LayoutCoordinateSpace,
    LayoutEvaluationContext, LayoutLengthExpr, LayoutPoint, LayoutRect, LayoutSize, LayoutUnit,
    LayoutUnitResolutionPhase, SafeAreaInsets, ScalePolicy, TextFitDiagnostic,
    TextFitDiagnosticCode, TextFitOutcome, TextFitResult, TextOverflowPolicy, TextPage,
};

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.000_1,
        "expected {actual} to be close to {expected}"
    );
}

#[test]
fn contain_maps_1000_by_800_to_letterboxed_content_rect() {
    let rect = ContentRect::calculate(
        LayoutSize::new(1280.0, 720.0),
        LayoutSize::new(1000.0, 800.0),
        ScalePolicy::Contain,
    )
    .expect("content rect");
    assert_eq!(rect.rect.origin, LayoutPoint::new(0.0, 118.75));
    assert_close(rect.rect.size.width, 1_000.0);
    assert_close(rect.rect.size.height, 562.5);
}

#[test]
fn cover_maps_1000_by_800_to_signed_crop_rect() {
    let rect = ContentRect::calculate(
        LayoutSize::new(1280.0, 720.0),
        LayoutSize::new(1000.0, 800.0),
        ScalePolicy::Cover,
    )
    .expect("content rect");
    assert_close(rect.rect.origin.x, -211.111_15);
    assert_close(rect.rect.origin.y, 0.0);
    assert_close(rect.rect.size.width, 1_422.222_3);
    assert_close(rect.rect.size.height, 800.0);
}

#[test]
fn stretch_reports_anisotropic_scale_without_bars_or_crop() {
    let rect = ContentRect::calculate(
        LayoutSize::new(1280.0, 720.0),
        LayoutSize::new(1000.0, 800.0),
        ScalePolicy::Stretch,
    )
    .expect("content rect");
    let metadata =
        rect.fit_transform_metadata(LayoutCoordinateSpace::Output, LayoutCoordinateSpace::Output);
    assert_close(metadata.scale_x, 0.781_25);
    assert_close(metadata.scale_y, 1.111_111_2);
    assert_eq!(metadata.bars, arcweft_layout::LayoutInsets::default());
    assert_eq!(metadata.crop, arcweft_layout::LayoutInsets::default());
}

#[test]
fn inverse_mapping_returns_design_point() {
    let rect = ContentRect::calculate(
        LayoutSize::new(1280.0, 720.0),
        LayoutSize::new(1000.0, 800.0),
        ScalePolicy::Contain,
    )
    .expect("content rect");
    let design = LayoutPoint::new(96.0, 48.0);
    let output = rect.map_point(design);
    assert_eq!(rect.unmap_point(output), design);
}

#[test]
fn non_16_by_9_metadata_reports_bars_and_crop() {
    let contain = ContentRect::calculate(
        LayoutSize::new(1280.0, 720.0),
        LayoutSize::new(1000.0, 800.0),
        ScalePolicy::Contain,
    )
    .expect("content rect");
    let contain_metadata = contain
        .fit_transform_metadata(LayoutCoordinateSpace::Output, LayoutCoordinateSpace::Output);
    assert_close(contain_metadata.bars.top, 118.75);
    assert_close(contain_metadata.bars.bottom, 118.75);
    assert_eq!(
        contain_metadata.crop,
        arcweft_layout::LayoutInsets::default()
    );

    let cover = ContentRect::calculate(
        LayoutSize::new(1280.0, 720.0),
        LayoutSize::new(1000.0, 800.0),
        ScalePolicy::Cover,
    )
    .expect("content rect");
    let cover_metadata =
        cover.fit_transform_metadata(LayoutCoordinateSpace::Output, LayoutCoordinateSpace::Output);
    assert_close(cover_metadata.crop.left, 211.111_15);
    assert_close(cover_metadata.crop.right, 211.111_15);
    assert_eq!(cover_metadata.bars, arcweft_layout::LayoutInsets::default());
}

#[test]
fn hit_test_mapping_converts_output_point_to_design_point() {
    let rect = ContentRect::calculate(
        LayoutSize::new(1280.0, 720.0),
        LayoutSize::new(1000.0, 800.0),
        ScalePolicy::Contain,
    )
    .expect("content rect");
    let mapping = rect.hit_test_mapping(LayoutPoint::new(500.0, 400.0), HitTestInputSpace::Output);
    assert_close(mapping.design_point.x, 640.0);
    assert_close(mapping.design_point.y, 360.0);
    assert!(mapping.inside_design_viewport);
    assert!(mapping.inside_content_rect);
    assert!(mapping.inside_output_viewport);
}

#[test]
fn length_expr_evaluates_against_context() {
    let content_rect = ContentRect::calculate(
        LayoutSize::new(1280.0, 720.0),
        LayoutSize::new(1000.0, 800.0),
        ScalePolicy::Contain,
    )
    .expect("content rect");
    let context = LayoutEvaluationContext {
        design_viewport: LayoutSize::new(1280.0, 720.0),
        output_viewport: LayoutSize::new(1000.0, 800.0),
        content_rect,
        containing_box: LayoutSize::new(400.0, 200.0),
        font_size: 16.0,
        glyph_ch: 8.0,
        safe_area: SafeAreaInsets {
            top: 4.0,
            right: 8.0,
            bottom: 12.0,
            left: 16.0,
        },
    };
    let expr = LayoutLengthExpr::Clamp {
        min: Box::new(LayoutLengthExpr::Literal {
            value: 10.0,
            unit: LayoutUnit::Px,
        }),
        value: Box::new(LayoutLengthExpr::Add {
            left: Box::new(LayoutLengthExpr::Literal {
                value: 50.0,
                unit: LayoutUnit::Percent,
            }),
            right: Box::new(LayoutLengthExpr::Literal {
                value: 2.0,
                unit: LayoutUnit::Em,
            }),
        }),
        max: Box::new(LayoutLengthExpr::Literal {
            value: 80.0,
            unit: LayoutUnit::Px,
        }),
    };
    assert_close(expr.evaluate(&context, true).expect("expr evaluates"), 80.0);
}

#[test]
fn unit_resolution_policy_identifies_context_dependencies() {
    assert_eq!(
        LayoutUnit::Px.earliest_resolution_phase(),
        LayoutUnitResolutionPhase::RuntimePlan
    );
    assert!(LayoutUnit::GlyphCh.requires_font_metrics());
    assert!(LayoutUnit::SafeAreaTop.requires_safe_area());
    assert!(LayoutUnit::Cw.requires_content_rect());
}

#[test]
fn text_fit_report_classifies_overflow_and_failure() {
    let result = TextFitResult {
        policy: TextOverflowPolicy::Page,
        pages: vec![
            TextPage {
                cluster_start: 0,
                cluster_end: 4,
            },
            TextPage {
                cluster_start: 4,
                cluster_end: 8,
            },
        ],
        fitted_font_size: None,
        expanded_bounds: None,
        diagnostics: Vec::new(),
    };
    assert_eq!(result.report().outcome, TextFitOutcome::Paginated);

    let failed = TextFitResult {
        policy: TextOverflowPolicy::FitText,
        pages: Vec::new(),
        fitted_font_size: Some(10.0),
        expanded_bounds: None,
        diagnostics: vec![TextFitDiagnostic {
            code: TextFitDiagnosticCode::FitTextFailed,
            message: "minimum font size still overflows".to_owned(),
        }],
    };
    let report = failed.report();
    assert!(report.failed());
    assert_eq!(report.outcome, TextFitOutcome::Failed);
}

#[test]
fn selected_capture_metadata_carries_scope_crop_mask_and_fit() {
    let fit = ContentRect::calculate(
        LayoutSize::new(1280.0, 720.0),
        LayoutSize::new(1000.0, 800.0),
        ScalePolicy::Contain,
    )
    .expect("content rect")
    .fit_transform_metadata(LayoutCoordinateSpace::Output, LayoutCoordinateSpace::Output);
    let metadata = CaptureMetadata::selected_object(
        CaptureRendererKind::NativeRichTextObserver,
        "object.dialogue.0.0",
        LayoutRect::new(96.0, 600.0, 808.0, 120.0),
        LayoutRect::new(96.0, 600.0, 808.0, 100.0),
        fit,
    );
    assert_eq!(metadata.coordinate_basis, LayoutCoordinateSpace::Output);
    assert_eq!(metadata.crop.basis, LayoutCoordinateSpace::Output);
    let mask = metadata.mask.expect("mask metadata");
    assert_eq!(mask.object_ids, vec!["object.dialogue.0.0".to_owned()]);
    assert!(mask.has_object_id_attachment);
    assert!(mask.has_alpha_mask);
}

#[test]
fn clipped_rect_preserves_signed_source_before_clipping() {
    let rect = LayoutRect::new(-10.0, 5.0, 30.0, 50.0);
    assert_eq!(
        rect.clipped_to(LayoutSize::new(100.0, 40.0)),
        LayoutRect::new(0.0, 5.0, 20.0, 35.0)
    );
}
