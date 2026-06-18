//! Sans I/O UI fragment data for Arcweft presentation integration.

use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::layer::LayerId;
use arcweft_presentation::semantic::{SemanticNode, SemanticRole, SemanticTree};
use std::collections::BTreeSet;
use thiserror::Error;

/// Stable component identifier resolved at bundle/load time.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComponentId(pub u32);

/// Stable key for one retained UI fragment node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeKey(pub u64);

/// Frame-local node identifier inside one flat UI semantic fragment.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct UiNodeId(pub u32);

/// Flat retained semantic node produced by UI component rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct UiSemanticNode {
    key: NodeKey,
    layer: LayerId,
    target: InteractionTarget,
    role: SemanticRole,
    bounds: HitRect,
    label: Option<String>,
    actions: Vec<PublicId>,
    enabled: bool,
    visible: bool,
}

/// Ordered semantic fragment emitted by UI components before presentation merge.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiSemanticFragment {
    nodes: Vec<UiSemanticNode>,
}

/// Builder for one flat UI semantic fragment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiSemanticFragmentBuilder {
    nodes: Vec<UiSemanticNode>,
    keys: BTreeSet<NodeKey>,
}

/// Error while building a UI semantic fragment.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum UiSemanticError {
    #[error("duplicate UI node key {0:?}")]
    DuplicateNodeKey(NodeKey),
    #[error("too many UI semantic nodes")]
    CapacityExceeded,
}

impl UiSemanticNode {
    pub fn new(
        key: NodeKey,
        layer: LayerId,
        target: InteractionTarget,
        role: SemanticRole,
        bounds: HitRect,
    ) -> Self {
        Self {
            key,
            layer,
            target,
            role,
            bounds,
            label: None,
            actions: Vec::new(),
            enabled: true,
            visible: true,
        }
    }

    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn with_action(mut self, action: PublicId) -> Self {
        self.actions.push(action);
        self
    }

    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub const fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub const fn key(&self) -> NodeKey {
        self.key
    }

    pub const fn layer(&self) -> &LayerId {
        &self.layer
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    pub const fn role(&self) -> SemanticRole {
        self.role
    }

    pub const fn bounds(&self) -> HitRect {
        self.bounds
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn actions(&self) -> &[PublicId] {
        &self.actions
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn visible(&self) -> bool {
        self.visible
    }

    fn to_semantic_node(&self) -> SemanticNode {
        let mut node = SemanticNode::new(
            self.layer.clone(),
            self.target.clone(),
            self.role,
            self.bounds,
        )
        .with_enabled(self.enabled)
        .with_visible(self.visible);
        if let Some(label) = &self.label {
            node = node.with_label(label.clone());
        }
        self.actions
            .iter()
            .cloned()
            .fold(node, SemanticNode::with_action)
    }
}

impl UiSemanticFragment {
    pub fn as_slice(&self) -> &[UiSemanticNode] {
        &self.nodes
    }

    pub fn to_semantic_tree(&self) -> SemanticTree {
        let mut tree = SemanticTree::default();
        for node in &self.nodes {
            tree.push(node.to_semantic_node());
        }
        tree
    }

    pub fn into_vec(self) -> Vec<UiSemanticNode> {
        self.nodes
    }
}

impl UiSemanticFragmentBuilder {
    pub fn push(&mut self, node: UiSemanticNode) -> Result<UiNodeId, UiSemanticError> {
        if !self.keys.insert(node.key()) {
            return Err(UiSemanticError::DuplicateNodeKey(node.key()));
        }
        let index =
            u32::try_from(self.nodes.len()).map_err(|_| UiSemanticError::CapacityExceeded)?;
        self.nodes.push(node);
        Ok(UiNodeId(index))
    }

    pub fn finish(self) -> UiSemanticFragment {
        UiSemanticFragment { nodes: self.nodes }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_presentation::input::{ActionTarget, InputEpoch};
    use arcweft_presentation::interaction::InteractionState;
    use arcweft_presentation::layer::{
        LayerInputPolicy, LayerKind, LayerNode, LayerOrder, LayerTree, RenderPhase,
    };

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
    fn ui_semantic_fragment_lowers_to_presentation_semantic_tree() {
        let ui_layer = layer_id("ui");
        let button_target = target("ui.confirm");
        let action = public_id("action.confirm");
        let mut builder = UiSemanticFragmentBuilder::default();
        let id = builder
            .push(
                UiSemanticNode::new(
                    NodeKey(10),
                    ui_layer,
                    button_target.clone(),
                    SemanticRole::Button,
                    HitRect::new(0.0, 0.0, 80.0, 24.0),
                )
                .with_label("Confirm")
                .with_action(action.clone()),
            )
            .unwrap();
        assert_eq!(id, UiNodeId(0));

        let tree = builder.finish().to_semantic_tree();
        let lowered = tree
            .lower_action(&button_target, &action)
            .expect("UI action lowers through presentation semantics");
        assert_eq!(lowered.target(), &ActionTarget::Entity(button_target));
        assert_eq!(lowered.kind(), &action);
    }

    #[test]
    fn ui_semantic_fragment_rejects_duplicate_node_keys() {
        let ui_layer = layer_id("ui");
        let mut builder = UiSemanticFragmentBuilder::default();
        builder
            .push(UiSemanticNode::new(
                NodeKey(1),
                ui_layer.clone(),
                target("ui.first"),
                SemanticRole::Button,
                HitRect::new(0.0, 0.0, 10.0, 10.0),
            ))
            .unwrap();

        assert_eq!(
            builder.push(UiSemanticNode::new(
                NodeKey(1),
                ui_layer,
                target("ui.second"),
                SemanticRole::Button,
                HitRect::new(10.0, 0.0, 10.0, 10.0),
            )),
            Err(UiSemanticError::DuplicateNodeKey(NodeKey(1)))
        );
    }

    #[test]
    fn ui_semantic_tree_routes_agent_invoke_through_layer_policy() {
        let root = layer_id("root");
        let ui = layer_id("ui");
        let button = target("ui.confirm");
        let action = public_id("action.confirm");
        let mut layers = LayerTree::new(LayerNode::new(
            root.clone(),
            LayerKind::Root,
            order(RenderPhase::Background, 0),
        ));
        layers
            .insert(
                LayerNode::new(ui.clone(), LayerKind::GameUi, order(RenderPhase::GameUi, 0))
                    .with_parent(root)
                    .with_input_policy(LayerInputPolicy::HitTest),
            )
            .unwrap();

        let mut builder = UiSemanticFragmentBuilder::default();
        builder
            .push(
                UiSemanticNode::new(
                    NodeKey(2),
                    ui,
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
}
