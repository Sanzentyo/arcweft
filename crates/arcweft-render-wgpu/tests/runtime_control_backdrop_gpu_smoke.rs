use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::image::{ImageObjectAlignment, ImageObjectFit, ImageObjectTransform};
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::semantic::SemanticRole;
use arcweft_presentation::text_input::{
    TextByteOffset, TextInputOptions, TextInputSessionId, TextRange,
};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, RenderControlBorderStyle, RenderControlFilter,
    RenderControlFilterList, RenderControlStyle, RenderControlVisualStyle, RenderImage,
    RenderImageFrame, RenderPreferences, RenderScene, RenderTextInputControl, RenderViewport,
    SharedFramePlanContext,
};

const TEST_FONT: &[u8] = include_bytes!("../../../web/assets/noto-sans-jp-vf.ttf");

fn prepare(
    scene: &RenderScene,
) -> Result<
    arcweft_render_wgpu::geometry::PreparedFrame,
    arcweft_render_wgpu::geometry::FramePlanError,
> {
    let mut planner = SharedFramePlanContext::new();
    planner.register_font_bytes(TEST_FONT.to_vec())?;
    planner.prepare(scene)
}
use arcweft_render_wgpu::offscreen::{
    CaptureAttachment, CaptureRequest, SharedFrameCapture, SharedOffscreenCapture,
};

#[test]
#[ignore = "requires a local wgpu adapter; exact PNG promotion remains pinned-only"]
fn prepared_control_backdrop_blur_executes_shared_renderer_path() {
    let Ok(mut capture) =
        pollster::block_on(SharedOffscreenCapture::new(wgpu::TextureFormat::Rgba8Unorm))
    else {
        eprintln!("no compatible wgpu adapter available for runtime backdrop smoke");
        return;
    };
    capture
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("capture font registers");
    let baseline = prepare(&scene(false)).expect("baseline frame prepares");
    let blurred = prepare(&scene(true)).expect("blurred frame prepares");

    let baseline = capture
        .capture(&baseline, &CaptureRequest::whole_frame_color())
        .expect("baseline frame captures");
    let blurred = capture
        .capture(&blurred, &CaptureRequest::whole_frame_color())
        .expect("blurred frame captures");

    assert_eq!(baseline.width, blurred.width);
    assert_eq!(baseline.height, blurred.height);
    assert_ne!(
        capture_pixel(&baseline, 45, 45),
        capture_pixel(&blurred, 45, 45),
        "transparent control backdrop blur should change pixels inside the control bounds"
    );
}

#[test]
#[ignore = "requires a local wgpu adapter; exact PNG promotion remains pinned-only"]
fn prepared_control_foreground_filter_blur_executes_shared_renderer_path() {
    let Ok(mut capture) =
        pollster::block_on(SharedOffscreenCapture::new(wgpu::TextureFormat::Rgba8Unorm))
    else {
        eprintln!("no compatible wgpu adapter available for runtime foreground filter smoke");
        return;
    };
    capture
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("capture font registers");
    let baseline = prepare(&foreground_scene(false)).expect("baseline foreground frame prepares");
    let blurred = prepare(&foreground_scene(true)).expect("blurred foreground frame prepares");

    let baseline = capture
        .capture(&baseline, &CaptureRequest::whole_frame_color())
        .expect("baseline foreground frame captures");
    let blurred = capture
        .capture(&blurred, &CaptureRequest::whole_frame_color())
        .expect("blurred foreground frame captures");

    assert_eq!(baseline.width, blurred.width);
    assert_eq!(baseline.height, blurred.height);
    assert_ne!(
        capture_pixel(&baseline, 33, 33),
        capture_pixel(&blurred, 33, 33),
        "foreground blur should affect control-content edge pixels without blurring the backdrop"
    );
}

#[test]
#[ignore = "requires a local wgpu adapter; exact PNG promotion remains pinned-only"]
fn rounded_runtime_control_stroke_draws_straight_edges() {
    let Ok(mut capture) =
        pollster::block_on(SharedOffscreenCapture::new(wgpu::TextureFormat::Rgba8Unorm))
    else {
        eprintln!("no compatible wgpu adapter available for runtime stroke smoke");
        return;
    };
    capture
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("capture font registers");
    let baseline = prepare(&stroke_scene(false)).expect("baseline prepares");
    let stroked = prepare(&stroke_scene(true)).expect("stroked prepares");

    let baseline = capture
        .capture(&baseline, &CaptureRequest::whole_frame_color())
        .expect("baseline frame captures");
    let stroked = capture
        .capture(&stroked, &CaptureRequest::whole_frame_color())
        .expect("stroked frame captures");

    assert_eq!(baseline.width, stroked.width);
    assert_eq!(baseline.height, stroked.height);
    assert_ne!(
        capture_pixel(&baseline, 80, 33),
        capture_pixel(&stroked, 80, 33),
        "rounded stroke should paint the top straight edge, not only the corners"
    );
    assert_ne!(
        capture_pixel(&baseline, 33, 72),
        capture_pixel(&stroked, 33, 72),
        "rounded stroke should paint the left straight edge, not only the corners"
    );
}

fn scene(backdrop: bool) -> RenderScene {
    RenderScene {
        content_avoidance_regions: Vec::new(),
        choices: Vec::new(),
        text_inputs: vec![control(backdrop)],
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: vec![checker_image()],
        viewport: RenderViewport {
            logical_width: 160.0,
            logical_height: 128.0,
            physical_width: 160,
            physical_height: 128,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
    }
}

fn stroke_scene(stroke: bool) -> RenderScene {
    RenderScene {
        content_avoidance_regions: Vec::new(),
        choices: Vec::new(),
        text_inputs: vec![stroke_control(stroke)],
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: vec![checker_image()],
        viewport: RenderViewport {
            logical_width: 160.0,
            logical_height: 128.0,
            physical_width: 160,
            physical_height: 128,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
    }
}

fn foreground_scene(filter: bool) -> RenderScene {
    RenderScene {
        content_avoidance_regions: Vec::new(),
        choices: Vec::new(),
        text_inputs: vec![foreground_control(filter)],
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: vec![checker_image()],
        viewport: RenderViewport {
            logical_width: 160.0,
            logical_height: 128.0,
            physical_width: 160,
            physical_height: 128,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
    }
}

fn stroke_control(stroke: bool) -> RenderTextInputControl {
    let style = RenderControlStyle {
        normal: RenderControlVisualStyle {
            fill: Some([0.0, 0.0, 0.0, 0.0]),
            radius_px: Some(14.0),
            border: stroke.then_some(RenderControlBorderStyle {
                color: [0.0, 1.0, 0.65, 1.0],
                width_px: 4.0,
            }),
            ..RenderControlVisualStyle::default()
        },
        ..RenderControlStyle::default()
    };
    RenderTextInputControl::new(
        target("input.stroke"),
        TextInputSessionId(11),
        "",
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(32.0, 32.0, 96.0, 80.0),
    )
    .with_style(style)
}

fn control(backdrop: bool) -> RenderTextInputControl {
    let style = RenderControlStyle {
        normal: RenderControlVisualStyle {
            fill: Some([0.0, 0.0, 0.0, 0.0]),
            backdrop_filters: backdrop.then_some(RenderControlFilterList {
                filters: vec![RenderControlFilter::Blur { radius_px: 6.0 }],
            }),
            ..RenderControlVisualStyle::default()
        },
        ..RenderControlStyle::default()
    };
    RenderTextInputControl::new(
        target("input.backdrop"),
        TextInputSessionId(9),
        "",
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(32.0, 32.0, 96.0, 80.0),
    )
    .with_style(style)
}

fn foreground_control(filter: bool) -> RenderTextInputControl {
    let style = RenderControlStyle {
        normal: RenderControlVisualStyle {
            fill: Some([1.0, 1.0, 1.0, 1.0]),
            filters: filter.then_some(RenderControlFilterList {
                filters: vec![RenderControlFilter::Blur { radius_px: 6.0 }],
            }),
            ..RenderControlVisualStyle::default()
        },
        ..RenderControlStyle::default()
    };
    RenderTextInputControl::new(
        target("input.foreground_filter"),
        TextInputSessionId(10),
        "",
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(32.0, 32.0, 96.0, 80.0),
    )
    .with_style(style)
}

fn checker_image() -> RenderImage {
    RenderImage {
        id: "checker".to_owned(),
        frame: RenderImageFrame {
            index: None,
            width: 160,
            height: 128,
            rgba: checker_rgba(160, 128),
        },
        bounds: HitRect::new(0.0, 0.0, 160.0, 128.0),
        containing_scroll_region: None,
        viewport_clip: None,
        placement: None,
        fit: ImageObjectFit::Stretch,
        alignment: ImageObjectAlignment::top_left(),
        transform: ImageObjectTransform::identity(),
        opacity_milli: 1_000,
    }
}

fn checker_rgba(width: u32, height: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let bright = ((x / 4) + (y / 4)) % 2 == 0;
            rgba.extend(if bright {
                [240, 72, 64, 255]
            } else {
                [24, 108, 232, 255]
            });
        }
    }
    rgba
}

fn target(value: &str) -> InteractionTarget {
    InteractionTarget::new(PublicId::try_new(value).unwrap())
}

fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * width + x) * 4) as usize;
    [
        rgba[offset],
        rgba[offset + 1],
        rgba[offset + 2],
        rgba[offset + 3],
    ]
}

fn capture_pixel(capture: &SharedFrameCapture, x: u32, y: u32) -> [u8; 4] {
    pixel(
        capture
            .attachment_rgba(CaptureAttachment::Color)
            .expect("whole-frame color attachment exists"),
        capture.width,
        x,
        y,
    )
}
