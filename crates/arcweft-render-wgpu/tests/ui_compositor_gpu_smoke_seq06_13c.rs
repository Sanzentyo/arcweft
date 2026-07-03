//! Optional visual smoke fixture for seq06.13c.
//!
//! This test is intentionally ignored until native/web pinned readback exists in CI.
//! It documents the retained UI scene shape that should be captured on both hosts.

use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::ui_scene::{
    UiAffine2, UiClipPath, UiColorRgba8, UiCompositingEffects, UiCompositingGroup, UiFillRule,
    UiGradientStop, UiMask, UiMaskGradient, UiMaskImage, UiMaskRepeat, UiPaintNode,
    UiPrimitiveRange, UiScene, UiSceneContext,
};

#[test]
#[ignore = "requires native/web pinned GPU readback harness"]
fn seq06_13c_path_clip_gradient_mask_visual_smoke_fixture() {
    let mut scene = UiScene::new(240.0, 160.0);
    scene.push_paint_node(UiPaintNode::Group(
        UiCompositingGroup::new(
            HitRect::new(20.0, 20.0, 160.0, 96.0),
            UiCompositingEffects {
                clip_path: Some(Box::new(UiClipPath::Path {
                    fill_rule: UiFillRule::EvenOdd,
                    data: "M0 0 C80 12 120 12 160 0 L160 96 L0 96 Z".into(),
                })),
                masks: vec![UiMask {
                    image: UiMaskImage::Gradient(UiMaskGradient::Linear {
                        angle_degrees: 90.0,
                        stops: vec![
                            UiGradientStop {
                                offset: 0.0,
                                color: UiColorRgba8 {
                                    red: 255,
                                    green: 255,
                                    blue: 255,
                                    alpha: 255,
                                },
                            },
                            UiGradientStop {
                                offset: 1.0,
                                color: UiColorRgba8 {
                                    red: 0,
                                    green: 0,
                                    blue: 0,
                                    alpha: 64,
                                },
                            },
                        ],
                    }),
                    repeat: UiMaskRepeat::NoRepeat,
                    ..UiMask::default()
                }],
                ..UiCompositingEffects::default()
            },
        )
        .with_children(vec![UiPaintNode::Direct(UiSceneContext {
            transform: UiAffine2::default(),
            opacity: 1.0,
            clip: None,
            primitive_range: UiPrimitiveRange { start: 0, end: 1 },
        })]),
    ));

    assert_eq!(scene.paint_nodes().len(), 1);
}
