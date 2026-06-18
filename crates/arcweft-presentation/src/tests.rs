use super::*;
use crate::gesture::{GestureArena, GestureKind, GestureOutcome};
use crate::hit::{HitRecord, HitRect, HitTree};
use crate::hover::{HoverPath, HoverTransition};
use crate::input::{
    Action, ActionBatch, ActionTarget, AgentInput, HostEvent, HostEventBatch, HostEventSource,
    InputEpoch, InputEvent, InputEventKind, InteractionTarget, KeyPhase, KeyboardInput, PointerId,
    PointerInput, PointerPhase, RawInputEvent, RawInputKind, TextInput, ViewportPoint,
};
use crate::interaction::{FocusState, InteractionState, PointerCapture};
use crate::layer::{
    LayerContent, LayerId, LayerInputPolicy, LayerKind, LayerNode, LayerOrder, LayerTransform,
    LayerTree, LayerTreeError, LayerVisibility, RenderPhase,
};
use crate::replay::{route_fingerprint, routing_hash};
use crate::router::{InputRouter, RouteDecision};
use crate::semantic::{SemanticActionError, SemanticNode, SemanticRole, SemanticTree};

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

#[test]
fn router_maps_viewport_pointer_into_layer_local_hit_bounds() {
    let root = layer_id("root");
    let translated = layer_id("translated");
    let target = interaction_target("ui.translated.button");
    let mut tree = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        layer_order(RenderPhase::Background, 0, 0),
    ));
    tree.insert(
        LayerNode::new(
            translated.clone(),
            LayerKind::GameUi,
            layer_order(RenderPhase::GameUi, 0, 10),
        )
        .with_parent(root)
        .with_transform(LayerTransform::translation_milli(100_000, 50_000))
        .with_input_policy(LayerInputPolicy::HitTest),
    )
    .expect("translated layer inserts");

    let mut hits = HitTree::default();
    hits.push(HitRecord::new(
        translated,
        target.clone(),
        HitRect::new(0.0, 0.0, 20.0, 20.0),
    ));
    let raw = RawInputEvent::new(
        InputEpoch(50),
        RawInputKind::Pointer(PointerInput {
            pointer: PointerId(4),
            position: ViewportPoint::new(110.0, 60.0),
            phase: PointerPhase::Down,
        }),
    );
    let routed = InputRouter::route(&raw, &tree, &hits, &InteractionState::default());
    assert_eq!(routed.event().map(InputEvent::target), Some(&target));

    let outside = RawInputEvent::new(
        InputEpoch(51),
        RawInputKind::Pointer(PointerInput {
            pointer: PointerId(4),
            position: ViewportPoint::new(10.0, 10.0),
            phase: PointerPhase::Down,
        }),
    );
    let missed = InputRouter::route(&outside, &tree, &hits, &InteractionState::default());
    assert_eq!(missed.decision(), &RouteDecision::NoTarget);
}

#[test]
fn router_skips_non_invertible_transform_without_blocking_lower_layer() {
    let root = layer_id("root");
    let lower = layer_id("lower");
    let broken = layer_id("broken");
    let lower_target = interaction_target("world.lower");
    let broken_target = interaction_target("modal.broken");
    let mut tree = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        layer_order(RenderPhase::Background, 0, 0),
    ));
    tree.insert(
        LayerNode::new(
            lower.clone(),
            LayerKind::Activity,
            layer_order(RenderPhase::World, 0, 10),
        )
        .with_parent(root.clone())
        .with_input_policy(LayerInputPolicy::HitTest),
    )
    .expect("lower inserts");
    tree.insert(
        LayerNode::new(
            broken.clone(),
            LayerKind::Modal,
            layer_order(RenderPhase::Modal, 0, 20),
        )
        .with_parent(root)
        .with_transform(LayerTransform::scale_milli(0, 0))
        .with_input_policy(LayerInputPolicy::Modal),
    )
    .expect("broken inserts");

    let mut hits = HitTree::default();
    hits.push(HitRecord::new(
        lower,
        lower_target.clone(),
        HitRect::new(0.0, 0.0, 100.0, 100.0),
    ));
    hits.push(HitRecord::new(
        broken,
        broken_target,
        HitRect::new(0.0, 0.0, 100.0, 100.0),
    ));

    let raw = RawInputEvent::new(
        InputEpoch(52),
        RawInputKind::Pointer(PointerInput {
            pointer: PointerId(5),
            position: ViewportPoint::new(10.0, 10.0),
            phase: PointerPhase::Down,
        }),
    );
    let routed = InputRouter::route(&raw, &tree, &hits, &InteractionState::default());
    assert_eq!(routed.event().map(InputEvent::target), Some(&lower_target));
}

#[test]
fn hover_transition_diffs_only_changed_suffix() {
    let root = interaction_target("root");
    let list = interaction_target("choice.list");
    let listen = interaction_target("choice.listen");
    let leave = interaction_target("choice.leave");
    let previous = HoverPath::new(
        PointerId(1),
        vec![root.clone(), list.clone(), listen.clone()],
    );
    let next = HoverPath::new(PointerId(1), vec![root, list, leave.clone()]);

    let transition = HoverTransition::diff(Some(&previous), Some(&next)).expect("diff exists");
    assert_eq!(transition.exited(), std::slice::from_ref(&listen));
    assert_eq!(transition.entered(), std::slice::from_ref(&leave));
}

#[test]
fn router_hover_path_uses_hit_record_path_and_respects_modal_block() {
    let root = layer_id("root");
    let world = layer_id("world");
    let modal = layer_id("modal");
    let root_target = interaction_target("root");
    let activity_target = interaction_target("activity.world");
    let button_target = interaction_target("activity.button");
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
    hits.push(
        HitRecord::new(
            world,
            button_target.clone(),
            HitRect::new(0.0, 0.0, 100.0, 100.0),
        )
        .with_hover_path(vec![
            root_target.clone(),
            activity_target,
            button_target.clone(),
        ]),
    );
    hits.push(HitRecord::new(
        modal,
        modal_target.clone(),
        HitRect::new(0.0, 0.0, 20.0, 20.0),
    ));

    let modal_path =
        InputRouter::hover_path(PointerId(2), ViewportPoint::new(5.0, 5.0), &tree, &hits)
            .expect("modal hover path");
    assert_eq!(modal_path.targets(), std::slice::from_ref(&modal_target));

    assert_eq!(
        InputRouter::hover_path(PointerId(2), ViewportPoint::new(50.0, 50.0), &tree, &hits),
        None
    );
}

#[test]
fn routing_hash_changes_when_layer_or_hit_routing_state_changes() {
    let root = layer_id("root");
    let world = layer_id("world");
    let target = interaction_target("activity.world");
    let mut base_tree = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        layer_order(RenderPhase::Background, 0, 0),
    ));
    base_tree
        .insert(
            LayerNode::new(
                world.clone(),
                LayerKind::Activity,
                layer_order(RenderPhase::World, 0, 10),
            )
            .with_parent(root.clone())
            .with_input_policy(LayerInputPolicy::HitTest),
        )
        .expect("world inserts");

    let mut base_hits = HitTree::default();
    base_hits.push(HitRecord::new(
        world.clone(),
        target.clone(),
        HitRect::new(0.0, 0.0, 100.0, 100.0),
    ));
    let base = routing_hash(&base_tree, &base_hits, &InteractionState::default());
    assert_eq!(
        base,
        routing_hash(&base_tree, &base_hits, &InteractionState::default())
    );

    let mut moved_hits = HitTree::default();
    moved_hits.push(HitRecord::new(
        world.clone(),
        target.clone(),
        HitRect::new(1.0, 0.0, 100.0, 100.0),
    ));
    assert_ne!(
        base,
        routing_hash(&base_tree, &moved_hits, &InteractionState::default())
    );

    let mut modal_tree = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        layer_order(RenderPhase::Background, 0, 0),
    ));
    modal_tree
        .insert(
            LayerNode::new(
                world.clone(),
                LayerKind::Activity,
                layer_order(RenderPhase::World, 0, 10),
            )
            .with_parent(root.clone())
            .with_input_policy(LayerInputPolicy::Modal),
        )
        .expect("modal-like world inserts");
    assert_ne!(
        base,
        routing_hash(&modal_tree, &base_hits, &InteractionState::default())
    );

    let mut transformed_tree = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        layer_order(RenderPhase::Background, 0, 0),
    ));
    transformed_tree
        .insert(
            LayerNode::new(
                world,
                LayerKind::Activity,
                layer_order(RenderPhase::World, 0, 10),
            )
            .with_parent(root)
            .with_transform(LayerTransform::translation_milli(1_000, 0))
            .with_input_policy(LayerInputPolicy::HitTest),
        )
        .expect("transformed world inserts");
    assert_ne!(
        base,
        routing_hash(&transformed_tree, &base_hits, &InteractionState::default())
    );
}

#[test]
fn route_fingerprint_captures_routing_state_and_decision() {
    let root = layer_id("root");
    let world = layer_id("world");
    let target = interaction_target("activity.world");
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
        .with_parent(root)
        .with_input_policy(LayerInputPolicy::HitTest),
    )
    .expect("world inserts");
    let mut hits = HitTree::default();
    hits.push(HitRecord::new(
        world,
        target,
        HitRect::new(0.0, 0.0, 100.0, 100.0),
    ));

    let raw = RawInputEvent::new(
        InputEpoch(70),
        RawInputKind::Pointer(PointerInput {
            pointer: PointerId(1),
            position: ViewportPoint::new(10.0, 10.0),
            phase: PointerPhase::Down,
        }),
    );
    let routed = InputRouter::route(&raw, &tree, &hits, &InteractionState::default());
    let first = route_fingerprint(&tree, &hits, &InteractionState::default(), &routed);
    let second = route_fingerprint(&tree, &hits, &InteractionState::default(), &routed);
    assert_eq!(first, second);
    assert_eq!(
        first.routing_hash(),
        routing_hash(&tree, &hits, &InteractionState::default())
    );
    assert_eq!(first.raw_epoch(), InputEpoch(70));

    let miss = RawInputEvent::new(
        InputEpoch(70),
        RawInputKind::Pointer(PointerInput {
            pointer: PointerId(1),
            position: ViewportPoint::new(200.0, 200.0),
            phase: PointerPhase::Down,
        }),
    );
    let missed = InputRouter::route(&miss, &tree, &hits, &InteractionState::default());
    let missed_fingerprint = route_fingerprint(&tree, &hits, &InteractionState::default(), &missed);
    assert_eq!(first.routing_hash(), missed_fingerprint.routing_hash());
    assert_ne!(first.decision_hash(), missed_fingerprint.decision_hash());
}

#[test]
fn gesture_arena_completes_small_pointer_sequence_as_tap() {
    let target = interaction_target("choice.listen");
    let mut arena = GestureArena::default();
    arena.begin(
        PointerId(1),
        target.clone(),
        ViewportPoint::new(10.0, 10.0),
        vec![GestureKind::Tap, GestureKind::Drag],
    );
    assert_eq!(
        arena.update(PointerId(1), ViewportPoint::new(12.0, 11.0)),
        GestureOutcome::Pending
    );
    assert_eq!(
        arena.end(PointerId(1), ViewportPoint::new(12.0, 11.0)),
        GestureOutcome::Completed {
            pointer: PointerId(1),
            target,
            winner: Some(GestureKind::Tap)
        }
    );
    assert!(arena.sessions().is_empty());
}

#[test]
fn gesture_arena_resolves_drag_after_threshold() {
    let target = interaction_target("activity.drag");
    let mut arena = GestureArena::default();
    arena.begin(
        PointerId(2),
        target.clone(),
        ViewportPoint::new(0.0, 0.0),
        vec![GestureKind::Tap, GestureKind::Drag],
    );
    assert_eq!(
        arena.update(PointerId(2), ViewportPoint::new(9.0, 0.0)),
        GestureOutcome::Won {
            pointer: PointerId(2),
            target: target.clone(),
            winner: GestureKind::Drag
        }
    );
    assert_eq!(
        arena.end(PointerId(2), ViewportPoint::new(12.0, 0.0)),
        GestureOutcome::Completed {
            pointer: PointerId(2),
            target,
            winner: Some(GestureKind::Drag)
        }
    );
}

#[test]
fn gesture_arena_axis_scroll_beats_generic_drag() {
    let target = interaction_target("scroll.panel");
    let mut arena = GestureArena::default();
    arena.begin(
        PointerId(3),
        target.clone(),
        ViewportPoint::new(0.0, 0.0),
        vec![GestureKind::Drag, GestureKind::ScrollY],
    );
    assert_eq!(
        arena.update(PointerId(3), ViewportPoint::new(2.0, 10.0)),
        GestureOutcome::Won {
            pointer: PointerId(3),
            target,
            winner: GestureKind::ScrollY
        }
    );
}

#[test]
fn gesture_arena_cancel_reports_current_winner_and_removes_session() {
    let target = interaction_target("activity.drag");
    let mut arena = GestureArena::default();
    arena.begin(
        PointerId(4),
        target.clone(),
        ViewportPoint::new(0.0, 0.0),
        vec![GestureKind::Drag],
    );
    assert!(matches!(
        arena.update(PointerId(4), ViewportPoint::new(10.0, 0.0)),
        GestureOutcome::Won {
            winner: GestureKind::Drag,
            ..
        }
    ));
    assert_eq!(
        arena.cancel(PointerId(4)),
        GestureOutcome::Cancelled {
            pointer: PointerId(4),
            target,
            winner: Some(GestureKind::Drag)
        }
    );
    assert!(arena.sessions().is_empty());
}

#[test]
fn semantic_tree_lowers_textbox_and_activity_actions_to_shared_action_batch_targets() {
    let textbox_target = interaction_target("textbox.main");
    let activity_target = interaction_target("activity.truck");
    let advance = PublicId::try_new("action.advance").unwrap();
    let pause = PublicId::try_new("action.pause").unwrap();
    let mut semantics = SemanticTree::default();
    semantics.push(
        SemanticNode::new(
            layer_id("dialogue"),
            textbox_target.clone(),
            SemanticRole::TextBox,
            HitRect::new(0.0, 0.0, 640.0, 160.0),
        )
        .with_label("Dialogue")
        .with_action(advance.clone()),
    );
    semantics.push(
        SemanticNode::new(
            layer_id("activity"),
            activity_target.clone(),
            SemanticRole::Activity,
            HitRect::new(0.0, 0.0, 320.0, 240.0),
        )
        .with_label("Truck")
        .with_action(pause.clone()),
    );

    let textbox_action = semantics
        .lower_action(&textbox_target, &advance)
        .expect("textbox action lowers");
    assert_eq!(
        textbox_action.target(),
        &ActionTarget::Entity(textbox_target.clone())
    );
    assert_eq!(textbox_action.kind(), &advance);

    let activity_action = semantics
        .lower_action(&activity_target, &pause)
        .expect("activity action lowers");
    assert_eq!(
        activity_action.target(),
        &ActionTarget::Activity(activity_target)
    );
    assert_eq!(activity_action.kind(), &pause);
}

#[test]
fn semantic_tree_rejects_hidden_disabled_and_undeclared_actions() {
    let hidden_target = interaction_target("button.hidden");
    let disabled_target = interaction_target("button.disabled");
    let visible_target = interaction_target("button.visible");
    let select = PublicId::try_new("action.select").unwrap();
    let inspect = PublicId::try_new("action.inspect").unwrap();
    let mut semantics = SemanticTree::default();
    semantics.push(
        SemanticNode::new(
            layer_id("ui"),
            hidden_target.clone(),
            SemanticRole::Button,
            HitRect::new(0.0, 0.0, 20.0, 20.0),
        )
        .with_visible(false)
        .with_action(select.clone()),
    );
    semantics.push(
        SemanticNode::new(
            layer_id("ui"),
            disabled_target.clone(),
            SemanticRole::Button,
            HitRect::new(0.0, 0.0, 20.0, 20.0),
        )
        .with_enabled(false)
        .with_action(select.clone()),
    );
    semantics.push(
        SemanticNode::new(
            layer_id("ui"),
            visible_target.clone(),
            SemanticRole::Button,
            HitRect::new(0.0, 0.0, 20.0, 20.0),
        )
        .with_action(select.clone()),
    );

    assert_eq!(
        semantics.lower_action(&hidden_target, &select),
        Err(SemanticActionError::Hidden(hidden_target))
    );
    assert_eq!(
        semantics.lower_action(&disabled_target, &select),
        Err(SemanticActionError::Disabled(disabled_target))
    );
    assert_eq!(
        semantics.lower_action(&visible_target, &inspect),
        Err(SemanticActionError::UndeclaredAction {
            target: visible_target,
            action: inspect
        })
    );
}

#[test]
fn semantic_tree_hit_records_route_through_existing_layer_policy() {
    let root = layer_id("root");
    let world = layer_id("world");
    let modal = layer_id("modal");
    let world_target = interaction_target("activity.world");
    let modal_target = interaction_target("modal.close");
    let inspect = PublicId::try_new("action.inspect").unwrap();
    let close = PublicId::try_new("action.close").unwrap();
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

    let mut semantics = SemanticTree::default();
    semantics.push(
        SemanticNode::new(
            world,
            world_target.clone(),
            SemanticRole::Activity,
            HitRect::new(0.0, 0.0, 100.0, 100.0),
        )
        .with_action(inspect.clone()),
    );
    semantics.push(
        SemanticNode::new(
            modal.clone(),
            modal_target.clone(),
            SemanticRole::Button,
            HitRect::new(0.0, 0.0, 20.0, 20.0),
        )
        .with_action(close.clone()),
    );
    let hits = semantics.to_hit_tree();

    let modal_hit = RawInputEvent::new(
        InputEpoch(80),
        RawInputKind::Pointer(PointerInput {
            pointer: PointerId(1),
            position: ViewportPoint::new(5.0, 5.0),
            phase: PointerPhase::Down,
        }),
    );
    let routed = InputRouter::route(&modal_hit, &tree, &hits, &InteractionState::default());
    assert_eq!(routed.event().map(InputEvent::target), Some(&modal_target));

    let lower_hit = RawInputEvent::new(
        InputEpoch(81),
        RawInputKind::Pointer(PointerInput {
            pointer: PointerId(1),
            position: ViewportPoint::new(50.0, 50.0),
            phase: PointerPhase::Down,
        }),
    );
    let blocked = InputRouter::route(&lower_hit, &tree, &hits, &InteractionState::default());
    assert_eq!(blocked.decision(), &RouteDecision::BlockedByModal { modal });

    assert_eq!(
        semantics.route_and_lower_action(
            InputEpoch(82),
            &world_target,
            &inspect,
            &tree,
            &InteractionState::default(),
        ),
        Err(SemanticActionError::RejectedByRouter(
            RouteDecision::BlockedByModal {
                modal: layer_id("modal")
            }
        ))
    );

    let modal_action = semantics
        .route_and_lower_action(
            InputEpoch(83),
            &modal_target,
            &close,
            &tree,
            &InteractionState::default(),
        )
        .expect("modal semantic action routes");
    assert_eq!(modal_action.target(), &ActionTarget::Entity(modal_target));
    assert_eq!(modal_action.kind(), &close);
}
