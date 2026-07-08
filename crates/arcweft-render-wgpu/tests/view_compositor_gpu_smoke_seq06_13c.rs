//! Optional visual smoke fixture for seq06.13c.
//!
//! This test is intentionally ignored until native/web pinned readback exists in CI.
//! It documents the retained View scene shape that should be captured on both hosts.

use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::view_scene::{
    ViewAffine2D, ViewClipPath, ViewColorRgba8, ViewCompositingEffects, ViewCompositingGroup,
    ViewFillRule, ViewGradientStop, ViewMask, ViewMaskGradient, ViewMaskImage, ViewMaskRepeat,
    ViewPaintNode, ViewPrimitiveRange, ViewScene, ViewSceneContext,
};

#[test]
#[ignore = "requires native/web pinned GPU readback harness"]
fn seq06_13c_path_clip_gradient_mask_visual_smoke_fixture() {
    let mut scene = ViewScene::new(240.0, 160.0);
    scene.push_paint_node(ViewPaintNode::Group(
        ViewCompositingGroup::new(
            HitRect::new(20.0, 20.0, 160.0, 96.0),
            ViewCompositingEffects {
                clip_path: Some(Box::new(ViewClipPath::Path {
                    fill_rule: ViewFillRule::EvenOdd,
                    data: "M0 0 C80 12 120 12 160 0 L160 96 L0 96 Z".into(),
                })),
                masks: vec![ViewMask {
                    image: ViewMaskImage::Gradient(ViewMaskGradient::Linear {
                        angle_degrees: 90.0,
                        stops: vec![
                            ViewGradientStop {
                                offset: 0.0,
                                color: ViewColorRgba8 {
                                    red: 255,
                                    green: 255,
                                    blue: 255,
                                    alpha: 255,
                                },
                            },
                            ViewGradientStop {
                                offset: 1.0,
                                color: ViewColorRgba8 {
                                    red: 0,
                                    green: 0,
                                    blue: 0,
                                    alpha: 64,
                                },
                            },
                        ],
                    }),
                    repeat: ViewMaskRepeat::NoRepeat,
                    ..ViewMask::default()
                }],
                ..ViewCompositingEffects::default()
            },
        )
        .with_children(vec![ViewPaintNode::Direct(ViewSceneContext {
            transform: ViewAffine2D::default(),
            opacity: 1.0,
            clip: None,
            primitive_range: ViewPrimitiveRange { start: 0, end: 1 },
        })]),
    ));

    assert_eq!(scene.paint_nodes().len(), 1);
}
