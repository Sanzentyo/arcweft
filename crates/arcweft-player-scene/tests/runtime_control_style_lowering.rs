use arcweft_bundle::resource_codec::view::{
    CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization, ViewInputKind,
    ViewInputPurpose, ViewRuntimeActionButton, ViewRuntimeActionButtonAction,
    ViewRuntimeButtonBounds, ViewRuntimeControlCornerFrameStyle, ViewRuntimeControlFilter,
    ViewRuntimeControlFilterList, ViewRuntimeControlVisualStyle, ViewRuntimeTextControl,
    ViewRuntimeTextControlBounds, ViewRuntimeTextControlHandlers, ViewRuntimeTextControlOptions,
    ViewRuntimeTextSelection, ViewSecureInputPolicy, ViewTextSelectionPolicy,
    ViewTextShortcutPolicy, ViewTextTabPolicy, ViewTextVerticalNavigationPolicy,
};
use arcweft_player_scene::action_buttons::RuntimeActionButtonLowerer;
use arcweft_player_scene::input::InputController;
use arcweft_player_scene::text_controls::RuntimeTextControlLowerer;
use arcweft_presentation::appearance::PresentationColor;
use arcweft_render_wgpu::geometry::{RenderControlFilter, RenderControlFilterList};

#[test]
fn runtime_text_control_style_reaches_render_text_input_control() {
    let mut input = InputController::default();
    let runtime =
        text_control_with_style("input.feedback", styled_fill_depth(12, 24, 48, 192, 1_700));

    let render = RuntimeTextControlLowerer::lower_for_frame(&mut input, &[runtime])
        .expect("text control lowers");

    assert_eq!(
        render[0].style.fill,
        Some([12.0 / 255.0, 24.0 / 255.0, 48.0 / 255.0, 192.0 / 255.0])
    );
    assert_eq!(render[0].style.depth_milli, Some(1_700));
}

#[test]
fn runtime_action_button_style_reaches_render_action_button() {
    let input = RuntimeTextControlLowerer::lower_controls(&[text_control_with_style(
        "input.feedback",
        ViewRuntimeControlVisualStyle::default(),
    )])
    .expect("text input lowers");
    let button = ViewRuntimeActionButton {
        public_id: "button.submit_feedback".to_owned(),
        target: "button.submit_feedback".to_owned(),
        view: None,
        containing_scroll_region: None,
        label: "Send".to_owned(),
        enabled: true,
        bounds: ViewRuntimeButtonBounds::new(484_000, 48_000, 128_000, 48_000),
        action: ViewRuntimeActionButtonAction::Noop,
        style: styled_fill_depth(64, 96, 64, 255, 2_100),
    };

    let render =
        RuntimeActionButtonLowerer::lower_buttons(&[button], &input).expect("action button lowers");

    assert_eq!(
        render[0].style.fill,
        Some([64.0 / 255.0, 96.0 / 255.0, 64.0 / 255.0, 1.0])
    );
    assert_eq!(render[0].style.depth_milli, Some(2_100));
}

#[test]
fn runtime_control_backdrop_filter_reaches_render_style() {
    let mut input = InputController::default();
    let runtime = text_control_with_style(
        "input.feedback",
        ViewRuntimeControlVisualStyle {
            backdrop_filters: Some(ViewRuntimeControlFilterList {
                filters: vec![ViewRuntimeControlFilter::Blur {
                    radius_milli: 12_000,
                }],
            }),
            ..ViewRuntimeControlVisualStyle::default()
        },
    );

    let render = RuntimeTextControlLowerer::lower_for_frame(&mut input, &[runtime])
        .expect("text control lowers");

    assert_eq!(
        render[0].style.backdrop_filters,
        Some(RenderControlFilterList {
            filters: vec![RenderControlFilter::Blur { radius_px: 12.0 }]
        })
    );
}

#[test]
fn runtime_control_font_family_reaches_render_style() {
    let mut input = InputController::default();
    let runtime = text_control_with_style(
        "input.feedback",
        ViewRuntimeControlVisualStyle {
            font_family: Some("Arcweft Demo, Yu Gothic, system-view".to_owned()),
            ..ViewRuntimeControlVisualStyle::default()
        },
    );

    let render = RuntimeTextControlLowerer::lower_for_frame(&mut input, &[runtime])
        .expect("text control lowers");

    assert_eq!(
        render[0].style.font_family.as_deref(),
        Some("Arcweft Demo, Yu Gothic, system-view")
    );
}

#[test]
fn runtime_text_area_preserves_current_visual_style() {
    let mut input = InputController::default();
    let target = "input.product_brief";
    let runtime = text_control_with_kind_and_style(
        target,
        ViewInputKind::TextArea,
        ViewRuntimeControlVisualStyle {
            fill: Some(PresentationColor::rgba(8, 14, 24, 164)),
            text: Some(PresentationColor::rgba(244, 247, 251, 255)),
            placeholder: Some(PresentationColor::rgba(148, 163, 184, 255)),
            selection: Some(PresentationColor::rgba(94, 234, 212, 116)),
            caret: Some(PresentationColor::rgba(94, 234, 212, 255)),
            composition_underline: Some(PresentationColor::rgba(251, 191, 36, 255)),
            font_family: Some(
                "Arcweft Demo, Yu Gothic View, Yu Gothic, Meiryo, system-view".to_owned(),
            ),
            letter_spacing_milli: Some(750),
            focus_ring: Some(
                arcweft_bundle::resource_codec::view::ViewRuntimeControlFocusRingStyle {
                    color: PresentationColor::rgba(94, 234, 212, 255),
                    width_milli: 2_000,
                    offset_milli: 0,
                },
            ),
            ..ViewRuntimeControlVisualStyle::default()
        },
    );

    let render = RuntimeTextControlLowerer::lower_for_frame(&mut input, &[runtime])
        .expect("text area lowers");

    assert_eq!(
        render[0].role,
        arcweft_presentation::semantic::SemanticRole::TextArea
    );
    assert_eq!(
        render[0].style.fill,
        Some([8.0 / 255.0, 14.0 / 255.0, 24.0 / 255.0, 164.0 / 255.0])
    );
    assert_eq!(
        render[0].style.font_family.as_deref(),
        Some("Arcweft Demo, Yu Gothic View, Yu Gothic, Meiryo, system-view")
    );
    assert!(render[0].style.focus_ring.is_some());
    assert_eq!(render[0].style.placeholder, Some([148, 163, 184, 255]));
    assert_eq!(render[0].style.letter_spacing_milli, Some(750));
    assert_eq!(
        render[0].style.composition_underline,
        Some([251.0 / 255.0, 191.0 / 255.0, 36.0 / 255.0, 1.0])
    );
}

#[test]
fn runtime_control_corner_frame_reaches_render_style() {
    let mut input = InputController::default();
    let runtime = text_control_with_style(
        "input.feedback",
        ViewRuntimeControlVisualStyle {
            corner_frame: Some(ViewRuntimeControlCornerFrameStyle {
                color: PresentationColor::rgba(94, 234, 212, 220),
                width_milli: 3_000,
                length_milli: 24_000,
                offset_milli: 2_000,
            }),
            ..ViewRuntimeControlVisualStyle::default()
        },
    );

    let render = RuntimeTextControlLowerer::lower_for_frame(&mut input, &[runtime])
        .expect("text control lowers");
    let corner_frame = render[0].style.corner_frame.expect("corner frame lowers");

    assert_rgba_near(
        corner_frame.color,
        [94.0 / 255.0, 234.0 / 255.0, 212.0 / 255.0, 220.0 / 255.0],
    );
    assert_f32_near(corner_frame.width_px, 3.0);
    assert_f32_near(corner_frame.length_px, 24.0);
    assert_f32_near(corner_frame.offset_px, 2.0);
}

fn assert_rgba_near(actual: [f32; 4], expected: [f32; 4]) {
    actual
        .into_iter()
        .zip(expected)
        .for_each(|(actual, expected)| assert_f32_near(actual, expected));
}

fn assert_f32_near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= f32::EPSILON,
        "expected {actual} to equal {expected}"
    );
}

fn styled_fill_depth(
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
    depth_milli: i32,
) -> ViewRuntimeControlVisualStyle {
    ViewRuntimeControlVisualStyle {
        fill: Some(PresentationColor::rgba(red, green, blue, alpha)),
        depth_milli: Some(depth_milli),
        ..ViewRuntimeControlVisualStyle::default()
    }
}

fn text_control_with_style(
    public_id: &str,
    style: ViewRuntimeControlVisualStyle,
) -> ViewRuntimeTextControl {
    text_control_with_kind_and_style(public_id, ViewInputKind::TextField, style)
}

fn text_control_with_kind_and_style(
    public_id: &str,
    kind: ViewInputKind,
    style: ViewRuntimeControlVisualStyle,
) -> ViewRuntimeTextControl {
    ViewRuntimeTextControl {
        public_id: public_id.to_owned(),
        target: public_id.to_owned(),
        view: None,
        containing_scroll_region: None,
        session: 41,
        value: "hello".to_owned(),
        selection: ViewRuntimeTextSelection::new(5, 5),
        options: ViewRuntimeTextControlOptions {
            purpose: ViewInputPurpose::Text,
            autocorrect: TextAssistPolicy::PlatformDefault,
            spellcheck: TextAssistPolicy::PlatformDefault,
            capitalization: TextCapitalization::None,
            enter_key: EnterKeyHint::Default,
            multiline: kind.is_multiline(),
            selection_policy: ViewTextSelectionPolicy::Enabled,
            shortcut_policy: ViewTextShortcutPolicy::Enabled,
            tab_policy: ViewTextTabPolicy::FocusNavigation,
            vertical_navigation_policy: ViewTextVerticalNavigationPolicy::LogicalLine,
            secure_policy: ViewSecureInputPolicy::Plain,
            composition_on_blur: CompositionOnBlurPolicy::Commit,
        },
        kind,
        bounds: ViewRuntimeTextControlBounds::from_px(48, 48, 420, 48),
        label: Some("Feedback".to_owned()),
        handlers: ViewRuntimeTextControlHandlers::default(),
        style,
    }
}
