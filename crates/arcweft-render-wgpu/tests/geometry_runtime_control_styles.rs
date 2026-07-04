use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::semantic::SemanticRole;
use arcweft_presentation::text_input::{
    TextByteOffset, TextControlValue, TextInputOptions, TextInputSessionId, TextRange, TextRevision,
};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, RenderActionButton, RenderActionButtonAction,
    RenderControlBorderStyle, RenderControlFocusRingStyle, RenderControlShadow,
    RenderControlShadowKind, RenderControlStyle, RenderControlVisualStyle, RenderPreferences,
    RenderScene, RenderTextInputControl, RenderTextSubmitImePolicy, RenderViewport,
    SharedFramePlanner,
};

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

    let frame = SharedFramePlanner::prepare(&scene).expect("frame prepares");

    assert!(
        frame
            .rectangles
            .iter()
            .any(|rect| rgba_near(rect.rgba, [0.1, 0.2, 0.3, 0.8]))
    );
    assert!(
        frame
            .text
            .iter()
            .any(|text| text.text == "Send" && text.rgba == [240, 248, 255, 255])
    );
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

    let frame = SharedFramePlanner::prepare(&scene).expect("frame prepares");

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
}

#[test]
fn supported_box_shadow_reaches_existing_shadow_pass_plan() {
    let button_target = target("button.submit_feedback");
    let scene = scene_with_button(button_target.clone(), InteractionVisualState::default());
    let frame = SharedFramePlanner::prepare(&scene).expect("frame prepares");

    let shadow = frame
        .control_shadows
        .iter()
        .find(|shadow| shadow.target == button_target)
        .expect("button shadow plan exists");

    assert_eq!(shadow.plan.passes().len(), 1);
    assert_f32_near(shadow.plan.passes()[0].shadow.blur_radius_px, 18.0);
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
            bounds: HitRect::new(484.0, 48.0, 128.0, 48.0),
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
            action: RenderActionButtonAction::TextInputSubmit {
                input_target,
                session: TextInputSessionId(41),
                value: TextControlValue::plain("hello"),
                selection: TextRange::new(TextByteOffset(5), TextByteOffset(5)),
                revision: TextRevision::default(),
                ime_policy: RenderTextSubmitImePolicy::Commit,
            },
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
        dialogue: None,
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
