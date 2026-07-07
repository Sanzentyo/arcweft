use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::view_clip_path::{
    MAX_CLIP_POLYGON_VERTICES, ViewClipGeometryPlan, ViewClipPathCommandPlan, ViewClipPathPlanError,
};
use arcweft_render_wgpu::view_compositor::{ViewCompositorNodePlan, ViewCompositorPlan};
use arcweft_render_wgpu::view_effects::ViewTextureExtent;
use arcweft_render_wgpu::view_mask::{
    ViewMaskAxisRepeat, ViewMaskChainPlan, ViewMaskChannel, ViewMaskGradientPlan, ViewMaskPlanError,
};
use arcweft_render_wgpu::view_scene::{
    ViewAffine2D, ViewClipPath, ViewColorRgba8, ViewCompositingEffects, ViewCompositingGroup,
    ViewElementMaskSource, ViewFillRule, ViewGradientStop, ViewLength, ViewMask, ViewMaskGradient,
    ViewMaskImage, ViewMaskPosition, ViewMaskRepeat, ViewMaskSize, ViewPaintNode, ViewPoint,
    ViewPrimitiveRange, ViewScene, ViewSceneContext, ViewShapeRadius,
};

fn direct() -> ViewPaintNode {
    ViewPaintNode::Direct(ViewSceneContext {
        transform: ViewAffine2D::IDENTITY,
        opacity: 1.0,
        clip: None,
        primitive_range: ViewPrimitiveRange { start: 0, end: 1 },
    })
}

fn color(red: u8, green: u8, blue: u8, alpha: u8) -> ViewColorRgba8 {
    ViewColorRgba8 {
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

    let inset = ViewClipGeometryPlan::from_clip_path(
        Some(&ViewClipPath::Inset {
            inset: [
                ViewLength::Px(4.0),
                ViewLength::Px(8.0),
                ViewLength::Px(12.0),
                ViewLength::Px(16.0),
            ],
            radius: [
                ViewLength::Px(2.0),
                ViewLength::Px(2.0),
                ViewLength::Px(2.0),
                ViewLength::Px(2.0),
            ],
        }),
        bounds,
    )
    .expect("inset resolves");
    assert!(inset.requires_geometry_pass());

    let circle = ViewClipGeometryPlan::from_clip_path(
        Some(&ViewClipPath::Circle {
            radius: ViewShapeRadius::ClosestSide,
            center: ViewPoint::percent(0.5, 0.5),
        }),
        bounds,
    )
    .expect("circle resolves");
    assert!(matches!(circle, ViewClipGeometryPlan::Ellipse { .. }));

    let polygon = ViewClipGeometryPlan::from_clip_path(
        Some(&ViewClipPath::Polygon {
            fill_rule: ViewFillRule::EvenOdd,
            points: vec![
                ViewPoint::percent(0.0, 0.0),
                ViewPoint::percent(1.0, 0.0),
                ViewPoint::percent(0.5, 1.0),
            ],
        }),
        bounds,
    )
    .expect("polygon resolves");
    assert!(matches!(
        polygon,
        ViewClipGeometryPlan::Polygon {
            fill_rule: ViewFillRule::EvenOdd,
            ..
        }
    ));
}

#[test]
fn path_clips_lines_curves_and_both_fill_rules() {
    let data = "M0 0 L80 0 Q100 20 80 40 C50 70 20 70 0 40 Z";
    for fill_rule in [ViewFillRule::NonZero, ViewFillRule::EvenOdd] {
        let plan = ViewClipGeometryPlan::from_clip_path(
            Some(&ViewClipPath::Path {
                fill_rule,
                data: data.into(),
            }),
            HitRect::new(5.0, 7.0, 100.0, 80.0),
        )
        .expect("path resolves");
        let ViewClipGeometryPlan::Path {
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
                .any(|command| matches!(command, ViewClipPathCommandPlan::QuadraticTo { .. }))
        );
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, ViewClipPathCommandPlan::CubicTo { .. }))
        );
        assert!(edges.len() > 4, "curves should flatten into multiple edges");
    }
}

#[test]
fn path_and_oversized_polygon_stay_structured_diagnostics() {
    assert_eq!(
        ViewClipGeometryPlan::from_clip_path(
            Some(&ViewClipPath::Path {
                fill_rule: ViewFillRule::NonZero,
                data: "M0 0 A20 20 0 0 1 30 30".into(),
            }),
            HitRect::new(0.0, 0.0, 32.0, 32.0),
        ),
        Err(ViewClipPathPlanError::UnsupportedPathCommand { command: 'A' })
    );

    assert_eq!(
        ViewClipGeometryPlan::from_clip_path(
            Some(&ViewClipPath::Path {
                fill_rule: ViewFillRule::NonZero,
                data: "M0 0 L0 0".into(),
            }),
            HitRect::new(0.0, 0.0, 32.0, 32.0),
        ),
        Err(ViewClipPathPlanError::DegeneratePathSegment {
            command: 'L',
            index: 0,
        })
    );

    let points = (0..=MAX_CLIP_POLYGON_VERTICES)
        .map(|_| ViewPoint::percent(0.0, 0.0))
        .collect::<Vec<_>>();
    assert_eq!(
        ViewClipGeometryPlan::from_clip_path(
            Some(&ViewClipPath::Polygon {
                fill_rule: ViewFillRule::NonZero,
                points,
            }),
            HitRect::new(0.0, 0.0, 32.0, 32.0),
        ),
        Err(ViewClipPathPlanError::TooManyPolygonVertices {
            count: MAX_CLIP_POLYGON_VERTICES + 1,
            maximum: MAX_CLIP_POLYGON_VERTICES,
        })
    );
}

#[test]
fn mask_sampling_resolves_size_position_repeat_and_channel() {
    let masks = [ViewMask {
        image: ViewMaskImage::Url("arcweft://mask/dialogue-card".into()),
        size: ViewMaskSize::Explicit {
            width: ViewLength::Percent(0.5),
            height: ViewLength::Px(20.0),
        },
        position: ViewMaskPosition {
            anchor: ViewPoint::percent(1.0, 0.5),
        },
        repeat: ViewMaskRepeat::RepeatX,
    }];
    let plan = ViewMaskChainPlan::from_masks(&masks, ViewMaskChannel::Luminance);
    let sampling = plan.passes()[0]
        .sampling_plan(
            ViewTextureExtent::new(200, 100),
            ViewTextureExtent::new(16, 16),
        )
        .expect("mask sampling resolves");

    assert_near_pair(sampling.tile_size_px, [100.0, 20.0]);
    assert_near_pair(sampling.tile_origin_px, [100.0, 40.0]);
    assert_eq!(sampling.repeat_mode_x, ViewMaskAxisRepeat::Repeat);
    assert_eq!(sampling.repeat_mode_y, ViewMaskAxisRepeat::NoRepeat);
    assert!(sampling.repeat_x);
    assert!(!sampling.repeat_y);
    assert_eq!(plan.passes()[0].channel, ViewMaskChannel::Luminance);
}

#[test]
fn mask_repeat_space_and_round_resolve_tile_distribution() {
    let space = ViewMask {
        image: ViewMaskImage::Url("arcweft://mask/space".into()),
        size: ViewMaskSize::Explicit {
            width: ViewLength::Px(30.0),
            height: ViewLength::Px(20.0),
        },
        repeat: ViewMaskRepeat::Space,
        ..ViewMask::default()
    };
    let plan = ViewMaskChainPlan::from_masks(&[space], ViewMaskChannel::Alpha);
    let sampling = plan.passes()[0]
        .sampling_plan(
            ViewTextureExtent::new(100, 60),
            ViewTextureExtent::new(10, 10),
        )
        .expect("space repeat resolves");
    assert_eq!(sampling.repeat_mode_x, ViewMaskAxisRepeat::Space);
    assert_eq!(sampling.tile_count[0], 3);
    assert!((sampling.tile_stride_px[0] - 35.0).abs() <= 0.001);

    let round = ViewMask {
        image: ViewMaskImage::Url("arcweft://mask/round".into()),
        size: ViewMaskSize::Explicit {
            width: ViewLength::Px(30.0),
            height: ViewLength::Px(20.0),
        },
        repeat: ViewMaskRepeat::Round,
        ..ViewMask::default()
    };
    let plan = ViewMaskChainPlan::from_masks(&[round], ViewMaskChannel::Alpha);
    let sampling = plan.passes()[0]
        .sampling_plan(
            ViewTextureExtent::new(100, 60),
            ViewTextureExtent::new(10, 10),
        )
        .expect("round repeat resolves");
    assert_eq!(sampling.repeat_mode_x, ViewMaskAxisRepeat::Round);
    assert_eq!(sampling.tile_count[0], 3);
    assert!((sampling.tile_size_px[0] - 33.333).abs() <= 0.01);
}

#[test]
fn gradient_mask_alpha_and_luminance_modes_produce_different_coverage() {
    let gradient = ViewMaskGradient::Linear {
        angle_degrees: 90.0,
        stops: vec![
            ViewGradientStop {
                offset: 0.0,
                color: color(255, 0, 0, 255),
            },
            ViewGradientStop {
                offset: 1.0,
                color: color(0, 0, 0, 0),
            },
        ],
    };
    let gradient_plan =
        ViewMaskGradientPlan::from_gradient(&gradient, [128.0, 64.0]).expect("gradient resolves");
    assert!((gradient_plan.stops[0].alpha_coverage - 1.0).abs() <= f32::EPSILON);
    assert!((gradient_plan.stops[0].luminance_coverage - 0.2126).abs() <= 0.0001);
}

#[test]
fn element_mask_is_structured_capture_diagnostic() {
    let element = ViewMask {
        image: ViewMaskImage::Element(ViewElementMaskSource {
            element_id: "speaker-portrait".into(),
        }),
        ..ViewMask::default()
    };
    let plan = ViewMaskChainPlan::from_masks(&[element], ViewMaskChannel::Alpha);
    assert_eq!(plan.unsupported_count(), 1);
    assert_eq!(
        plan.passes()[0]
            .sampling_plan(
                ViewTextureExtent::new(64, 64),
                ViewTextureExtent::new(64, 64)
            )
            .expect_err("element mask needs capture resource"),
        ViewMaskPlanError::ElementMaskCaptureUnavailable {
            element_id: "speaker-portrait".into(),
        }
    );
}

#[test]
fn compositor_plan_counts_path_clip_and_gradient_mask_passes_deterministically() {
    let mut scene = ViewScene::new(320.0, 180.0);
    let group = ViewCompositingGroup::new(
        HitRect::new(20.0, 20.0, 96.0, 64.0),
        ViewCompositingEffects {
            clip_path: Some(Box::new(ViewClipPath::Path {
                fill_rule: ViewFillRule::EvenOdd,
                data: "M0 0 L96 0 Q96 64 48 64 L0 64 Z".into(),
            })),
            masks: vec![ViewMask {
                image: ViewMaskImage::Gradient(ViewMaskGradient::Linear {
                    angle_degrees: 0.0,
                    stops: vec![
                        ViewGradientStop {
                            offset: 0.0,
                            color: color(255, 255, 255, 255),
                        },
                        ViewGradientStop {
                            offset: 1.0,
                            color: color(0, 0, 0, 0),
                        },
                    ],
                }),
                repeat: ViewMaskRepeat::NoRepeat,
                ..ViewMask::default()
            }],
            ..ViewCompositingEffects::default()
        },
    )
    .with_children(vec![direct()]);
    scene.push_paint_node(ViewPaintNode::Group(group));

    let plan = ViewCompositorPlan::from_scene(&scene, 1.0);
    let ViewCompositorNodePlan::Group { effects, .. } = &plan.nodes()[0] else {
        panic!("expected group node");
    };
    assert!(matches!(
        effects.clip_path.as_ref().expect("clip plans"),
        ViewClipGeometryPlan::Path { .. }
    ));
    assert_eq!(effects.masks.passes().len(), 1);
    assert!(
        plan.shader_pass_count() >= 3,
        "root composite + clip + mask + group blend"
    );
}
