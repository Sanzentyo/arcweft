use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::ui_blend::{UiBlendPassPlan, UiBlendShaderMode, supported_blend_modes};
use arcweft_render_wgpu::ui_box_shadow::UiBoxShadowPassPlan;
use arcweft_render_wgpu::ui_compositor::{UiCompositorNodePlan, UiCompositorPlan};
use arcweft_render_wgpu::ui_scene::{
    UiAffine2D, UiBlendMode, UiBoxShadow, UiBoxShadowKind, UiBoxShadowList, UiColorRgba8,
    UiCompositingEffects, UiCompositingGroup, UiPaintNode, UiPrimitiveRange, UiScene,
    UiSceneContext,
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
        transform: UiAffine2D::IDENTITY,
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
fn inset_box_shadow_plans_as_typed_inset_pass() {
    let shadows = UiBoxShadowList::new([UiBoxShadow::inset(0.0, 2.0, 6.0, 0.0, 8.0, rgba(190))]);

    let plan = UiBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 80.0, 40.0))
        .expect("inset shadow should plan after seq06.13e");

    assert_eq!(plan.passes().len(), 1);
    assert_eq!(plan.passes()[0].shadow.kind, UiBoxShadowKind::Inset);
    assert!(plan.visual_outset_px().abs() <= f32::EPSILON);
    assert!(plan.visual_inset_px() > 0.0);
}

#[test]
fn mixed_outer_and_inset_shadows_preserve_stage_metadata() {
    let shadows = UiBoxShadowList::new([
        UiBoxShadow::outer(0.0, 8.0, 12.0, 2.0, 8.0, rgba(120)),
        UiBoxShadow::inset(0.0, 2.0, 6.0, 1.0, 8.0, rgba(190)),
    ]);

    let plan = UiBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 80.0, 40.0))
        .expect("mixed shadows should plan");

    assert_eq!(
        plan.passes()
            .iter()
            .map(|pass| pass.shadow_index)
            .collect::<Vec<_>>(),
        vec![1, 0]
    );
    assert_eq!(
        plan.passes_for_kind(UiBoxShadowKind::Outer)
            .map(|pass| pass.shadow_index)
            .collect::<Vec<_>>(),
        vec![0]
    );
    assert_eq!(
        plan.passes_for_kind(UiBoxShadowKind::Inset)
            .map(|pass| pass.shadow_index)
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert!(plan.visual_outset_px() > 0.0);
    assert!(plan.visual_inset_px() > 0.0);
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
