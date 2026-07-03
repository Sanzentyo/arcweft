use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::ui_clip_path::{
    MAX_CLIP_POLYGON_VERTICES, UiClipGeometryPlan, UiClipPathCommandPlan, UiClipPathPlanError,
};
use arcweft_render_wgpu::ui_compositor::{UiCompositorNodePlan, UiCompositorPlan};
use arcweft_render_wgpu::ui_effects::UiTextureExtent;
use arcweft_render_wgpu::ui_mask::{
    UiMaskAxisRepeat, UiMaskChainPlan, UiMaskChannel, UiMaskGradientPlan, UiMaskPlanError,
};
use arcweft_render_wgpu::ui_scene::{
    UiAffine2, UiClipPath, UiColorRgba8, UiCompositingEffects, UiCompositingGroup,
    UiElementMaskSource, UiFillRule, UiGradientStop, UiLength, UiMask, UiMaskGradient, UiMaskImage,
    UiMaskPosition, UiMaskRepeat, UiMaskSize, UiPaintNode, UiPoint, UiPrimitiveRange, UiScene,
    UiSceneContext, UiShapeRadius,
};

fn direct() -> UiPaintNode {
    UiPaintNode::Direct(UiSceneContext {
        transform: UiAffine2::IDENTITY,
        opacity: 1.0,
        clip: None,
        primitive_range: UiPrimitiveRange { start: 0, end: 1 },
    })
}

fn color(red: u8, green: u8, blue: u8, alpha: u8) -> UiColorRgba8 {
    UiColorRgba8 {
        red,
        green,
        blue,
        alpha,
    }
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
fn path_clips_lines_curves_and_both_fill_rules() {
    let data = "M0 0 L80 0 Q100 20 80 40 C50 70 20 70 0 40 Z";
    for fill_rule in [UiFillRule::NonZero, UiFillRule::EvenOdd] {
        let plan = UiClipGeometryPlan::from_clip_path(
            Some(&UiClipPath::Path {
                fill_rule,
                data: data.into(),
            }),
            HitRect::new(5.0, 7.0, 100.0, 80.0),
        )
        .expect("path resolves");
        let UiClipGeometryPlan::Path {
            fill_rule: planned_rule,
            commands,
            edges,
        } = plan
        else {
            panic!("expected path plan");
        };
        assert_eq!(planned_rule, fill_rule);
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, UiClipPathCommandPlan::QuadraticTo { .. }))
        );
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, UiClipPathCommandPlan::CubicTo { .. }))
        );
        assert!(edges.len() > 4, "curves should flatten into multiple edges");
    }
}

#[test]
fn path_and_oversized_polygon_stay_structured_diagnostics() {
    assert_eq!(
        UiClipGeometryPlan::from_clip_path(
            Some(&UiClipPath::Path {
                fill_rule: UiFillRule::NonZero,
                data: "M0 0 A20 20 0 0 1 30 30".into(),
            }),
            HitRect::new(0.0, 0.0, 32.0, 32.0),
        ),
        Err(UiClipPathPlanError::UnsupportedPathCommand { command: 'A' })
    );

    assert_eq!(
        UiClipGeometryPlan::from_clip_path(
            Some(&UiClipPath::Path {
                fill_rule: UiFillRule::NonZero,
                data: "M0 0 L0 0".into(),
            }),
            HitRect::new(0.0, 0.0, 32.0, 32.0),
        ),
        Err(UiClipPathPlanError::DegeneratePathSegment {
            command: 'L',
            index: 0,
        })
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
    assert_eq!(sampling.repeat_mode_x, UiMaskAxisRepeat::Repeat);
    assert_eq!(sampling.repeat_mode_y, UiMaskAxisRepeat::NoRepeat);
    assert!(sampling.repeat_x);
    assert!(!sampling.repeat_y);
    assert_eq!(plan.passes()[0].channel, UiMaskChannel::Luminance);
}

#[test]
fn mask_repeat_space_and_round_resolve_tile_distribution() {
    let space = UiMask {
        image: UiMaskImage::Url("arcweft://mask/space".into()),
        size: UiMaskSize::Explicit {
            width: UiLength::Px(30.0),
            height: UiLength::Px(20.0),
        },
        repeat: UiMaskRepeat::Space,
        ..UiMask::default()
    };
    let plan = UiMaskChainPlan::from_masks(&[space], UiMaskChannel::Alpha);
    let sampling = plan.passes()[0]
        .sampling_plan(UiTextureExtent::new(100, 60), UiTextureExtent::new(10, 10))
        .expect("space repeat resolves");
    assert_eq!(sampling.repeat_mode_x, UiMaskAxisRepeat::Space);
    assert_eq!(sampling.tile_count[0], 3);
    assert!((sampling.tile_stride_px[0] - 35.0).abs() <= 0.001);

    let round = UiMask {
        image: UiMaskImage::Url("arcweft://mask/round".into()),
        size: UiMaskSize::Explicit {
            width: UiLength::Px(30.0),
            height: UiLength::Px(20.0),
        },
        repeat: UiMaskRepeat::Round,
        ..UiMask::default()
    };
    let plan = UiMaskChainPlan::from_masks(&[round], UiMaskChannel::Alpha);
    let sampling = plan.passes()[0]
        .sampling_plan(UiTextureExtent::new(100, 60), UiTextureExtent::new(10, 10))
        .expect("round repeat resolves");
    assert_eq!(sampling.repeat_mode_x, UiMaskAxisRepeat::Round);
    assert_eq!(sampling.tile_count[0], 3);
    assert!((sampling.tile_size_px[0] - 33.333).abs() <= 0.01);
}

#[test]
fn gradient_mask_alpha_and_luminance_modes_produce_different_coverage() {
    let gradient = UiMaskGradient::Linear {
        angle_degrees: 90.0,
        stops: vec![
            UiGradientStop {
                offset: 0.0,
                color: color(255, 0, 0, 255),
            },
            UiGradientStop {
                offset: 1.0,
                color: color(0, 0, 0, 0),
            },
        ],
    };
    let gradient_plan =
        UiMaskGradientPlan::from_gradient(&gradient, [128.0, 64.0]).expect("gradient resolves");
    assert!((gradient_plan.stops[0].alpha_coverage - 1.0).abs() <= f32::EPSILON);
    assert!((gradient_plan.stops[0].luminance_coverage - 0.2126).abs() <= 0.0001);
}

#[test]
fn element_mask_is_structured_capture_diagnostic() {
    let element = UiMask {
        image: UiMaskImage::Element(UiElementMaskSource {
            element_id: "speaker-portrait".into(),
        }),
        ..UiMask::default()
    };
    let plan = UiMaskChainPlan::from_masks(&[element], UiMaskChannel::Alpha);
    assert_eq!(plan.unsupported_count(), 1);
    assert_eq!(
        plan.passes()[0]
            .sampling_plan(UiTextureExtent::new(64, 64), UiTextureExtent::new(64, 64))
            .expect_err("element mask needs capture resource"),
        UiMaskPlanError::ElementMaskCaptureUnavailable {
            element_id: "speaker-portrait".into(),
        }
    );
}

#[test]
fn compositor_plan_counts_path_clip_and_gradient_mask_passes_deterministically() {
    let mut scene = UiScene::new(320.0, 180.0);
    let group = UiCompositingGroup::new(
        HitRect::new(20.0, 20.0, 96.0, 64.0),
        UiCompositingEffects {
            clip_path: Some(Box::new(UiClipPath::Path {
                fill_rule: UiFillRule::EvenOdd,
                data: "M0 0 L96 0 Q96 64 48 64 L0 64 Z".into(),
            })),
            masks: vec![UiMask {
                image: UiMaskImage::Gradient(UiMaskGradient::Linear {
                    angle_degrees: 0.0,
                    stops: vec![
                        UiGradientStop {
                            offset: 0.0,
                            color: color(255, 255, 255, 255),
                        },
                        UiGradientStop {
                            offset: 1.0,
                            color: color(0, 0, 0, 0),
                        },
                    ],
                }),
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
    assert!(matches!(
        effects.clip_path.as_ref().expect("clip plans"),
        UiClipGeometryPlan::Path { .. }
    ));
    assert_eq!(effects.masks.passes().len(), 1);
    assert!(
        plan.shader_pass_count() >= 3,
        "root composite + clip + mask + group blend"
    );
}
