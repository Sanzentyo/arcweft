use arcweft_render_text::{RichTextColor, RichTextFontFamily, RichTextStyle};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, ContentRect, InteractionVisualState, LayoutPoint, LayoutRect, LayoutSize,
    RenderChoiceItem, RenderDialogue, RenderFontFamily, RenderPreferences, RenderScene,
    RenderTextSlant, RenderTextWeight, RenderViewport, ScalePolicy, SharedFramePlanner,
};
use arcweft_render_wgpu::sample::{DemoAnimationClock, DemoImageKind, generated_demo_images};

fn scene() -> RenderScene {
    let viewport = RenderViewport {
        logical_width: 1280.0,
        logical_height: 720.0,
        physical_width: 1280,
        physical_height: 720,
        scale_factor: 1.0,
    };
    RenderScene {
        dialogue: None,
        choices: vec![
            RenderChoiceItem {
                id: "choice.one".to_owned(),
                label: "One".to_owned(),
            },
            RenderChoiceItem {
                id: "choice.two".to_owned(),
                label: "Two".to_owned(),
            },
        ],
        images: Vec::new(),
        viewport,
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
    }
}

fn generated_scene(elapsed_millis: u64) -> RenderScene {
    let mut scene = scene();
    scene.images = generated_demo_images(
        scene.viewport,
        DemoAnimationClock::from_millis(elapsed_millis),
    );
    scene.visual_time_millis = elapsed_millis;
    scene
}

#[test]
fn content_rect_contain_preserves_design_aspect_ratio_in_tall_output() {
    let rect = ContentRect::calculate(
        LayoutSize::new(1280.0, 720.0),
        LayoutSize::new(1000.0, 800.0),
        ScalePolicy::Contain,
    )
    .expect("content rect calculates");

    assert_eq!(rect.policy, ScalePolicy::Contain);
    assert!((rect.scale_x - 0.78125).abs() < f32::EPSILON);
    assert!((rect.scale_y - 0.78125).abs() < f32::EPSILON);
    assert_eq!(rect.rect.origin, LayoutPoint::new(0.0, 118.75));
    assert_eq!(rect.rect.size, LayoutSize::new(1000.0, 562.5));
}

#[test]
fn content_rect_modes_make_raw_cover_and_stretch_distinct() {
    let design = LayoutSize::new(1280.0, 720.0);
    let output = LayoutSize::new(1000.0, 800.0);
    let raw = ContentRect::calculate(design, output, ScalePolicy::None).expect("raw rect");
    let cover = ContentRect::calculate(design, output, ScalePolicy::Cover).expect("cover rect");
    let stretch =
        ContentRect::calculate(design, output, ScalePolicy::Stretch).expect("stretch rect");

    assert_eq!(raw.rect, LayoutRect::new(0.0, 0.0, 1280.0, 720.0));
    assert!((raw.scale_x - 1.0).abs() < f32::EPSILON);
    assert!((raw.scale_y - 1.0).abs() < f32::EPSILON);
    assert!((cover.scale_x - 1.111_111_2).abs() < 0.000_001);
    assert!((cover.rect.origin.x + 211.111_15).abs() < 0.000_01);
    assert!(cover.rect.origin.y.abs() < 0.000_1);
    assert_eq!(stretch.rect, LayoutRect::new(0.0, 0.0, 1000.0, 800.0));
    assert!((stretch.scale_x - 0.78125).abs() < f32::EPSILON);
    assert!((stretch.scale_y - 1.111_111_2).abs() < 0.000_001);
}

#[test]
fn content_rect_maps_design_rect_into_output_space() {
    let rect = ContentRect::calculate(
        LayoutSize::new(1280.0, 720.0),
        LayoutSize::new(1000.0, 800.0),
        ScalePolicy::Contain,
    )
    .expect("content rect calculates");

    assert_eq!(
        rect.map_rect(LayoutRect::new(96.0, 48.0, 320.0, 160.0)),
        LayoutRect::new(75.0, 156.25, 250.0, 125.0)
    );
}

#[test]
fn planner_uses_the_same_choice_geometry_for_render_and_hit_test() {
    let frame = SharedFramePlanner::prepare(&scene()).expect("frame plans");
    assert_eq!(frame.choices.len(), 2);
    for choice in &frame.choices {
        assert!(frame.hits.find_target(&choice.target).is_some());
        assert!(frame.semantics.find(&choice.target).is_some());
    }
}

#[test]
fn keyboard_navigation_wraps_across_stable_targets() {
    let frame = SharedFramePlanner::prepare(&scene()).expect("frame plans");
    let first = frame.first_choice_target().expect("first target");
    let previous = frame
        .adjacent_choice_target(Some(&first), -1)
        .expect("wrapped target");
    assert_eq!(previous, frame.choices[1].target);
}

#[test]
fn interaction_visual_state_changes_the_prepared_choice_rectangles() {
    let base_scene = scene();
    let neutral = SharedFramePlanner::prepare(&base_scene).expect("neutral frame plans");
    let first = neutral.first_choice_target().expect("first target");
    let focused = SharedFramePlanner::prepare(&RenderScene {
        interaction: InteractionVisualState {
            focused: Some(first.clone()),
            hovered: None,
            pressed: None,
        },
        ..base_scene.clone()
    })
    .expect("focused frame plans");
    let pressed = SharedFramePlanner::prepare(&RenderScene {
        interaction: InteractionVisualState {
            focused: Some(first.clone()),
            hovered: Some(first.clone()),
            pressed: Some(first),
        },
        ..base_scene
    })
    .expect("pressed frame plans");

    assert!(
        focused.rectangles.len() > neutral.rectangles.len(),
        "focused choice should add a visible focus ring"
    );
    assert!(
        focused.rectangles[1]
            .rgba
            .iter()
            .zip(pressed.rectangles[1].rgba.iter())
            .any(|(focused, pressed)| (focused - pressed).abs() > f32::EPSILON),
        "pressed choice should use a distinct fill"
    );
}

#[test]
fn choice_geometry_ignores_legacy_scroll_offset() {
    let base_scene = RenderScene {
        dialogue: Some(RenderDialogue::plain(
            "Guide",
            "Choice geometry stays fixed.",
        )),
        visual_time_millis: 5_000,
        ..scene()
    };
    let neutral = SharedFramePlanner::prepare(&base_scene).expect("neutral frame plans");
    let scrolled = SharedFramePlanner::prepare(&RenderScene {
        choice_scroll: ChoiceScroll { offset_y: 240.0 },
        ..base_scene
    })
    .expect("scrolled frame plans");

    let neutral_target = neutral.first_choice_target().expect("neutral target");
    let scrolled_target = scrolled.first_choice_target().expect("scrolled target");
    assert_eq!(
        neutral
            .hits
            .find_target(&neutral_target)
            .expect("neutral hit")
            .bounds(),
        scrolled
            .hits
            .find_target(&scrolled_target)
            .expect("scrolled hit")
            .bounds()
    );
}

#[test]
fn dialogue_surface_styles_are_preserved_for_shared_text_blocks() {
    let frame = SharedFramePlanner::prepare(&RenderScene {
        dialogue: Some(RenderDialogue {
            speaker: "Narrator".to_owned(),
            text: "Surface style reaches the canvas renderer.".to_owned(),
            base_styles: vec![
                RichTextStyle::Color {
                    value: RichTextColor::Rgb {
                        red: 220,
                        green: 180,
                        blue: 140,
                    },
                },
                RichTextStyle::Font {
                    family: RichTextFontFamily::Named {
                        name: "Yu Mincho".to_owned(),
                    },
                },
                RichTextStyle::Size {
                    points: Some(31),
                    raw: "31px".to_owned(),
                },
                RichTextStyle::Strong {
                    attrs: String::new(),
                },
                RichTextStyle::Italic {
                    attrs: String::new(),
                },
            ],
            text_runs: Vec::new(),
        }),
        visual_time_millis: 5_000,
        ..scene()
    })
    .expect("frame plans");

    let body = frame
        .text
        .iter()
        .find(|block| block.text.contains("Surface style"))
        .expect("body text block");
    assert_eq!(body.rgba, [220, 180, 140, 255]);
    assert_eq!(
        body.font_family,
        RenderFontFamily::Named("Yu Mincho".to_owned())
    );
    assert_eq!(body.weight, RenderTextWeight::Bold);
    assert_eq!(body.slant, RenderTextSlant::Italic);
    assert!((body.font_size - 31.0).abs() < f32::EPSILON);
}

#[test]
fn generated_visual_demo_supplies_background_character_and_animated_frames() {
    let frame_a = SharedFramePlanner::prepare(&generated_scene(0)).expect("frame plans");
    let frame_b = SharedFramePlanner::prepare(&generated_scene(170)).expect("frame plans");

    assert_eq!(frame_a.images.len(), 4);
    assert_eq!(frame_a.images[0].id, DemoImageKind::Background.asset_id());
    assert_eq!(
        frame_a.images[1].id,
        DemoImageKind::CharacterStand.asset_id()
    );
    assert_ne!(
        frame_a.images[2].frame.rgba, frame_b.images[2].frame.rgba,
        "GIF sample frame should animate over time"
    );
    assert_ne!(
        frame_a.images[3].frame.rgba, frame_b.images[3].frame.rgba,
        "WebP sample frame should animate over time"
    );
}
