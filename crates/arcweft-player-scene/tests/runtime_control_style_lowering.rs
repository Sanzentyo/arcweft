use arcweft_bundle::resource_codec::ui::{
    CompositionOnBlurPolicy, EnterKeyHint, RgbaColor, TextAssistPolicy, TextCapitalization,
    UiInputKind, UiInputPurpose, UiRuntimeActionButton, UiRuntimeActionButtonAction,
    UiRuntimeButtonBounds, UiRuntimeControlStyle, UiRuntimeControlVisualStyle,
    UiRuntimeTextControl, UiRuntimeTextControlBounds, UiRuntimeTextControlHandlers,
    UiRuntimeTextControlOptions, UiRuntimeTextSelection, UiSecureInputPolicy,
    UiTextSelectionPolicy, UiTextShortcutPolicy, UiTextSubmitImePolicy, UiTextTabPolicy,
};
use arcweft_player_scene::action_buttons::RuntimeActionButtonLowerer;
use arcweft_player_scene::input::InputController;
use arcweft_player_scene::text_controls::RuntimeTextControlLowerer;

#[test]
fn runtime_text_control_style_reaches_render_text_input_control() {
    let mut input = InputController::default();
    let runtime =
        text_control_with_style("input.feedback", styled_fill_depth(12, 24, 48, 192, 1_700));

    let render = RuntimeTextControlLowerer::lower_for_frame(&mut input, &[runtime])
        .expect("text control lowers");

    assert_eq!(
        render[0].style.normal.fill,
        Some([12.0 / 255.0, 24.0 / 255.0, 48.0 / 255.0, 192.0 / 255.0])
    );
    assert_eq!(render[0].style.normal.depth_milli, Some(1_700));
}

#[test]
fn runtime_action_button_style_reaches_render_action_button() {
    let input = RuntimeTextControlLowerer::lower_controls(&[text_control_with_style(
        "input.feedback",
        UiRuntimeControlStyle::default(),
    )])
    .expect("text input lowers");
    let button = UiRuntimeActionButton {
        public_id: "button.submit_feedback".to_owned(),
        target: "button.submit_feedback".to_owned(),
        label: "Send".to_owned(),
        enabled: true,
        bounds: UiRuntimeButtonBounds::new(484_000, 48_000, 128_000, 48_000),
        action: UiRuntimeActionButtonAction::TextInputSubmit {
            input_target: "input.feedback".to_owned(),
            ime_policy: UiTextSubmitImePolicy::Commit,
        },
        style: styled_fill_depth(64, 96, 64, 255, 2_100),
    };

    let render =
        RuntimeActionButtonLowerer::lower_buttons(&[button], &input).expect("action button lowers");

    assert_eq!(
        render[0].style.normal.fill,
        Some([64.0 / 255.0, 96.0 / 255.0, 64.0 / 255.0, 1.0])
    );
    assert_eq!(render[0].style.normal.depth_milli, Some(2_100));
}

fn styled_fill_depth(
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
    depth_milli: i32,
) -> UiRuntimeControlStyle {
    UiRuntimeControlStyle {
        normal: UiRuntimeControlVisualStyle {
            fill: Some(RgbaColor::rgba(red, green, blue, alpha)),
            depth_milli: Some(depth_milli),
            ..UiRuntimeControlVisualStyle::default()
        },
        ..UiRuntimeControlStyle::default()
    }
}

fn text_control_with_style(public_id: &str, style: UiRuntimeControlStyle) -> UiRuntimeTextControl {
    UiRuntimeTextControl {
        public_id: public_id.to_owned(),
        target: public_id.to_owned(),
        session: 41,
        value: "hello".to_owned(),
        selection: UiRuntimeTextSelection::new(5, 5),
        options: UiRuntimeTextControlOptions {
            purpose: UiInputPurpose::Text,
            autocorrect: TextAssistPolicy::PlatformDefault,
            spellcheck: TextAssistPolicy::PlatformDefault,
            capitalization: TextCapitalization::None,
            enter_key: EnterKeyHint::Default,
            multiline: false,
            selection_policy: UiTextSelectionPolicy::Enabled,
            shortcut_policy: UiTextShortcutPolicy::Enabled,
            tab_policy: UiTextTabPolicy::FocusNavigation,
            secure_policy: UiSecureInputPolicy::Plain,
            composition_on_blur: CompositionOnBlurPolicy::Commit,
        },
        kind: UiInputKind::TextField,
        bounds: UiRuntimeTextControlBounds::from_px(48, 48, 420, 48),
        label: Some("Feedback".to_owned()),
        handlers: UiRuntimeTextControlHandlers::default(),
        style,
    }
}
