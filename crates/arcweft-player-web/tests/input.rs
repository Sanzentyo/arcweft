use arcweft_bundle::resource_codec::view::{
    CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization, ViewInputKind,
    ViewInputPurpose, ViewRuntimeControlStyle, ViewRuntimeTextControl,
    ViewRuntimeTextControlBounds, ViewRuntimeTextControlHandlers, ViewRuntimeTextControlOptions,
    ViewRuntimeTextSelection, ViewSecureInputPolicy, ViewTextSelectionPolicy,
    ViewTextShortcutPolicy, ViewTextTabPolicy, ViewTextVerticalNavigationPolicy,
};
use arcweft_id::PublicId;
use arcweft_player_scene::{
    input::{InputController, InputPointerModifiers},
    text_controls::RuntimeTextControlLowerer,
};
use arcweft_presentation::{
    hit::HitRect,
    input::{InteractionTarget, KeyPhase, PointerId, ViewportPoint},
    text_input::{TextInput, TextInputSerial, TextInputSessionId},
};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, RenderActionButton, RenderActionButtonAction,
    RenderChoiceItem, RenderControlStyle, RenderFocusAutoScrollPolicy, RenderPreferences,
    RenderScene, RenderScrollAxis, RenderScrollOverflow, RenderScrollRegion,
    RenderTextInputControl, RenderViewport, SharedFramePlanner,
};

fn frame() -> arcweft_render_wgpu::geometry::PreparedFrame {
    SharedFramePlanner::prepare(&RenderScene {
        dialogue: None,
        choices: vec![
            RenderChoiceItem {
                id: "choice.first".to_owned(),
                label: "First".to_owned(),
            },
            RenderChoiceItem {
                id: "choice.second".to_owned(),
                label: "Second".to_owned(),
            },
        ],
        text_inputs: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: Vec::new(),
        viewport: RenderViewport {
            logical_width: 1280.0,
            logical_height: 720.0,
            physical_width: 1280,
            physical_height: 720,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
    })
    .expect("frame plans")
}

fn frame_with_containing_scroll_region() -> arcweft_render_wgpu::geometry::PreparedFrame {
    SharedFramePlanner::prepare(&RenderScene {
        dialogue: None,
        choices: Vec::new(),
        text_inputs: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: Vec::new(),
        viewport: RenderViewport {
            logical_width: 1280.0,
            logical_height: 720.0,
            physical_width: 1280,
            physical_height: 720,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: vec![RenderScrollRegion {
            id: "scroll.main".to_owned(),
            bounds: arcweft_presentation::hit::HitRect::new(10.0, 20.0, 300.0, 120.0),
            content_width: 300.0,
            content_height: 420.0,
            offset_x: 0.0,
            offset_y: 0.0,
            axis: RenderScrollAxis::Vertical,
            overflow: RenderScrollOverflow::Auto,
            auto_scroll_focus: RenderFocusAutoScrollPolicy::Nearest,
        }],
    })
    .expect("scroll frame plans")
}

fn frame_with_horizontal_scroll_region() -> arcweft_render_wgpu::geometry::PreparedFrame {
    SharedFramePlanner::prepare(&RenderScene {
        dialogue: None,
        choices: Vec::new(),
        text_inputs: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: Vec::new(),
        viewport: RenderViewport {
            logical_width: 1280.0,
            logical_height: 720.0,
            physical_width: 1280,
            physical_height: 720,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: vec![RenderScrollRegion {
            id: "scroll.gallery".to_owned(),
            bounds: arcweft_presentation::hit::HitRect::new(10.0, 20.0, 300.0, 120.0),
            content_width: 720.0,
            content_height: 120.0,
            offset_x: 0.0,
            offset_y: 0.0,
            axis: RenderScrollAxis::Horizontal,
            overflow: RenderScrollOverflow::Auto,
            auto_scroll_focus: RenderFocusAutoScrollPolicy::Nearest,
        }],
    })
    .expect("horizontal scroll frame plans")
}

fn frame_with_action_button(
    buttons: Vec<RenderActionButton>,
    focused: Option<InteractionTarget>,
) -> arcweft_render_wgpu::geometry::PreparedFrame {
    SharedFramePlanner::prepare(&RenderScene {
        dialogue: None,
        choices: Vec::new(),
        text_inputs: Vec::new(),
        action_buttons: buttons,
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: Vec::new(),
        viewport: RenderViewport {
            logical_width: 800.0,
            logical_height: 450.0,
            physical_width: 800,
            physical_height: 450,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState {
            focused,
            hovered: None,
            pressed: None,
        },
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
    })
    .expect("button frame plans")
}

#[test]
fn pointer_activation_emits_a_typed_semantic_choice_action() {
    let frame = frame();
    let target = frame.first_choice_target().expect("first target");
    let bounds = frame
        .hits
        .find_target(&target)
        .expect("hit bounds")
        .bounds();
    let point = ViewportPoint::new(
        bounds.x + bounds.width * 0.5,
        bounds.y + bounds.height * 0.5,
    );
    let mut input = InputController::default();
    input.pointer_down(&frame, PointerId(0), point, InputPointerModifiers::NONE);
    let outcome = input.pointer_up(&frame, PointerId(0), point, InputPointerModifiers::NONE);

    assert_eq!(outcome.actions.len(), 1);
    assert_eq!(outcome.actions[0].kind().as_str(), "action.choice.select");
    assert_eq!(
        outcome.actions[0].payload().map(String::as_str),
        Some("choice.first")
    );
}

#[test]
fn drag_beyond_the_activation_threshold_does_not_select() {
    let frame = frame();
    let target = frame.first_choice_target().expect("first target");
    let bounds = frame
        .hits
        .find_target(&target)
        .expect("hit bounds")
        .bounds();
    let start = ViewportPoint::new(bounds.x + 10.0, bounds.y + 10.0);
    let moved = ViewportPoint::new(bounds.x + 30.0, bounds.y + 10.0);
    let mut input = InputController::default();
    input.pointer_down(&frame, PointerId(0), start, InputPointerModifiers::NONE);
    input.pointer_move(&frame, PointerId(0), moved);
    let outcome = input.pointer_up(&frame, PointerId(0), moved, InputPointerModifiers::NONE);

    assert!(outcome.actions.is_empty());
}

#[test]
fn keyboard_focus_navigation_activates_the_focused_choice() {
    let frame = frame();
    let mut input = InputController::default();
    input.ensure_choice_focus(&frame);
    input.keyboard(&frame, "ArrowDown", KeyPhase::Down);
    let outcome = input.keyboard(&frame, "Enter", KeyPhase::Down);

    assert_eq!(outcome.actions.len(), 1);
    assert_eq!(
        outcome.actions[0].payload().map(String::as_str),
        Some("choice.second")
    );
}

#[test]
fn wheel_input_without_scroll_region_is_noop_for_choice_scroll_state() {
    let frame = frame();
    let mut input = InputController::default();
    let before = input.choice_scroll();
    let outcome = input.wheel(&frame, 180.0);

    assert!(!outcome.redraw);
    assert_eq!(input.choice_scroll(), before);
}

#[test]
fn wheel_input_updates_scroll_region_under_pointer() {
    let frame = frame_with_containing_scroll_region();
    let mut input = InputController::default();

    input.pointer_move(&frame, PointerId(0), ViewportPoint::new(20.0, 30.0));
    input.wheel(&frame, -180.0);
    assert!((input.scroll_offset_y("scroll.main") - 180.0).abs() < f32::EPSILON);

    input.wheel(&frame, -400.0);
    assert!((input.scroll_offset_y("scroll.main") - 300.0).abs() < f32::EPSILON);

    input.wheel(&frame, 500.0);
    assert!(input.scroll_offset_y("scroll.main").abs() < f32::EPSILON);
}

#[test]
fn wheel_input_updates_horizontal_scroll_region_under_pointer() {
    let frame = frame_with_horizontal_scroll_region();
    let mut input = InputController::default();

    input.pointer_move(&frame, PointerId(0), ViewportPoint::new(20.0, 30.0));
    input.wheel(&frame, -180.0);
    assert!((input.scroll_offset_x("scroll.gallery") - 180.0).abs() < f32::EPSILON);
    assert!(input.scroll_offset_y("scroll.gallery").abs() < f32::EPSILON);
}

#[test]
fn web_hidden_runtime_text_control_rejects_stale_writeback() {
    let runtime = runtime_control("input.name", ViewInputKind::TextField, "Ada");
    let mut input = InputController::default();
    let controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("initial controls lower");
    let target = controls[0].target.clone();
    let _focused_frame =
        SharedFramePlanner::prepare(&scene_with_text_inputs(controls.clone(), Some(target)))
            .expect("focused frame prepares");
    input
        .activate_text_control(&controls[0])
        .expect("text editor activates");

    let hidden_controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, &[]).expect("hidden controls lower");
    let hidden_frame = SharedFramePlanner::prepare(&scene_with_text_inputs(hidden_controls, None))
        .expect("hidden frame prepares");

    assert!(input.focused_text_editor().is_none());
    assert!(input.visual_state().focused.is_none());

    let outcome = input
        .text_input(
            &hidden_frame,
            TextInput::committed(
                TextInputSessionId(runtime.session),
                TextInputSerial(9),
                " Lovelace",
            ),
        )
        .expect("stale text input is ignored");

    assert!(outcome.text_control_write_backs().is_empty());
    assert!(input.focused_text_editor().is_none());
}

#[test]
fn web_hidden_view_action_button_rejects_stale_hit_and_focus() {
    let button = render_action_button("button.submit", "action.feedback.submit");
    let target = button.target.clone();
    let live_frame = frame_with_action_button(vec![button], Some(target.clone()));
    let live_bounds = live_frame
        .hits
        .find_target(&target)
        .expect("live button is hittable")
        .bounds();
    let position = ViewportPoint::new(
        live_bounds.x + live_bounds.width * 0.5,
        live_bounds.y + live_bounds.height * 0.5,
    );

    let mut input = InputController::default();
    input.pointer_down(
        &live_frame,
        PointerId(0),
        position,
        InputPointerModifiers::NONE,
    );
    assert_eq!(input.visual_state().pressed, Some(target.clone()));

    let hidden_frame = frame_with_action_button(Vec::new(), None);
    assert!(hidden_frame.hits.find_target(&target).is_none());
    assert!(hidden_frame.action_button_for_target(&target).is_none());
    assert!(!hidden_frame.keyboard_focus_targets().contains(&target));

    let outcome = input.pointer_up(
        &hidden_frame,
        PointerId(0),
        position,
        InputPointerModifiers::NONE,
    );
    assert!(outcome.actions().is_empty());
    assert!(outcome.text_control_write_backs().is_empty());
    assert!(input.visual_state().pressed.is_none());
}

#[test]
fn web_hidden_view_text_control_rejects_stale_hit_and_focus() {
    let runtime = runtime_control("input.name", ViewInputKind::TextField, "Ada");
    let mut input = InputController::default();
    let controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("initial controls lower");
    let target = controls[0].target.clone();
    let live_frame =
        SharedFramePlanner::prepare(&scene_with_text_inputs(controls, Some(target.clone())))
            .expect("focused frame prepares");

    assert!(live_frame.hits.find_target(&target).is_some());
    assert!(live_frame.focused_text_input_target().is_some());
    assert!(live_frame.keyboard_focus_targets().contains(&target));

    let hidden_controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, &[]).expect("hidden controls lower");
    let hidden_frame = SharedFramePlanner::prepare(&scene_with_text_inputs(hidden_controls, None))
        .expect("hidden frame prepares");

    assert!(hidden_frame.hits.find_target(&target).is_none());
    assert!(hidden_frame.focused_text_input_target().is_none());
    assert!(!hidden_frame.keyboard_focus_targets().contains(&target));
}

fn render_action_button(target: &str, action: &str) -> RenderActionButton {
    RenderActionButton {
        target: InteractionTarget::new(PublicId::try_new(target).expect("valid target id")),
        label: "Send".to_owned(),
        enabled: true,
        containing_scroll_region: None,
        bounds: HitRect::new(48.0, 48.0, 180.0, 48.0),
        viewport_clip: None,
        style: RenderControlStyle::default(),
        action: RenderActionButtonAction::ActionInvoke {
            action: PublicId::try_new(action).expect("valid action id"),
            payload: Some("ready".to_owned()),
        },
    }
}

fn runtime_control(public_id: &str, kind: ViewInputKind, value: &str) -> ViewRuntimeTextControl {
    let end = u32::try_from(value.len()).expect("test text length fits in u32");
    ViewRuntimeTextControl {
        public_id: public_id.to_owned(),
        target: public_id.to_owned(),
        view: Some("view.WebPanel".to_owned()),
        containing_scroll_region: None,
        session: stable_test_session(public_id),
        value: value.to_owned(),
        selection: ViewRuntimeTextSelection::new(end, end),
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
        bounds: ViewRuntimeTextControlBounds::from_px(48, 48, 260, 48),
        label: Some("Name".to_owned()),
        handlers: ViewRuntimeTextControlHandlers::default(),
        style: ViewRuntimeControlStyle::default(),
    }
}

fn scene_with_text_inputs(
    text_inputs: Vec<RenderTextInputControl>,
    focused: Option<InteractionTarget>,
) -> RenderScene {
    RenderScene {
        dialogue: None,
        choices: Vec::new(),
        text_inputs,
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: Vec::new(),
        viewport: RenderViewport {
            logical_width: 800.0,
            logical_height: 450.0,
            physical_width: 800,
            physical_height: 450,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState {
            focused,
            hovered: None,
            pressed: None,
        },
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
    }
}

fn stable_test_session(public_id: &str) -> u64 {
    public_id.as_bytes().iter().fold(7_u64, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(u64::from(*byte))
    })
}
