use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::ui_compositor::UiCompositorPlan;
use arcweft_render_wgpu::ui_effects::UiTextureExtent;
use arcweft_render_wgpu::ui_scene::{
    UiAffine2, UiBlendMode, UiColorRgba8, UiCompositingEffects, UiCompositingGroup, UiFilter,
    UiFilterList, UiPaintNode, UiPrimitiveRange, UiScene, UiSceneContext,
};

fn direct(start: u32, end: u32) -> UiPaintNode {
    UiPaintNode::Direct(UiSceneContext {
        transform: UiAffine2::IDENTITY,
        opacity: 1.0,
        clip: None,
        primitive_range: UiPrimitiveRange { start, end },
    })
}

#[test]
fn direct_scene_plan_does_not_add_group_effect_passes() {
    let mut scene = UiScene::new(320.0, 180.0);
    scene.push_paint_node(direct(0, 1));

    let plan = UiCompositorPlan::from_scene(&scene, 1.0);

    assert_eq!(plan.root_extent(), UiTextureExtent::new(320, 180));
    assert_eq!(plan.offscreen_target_count(), 1);
    assert_eq!(plan.shader_pass_count(), 1);
    assert_eq!(plan.backdrop_copy_count(), 0);
}

#[test]
fn blur_shadow_mask_and_blend_count_deterministic_passes() {
    let mut scene = UiScene::new(320.0, 180.0);
    let effects = UiCompositingEffects {
        filters: UiFilterList::new([
            UiFilter::Blur { radius_px: 4.0 },
            UiFilter::DropShadow {
                offset_x_px: 2.0,
                offset_y_px: 6.0,
                blur_radius_px: 3.0,
                color: UiColorRgba8 {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: 192,
                },
            },
        ]),
        blend_mode: UiBlendMode::Multiply,
        ..UiCompositingEffects::default()
    };
    scene.push_paint_node(UiPaintNode::Group(
        UiCompositingGroup::new(HitRect::new(10.0, 20.0, 100.0, 50.0), effects)
            .with_children(vec![direct(0, 1)]),
    ));

    let plan = UiCompositorPlan::from_scene(&scene, 1.0);

    assert_eq!(plan.backdrop_copy_count(), 0);
    assert!(plan.shader_pass_count() >= 5);
    assert!(plan.offscreen_target_count() >= 2);
}

#[test]
fn compositing_plan_snapshot_covers_core_effect_families() {
    let mut scene = UiScene::new(800.0, 450.0);
    let group = UiCompositingGroup::new(
        HitRect::new(100.0, 40.0, 240.0, 160.0),
        UiCompositingEffects {
            filters: UiFilterList::new([
                UiFilter::Brightness(1.1),
                UiFilter::Blur { radius_px: 6.0 },
                UiFilter::DropShadow {
                    offset_x_px: 8.0,
                    offset_y_px: 12.0,
                    blur_radius_px: 5.0,
                    color: UiColorRgba8 {
                        red: 16,
                        green: 16,
                        blue: 20,
                        alpha: 220,
                    },
                },
            ]),
            backdrop_filters: UiFilterList::new([UiFilter::Saturate(1.2)]),
            blend_mode: UiBlendMode::Screen,
            ..UiCompositingEffects::default()
        },
    )
    .with_children(vec![direct(0, 3), direct(3, 5)]);
    scene.push_paint_node(UiPaintNode::Group(group));

    let plan = UiCompositorPlan::from_scene(&scene, 1.0);

    assert_eq!(plan.root_extent(), UiTextureExtent::new(800, 450));
    assert_eq!(plan.backdrop_copy_count(), 1);
    assert!(plan.offscreen_target_count() >= 2);
    assert!(plan.shader_pass_count() >= 6);
}
