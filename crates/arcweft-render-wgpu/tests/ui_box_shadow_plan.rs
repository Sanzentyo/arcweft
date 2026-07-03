use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::ui_blend::{UiBlendPassPlan, UiBlendShaderMode, supported_blend_modes};
use arcweft_render_wgpu::ui_box_shadow::{UiBoxShadowPassPlan, UiBoxShadowPlanError};
use arcweft_render_wgpu::ui_compositor::{UiCompositorNodePlan, UiCompositorPlan};
use arcweft_render_wgpu::ui_scene::{
    UiAffine2, UiBlendMode, UiBoxShadow, UiBoxShadowList, UiColorRgba8, UiCompositingEffects,
    UiCompositingGroup, UiPaintNode, UiPrimitiveRange, UiScene, UiSceneContext,
};

fn rgba(alpha: u8) -> UiColorRgba8 {
    UiColorRgba8 {
        red: 32,
        green: 40,
        blue: 64,
        alpha,
    }
}

fn direct(start: u32, end: u32) -> UiPaintNode {
    UiPaintNode::Direct(UiSceneContext {
        transform: UiAffine2::IDENTITY,
        opacity: 1.0,
        clip: None,
        primitive_range: UiPrimitiveRange { start, end },
    })
}

#[test]
fn outer_box_shadow_plans_before_children_and_expands_visual_extent() {
    let mut scene = UiScene::new(320.0, 180.0);
    let effects = UiCompositingEffects {
        box_shadows: UiBoxShadowList::new([UiBoxShadow::outer(
            4.0,
            8.0,
            12.0,
            2.0,
            10.0,
            rgba(180),
        )]),
        ..UiCompositingEffects::default()
    };
    scene.push_paint_node(UiPaintNode::Group(
        UiCompositingGroup::new(HitRect::new(40.0, 30.0, 120.0, 60.0), effects)
            .with_children(vec![direct(0, 1)]),
    ));

    let plan = UiCompositorPlan::from_scene(&scene, 1.0);
    let UiCompositorNodePlan::Group {
        visual_extent,
        effects,
        ..
    } = &plan.nodes()[0]
    else {
        panic!("expected group node");
    };
    let box_shadows = effects
        .box_shadows
        .as_ref()
        .expect("box-shadow plan succeeds");

    assert_eq!(box_shadows.passes().len(), 1);
    assert!(visual_extent.width > 120);
    assert!(visual_extent.height > 60);
    assert!(plan.shader_pass_count() >= 3);
}

#[test]
fn inset_box_shadow_is_a_typed_diagnostic() {
    let shadows = UiBoxShadowList::new([UiBoxShadow::inset(0.0, 2.0, 6.0, 0.0, 8.0, rgba(190))]);

    assert_eq!(
        UiBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 80.0, 40.0)),
        Err(UiBoxShadowPlanError::InsetUnsupported { shadow_index: 0 })
    );
}

#[test]
fn hsl_family_blends_remain_supported() {
    for mode in [
        UiBlendMode::Hue,
        UiBlendMode::Saturation,
        UiBlendMode::Color,
        UiBlendMode::Luminosity,
    ] {
        assert!(supported_blend_modes().contains(&mode));
        assert!(UiBlendPassPlan::from_mode(mode).is_some());
    }
    assert_eq!(
        UiBlendPassPlan::from_mode(UiBlendMode::Luminosity).map(|plan| plan.shader_mode),
        Some(UiBlendShaderMode::Luminosity)
    );
}
