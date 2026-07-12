use super::*;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::semantic::SemanticRole;
use arcweft_presentation::text_input::{
    TextByteOffset, TextCompositionUpdate, TextControlWriteBackKind, TextEditCommand,
    TextInputOperation, TextInputOptions, TextInputPrivacy, TextInputSerial, TextInputSessionId,
    TextRange,
};
use arcweft_render_wgpu::geometry::{
    PreparedTextBoxState, RenderActionButton, RenderControlStyle, RenderPreferences, RenderScene,
    RenderScrollAxis, RenderScrollIndicatorsPolicy, RenderScrollOverflow,
    RenderScrollOverscrollPolicy, RenderScrollRegion, RenderViewport, SharedFramePlanner,
};

fn target(name: &str) -> arcweft_presentation::input::InteractionTarget {
    arcweft_presentation::input::InteractionTarget::new(
        PublicId::try_new(format!("target.{name}")).unwrap(),
    )
}

fn scene(control: RenderTextInputControl) -> RenderScene {
    RenderScene {
        content_avoidance_regions: Vec::new(),
        choices: Vec::new(),
        text_inputs: vec![control],
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: Vec::new(),
        viewport: RenderViewport {
            logical_width: 640.0,
            logical_height: 360.0,
            physical_width: 640,
            physical_height: 360,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
    }
}

fn prepare_with_textbox(scene: &RenderScene, reveal_complete: bool) -> PreparedFrame {
    let mut frame = SharedFramePlanner::prepare(scene).expect("frame prepares");
    frame.push_textbox(PreparedTextBoxState {
        textbox: 0,
        entry: 0,
        mount: 0,
        revision: 0,
        instance: 0,
        stage: 0,
        bounds: HitRect::new(32.0, 180.0, 576.0, 148.0),
        reveal_complete,
        advance_available: true,
    });
    frame
}

fn scroll_frame() -> PreparedFrame {
    SharedFramePlanner::prepare(&RenderScene {
        content_avoidance_regions: Vec::new(),
        choices: Vec::new(),
        text_inputs: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: Vec::new(),
        viewport: RenderViewport {
            logical_width: 640.0,
            logical_height: 360.0,
            physical_width: 640,
            physical_height: 360,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: vec![RenderScrollRegion {
            id: "scroll.editor".to_owned(),
            bounds: HitRect::new(20.0, 30.0, 220.0, 80.0),
            content_width: 220.0,
            content_height: 260.0,
            offset_x: 0.0,
            offset_y: 0.0,
            overscroll_x: 0.0,
            overscroll_y: 0.0,
            axis: RenderScrollAxis::Vertical,
            overflow: RenderScrollOverflow::Auto,
            indicators: RenderScrollIndicatorsPolicy::Auto,
            overscroll: RenderScrollOverscrollPolicy::Clamp,
            auto_scroll_focus: RenderFocusAutoScrollPolicy::Nearest,
            indicator_activity_millis: None,
        }],
    })
    .expect("scroll frame prepares")
}

fn horizontal_scroll_frame() -> PreparedFrame {
    SharedFramePlanner::prepare(&RenderScene {
        content_avoidance_regions: Vec::new(),
        choices: Vec::new(),
        text_inputs: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: Vec::new(),
        viewport: RenderViewport {
            logical_width: 640.0,
            logical_height: 360.0,
            physical_width: 640,
            physical_height: 360,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: vec![RenderScrollRegion {
            id: "scroll.gallery".to_owned(),
            bounds: HitRect::new(20.0, 30.0, 100.0, 80.0),
            content_width: 260.0,
            content_height: 80.0,
            offset_x: 0.0,
            offset_y: 0.0,
            overscroll_x: 0.0,
            overscroll_y: 0.0,
            axis: RenderScrollAxis::Horizontal,
            overflow: RenderScrollOverflow::Auto,
            indicators: RenderScrollIndicatorsPolicy::Auto,
            overscroll: RenderScrollOverscrollPolicy::Clamp,
            auto_scroll_focus: RenderFocusAutoScrollPolicy::Nearest,
            indicator_activity_millis: None,
        }],
    })
    .expect("horizontal scroll frame prepares")
}

fn nested_scroll_frame(
    child_overscroll: RenderScrollOverscrollPolicy,
    reduce_motion: bool,
    visual_time_millis: u64,
) -> PreparedFrame {
    SharedFramePlanner::prepare(&RenderScene {
        content_avoidance_regions: Vec::new(),
        choices: Vec::new(),
        text_inputs: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: Vec::new(),
        viewport: RenderViewport {
            logical_width: 640.0,
            logical_height: 360.0,
            physical_width: 640,
            physical_height: 360,
            scale_factor: 1.0,
        },
        visual_time_millis,
        preferences: RenderPreferences {
            reduce_motion,
            ..RenderPreferences::default()
        },
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: vec![
            RenderScrollRegion {
                id: "scroll.child".to_owned(),
                bounds: HitRect::new(30.0, 30.0, 180.0, 60.0),
                content_width: 180.0,
                content_height: 180.0,
                offset_x: 0.0,
                offset_y: 0.0,
                overscroll_x: 0.0,
                overscroll_y: 0.0,
                axis: RenderScrollAxis::Vertical,
                overflow: RenderScrollOverflow::Auto,
                indicators: RenderScrollIndicatorsPolicy::Auto,
                overscroll: child_overscroll,
                auto_scroll_focus: RenderFocusAutoScrollPolicy::Nearest,
                indicator_activity_millis: None,
            },
            // Bundle lowering records nested children before their parent, so
            // chaining must derive inner-to-outer order from geometry.
            RenderScrollRegion {
                id: "scroll.parent".to_owned(),
                bounds: HitRect::new(10.0, 10.0, 260.0, 180.0),
                content_width: 260.0,
                content_height: 500.0,
                offset_x: 0.0,
                offset_y: 0.0,
                overscroll_x: 0.0,
                overscroll_y: 0.0,
                axis: RenderScrollAxis::Vertical,
                overflow: RenderScrollOverflow::Auto,
                indicators: RenderScrollIndicatorsPolicy::Auto,
                overscroll: RenderScrollOverscrollPolicy::Clamp,
                auto_scroll_focus: RenderFocusAutoScrollPolicy::Nearest,
                indicator_activity_millis: None,
            },
        ],
    })
    .expect("nested scroll frame prepares")
}

#[test]
fn wheel_updates_scroll_region_under_pointer_and_clamps() {
    let frame = scroll_frame();
    let mut input = InputController::default();

    input.pointer_move(&frame, PointerId(0), ViewportPoint::new(30.0, 40.0));
    input.wheel(&frame, -90.0);
    assert!((input.scroll_offset_y("scroll.editor") - 90.0).abs() < f32::EPSILON);

    input.wheel(&frame, -300.0);
    assert!((input.scroll_offset_y("scroll.editor") - 180.0).abs() < f32::EPSILON);

    input.wheel(&frame, 300.0);
    assert!(input.scroll_offset_y("scroll.editor").abs() < f32::EPSILON);
}

#[test]
fn precision_scroll_uses_x_delta_for_horizontal_region() {
    let frame = horizontal_scroll_frame();
    let mut input = InputController::default();

    input.pointer_move(&frame, PointerId(0), ViewportPoint::new(30.0, 40.0));
    let outcome = input.precision_scroll(&frame, -40.0, -5.0);

    assert!(outcome.redraw);
    assert!((input.scroll_offset_x("scroll.gallery") - 40.0).abs() < f32::EPSILON);
    assert!(input.scroll_offset_y("scroll.gallery").abs() < f32::EPSILON);
}

#[test]
fn right_stick_analog_scroll_uses_the_shared_pointer_region_route() {
    let frame = scroll_frame();
    let mut input = InputController::default();
    input.pointer_move(&frame, PointerId(0), ViewportPoint::new(30.0, 40.0));

    let first = input.controller(
        &frame,
        ControllerInputChange::Axis {
            axis: crate::controller::ControllerAxis::RightY,
            value: 1.0,
            time_millis: 0,
        },
    );
    assert!(!first.redraw);
    let outcome = input.controller(
        &frame,
        ControllerInputChange::Axis {
            axis: crate::controller::ControllerAxis::RightY,
            value: 1.0,
            time_millis: 100,
        },
    );

    assert!(outcome.redraw);
    assert!((input.scroll_offset_y("scroll.editor") - 72.0).abs() < f32::EPSILON);
}

#[test]
fn scroll_region_by_id_scrolls_without_pointer_and_clamps() {
    let frame = horizontal_scroll_frame();
    let mut input = InputController::default();

    let outcome = input.scroll_region_by_id(&frame, "scroll.gallery", -400.0, 0.0);

    assert!(outcome.redraw);
    assert!((input.scroll_offset_x("scroll.gallery") - 160.0).abs() < f32::EPSILON);
}

#[test]
fn missing_scroll_region_by_id_is_noop() {
    let frame = horizontal_scroll_frame();
    let mut input = InputController::default();

    let outcome = input.scroll_region_by_id(&frame, "scroll.missing", -400.0, 0.0);

    assert!(!outcome.redraw);
    assert!(input.snapshot().scroll_offsets.is_empty());
}

#[test]
fn clamp_overscroll_chains_unconsumed_delta_to_parent() {
    let frame = nested_scroll_frame(RenderScrollOverscrollPolicy::Clamp, false, 100);
    let mut input = InputController::default();
    input.pointer_move(&frame, PointerId(0), ViewportPoint::new(40.0, 40.0));

    let outcome = input.wheel(&frame, -200.0);

    assert!(outcome.redraw);
    assert!((input.scroll_offset_y("scroll.child") - 120.0).abs() < f32::EPSILON);
    assert!((input.scroll_offset_y("scroll.parent") - 80.0).abs() < f32::EPSILON);
}

#[test]
fn contain_overscroll_stops_scroll_chaining_without_transient_offset() {
    let frame = nested_scroll_frame(RenderScrollOverscrollPolicy::Contain, false, 100);
    let mut input = InputController::default();
    input.pointer_move(&frame, PointerId(0), ViewportPoint::new(40.0, 40.0));

    input.wheel(&frame, -200.0);

    assert!((input.scroll_offset_y("scroll.child") - 120.0).abs() < f32::EPSILON);
    assert!(input.scroll_offset_y("scroll.parent").abs() < f32::EPSILON);
    let mut child = frame.scroll_regions[0].clone();
    input.resolve_scroll_region(&mut child, 100, false);
    assert!(child.overscroll_y.abs() < f32::EPSILON);
}

#[test]
fn elastic_overscroll_is_transient_and_settles_without_changing_snapshot_offset() {
    let frame = nested_scroll_frame(RenderScrollOverscrollPolicy::Elastic, false, 100);
    let mut input = InputController::default();
    input.pointer_move(&frame, PointerId(0), ViewportPoint::new(40.0, 40.0));

    input.wheel(&frame, -200.0);

    assert!((input.scroll_offset_y("scroll.child") - 120.0).abs() < f32::EPSILON);
    assert!(input.scroll_offset_y("scroll.parent").abs() < f32::EPSILON);
    let snapshot = input.snapshot();
    let mut child = frame.scroll_regions[0].clone();
    input.resolve_scroll_region(&mut child, 100, false);
    assert!(child.overscroll_y > 0.0);
    assert!(child.visual_offset_y() > child.offset_y);

    let initial_displacement = child.overscroll_y;
    input.resolve_scroll_region(&mut child, 350, false);
    assert!(child.overscroll_y < initial_displacement);
    input.resolve_scroll_region(&mut child, 1_100, false);
    assert!(child.overscroll_y.abs() < f32::EPSILON);
    assert_eq!(input.snapshot(), snapshot);
}

#[test]
fn reduce_motion_consumes_elastic_boundary_delta_without_displacement() {
    let frame = nested_scroll_frame(RenderScrollOverscrollPolicy::Elastic, true, 100);
    let mut input = InputController::default();
    input.pointer_move(&frame, PointerId(0), ViewportPoint::new(40.0, 40.0));

    input.wheel(&frame, -200.0);

    let mut child = frame.scroll_regions[0].clone();
    input.resolve_scroll_region(&mut child, 100, true);
    assert!(child.overscroll_y.abs() < f32::EPSILON);
    assert!(input.scroll_offset_y("scroll.parent").abs() < f32::EPSILON);
}

#[test]
fn focus_auto_scroll_policy_offsets_are_clamped() {
    assert!(
        (focus_auto_scroll_offset(
            RenderFocusAutoScrollPolicy::Nearest,
            0.0,
            30.0,
            100.0,
            80.0,
            80.0,
            160.0,
        ) - 30.0)
            .abs()
            < f32::EPSILON
    );
    assert!(
        (focus_auto_scroll_offset(
            RenderFocusAutoScrollPolicy::Start,
            0.0,
            30.0,
            100.0,
            80.0,
            80.0,
            160.0,
        ) - 50.0)
            .abs()
            < f32::EPSILON
    );
    assert!(
        (focus_auto_scroll_offset(
            RenderFocusAutoScrollPolicy::End,
            0.0,
            30.0,
            100.0,
            80.0,
            80.0,
            160.0,
        ) - 30.0)
            .abs()
            < f32::EPSILON
    );
    assert!(
        (focus_auto_scroll_offset(
            RenderFocusAutoScrollPolicy::Disabled,
            24.0,
            30.0,
            100.0,
            80.0,
            80.0,
            160.0,
        ) - 24.0)
            .abs()
            < f32::EPSILON
    );
}

#[test]
fn ensure_choice_focus_does_not_autofocus_view_text_controls() {
    let target = target("text_input.no_auto_focus");
    let control = RenderTextInputControl::new(
        target,
        TextInputSessionId(43),
        "",
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        TextInputOptions::default(),
        SemanticRole::TextArea,
        HitRect::new(20.0, 30.0, 220.0, 80.0),
    );
    let frame = SharedFramePlanner::prepare(&scene(control)).unwrap();
    let mut input = InputController::default();

    assert!(!input.ensure_choice_focus(&frame));
    assert!(input.visual_state().focused.is_none());
    assert!(input.focused_text_editor().is_none());
}

#[test]
fn text_input_edits_player_owned_focused_text_editor_state() {
    let target = target("text_input.editor");
    let session = TextInputSessionId(42);
    let control = RenderTextInputControl::new(
        target.clone(),
        session,
        "abc",
        TextRange::new(TextByteOffset(3), TextByteOffset(3)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = SharedFramePlanner::prepare(&RenderScene {
        interaction: InteractionVisualState {
            focused: Some(target),
            hovered: None,
            pressed: None,
        },
        ..scene(control.clone())
    })
    .unwrap();
    let mut input = InputController::default();
    input.activate_text_control(&control).unwrap();
    let outcome = input
        .text_input(
            &frame,
            TextInput::committed(session, TextInputSerial(7), "d"),
        )
        .unwrap();

    assert!(outcome.redraw);
    assert_eq!(input.focused_text_editor().unwrap().text(), "abcd");
    let next_control = input.apply_live_text_control_state(control);
    assert_eq!(next_control.value, "abcd");
}

#[test]
fn pointer_activation_on_text_input_does_not_advance_dialogue() {
    let target = target("text_input.pointer");
    let control = RenderTextInputControl::new(
        target,
        TextInputSessionId(51),
        "abc",
        TextRange::new(TextByteOffset(3), TextByteOffset(3)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = prepare_with_textbox(&scene(control.clone()), false);
    let mut input = InputController::default();
    input.activate_text_control(&control).unwrap();
    let position = ViewportPoint::new(30.0, 40.0);

    let down = input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    let up = input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

    assert!(!down.dialogue_progress.advances());
    assert!(!up.dialogue_progress.advances());
}

#[test]
fn pointer_activation_on_action_button_clears_text_editor_focus() {
    let text_target = target("text_input.button_defocus");
    let button_target = target("button.button_defocus");
    let control = RenderTextInputControl::new(
        text_target,
        TextInputSessionId(69),
        "draft",
        TextRange::new(TextByteOffset(5), TextByteOffset(5)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let scene = RenderScene {
        action_buttons: vec![RenderActionButton {
            target: button_target.clone(),
            label: "Send".to_owned(),
            enabled: true,
            containing_scroll_region: None,
            bounds: HitRect::new(300.0, 30.0, 120.0, 32.0),
            viewport_clip: None,
            style: RenderControlStyle::default(),
            action: RenderActionButtonAction::Noop,
        }],
        ..scene(control.clone())
    };
    let frame = prepare_with_textbox(&scene, false);
    let mut input = InputController::default();
    input.activate_text_control(&control).unwrap();
    assert!(input.focused_text_editor().is_some());

    let position = ViewportPoint::new(320.0, 44.0);
    let down = input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    let up = input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

    assert!(!down.dialogue_progress.advances());
    assert!(!up.dialogue_progress.advances());
    assert!(input.focused_text_editor().is_none());
    assert_eq!(input.interaction().focus().target(), Some(&button_target));
}

#[test]
fn pointer_activation_on_blank_area_advances_dialogue_without_view_control_focus() {
    let target = target("text_input.blank_advance");
    let control = RenderTextInputControl::new(
        target,
        TextInputSessionId(68),
        "",
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = prepare_with_textbox(&scene(control), true);
    let mut input = InputController::default();
    let position = ViewportPoint::new(500.0, 80.0);

    let down = input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    let up = input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

    assert!(!down.dialogue_progress.advances());
    assert!(up.dialogue_progress.advances());
}

#[test]
fn pointer_activation_on_revealing_dialogue_completes_reveal_before_advance() {
    let target = target("text_input.blank_reveal");
    let control = RenderTextInputControl::new(
        target,
        TextInputSessionId(70),
        "",
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = prepare_with_textbox(&scene(control), false);
    let mut input = InputController::default();
    let position = ViewportPoint::new(500.0, 80.0);

    let down = input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    let up = input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

    assert!(!down.dialogue_progress.reveals());
    assert!(!down.dialogue_progress.advances());
    assert!(up.dialogue_progress.reveals());
    assert!(!up.dialogue_progress.advances());
}

#[test]
fn enter_without_view_control_focus_advances_dialogue() {
    let target = target("text_input.unfocused_enter");
    let control = RenderTextInputControl::new(
        target,
        TextInputSessionId(64),
        "",
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = prepare_with_textbox(&scene(control.clone()), true);
    let mut input = InputController::default();

    let outcome = input.keyboard(&frame, "Enter", KeyPhase::Down);

    assert!(outcome.dialogue_progress.advances());
}

#[test]
fn enter_without_view_control_focus_completes_dialogue_reveal_before_advance() {
    let target = target("text_input.unfocused_enter_reveal");
    let control = RenderTextInputControl::new(
        target,
        TextInputSessionId(71),
        "",
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = prepare_with_textbox(&scene(control.clone()), false);
    let mut input = InputController::default();

    let outcome = input.keyboard(&frame, "Enter", KeyPhase::Down);

    assert!(outcome.dialogue_progress.reveals());
    assert!(!outcome.dialogue_progress.advances());
}

#[test]
fn enter_with_text_input_focus_does_not_advance_dialogue() {
    let target = target("text_input.focused_enter");
    let control = RenderTextInputControl::new(
        target,
        TextInputSessionId(65),
        "",
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = prepare_with_textbox(&scene(control.clone()), false);
    let mut input = InputController::default();
    let position = ViewportPoint::new(30.0, 40.0);
    input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

    let outcome = input.keyboard(&frame, "Enter", KeyPhase::Down);

    assert!(!outcome.dialogue_progress.advances());
}

#[test]
fn backspace_advances_dialogue_only_without_view_control_focus() {
    let target = target("text_input.backspace_focus");
    let control = RenderTextInputControl::new(
        target,
        TextInputSessionId(66),
        "",
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = prepare_with_textbox(&scene(control), true);
    let mut input = InputController::default();

    let unfocused = input.keyboard(&frame, "Backspace", KeyPhase::Down);
    assert!(unfocused.dialogue_progress.advances());

    let position = ViewportPoint::new(30.0, 40.0);
    input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);
    let focused = input.keyboard(&frame, "Backspace", KeyPhase::Down);

    assert!(!focused.dialogue_progress.advances());
}

#[test]
fn pointer_down_outside_view_control_clears_text_focus_without_advancing() {
    let target = target("text_input.blank_defocus");
    let control = RenderTextInputControl::new(
        target,
        TextInputSessionId(67),
        "",
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = prepare_with_textbox(&scene(control.clone()), true);
    let mut input = InputController::default();

    let text_position = ViewportPoint::new(30.0, 40.0);
    input.pointer_down(
        &frame,
        PointerId(0),
        text_position,
        InputPointerModifiers::NONE,
    );
    input.pointer_up(
        &frame,
        PointerId(0),
        text_position,
        InputPointerModifiers::NONE,
    );
    assert!(input.interaction().focus().target().is_some());
    input.activate_text_control(&control).unwrap();
    assert!(input.focused_text_editor().is_some());

    let blank_position = ViewportPoint::new(500.0, 500.0);
    let blank = input.pointer_down(
        &frame,
        PointerId(0),
        blank_position,
        InputPointerModifiers::NONE,
    );
    let blank_up = input.pointer_up(
        &frame,
        PointerId(0),
        blank_position,
        InputPointerModifiers::NONE,
    );
    assert!(!blank.dialogue_progress.advances());
    assert!(!blank_up.dialogue_progress.advances());
    assert!(input.interaction().focus().target().is_none());
    assert!(input.focused_text_editor().is_none());

    let second_down = input.pointer_down(
        &frame,
        PointerId(0),
        blank_position,
        InputPointerModifiers::NONE,
    );
    let second_up = input.pointer_up(
        &frame,
        PointerId(0),
        blank_position,
        InputPointerModifiers::NONE,
    );
    assert!(!second_down.dialogue_progress.advances());
    assert!(second_up.dialogue_progress.advances());

    let advance = input.keyboard(&frame, "Backspace", KeyPhase::Down);
    assert!(advance.dialogue_progress.advances());
}

#[test]
fn committed_text_input_emits_typed_change_write_back() {
    let target = target("text_input.change");
    let session = TextInputSessionId(52);
    let control = RenderTextInputControl::new(
        target.clone(),
        session,
        "ab",
        TextRange::new(TextByteOffset(2), TextByteOffset(2)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = SharedFramePlanner::prepare(&RenderScene {
        interaction: InteractionVisualState {
            focused: Some(target),
            hovered: None,
            pressed: None,
        },
        ..scene(control.clone())
    })
    .unwrap();
    let mut input = InputController::default();
    input.activate_text_control(&control).unwrap();

    let outcome = input
        .text_input(
            &frame,
            TextInput::committed(session, TextInputSerial(8), "c"),
        )
        .unwrap();

    assert_eq!(outcome.text_control_write_backs().len(), 1);
    let event = &outcome.text_control_write_backs()[0];
    assert_eq!(event.kind(), TextControlWriteBackKind::Change);
    assert_eq!(event.value().as_str(), "abc");
    assert_eq!(
        event.target(),
        input.focused_text_editor().unwrap().target()
    );
}

#[test]
fn submit_command_is_distinguishable_from_change() {
    let target = target("text_input.submit");
    let session = TextInputSessionId(53);
    let control = RenderTextInputControl::new(
        target.clone(),
        session,
        "ready",
        TextRange::new(TextByteOffset(5), TextByteOffset(5)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = SharedFramePlanner::prepare(&RenderScene {
        interaction: InteractionVisualState {
            focused: Some(target),
            hovered: None,
            pressed: None,
        },
        ..scene(control.clone())
    })
    .unwrap();
    let mut input = InputController::default();
    input.activate_text_control(&control).unwrap();

    let outcome = input
        .text_input(
            &frame,
            TextInput::single(
                session,
                TextInputSerial(9),
                TextInputOperation::Command(TextEditCommand::Submit),
            ),
        )
        .unwrap();

    assert_eq!(outcome.text_control_write_backs().len(), 1);
    let event = &outcome.text_control_write_backs()[0];
    assert!(event.is_submit());
    assert!(!event.is_change());
    assert_eq!(event.value().as_str(), "ready");
}

#[test]
fn submit_command_with_text_focus_does_not_advance_active_dialogue() {
    let target = target("text_input.dialogue_submit");
    let session = TextInputSessionId(63);
    let control = RenderTextInputControl::new(
        target.clone(),
        session,
        "ready",
        TextRange::new(TextByteOffset(5), TextByteOffset(5)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = prepare_with_textbox(
        &RenderScene {
            interaction: InteractionVisualState {
                focused: Some(target),
                hovered: None,
                pressed: None,
            },
            ..scene(control.clone())
        },
        false,
    );
    let mut input = InputController::default();
    input.activate_text_control(&control).unwrap();

    let outcome = input
        .text_input(
            &frame,
            TextInput::single(
                session,
                TextInputSerial(19),
                TextInputOperation::Command(TextEditCommand::Submit),
            ),
        )
        .unwrap();

    assert!(!outcome.dialogue_progress.advances());
    assert_eq!(outcome.text_control_write_backs().len(), 1);
    assert!(outcome.text_control_write_backs()[0].is_submit());
}

#[test]
fn ime_preedit_does_not_write_back_until_commit() {
    let target = target("text_input.ime");
    let session = TextInputSessionId(54);
    let control = RenderTextInputControl::new(
        target.clone(),
        session,
        "",
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = SharedFramePlanner::prepare(&RenderScene {
        interaction: InteractionVisualState {
            focused: Some(target),
            hovered: None,
            pressed: None,
        },
        ..scene(control.clone())
    })
    .unwrap();
    let mut input = InputController::default();
    input.activate_text_control(&control).unwrap();

    let preedit = input
        .text_input(
            &frame,
            TextInput::single(
                session,
                TextInputSerial(10),
                TextInputOperation::SetComposition(TextCompositionUpdate::new(
                    "に",
                    TextRange::new(TextByteOffset(0), TextByteOffset(3)),
                )),
            ),
        )
        .unwrap();
    assert!(preedit.text_control_write_backs().is_empty());

    let commit = input
        .text_input(
            &frame,
            TextInput::committed(session, TextInputSerial(11), "日"),
        )
        .unwrap();
    assert_eq!(commit.text_control_write_backs().len(), 1);
    assert_eq!(commit.text_control_write_backs()[0].value().as_str(), "日");
}

#[test]
fn focus_loss_commits_active_ime_composition() {
    let target = target("text_input.ime_focus_loss");
    let session = TextInputSessionId(55);
    let control = RenderTextInputControl::new(
        target.clone(),
        session,
        "",
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = SharedFramePlanner::prepare(&RenderScene {
        interaction: InteractionVisualState {
            focused: Some(target),
            hovered: None,
            pressed: None,
        },
        ..scene(control.clone())
    })
    .unwrap();
    let mut input = InputController::default();
    input.activate_text_control(&control).unwrap();
    let preedit = "ちょう";

    let outcome = input
        .text_input(
            &frame,
            TextInput::single(
                session,
                TextInputSerial(12),
                TextInputOperation::SetComposition(TextCompositionUpdate::new(
                    preedit,
                    TextRange::new(
                        TextByteOffset(0),
                        TextByteOffset(u32::try_from(preedit.len()).unwrap()),
                    ),
                )),
            ),
        )
        .unwrap();
    assert!(outcome.redraw);
    assert!(input.ime_composing());
    assert!(input.focused_text_editor().is_some());

    let outcome = input.focus_changed(false);

    assert!(outcome.redraw);
    assert!(!input.ime_composing());
    assert!(input.focused_text_editor().is_none());
    assert_eq!(outcome.text_control_write_backs().len(), 1);
    assert_eq!(
        outcome.text_control_write_backs()[0].value().as_str(),
        preedit
    );
}

#[test]
fn no_op_delete_command_does_not_emit_change_write_back() {
    let target = target("text_input.noop_delete");
    let session = TextInputSessionId(56);
    let control = RenderTextInputControl::new(
        target.clone(),
        session,
        "",
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = SharedFramePlanner::prepare(&RenderScene {
        interaction: InteractionVisualState {
            focused: Some(target),
            hovered: None,
            pressed: None,
        },
        ..scene(control.clone())
    })
    .unwrap();
    let mut input = InputController::default();
    input.activate_text_control(&control).unwrap();

    let outcome = input
        .text_input(
            &frame,
            TextInput::single(
                session,
                TextInputSerial(13),
                TextInputOperation::Command(TextEditCommand::Backspace),
            ),
        )
        .unwrap();

    assert!(outcome.text_control_write_backs().is_empty());
}

#[test]
fn secure_write_back_value_is_available_but_redacted_in_debug() {
    let target = target("text_input.secure");
    let session = TextInputSessionId(55);
    let control = RenderTextInputControl::new(
        target.clone(),
        session,
        "",
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        TextInputOptions::default().secure(true),
        SemanticRole::SecureTextField,
        HitRect::new(20.0, 30.0, 220.0, 32.0),
    );
    let frame = SharedFramePlanner::prepare(&RenderScene {
        interaction: InteractionVisualState {
            focused: Some(target),
            hovered: None,
            pressed: None,
        },
        ..scene(control.clone())
    })
    .unwrap();
    let mut input = InputController::default();
    input.activate_text_control(&control).unwrap();

    let outcome = input
        .text_input(
            &frame,
            TextInput::committed(session, TextInputSerial(12), "secret")
                .with_privacy(TextInputPrivacy::Sensitive),
        )
        .unwrap();

    let event = &outcome.text_control_write_backs()[0];
    assert_eq!(event.value().as_str(), "secret");
    assert!(event.value().is_sensitive());
    let debug = format!("{event:?}");
    assert!(!debug.contains("secret"));
    assert!(debug.contains("<redacted>"));
}
