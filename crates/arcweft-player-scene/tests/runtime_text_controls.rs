use arcweft_bundle::resource_codec::ui::{
    CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization, UiInputKind,
    UiInputPurpose, UiRuntimeControlStyle, UiRuntimeTextControl, UiRuntimeTextControlBounds,
    UiRuntimeTextControlHandlers, UiRuntimeTextControlOptions, UiRuntimeTextSelection,
    UiSecureInputPolicy, UiTextSelectionPolicy, UiTextShortcutPolicy, UiTextTabPolicy,
    UiTextVerticalNavigationPolicy,
};
use arcweft_player_scene::input::InputController;
use arcweft_player_scene::text_controls::RuntimeTextControlLowerer;
use arcweft_presentation::input::{InteractionTarget, PointerId};
use arcweft_presentation::text_input::{
    PlatformTextSelection, TextByteOffset, TextInput, TextInputOperation, TextInputSerial,
    TextInputSessionId, TextRange, TextSelectionAffinity, TextSelectionPolicy, TextShortcutPolicy,
    TextTabPolicy, TextVerticalNavigationPolicy,
};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, RenderPreferences, RenderScene, RenderTextInputControl,
    RenderViewport, SharedFramePlanner,
};

#[test]
fn runtime_text_control_lowers_into_render_scene_and_focused_target() {
    let runtime = runtime_control("input.name", UiInputKind::TextField, "Ada");
    let mut input = InputController::default();
    let controls = RuntimeTextControlLowerer::lower_for_frame(&mut input, &[runtime])
        .expect("runtime controls lower");
    let target = controls[0].target.clone();
    let scene = scene_with_text_inputs(controls, Some(target));
    let prepared = SharedFramePlanner::prepare(&scene).expect("frame prepares");

    assert_eq!(scene.text_inputs.len(), 1);
    assert!(prepared.focused_text_input_target().is_some());
}

#[test]
fn committed_text_updates_player_owned_state_and_next_frame_value() {
    let runtime = runtime_control("input.name", UiInputKind::TextField, "Ada");
    let mut input = InputController::default();
    let controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("initial controls lower");
    let target = controls[0].target.clone();
    let frame = SharedFramePlanner::prepare(&scene_with_text_inputs(
        controls.clone(),
        Some(target.clone()),
    ))
    .expect("focused frame prepares");
    input
        .activate_text_control(&controls[0])
        .expect("text editor activates");

    let outcome = input
        .text_input(
            &frame,
            TextInput::committed(
                TextInputSessionId(runtime.session),
                TextInputSerial(1),
                " Lovelace",
            ),
        )
        .expect("commit applies");

    assert!(outcome.actions.is_empty());
    let next_controls = RuntimeTextControlLowerer::lower_for_frame(&mut input, &[runtime])
        .expect("live controls lower");
    assert_eq!(next_controls[0].value, "Ada Lovelace");
    let next = SharedFramePlanner::prepare(&scene_with_text_inputs(next_controls, Some(target)))
        .expect("next frame prepares");
    assert_eq!(
        next.focused_text_input_target()
            .expect("focused target remains")
            .snapshot
            .surrounding_text(),
        "Ada Lovelace"
    );
}

#[test]
fn selection_update_changes_next_prepared_caret_geometry() {
    let runtime = runtime_control("input.name", UiInputKind::TextField, "Ada");
    let mut input = InputController::default();
    let controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("initial controls lower");
    let target = controls[0].target.clone();
    let initial = SharedFramePlanner::prepare(&scene_with_text_inputs(
        controls.clone(),
        Some(target.clone()),
    ))
    .expect("initial frame prepares");
    input
        .activate_text_control(&controls[0])
        .expect("text editor activates");
    let initial_caret = initial
        .focused_text_input_target()
        .expect("initial focused target")
        .geometry
        .viewport_caret_rect();

    input
        .text_input(
            &initial,
            TextInput::single(
                TextInputSessionId(runtime.session),
                TextInputSerial(2),
                TextInputOperation::SetSelection(PlatformTextSelection::new(
                    TextRange::new(TextByteOffset(1), TextByteOffset(1)),
                    TextSelectionAffinity::Downstream,
                )),
            ),
        )
        .expect("selection applies");

    let next_controls = RuntimeTextControlLowerer::lower_for_frame(&mut input, &[runtime])
        .expect("live controls lower");
    let next = SharedFramePlanner::prepare(&scene_with_text_inputs(next_controls, Some(target)))
        .expect("next frame prepares");
    let focused = next
        .focused_text_input_target()
        .expect("focused target remains");

    assert_eq!(
        focused.snapshot.selection(),
        TextRange::new(TextByteOffset(1), TextByteOffset(1))
    );
    assert!(focused.geometry.viewport_caret_rect().x < initial_caret.x);
}

#[test]
fn secure_runtime_text_control_redacts_snapshot_and_visual_secret() {
    let runtime = runtime_control("input.password", UiInputKind::SecureField, "secret");
    let mut input = InputController::default();
    let controls = RuntimeTextControlLowerer::lower_for_frame(&mut input, &[runtime])
        .expect("secure control lowers");
    let target = controls[0].target.clone();
    let frame = SharedFramePlanner::prepare(&scene_with_text_inputs(controls, Some(target)))
        .expect("secure frame prepares");
    let focused = frame
        .focused_text_input_target()
        .expect("secure field is focused");

    assert_eq!(focused.snapshot.surrounding_text(), "");
    assert!(focused.snapshot.character_bounds().is_empty());
    assert!(focused.geometry.viewport_character_bounds().is_empty());
    assert!(frame.text.iter().all(|block| block.text != "secret"));
    assert!(frame.text.iter().any(|block| block.text == "******"));
}

#[test]
fn pointer_focus_uses_lower_for_frame_activation_path() {
    let runtime = runtime_control("input.name", UiInputKind::TextField, "Ada");
    let mut input = InputController::default();
    let controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("initial controls lower");
    let frame = SharedFramePlanner::prepare(&scene_with_text_inputs(controls, None))
        .expect("hit frame prepares");

    input.pointer_down(&frame, PointerId(0), viewport_point(60.0, 60.0));
    input.pointer_up(&frame, PointerId(0), viewport_point(60.0, 60.0));

    let controls = RuntimeTextControlLowerer::lower_for_frame(&mut input, &[runtime])
        .expect("focused controls lower through shared path");
    assert_eq!(
        input.visual_state().focused.as_ref(),
        Some(&controls[0].target)
    );
}

#[test]
fn pointer_click_places_caret_after_focused_geometry_is_available() {
    let runtime = runtime_control("input.name", UiInputKind::TextField, "Ada");
    let mut input = InputController::default();
    let controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("initial controls lower");
    let initial = SharedFramePlanner::prepare(&scene_with_text_inputs(controls, None))
        .expect("hit frame prepares");

    input.pointer_down(&initial, PointerId(0), viewport_point(58.0, 60.0));

    let focused_controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("focused controls lower");
    let focused = SharedFramePlanner::prepare(&scene_with_text_inputs(
        focused_controls,
        input.visual_state().focused.clone(),
    ))
    .expect("focused frame prepares");

    assert!(
        input
            .apply_pending_text_pointer_selection(&focused)
            .expect("pending pointer selection applies")
    );
    assert_eq!(
        input.focused_text_editor().unwrap().selection(),
        TextRange::new(TextByteOffset(0), TextByteOffset(0))
    );
}

#[test]
fn pointer_drag_extends_focused_text_selection() {
    let runtime = runtime_control("input.name", UiInputKind::TextField, "Ada");
    let mut input = InputController::default();
    let controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("initial controls lower");
    let initial = SharedFramePlanner::prepare(&scene_with_text_inputs(controls, None))
        .expect("hit frame prepares");

    input.pointer_down(&initial, PointerId(0), viewport_point(58.0, 60.0));
    let focused_controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("focused controls lower");
    let focused = SharedFramePlanner::prepare(&scene_with_text_inputs(
        focused_controls,
        input.visual_state().focused.clone(),
    ))
    .expect("focused frame prepares");
    input
        .apply_pending_text_pointer_selection(&focused)
        .expect("pending pointer selection applies");

    input.pointer_move(&focused, PointerId(0), viewport_point(92.0, 60.0));

    let selection = input.focused_text_editor().unwrap().selection();
    assert_eq!(*selection.start(), TextByteOffset(0));
    assert!(selection.end().0 > 0);
}

#[test]
fn runtime_text_control_lowers_selection_shortcut_and_tab_policies() {
    let mut runtime = runtime_control("input.notes", UiInputKind::TextArea, "notes");
    runtime.options.selection_policy = UiTextSelectionPolicy::Disabled;
    runtime.options.shortcut_policy = UiTextShortcutPolicy::Disabled;
    runtime.options.tab_policy = UiTextTabPolicy::InsertTab;
    runtime.options.vertical_navigation_policy = UiTextVerticalNavigationPolicy::VisualLine;
    let mut input = InputController::default();

    let controls = RuntimeTextControlLowerer::lower_for_frame(&mut input, &[runtime])
        .expect("runtime controls lower");

    assert_eq!(
        controls[0].options.selection_policy(),
        TextSelectionPolicy::Disabled
    );
    assert_eq!(
        controls[0].options.shortcut_policy(),
        TextShortcutPolicy::Disabled
    );
    assert_eq!(controls[0].options.tab_policy(), TextTabPolicy::InsertTab);
    assert_eq!(
        controls[0].options.vertical_navigation_policy(),
        TextVerticalNavigationPolicy::VisualLine
    );
}

fn runtime_control(public_id: &str, kind: UiInputKind, value: &str) -> UiRuntimeTextControl {
    let end = u32::try_from(value.len()).expect("test text length fits in u32");
    UiRuntimeTextControl {
        public_id: public_id.to_owned(),
        target: public_id.to_owned(),
        session: stable_test_session(public_id),
        value: value.to_owned(),
        selection: UiRuntimeTextSelection::new(end, end),
        options: UiRuntimeTextControlOptions {
            purpose: UiInputPurpose::Text,
            autocorrect: TextAssistPolicy::PlatformDefault,
            spellcheck: TextAssistPolicy::PlatformDefault,
            capitalization: TextCapitalization::None,
            enter_key: EnterKeyHint::Default,
            multiline: kind.is_multiline(),
            selection_policy: UiTextSelectionPolicy::Enabled,
            shortcut_policy: UiTextShortcutPolicy::Enabled,
            tab_policy: UiTextTabPolicy::FocusNavigation,
            vertical_navigation_policy: UiTextVerticalNavigationPolicy::LogicalLine,
            secure_policy: if kind.is_secure() {
                UiSecureInputPolicy::Password
            } else {
                UiSecureInputPolicy::Plain
            },
            composition_on_blur: CompositionOnBlurPolicy::Commit,
        },
        kind,
        bounds: UiRuntimeTextControlBounds::from_px(48, 48, 260, 48),
        label: Some("Name".to_owned()),
        handlers: UiRuntimeTextControlHandlers::default(),
        style: UiRuntimeControlStyle::default(),
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
    }
}

fn viewport_point(x: f32, y: f32) -> arcweft_presentation::input::ViewportPoint {
    arcweft_presentation::input::ViewportPoint::new(x, y)
}

fn stable_test_session(public_id: &str) -> u64 {
    public_id.as_bytes().iter().fold(7_u64, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(u64::from(*byte))
    })
}
