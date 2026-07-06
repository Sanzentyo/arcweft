use arcweft_player_scene::input::InputController;
use arcweft_presentation::input::{PointerId, ViewportPoint};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, RenderChoiceItem, RenderPreferences, RenderScene,
    RenderViewport, SharedFramePlanner,
};

fn frame() -> arcweft_render_wgpu::geometry::PreparedFrame {
    SharedFramePlanner::prepare(&RenderScene {
        dialogue: None,
        choices: vec![
            RenderChoiceItem {
                id: "first".to_owned(),
                label: "First".to_owned(),
            },
            RenderChoiceItem {
                id: "second".to_owned(),
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
    .unwrap()
}

fn center(bounds: arcweft_presentation::hit::HitRect) -> ViewportPoint {
    ViewportPoint::new(
        bounds.x + bounds.width * 0.5,
        bounds.y + bounds.height * 0.5,
    )
}

#[test]
fn web_choice_visuals_are_derived_from_shared_interaction_state() {
    let frame = frame();
    let first = frame.choices[0].target.clone();
    let second = frame.choices[1].target.clone();
    let first_point = center(frame.hits.find_target(&first).unwrap().bounds());
    let second_point = center(frame.hits.find_target(&second).unwrap().bounds());
    let mut input = InputController::default();

    input.pointer_move(&frame, PointerId(0), second_point);
    assert_eq!(input.visual_state().hovered, Some(second));

    input.pointer_down(&frame, PointerId(0), first_point);
    assert_eq!(input.visual_state().pressed, Some(first));

    input.pointer_up(&frame, PointerId(0), first_point);
    assert!(input.visual_state().pressed.is_none());
}
