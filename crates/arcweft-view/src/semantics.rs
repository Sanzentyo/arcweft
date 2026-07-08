//! View semantic fragment lowering into presentation semantics.

use crate::{NodeKey, SemanticSpecId, ViewError};
use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::layer::LayerId;
use arcweft_presentation::semantic::{SemanticNode, SemanticRole, SemanticTree};
use std::collections::BTreeSet;

/// Frame-local node identifier inside one flat View semantic fragment.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ViewNodeId(pub u32);

/// Flat retained semantic node produced by View view rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewSemanticNode {
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

/// Ordered semantic fragment emitted by View views before presentation merge.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewSemanticFragment {
    nodes: Vec<ViewSemanticNode>,
}

/// Builder for one flat View semantic fragment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewSemanticFragmentBuilder {
    nodes: Vec<ViewSemanticNode>,
    keys: BTreeSet<NodeKey>,
}

impl ViewSemanticNode {
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

impl ViewSemanticFragment {
    pub fn as_slice(&self) -> &[ViewSemanticNode] {
        &self.nodes
    }

    pub fn get(&self, id: SemanticSpecId) -> Option<&ViewSemanticNode> {
        self.nodes.get(id.0 as usize)
    }

    pub fn find_target(&self, target: &InteractionTarget) -> Option<&ViewSemanticNode> {
        self.nodes.iter().find(|node| node.target() == target)
    }

    pub fn to_semantic_tree(&self) -> SemanticTree {
        let mut tree = SemanticTree::default();
        for node in &self.nodes {
            tree.push(node.to_semantic_node());
        }
        tree
    }

    pub fn into_vec(self) -> Vec<ViewSemanticNode> {
        self.nodes
    }
}

impl ViewSemanticFragmentBuilder {
    pub fn push(&mut self, node: ViewSemanticNode) -> Result<ViewNodeId, ViewError> {
        if !self.keys.insert(node.key()) {
            return Err(ViewError::DuplicateNodeKey(node.key()));
        }
        let index = u32::try_from(self.nodes.len()).map_err(|_| ViewError::CapacityExceeded)?;
        self.nodes.push(node);
        Ok(ViewNodeId(index))
    }

    pub fn finish(self) -> ViewSemanticFragment {
        ViewSemanticFragment { nodes: self.nodes }
    }
}
