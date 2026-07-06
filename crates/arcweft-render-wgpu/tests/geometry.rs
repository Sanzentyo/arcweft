use arcweft_id::PublicId;
use arcweft_layout::{ContentRect, LayoutPoint, LayoutRect, LayoutSize, ScalePolicy};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::image::{ImageObjectAlignment, ImageObjectFit, ImageObjectTransform};
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::semantic::SemanticRole;
use arcweft_presentation::text_input::{
    TextByteOffset, TextInputOptions, TextInputPurpose, TextInputSessionId, TextRange,
};
use arcweft_render_text::{RichTextColor, RichTextFontFamily, RichTextStyle};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, FocusNavigationDirection, InteractionVisualState, RenderActionButton,
    RenderActionButtonAction, RenderChoiceItem, RenderControlFilter, RenderControlFilterList,
    RenderControlShadow, RenderControlShadowKind, RenderControlStyle, RenderControlVisualStyle,
    RenderDialogue, RenderFocusGroup, RenderFocusGroupPolicy, RenderFocusInitialPolicy,
    RenderFocusNavigation, RenderFocusNavigationEdge, RenderFocusSkipPolicy,
    RenderFocusTargetResolution, RenderFocusWrapPolicy, RenderFontFamily, RenderImage,
    RenderImageFrame, RenderPreferences, RenderScene, RenderScrollAxis, RenderScrollOverflow,
    RenderScrollRegion, RenderTextInputControl, RenderTextSlant, RenderTextWeight, RenderViewport,
    SharedFramePlanContext, SharedFramePlanner,
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
        text_inputs: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: Vec::new(),
        viewport,
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
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

fn render_image(id: &str, bounds: HitRect) -> RenderImage {
    RenderImage {
        id: id.to_owned(),
        frame: RenderImageFrame {
            index: None,
            width: 10,
            height: 10,
            rgba: vec![255; 10 * 10 * 4],
        },
        bounds,
        containing_scroll_region: None,
        viewport_clip: None,
        placement: None,
        fit: ImageObjectFit::Stretch,
        alignment: ImageObjectAlignment::center(),
        transform: ImageObjectTransform::identity(),
        opacity_milli: 1_000,
    }
}

#[test]
fn viewport_physical_scale_factor_for_renderer_is_finite_f32() {
    let mut viewport = scene().viewport;
    viewport.scale_factor = 2.0;
    assert!((viewport.physical_scale_factor_f32() - 2.0).abs() < f32::EPSILON);

    viewport.scale_factor = 0.0;
    assert!((viewport.physical_scale_factor_f32() - f32::EPSILON).abs() < f32::EPSILON);

    viewport.scale_factor = f64::NAN;
    assert!((viewport.physical_scale_factor_f32() - 1.0).abs() < f32::EPSILON);
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
    let raw = ContentRect::calculate(design, output, ScalePolicy::Raw).expect("raw rect");
    let cover = ContentRect::calculate(design, output, ScalePolicy::Cover).expect("cover rect");
    let stretch =
        ContentRect::calculate(design, output, ScalePolicy::Stretch).expect("stretch rect");

    assert_eq!(raw.rect, LayoutRect::from_xywh(0.0, 0.0, 1280.0, 720.0));
    assert!((raw.scale_x - 1.0).abs() < f32::EPSILON);
    assert!((raw.scale_y - 1.0).abs() < f32::EPSILON);
    assert!((cover.scale_x - 1.111_111_2).abs() < 0.000_001);
    assert!((cover.rect.origin.x + 211.111_15).abs() < 0.000_01);
    assert!(cover.rect.origin.y.abs() < 0.000_1);
    assert_eq!(stretch.rect, LayoutRect::from_xywh(0.0, 0.0, 1000.0, 800.0));
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
        rect.map_rect(LayoutRect::from_xywh(96.0, 48.0, 320.0, 160.0)),
        LayoutRect::from_xywh(75.0, 156.25, 250.0, 125.0)
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
fn directional_keyboard_navigation_uses_focus_target_geometry() {
    let top_left = text_target("top_left");
    let top_right = text_target("top_right");
    let bottom_left = text_target("bottom_left");
    let bottom_right = text_target("bottom_right");
    let mut scene = scene();
    scene.choices.clear();
    scene.text_inputs = vec![
        text_control(
            top_left.clone(),
            "",
            SemanticRole::TextField,
            TextInputOptions::default(),
            HitRect::new(80.0, 80.0, 220.0, 44.0),
        ),
        text_control(
            top_right.clone(),
            "",
            SemanticRole::TextField,
            TextInputOptions::default(),
            HitRect::new(360.0, 80.0, 220.0, 44.0),
        ),
        text_control(
            bottom_left.clone(),
            "",
            SemanticRole::TextField,
            TextInputOptions::default(),
            HitRect::new(80.0, 180.0, 220.0, 44.0),
        ),
        text_control(
            bottom_right.clone(),
            "",
            SemanticRole::TextField,
            TextInputOptions::default(),
            HitRect::new(360.0, 180.0, 220.0, 44.0),
        ),
    ];

    let frame = SharedFramePlanner::prepare(&scene).expect("frame plans");

    assert_eq!(
        frame.directional_keyboard_focus_target(Some(&top_left), FocusNavigationDirection::Right),
        Some(top_right.clone())
    );
    assert_eq!(
        frame.directional_keyboard_focus_target(Some(&top_left), FocusNavigationDirection::Down),
        Some(bottom_left.clone())
    );
    assert_eq!(
        frame.directional_keyboard_focus_target(Some(&bottom_right), FocusNavigationDirection::Up),
        Some(top_right)
    );
    assert_eq!(
        frame
            .directional_keyboard_focus_target(Some(&bottom_right), FocusNavigationDirection::Left),
        Some(bottom_left)
    );
    assert_eq!(
        frame.directional_keyboard_focus_target(Some(&top_left), FocusNavigationDirection::Up),
        None
    );
}

#[test]
fn focus_navigation_explicit_edge_overrides_geometry() {
    let top_left = text_target("focus_nav.top_left");
    let top_right = text_target("focus_nav.top_right");
    let bottom_left = text_target("focus_nav.bottom_left");
    let mut scene = scene();
    scene.choices.clear();
    scene.text_inputs = vec![
        text_control(
            top_left.clone(),
            "",
            SemanticRole::TextField,
            TextInputOptions::default(),
            HitRect::new(80.0, 80.0, 220.0, 44.0),
        ),
        text_control(
            top_right,
            "",
            SemanticRole::TextField,
            TextInputOptions::default(),
            HitRect::new(360.0, 80.0, 220.0, 44.0),
        ),
        text_control(
            bottom_left.clone(),
            "",
            SemanticRole::TextField,
            TextInputOptions::default(),
            HitRect::new(80.0, 180.0, 220.0, 44.0),
        ),
    ];
    scene.focus_navigation = vec![RenderFocusNavigation {
        target: top_left.clone(),
        group: None,
        edges: vec![RenderFocusNavigationEdge {
            direction: FocusNavigationDirection::Right,
            target: RenderFocusTargetResolution::Explicit(bottom_left.clone()),
        }],
    }];

    let frame = SharedFramePlanner::prepare(&scene).expect("frame plans");

    assert_eq!(
        frame.focus_target(Some(&top_left), FocusNavigationDirection::Right),
        Some(bottom_left)
    );
}

#[test]
fn focus_navigation_next_previous_respects_no_wrap_group() {
    let first = text_target("focus_nav.first");
    let second = text_target("focus_nav.second");
    let mut scene = scene();
    scene.choices.clear();
    scene.text_inputs = vec![
        text_control(
            first.clone(),
            "",
            SemanticRole::TextField,
            TextInputOptions::default(),
            HitRect::new(80.0, 80.0, 220.0, 44.0),
        ),
        text_control(
            second.clone(),
            "",
            SemanticRole::TextField,
            TextInputOptions::default(),
            HitRect::new(360.0, 80.0, 220.0, 44.0),
        ),
    ];
    scene.focus_groups = vec![RenderFocusGroup {
        public_id: "group.focus_nav".to_owned(),
        parent: None,
        policy: RenderFocusGroupPolicy::Normal,
        initial: RenderFocusInitialPolicy::Auto,
        wrap: RenderFocusWrapPolicy::NoWrap,
        disabled_skip: RenderFocusSkipPolicy::Skip,
        hidden_skip: RenderFocusSkipPolicy::Skip,
    }];
    scene.focus_navigation = vec![
        RenderFocusNavigation {
            target: first.clone(),
            group: Some("group.focus_nav".to_owned()),
            edges: Vec::new(),
        },
        RenderFocusNavigation {
            target: second.clone(),
            group: Some("group.focus_nav".to_owned()),
            edges: Vec::new(),
        },
    ];

    let frame = SharedFramePlanner::prepare(&scene).expect("frame plans");

    assert_eq!(
        frame.focus_target(Some(&first), FocusNavigationDirection::Next),
        Some(second.clone())
    );
    assert_eq!(
        frame.focus_target(Some(&second), FocusNavigationDirection::Next),
        None
    );
    assert_eq!(
        frame.focus_target(Some(&first), FocusNavigationDirection::Previous),
        None
    );
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
fn choice_geometry_ignores_scroll_offset() {
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
fn scroll_regions_survive_frame_planning_and_viewport_mapping() {
    let frame = SharedFramePlanner::prepare(&RenderScene {
        scroll_regions: vec![RenderScrollRegion {
            id: "scroll.story".to_owned(),
            bounds: HitRect::new(100.0, 50.0, 300.0, 120.0),
            content_width: 300.0,
            content_height: 480.0,
            offset_x: 0.0,
            offset_y: 90.0,
            axis: RenderScrollAxis::Vertical,
            overflow: RenderScrollOverflow::Auto,
        }],
        ..scene()
    })
    .expect("frame plans")
    .mapped_to_viewport(
        RenderViewport {
            logical_width: 640.0,
            logical_height: 360.0,
            physical_width: 640,
            physical_height: 360,
            scale_factor: 1.0,
        },
        ContentRect::calculate(
            LayoutSize::new(1280.0, 720.0),
            LayoutSize::new(640.0, 360.0),
            ScalePolicy::Contain,
        )
        .expect("content rect"),
    );

    let region = frame.scroll_regions.first().expect("scroll region");
    assert_eq!(region.id, "scroll.story");
    assert_eq!(region.bounds, HitRect::new(50.0, 25.0, 150.0, 60.0));
    assert!((region.content_width - 150.0).abs() < f32::EPSILON);
    assert!((region.content_height - 240.0).abs() < f32::EPSILON);
    assert!(region.offset_x.abs() < f32::EPSILON);
    assert!((region.offset_y - 45.0).abs() < f32::EPSILON);
}

#[test]
fn hidden_scroll_region_reports_no_scroll_range() {
    let region = RenderScrollRegion {
        id: "scroll.story".to_owned(),
        bounds: HitRect::new(100.0, 50.0, 300.0, 120.0),
        content_width: 300.0,
        content_height: 480.0,
        offset_x: 0.0,
        offset_y: 0.0,
        axis: RenderScrollAxis::Vertical,
        overflow: RenderScrollOverflow::Hidden,
    };

    assert!(region.max_offset_y().abs() < f32::EPSILON);
    assert!(region.clamped_offset_y(90.0).abs() < f32::EPSILON);
}

#[test]
fn scroll_region_offsets_and_clips_owned_text_controls() {
    let target = text_target("scroll.feedback.input");
    let control = text_control(
        target.clone(),
        "inside scroll",
        SemanticRole::TextField,
        TextInputOptions::default(),
        HitRect::new(100.0, 170.0, 280.0, 48.0),
    )
    .with_containing_scroll_region("scroll.feedback");

    let mut scene = scene();
    scene.choices.clear();
    scene.text_inputs = vec![control];
    scene.scroll_regions = vec![RenderScrollRegion {
        id: "scroll.feedback".to_owned(),
        bounds: HitRect::new(100.0, 100.0, 320.0, 80.0),
        content_width: 320.0,
        content_height: 240.0,
        offset_x: 0.0,
        offset_y: 60.0,
        axis: RenderScrollAxis::Vertical,
        overflow: RenderScrollOverflow::Auto,
    }];

    let frame = SharedFramePlanner::prepare(&scene).expect("frame plans");
    let hit = frame.hits.find_target(&target).expect("scrolled hit");
    assert_eq!(hit.bounds(), HitRect::new(100.0, 110.0, 280.0, 48.0));
    let semantic = frame.semantics.find(&target).expect("scrolled semantic");
    assert_eq!(semantic.bounds(), HitRect::new(100.0, 110.0, 280.0, 48.0));

    let text = frame
        .text
        .iter()
        .find(|block| block.text == "inside scroll")
        .expect("scrolled text block");
    assert_eq!(
        text.clip_bounds,
        Some(HitRect::new(108.0, 114.0, 264.0, 40.0))
    );
}

#[test]
fn horizontal_scroll_region_offsets_and_clips_owned_text_controls() {
    let target = text_target("scroll.gallery.input");
    let control = text_control(
        target.clone(),
        "inside horizontal scroll",
        SemanticRole::TextField,
        TextInputOptions::default(),
        HitRect::new(260.0, 100.0, 280.0, 48.0),
    )
    .with_containing_scroll_region("scroll.gallery");

    let mut scene = scene();
    scene.choices.clear();
    scene.text_inputs = vec![control];
    scene.scroll_regions = vec![RenderScrollRegion {
        id: "scroll.gallery".to_owned(),
        bounds: HitRect::new(100.0, 100.0, 320.0, 80.0),
        content_width: 640.0,
        content_height: 80.0,
        offset_x: 160.0,
        offset_y: 0.0,
        axis: RenderScrollAxis::Horizontal,
        overflow: RenderScrollOverflow::Auto,
    }];

    let frame = SharedFramePlanner::prepare(&scene).expect("frame plans");
    let hit = frame.hits.find_target(&target).expect("scrolled hit");
    assert_eq!(hit.bounds(), HitRect::new(100.0, 100.0, 280.0, 48.0));
    let semantic = frame.semantics.find(&target).expect("scrolled semantic");
    assert_eq!(semantic.bounds(), HitRect::new(100.0, 100.0, 280.0, 48.0));
    let text = frame
        .text
        .iter()
        .find(|block| block.text == "inside horizontal scroll")
        .expect("scrolled text block");
    assert_eq!(
        text.clip_bounds,
        Some(HitRect::new(108.0, 104.0, 264.0, 40.0))
    );
}

#[test]
fn scroll_region_offsets_and_clips_owned_images() {
    let mut image = render_image("image.scroll.card", HitRect::new(100.0, 170.0, 200.0, 80.0));
    image.containing_scroll_region = Some("scroll.gallery".to_owned());

    let mut scene = scene();
    scene.choices.clear();
    scene.images = vec![image];
    scene.scroll_regions = vec![RenderScrollRegion {
        id: "scroll.gallery".to_owned(),
        bounds: HitRect::new(100.0, 100.0, 160.0, 80.0),
        content_width: 240.0,
        content_height: 260.0,
        offset_x: 0.0,
        offset_y: 60.0,
        axis: RenderScrollAxis::Vertical,
        overflow: RenderScrollOverflow::Auto,
    }];

    let frame = SharedFramePlanner::prepare(&scene).expect("frame plans");
    assert_eq!(frame.images.len(), 1);
    let image = &frame.images[0];
    assert_eq!(image.bounds, HitRect::new(100.0, 110.0, 200.0, 80.0));
    assert_eq!(
        image.viewport_clip,
        Some(HitRect::new(100.0, 100.0, 160.0, 80.0))
    );
    let quad = image.visible_quad().expect("image is partially visible");
    assert_eq!(quad.rect, HitRect::new(100.0, 110.0, 160.0, 70.0));
    assert!((quad.uv_left - 0.0).abs() < f32::EPSILON);
    assert!((quad.uv_top - 0.0).abs() < f32::EPSILON);
    assert!((quad.uv_right - 0.8).abs() < f32::EPSILON);
    assert!((quad.uv_bottom - 0.875).abs() < f32::EPSILON);
}

#[test]
fn scroll_region_drops_images_outside_viewport() {
    let mut image = render_image(
        "image.scroll.offscreen",
        HitRect::new(100.0, 280.0, 200.0, 80.0),
    );
    image.containing_scroll_region = Some("scroll.gallery".to_owned());

    let mut scene = scene();
    scene.choices.clear();
    scene.images = vec![image];
    scene.scroll_regions = vec![RenderScrollRegion {
        id: "scroll.gallery".to_owned(),
        bounds: HitRect::new(100.0, 100.0, 160.0, 80.0),
        content_width: 240.0,
        content_height: 360.0,
        offset_x: 0.0,
        offset_y: 60.0,
        axis: RenderScrollAxis::Vertical,
        overflow: RenderScrollOverflow::Auto,
    }];

    let frame = SharedFramePlanner::prepare(&scene).expect("frame plans");
    assert!(frame.images.is_empty());
}

#[test]
fn scroll_region_offsets_and_clips_owned_action_buttons() {
    let target = text_target("scroll.feedback.send");
    let mut scene = scene();
    scene.choices.clear();
    scene.action_buttons = vec![RenderActionButton {
        target: target.clone(),
        label: "Send".to_owned(),
        enabled: true,
        containing_scroll_region: Some("scroll.feedback".to_owned()),
        bounds: HitRect::new(100.0, 170.0, 180.0, 48.0),
        viewport_clip: None,
        style: RenderControlStyle::default(),
        action: RenderActionButtonAction::Noop,
    }];
    scene.scroll_regions = vec![RenderScrollRegion {
        id: "scroll.feedback".to_owned(),
        bounds: HitRect::new(100.0, 100.0, 320.0, 80.0),
        content_width: 320.0,
        content_height: 240.0,
        offset_x: 0.0,
        offset_y: 60.0,
        axis: RenderScrollAxis::Vertical,
        overflow: RenderScrollOverflow::Auto,
    }];

    let frame = SharedFramePlanner::prepare(&scene).expect("frame plans");
    let hit = frame.hits.find_target(&target).expect("scrolled hit");
    assert_eq!(hit.bounds(), HitRect::new(100.0, 110.0, 180.0, 48.0));
    let semantic = frame.semantics.find(&target).expect("scrolled semantic");
    assert_eq!(semantic.bounds(), HitRect::new(100.0, 110.0, 180.0, 48.0));
    let text = frame
        .text
        .iter()
        .find(|block| block.text == "Send")
        .expect("button text block");
    assert_eq!(
        text.clip_bounds,
        Some(HitRect::new(100.0, 110.0, 180.0, 48.0))
    );
}

#[test]
fn scroll_region_uses_visible_bounds_for_runtime_control_effect_plans() {
    let target = text_target("scroll.feedback.effects");
    let mut scene = scene();
    scene.choices.clear();
    scene.action_buttons = vec![RenderActionButton {
        target,
        label: "Send".to_owned(),
        enabled: true,
        containing_scroll_region: Some("scroll.feedback".to_owned()),
        bounds: HitRect::new(100.0, 170.0, 180.0, 48.0),
        viewport_clip: None,
        style: RenderControlStyle {
            normal: RenderControlVisualStyle {
                filters: Some(RenderControlFilterList {
                    filters: vec![RenderControlFilter::Blur { radius_px: 2.0 }],
                }),
                backdrop_filters: Some(RenderControlFilterList {
                    filters: vec![RenderControlFilter::Brightness { factor: 1.1 }],
                }),
                shadows: vec![RenderControlShadow {
                    offset_x_px: 0.0,
                    offset_y_px: 8.0,
                    blur_radius_px: 18.0,
                    spread_radius_px: 0.0,
                    border_radius_px: 12.0,
                    color: [0, 0, 0, 128],
                    kind: RenderControlShadowKind::Outer,
                }],
                ..RenderControlVisualStyle::default()
            },
            ..RenderControlStyle::default()
        },
        action: RenderActionButtonAction::Noop,
    }];
    scene.scroll_regions = vec![RenderScrollRegion {
        id: "scroll.feedback".to_owned(),
        bounds: HitRect::new(100.0, 100.0, 320.0, 80.0),
        content_width: 320.0,
        content_height: 240.0,
        offset_x: 0.0,
        offset_y: 60.0,
        axis: RenderScrollAxis::Vertical,
        overflow: RenderScrollOverflow::Auto,
    }];

    let frame = SharedFramePlanner::prepare(&scene).expect("frame plans");
    let visible = HitRect::new(100.0, 110.0, 180.0, 48.0);
    assert_eq!(frame.control_backdrops[0].bounds, visible);
    assert_eq!(frame.control_filters[0].bounds, visible);
    assert_eq!(frame.control_paints[0].bounds, visible);
    assert_eq!(frame.control_shadows[0].plan.passes()[0].body_rect, visible);
}

#[test]
fn dialogue_surface_styles_are_preserved_for_styled_paragraph() {
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

    assert!(
        frame
            .text
            .iter()
            .all(|block| !block.text.contains("Surface style"))
    );
    let body = frame
        .styled_paragraphs
        .iter()
        .find(|paragraph| paragraph.text.contains("Surface style"))
        .expect("styled paragraph");
    let span = body.spans.first().expect("styled span");
    assert_eq!(span.style.color, [220, 180, 140, 255]);
    assert_eq!(
        span.style.font_family,
        RenderFontFamily::Named("Yu Mincho".to_owned())
    );
    assert_eq!(span.style.weight, RenderTextWeight::Bold);
    assert_eq!(span.style.slant, RenderTextSlant::Italic);
    assert!((span.style.font_size - 31.0).abs() < f32::EPSILON);
}

fn text_target(name: &str) -> InteractionTarget {
    InteractionTarget::new(PublicId::try_new(format!("target.{name}")).unwrap())
}

fn text_control(
    target: InteractionTarget,
    value: &str,
    role: SemanticRole,
    options: TextInputOptions,
    bounds: HitRect,
) -> RenderTextInputControl {
    let end = TextByteOffset(u32::try_from(value.len()).unwrap());
    RenderTextInputControl::new(
        target,
        TextInputSessionId(77),
        value,
        TextRange::new(end, end),
        options,
        role,
        bounds,
    )
}

#[test]
fn focused_text_input_target_produces_real_focused_text_field() {
    let target = text_target("real.text_field");
    let control = text_control(
        target.clone(),
        "abc",
        SemanticRole::TextField,
        TextInputOptions::default(),
        HitRect::new(96.0, 88.0, 260.0, 32.0),
    );
    let mut scene = scene();
    scene.choices.clear();
    scene.text_inputs = vec![control];
    scene.interaction = InteractionVisualState {
        focused: Some(target.clone()),
        hovered: None,
        pressed: None,
    };

    let frame = SharedFramePlanner::prepare(&scene).expect("text input frame plans");
    let focused = frame.focused_text_input_target().expect("focused target");

    assert_eq!(focused.snapshot.target(), &target);
    assert_eq!(focused.snapshot.surrounding_text(), "abc");
    assert_eq!(focused.geometry.viewport_character_bounds().len(), 3);
    assert_eq!(
        frame.semantics.find(&target).expect("semantic node").role(),
        SemanticRole::TextField,
    );
}

#[test]
fn stateful_planner_reuses_text_control_layout_cache_with_registered_fonts() {
    let target = text_target("real.cached_text_field");
    let control = text_control(
        target.clone(),
        "llll wide あいう",
        SemanticRole::TextField,
        TextInputOptions::default(),
        HitRect::new(96.0, 88.0, 360.0, 48.0),
    );
    let mut scene = scene();
    scene.choices.clear();
    scene.text_inputs = vec![control];
    scene.interaction.focused = Some(target);

    let font_bytes = include_bytes!("../../../web/assets/arcweft-demo.ttf");
    let mut planner = SharedFramePlanContext::new();
    planner
        .register_font_bytes(font_bytes.to_vec())
        .expect("font bytes register");
    let initial = planner.stats();
    assert_eq!(initial.registered_font_bytes, font_bytes.len());

    planner.prepare(&scene).expect("first frame plans");
    let first = planner.stats();
    assert!(
        first.text_control_layout_cache_misses > initial.text_control_layout_cache_misses,
        "first prepare should shape and populate the text-control layout cache"
    );

    planner.prepare(&scene).expect("second frame plans");
    let second = planner.stats();
    assert!(
        second.text_control_layout_cache_hits > first.text_control_layout_cache_hits,
        "second prepare of the same scene should reuse the shaped text-control layout"
    );
    assert_eq!(
        second.text_control_layout_cache_misses, first.text_control_layout_cache_misses,
        "same scene should not shape again after the cache is warm"
    );
}

#[test]
fn focused_text_input_target_secure_field_redacts_value_and_character_geometry() {
    let target = text_target("secure.password");
    let control = text_control(
        target.clone(),
        "secret",
        SemanticRole::SecureTextField,
        TextInputOptions::default().with_purpose(TextInputPurpose::Password),
        HitRect::new(32.0, 44.0, 240.0, 32.0),
    );
    let mut scene = scene();
    scene.choices.clear();
    scene.text_inputs = vec![control];
    scene.interaction.focused = Some(target);

    let focused = SharedFramePlanner::prepare(&scene)
        .expect("secure text input frame plans")
        .focused_text_input_target()
        .expect("focused target");

    assert!(focused.snapshot.options().is_secure());
    assert_eq!(focused.snapshot.surrounding_text(), "");
    assert!(focused.snapshot.character_bounds().is_empty());
    assert!(focused.geometry.viewport_character_bounds().is_empty());
    assert!(focused.geometry.screen_character_bounds().is_empty());
}

#[test]
fn focused_text_input_target_browser_and_native_use_same_geometry_snapshot_source() {
    let target = text_target("geometry.shared");
    let bounds = HitRect::new(50.0, 60.0, 300.0, 40.0);
    let control = text_control(
        target.clone(),
        "hi",
        SemanticRole::TextField,
        TextInputOptions::default(),
        bounds,
    );
    let mut scene = scene();
    scene.choices.clear();
    scene.text_inputs = vec![control];
    scene.viewport.scale_factor = 2.0;
    scene.viewport.physical_width = 2560;
    scene.viewport.physical_height = 1440;
    scene.interaction.focused = Some(target);

    let focused = SharedFramePlanner::prepare(&scene)
        .expect("hidpi text input frame plans")
        .focused_text_input_target()
        .expect("focused target");
    let geometry = focused.geometry;

    assert_eq!(
        focused.snapshot.control_rect(),
        geometry.viewport_control_rect()
    );
    assert_eq!(
        focused.snapshot.caret_rect(),
        geometry.viewport_caret_rect()
    );
    assert_eq!(geometry.viewport_control_rect(), bounds);
    assert_eq!(
        geometry.screen_control_rect(),
        HitRect::new(100.0, 120.0, 600.0, 80.0)
    );
}

#[test]
fn prepared_frame_fit_mapping_scales_runtime_control_geometry_and_ime_snapshots() {
    let target = text_target("geometry.fit");
    let bounds = HitRect::new(50.0, 60.0, 300.0, 40.0);
    let control = text_control(
        target.clone(),
        "hi",
        SemanticRole::TextField,
        TextInputOptions::default(),
        bounds,
    );
    let mut scene = scene();
    scene.choices.clear();
    scene.text_inputs = vec![control];
    scene.interaction.focused = Some(target.clone());
    let frame = SharedFramePlanner::prepare(&scene).expect("text input frame plans");
    let output = RenderViewport {
        logical_width: 640.0,
        logical_height: 360.0,
        physical_width: 640,
        physical_height: 360,
        scale_factor: 1.0,
    };
    let content = ContentRect::calculate(
        LayoutSize::new(1280.0, 720.0),
        LayoutSize::new(640.0, 360.0),
        ScalePolicy::Contain,
    )
    .expect("content rect");

    let mapped = frame.mapped_to_viewport(output, content);
    let focused = mapped
        .focused_text_input_target()
        .expect("focused target remains available");

    assert_eq!(
        mapped
            .semantics
            .find(&target)
            .expect("mapped semantic")
            .bounds(),
        HitRect::new(25.0, 30.0, 150.0, 20.0)
    );
    assert_eq!(
        mapped
            .hits
            .find_target(&target)
            .expect("mapped hit")
            .bounds(),
        HitRect::new(25.0, 30.0, 150.0, 20.0)
    );
    assert_eq!(
        focused.snapshot.control_rect(),
        HitRect::new(25.0, 30.0, 150.0, 20.0)
    );
    assert_eq!(
        focused.geometry.viewport_control_rect(),
        HitRect::new(25.0, 30.0, 150.0, 20.0)
    );
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
