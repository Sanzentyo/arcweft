#[test]
#[ignore = "exact compositing PNG promotion requires pinned native GPU environment"]
fn exact_png_promotion_lane_is_manual_only() {
    let promotion = include_str!("fixtures/compositing-capture/promotion-review.md");
    assert!(promotion.contains("manual-only"));
    assert!(promotion.contains("Do not enable default CI enforcement"));
}
