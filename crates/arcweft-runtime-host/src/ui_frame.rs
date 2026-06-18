use arcweft_presentation::layer::{LayerId, LayerTree};
use arcweft_presentation::semantic::SemanticTree;
use arcweft_ui::{DisplayList, UiSemanticFragment};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// One committed UI layer payload after component rendering, layout, and semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct UiFrameLayer {
    layer: LayerId,
    display: DisplayList,
    semantics: SemanticTree,
}

/// Ordered UI frame payload ready for host renderer and Agent observation phases.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiFrameCommit {
    layers: Vec<UiFrameLayer>,
}

/// Builder that validates UI frame output against the committed `LayerTree`.
#[derive(Clone, Debug, PartialEq)]
pub struct UiFrameCommitBuilder {
    render_order: Vec<LayerId>,
    known_layers: BTreeSet<LayerId>,
    layers: BTreeMap<LayerId, UiFrameLayer>,
}

/// Error while collecting a UI frame commit.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum UiFrameCommitError {
    #[error("UI frame layer is not present in the committed LayerTree: {0:?}")]
    UnknownLayer(LayerId),
    #[error("UI frame layer was submitted more than once: {0:?}")]
    DuplicateLayer(LayerId),
}

impl UiFrameLayer {
    pub fn new(layer: LayerId, display: DisplayList, semantics: SemanticTree) -> Self {
        Self {
            layer,
            display,
            semantics,
        }
    }

    pub const fn layer(&self) -> &LayerId {
        &self.layer
    }

    pub const fn display(&self) -> &DisplayList {
        &self.display
    }

    pub const fn semantics(&self) -> &SemanticTree {
        &self.semantics
    }
}

impl UiFrameCommit {
    pub fn as_slice(&self) -> &[UiFrameLayer] {
        &self.layers
    }

    pub fn merged_semantics(&self) -> SemanticTree {
        let mut tree = SemanticTree::default();
        self.layers
            .iter()
            .flat_map(|layer| layer.semantics().as_slice().iter().cloned())
            .for_each(|node| tree.push(node));
        tree
    }

    pub fn into_vec(self) -> Vec<UiFrameLayer> {
        self.layers
    }
}

impl UiFrameCommitBuilder {
    pub fn new(layer_tree: &LayerTree) -> Self {
        Self {
            render_order: layer_tree.render_order().to_vec(),
            known_layers: layer_tree.render_order().iter().cloned().collect(),
            layers: BTreeMap::new(),
        }
    }

    pub fn push_layer(
        &mut self,
        layer: LayerId,
        display: DisplayList,
        semantics: &UiSemanticFragment,
    ) -> Result<(), UiFrameCommitError> {
        if !self.known_layers.contains(&layer) {
            return Err(UiFrameCommitError::UnknownLayer(layer));
        }
        if self.layers.contains_key(&layer) {
            return Err(UiFrameCommitError::DuplicateLayer(layer));
        }
        let frame_layer = UiFrameLayer::new(layer.clone(), display, semantics.to_semantic_tree());
        self.layers.insert(layer, frame_layer);
        Ok(())
    }

    pub fn finish(self) -> UiFrameCommit {
        let layers = self
            .render_order
            .into_iter()
            .filter_map(|layer| self.layers.get(&layer).cloned())
            .collect();
        UiFrameCommit { layers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PresentationActionDestination, dispatch_presentation_action};
    use arcweft_id::PublicId;
    use arcweft_presentation::hit::HitRect;
    use arcweft_presentation::input::{ActionTarget, InteractionTarget};
    use arcweft_presentation::layer::{LayerKind, LayerNode, LayerOrder, RenderPhase};
    use arcweft_presentation::semantic::SemanticRole;
    use arcweft_ui::{NodeKey, UiSemanticFragment, UiSemanticFragmentBuilder, UiSemanticNode};

    fn public_id(name: &str) -> PublicId {
        PublicId::try_new(name).unwrap()
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

    fn semantic_fragment(
        key: u64,
        layer: LayerId,
        target: InteractionTarget,
        action: PublicId,
    ) -> UiSemanticFragment {
        let mut builder = UiSemanticFragmentBuilder::default();
        builder
            .push(
                UiSemanticNode::new(
                    NodeKey(key),
                    layer,
                    target,
                    SemanticRole::Button,
                    HitRect::new(0.0, 0.0, 20.0, 20.0),
                )
                .with_action(action),
            )
            .unwrap();
        builder.finish()
    }

    #[test]
    fn ui_frame_commit_orders_layers_by_committed_layer_tree() {
        let root = layer_id("root");
        let ui = layer_id("ui");
        let modal = layer_id("modal");
        let mut layers = LayerTree::new(LayerNode::new(
            root.clone(),
            LayerKind::Root,
            order(RenderPhase::Background, 0),
        ));
        layers
            .insert(
                LayerNode::new(ui.clone(), LayerKind::GameUi, order(RenderPhase::GameUi, 0))
                    .with_parent(root.clone()),
            )
            .unwrap();
        layers
            .insert(
                LayerNode::new(
                    modal.clone(),
                    LayerKind::Modal,
                    order(RenderPhase::Modal, 0),
                )
                .with_parent(root),
            )
            .unwrap();

        let mut builder = UiFrameCommitBuilder::new(&layers);
        builder
            .push_layer(
                modal.clone(),
                DisplayList::default(),
                &semantic_fragment(
                    1,
                    modal.clone(),
                    target("modal.close"),
                    public_id("action.close"),
                ),
            )
            .unwrap();
        builder
            .push_layer(
                ui.clone(),
                DisplayList::default(),
                &semantic_fragment(
                    2,
                    ui.clone(),
                    target("ui.confirm"),
                    public_id("action.confirm"),
                ),
            )
            .unwrap();

        let commit = builder.finish();
        let committed_layers = commit
            .as_slice()
            .iter()
            .map(UiFrameLayer::layer)
            .collect::<Vec<_>>();
        assert_eq!(committed_layers, vec![&ui, &modal]);
    }

    #[test]
    fn ui_frame_commit_rejects_unknown_and_duplicate_layers_without_replacing_existing() {
        let root = layer_id("root");
        let ui = layer_id("ui");
        let missing = layer_id("missing");
        let mut layers = LayerTree::new(LayerNode::new(
            root.clone(),
            LayerKind::Root,
            order(RenderPhase::Background, 0),
        ));
        layers
            .insert(
                LayerNode::new(ui.clone(), LayerKind::GameUi, order(RenderPhase::GameUi, 0))
                    .with_parent(root),
            )
            .unwrap();

        let mut builder = UiFrameCommitBuilder::new(&layers);
        assert_eq!(
            builder.push_layer(
                missing.clone(),
                DisplayList::default(),
                &UiSemanticFragment::default(),
            ),
            Err(UiFrameCommitError::UnknownLayer(missing))
        );
        builder
            .push_layer(
                ui.clone(),
                DisplayList::default(),
                &semantic_fragment(1, ui.clone(), target("ui.first"), public_id("action.first")),
            )
            .unwrap();
        assert_eq!(
            builder.push_layer(
                ui.clone(),
                DisplayList::default(),
                &semantic_fragment(
                    2,
                    ui.clone(),
                    target("ui.second"),
                    public_id("action.second"),
                ),
            ),
            Err(UiFrameCommitError::DuplicateLayer(ui.clone()))
        );

        let commit = builder.finish();
        assert!(
            commit
                .merged_semantics()
                .find(&target("ui.first"))
                .is_some()
        );
        assert!(
            commit
                .merged_semantics()
                .find(&target("ui.second"))
                .is_none()
        );
    }

    #[test]
    fn ui_frame_commit_semantics_feed_presentation_dispatch() {
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
                    .with_parent(root),
            )
            .unwrap();

        let mut builder = UiFrameCommitBuilder::new(&layers);
        builder
            .push_layer(
                ui,
                DisplayList::default(),
                &semantic_fragment(1, layer_id("ui"), button.clone(), action.clone()),
            )
            .unwrap();
        let semantics = builder.finish().merged_semantics();
        let dispatched = dispatch_presentation_action(
            &semantics,
            semantics.lower_action(&button, &action).unwrap(),
        )
        .unwrap();

        assert_eq!(
            dispatched.destination(),
            PresentationActionDestination::UiEntity
        );
        assert_eq!(dispatched.action().target(), &ActionTarget::Entity(button));
    }
}
