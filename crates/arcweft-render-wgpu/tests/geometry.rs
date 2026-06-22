use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, RenderChoiceItem, RenderPreferences, RenderScene,
    RenderViewport, SharedFramePlanner,
};
use arcweft_render_wgpu::sample::{DemoAnimationClock, DemoImageKind, generated_demo_images};

fn scene() -> RenderScene {
    let viewport = RenderViewport {
        logical_width: 1280.0,
        logical_height: 720.0,
        physical_width: 1280,
        physical_height: 720,
        scale_factor: 1.0,
    };
    RenderScene {
        dialogue: None,
        choices: vec![
            RenderChoiceItem {
                id: "choice.one".to_owned(),
                label: "One".to_owned(),
            },
            RenderChoiceItem {
                id: "choice.two".to_owned(),
                label: "Two".to_owned(),
            },
        ],
        images: Vec::new(),
        viewport,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
    }
}

fn generated_scene(elapsed_millis: u64) -> RenderScene {
    let mut scene = scene();
    scene.images = generated_demo_images(
        scene.viewport,
        DemoAnimationClock::from_millis(elapsed_millis),
    );
    scene
}

#[test]
fn planner_uses_the_same_choice_geometry_for_render_and_hit_test() {
    let frame = SharedFramePlanner::prepare(&scene()).expect("frame plans");
    assert_eq!(frame.choices.len(), 2);
    for choice in &frame.choices {
        assert!(frame.hits.find_target(&choice.target).is_some());
        assert!(frame.semantics.find(&choice.target).is_some());
    }
}

#[test]
fn keyboard_navigation_wraps_across_stable_targets() {
    let frame = SharedFramePlanner::prepare(&scene()).expect("frame plans");
    let first = frame.first_choice_target().expect("first target");
    let previous = frame
        .adjacent_choice_target(Some(&first), -1)
        .expect("wrapped target");
    assert_eq!(previous, frame.choices[1].target);
}

#[test]
fn interaction_visual_state_changes_the_prepared_choice_rectangles() {
    let base_scene = scene();
    let neutral = SharedFramePlanner::prepare(&base_scene).expect("neutral frame plans");
    let first = neutral.first_choice_target().expect("first target");
    let focused = SharedFramePlanner::prepare(&RenderScene {
        interaction: InteractionVisualState {
            focused: Some(first.clone()),
            hovered: None,
            pressed: None,
        },
        ..base_scene.clone()
    })
    .expect("focused frame plans");
    let pressed = SharedFramePlanner::prepare(&RenderScene {
        interaction: InteractionVisualState {
            focused: Some(first.clone()),
            hovered: Some(first.clone()),
            pressed: Some(first),
        },
        ..base_scene
    })
    .expect("pressed frame plans");

    assert!(
        focused.rectangles.len() > neutral.rectangles.len(),
        "focused choice should add a visible focus ring"
    );
    assert!(
        focused.rectangles[1]
            .rgba
            .iter()
            .zip(pressed.rectangles[1].rgba.iter())
            .any(|(focused, pressed)| (focused - pressed).abs() > f32::EPSILON),
        "pressed choice should use a distinct fill"
    );
}

#[test]
fn generated_visual_demo_supplies_background_character_and_animated_frames() {
    let frame_a = SharedFramePlanner::prepare(&generated_scene(0)).expect("frame plans");
    let frame_b = SharedFramePlanner::prepare(&generated_scene(170)).expect("frame plans");

    assert_eq!(frame_a.images.len(), 4);
    assert_eq!(frame_a.images[0].id, DemoImageKind::Background.asset_id());
    assert_eq!(
        frame_a.images[1].id,
        DemoImageKind::CharacterStand.asset_id()
    );
    assert_ne!(
        frame_a.images[2].frame.rgba, frame_b.images[2].frame.rgba,
        "GIF sample frame should animate over time"
    );
    assert_ne!(
        frame_a.images[3].frame.rgba, frame_b.images[3].frame.rgba,
        "WebP sample frame should animate over time"
    );
}
