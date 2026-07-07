//! Retained-UI fixture shape for seq06.13c visual smoke tests.
//!
//! This is documentation-oriented fixture code. Copy into an integration test or
//! host visual harness after applying the overlay to the repository.

use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::view_scene::{
    ViewClipPath, ViewColorRgba8, ViewCompositingEffects, ViewCompositingGroup,
    ViewElementMaskSource, ViewFillRule, ViewGradientStop, ViewLength, ViewMask, ViewMaskGradient,
    ViewMaskImage, ViewMaskRepeat, ViewMaskSize, ViewPaintNode, ViewPrimitiveRange, ViewScene,
    ViewSceneContext,
};

pub fn seq06_13c_scene() -> ViewScene {
    let mut scene = ViewScene::new(320.0, 180.0);
    scene.push_paint_node(ViewPaintNode::Group(
        ViewCompositingGroup::new(
            HitRect::new(24.0, 24.0, 180.0, 96.0),
            ViewCompositingEffects {
                clip_path: Some(Box::new(ViewClipPath::Path {
                    fill_rule: ViewFillRule::EvenOdd,
                    data: "M0 0 L180 0 Q180 96 90 96 L0 96 Z".into(),
                })),
                masks: vec![
                    ViewMask {
                        image: ViewMaskImage::Gradient(ViewMaskGradient::Linear {
                            angle_degrees: 90.0,
                            stops: vec![
                                ViewGradientStop {
                                    offset: 0.0,
                                    color: ViewColorRgba8 {
                                        red: 255,
                                        green: 0,
                                        blue: 0,
                                        alpha: 255,
                                    },
                                },
                                ViewGradientStop {
                                    offset: 1.0,
                                    color: ViewColorRgba8 {
                                        red: 0,
                                        green: 0,
                                        blue: 0,
                                        alpha: 0,
                                    },
                                },
                            ],
                        }),
                        repeat: ViewMaskRepeat::NoRepeat,
                        ..ViewMask::default()
                    },
                    ViewMask {
                        image: ViewMaskImage::Url("arcweft://mask/space-dots".into()),
                        size: ViewMaskSize::Explicit {
                            width: ViewLength::Px(30.0),
                            height: ViewLength::Px(20.0),
                        },
                        repeat: ViewMaskRepeat::Space,
                        ..ViewMask::default()
                    },
                ],
                ..ViewCompositingEffects::default()
            },
        )
        .with_children(vec![ViewPaintNode::Direct(ViewSceneContext {
            transform: Default::default(),
            opacity: 1.0,
            clip: None,
            primitive_range: ViewPrimitiveRange { start: 0, end: 1 },
        })]),
    ));
    scene.push_paint_node(ViewPaintNode::Group(
        ViewCompositingGroup::new(
            HitRect::new(220.0, 24.0, 72.0, 72.0),
            ViewCompositingEffects {
                masks: vec![ViewMask {
                    image: ViewMaskImage::Element(ViewElementMaskSource {
                        element_id: "speaker-portrait".into(),
                    }),
                    ..ViewMask::default()
                }],
                ..ViewCompositingEffects::default()
            },
        )
        .with_children(vec![ViewPaintNode::Direct(ViewSceneContext {
            transform: Default::default(),
            opacity: 1.0,
            clip: None,
            primitive_range: ViewPrimitiveRange { start: 1, end: 2 },
        })]),
    ));
    scene
}
