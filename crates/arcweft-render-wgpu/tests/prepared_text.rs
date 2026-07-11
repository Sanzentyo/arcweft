use arcweft_presentation::{
    fx::{
        FiniteF32, FxColor, Length, Opacity, ResolvedFxGlyphPass, ResolvedFxMask,
        ResolvedFxOffscreenPass, ResolvedFxPostProcess,
    },
    hit::HitRect,
};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, RenderChoiceItem, RenderFontFamily, RenderPreferences,
    RenderScene, RenderTextBlock, RenderTextSelectionPolicy, RenderTextSlant, RenderTextWeight,
    RenderViewport, SharedFramePlanContext,
};
use arcweft_render_wgpu::offscreen::SharedOffscreenCapture;

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
    let baseline = capture.capture_frame(&baseline).expect("baseline captures");
    let rendered = capture
        .capture_frame(&frame)
        .expect("prepared text captures");

    assert_ne!(baseline.rgba, rendered.rgba);
}

fn empty_scene() -> RenderScene {
    RenderScene {
        dialogue: None,
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
