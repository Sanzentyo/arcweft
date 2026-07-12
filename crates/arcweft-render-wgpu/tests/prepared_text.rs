use arcweft_glyphon::{PreparedTextItem, TextCaretPaint, TextCompositionUnderline};
use arcweft_presentation::{
    fx::{
        FiniteF32, FxColor, Length, Opacity, ResolvedFxGlyphPass, ResolvedFxMask,
        ResolvedFxOffscreenPass, ResolvedFxPostProcess,
    },
    hit::HitRect,
};
use arcweft_render_text::{RichTextRange, TextColor};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, PreparedFrame, PreparedViewScene, RenderChoiceItem,
    RenderFontFamily, RenderPreferences, RenderScene, RenderTextBlock, RenderTextSelectionPolicy,
    RenderTextSlant, RenderTextWeight, RenderViewport, SharedFramePlanContext,
};
use arcweft_render_wgpu::offscreen::{
    CaptureAttachment, CaptureRequest, SharedOffscreenCapture, SharedOffscreenCaptureError,
};
use arcweft_render_wgpu::renderer::SharedRendererError;
use arcweft_render_wgpu::view_compositor::ViewCompositorError;
use arcweft_render_wgpu::view_scene::{
    ViewAffine2D, ViewClip, ViewColorRgba8, ViewCompositingEffects, ViewCompositingGroup,
    ViewPaintNode, ViewPrimitive, ViewPrimitiveRange, ViewScene, ViewSceneContext, ViewSolidRect,
    ViewTextPrimitive,
};
use arcweft_text_layout::LayoutRect;

const TEST_FONT: &[u8] = include_bytes!("../../../web/assets/noto-sans-jp-vf.ttf");

fn viewport() -> RenderViewport {
    RenderViewport {
        logical_width: 640.0,
        logical_height: 360.0,
        physical_width: 1_280,
        physical_height: 720,
        scale_factor: 2.0,
    }
}

fn block() -> RenderTextBlock {
    RenderTextBlock {
        target: None,
        text: "office 日本".to_owned(),
        bounds: HitRect::new(20.0, 30.0, 300.0, 80.0),
        clip_bounds: Some(HitRect::new(20.0, 30.0, 300.0, 80.0)),
        buffer_width: Some(300.0),
        buffer_height: Some(80.0),
        font_size: 24.0,
        line_height: 32.0,
        font_family: RenderFontFamily::SansSerif,
        weight: RenderTextWeight::Regular,
        slant: RenderTextSlant::Upright,
        rgba: [230, 235, 245, 255],
        selection_policy: RenderTextSelectionPolicy::Disabled,
        selection: None,
        selection_rgba: [0.2, 0.4, 0.8, 0.5],
    }
}

#[test]
fn ordinary_block_prepares_once_with_shared_layout_and_frame_scale() {
    let mut planner = SharedFramePlanContext::new();
    planner
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("project font registers");

    let first = planner
        .prepare_text_block(&block(), viewport())
        .expect("ordinary text prepares");
    let second = planner
        .prepare_text_block(&block(), viewport())
        .expect("same ordinary text prepares from cache");

    assert_eq!(first.layout.hash, second.layout.hash);
    assert_eq!(first.glyphs.len(), first.paint.glyphs.len());
    assert!(!first.interaction.character_bounds.is_empty());
    assert!((first.submission().raster_scale() - 2.0).abs() < f32::EPSILON);
    let stats = planner.stats();
    assert_eq!(stats.prepared_text_shape_cache_misses, 1);
    assert_eq!(stats.prepared_text_shape_cache_hits, 1);
}

#[test]
fn mapped_frame_finalization_replaces_ordinary_renderer_input_in_order() {
    let mut planner = SharedFramePlanContext::new();
    planner
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("project font registers");
    let mut scene = empty_scene();
    scene.choices.push(RenderChoiceItem {
        id: "choice_one".to_owned(),
        label: "選択肢 One".to_owned(),
    });
    let mut frame = planner.prepare(&scene).expect("choice frame prepares");
    let ordinary_count = frame.text.len();
    assert!(ordinary_count > 0);

    planner
        .finalize_text(&mut frame)
        .expect("mapped text finalizes");

    assert!(frame.text.is_empty());
    assert_eq!(frame.prepared_text.len(), ordinary_count);
    assert!(
        frame
            .prepared_text
            .items()
            .iter()
            .all(|item| !item.glyphs.is_empty())
    );
}

#[test]
#[ignore = "requires a local wgpu adapter; exercised by the prepared-text Tier 2 gate"]
fn prepared_batch_renders_without_renderer_side_shaping() {
    let mut planner = SharedFramePlanContext::new();
    planner
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("project font registers");
    let mut frame = planner
        .prepare(&empty_scene())
        .expect("empty frame prepares");
    let mut baseline = frame.clone();
    let mut item = planner
        .prepare_text_block(&block(), viewport())
        .expect("ordinary text prepares");
    baseline
        .prepared_text
        .push(item.clone())
        .expect("baseline item index fits");
    let half = Opacity::try_new(FiniteF32::try_new(0.5).expect("finite")).expect("opacity");
    item.paint.glyphs[0].effects.push(ResolvedFxGlyphPass::new(
        Length::try_pixels(3.0).expect("offset"),
        Length::try_pixels(2.0).expect("offset"),
        FxColor::from_rgba8([120, 200, 255, 180]),
    ));
    item.paint.glyphs[0].masks.push(ResolvedFxMask {
        coverage: half,
        invert: false,
    });
    item.paint.offscreen_passes.push(ResolvedFxOffscreenPass {
        blur_radius: Length::try_pixels(1.5).expect("blur"),
        brightness: FiniteF32::ONE,
        contrast: FiniteF32::ONE,
        saturation: FiniteF32::ONE,
    });
    item.paint.post_processes.push(ResolvedFxPostProcess::Tint {
        color: FxColor::from_rgba8([255, 80, 120, 255]),
        amount: half,
    });
    frame
        .prepared_text
        .push(item)
        .expect("prepared item index fits");

    let Ok(mut capture) =
        pollster::block_on(SharedOffscreenCapture::new(wgpu::TextureFormat::Rgba8Unorm))
    else {
        eprintln!("no compatible wgpu adapter available for prepared-text smoke");
        return;
    };
    capture
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("capture registers identical project font bytes");
    let baseline = capture
        .capture(&baseline, &CaptureRequest::whole_frame_color())
        .expect("baseline captures");
    let rendered = capture
        .capture(&frame, &CaptureRequest::whole_frame_color())
        .expect("prepared text captures");

    assert_ne!(color_rgba(&baseline), color_rgba(&rendered));
}

#[test]
#[ignore = "requires a local wgpu adapter; exercised by the prepared-text Tier 2 gate"]
fn multiple_prepared_submissions_keep_vertex_buffers_alive_until_submit() {
    let mut planner = SharedFramePlanContext::new();
    planner
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("project font registers");
    let baseline = planner
        .prepare(&empty_scene())
        .expect("empty frame prepares");
    let mut frame = baseline.clone();
    let mut short = block();
    short.text = "A".to_owned();
    frame
        .prepared_text
        .push(
            planner
                .prepare_text_block(&short, viewport())
                .expect("short text prepares"),
        )
        .expect("short prepared item index fits");
    let mut long = block();
    long.text = "prepared glyph buffer growth ".repeat(24);
    long.bounds = HitRect::new(20.0, 70.0, 600.0, 260.0);
    long.clip_bounds = Some(long.bounds);
    long.buffer_width = Some(long.bounds.width);
    long.buffer_height = Some(long.bounds.height);
    frame
        .prepared_text
        .push(
            planner
                .prepare_text_block(&long, viewport())
                .expect("long text prepares"),
        )
        .expect("long prepared item index fits");

    let Ok(mut capture) =
        pollster::block_on(SharedOffscreenCapture::new(wgpu::TextureFormat::Rgba8Unorm))
    else {
        eprintln!("no compatible wgpu adapter available for prepared vertex lifetime smoke");
        return;
    };
    capture
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("capture registers identical project font bytes");
    let baseline = capture
        .capture(&baseline, &CaptureRequest::whole_frame_color())
        .expect("baseline captures");
    let rendered = capture
        .capture(&frame, &CaptureRequest::whole_frame_color())
        .expect("multiple prepared submissions capture without invalidating earlier buffers");

    assert_ne!(color_rgba(&baseline), color_rgba(&rendered));
}

#[test]
#[ignore = "requires a local wgpu adapter; exercised by the prepared-text Tier 2 gate"]
fn view_text_renders_at_primitive_position_without_late_duplicate_submission() {
    let mut planner = SharedFramePlanContext::new();
    planner
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("project font registers");
    let item = planner
        .prepare_text_block(&block(), viewport())
        .expect("ordinary text prepares");

    let red_only = view_text_frame(&mut planner, None, false);
    let text_over_red = view_text_frame(&mut planner, Some(&item), false);
    let red_then_blue = view_text_frame(&mut planner, None, true);
    let text_then_blue = view_text_frame(&mut planner, Some(&item), true);

    let Ok(mut capture) =
        pollster::block_on(SharedOffscreenCapture::new(wgpu::TextureFormat::Rgba8Unorm))
    else {
        eprintln!("no compatible wgpu adapter available for View text painter-order smoke");
        return;
    };
    capture
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("capture registers identical project font bytes");
    let red_only = capture
        .capture(&red_only, &CaptureRequest::whole_frame_color())
        .expect("red frame captures");
    let text_over_red = capture
        .capture(&text_over_red, &CaptureRequest::whole_frame_color())
        .expect("View text frame captures");
    let red_then_blue = capture
        .capture(&red_then_blue, &CaptureRequest::whole_frame_color())
        .expect("blue control frame captures");
    let text_then_blue = capture
        .capture(&text_then_blue, &CaptureRequest::whole_frame_color())
        .expect("covered View text frame captures");

    assert_ne!(
        color_rgba(&red_only),
        color_rgba(&text_over_red),
        "Text primitive must invoke the shared glyph renderer"
    );
    assert_eq!(
        color_rgba(&red_then_blue),
        color_rgba(&text_then_blue),
        "a later opaque primitive must cover Text, and the prepared item must not be submitted again"
    );
}

#[test]
#[ignore = "requires a local wgpu adapter; exercised by the prepared-text Tier 2 gate"]
fn view_text_obeys_transform_clip_opacity_inside_offscreen_group() {
    let mut planner = SharedFramePlanContext::new();
    planner
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("project font registers");
    let item = planner
        .prepare_text_block(&block(), viewport())
        .expect("ordinary text prepares");
    let baseline = view_text_frame(&mut planner, None, false);
    let opaque = grouped_view_text_frame(&mut planner, &item, 1.0, 1.0);
    let translucent = grouped_view_text_frame(&mut planner, &item, 0.5, 0.5);

    let Ok(mut capture) =
        pollster::block_on(SharedOffscreenCapture::new(wgpu::TextureFormat::Rgba8Unorm))
    else {
        eprintln!("no compatible wgpu adapter available for grouped View text smoke");
        return;
    };
    capture
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("capture registers identical project font bytes");
    let baseline = capture
        .capture(&baseline, &CaptureRequest::whole_frame_color())
        .expect("baseline captures");
    let opaque = capture
        .capture(&opaque, &CaptureRequest::whole_frame_color())
        .expect("opaque group captures");
    let translucent = capture
        .capture(&translucent, &CaptureRequest::whole_frame_color())
        .expect("translucent group captures");

    assert_ne!(
        color_rgba(&opaque),
        color_rgba(&translucent),
        "both opacity scopes must apply"
    );
    let changed = color_rgba(&baseline)
        .chunks_exact(4)
        .zip(color_rgba(&translucent).chunks_exact(4))
        .enumerate()
        .filter_map(|(index, (before, after))| (before != after).then_some(index))
        .collect::<Vec<_>>();
    assert!(
        !changed.is_empty(),
        "grouped Text must reach the offscreen target"
    );
    let width = usize::try_from(translucent.width).expect("capture width fits");
    assert!(
        changed.into_iter().all(|index| {
            let x = index % width;
            let y = index / width;
            (180..300).contains(&x) && (60..220).contains(&y)
        }),
        "transformed glyph pixels must remain inside the context clip"
    );
}

#[test]
#[ignore = "requires a local wgpu adapter; exercised by the prepared-text Tier 2 gate"]
fn missing_view_text_id_is_a_typed_compositor_failure() {
    let mut planner = SharedFramePlanContext::new();
    let mut frame = planner
        .prepare(&empty_scene())
        .expect("base frame prepares");
    let mut scene = ViewScene::new(viewport().logical_width, viewport().logical_height);
    scene.push_primitive(ViewPrimitive::Text(ViewTextPrimitive {
        text: arcweft_glyphon::PreparedTextId::from_index(7),
    }));
    scene.push_paint_node(ViewPaintNode::Direct(ViewSceneContext {
        transform: ViewAffine2D::IDENTITY,
        opacity: 1.0,
        clip: None,
        primitive_range: ViewPrimitiveRange { start: 0, end: 1 },
    }));
    frame.push_view_scene(PreparedViewScene::new(scene));

    let Ok(mut capture) =
        pollster::block_on(SharedOffscreenCapture::new(wgpu::TextureFormat::Rgba8Unorm))
    else {
        eprintln!("no compatible wgpu adapter available for missing View text smoke");
        return;
    };
    let error = capture
        .capture(&frame, &CaptureRequest::whole_frame_color())
        .expect_err("missing prepared text must not become a no-op");

    assert!(matches!(
        error,
        SharedOffscreenCaptureError::SharedRenderer(SharedRendererError::ViewCompositor(
            ViewCompositorError::MissingPreparedText { text_index: 7 }
        ))
    ));
}

#[test]
#[ignore = "requires a local wgpu adapter; exercised by the prepared-text Tier 2 gate"]
fn view_text_interaction_paints_selection_before_glyphs_and_ime_after() {
    let mut planner = SharedFramePlanContext::new();
    planner
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("project font registers");
    let mut item = planner
        .prepare_text_block(&block(), viewport())
        .expect("ordinary text prepares");
    item.interaction.selection_rects = vec![LayoutRect::new(10.0, 20.0, 340.0, 100.0)];
    item.interaction.selection_rgba = [0.0, 0.0, 1.0, 1.0];
    item.interaction.caret = Some(TextCaretPaint {
        bounds: LayoutRect::new(24.0, 34.0, 4.0, 24.0),
        color: TextColor::rgba(255, 0, 0, 255),
        visible: true,
    });
    item.interaction.composition_underlines = vec![TextCompositionUnderline {
        source_range: RichTextRange::new(0, 1),
        bounds: LayoutRect::new(20.0, 92.0, 100.0, 3.0),
        color: TextColor::rgba(0, 255, 0, 255),
        thickness: 3.0,
    }];
    let frame = view_text_frame(&mut planner, Some(&item), false);

    let Ok(mut capture) =
        pollster::block_on(SharedOffscreenCapture::new(wgpu::TextureFormat::Rgba8Unorm))
    else {
        eprintln!("no compatible wgpu adapter available for View interaction smoke");
        return;
    };
    capture
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("capture registers identical project font bytes");
    let capture = capture
        .capture(&frame, &CaptureRequest::whole_frame_color())
        .expect("interaction frame captures");

    let outside_item_clip = capture_pixel(&capture, 30, 50);
    assert!(
        outside_item_clip[0] > outside_item_clip[2],
        "selection must be intersected with the prepared item clip"
    );
    let caret = capture_pixel(&capture, 52, 80);
    assert!(caret[0] > 220 && caret[1] < 32 && caret[2] < 32);
    let underline = capture_pixel(&capture, 100, 186);
    assert!(underline[1] > 220 && underline[0] < 32 && underline[2] < 32);
    assert!(
        capture_region(&capture, 60, 68, 520, 104).any(|pixel| pixel[0] > 80 && pixel[1] > 80),
        "glyphs must remain visible over the opaque selection background"
    );
}

fn view_text_frame(
    planner: &mut SharedFramePlanContext,
    item: Option<&PreparedTextItem>,
    cover_text: bool,
) -> PreparedFrame {
    let mut frame = planner
        .prepare(&empty_scene())
        .expect("base frame prepares");
    let mut scene = ViewScene::new(viewport().logical_width, viewport().logical_height);
    let paint_bounds = HitRect::new(12.0, 20.0, 340.0, 100.0);
    scene.push_primitive(ViewPrimitive::SolidRect(ViewSolidRect {
        bounds: paint_bounds,
        color: ViewColorRgba8 {
            red: 96,
            green: 12,
            blue: 18,
            alpha: 255,
        },
    }));
    if let Some(item) = item {
        let text = frame
            .prepared_text
            .push(item.clone())
            .expect("prepared text index fits");
        scene.push_primitive(ViewPrimitive::Text(ViewTextPrimitive { text }));
    }
    if cover_text {
        scene.push_primitive(ViewPrimitive::SolidRect(ViewSolidRect {
            bounds: paint_bounds,
            color: ViewColorRgba8 {
                red: 8,
                green: 36,
                blue: 148,
                alpha: 255,
            },
        }));
    }
    let end = u32::try_from(scene.primitives().len()).expect("test primitive count fits");
    scene.push_paint_node(ViewPaintNode::Direct(ViewSceneContext {
        transform: ViewAffine2D::IDENTITY,
        opacity: 1.0,
        clip: None,
        primitive_range: ViewPrimitiveRange { start: 0, end },
    }));
    frame.push_view_scene(PreparedViewScene::new(scene));
    frame
}

fn grouped_view_text_frame(
    planner: &mut SharedFramePlanContext,
    item: &PreparedTextItem,
    context_opacity: f32,
    group_opacity: f32,
) -> PreparedFrame {
    let mut frame = planner
        .prepare(&empty_scene())
        .expect("base frame prepares");
    let text = frame
        .prepared_text
        .push(item.clone())
        .expect("prepared text index fits");
    let mut scene = ViewScene::new(viewport().logical_width, viewport().logical_height);
    scene.push_primitive(ViewPrimitive::SolidRect(ViewSolidRect {
        bounds: HitRect::new(12.0, 20.0, 340.0, 100.0),
        color: ViewColorRgba8 {
            red: 96,
            green: 12,
            blue: 18,
            alpha: 255,
        },
    }));
    scene.push_primitive(ViewPrimitive::Text(ViewTextPrimitive { text }));
    scene.push_paint_node(ViewPaintNode::Direct(ViewSceneContext {
        transform: ViewAffine2D::IDENTITY,
        opacity: 1.0,
        clip: None,
        primitive_range: ViewPrimitiveRange { start: 0, end: 1 },
    }));
    let text_context = ViewPaintNode::Direct(ViewSceneContext {
        transform: ViewAffine2D {
            tx: 64.0,
            ..ViewAffine2D::IDENTITY
        },
        opacity: context_opacity,
        clip: Some(ViewClip::Rect(HitRect::new(90.0, 30.0, 60.0, 80.0))),
        primitive_range: ViewPrimitiveRange { start: 1, end: 2 },
    });
    scene.push_paint_node(ViewPaintNode::Group(
        ViewCompositingGroup::new(
            HitRect::new(70.0, 20.0, 100.0, 100.0),
            ViewCompositingEffects {
                opacity: group_opacity,
                ..ViewCompositingEffects::default()
            },
        )
        .with_children(vec![text_context]),
    ));
    frame.push_view_scene(PreparedViewScene::new(scene));
    frame
}

fn capture_pixel(
    capture: &arcweft_render_wgpu::offscreen::SharedFrameCapture,
    x: usize,
    y: usize,
) -> [u8; 4] {
    let width = usize::try_from(capture.width).expect("capture width fits");
    let offset = y.saturating_mul(width).saturating_add(x).saturating_mul(4);
    color_rgba(capture)[offset..offset + 4]
        .try_into()
        .expect("pixel is present")
}

fn color_rgba(capture: &arcweft_render_wgpu::offscreen::SharedFrameCapture) -> &[u8] {
    capture
        .attachment_rgba(CaptureAttachment::Color)
        .expect("whole-frame color attachment exists")
}

fn capture_region(
    capture: &arcweft_render_wgpu::offscreen::SharedFrameCapture,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> impl Iterator<Item = [u8; 4]> + '_ {
    (y..y + height)
        .flat_map(move |row| (x..x + width).map(move |column| capture_pixel(capture, column, row)))
}

fn empty_scene() -> RenderScene {
    RenderScene {
        content_avoidance_regions: Vec::new(),
        choices: Vec::new(),
        text_inputs: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: Vec::new(),
        viewport: viewport(),
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
    }
}
