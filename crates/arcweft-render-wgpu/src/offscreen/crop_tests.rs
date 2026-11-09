use super::*;
use crate::geometry::{PreparedTextDocumentRequest, PreparedViewScene, SharedFramePlanContext};
use crate::view_scene::{
    ViewAffine2D, ViewClip, ViewCompositingEffects, ViewCompositingGroup, ViewFilter,
    ViewFilterList, ViewPaintNode, ViewPrimitive, ViewPrimitiveRange, ViewScene, ViewSceneContext,
    ViewTextPrimitive,
};
use arcweft_render_text::{
    ResolvedTextDocument, ResolvedTextRun, ResolvedTextRunSource, ResolvedTextStyle,
    TextDocumentRevision, TextFontFamily,
};
use arcweft_text_layout::{LayoutPoint, LayoutRect, LayoutSize};
use arcweft_text_model::{RichTextPresentation, RichTextRange};

const FONT: &[u8] = include_bytes!("../../../../web/assets/noto-sans-jp-vf.ttf");

#[test]
fn coverage_budget_charges_actual_region_targets_and_preserves_full_frame_limits() {
    let mut region = tests::prepared_raster_region(0, 0, [1, 2, 3, 255]);
    region.rect = PixelRect {
        left: 10,
        top: 20,
        right: 14,
        bottom: 23,
    };
    region.coverage = RasterCoverage::PreparedText(
        PreparedTextSelection::try_new(PreparedTextId::from_index(0), [0]).unwrap(),
        PreparedTextCoverageExtent::Region,
    );
    let budget = PreparedTextCaptureBudget {
        passes: 2,
        render_pixels: 24,
        readback_bytes: 1536,
    };
    let full = PixelRect::full(1280, 720);
    let scope = RasterScope::Regions(vec![region.clone(), region.clone()]);
    validate_prepared_text_capture_budget(
        1280,
        720,
        full,
        &scope,
        &[CaptureAttachment::Mask],
        budget,
    )
    .unwrap();
    assert!(matches!(
        validate_prepared_text_capture_budget(
            1280,
            720,
            full,
            &scope,
            &[CaptureAttachment::Mask],
            PreparedTextCaptureBudget {
                render_pixels: 23,
                ..budget
            }
        ),
        Err(
            SharedOffscreenCaptureError::PreparedTextCoverageRenderBudgetExceeded {
                actual: 24,
                limit: 23
            }
        ),
    ));
    region.coverage = RasterCoverage::PreparedText(
        PreparedTextSelection::try_new(PreparedTextId::from_index(0), [0]).unwrap(),
        PreparedTextCoverageExtent::Frame,
    );
    assert!(matches!(
        validate_prepared_text_capture_budget(
            1280,
            720,
            full,
            &RasterScope::Regions(vec![region]),
            &[CaptureAttachment::Mask],
            budget
        ),
        Err(
            SharedOffscreenCaptureError::PreparedTextCoverageRenderBudgetExceeded {
                actual: 921600,
                limit: 24
            }
        ),
    ));
}

fn coverage_fixture(scale: f32, view: bool, group: bool) -> (PreparedFrame, PreparedTextSelection) {
    let mut planner = SharedFramePlanContext::new();
    planner.register_font_bytes(FONT.to_vec()).unwrap();
    let viewport = RenderViewport {
        logical_width: 256.0,
        logical_height: 128.0,
        physical_width: (256.0 * scale).to_u32().unwrap(),
        physical_height: (128.0 * scale).to_u32().unwrap(),
        scale_factor: f64::from(scale),
    };
    let mut frame = tests::prepared_frame(viewport);
    let text = "office 日本";
    let range = RichTextRange::new(0, text.len());
    let doc = ResolvedTextDocument::new(
        text,
        0,
        vec![
            ResolvedTextRun::new(
                range,
                range,
                ResolvedTextStyle::new(vec![TextFontFamily::SansSerif], 24_000, 32_000).unwrap(),
                RichTextPresentation::default(),
                ResolvedTextRunSource::Plain,
            )
            .unwrap(),
        ],
        Vec::new(),
        TextDocumentRevision::new(0),
    )
    .unwrap();
    let item = planner
        .prepare_text_document(
            &doc,
            &PreparedTextDocumentRequest {
                origin: LayoutPoint::new(20.25, 30.5),
                size: LayoutSize::new(180.0, 65.0),
                metric_scale: arcweft_render_text::TextMetricScale::IDENTITY,
                container_bounds: LayoutRect::new(20.25, 30.5, 180.0, 65.0),
                clip: Some(LayoutRect::new(28.25, 32.5, 180.0, 65.0)),
                target: None,
                selection_enabled: false,
                selection: None,
                selection_rgba: [0.0; 4],
            },
            viewport,
        )
        .unwrap();
    let indices = (0..item.glyphs.len())
        .map(|index| u32::try_from(index).unwrap())
        .collect::<Vec<_>>();
    let id = frame.text.push(item).unwrap();
    if view {
        let mut scene = ViewScene::new(256.0, 128.0);
        scene.push_primitive(ViewPrimitive::Text(ViewTextPrimitive { text: id }));
        let context = ViewSceneContext {
            transform: ViewAffine2D {
                m11: 1.0,
                m12: 0.15,
                m21: 0.1,
                m22: 1.0,
                tx: 7.5,
                ty: -3.25,
            },
            opacity: 0.75,
            clip: Some(ViewClip::Rect(HitRect::new(40.0, 25.0, 140.0, 64.0))),
            primitive_range: ViewPrimitiveRange { start: 0, end: 1 },
        };
        if group {
            scene.push_paint_node(ViewPaintNode::Group(
                ViewCompositingGroup::new(
                    HitRect::new(0.0, 0.0, 230.0, 110.0),
                    ViewCompositingEffects {
                        opacity: 0.8,
                        ..Default::default()
                    },
                )
                .with_children(vec![ViewPaintNode::Direct(context)]),
            ));
        } else {
            scene.push_context(context);
        }
        frame.push_view_scene(PreparedViewScene::new(scene));
    }
    (frame, PreparedTextSelection::try_new(id, indices).unwrap())
}

#[test]
fn coverage_plan_keeps_nonlocal_text_and_ancestor_effects_in_full_frame_space() {
    let (mut frame, selection) = coverage_fixture(1.0, true, true);
    let request = CaptureRequest::new(
        [CaptureAttachment::Mask],
        CaptureScope::Regions(vec![CaptureRegion::prepared_text(
            arcweft_id::PublicId::try_new("capture.local").unwrap(),
            HitRect::new(41.0, 31.0, 115.0, 45.0),
            [1, 2, 3, 255],
            selection.clone(),
        )]),
        CaptureCropPolicy::FullFrame,
    );
    let extent = |frame: &PreparedFrame| {
        let plan = CapturePlan::new(frame, &request).unwrap();
        let RasterScope::Regions(regions) = plan.scope else {
            panic!("one region")
        };
        let RasterCoverage::PreparedText(_, extent) = &regions[0].coverage else {
            panic!("prepared text")
        };
        *extent
    };
    assert!(matches!(extent(&frame), PreparedTextCoverageExtent::Region));
    let local_scene = frame.view_scenes()[0].scene.clone();
    let mut nonlocal_scene = local_scene.clone();
    let mut nodes = nonlocal_scene.paint_nodes().to_vec();
    let ViewPaintNode::Group(group) = &mut nodes[0] else {
        panic!("fixture group")
    };
    group.effects.filters = ViewFilterList::new([ViewFilter::Blur { radius_px: 1.0 }]);
    nonlocal_scene.replace_paint_nodes(nodes);
    frame = frame.with_view_scenes(vec![PreparedViewScene::new(nonlocal_scene)]);
    assert!(matches!(extent(&frame), PreparedTextCoverageExtent::Frame));
    frame = frame.with_view_scenes(vec![PreparedViewScene::new(local_scene)]);
    frame
        .text
        .get_mut(selection.text())
        .unwrap()
        .paint
        .offscreen_passes
        .push(arcweft_presentation::fx::ResolvedFxOffscreenPass {
            blur_radius: arcweft_presentation::fx::Length::try_pixels(1.0).unwrap(),
            brightness: arcweft_presentation::fx::FiniteF32::ONE,
            contrast: arcweft_presentation::fx::FiniteF32::ONE,
            saturation: arcweft_presentation::fx::FiniteF32::ONE,
        });
    assert!(matches!(extent(&frame), PreparedTextCoverageExtent::Frame));
}

#[test]
#[ignore = "tier 2: exact native GPU crop/full-frame alpha parity"]
fn cropped_prepared_coverage_matches_original_full_frame_alpha() {
    let mut capture = pollster::block_on(SharedOffscreenCapture::new(
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ))
    .unwrap();
    capture.register_font_bytes(FONT.to_vec()).unwrap();
    for scale in [1.0, 1.25, 2.0] {
        for (view, group) in [(false, false), (true, false), (true, true)] {
            let (frame, selection) = coverage_fixture(scale, view, group);
            let rect = physical_rect(frame.viewport, HitRect::new(41.0, 31.0, 115.0, 45.0));
            let full = PixelRect::full(
                frame.viewport.physical_width,
                frame.viewport.physical_height,
            );
            let expected = capture
                .render_prepared_text_coverage(&frame, 0, &selection, rect, full)
                .unwrap();
            let actual = capture
                .render_prepared_text_coverage(&frame, 0, &selection, rect, rect)
                .unwrap();
            assert!(expected.alpha.iter().any(|alpha| *alpha > 0));
            assert_eq!(actual.rect, expected.rect);
            assert_eq!(actual.alpha.len(), expected.alpha.len());
            let differences = actual
                .alpha
                .iter()
                .zip(&expected.alpha)
                .enumerate()
                .filter(|(_, (actual, expected))| actual != expected)
                .take(8)
                .collect::<Vec<_>>();
            assert!(
                differences.is_empty(),
                "scale={scale} view={view} group={group}, first alpha differences: {differences:?}"
            );
        }
    }
}
