use super::*;
use crate::hit::{HitRecord, HitRect, HitTree};
use crate::input::{
    Action, ActionBatch, ActionTarget, AgentInput, HostEvent, HostEventBatch, HostEventSource,
    InputEpoch, InputEvent, InputEventKind, InteractionTarget, KeyPhase, KeyboardInput, PointerId,
    PointerInput, PointerPhase, RawInputEvent, RawInputKind, TextInput, ViewportPoint,
};
use crate::interaction::{FocusState, InteractionState, PointerCapture};
use crate::layer::{
    LayerContent, LayerId, LayerInputPolicy, LayerKind, LayerNode, LayerOrder, LayerTree,
    LayerTreeError, LayerVisibility, RenderPhase,
};
use crate::router::{InputRouter, RouteDecision};

#[test]
fn registry_clears_values_when_scope_exits() {
    let mut registry = PresentationRegistry::default();
    let line_scope = PresentationScope::line();
    let flow_scope = PresentationScope::flow();

    let line_bg = bg(asset("bg.room"), line_scope.clone());
    let flow_bg = PresentationHandle::new(
        BackgroundSurface::new(asset("bg.evening")),
        PresentationTarget::scene(),
        PresentationSlot::new(PublicId::try_new("slot.background.flow").unwrap()),
        flow_scope.clone(),
    );
    let line_ref = line_bg.slot_ref();
    let flow_ref = flow_bg.slot_ref();

    assert_eq!(registry.set(line_bg), None);
    assert_eq!(registry.set(flow_bg), None);
    assert!(registry.get(&line_ref).is_some());
    assert!(registry.get(&flow_ref).is_some());

    let removed = registry.exit_scope(&line_scope);
    assert_eq!(removed.len(), 1);
    assert!(registry.get(&line_ref).is_none());
    assert!(registry.get(&flow_ref).is_some());
}

#[test]
fn clear_returns_registered_value() {
    let mut registry = PresentationRegistry::default();
    let scope = PresentationScope::line();
    let handle = bg(asset("bg.room"), scope.clone());
    let clear = clear_bg(scope);

    registry.set(handle);
    let removed = registry.clear(&clear).expect("registered background");
    assert_eq!(removed.asset().as_str(), "asset.bg.room");
    assert!(registry.clear(&clear).is_none());
}

#[test]
fn handles_share_scope_lifetime_and_default_slots() {
    let line_scope = PresentationScope::line();
    let alice = PublicId::try_new("character.alice").unwrap();
    let background = bg(asset("bg.room"), line_scope.clone());
    let shown_alice = show_character(&alice, "smile", line_scope.clone());

    assert_eq!(background.scope(), &line_scope);
    assert_eq!(shown_alice.scope(), &line_scope);
    assert_eq!(background.target().id().as_str(), "target.scene");
    assert_eq!(background.slot().id().as_str(), "slot.background.default");
    assert_eq!(
        shown_alice.slot().id().as_str(),
        "slot.character.alice.default"
    );
    assert_eq!(shown_alice.value().character().as_str(), "character.alice");
    assert_eq!(
        shown_alice.value().expression().map(PublicId::as_str),
        Some("expression.smile")
    );
}

#[test]
fn slot_value_behaves_like_static_option() {
    let mut slot = SlotValue::empty(
        PresentationTarget::scene(),
        PresentationSlot::default_background(),
    );
    assert!(slot.get().is_none());

    let first = bg(asset("bg.room"), PresentationScope::flow());
    assert!(slot.set(first).is_none());
    assert_eq!(
        slot.get()
            .map(BackgroundSurface::asset)
            .map(PublicId::as_str),
        Some("asset.bg.room")
    );

    let second = bg(asset("bg.evening"), PresentationScope::line());
    let previous = slot.set(second).expect("previous background is returned");
    assert_eq!(previous.asset().as_str(), "asset.bg.room");
    assert_eq!(
        slot.get()
            .map(BackgroundSurface::asset)
            .map(PublicId::as_str),
        Some("asset.bg.evening")
    );

    let cleared = slot.clear().expect("background clears");
    assert_eq!(cleared.asset().as_str(), "asset.bg.evening");
    assert!(slot.get().is_none());
    assert_eq!(
        clear_bg(PresentationScope::line()).slot().id().as_str(),
        "slot.background.default"
    );
    assert_eq!(
        hide_character(
            &PublicId::try_new("character.alice").unwrap(),
            PresentationScope::line()
        )
        .slot()
        .id()
        .as_str(),
        "slot.character.alice.default"
    );
}

#[test]
fn routed_input_keeps_raw_epoch_and_stable_target() {
    let target = InteractionTarget::new(PublicId::try_new("target.textbox.main").unwrap());
    let raw = RawInputEvent::new(
        InputEpoch(7),
        RawInputKind::Agent(AgentInput {
            action: PublicId::try_new("action.advance").unwrap(),
            target: Some(target.clone()),
        }),
    );
    let routed = InputEvent::new(
        raw.epoch(),
        target.clone(),
        InputEventKind::Key {
            key: "Enter".to_owned(),
            phase: KeyPhase::Down,
        },
    );

    assert_eq!(routed.raw_epoch(), InputEpoch(7));
    assert_eq!(routed.target(), &target);
    assert!(matches!(
        routed.kind(),
        InputEventKind::Key {
            key,
            phase: KeyPhase::Down
        } if key == "Enter"
    ));
}

#[test]
fn action_and_host_event_batches_preserve_ordered_owned_data() {
    let target = InteractionTarget::new(PublicId::try_new("target.activity.truck").unwrap());
    let mut actions = ActionBatch::default();
    actions.push(Action::new(
        ActionTarget::Activity(target.clone()),
        PublicId::try_new("action.pause").unwrap(),
    ));
    actions.push(
        Action::new(
            ActionTarget::Entity(target.clone()),
            PublicId::try_new("action.inspect").unwrap(),
        )
        .with_payload("bbox"),
    );

    assert_eq!(actions.as_slice().len(), 2);
    assert_eq!(actions.as_slice()[0].kind().as_str(), "action.pause");
    assert_eq!(
        actions.as_slice()[1].payload().map(String::as_str),
        Some("bbox")
    );

    let mut events = HostEventBatch::default();
    events.push(HostEvent::new(
        HostEventSource::Activity(target),
        PublicId::try_new("host.activity.ready").unwrap(),
    ));
    events.push(
        HostEvent::new(
            HostEventSource::Task(PublicId::try_new("task.load").unwrap()),
            PublicId::try_new("host.task.done").unwrap(),
        )
        .with_payload("ok"),
    );

    assert_eq!(events.as_slice().len(), 2);
    assert_eq!(events.as_slice()[0].kind().as_str(), "host.activity.ready");
    assert_eq!(
        events.as_slice()[1].payload().map(String::as_str),
        Some("ok")
    );
}

fn layer_id(name: &str) -> LayerId {
    LayerId::new(PublicId::try_new(format!("layer.{name}")).unwrap())
}

fn layer_order(phase: RenderPhase, z: i32, stable_index: u32) -> LayerOrder {
    LayerOrder {
        phase,
        z,
        stable_index,
    }
}

fn interaction_target(name: &str) -> InteractionTarget {
    InteractionTarget::new(PublicId::try_new(format!("target.{name}")).unwrap())
}

#[test]
fn layer_tree_derives_render_and_input_order_from_same_nodes() {
    let root = layer_id("root");
    let mut tree = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        layer_order(RenderPhase::Background, 0, 0),
    ));
    let dialogue = layer_id("dialogue");
    let modal = layer_id("modal");

    tree.insert(
        LayerNode::new(
            dialogue.clone(),
            LayerKind::TextBox,
            layer_order(RenderPhase::Dialogue, 0, 10),
        )
        .with_parent(root.clone())
        .with_content(LayerContent::TextBox(
            PublicId::try_new("textbox.main").unwrap(),
        ))
        .with_input_policy(LayerInputPolicy::HitTest),
    )
    .expect("dialogue layer inserts");
    tree.insert(
        LayerNode::new(
            modal.clone(),
            LayerKind::Modal,
            layer_order(RenderPhase::Modal, 0, 20),
        )
        .with_parent(root.clone())
        .with_content(LayerContent::Activity(
            PublicId::try_new("activity.pause_menu").unwrap(),
        ))
        .with_input_policy(LayerInputPolicy::Modal),
    )
    .expect("modal layer inserts");

    assert_eq!(tree.root(), &root);
    assert_eq!(
        tree.render_order(),
        &[root.clone(), dialogue.clone(), modal.clone()]
    );
    assert_eq!(
        tree.input_order(),
        &[modal.clone(), dialogue.clone(), root.clone()]
    );
    assert_eq!(
        tree.get(&root).map(LayerNode::children),
        Some(&[dialogue, modal][..])
    );
    assert_eq!(
        tree.get(&layer_id("dialogue")).map(LayerNode::content),
        Some(&LayerContent::TextBox(
            PublicId::try_new("textbox.main").unwrap()
        ))
    );
}

#[test]
fn layer_tree_rejects_duplicate_and_missing_parent() {
    let root = layer_id("root");
    let mut tree = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        layer_order(RenderPhase::Background, 0, 0),
    ));

    let duplicate = LayerNode::new(
        root.clone(),
        LayerKind::Debug,
        layer_order(RenderPhase::Debug, 0, 0),
    );
    assert_eq!(
        tree.insert(duplicate),
        Err(LayerTreeError::DuplicateLayer(root.clone()))
    );

    let orphan = layer_id("orphan");
    let missing = layer_id("missing");
    assert_eq!(
        tree.insert(
            LayerNode::new(
                orphan,
                LayerKind::Activity,
                layer_order(RenderPhase::World, 0, 0),
            )
            .with_parent(missing.clone())
        ),
        Err(LayerTreeError::MissingParent(missing))
    );
}

#[test]
fn hidden_layers_are_not_routable_or_rendered() {
    let root = layer_id("root");
    let hidden = layer_id("hidden");
    let mut tree = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        layer_order(RenderPhase::Background, 0, 0),
    ));

    tree.insert(
        LayerNode::new(
            hidden,
            LayerKind::Debug,
            layer_order(RenderPhase::Debug, 0, 0),
        )
        .with_parent(root.clone())
        .with_visibility(LayerVisibility::Hidden),
    )
    .expect("hidden layer inserts");

    assert_eq!(tree.render_order(), std::slice::from_ref(&root));
    assert_eq!(tree.input_order(), std::slice::from_ref(&root));
}

#[test]
fn router_routes_pointer_by_layer_order_and_modal_blocks_lower_layers() {
    let root = layer_id("root");
    let world = layer_id("world");
    let modal = layer_id("modal");
    let world_target = interaction_target("activity.world");
    let modal_target = interaction_target("modal.close");
    let mut tree = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        layer_order(RenderPhase::Background, 0, 0),
    ));
    tree.insert(
        LayerNode::new(
            world.clone(),
            LayerKind::Activity,
            layer_order(RenderPhase::World, 0, 10),
        )
        .with_parent(root.clone())
        .with_input_policy(LayerInputPolicy::HitTest),
    )
    .expect("world inserts");
    tree.insert(
        LayerNode::new(
            modal.clone(),
            LayerKind::Modal,
            layer_order(RenderPhase::Modal, 0, 20),
        )
        .with_parent(root)
        .with_input_policy(LayerInputPolicy::Modal),
    )
    .expect("modal inserts");

    let mut hits = HitTree::default();
    hits.push(HitRecord::new(
        world.clone(),
        world_target,
        HitRect::new(0.0, 0.0, 100.0, 100.0),
    ));
    hits.push(HitRecord::new(
        modal.clone(),
        modal_target.clone(),
        HitRect::new(0.0, 0.0, 20.0, 20.0),
    ));

    let modal_hit = RawInputEvent::new(
        InputEpoch(10),
        RawInputKind::Pointer(PointerInput {
            pointer: PointerId(1),
            position: ViewportPoint::new(5.0, 5.0),
            phase: PointerPhase::Down,
        }),
    );
    let routed = InputRouter::route(&modal_hit, &tree, &hits, &InteractionState::default());
    assert!(matches!(
        routed.event().map(InputEvent::target),
        Some(target) if target == &modal_target
    ));

    let lower_hit = RawInputEvent::new(
        InputEpoch(11),
        RawInputKind::Pointer(PointerInput {
            pointer: PointerId(1),
            position: ViewportPoint::new(50.0, 50.0),
            phase: PointerPhase::Down,
        }),
    );
    let blocked = InputRouter::route(&lower_hit, &tree, &hits, &InteractionState::default());
    assert_eq!(blocked.decision(), &RouteDecision::BlockedByModal { modal });
}

#[test]
fn router_keeps_agent_invocation_inside_layer_and_modal_policy() {
    let root = layer_id("root");
    let world = layer_id("world");
    let modal = layer_id("modal");
    let world_target = interaction_target("activity.world");
    let modal_target = interaction_target("modal.close");
    let mut tree = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        layer_order(RenderPhase::Background, 0, 0),
    ));
    tree.insert(
        LayerNode::new(
            world.clone(),
            LayerKind::Activity,
            layer_order(RenderPhase::World, 0, 10),
        )
        .with_parent(root.clone())
        .with_input_policy(LayerInputPolicy::HitTest),
    )
    .expect("world inserts");
    tree.insert(
        LayerNode::new(
            modal.clone(),
            LayerKind::Modal,
            layer_order(RenderPhase::Modal, 0, 20),
        )
        .with_parent(root)
        .with_input_policy(LayerInputPolicy::Modal),
    )
    .expect("modal inserts");

    let mut hits = HitTree::default();
    hits.push(HitRecord::new(
        world,
        world_target.clone(),
        HitRect::new(0.0, 0.0, 100.0, 100.0),
    ));
    hits.push(HitRecord::new(
        modal.clone(),
        modal_target.clone(),
        HitRect::new(0.0, 0.0, 20.0, 20.0),
    ));

    let lower_agent = RawInputEvent::new(
        InputEpoch(20),
        RawInputKind::Agent(AgentInput {
            action: PublicId::try_new("action.inspect").unwrap(),
            target: Some(world_target),
        }),
    );
    let blocked = InputRouter::route(&lower_agent, &tree, &hits, &InteractionState::default());
    assert_eq!(
        blocked.decision(),
        &RouteDecision::BlockedByModal {
            modal: modal.clone()
        }
    );

    let modal_agent = RawInputEvent::new(
        InputEpoch(21),
        RawInputKind::Agent(AgentInput {
            action: PublicId::try_new("action.close").unwrap(),
            target: Some(modal_target.clone()),
        }),
    );
    let routed = InputRouter::route(&modal_agent, &tree, &hits, &InteractionState::default());
    assert!(matches!(
        routed.event().map(InputEvent::target),
        Some(target) if target == &modal_target
    ));
}

#[test]
fn router_routes_keyboard_and_text_to_focus_target() {
    let root = layer_id("root");
    let dialogue = layer_id("dialogue");
    let target = interaction_target("textbox.main");
    let mut tree = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        layer_order(RenderPhase::Background, 0, 0),
    ));
    tree.insert(
        LayerNode::new(
            dialogue.clone(),
            LayerKind::TextBox,
            layer_order(RenderPhase::Dialogue, 0, 10),
        )
        .with_parent(root)
        .with_input_policy(LayerInputPolicy::HitTest),
    )
    .expect("dialogue inserts");

    let mut hits = HitTree::default();
    hits.push(HitRecord::new(
        dialogue.clone(),
        target.clone(),
        HitRect::new(0.0, 0.0, 500.0, 160.0),
    ));
    let mut state = InteractionState::default();
    state.set_focus(FocusState::new(dialogue, target.clone()));

    let key = RawInputEvent::new(
        InputEpoch(30),
        RawInputKind::Keyboard(KeyboardInput {
            key: "Enter".to_owned(),
            phase: KeyPhase::Down,
        }),
    );
    let routed_key = InputRouter::route(&key, &tree, &hits, &state);
    assert!(matches!(
        routed_key.event().map(InputEvent::kind),
        Some(InputEventKind::Key {
            key,
            phase: KeyPhase::Down
        }) if key == "Enter"
    ));

    let text = RawInputEvent::new(InputEpoch(31), RawInputKind::Text(TextInput::new("abc")));
    let routed_text = InputRouter::route(&text, &tree, &hits, &state);
    assert!(matches!(
        routed_text.event().map(InputEvent::kind),
        Some(InputEventKind::Text(value)) if value == "abc"
    ));
    assert_eq!(routed_text.event().map(InputEvent::target), Some(&target));
}

#[test]
fn router_sends_pointer_events_to_active_capture_owner() {
    let root = layer_id("root");
    let activity = layer_id("activity");
    let target = interaction_target("activity.drag");
    let mut tree = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        layer_order(RenderPhase::Background, 0, 0),
    ));
    tree.insert(
        LayerNode::new(
            activity.clone(),
            LayerKind::Activity,
            layer_order(RenderPhase::World, 0, 10),
        )
        .with_parent(root)
        .with_input_policy(LayerInputPolicy::Capture),
    )
    .expect("activity inserts");

    let mut state = InteractionState::default();
    state.capture_pointer(PointerCapture::new(PointerId(9), activity, target.clone()));
    let raw = RawInputEvent::new(
        InputEpoch(40),
        RawInputKind::Pointer(PointerInput {
            pointer: PointerId(9),
            position: ViewportPoint::new(999.0, 999.0),
            phase: PointerPhase::Move,
        }),
    );

    let routed = InputRouter::route(&raw, &tree, &HitTree::default(), &state);
    assert_eq!(routed.event().map(InputEvent::target), Some(&target));
    assert!(matches!(
        routed.event().map(InputEvent::kind),
        Some(InputEventKind::Pointer {
            phase: PointerPhase::Move
        })
    ));
}
