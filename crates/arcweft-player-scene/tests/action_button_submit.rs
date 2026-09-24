use arcweft_bundle::resource_codec::view::{
    ViewActionPayloadResource, ViewActionTextControlPayloadField, ViewRuntimeActionButton,
    ViewRuntimeActionButtonAction, ViewRuntimeButtonBounds, ViewRuntimeControlVisualStyle,
};
use arcweft_id::PublicId;
use arcweft_interaction_model::input::InputActionId;
use arcweft_player_scene::action_buttons::RuntimeActionButtonLowerer;
use arcweft_player_scene::fonts::DEFAULT_PLAYER_FONT_RESOURCE_BYTES;
use arcweft_player_scene::input::{
    DialogueInputAction, InputController, InputDiagnosticKind, InputPointerModifiers,
};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::{InteractionTarget, KeyPhase, PointerId, ViewportPoint};
use arcweft_presentation::semantic::{
    SemanticActionError, SemanticNode, SemanticRole, SemanticTree,
};
use arcweft_presentation::text_input::{
    TextByteOffset, TextInputOptions, TextInputSessionId, TextRange,
};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, PreparedDialogueViewState, PreparedFrame,
    RenderActionButton, RenderActionButtonAction, RenderControlVisualStyle, RenderPreferences,
    RenderScene, RenderTextInputControl, RenderViewport, SharedFramePlanContext,
};

fn prepare(
    scene: &RenderScene,
) -> Result<PreparedFrame, arcweft_render_wgpu::geometry::FramePlanError> {
    let mut planner = SharedFramePlanContext::new();
    for bytes in DEFAULT_PLAYER_FONT_RESOURCE_BYTES {
        planner.register_font_bytes(bytes.to_vec())?;
    }
    planner.prepare(scene)
}

fn target(value: &str) -> InteractionTarget {
    InteractionTarget::new(PublicId::try_new(value).unwrap())
}

fn scene_with_text_input_and_action_button() -> RenderScene {
    let input_target = target("input.feedback");
    let button_target = target("button.submit_feedback");
    let selection = TextRange::new(TextByteOffset(5), TextByteOffset(5));
    RenderScene {
        content_avoidance_regions: Vec::new(),
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
            dialogue_mount: None,
            label: "Send".to_owned(),
            enabled: true,
            containing_scroll_region: None,
            bounds: HitRect::new(484.0, 48.0, 128.0, 48.0),
            viewport_clip: None,
            style: RenderControlVisualStyle::default(),
            action: RenderActionButtonAction::ActionInvoke {
                action: PublicId::try_new("action.feedback.submit_name").unwrap(),
                payload: Some("hello".to_owned()),
            },
        }],
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
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
    }
}

fn action_invoke_scene() -> RenderScene {
    RenderScene {
        content_avoidance_regions: Vec::new(),
        choices: Vec::new(),
        text_inputs: Vec::new(),
        action_buttons: vec![RenderActionButton {
            target: target("button.continue"),
            dialogue_mount: None,
            label: "Continue".to_owned(),
            enabled: true,
            containing_scroll_region: None,
            bounds: HitRect::new(48.0, 48.0, 180.0, 48.0),
            viewport_clip: None,
            style: RenderControlVisualStyle::default(),
            action: RenderActionButtonAction::ActionInvoke {
                action: PublicId::try_new("action.feedback.submit_name").unwrap(),
                payload: Some("Ada".to_owned()),
            },
        }],
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
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
    }
}

fn prepare_with_dialogue_view(scene: &RenderScene) -> PreparedFrame {
    let mut frame = prepare(scene).expect("frame prepares");
    frame.push_dialogue_view(PreparedDialogueViewState {
        dialogue: 19,
        entry: 23,
        mount: arcweft_view::ViewMountId::from_raw(7),
        revision: 31,
        instance: 29,
        stage: 4,
        bounds: HitRect::new(32.0, 300.0, 736.0, 148.0),
        reveal_complete: false,
        advance_available: false,
        primary_action: None,
    });
    frame
}

#[test]
fn pointer_activation_on_action_button_emits_semantic_action() {
    let scene = scene_with_text_input_and_action_button();
    let frame = prepare(&scene).unwrap();
    let mut input = InputController::default();
    input.activate_text_control(&scene.text_inputs[0]).unwrap();
    let position = ViewportPoint::new(500.0, 60.0);

    input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    let outcome = input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

    assert!(outcome.text_control_write_backs().is_empty());
    assert_eq!(outcome.actions().len(), 1);
    assert_eq!(
        outcome.actions()[0].kind().as_str(),
        "action.feedback.submit_name"
    );
    assert_eq!(
        outcome.actions()[0].payload().map(String::as_str),
        Some("hello")
    );
}

#[test]
fn pointer_activation_on_action_button_does_not_implicitly_advance_dialogue() {
    let scene = action_invoke_scene();
    let frame = prepare_with_dialogue_view(&scene);
    let mut input = InputController::default();
    let position = ViewportPoint::new(80.0, 72.0);

    input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    let outcome = input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

    assert!(!outcome.dialogue_progress.advances());
    assert_eq!(outcome.actions().len(), 1);
    assert_eq!(
        outcome.actions()[0].kind().as_str(),
        "action.feedback.submit_name"
    );
}

#[test]
fn accepted_dialogue_action_button_emits_the_exact_prepared_dialogue_token() {
    let mut scene = action_invoke_scene();
    scene.action_buttons[0].dialogue_mount = Some(arcweft_view::ViewMountId::from_raw(7));
    let frame = prepare_with_dialogue_view(&scene);
    let mut input = InputController::default();
    let position = ViewportPoint::new(64.0, 64.0);

    input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    let outcome = input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

    assert_eq!(outcome.actions().len(), 1);
    assert_eq!(
        outcome.dialogue_input_actions,
        vec![DialogueInputAction {
            observed: arcweft_view::DialogueAdvanceTarget::new(
                arcweft_view::DialoguePresentationId::new(19),
                arcweft_view::DialogueEntryId::new(23),
                arcweft_view::DialogueInstanceId::new(29),
                arcweft_view::DialogueStageIndex::new(4),
                arcweft_view::DialogueRevision::new(31),
            ),
            action: InputActionId::new("action.feedback.submit_name").unwrap(),
        }]
    );
    assert!(!outcome.dialogue_progress.advances());
}

#[test]
fn dialogue_action_button_with_unmatched_root_mount_emits_only_generic_action() {
    let mut scene = action_invoke_scene();
    scene.action_buttons[0].dialogue_mount = Some(arcweft_view::ViewMountId::from_raw(8));
    let frame = prepare_with_dialogue_view(&scene);
    let mut input = InputController::default();
    let position = ViewportPoint::new(64.0, 64.0);

    input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    let outcome = input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

    assert_eq!(outcome.actions().len(), 1);
    assert!(outcome.dialogue_input_actions.is_empty());
}

#[test]
fn pointer_activation_on_noop_button_does_not_emit_action_or_write_back() {
    let mut scene = scene_with_text_input_and_action_button();
    scene.action_buttons[0].action = RenderActionButtonAction::Noop;
    let frame = prepare(&scene).unwrap();
    let mut input = InputController::default();
    input.activate_text_control(&scene.text_inputs[0]).unwrap();
    let position = ViewportPoint::new(500.0, 60.0);

    input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    let outcome = input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

    assert!(outcome.actions().is_empty());
    assert!(outcome.text_control_write_backs().is_empty());
    assert!(outcome.diagnostics.is_empty());
}

#[test]
fn pointer_activation_on_action_invoke_button_emits_semantic_action() {
    let scene = action_invoke_scene();
    let frame = prepare(&scene).unwrap();
    let mut input = InputController::default();
    let position = ViewportPoint::new(64.0, 64.0);

    input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    let outcome = input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

    assert!(outcome.text_control_write_backs().is_empty());
    assert_eq!(outcome.actions().len(), 1);
    assert_eq!(
        outcome.actions()[0].kind().as_str(),
        "action.feedback.submit_name"
    );
    assert_eq!(
        outcome.actions()[0].payload().map(String::as_str),
        Some("Ada")
    );
}

#[test]
fn pointer_activation_reports_semantic_action_rejection() {
    let mut scene = action_invoke_scene();
    scene.action_buttons[0].dialogue_mount = Some(arcweft_view::ViewMountId::from_raw(7));
    let mut frame = prepare_with_dialogue_view(&scene);
    let original_node = frame
        .semantics
        .find(&target("button.continue"))
        .expect("button semantic node exists")
        .clone();
    frame.semantics = SemanticTree::default();
    frame.semantics.push(
        SemanticNode::new(
            original_node.layer().clone(),
            original_node.target().clone(),
            original_node.role(),
            original_node.bounds(),
        )
        .with_label("Continue")
        .with_enabled(true),
    );
    let mut input = InputController::default();
    let position = ViewportPoint::new(64.0, 64.0);

    input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    let outcome = input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

    assert!(outcome.actions().is_empty());
    assert_eq!(outcome.diagnostics.len(), 1);
    assert_eq!(outcome.diagnostics[0].target, target("button.continue"));
    assert_eq!(
        outcome.diagnostics[0].kind,
        InputDiagnosticKind::SemanticActionRejected {
            action: PublicId::try_new("action.feedback.submit_name").unwrap(),
            reason: SemanticActionError::UndeclaredAction {
                target: target("button.continue"),
                action: PublicId::try_new("action.feedback.submit_name").unwrap(),
            },
        }
    );
    assert!(outcome.dialogue_input_actions.is_empty());
}

#[test]
fn runtime_action_invoke_payload_reads_text_control_projection() {
    let input_target = target("input.visitor_name");
    let text_inputs = vec![RenderTextInputControl::new(
        input_target,
        TextInputSessionId(41),
        "Ada",
        TextRange::new(TextByteOffset(3), TextByteOffset(3)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(48.0, 48.0, 420.0, 48.0),
    )];
    let buttons = RuntimeActionButtonLowerer::lower_buttons(
        &[ViewRuntimeActionButton {
            public_id: "button.continue".to_owned(),
            target: "button.continue".to_owned(),
            dialogue_mount: Some(arcweft_view::ViewMountId::from_raw(42)),
            view: None,
            containing_scroll_region: None,
            label: "Continue".to_owned(),
            enabled: true,
            bounds: ViewRuntimeButtonBounds::new(484_000, 48_000, 180_000, 48_000),
            action: ViewRuntimeActionButtonAction::ActionInvoke {
                action: "action.feedback.submit_name".to_owned(),
                payload: Some(ViewActionPayloadResource::TextControlProjection {
                    input: "input.visitor_name".to_owned(),
                    field: ViewActionTextControlPayloadField::Text,
                }),
            },
            style: ViewRuntimeControlVisualStyle::default(),
        }],
        &text_inputs,
    )
    .expect("runtime button lowers");
    assert_eq!(
        buttons[0].dialogue_mount,
        Some(arcweft_view::ViewMountId::from_raw(42))
    );

    let RenderActionButtonAction::ActionInvoke { payload, .. } = &buttons[0].action else {
        panic!("expected action invoke render action");
    };
    assert_eq!(payload.as_deref(), Some("Ada"));

    let literal_buttons = RuntimeActionButtonLowerer::lower_buttons(
        &[ViewRuntimeActionButton {
            public_id: "button.literal".to_owned(),
            target: "button.literal".to_owned(),
            dialogue_mount: None,
            view: None,
            containing_scroll_region: None,
            label: "Literal".to_owned(),
            enabled: true,
            bounds: ViewRuntimeButtonBounds::new(484_000, 112_000, 180_000, 48_000),
            action: ViewRuntimeActionButtonAction::ActionInvoke {
                action: "action.feedback.submit_name".to_owned(),
                payload: Some(ViewActionPayloadResource::LiteralString {
                    value: "input.visitor_name.text".to_owned(),
                }),
            },
            style: ViewRuntimeControlVisualStyle::default(),
        }],
        &text_inputs,
    )
    .expect("literal payload button lowers");
    let RenderActionButtonAction::ActionInvoke { payload, .. } = &literal_buttons[0].action else {
        panic!("expected action invoke render action");
    };
    assert_eq!(payload.as_deref(), Some("input.visitor_name.text"));
}

#[test]
fn keyboard_activation_on_focused_action_button_emits_semantic_action() {
    let scene = scene_with_text_input_and_action_button();
    let frame = prepare(&scene).unwrap();
    let mut input = InputController::default();
    input.activate_text_control(&scene.text_inputs[0]).unwrap();
    input.keyboard(&frame, "Tab", KeyPhase::Down);

    let outcome = input.keyboard(&frame, "Enter", KeyPhase::Down);

    assert!(outcome.text_control_write_backs().is_empty());
    assert_eq!(
        outcome.actions()[0].kind().as_str(),
        "action.feedback.submit_name"
    );
}

#[test]
fn arrow_navigation_moves_from_text_field_to_right_action_button() {
    let scene = scene_with_text_input_and_action_button();
    let frame = prepare(&scene).unwrap();
    let mut input = InputController::default();
    input.activate_text_control(&scene.text_inputs[0]).unwrap();
    input.ensure_choice_focus(&frame);

    input.keyboard(&frame, "ArrowRight", KeyPhase::Down);
    let outcome = input.keyboard(&frame, "Enter", KeyPhase::Down);

    assert!(outcome.text_control_write_backs().is_empty());
    assert_eq!(
        outcome.actions()[0].kind().as_str(),
        "action.feedback.submit_name"
    );
}
