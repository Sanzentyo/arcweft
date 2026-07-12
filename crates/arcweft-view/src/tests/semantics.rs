use crate::{NodeKey, ViewError, ViewNodeId, ViewSemanticFragmentBuilder, ViewSemanticNode};
use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::{ActionTarget, InputEpoch, InteractionTarget};
use arcweft_presentation::interaction::InteractionState;
use arcweft_presentation::layer::{
    LayerId, LayerInputPolicy, LayerKind, LayerNode, LayerOrder, LayerTree, RenderPhase,
};
use arcweft_presentation::semantic::SemanticRole;

fn public_id(value: &str) -> PublicId {
    PublicId::try_new(value).unwrap()
}

fn layer_id(name: &str) -> LayerId {
    LayerId::new(public_id(&format!("layer.{name}")))
}

fn target(name: &str) -> InteractionTarget {
    InteractionTarget::new(public_id(&format!("target.{name}")))
}

fn order(phase: RenderPhase, z: i32) -> LayerOrder {
    LayerOrder {
        phase,
        z,
        stable_index: 0,
    }
}

#[test]
fn view_semantic_fragment_lowers_to_presentation_semantic_tree() {
    let view_layer = layer_id("view");
    let button_target = target("view.confirm");
    let action = public_id("action.confirm");
    let mut builder = ViewSemanticFragmentBuilder::default();
    let id = builder
        .push(
            ViewSemanticNode::new(
                NodeKey(10),
                view_layer,
                button_target.clone(),
                SemanticRole::Button,
                HitRect::new(0.0, 0.0, 80.0, 24.0),
            )
            .with_label("Confirm")
            .with_action(action.clone()),
        )
        .unwrap();
    assert_eq!(id, ViewNodeId(0));

    let tree = builder.finish().to_semantic_tree();
    let lowered = tree
        .lower_action(&button_target, &action)
        .expect("View action lowers through presentation semantics");
    assert_eq!(lowered.target(), &ActionTarget::Entity(button_target));
    assert_eq!(lowered.kind(), &action);
}

#[test]
fn view_semantic_fragment_rejects_duplicate_node_keys() {
    let view_layer = layer_id("view");
    let mut builder = ViewSemanticFragmentBuilder::default();
    builder
        .push(ViewSemanticNode::new(
            NodeKey(1),
            view_layer.clone(),
            target("view.first"),
            SemanticRole::Button,
            HitRect::new(0.0, 0.0, 10.0, 10.0),
        ))
        .unwrap();

    assert_eq!(
        builder.push(ViewSemanticNode::new(
            NodeKey(1),
            view_layer,
            target("view.second"),
            SemanticRole::Button,
            HitRect::new(10.0, 0.0, 10.0, 10.0),
        )),
        Err(ViewError::DuplicateNodeKey(NodeKey(1)))
    );
}

#[test]
fn view_semantic_tree_routes_agent_invoke_through_layer_policy() {
    let root = layer_id("root");
    let view = layer_id("view");
    let button = target("view.confirm");
    let action = public_id("action.confirm");
    let mut layers = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        order(RenderPhase::Background, 0),
    ));
    layers
        .insert(
            LayerNode::new(
                view.clone(),
                LayerKind::GameView,
                order(RenderPhase::GameView, 0),
            )
            .with_parent(root)
            .with_input_policy(LayerInputPolicy::HitTest),
        )
        .unwrap();

    let mut builder = ViewSemanticFragmentBuilder::default();
    builder
        .push(
            ViewSemanticNode::new(
                NodeKey(2),
                view,
                button.clone(),
                SemanticRole::Button,
                HitRect::new(0.0, 0.0, 80.0, 24.0),
            )
            .with_action(action.clone()),
        )
        .unwrap();

    let tree = builder.finish().to_semantic_tree();
    let lowered = tree
        .route_and_lower_action(
            InputEpoch(1),
            &button,
            &action,
            &layers,
            &InteractionState::default(),
        )
        .unwrap();
    assert_eq!(lowered.target(), &ActionTarget::Entity(button));
}
