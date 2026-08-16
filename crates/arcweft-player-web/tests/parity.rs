use arcweft_player_web::report::{WebFrameBounds, WebFrameObservationReport};
use arcweft_presentation::{
    hit::HitRect,
    image::{ImageObjectAlignment, ImageObjectFit, ImageObjectTransform},
};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, RenderFocusAutoScrollPolicy, RenderImage,
    RenderImageFrame, RenderPreferences, RenderScene, RenderScrollAxis,
    RenderScrollIndicatorsPolicy, RenderScrollOverflow, RenderScrollOverscrollPolicy,
    RenderScrollRegion, RenderViewport,
};

mod support;
use support::prepare;

#[test]
fn web_frame_report_drops_released_image_handle_resources() {
    let live = image_frame_report(vec![test_render_image("image.glass_bg")]);

    assert_eq!(live.image_count, 1);
    assert_eq!(
        live.images
            .iter()
            .map(|image| image.id.as_str())
            .collect::<Vec<_>>(),
        vec!["image.glass_bg"]
    );
    assert_eq!(
        live.images[0].bounds,
        WebFrameBounds {
            x_milli: 24_000,
            y_milli: 32_000,
            width_milli: 128_000,
            height_milli: 96_000,
        }
    );

    let released = image_frame_report(Vec::new());
    assert_eq!(released.image_count, 0);
    assert!(released.images.is_empty());
}

#[test]
fn web_frame_report_uses_visible_bounds_for_scroll_clipped_images() {
    let mut image = test_render_image("image.scroll.card");
    image.bounds = HitRect::new(100.0, 170.0, 200.0, 80.0);
    image.containing_scroll_region = Some("scroll.gallery".to_owned());
    let scene = RenderScene {
        content_avoidance_regions: Vec::new(),
        choices: Vec::new(),
        text_inputs: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: vec![image],
        viewport: RenderViewport {
            logical_width: 800.0,
            logical_height: 450.0,
            physical_width: 800,
            physical_height: 450,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: vec![RenderScrollRegion {
            id: "scroll.gallery".to_owned(),
            bounds: HitRect::new(100.0, 100.0, 160.0, 80.0),
            content_width: 240.0,
            content_height: 260.0,
            min_offset_x: 0.0,
            max_offset_x: 0.0,
            min_offset_y: 0.0,
            max_offset_y: 180.0,
            offset_x: 0.0,
            offset_y: 60.0,
            overscroll_x: 0.0,
            overscroll_y: 0.0,
            axis: RenderScrollAxis::Vertical,
            overflow: RenderScrollOverflow::Auto,
            indicators: RenderScrollIndicatorsPolicy::Auto,
            overscroll: RenderScrollOverscrollPolicy::Clamp,
            auto_scroll_focus: RenderFocusAutoScrollPolicy::Nearest,
            indicator_activity_millis: None,
        }],
    };
    let prepared = prepare(&scene).expect("frame prepares");
    let report = WebFrameObservationReport::from_prepared_frame(&prepared);

    assert_eq!(report.image_count, 1);
    assert_eq!(
        report.images[0].bounds,
        WebFrameBounds {
            x_milli: 100_000,
            y_milli: 110_000,
            width_milli: 160_000,
            height_milli: 70_000,
        }
    );
}

fn image_frame_report(images: Vec<RenderImage>) -> WebFrameObservationReport {
    let prepared = prepare(&RenderScene {
        content_avoidance_regions: Vec::new(),
        choices: Vec::new(),
        text_inputs: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images,
        viewport: RenderViewport {
            logical_width: 800.0,
            logical_height: 450.0,
            physical_width: 800,
            physical_height: 450,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
    })
    .expect("image frame prepares");
    WebFrameObservationReport::from_prepared_frame(&prepared)
}

fn test_render_image(id: &str) -> RenderImage {
    RenderImage {
        id: id.to_owned(),
        frame: RenderImageFrame {
            index: None,
            width: 2,
            height: 2,
            rgba: vec![
                0x20, 0x30, 0x40, 0xff, 0x50, 0x60, 0x70, 0xff, 0x80, 0x90, 0xa0, 0xff, 0xb0, 0xc0,
                0xd0, 0xff,
            ],
        },
        bounds: HitRect::new(24.0, 32.0, 128.0, 96.0),
        containing_scroll_region: None,
        viewport_clip: None,
        placement: None,
        fit: ImageObjectFit::Stretch,
        alignment: ImageObjectAlignment::default(),
        transform: ImageObjectTransform::identity(),
        opacity_milli: 1_000,
    }
}
