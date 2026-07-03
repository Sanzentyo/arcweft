//! Retained-UI fixture shape for seq06.13c visual smoke tests.
//!
//! This is documentation-oriented fixture code. Copy into an integration test or
//! host visual harness after applying the overlay to the repository.

use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::ui_scene::{
    UiClipPath, UiColorRgba8, UiCompositingEffects, UiCompositingGroup,
    UiElementMaskSource, UiFillRule, UiGradientStop, UiMask, UiMaskGradient,
    UiMaskImage, UiMaskRepeat, UiMaskSize, UiLength, UiPaintNode, UiPrimitiveRange,
    UiScene, UiSceneContext,
};

pub fn seq06_13c_scene() -> UiScene {
    let mut scene = UiScene::new(320.0, 180.0);
    scene.push_paint_node(UiPaintNode::Group(
        UiCompositingGroup::new(
            HitRect::new(24.0, 24.0, 180.0, 96.0),
            UiCompositingEffects {
                clip_path: Some(Box::new(UiClipPath::Path {
                    fill_rule: UiFillRule::EvenOdd,
                    data: "M0 0 L180 0 Q180 96 90 96 L0 96 Z".into(),
                })),
                masks: vec![
                    UiMask {
                        image: UiMaskImage::Gradient(UiMaskGradient::Linear {
                            angle_degrees: 90.0,
                            stops: vec![
                                UiGradientStop {
                                    offset: 0.0,
                                    color: UiColorRgba8 { red: 255, green: 0, blue: 0, alpha: 255 },
                                },
                                UiGradientStop {
                                    offset: 1.0,
                                    color: UiColorRgba8 { red: 0, green: 0, blue: 0, alpha: 0 },
                                },
                            ],
                        }),
                        repeat: UiMaskRepeat::NoRepeat,
                        ..UiMask::default()
                    },
                    UiMask {
                        image: UiMaskImage::Url("arcweft://mask/space-dots".into()),
                        size: UiMaskSize::Explicit {
                            width: UiLength::Px(30.0),
                            height: UiLength::Px(20.0),
                        },
                        repeat: UiMaskRepeat::Space,
                        ..UiMask::default()
                    },
                ],
                ..UiCompositingEffects::default()
            },
        )
        .with_children(vec![UiPaintNode::Direct(UiSceneContext {
            transform: Default::default(),
            opacity: 1.0,
            clip: None,
            primitive_range: UiPrimitiveRange { start: 0, end: 1 },
        })]),
    ));
    scene.push_paint_node(UiPaintNode::Group(
        UiCompositingGroup::new(
            HitRect::new(220.0, 24.0, 72.0, 72.0),
            UiCompositingEffects {
                masks: vec![UiMask {
                    image: UiMaskImage::Element(UiElementMaskSource {
                        element_id: "speaker-portrait".into(),
                    }),
                    ..UiMask::default()
                }],
                ..UiCompositingEffects::default()
            },
        )
        .with_children(vec![UiPaintNode::Direct(UiSceneContext {
            transform: Default::default(),
            opacity: 1.0,
            clip: None,
            primitive_range: UiPrimitiveRange { start: 1, end: 2 },
        })]),
    ));
    scene
}
