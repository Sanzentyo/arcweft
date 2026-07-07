use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::view_blend::{
    ViewBlendPassPlan, ViewBlendShaderMode, supported_blend_modes,
};
use arcweft_render_wgpu::view_box_shadow::{ViewBoxShadowPassPlan, ViewBoxShadowPlanError};
use arcweft_render_wgpu::view_compositor::{ViewCompositorNodePlan, ViewCompositorPlan};
use arcweft_render_wgpu::view_scene::{
    ViewAffine2D, ViewBlendMode, ViewBoxShadow, ViewBoxShadowCorner, ViewBoxShadowCornerRadius,
    ViewBoxShadowKind, ViewBoxShadowList, ViewBoxShadowRadii, ViewBoxShadowRadiusAxis,
    ViewColorRgba8, ViewCompositingEffects, ViewCompositingGroup, ViewPaintNode,
    ViewPrimitiveRange, ViewScene, ViewSceneContext,
};

fn rgba(alpha: u8) -> ViewColorRgba8 {
    ViewColorRgba8 {
        red: 32,
        green: 40,
        blue: 64,
        alpha,
    }
}

fn direct(start: u32, end: u32) -> ViewPaintNode {
    ViewPaintNode::Direct(ViewSceneContext {
        transform: ViewAffine2D::IDENTITY,
        opacity: 1.0,
        clip: None,
        primitive_range: ViewPrimitiveRange { start, end },
    })
}

fn per_corner_radii() -> ViewBoxShadowRadii {
    ViewBoxShadowRadii::from_corners(
        ViewBoxShadowCornerRadius::new(16.0, 8.0),
        ViewBoxShadowCornerRadius::new(4.0, 12.0),
        ViewBoxShadowCornerRadius::new(20.0, 10.0),
        ViewBoxShadowCornerRadius::new(6.0, 18.0),
    )
}

#[test]
fn outer_box_shadow_plans_before_children_and_expands_visual_extent() {
    let mut scene = ViewScene::new(320.0, 180.0);
    let effects = ViewCompositingEffects {
        box_shadows: ViewBoxShadowList::new([ViewBoxShadow::outer(
            4.0,
            8.0,
            12.0,
            2.0,
            10.0,
            rgba(180),
        )]),
        ..ViewCompositingEffects::default()
    };
    scene.push_paint_node(ViewPaintNode::Group(
        ViewCompositingGroup::new(HitRect::new(40.0, 30.0, 120.0, 60.0), effects)
            .with_children(vec![direct(0, 1)]),
    ));

    let plan = ViewCompositorPlan::from_scene(&scene, 1.0);
    let ViewCompositorNodePlan::Group {
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
    let shadows =
        ViewBoxShadowList::new([ViewBoxShadow::inset(0.0, 2.0, 6.0, 0.0, 8.0, rgba(190))]);

    let plan = ViewBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 80.0, 40.0))
        .expect("inset shadow should plan after seq06.13e");

    assert_eq!(plan.passes().len(), 1);
    assert_eq!(plan.passes()[0].shadow.kind, ViewBoxShadowKind::Inset);
    assert!(plan.visual_outset_px().abs() <= f32::EPSILON);
    assert!(plan.visual_inset_px() > 0.0);
}

#[test]
fn mixed_outer_and_inset_shadows_preserve_stage_metadata() {
    let shadows = ViewBoxShadowList::new([
        ViewBoxShadow::outer(0.0, 8.0, 12.0, 2.0, 8.0, rgba(120)),
        ViewBoxShadow::inset(0.0, 2.0, 6.0, 1.0, 8.0, rgba(190)),
    ]);

    let plan = ViewBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 80.0, 40.0))
        .expect("mixed shadows should plan");

    assert_eq!(
        plan.passes()
            .iter()
            .map(|pass| pass.shadow_index)
            .collect::<Vec<_>>(),
        vec![1, 0]
    );
    assert_eq!(
        plan.passes_for_kind(ViewBoxShadowKind::Outer)
            .map(|pass| pass.shadow_index)
            .collect::<Vec<_>>(),
        vec![0]
    );
    assert_eq!(
        plan.passes_for_kind(ViewBoxShadowKind::Inset)
            .map(|pass| pass.shadow_index)
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert!(plan.visual_outset_px() > 0.0);
    assert!(plan.visual_inset_px() > 0.0);
}

#[test]
fn outer_shadow_plan_preserves_per_corner_radii() {
    let shadows = ViewBoxShadowList::new([ViewBoxShadow::outer_with_radii(
        0.0,
        8.0,
        12.0,
        2.0,
        per_corner_radii(),
        rgba(160),
    )]);

    let plan = ViewBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 160.0, 90.0))
        .expect("per-corner outer shadow plans");

    assert_eq!(plan.passes()[0].body_radii, per_corner_radii());
    assert_eq!(
        plan.passes()[0].shadow_radii,
        ViewBoxShadowRadii::from_corners(
            ViewBoxShadowCornerRadius::new(18.0, 10.0),
            ViewBoxShadowCornerRadius::new(6.0, 14.0),
            ViewBoxShadowCornerRadius::new(22.0, 12.0),
            ViewBoxShadowCornerRadius::new(8.0, 20.0),
        )
    );
}

#[test]
fn inset_shadow_plan_preserves_elliptical_radii() {
    let shadows = ViewBoxShadowList::new([ViewBoxShadow::inset_with_radii(
        0.0,
        2.0,
        6.0,
        1.0,
        per_corner_radii(),
        rgba(190),
    )]);

    let plan = ViewBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 160.0, 90.0))
        .expect("elliptical inset shadow plans");

    assert_eq!(plan.passes()[0].body_radii, per_corner_radii());
    assert_eq!(
        plan.passes()[0].shadow_radii,
        ViewBoxShadowRadii::from_corners(
            ViewBoxShadowCornerRadius::new(15.0, 7.0),
            ViewBoxShadowCornerRadius::new(3.0, 11.0),
            ViewBoxShadowCornerRadius::new(19.0, 9.0),
            ViewBoxShadowCornerRadius::new(5.0, 17.0),
        )
    );
}

#[test]
fn oversized_mixed_corner_radii_are_css_normalized() {
    let radii = ViewBoxShadowRadii::from_corners(
        ViewBoxShadowCornerRadius::new(90.0, 60.0),
        ViewBoxShadowCornerRadius::new(90.0, 60.0),
        ViewBoxShadowCornerRadius::new(90.0, 60.0),
        ViewBoxShadowCornerRadius::new(90.0, 60.0),
    );
    let shadows = ViewBoxShadowList::new([ViewBoxShadow::outer_with_radii(
        0.0,
        8.0,
        12.0,
        0.0,
        radii,
        rgba(160),
    )]);

    let plan = ViewBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 100.0, 50.0))
        .expect("oversized radii normalize");

    assert_eq!(
        plan.passes()[0].body_radii,
        ViewBoxShadowRadii::from_corners(
            ViewBoxShadowCornerRadius::new(37.5, 25.0),
            ViewBoxShadowCornerRadius::new(37.5, 25.0),
            ViewBoxShadowCornerRadius::new(37.5, 25.0),
            ViewBoxShadowCornerRadius::new(37.5, 25.0),
        )
    );
}

#[test]
fn non_finite_radius_is_typed_diagnostic() {
    let radii = ViewBoxShadowRadii::from_corners(
        ViewBoxShadowCornerRadius::new(16.0, 8.0),
        ViewBoxShadowCornerRadius::new(4.0, f32::INFINITY),
        ViewBoxShadowCornerRadius::new(20.0, 10.0),
        ViewBoxShadowCornerRadius::new(6.0, 18.0),
    );
    let shadows = ViewBoxShadowList::new([ViewBoxShadow::outer_with_radii(
        0.0,
        8.0,
        12.0,
        2.0,
        radii,
        rgba(160),
    )]);

    assert_eq!(
        ViewBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 160.0, 90.0)),
        Err(ViewBoxShadowPlanError::NonFiniteRadius {
            shadow_index: 0,
            corner: ViewBoxShadowCorner::TopRight,
            axis: ViewBoxShadowRadiusAxis::Y,
        })
    );
}

#[test]
fn negative_radius_is_typed_diagnostic() {
    let radii = ViewBoxShadowRadii::from_corners(
        ViewBoxShadowCornerRadius::new(16.0, 8.0),
        ViewBoxShadowCornerRadius::new(4.0, 12.0),
        ViewBoxShadowCornerRadius::new(-2.0, 10.0),
        ViewBoxShadowCornerRadius::new(6.0, 18.0),
    );
    let shadows = ViewBoxShadowList::new([ViewBoxShadow::outer_with_radii(
        0.0,
        8.0,
        12.0,
        2.0,
        radii,
        rgba(160),
    )]);

    assert_eq!(
        ViewBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(0.0, 0.0, 160.0, 90.0)),
        Err(ViewBoxShadowPlanError::DegenerateRadius {
            shadow_index: 0,
            corner: ViewBoxShadowCorner::BottomRight,
            axis: ViewBoxShadowRadiusAxis::X,
            value: -2.0,
            reason: "corner radius cannot be negative",
        })
    );
}

#[test]
fn hsl_family_blends_remain_supported() {
    for mode in [
        ViewBlendMode::Hue,
        ViewBlendMode::Saturation,
        ViewBlendMode::Color,
        ViewBlendMode::Luminosity,
    ] {
        assert!(supported_blend_modes().contains(&mode));
        assert!(ViewBlendPassPlan::from_mode(mode).is_some());
    }
    assert_eq!(
        ViewBlendPassPlan::from_mode(ViewBlendMode::Luminosity).map(|plan| plan.shader_mode),
        Some(ViewBlendShaderMode::Luminosity)
    );
}
