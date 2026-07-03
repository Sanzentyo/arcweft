use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::ui_clip_path::{
    MAX_CLIP_POLYGON_VERTICES, UiClipGeometryPlan, UiClipPathPlanError,
};
use arcweft_render_wgpu::ui_compositor::{UiCompositorNodePlan, UiCompositorPlan};
use arcweft_render_wgpu::ui_effects::UiTextureExtent;
use arcweft_render_wgpu::ui_mask::{UiMaskChainPlan, UiMaskChannel, UiMaskPlanError};
use arcweft_render_wgpu::ui_scene::{
    UiAffine2, UiClipPath, UiCompositingEffects, UiCompositingGroup, UiFillRule, UiLength, UiMask,
    UiMaskImage, UiMaskPosition, UiMaskRepeat, UiMaskSize, UiPaintNode, UiPoint, UiPrimitiveRange,
    UiScene, UiSceneContext, UiShapeRadius,
};

fn direct() -> UiPaintNode {
    UiPaintNode::Direct(UiSceneContext {
        transform: UiAffine2::IDENTITY,
        opacity: 1.0,
        clip: None,
        primitive_range: UiPrimitiveRange { start: 0, end: 1 },
    })
}

fn assert_near_pair(actual: [f32; 2], expected: [f32; 2]) {
    const EPSILON: f32 = 0.001;
    assert!(
        (actual[0] - expected[0]).abs() <= EPSILON,
        "{actual:?} != {expected:?}"
    );
    assert!(
        (actual[1] - expected[1]).abs() <= EPSILON,
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn inset_circle_ellipse_and_polygon_clips_plan_as_shader_geometry() {
    let bounds = HitRect::new(10.0, 20.0, 100.0, 80.0);

    let inset = UiClipGeometryPlan::from_clip_path(
        Some(&UiClipPath::Inset {
            inset: [
                UiLength::Px(4.0),
                UiLength::Px(8.0),
                UiLength::Px(12.0),
                UiLength::Px(16.0),
            ],
            radius: [
                UiLength::Px(2.0),
                UiLength::Px(2.0),
                UiLength::Px(2.0),
                UiLength::Px(2.0),
            ],
        }),
        bounds,
    )
    .expect("inset resolves");
    assert!(inset.requires_geometry_pass());

    let circle = UiClipGeometryPlan::from_clip_path(
        Some(&UiClipPath::Circle {
            radius: UiShapeRadius::ClosestSide,
            center: UiPoint::percent(0.5, 0.5),
        }),
        bounds,
    )
    .expect("circle resolves");
    assert!(matches!(circle, UiClipGeometryPlan::Ellipse { .. }));

    let polygon = UiClipGeometryPlan::from_clip_path(
        Some(&UiClipPath::Polygon {
            fill_rule: UiFillRule::EvenOdd,
            points: vec![
                UiPoint::percent(0.0, 0.0),
                UiPoint::percent(1.0, 0.0),
                UiPoint::percent(0.5, 1.0),
            ],
        }),
        bounds,
    )
    .expect("polygon resolves");
    assert!(matches!(
        polygon,
        UiClipGeometryPlan::Polygon {
            fill_rule: UiFillRule::EvenOdd,
            ..
        }
    ));
}

#[test]
fn path_and_oversized_polygon_stay_structured_diagnostics() {
    assert_eq!(
        UiClipGeometryPlan::from_clip_path(
            Some(&UiClipPath::Path {
                fill_rule: UiFillRule::NonZero,
                data: "M0 0 L1 1".into(),
            }),
            HitRect::new(0.0, 0.0, 32.0, 32.0),
        ),
        Err(UiClipPathPlanError::PathUnsupported)
    );

    let points = (0..=MAX_CLIP_POLYGON_VERTICES)
        .map(|_| UiPoint::percent(0.0, 0.0))
        .collect::<Vec<_>>();
    assert_eq!(
        UiClipGeometryPlan::from_clip_path(
            Some(&UiClipPath::Polygon {
                fill_rule: UiFillRule::NonZero,
                points,
            }),
            HitRect::new(0.0, 0.0, 32.0, 32.0),
        ),
        Err(UiClipPathPlanError::TooManyPolygonVertices {
            count: MAX_CLIP_POLYGON_VERTICES + 1,
            maximum: MAX_CLIP_POLYGON_VERTICES,
        })
    );
}

#[test]
fn mask_sampling_resolves_size_position_repeat_and_channel() {
    let masks = [UiMask {
        image: UiMaskImage::Url("arcweft://mask/dialogue-card".into()),
        size: UiMaskSize::Explicit {
            width: UiLength::Percent(0.5),
            height: UiLength::Px(20.0),
        },
        position: UiMaskPosition {
            anchor: UiPoint::percent(1.0, 0.5),
        },
        repeat: UiMaskRepeat::RepeatX,
    }];
    let plan = UiMaskChainPlan::from_masks(&masks, UiMaskChannel::Luminance);
    let sampling = plan.passes()[0]
        .sampling_plan(UiTextureExtent::new(200, 100), UiTextureExtent::new(16, 16))
        .expect("mask sampling resolves");

    assert_near_pair(sampling.tile_size_px, [100.0, 20.0]);
    assert_near_pair(sampling.tile_origin_px, [100.0, 40.0]);
    assert!(sampling.repeat_x);
    assert!(!sampling.repeat_y);
    assert_eq!(plan.passes()[0].channel, UiMaskChannel::Luminance);
}

#[test]
fn unsupported_mask_images_and_space_round_repeat_are_diagnostics() {
    let unsupported_image = UiMask {
        image: UiMaskImage::Unsupported("gradient mask image".into()),
        ..UiMask::default()
    };
    let plan = UiMaskChainPlan::from_masks(&[unsupported_image], UiMaskChannel::Alpha);
    assert_eq!(plan.unsupported_count(), 1);
    assert_eq!(
        plan.passes()[0]
            .sampling_plan(UiTextureExtent::new(64, 64), UiTextureExtent::new(8, 8))
            .expect_err("unsupported image is rejected"),
        UiMaskPlanError::UnsupportedImage("gradient mask image".into())
    );

    let repeat_space = UiMask {
        image: UiMaskImage::Url("arcweft://mask/space".into()),
        repeat: UiMaskRepeat::Space,
        ..UiMask::default()
    };
    let plan = UiMaskChainPlan::from_masks(&[repeat_space], UiMaskChannel::Alpha);
    assert_eq!(plan.unsupported_count(), 1);
    assert!(matches!(
        plan.passes()[0].sampling_plan(UiTextureExtent::new(64, 64), UiTextureExtent::new(8, 8)),
        Err(UiMaskPlanError::UnsupportedRepeat(_))
    ));
}

#[test]
fn compositor_plan_counts_clip_and_mask_passes_deterministically() {
    let mut scene = UiScene::new(320.0, 180.0);
    let group = UiCompositingGroup::new(
        HitRect::new(20.0, 20.0, 96.0, 64.0),
        UiCompositingEffects {
            clip_path: Some(Box::new(UiClipPath::Ellipse {
                radius_x: UiShapeRadius::Length(UiLength::Percent(0.5)),
                radius_y: UiShapeRadius::Length(UiLength::Percent(0.5)),
                center: UiPoint::percent(0.5, 0.5),
            })),
            masks: vec![UiMask {
                image: UiMaskImage::Url("arcweft://mask/dialogue-card".into()),
                repeat: UiMaskRepeat::NoRepeat,
                ..UiMask::default()
            }],
            ..UiCompositingEffects::default()
        },
    )
    .with_children(vec![direct()]);
    scene.push_paint_node(UiPaintNode::Group(group));

    let plan = UiCompositorPlan::from_scene(&scene, 1.0);
    let UiCompositorNodePlan::Group { effects, .. } = &plan.nodes()[0] else {
        panic!("expected group node");
    };
    assert!(
        effects
            .clip_path
            .as_ref()
            .expect("clip plans")
            .requires_geometry_pass()
    );
    assert_eq!(effects.masks.passes().len(), 1);
    assert!(
        plan.shader_pass_count() >= 3,
        "root composite + clip + mask + group blend"
    );
}
