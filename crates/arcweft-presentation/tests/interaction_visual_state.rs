use arcweft_id::PublicId;
use arcweft_presentation::hit::{HitRecord, HitRect, HitTree};
use arcweft_presentation::hover::HoverPath;
use arcweft_presentation::input::{
    InputEpoch, InputEvent, InputEventKind, InteractionTarget, PointerId,
};
use arcweft_presentation::interaction::{
    FocusState, InteractionState, PointerCapture, PressedTarget,
};
use arcweft_presentation::layer::{
    LayerId, LayerKind, LayerNode, LayerOrder, LayerTree, RenderPhase,
};
use arcweft_presentation::replay::routing_hash;

fn public_id(value: &str) -> PublicId {
    PublicId::try_new(value).unwrap()
}

fn target(value: &str) -> InteractionTarget {
    InteractionTarget::new(public_id(&format!("target.{value}")))
}

fn layer(value: &str) -> LayerId {
    LayerId::new(public_id(&format!("layer.{value}")))
}

fn tree(root: &LayerId) -> LayerTree {
    LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        LayerOrder {
            phase: RenderPhase::Background,
            z: 0,
            stable_index: 0,
        },
    ))
}

#[test]
fn shared_state_tracks_hover_focus_pressed_and_capture_by_stable_target() {
    let ui = layer("ui");
    let first = target("button.first");
    let second = target("button.second");
    let mut state = InteractionState::default();

    let _ = state.set_hover_path(HoverPath::new(
        PointerId(2),
        vec![target("panel"), second.clone()],
    ));
    let _ = state.set_hover_path(HoverPath::new(PointerId(1), vec![first.clone()]));
    state.set_focus(FocusState::new(ui.clone(), second.clone()));
    state.capture_pointer(PointerCapture::new(
        PointerId(2),
        ui.clone(),
        second.clone(),
    ));
    state.press_pointer(PressedTarget::new(PointerId(2), ui, second.clone()));

    assert_eq!(state.primary_hovered_target(), Some(&first));
    assert_eq!(state.primary_pressed_target(), Some(&second));
    assert!(state.is_hovered(&second));
    assert!(state.is_focused(&second));
    assert!(state.is_pressed(&second));
    assert_eq!(
        state.capture_for(PointerId(2)).map(PointerCapture::target),
        Some(&second)
    );

    state.clear_pointer(PointerId(2));
    assert!(!state.is_pressed(&second));
    assert!(state.capture_for(PointerId(2)).is_none());
    assert!(state.hovered_target(PointerId(2)).is_none());
}

#[test]
fn focus_events_and_hit_hover_paths_keep_behavior_on_the_owned_types() {
    let button = target("button.confirm");
    let parent = target("panel.confirmation");
    let gained = InputEvent::focus_changed(InputEpoch(9), button.clone(), true);
    let lost = InputEvent::focus_changed(InputEpoch(10), button.clone(), false);

    assert_eq!(gained.kind().focus_changed(), Some(true));
    assert_eq!(lost.kind().focus_changed(), Some(false));
    assert!(InputEventKind::Activate.is_activate());

    let record = HitRecord::new(
        layer("ui"),
        button.clone(),
        HitRect::new(0.0, 0.0, 80.0, 24.0),
    )
    .with_hover_path(vec![parent]);
    assert_eq!(record.hover_path().last(), Some(&button));
}

#[test]
fn replay_hash_changes_when_hover_or_pressed_state_changes() {
    let root = layer("root");
    let ui = layer("ui");
    let button = target("button.confirm");
    let layers = tree(&root);
    let mut hits = HitTree::default();
    hits.push(HitRecord::new(
        ui.clone(),
        button.clone(),
        HitRect::new(0.0, 0.0, 80.0, 24.0),
    ));

    let neutral = routing_hash(&layers, &hits, &InteractionState::default());
    let mut hovered = InteractionState::default();
    let _ = hovered.set_hover_path(HoverPath::new(PointerId(0), vec![button.clone()]));
    let hovered_hash = routing_hash(&layers, &hits, &hovered);
    assert_ne!(neutral, hovered_hash);

    hovered.press_pointer(PressedTarget::new(PointerId(0), ui, button));
    assert_ne!(hovered_hash, routing_hash(&layers, &hits, &hovered));
}
