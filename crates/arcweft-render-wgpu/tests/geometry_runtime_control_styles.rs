use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::semantic::SemanticRole;
use arcweft_presentation::text_input::{
    TextByteOffset, TextInputOptions, TextInputSessionId, TextRange,
};
use arcweft_render_text::TextFontFamily;
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, PaintRectCornerRadius, PaintRectRadii,
    RenderActionButton, RenderActionButtonAction, RenderControlBorderStyle,
    RenderControlCornerFrameStyle, RenderControlFilter, RenderControlFilterList,
    RenderControlFocusRingStyle, RenderControlShadow, RenderControlShadowKind, RenderControlStyle,
    RenderControlVisualStyle, RenderPreferences, RenderScene, RenderTextInputControl,
    RenderViewport, RuntimeControlBackdropSamplePolicy,
};
use arcweft_render_wgpu::view_scene::ViewFilter;

const TEST_FONT: &[u8] = include_bytes!("../../../web/assets/noto-sans-jp-vf.ttf");

fn prepare(scene: &RenderScene) -> arcweft_render_wgpu::geometry::PreparedFrame {
    let mut planner = arcweft_render_wgpu::geometry::SharedFramePlanContext::new();
    planner
        .register_font_bytes(TEST_FONT.to_vec())
        .expect("test font registers");
    planner.prepare(scene).expect("frame prepares")
}

#[test]
fn action_button_hover_uses_authored_fill_and_text_color() {
    let button_target = target("button.submit_feedback");
    let scene = scene_with_button(
        button_target.clone(),
        InteractionVisualState {
            focused: None,
            hovered: Some(button_target.clone()),
            pressed: None,
        },
    );

    let frame = prepare(&scene);

    assert!(
        frame
            .rectangles
            .iter()
            .any(|rect| rgba_near(rect.rgba, [0.1, 0.2, 0.3, 0.8]))
    );
    assert!(frame.text.items().iter().any(|text| {
        text.interaction.text == "Send"
            && text
                .paint
                .glyphs
                .iter()
                .all(|glyph| glyph.color.channels() == [240, 248, 255, 255])
    }));
}

#[test]
fn focused_text_control_uses_authored_focus_ring() {
    let input_target = target("input.feedback");
    let mut control = text_control(input_target.clone());
    control = control.with_style(RenderControlStyle {
        focus_visible: Some(RenderControlVisualStyle {
            focus_ring: Some(RenderControlFocusRingStyle {
                color: [1.0, 0.9, 0.1, 1.0],
                width_px: 3.0,
                offset_px: 2.0,
            }),
            border: Some(RenderControlBorderStyle {
                color: [0.5, 0.8, 0.5, 1.0],
                width_px: 2.0,
            }),
            ..RenderControlVisualStyle::default()
        }),
        ..RenderControlStyle::default()
    });
    let scene = scene(
        vec![control],
        Vec::new(),
        InteractionVisualState {
            focused: Some(input_target),
            hovered: None,
            pressed: None,
        },
    );

    let frame = prepare(&scene);

    assert!(
        frame
            .rectangles
            .iter()
            .any(|rect| rgba_near(rect.rgba, [1.0, 0.9, 0.1, 1.0]))
    );
    assert!(
        frame
            .rectangles
            .iter()
            .any(|rect| rgba_near(rect.rgba, [0.5, 0.8, 0.5, 1.0]))
    );
    let focus_ring = frame
        .rectangles
        .iter()
        .find(|rect| rgba_near(rect.rgba, [1.0, 0.9, 0.1, 1.0]))
        .expect("focus ring rectangle exists");
    let border = frame
        .rectangles
        .iter()
        .find(|rect| rgba_near(rect.rgba, [0.5, 0.8, 0.5, 1.0]))
        .expect("border rectangle exists");

    assert_f32_near(focus_ring.stroke_width_px, 3.0);
    assert_f32_near(border.stroke_width_px, 2.0);
}

#[test]
fn focused_text_control_uses_authored_selection_and_caret_colors() {
    let input_target = target("input.feedback");
    let control = text_control(input_target.clone())
        .with_selection(TextRange::new(TextByteOffset(0), TextByteOffset(2)))
        .with_style(RenderControlStyle {
            normal: RenderControlVisualStyle {
                selection: Some([0.2, 0.5, 0.8, 0.6]),
                caret: Some([0.9, 0.8, 0.1, 1.0]),
                ..RenderControlVisualStyle::default()
            },
            ..RenderControlStyle::default()
        });
    let scene = scene(
        vec![control],
        Vec::new(),
        InteractionVisualState {
            focused: Some(input_target),
            hovered: None,
            pressed: None,
        },
    );

    let frame = prepare(&scene);

    let interaction = &frame.text.items()[0].interaction;
    assert!(rgba_near(interaction.selection_rgba, [0.2, 0.5, 0.8, 0.6]));
    assert!(!interaction.selection_rects.is_empty());
    assert_eq!(
        interaction
            .caret
            .expect("focused caret exists")
            .color
            .channels(),
        [230, 204, 26, 255]
    );
}

#[test]
fn text_control_fill_and_inner_marks_use_authored_corner_radii() {
    let input_target = target("input.feedback");
    let radii = PaintRectRadii::new(
        PaintRectCornerRadius::new(12.0, 6.0),
        PaintRectCornerRadius::new(10.0, 5.0),
        PaintRectCornerRadius::new(8.0, 4.0),
        PaintRectCornerRadius::new(6.0, 3.0),
    );
    let control = text_control(input_target.clone())
        .with_selection(TextRange::new(TextByteOffset(0), TextByteOffset(2)))
        .with_style(RenderControlStyle {
            normal: RenderControlVisualStyle {
                fill: Some([0.1, 0.2, 0.3, 0.4]),
                selection: Some([0.2, 0.5, 0.8, 0.6]),
                caret: Some([0.9, 0.8, 0.1, 1.0]),
                radii_px: Some(radii),
                ..RenderControlVisualStyle::default()
            },
            ..RenderControlStyle::default()
        });
    let scene = scene(
        vec![control],
        Vec::new(),
        InteractionVisualState {
            focused: Some(input_target),
            hovered: None,
            pressed: None,
        },
    );

    let frame = prepare(&scene);
    let fill = frame
        .rectangles
        .iter()
        .find(|rect| rgba_near(rect.rgba, [0.1, 0.2, 0.3, 0.4]))
        .expect("fill rect exists");
    assert_eq!(fill.radii, radii);
    let item = &frame.text.items()[0];
    assert!(!item.interaction.selection_rects.is_empty());
    let clip = item.clip.expect("text interaction clip exists");
    assert!(item.interaction.selection_rects.iter().all(|selection| {
        selection.x >= clip.x
            && selection.y >= clip.y
            && selection.right() <= clip.right()
            && selection.bottom() <= clip.bottom()
    }));
}

#[test]
fn text_control_corner_frame_draws_independent_corner_segments() {
    let input_target = target("input.feedback");
    let control = text_control(input_target).with_style(RenderControlStyle {
        normal: RenderControlVisualStyle {
            corner_frame: Some(RenderControlCornerFrameStyle {
                color: [0.1, 0.9, 0.8, 1.0],
                width_px: 3.0,
                length_px: 24.0,
                offset_px: 2.0,
            }),
            ..RenderControlVisualStyle::default()
        },
        ..RenderControlStyle::default()
    });
    let scene = scene(vec![control], Vec::new(), InteractionVisualState::default());

    let frame = prepare(&scene);
    let corner_segments = frame
        .rectangles
        .iter()
        .filter(|rect| rgba_near(rect.rgba, [0.1, 0.9, 0.8, 1.0]))
        .collect::<Vec<_>>();

    assert_eq!(corner_segments.len(), 8);
    assert!(
        corner_segments
            .iter()
            .any(|rect| rect.bounds.width > rect.bounds.height),
        "corner frame should include horizontal segments"
    );
    assert!(
        corner_segments
            .iter()
            .any(|rect| rect.bounds.height > rect.bounds.width),
        "corner frame should include vertical segments"
    );
}

#[test]
fn supported_box_shadow_reaches_existing_shadow_pass_plan() {
    let button_target = target("button.submit_feedback");
    let scene = scene_with_button(button_target.clone(), InteractionVisualState::default());
    let frame = prepare(&scene);

    let shadow = frame
        .control_shadows
        .iter()
        .find(|shadow| shadow.target == button_target)
        .expect("button shadow plan exists");

    assert_eq!(shadow.plan.passes().len(), 1);
    assert_f32_near(shadow.plan.passes()[0].shadow.blur_radius_px, 18.0);
}

#[test]
fn backdrop_filter_reaches_runtime_control_backdrop_plan() {
    let input_target = target("input.feedback");
    let control = text_control(input_target.clone()).with_style(RenderControlStyle {
        normal: RenderControlVisualStyle {
            backdrop_filters: Some(RenderControlFilterList {
                filters: vec![RenderControlFilter::Blur { radius_px: 12.0 }],
            }),
            fill: Some([0.8, 0.8, 0.9, 0.42]),
            depth_milli: Some(2_000),
            ..RenderControlVisualStyle::default()
        },
        ..RenderControlStyle::default()
    });
    let scene = scene(vec![control], Vec::new(), InteractionVisualState::default());

    let frame = prepare(&scene);
    let backdrop = frame
        .control_backdrops
        .iter()
        .find(|backdrop| backdrop.target == input_target)
        .expect("control backdrop plan exists");

    assert_eq!(backdrop.bounds, HitRect::new(48.0, 48.0, 420.0, 48.0));
    assert_eq!(
        backdrop.sample_policy,
        RuntimeControlBackdropSamplePolicy::PriorFrameContent
    );
    assert_eq!(
        backdrop.filters.filters(),
        &[ViewFilter::Blur { radius_px: 12.0 }]
    );
}

#[test]
fn runtime_control_paint_span_carries_inline_backdrop_order() {
    let input_target = target("input.feedback");
    let control = text_control(input_target.clone()).with_style(RenderControlStyle {
        normal: RenderControlVisualStyle {
            backdrop_filters: Some(RenderControlFilterList {
                filters: vec![RenderControlFilter::Blur { radius_px: 8.0 }],
            }),
            fill: Some([0.2, 0.4, 0.6, 0.5]),
            ..RenderControlVisualStyle::default()
        },
        ..RenderControlStyle::default()
    });
    let scene = scene(vec![control], Vec::new(), InteractionVisualState::default());

    let frame = prepare(&scene);
    let paint = frame
        .control_paints
        .iter()
        .find(|paint| paint.target == input_target)
        .expect("control paint span exists");

    assert_eq!(paint.backdrop_range, 0..1);
    assert_eq!(paint.text_range.len(), 1);
    assert!(
        frame.rectangles[paint.rectangle_range.clone()]
            .iter()
            .any(|rect| rgba_near(rect.rgba, [0.2, 0.4, 0.6, 0.5]))
    );
    assert_eq!(
        frame.text.items()[paint.text_range.start].interaction.text,
        "hello"
    );
}

#[test]
fn text_controls_and_buttons_use_authored_font_family() {
    let input_target = target("input.feedback");
    let button_target = target("button.submit_feedback");
    let font_family = "Arcweft Demo, Yu Gothic, system-view".to_owned();
    let control = text_control(input_target.clone()).with_style(RenderControlStyle {
        normal: RenderControlVisualStyle {
            font_family: Some(font_family.clone()),
            ..RenderControlVisualStyle::default()
        },
        ..RenderControlStyle::default()
    });
    let button = RenderActionButton {
        target: button_target,
        label: "Send".to_owned(),
        enabled: true,
        containing_scroll_region: None,
        bounds: HitRect::new(484.0, 48.0, 128.0, 48.0),
        viewport_clip: None,
        style: RenderControlStyle {
            normal: RenderControlVisualStyle {
                font_family: Some(font_family.clone()),
                ..RenderControlVisualStyle::default()
            },
            ..RenderControlStyle::default()
        },
        action: RenderActionButtonAction::Noop,
    };
    let scene = scene(
        vec![control],
        vec![button],
        InteractionVisualState::default(),
    );

    let frame = prepare(&scene);
    let input_text = frame
        .text
        .items()
        .iter()
        .find(|text| text.interaction.text == "hello")
        .expect("input text block exists");
    let button_text = frame
        .text
        .items()
        .iter()
        .find(|text| text.interaction.text == "Send")
        .expect("button text block exists");

    assert_eq!(
        input_text.layout.runs[0].style.font_families(),
        &[
            TextFontFamily::Named("Arcweft Demo".to_owned()),
            TextFontFamily::Named("Yu Gothic".to_owned()),
            TextFontFamily::Named("system-view".to_owned()),
        ]
    );
    assert_eq!(
        button_text.layout.runs[0].style.font_families(),
        &[
            TextFontFamily::Named("Arcweft Demo".to_owned()),
            TextFontFamily::Named("Yu Gothic".to_owned()),
            TextFontFamily::Named("system-view".to_owned()),
        ]
    );
}

#[test]
fn text_control_uses_authored_font_metrics() {
    let control = text_control(target("input.metrics")).with_style(RenderControlStyle {
        normal: RenderControlVisualStyle {
            font_size_px: Some(18.0),
            line_height_px: Some(24.0),
            ..RenderControlVisualStyle::default()
        },
        ..RenderControlStyle::default()
    });
    let scene = scene(vec![control], Vec::new(), InteractionVisualState::default());

    let frame = prepare(&scene);
    let input_text = frame
        .text
        .items()
        .iter()
        .find(|text| text.interaction.text == "hello")
        .expect("input text block exists");

    assert_eq!(input_text.layout.runs[0].style.font_size_milli(), 18_000);
    assert_eq!(input_text.layout.runs[0].style.line_height_milli(), 24_000);
}

#[test]
fn foreground_filter_reaches_runtime_control_filter_plan() {
    let button_target = target("button.submit_feedback");
    let input_target = target("input.feedback");
    let button = RenderActionButton {
        target: button_target.clone(),
        label: "Send".to_owned(),
        enabled: true,
        containing_scroll_region: None,
        bounds: HitRect::new(484.0, 48.0, 128.0, 48.0),
        viewport_clip: None,
        style: RenderControlStyle {
            normal: RenderControlVisualStyle {
                filters: Some(RenderControlFilterList {
                    filters: vec![RenderControlFilter::Blur { radius_px: 2.5 }],
                }),
                ..RenderControlVisualStyle::default()
            },
            ..RenderControlStyle::default()
        },
        action: RenderActionButtonAction::Noop,
    };
    let scene = scene(
        vec![text_control(input_target)],
        vec![button],
        InteractionVisualState::default(),
    );

    let frame = prepare(&scene);
    let filter = frame
        .control_filters
        .iter()
        .find(|filter| filter.target == button_target)
        .expect("control filter plan exists");

    assert_eq!(
        filter.filters.filters(),
        &[ViewFilter::Blur { radius_px: 2.5 }]
    );
    let paint = frame
        .control_paints
        .iter()
        .find(|paint| paint.target == button_target)
        .expect("button paint span exists");
    assert_eq!(paint.filter_range, 0..1);
}

#[test]
fn runtime_control_color_matrix_filters_reach_view_filter_plan() {
    let input_target = target("input.feedback");
    let control = text_control(input_target.clone()).with_style(RenderControlStyle {
        normal: RenderControlVisualStyle {
            backdrop_filters: Some(RenderControlFilterList {
                filters: vec![
                    RenderControlFilter::Brightness { factor: 1.2 },
                    RenderControlFilter::Contrast { factor: 0.9 },
                    RenderControlFilter::Saturate { factor: 1.4 },
                    RenderControlFilter::HueRotateDegrees { degrees: 12.0 },
                    RenderControlFilter::Opacity { amount: 0.85 },
                ],
            }),
            ..RenderControlVisualStyle::default()
        },
        ..RenderControlStyle::default()
    });
    let scene = scene(vec![control], Vec::new(), InteractionVisualState::default());

    let frame = prepare(&scene);
    let backdrop = frame
        .control_backdrops
        .iter()
        .find(|backdrop| backdrop.target == input_target)
        .expect("control backdrop plan exists");

    assert_eq!(
        backdrop.filters.filters(),
        &[
            ViewFilter::Brightness(1.2),
            ViewFilter::Contrast(0.9),
            ViewFilter::Saturate(1.4),
            ViewFilter::HueRotateDegrees(12.0),
            ViewFilter::Opacity(0.85),
        ]
    );
}

#[test]
fn authored_control_depth_orders_text_inputs_and_buttons_together() {
    let input_target = target("input.feedback");
    let button_target = target("button.submit_feedback");
    let input = text_control(input_target.clone()).with_style(RenderControlStyle {
        normal: RenderControlVisualStyle {
            fill: Some([0.8, 0.1, 0.1, 0.75]),
            depth_milli: Some(3_000),
            ..RenderControlVisualStyle::default()
        },
        ..RenderControlStyle::default()
    });
    let button = RenderActionButton {
        target: button_target,
        label: "Send".to_owned(),
        enabled: true,
        containing_scroll_region: None,
        bounds: HitRect::new(72.0, 52.0, 128.0, 48.0),
        viewport_clip: None,
        style: RenderControlStyle {
            normal: RenderControlVisualStyle {
                fill: Some([0.1, 0.2, 0.8, 0.75]),
                depth_milli: Some(1_000),
                ..RenderControlVisualStyle::default()
            },
            ..RenderControlStyle::default()
        },
        action: RenderActionButtonAction::Noop,
    };
    let scene = scene(vec![input], vec![button], InteractionVisualState::default());

    let frame = prepare(&scene);

    let button_rect = frame
        .rectangles
        .iter()
        .position(|rect| rgba_near(rect.rgba, [0.1, 0.2, 0.8, 0.75]))
        .expect("button rectangle exists");
    let input_rect = frame
        .rectangles
        .iter()
        .position(|rect| rgba_near(rect.rgba, [0.8, 0.1, 0.1, 0.75]))
        .expect("input rectangle exists");
    assert!(
        button_rect < input_rect,
        "lower-depth button should be painted before higher-depth text input"
    );
}

fn scene_with_button(
    button_target: InteractionTarget,
    interaction: InteractionVisualState,
) -> RenderScene {
    let input_target = target("input.feedback");
    scene(
        vec![text_control(input_target.clone())],
        vec![RenderActionButton {
            target: button_target,
            label: "Send".to_owned(),
            enabled: true,
            containing_scroll_region: None,
            bounds: HitRect::new(484.0, 48.0, 128.0, 48.0),
            viewport_clip: None,
            style: RenderControlStyle {
                normal: RenderControlVisualStyle {
                    fill: Some([0.07, 0.12, 0.09, 0.72]),
                    text: Some([240, 248, 255, 255]),
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
                hover: Some(RenderControlVisualStyle {
                    fill: Some([0.1, 0.2, 0.3, 0.8]),
                    ..RenderControlVisualStyle::default()
                }),
                ..RenderControlStyle::default()
            },
            action: RenderActionButtonAction::Noop,
        }],
        interaction,
    )
}

fn text_control(target: InteractionTarget) -> RenderTextInputControl {
    RenderTextInputControl::new(
        target,
        TextInputSessionId(41),
        "hello",
        TextRange::new(TextByteOffset(5), TextByteOffset(5)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(48.0, 48.0, 420.0, 48.0),
    )
}

fn scene(
    text_inputs: Vec<RenderTextInputControl>,
    action_buttons: Vec<RenderActionButton>,
    interaction: InteractionVisualState,
) -> RenderScene {
    RenderScene {
        content_avoidance_regions: Vec::new(),
        choices: Vec::new(),
        text_inputs,
        action_buttons,
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: Vec::new(),
        viewport: RenderViewport {
            logical_width: 800.0,
            logical_height: 480.0,
            physical_width: 800,
            physical_height: 480,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction,
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
    }
}

fn target(value: &str) -> InteractionTarget {
    InteractionTarget::new(PublicId::try_new(value).unwrap())
}

fn rgba_near(actual: [f32; 4], expected: [f32; 4]) -> bool {
    actual
        .into_iter()
        .zip(expected)
        .all(|(actual, expected)| (actual - expected).abs() <= f32::EPSILON)
}

fn assert_f32_near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= f32::EPSILON,
        "expected {actual} to equal {expected}"
    );
}
