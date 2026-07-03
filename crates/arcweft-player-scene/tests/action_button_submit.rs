use arcweft_id::PublicId;
use arcweft_player_scene::input::{InputController, InputDiagnosticKind};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::{InteractionTarget, KeyPhase, PointerId, ViewportPoint};
use arcweft_presentation::semantic::SemanticRole;
use arcweft_presentation::text_input::{
    TextByteOffset, TextControlValue, TextControlWriteBackKind, TextInput, TextInputOperation,
    TextInputOptions, TextInputSerial, TextInputSessionId, TextRange, TextRevision,
};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, RenderActionButton, RenderActionButtonAction,
    RenderPreferences, RenderScene, RenderTextInputControl, RenderTextSubmitImePolicy,
    RenderViewport, SharedFramePlanner,
};

fn target(value: &str) -> InteractionTarget {
    InteractionTarget::new(PublicId::try_new(value).unwrap())
}

fn scene(ime_policy: RenderTextSubmitImePolicy) -> RenderScene {
    let input_target = target("input.feedback");
    let button_target = target("button.submit_feedback");
    let selection = TextRange::new(TextByteOffset(5), TextByteOffset(5));
    RenderScene {
        dialogue: None,
        choices: Vec::new(),
        text_inputs: vec![RenderTextInputControl::new(
            input_target.clone(),
            TextInputSessionId(41),
            "hello",
            selection,
            TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(48.0, 48.0, 420.0, 48.0),
        )],
        action_buttons: vec![RenderActionButton {
            target: button_target,
            label: "Send".to_owned(),
            enabled: true,
            bounds: HitRect::new(484.0, 48.0, 128.0, 48.0),
            action: RenderActionButtonAction::TextInputSubmit {
                input_target,
                session: TextInputSessionId(41),
                value: TextControlValue::plain("hello"),
                selection,
                revision: TextRevision::default(),
                ime_policy,
            },
        }],
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
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
    }
}

#[test]
fn pointer_activation_on_action_button_emits_submit_write_back() {
    let scene = scene(RenderTextSubmitImePolicy::Commit);
    let frame = SharedFramePlanner::prepare(&scene).unwrap();
    let mut input = InputController::default();
    input.activate_text_control(&scene.text_inputs[0]).unwrap();
    let position = ViewportPoint::new(500.0, 60.0);

    input.pointer_down(&frame, PointerId(0), position);
    let outcome = input.pointer_up(&frame, PointerId(0), position);

    assert_eq!(outcome.text_control_write_backs().len(), 1);
    let write_back = &outcome.text_control_write_backs()[0];
    assert_eq!(write_back.kind(), TextControlWriteBackKind::Submit);
    assert_eq!(write_back.value().as_str(), "hello");
}

#[test]
fn keyboard_activation_on_focused_action_button_emits_submit_write_back() {
    let scene = scene(RenderTextSubmitImePolicy::Commit);
    let frame = SharedFramePlanner::prepare(&scene).unwrap();
    let mut input = InputController::default();
    input.activate_text_control(&scene.text_inputs[0]).unwrap();
    input.keyboard(&frame, "Tab", KeyPhase::Down);

    let outcome = input.keyboard(&frame, "Enter", KeyPhase::Down);

    assert_eq!(outcome.text_control_write_backs().len(), 1);
    assert_eq!(
        outcome.text_control_write_backs()[0].kind(),
        TextControlWriteBackKind::Submit
    );
}

#[test]
fn arrow_navigation_moves_from_text_field_to_right_action_button() {
    let scene = scene(RenderTextSubmitImePolicy::Commit);
    let frame = SharedFramePlanner::prepare(&scene).unwrap();
    let mut input = InputController::default();
    input.activate_text_control(&scene.text_inputs[0]).unwrap();
    input.ensure_choice_focus(&frame);

    input.keyboard(&frame, "ArrowRight", KeyPhase::Down);
    let outcome = input.keyboard(&frame, "Enter", KeyPhase::Down);

    assert_eq!(outcome.text_control_write_backs().len(), 1);
    assert_eq!(
        outcome.text_control_write_backs()[0].kind(),
        TextControlWriteBackKind::Submit
    );
}

#[test]
fn reject_ime_policy_reports_diagnostic_without_submit() {
    let scene = scene(RenderTextSubmitImePolicy::Reject);
    let frame = SharedFramePlanner::prepare(&scene).unwrap();
    let mut input = InputController::default();
    input.activate_text_control(&scene.text_inputs[0]).unwrap();
    input
        .text_input(
            &frame,
            TextInput::single(
                TextInputSessionId(41),
                TextInputSerial(1),
                TextInputOperation::StartComposition,
            ),
        )
        .unwrap();

    input.pointer_down(&frame, PointerId(0), ViewportPoint::new(500.0, 60.0));
    let outcome = input.pointer_up(&frame, PointerId(0), ViewportPoint::new(500.0, 60.0));

    assert!(outcome.text_control_write_backs().is_empty());
    assert_eq!(outcome.diagnostics.len(), 1);
    assert_eq!(
        outcome.diagnostics[0].kind,
        InputDiagnosticKind::ImeCompositionRejectedActionButtonSubmit
    );
}
