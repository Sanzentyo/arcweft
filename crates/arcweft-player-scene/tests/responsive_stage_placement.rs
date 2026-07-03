//! Focused integration test sketch for player-side image placement.
//!
//! This test should be enabled after applying the overlay to a full checkout with
//! a minimal in-memory `ArcweftBundle` image asset. It documents the expected
//! player-prepared-frame contract: `RenderImage.bounds` and
//! `RenderImage.placement.output_bbox` must match exactly.

use arcweft_layout::{
    LayoutRect,
    stage_placement::{StageAnchor, StageInsets, StagePlacement, StageSize},
};
use num_traits::ToPrimitive;

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

#[test]
fn player_frame_uses_same_resolved_bbox_for_render_and_observe() {
    let placement = StagePlacement::anchor(
        StageAnchor::TopRight,
        StageAnchor::TopRight,
        StageSize::new(250_000, 430_000),
    )
    .with_margins(StageInsets::new(20_000, 100_000, 0, 0));

    let resolved = placement
        .resolve(arcweft_layout::stage_placement::StagePlacementContext::new(
            arcweft_layout::LayoutSize::new(1280.0, 720.0),
            arcweft_layout::LayoutSize::new(1920.0, 1080.0),
        ))
        .unwrap();

    assert_eq!(
        rect_milli(resolved.output_bbox),
        (1_395_000, 30_000, 375_000, 645_000)
    );
}
