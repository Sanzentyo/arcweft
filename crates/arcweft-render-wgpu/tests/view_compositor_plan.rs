use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::view_compositor::ViewCompositorPlan;
use arcweft_render_wgpu::view_effects::ViewTextureExtent;
use arcweft_render_wgpu::view_scene::{
    ViewAffine2D, ViewBlendMode, ViewColorRgba8, ViewCompositingEffects, ViewCompositingGroup,
    ViewFilter, ViewFilterList, ViewPaintNode, ViewPrimitiveRange, ViewScene, ViewSceneContext,
};

fn direct(start: u32, end: u32) -> ViewPaintNode {
    ViewPaintNode::Direct(ViewSceneContext {
        transform: ViewAffine2D::IDENTITY,
        opacity: 1.0,
        clip: None,
        primitive_range: ViewPrimitiveRange { start, end },
    })
}

#[test]
fn direct_scene_plan_does_not_add_group_effect_passes() {
    let mut scene = ViewScene::new(320.0, 180.0);
    scene.push_paint_node(direct(0, 1));

    let plan = ViewCompositorPlan::from_scene(&scene, 1.0);

    assert_eq!(plan.root_extent(), ViewTextureExtent::new(320, 180));
    assert_eq!(plan.offscreen_target_count(), 1);
    assert_eq!(plan.shader_pass_count(), 1);
    assert_eq!(plan.backdrop_copy_count(), 0);
}

#[test]
fn blur_shadow_mask_and_blend_count_deterministic_passes() {
    let mut scene = ViewScene::new(320.0, 180.0);
    let effects = ViewCompositingEffects {
        filters: ViewFilterList::new([
            ViewFilter::Blur { radius_px: 4.0 },
            ViewFilter::DropShadow {
                offset_x_px: 2.0,
                offset_y_px: 6.0,
                blur_radius_px: 3.0,
                color: ViewColorRgba8 {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: 192,
                },
            },
        ]),
        blend_mode: ViewBlendMode::Multiply,
        ..ViewCompositingEffects::default()
    };
    scene.push_paint_node(ViewPaintNode::Group(
        ViewCompositingGroup::new(HitRect::new(10.0, 20.0, 100.0, 50.0), effects)
            .with_children(vec![direct(0, 1)]),
    ));

    let plan = ViewCompositorPlan::from_scene(&scene, 1.0);

    assert_eq!(plan.backdrop_copy_count(), 0);
    assert!(plan.shader_pass_count() >= 5);
    assert!(plan.offscreen_target_count() >= 2);
}

#[test]
fn compositing_plan_snapshot_covers_core_effect_families() {
    let mut scene = ViewScene::new(800.0, 450.0);
    let group = ViewCompositingGroup::new(
        HitRect::new(100.0, 40.0, 240.0, 160.0),
        ViewCompositingEffects {
            filters: ViewFilterList::new([
                ViewFilter::Brightness(1.1),
                ViewFilter::Blur { radius_px: 6.0 },
                ViewFilter::DropShadow {
                    offset_x_px: 8.0,
                    offset_y_px: 12.0,
                    blur_radius_px: 5.0,
                    color: ViewColorRgba8 {
                        red: 16,
                        green: 16,
                        blue: 20,
                        alpha: 220,
                    },
                },
            ]),
            backdrop_filters: ViewFilterList::new([ViewFilter::Saturate(1.2)]),
            blend_mode: ViewBlendMode::Screen,
            ..ViewCompositingEffects::default()
        },
    )
    .with_children(vec![direct(0, 3), direct(3, 5)]);
    scene.push_paint_node(ViewPaintNode::Group(group));

    let plan = ViewCompositorPlan::from_scene(&scene, 1.0);

    assert_eq!(plan.root_extent(), ViewTextureExtent::new(800, 450));
    assert_eq!(plan.backdrop_copy_count(), 1);
    assert!(plan.offscreen_target_count() >= 2);
    assert!(plan.shader_pass_count() >= 6);
}
