use arcweft_bundle::resource_codec::view::{
    CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization, ViewInputKind,
    ViewInputPurpose, ViewRuntimeControlStyle, ViewRuntimeTextControl,
    ViewRuntimeTextControlBounds, ViewRuntimeTextControlHandlers, ViewRuntimeTextControlOptions,
    ViewRuntimeTextSelection, ViewSecureInputPolicy, ViewTextSelectionPolicy,
    ViewTextShortcutPolicy, ViewTextTabPolicy, ViewTextVerticalNavigationPolicy,
};
use arcweft_player_scene::input::{InputController, InputPointerModifiers};
use arcweft_player_scene::text_controls::RuntimeTextControlLowerer;
use arcweft_presentation::input::{InteractionTarget, PointerId};
use arcweft_presentation::text_input::{
    PlatformTextSelection, TextByteOffset, TextInput, TextInputOperation, TextInputSerial,
    TextInputSessionId, TextRange, TextSelectionAffinity, TextSelectionPolicy, TextShortcutPolicy,
    TextTabPolicy, TextVerticalNavigationPolicy,
};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, RenderPreferences, RenderScene, RenderScrollAxis,
    RenderScrollOverflow, RenderScrollRegion, RenderTextInputControl, RenderViewport,
    SharedFramePlanner,
};

#[test]
fn runtime_text_control_lowers_into_render_scene_and_focused_target() {
    let runtime = runtime_control("input.name", ViewInputKind::TextField, "Ada");
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
    let runtime = runtime_control("input.name", ViewInputKind::TextField, "Ada");
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
    let runtime = runtime_control("input.name", ViewInputKind::TextField, "Ada");
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
    let runtime = runtime_control("input.password", ViewInputKind::SecureField, "secret");
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
    let runtime = runtime_control("input.name", ViewInputKind::TextField, "Ada");
    let mut input = InputController::default();
    let controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("initial controls lower");
    let frame = SharedFramePlanner::prepare(&scene_with_text_inputs(controls, None))
        .expect("hit frame prepares");

    input.pointer_down(
        &frame,
        PointerId(0),
        viewport_point(60.0, 60.0),
        InputPointerModifiers::NONE,
    );
    input.pointer_up(
        &frame,
        PointerId(0),
        viewport_point(60.0, 60.0),
        InputPointerModifiers::NONE,
    );

    let controls = RuntimeTextControlLowerer::lower_for_frame(&mut input, &[runtime])
        .expect("focused controls lower through shared path");
    assert_eq!(
        input.visual_state().focused.as_ref(),
        Some(&controls[0].target)
    );
}

#[test]
fn pointer_click_places_caret_after_focused_geometry_is_available() {
    let runtime = runtime_control("input.name", ViewInputKind::TextField, "Ada");
    let mut input = InputController::default();
    let controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("initial controls lower");
    let initial = SharedFramePlanner::prepare(&scene_with_text_inputs(controls, None))
        .expect("hit frame prepares");

    input.pointer_down(
        &initial,
        PointerId(0),
        viewport_point(58.0, 60.0),
        InputPointerModifiers::NONE,
    );

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
fn shift_pointer_click_extends_focused_text_selection() {
    let mut runtime = runtime_control("input.name", ViewInputKind::TextField, "Ada");
    runtime.selection = ViewRuntimeTextSelection::new(1, 1);
    let mut input = InputController::default();
    let controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("focused controls lower");
    let target = controls[0].target.clone();
    input
        .activate_text_control(&controls[0])
        .expect("text editor activates");
    let focused = SharedFramePlanner::prepare(&scene_with_text_inputs(controls, Some(target)))
        .expect("focused frame prepares");
    let shift = InputPointerModifiers::new(true);

    input.pointer_down(&focused, PointerId(0), viewport_point(150.0, 60.0), shift);
    input.pointer_up(&focused, PointerId(0), viewport_point(150.0, 60.0), shift);

    let selection = input.focused_text_editor().unwrap().selection();
    assert_eq!(*selection.start(), TextByteOffset(1));
    assert!(selection.end().0 > 1);
}

#[test]
fn pointer_drag_extends_focused_text_selection() {
    let runtime = runtime_control("input.name", ViewInputKind::TextField, "Ada");
    let mut input = InputController::default();
    let controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("initial controls lower");
    let initial = SharedFramePlanner::prepare(&scene_with_text_inputs(controls, None))
        .expect("hit frame prepares");

    input.pointer_down(
        &initial,
        PointerId(0),
        viewport_point(58.0, 60.0),
        InputPointerModifiers::NONE,
    );
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
fn repeated_pointer_clicks_select_word_then_line() {
    let runtime = runtime_control("input.name", ViewInputKind::TextField, "alpha beta");
    let mut input = InputController::default();
    let controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("controls lower");
    let target = controls[0].target.clone();
    input
        .activate_text_control(&controls[0])
        .expect("text editor activates");
    let frame = SharedFramePlanner::prepare(&scene_with_text_inputs(controls, Some(target)))
        .expect("focused frame prepares");
    let position = viewport_point(70.0, 60.0);

    for _ in 0..2 {
        input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
        input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    }
    let word_selection = input.focused_text_editor().unwrap().selection();
    assert_eq!(
        word_selection,
        TextRange::new(TextByteOffset(0), TextByteOffset(5))
    );

    input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    let line_selection = input.focused_text_editor().unwrap().selection();
    assert_eq!(
        line_selection,
        TextRange::new(TextByteOffset(0), TextByteOffset(10))
    );
}

#[test]
fn selected_text_drag_moves_text_and_emits_writeback() {
    let mut runtime = runtime_control("input.name", ViewInputKind::TextField, "alpha beta");
    runtime.selection = ViewRuntimeTextSelection::new(0, 5);
    let mut input = InputController::default();
    let controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("controls lower");
    let target = controls[0].target.clone();
    input
        .activate_text_control(&controls[0])
        .expect("text editor activates");
    let frame = SharedFramePlanner::prepare(&scene_with_text_inputs(controls, Some(target)))
        .expect("focused frame prepares");

    input.pointer_down(
        &frame,
        PointerId(0),
        viewport_point(70.0, 60.0),
        InputPointerModifiers::NONE,
    );
    input.pointer_move(&frame, PointerId(0), viewport_point(190.0, 60.0));
    let outcome = input.pointer_up(
        &frame,
        PointerId(0),
        viewport_point(190.0, 60.0),
        InputPointerModifiers::NONE,
    );

    assert_eq!(input.focused_text_editor().unwrap().text(), " betaalpha");
    assert_eq!(outcome.text_control_write_backs().len(), 1);
    assert_eq!(
        outcome.text_control_write_backs()[0].value().as_str(),
        " betaalpha"
    );
}

#[test]
fn pointer_drag_selection_autoscrolls_containing_scroll_region() {
    let runtime = runtime_control("input.notes", ViewInputKind::TextArea, "alpha\nbeta\ngamma");
    let mut input = InputController::default();
    let mut controls =
        RuntimeTextControlLowerer::lower_for_frame(&mut input, std::slice::from_ref(&runtime))
            .expect("controls lower");
    controls[0].containing_scroll_region = Some("scroll.notes".to_owned());
    let target = controls[0].target.clone();
    input
        .activate_text_control(&controls[0])
        .expect("text editor activates");
    let mut scene = scene_with_text_inputs(controls, Some(target));
    scene.scroll_regions.push(RenderScrollRegion {
        id: "scroll.notes".to_owned(),
        bounds: arcweft_presentation::hit::HitRect::new(40.0, 40.0, 300.0, 70.0),
        content_width: 300.0,
        content_height: 220.0,
        offset_x: 0.0,
        offset_y: 0.0,
        axis: RenderScrollAxis::Vertical,
        overflow: RenderScrollOverflow::Auto,
    });
    let frame = SharedFramePlanner::prepare(&scene).expect("focused frame prepares");

    input.pointer_down(
        &frame,
        PointerId(0),
        viewport_point(70.0, 60.0),
        InputPointerModifiers::NONE,
    );
    input.pointer_move(&frame, PointerId(0), viewport_point(70.0, 106.0));

    assert!(input.scroll_offset_y("scroll.notes") > 0.0);
}

#[test]
fn runtime_text_control_lowers_selection_shortcut_and_tab_policies() {
    let mut runtime = runtime_control("input.notes", ViewInputKind::TextArea, "notes");
    runtime.options.selection_policy = ViewTextSelectionPolicy::Disabled;
    runtime.options.shortcut_policy = ViewTextShortcutPolicy::Disabled;
    runtime.options.tab_policy = ViewTextTabPolicy::InsertTab;
    runtime.options.vertical_navigation_policy = ViewTextVerticalNavigationPolicy::VisualLine;
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

#[test]
fn hidden_runtime_text_control_clears_focus_and_rejects_stale_writeback() {
    let runtime = runtime_control("input.name", ViewInputKind::TextField, "Ada");
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
    assert!(frame.focused_text_input_target().is_some());
}

fn runtime_control(public_id: &str, kind: ViewInputKind, value: &str) -> ViewRuntimeTextControl {
    let end = u32::try_from(value.len()).expect("test text length fits in u32");
    ViewRuntimeTextControl {
        public_id: public_id.to_owned(),
        target: public_id.to_owned(),
        view: None,
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
            secure_policy: if kind.is_secure() {
                ViewSecureInputPolicy::Password
            } else {
                ViewSecureInputPolicy::Plain
            },
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

fn viewport_point(x: f32, y: f32) -> arcweft_presentation::input::ViewportPoint {
    arcweft_presentation::input::ViewportPoint::new(x, y)
}

fn stable_test_session(public_id: &str) -> u64 {
    public_id.as_bytes().iter().fold(7_u64, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(u64::from(*byte))
    })
}
