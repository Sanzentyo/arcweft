use arcweft_layout::{
    LayoutRect, LayoutSize, SafeAreaInsets,
    stage_placement::{
        StageAnchor, StageInsets, StagePlacement, StagePlacementContext, StageRect, StageSize,
    },
};
use num_traits::ToPrimitive;

fn standing_top_right() -> StagePlacement {
    StagePlacement::anchor(
        StageAnchor::TopRight,
        StageAnchor::TopRight,
        StageSize::new(250_000, 430_000),
    )
    .with_margins(StageInsets::new(20_000, 100_000, 0, 0))
}

fn context(width: f32, height: f32) -> StagePlacementContext {
    StagePlacementContext::new(
        LayoutSize::new(1280.0, 720.0),
        LayoutSize::new(width, height),
    )
}

fn f32_to_i32_milli(value: f32) -> i32 {
    let milli = f64::from(value) * 1_000.0;
    let rounded = milli.round();
    if rounded.is_finite() {
        rounded
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX))
            .to_i32()
            .unwrap_or(0)
    } else {
        0
    }
}

fn rect_milli(rect: LayoutRect) -> (i32, i32, u32, u32) {
    (
        f32_to_i32_milli(rect.origin.x),
        f32_to_i32_milli(rect.origin.y),
        u32::try_from(f32_to_i32_milli(rect.size.width)).unwrap(),
        u32::try_from(f32_to_i32_milli(rect.size.height)).unwrap(),
    )
}

fn assert_rect_milli_eq(rect: LayoutRect, expected: (i32, i32, u32, u32)) {
    assert_eq!(rect_milli(rect), expected);
}

#[test]
fn anchored_standing_image_scales_at_all_required_viewports() {
    for (width, height, expected) in [
        (1280.0, 720.0, (930_000, 20_000, 250_000, 430_000)),
        (1920.0, 1080.0, (1_395_000, 30_000, 375_000, 645_000)),
        (2560.0, 1440.0, (1_860_000, 40_000, 500_000, 860_000)),
    ] {
        let resolved = standing_top_right()
            .resolve(context(width, height))
            .unwrap();
        assert_rect_milli_eq(resolved.output_bbox, expected);
        assert!(resolved.diagnostics.is_empty());
    }
}

#[test]
fn absolute_mode_is_explicit_raw_output_placement() {
    let resolved = StagePlacement::absolute(StageRect::new(930_000, 20_000, 250_000, 430_000))
        .resolve(context(2560.0, 1440.0))
        .unwrap();
    assert_rect_milli_eq(resolved.output_bbox, (930_000, 20_000, 250_000, 430_000));
}

#[test]
fn high_dpi_keeps_logical_bbox_and_scales_physical_bbox() {
    let resolved = standing_top_right()
        .resolve(
            context(1920.0, 1080.0)
                .with_physical_viewport(LayoutSize::new(3840.0, 2160.0))
                .with_scale_factor(2.0),
        )
        .unwrap();
    assert_rect_milli_eq(resolved.output_bbox, (1_395_000, 30_000, 375_000, 645_000));
    assert_rect_milli_eq(
        resolved.physical_bbox,
        (2_790_000, 60_000, 750_000, 1_290_000),
    );
}

#[test]
fn safe_area_insets_change_the_anchor_basis() {
    let resolved = standing_top_right()
        .with_safe_area(true)
        .resolve(context(1280.0, 720.0).with_safe_area(SafeAreaInsets {
            top: 0.0,
            right: 40.0,
            bottom: 0.0,
            left: 0.0,
        }))
        .unwrap();
    assert_rect_milli_eq(resolved.output_bbox, (890_000, 20_000, 250_000, 430_000));
}
