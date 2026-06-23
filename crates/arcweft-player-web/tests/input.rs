use arcweft_player_scene::input::InputController;
use arcweft_presentation::input::{KeyPhase, PointerId, ViewportPoint};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, RenderChoiceItem, RenderPreferences, RenderScene,
    RenderViewport, SharedFramePlanner,
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
    })
    .expect("frame plans")
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
    input.pointer_down(&frame, PointerId(0), point);
    let outcome = input.pointer_up(&frame, PointerId(0), point);

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
    input.pointer_down(&frame, PointerId(0), start);
    input.pointer_move(&frame, PointerId(0), moved);
    let outcome = input.pointer_up(&frame, PointerId(0), moved);

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
fn wheel_input_does_not_move_choice_scroll_state() {
    let mut input = InputController::default();
    let before = input.choice_scroll();
    let outcome = input.wheel(180.0);

    assert!(outcome.redraw);
    assert_eq!(input.choice_scroll(), before);
}
